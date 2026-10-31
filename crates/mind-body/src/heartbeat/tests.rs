//! Behaviour of the shared heartbeat through its ports: a fake brake, fake
//! mouths, real minds over temporary directories, a fixed clock. Every rule is
//! pinned by a test that fails when the rule breaks.
//!
//! The rig's clock is 1000 s past the epoch of its minds, and a Mind's turns are
//! admitted at the times a test names, so waits are exact. The default beat is
//! the tally's 900 s unless a test makes the Minds vote.

use std::cell::{Cell, RefCell};

use chrono::Duration;
use eureka_pipeline::PipelineKind as K;
use huginn_mind::PipelineSelectionPage;
use tempfile::TempDir;

use super::*;
use crate::control::{Brake, ControlState, SwarmState};
use crate::testkit::{Fixed, Recording, TestMind, at, close, recorded, reference, run_in};

struct Counting<'a> {
    inner: &'a TestMind,
    calls: &'a Cell<u32>,
    down: &'a Cell<bool>,
}

impl MindPort for Counting<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        if self.down.get() {
            return Err(anyhow!("the daemon is down"));
        }
        self.calls.set(self.calls.get() + 1);
        self.inner.query(selection)
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<eureka_pipeline::PipelineDocument>) -> Result<huginn_mind::PipelineAdmissionOutcome> {
        self.calls.set(self.calls.get() + 1);
        self.inner.admit(agent, session, documents)
    }
}

struct FakeControl {
    swarm: RefCell<SwarmState>,
    braked: RefCell<Vec<String>>,
}

impl FakeControl {
    fn new() -> Self {
        let brake = Brake { released: true, set_at: at(0), set_by: "op".into() };
        Self { swarm: RefCell::new(SwarmState { brake: Some(brake) }), braked: RefCell::default() }
    }
}

impl Control for FakeControl {
    fn envelope(&self) -> SwarmEffective {
        self.swarm.borrow().effective()
    }

    fn effective(&self, instance: &Slug) -> Effective {
        let brake = Brake { released: !self.braked.borrow().contains(&instance.0), set_at: at(0), set_by: "op".into() };
        ControlState { brake: Some(brake) }.effective(instance, self.envelope())
    }
}

#[derive(Default)]
struct FakeChat {
    deliveries: RefCell<Vec<ChatDelivery>>,
    pending_calls: Cell<u32>,
}

impl Chat for FakeChat {
    fn pending(&self, after_seq: u64) -> Result<Vec<ChatDelivery>> {
        self.pending_calls.set(self.pending_calls.get() + 1);
        Ok(self.deliveries.borrow().iter().filter(|delivery| delivery.seq > after_seq).cloned().collect())
    }

    fn send(&self, _: &crate::chat_client::ChatSendRequest) -> Result<crate::chat_client::ChatSendReceipt> {
        panic!("the heartbeat sends nothing")
    }

    fn ack(&self, _: u64) -> Result<()> {
        panic!("the heartbeat never acks a delivery")
    }
}

#[derive(Default)]
struct MemStore {
    state: RefCell<Option<HeartbeatState>>,
    traces: RefCell<Vec<PollTrace>>,
}

impl Store for MemStore {
    fn load(&self) -> Result<Option<HeartbeatState>> {
        Ok(self.state.borrow().clone())
    }

    fn save(&self, state: &HeartbeatState) -> Result<()> {
        *self.state.borrow_mut() = Some(state.clone());
        Ok(())
    }

    fn record(&self, trace: &PollTrace) -> Result<()> {
        self.traces.borrow_mut().push(trace.clone());
        Ok(())
    }
}

fn persona(slug: &str, name: &str) -> PersonaId {
    PersonaId { slug: Slug(slug.into()), public_name: name.into(), handle: slug.into(), matrix_id: format!("@{slug}:chat"), huginn: "127.0.0.1:4100".parse().unwrap() }
}

fn delivery(seq: u64, sender: &str, text: &str, mentions: &[&str]) -> ChatDelivery {
    ChatDelivery {
        seq,
        room_alias: "#swarm:chat".into(),
        room_id: "!swarm:chat".into(),
        event_id: format!("$e{seq}"),
        sender: sender.into(),
        sender_display: sender.into(),
        text: text.into(),
        at_unix_ms: 1,
        context: vec![],
        mentions: mentions.iter().map(|id| (*id).to_string()).collect(),
        is_direct: false,
    }
}

const ANN: usize = 0;
const BOB: usize = 1;
const NAMES: [&str; 2] = ["ann", "bob"];

struct Rig {
    _dir: TempDir,
    minds: Vec<TestMind>,
    calls: Vec<Cell<u32>>,
    down: Vec<Cell<bool>>,
    chats: Vec<FakeChat>,
    control: FakeControl,
    launcher: Recording,
    clock: Fixed,
    store: MemStore,
    config: Config,
    labels: Cell<u32>,
}

impl Rig {
    fn new() -> Box<Self> {
        Box::new(Self {
            _dir: TempDir::new().unwrap(),
            minds: vec![TestMind::seeded_as("ann"), TestMind::seeded_as("bob")],
            calls: vec![Cell::new(0), Cell::new(0)],
            down: vec![Cell::new(false), Cell::new(false)],
            chats: vec![FakeChat::default(), FakeChat::default()],
            control: FakeControl::new(),
            launcher: Recording::default(),
            clock: Fixed(Cell::new(at(1_000_000))),
            store: MemStore::default(),
            config: Config::new(vec![persona("ann", "Ann"), persona("bob", "Bob")], vec!["@op:chat".into()]).unwrap(),
            labels: Cell::new(0),
        })
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.0.get()
    }

    fn advance(&self, seconds: i64) {
        self.clock.0.set(self.now() + Duration::seconds(seconds));
    }

    fn say(&self, who: usize, delivery: ChatDelivery) {
        self.chats[who].deliveries.borrow_mut().push(delivery);
    }

    fn poll(&self) -> Outcome {
        let ports: Vec<Counting> = self.minds.iter().zip(self.calls.iter().zip(&self.down)).map(|(inner, (calls, down))| Counting { inner, calls, down }).collect();
        let seats: Vec<Seat> = self.config.personas.iter().enumerate().map(|(index, who)| Seat { who: who.clone(), mind: &ports[index], chat: &self.chats[index] }).collect();
        let organs = Organs { seats: &seats, control: &self.control, launcher: &self.launcher, clock: &self.clock, host: "test-host", store: &self.store };
        poll(&organs, &self.config).unwrap()
    }

    /// A finished Persona turn of `who`, admitted `ago` seconds before now.
    fn turn(&self, who: usize, ago: i64) -> String {
        let label = format!("old-{}", self.labels.get());
        self.labels.set(self.labels.get() + 1);
        self.minds[who].admit_at(self.now() - Duration::seconds(ago));
        self.minds[who].committed(vec![run_in(NAMES[who], &label, RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
        self.minds[who].committed(vec![close(reference(K::Run, &format!("{}:run:{label}", NAMES[who])), recorded())]);
        label
    }

    /// A finished Self run of `who`, ended `ago` seconds before now.
    fn self_run_ended(&self, who: usize, ago: i64) {
        let label = format!("self-{}", self.labels.get());
        self.labels.set(self.labels.get() + 1);
        self.minds[who].admit_at(self.now() - Duration::seconds(ago + 5));
        self.minds[who].committed(vec![run_in(NAMES[who], &label, RunTurn::SelfRun, RunOperator::Mind, &[])]);
        self.minds[who].admit_at(self.now() - Duration::seconds(ago));
        self.minds[who].committed(vec![close(reference(K::Run, &format!("{}:run:{label}", NAMES[who])), recorded())]);
    }

    /// `who` admits a pulse, `ago` seconds before now.
    fn pulse(&self, who: usize, ago: i64, beat: Option<u32>, urgency: Option<u16>) {
        self.minds[who].admit_at(self.now() - Duration::seconds(ago));
        let label = format!("p{}", self.labels.get());
        self.labels.set(self.labels.get() + 1);
        self.minds[who].committed(vec![eureka_pipeline::PipelineDocument::Pulse(eureka_pipeline::PipelinePulse {
            instance: Slug(NAMES[who].into()),
            label: eureka_pipeline::Label(label),
            tempo_vote_beat_s: beat,
            urgency_permille: urgency,
            note: "test".into(),
        })]);
    }

    fn runs(&self, who: usize) -> usize {
        self.minds[who].ids(K::Run).len()
    }

    fn launched(&self) -> Vec<Slug> {
        self.launcher.started.borrow().iter().map(|(slug, _)| slug.clone()).collect()
    }

    /// Ends the open Persona run of `who`, as its turn would.
    fn end_run(&self, who: usize) {
        let ids = self.minds[who].ids(K::Run);
        self.minds[who].committed(vec![close(reference(K::Run, ids.last().unwrap()), recorded())]);
    }

    fn total_calls(&self) -> u32 {
        self.calls.iter().map(Cell::get).sum::<u32>() + self.chats.iter().map(|chat| chat.pending_calls.get()).sum::<u32>()
    }
}

#[test]
fn each_persona_carries_her_own_huginn_and_a_missing_or_malformed_one_refuses_the_config() {
    let specs = "ann,ann,Ann,@ann:chat,rudp://127.0.0.1:4101; bob,bob,Bob Smith,@bob:chat,rudp://10.0.0.2:4102";
    let personas: Vec<PersonaId> = specs.split(';').map(PersonaId::parse).collect::<Result<_>>().unwrap();
    assert_eq!(personas.iter().map(|who| who.huginn.to_string()).collect::<Vec<_>>(), ["127.0.0.1:4101", "10.0.0.2:4102"]);
    for bad in ["ann,ann,Ann,@ann:chat", "ann,ann,Ann,@ann:chat,", "ann,ann,Ann,@ann:chat,127.0.0.1:4101", "ann,ann,Ann,@ann:chat,rudp://localhost:4101"] {
        assert!(PersonaId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_held_swarm_brake_reads_nothing() {
    let rig = Rig::new();
    rig.control.swarm.borrow_mut().brake.as_mut().unwrap().released = false;
    rig.say(ANN, delivery(1, "@op:chat", "Ann, please", &[]));
    assert_eq!(rig.poll(), Outcome::Held(HeldReason::BrakeHeld));
    assert_eq!(rig.total_calls(), 0, "no mind and no mouth was touched");
    assert!(rig.store.state.borrow().is_none() && rig.launched().is_empty());
}

#[test]
fn a_beat_launches_exactly_one_persona_the_one_who_has_waited_longest() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "both are idle and the beat is due, but one beat is one turn");
    assert_eq!((rig.runs(ANN), rig.runs(BOB)), (2, 1));
    assert_eq!(rig.launched(), vec![Slug("ann".into())]);
}

#[test]
fn the_next_beat_falls_a_beat_later_and_waiting_alone_round_robins() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    rig.poll();
    rig.end_run(ANN);
    rig.advance(60);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the default beat of 900 s has not passed");
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "bob has now waited longest");
    assert_eq!(rig.store.state.borrow().as_ref().unwrap().last_beat_at, rig.now());
}

#[test]
fn the_beat_is_the_lower_median_of_the_minds_newest_votes() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 100, Some(60), None);
    rig.pulse(BOB, 100, Some(86_400), None);
    rig.poll();
    assert_eq!(rig.store.traces.borrow().last().unwrap().tempo_s, 60, "the lower median of two votes is the lower");
    // Ann's newest vote replaces her old one; a pulse with no vote keeps it.
    rig.pulse(ANN, 50, Some(7200), None);
    rig.pulse(ANN, 40, None, Some(10));
    rig.advance(1);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.tempo_s, 7200);
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200));
}

#[test]
fn no_votes_is_the_default_beat() {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(rig.store.traces.borrow().last().unwrap().tempo_s, 900);
}

#[test]
fn a_self_declared_urgency_bends_the_pick_until_a_turn_consumes_it() {
    let rig = Rig::new();
    rig.turn(ANN, 1500);
    rig.turn(BOB, 1000);
    // Without urgency Ann (1500 s) beats Bob (1000 s). Bob declared a full urgency after his last turn: ten times the wait.
    rig.pulse(BOB, 500, None, Some(1000));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
    rig.end_run(BOB);
    rig.advance(900);
    // His turn came after the pulse and consumed it: Ann, who has waited longest, takes the beat.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn without_a_declared_urgency_the_longest_waiter_wins() {
    let rig = Rig::new();
    rig.turn(ANN, 1500);
    rig.turn(BOB, 1000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn urgency_at_one_order_is_worth_ten_times_the_wait() {
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    rig.pulse(BOB, 80, None, Some(1000));
    // Bob: 90 s with full urgency scores 0.1 x 10 = 1.0; Ann: 1000 s scores 1.11. Ann wins.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    rig.end_run(ANN);
    rig.advance(900);
    // Now Bob has waited 990 s at full urgency (9.9 beats x 10) against Ann's 900 s (1 beat): Bob wins.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
}

#[test]
fn the_operator_naming_a_persona_launches_her_at_once_off_the_beat_and_once_per_message() {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@op:chat", "Bob, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the beat has just fallen, and the operator does not wait for the next");
    assert_eq!(rig.store.state.borrow().as_ref().unwrap().last_beat_at, rig.now(), "an off-beat launch does not move the beat");
    // Her turn fails without acking; the same message does not launch her again.
    rig.end_run(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert_eq!(rig.runs(BOB), 3);
    // A tag in m.mentions counts as naming.
    rig.say(ANN, delivery(2, "@op:chat", "please look", &["@ann:chat"]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn a_member_naming_a_persona_launches_nobody_off_the_beat_but_counts_as_full_urgency_on_it() {
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@member:chat", "Bob, are you there?", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "a member does not start a turn");
    rig.advance(900);
    // Bob waited 990 s x ten for the member addressing him; Ann waited 1900 s x one: Bob wins the beat.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
}

#[test]
fn a_self_run_that_ended_since_her_last_turn_lets_her_start_the_next_at_once() {
    let rig = Rig::new();
    rig.turn(ANN, 500);
    rig.turn(BOB, 500);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.self_run_ended(ANN, 100);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "off the beat, for the Mind whose Self run ended");
    rig.end_run(ANN);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "her new turn is after the Self run: it was consumed");
}

#[test]
fn neither_an_operator_mention_nor_a_self_run_end_launches_a_busy_or_braked_persona() {
    let rig = Rig::new();
    rig.turn(ANN, 500);
    rig.turn(BOB, 500);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.control.braked.borrow_mut().push("ann".into());
    rig.say(ANN, delivery(1, "@op:chat", "Ann, now", &[]));
    rig.self_run_ended(BOB, 50);
    // Bob is busy: a Persona run is open.
    rig.minds[BOB].committed(vec![run_in("bob", "open-turn", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert_eq!(rig.launched(), vec![], "nothing started");
}

#[test]
fn a_braked_or_busy_persona_is_never_the_beat_winner() {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    rig.turn(BOB, 100);
    rig.control.braked.borrow_mut().push("ann".into());
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the longest waiter is braked");
    rig.end_run(BOB);
    rig.control.braked.borrow_mut().clear();
    rig.minds[ANN].committed(vec![run_in("ann", "open-turn", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the longest waiter is in a turn");
}

#[test]
fn a_persona_that_has_never_acted_goes_first() {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
}

#[test]
fn a_mind_that_cannot_be_read_is_skipped_and_the_others_are_served() {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    rig.turn(BOB, 100);
    rig.down[ANN].set(true);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert!(trace.instances[ANN].note.contains("could not be read"), "{:?}", trace.instances[ANN]);
}

#[test]
fn the_heartbeat_holds_no_usage_port_so_usage_cannot_change_a_launch() {
    // `Organs` has no connector and no readout: the only inputs are the brakes, the minds and the mouths.
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    let first = rig.poll();
    let again = Rig::new();
    again.turn(ANN, 3000);
    again.turn(BOB, 1000);
    assert_eq!(first, again.poll());
}
