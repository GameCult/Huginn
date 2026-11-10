//! Behaviour of the shared heartbeat through its ports: a fake brake, fake
//! mouths, real minds over temporary directories, a fixed clock. Every rule is
//! pinned by a test that fails when the rule breaks.
//!
//! The rig's clock is 1000 s past the epoch of its minds, and a Mind's turns are
//! admitted at the times a test names, so waits are exact. The default beat is
//! the tally's 900 s unless a test makes the Minds vote.

use std::cell::{Cell, RefCell};

use chrono::{Duration, TimeZone};
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
    /// Queries of a newest-shaped read (`ran`, `voted`, `urged`) that did not ask for one header.
    unbounded: &'a Cell<u32>,
    /// Reads of the stewardship kind, and whether they fail while everything else answers.
    stewardship_reads: &'a Cell<u32>,
    stewardship_down: &'a Cell<bool>,
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
        if selection.schemas.iter().flatten().any(|schema| schema == K::Stewardship.type_id()) {
            self.stewardship_reads.set(self.stewardship_reads.get() + 1);
            if self.stewardship_down.get() {
                return Err(anyhow!("the stewardship read failed"));
            }
        }
        let newest_shaped = selection.fields.iter().flatten().any(|predicate| matches!(predicate.index.as_str(), "ran" | "voted" | "urged"));
        if newest_shaped && selection.limit != Some(1) {
            self.unbounded.set(self.unbounded.get() + 1);
        }
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

    fn read(&self, _: &crate::chat_client::ChatRead) -> Result<()> {
        panic!("the heartbeat reads nothing")
    }
}

#[derive(Default)]
struct MemStore {
    state: RefCell<Option<HeartbeatState>>,
    traces: RefCell<Vec<PollTrace>>,
    addressing: RefCell<Option<Addressing>>,
    addressing_saves: Cell<u32>,
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

    fn load_addressing(&self) -> Result<Option<Addressing>> {
        Ok(self.addressing.borrow().clone())
    }

    fn save_addressing(&self, addressing: &Addressing) -> Result<()> {
        self.addressing_saves.set(self.addressing_saves.get() + 1);
        *self.addressing.borrow_mut() = Some(addressing.clone());
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
        image: None,
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
    unbounded: Vec<Cell<u32>>,
    stewardship_reads: Cell<u32>,
    stewardship_down: Cell<bool>,
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
            unbounded: vec![Cell::new(0), Cell::new(0)],
            stewardship_reads: Cell::new(0),
            stewardship_down: Cell::new(false),
            chats: vec![FakeChat::default(), FakeChat::default()],
            control: FakeControl::new(),
            launcher: Recording::default(),
            clock: Fixed(Cell::new(at(1_000_000))),
            store: MemStore::default(),
            config: Config::new(vec![persona("ann", "Ann"), persona("bob", "Bob")], vec!["@op:chat".into()], None).unwrap(),
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
        // Launches are admitted by the mind own clock; keep it on the rig clock so a launched turn is dated now.
        for mind in &self.minds {
            mind.admit_at(self.now());
        }
        let ports: Vec<Counting> = self.minds.iter().zip(self.calls.iter().zip(self.down.iter().zip(&self.unbounded))).map(|(inner, (calls, (down, unbounded)))| Counting { inner, calls, down, unbounded, stewardship_reads: &self.stewardship_reads, stewardship_down: &self.stewardship_down }).collect();
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
        let label = format!("p{}", self.labels.get());
        self.labels.set(self.labels.get() + 1);
        self.pulse_labelled(who, ago, &label, beat, urgency);
    }

    fn pulse_labelled(&self, who: usize, ago: i64, label: &str, beat: Option<u32>, urgency: Option<u16>) {
        self.minds[who].admit_at(self.now() - Duration::seconds(ago));
        let label = label.to_string();
        self.minds[who].committed(vec![eureka_pipeline::PipelineDocument::Pulse(eureka_pipeline::PipelinePulse {
            instance: Slug(NAMES[who].into()),
            label: eureka_pipeline::Label(label),
            tempo_vote_beat_s: beat,
            tempo_vote_withdrawn: false,
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
}

#[test]
fn a_minds_newest_vote_replaces_her_old_one_and_a_pulse_with_no_vote_keeps_it() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 100, Some(60), None);
    rig.pulse(ANN, 50, Some(7200), None);
    rig.pulse(ANN, 40, None, Some(10));
    rig.pulse(BOB, 100, Some(86_400), None);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.tempo_s, 7200);
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200));
}

#[test]
fn a_standing_vote_stands_until_she_replaces_it_whatever_else_she_writes() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 2000, Some(3600), None);
    for n in 1..=20 {
        rig.pulse(ANN, 2000 - n * 10, None, Some(1));
    }
    rig.pulse(BOB, 100, Some(86_400), None);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(3600));
    assert_eq!(trace.instances[ANN].urgency_permille, 1, "the newest declared urgency is read too");
    assert_eq!(trace.tempo_s, 3600);
}

#[test]
fn her_newest_vote_is_found_across_pages_of_pulses() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 2000, Some(600), None);
    for n in 1..=12 {
        rig.pulse(ANN, 2000 - n * 10, None, Some(1));
    }
    rig.pulse(ANN, 1000, Some(7200), None);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200));
}

#[test]
fn her_newest_vote_is_the_latest_admitted_not_the_highest_label() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse_labelled(ANN, 300, "2026-10-31-t9", Some(600), None);
    rig.pulse_labelled(ANN, 200, "2026-10-31-t10", Some(7200), None);
    rig.pulse_labelled(ANN, 100, "2026-10-31-t11", None, Some(5));
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200));
}

#[test]
fn a_mind_whose_launch_keeps_failing_neither_starves_the_beat_nor_reads_as_never_acted() {
    let rig = Rig::new();
    rig.turn(ANN, 5000);
    rig.turn(BOB, 1000);
    *rig.launcher.fail_for.borrow_mut() = Some(Slug("ann".into()));
    for beat in 1..=12 {
        rig.advance(1000);
        assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "beat {beat}");
        rig.end_run(BOB);
        let trace = rig.store.traces.borrow().last().cloned().unwrap();
        assert!(trace.instances[ANN].note.contains("launch"), "beat {beat}: {:?}", trace.instances[ANN]);
        assert!(trace.instances[ANN].idle_s >= 5000, "beat {beat}: she acted before; withdrawn launches do not erase that: {:?}", trace.instances[ANN]);
    }
    assert_eq!(rig.launched().iter().filter(|slug| slug.0 == "bob").count(), 12);
}

#[test]
fn a_mind_that_cannot_be_read_is_not_counted() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 100, Some(7200), None);
    rig.down[BOB].set(true);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200));
    assert_eq!(trace.tempo_s, 7200, "the unreadable Mind abstains: her 7200 is the only vote cast");
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
    assert_eq!(rig.runs(BOB), 2, "her old turn and the one launch, no second launch for the same message");
    assert_eq!(rig.launched(), vec![Slug("bob".into())]);
    // A tag in m.mentions counts as naming.
    rig.say(ANN, delivery(2, "@op:chat", "please look", &["@ann:chat"]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn another_mind_naming_a_persona_wakes_her_once_off_the_beat() {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@ann:chat", "Bob, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "a Mind of the roster wakes her at once");
    rig.end_run(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the same message polled again wakes nobody");
    assert_eq!(rig.runs(BOB), 2);
}

#[test]
fn a_mind_naming_no_one_or_herself_wakes_nobody() {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@ann:chat", "nothing to anyone", &[]));
    rig.say(BOB, delivery(2, "@bob:chat", "Bob reminds Bob", &["@bob:chat"]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
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
fn a_member_message_naming_nobody_of_hers_scores_nobody() {
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@member:chat", "good morning, swarm", &[]));
    rig.say(BOB, delivery(2, "@member:chat", "is BDS green?", &[]));
    rig.advance(900);
    // Ann waited 1900 s at one order; Bob 990 s, and a member naming nothing of his gives him no urgency: Ann wins the beat.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "BDS is nobody's here: no one stewards it");
}

/// `who` also stewards `repo`, with `names` listed or none.
fn steward(rig: &Rig, who: usize, repo: &str, names: &[&str]) {
    rig.minds[who].committed(vec![eureka_pipeline::PipelineDocument::Stewardship(eureka_pipeline::PipelineStewardship {
        instance: Slug(NAMES[who].into()),
        repo: repo.into(),
        sequence: 1,
        assigned_on: eureka_pipeline::Date("2026-11-06".into()),
        note: "assigned".into(),
        names: names.iter().map(|name| (*name).into()).collect(),
    })]);
}

/// A rig on the beat's far side, so only an off-beat wake can launch.
fn quiet_rig() -> Box<Rig> {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig
}

#[test]
fn the_operator_naming_a_project_untagged_wakes_its_idle_steward_once_off_the_beat() {
    let rig = quiet_rig();
    steward(&rig, ANN, "GameCult/BadDaySimulator", &["BDS"]);
    rig.say(ANN, delivery(1, "@op:chat", "BDS is down", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    rig.end_run(ANN);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the same message polled again launches nobody");
    rig.say(ANN, delivery(2, "@op:chat", "BDSX is down", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "a substring of a name is not the name");
}

#[test]
fn a_listed_name_wakes_on_its_exact_spelling_only_and_a_derived_name_in_any_case() {
    let rig = quiet_rig();
    steward(&rig, ANN, "GameCult/Thing", &["Thing"]);
    rig.say(ANN, delivery(1, "@op:chat", "one more thing", &[]));
    rig.say(ANN, delivery(2, "@op:chat", "THING", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "a listed name is exactly as written");
    rig.say(ANN, delivery(3, "@op:chat", "Thing is broken", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    let rig = quiet_rig();
    steward(&rig, BOB, "GameCult/BadDaySimulator", &[]);
    rig.say(BOB, delivery(1, "@op:chat", "is the BAD DAY SIMULATOR build green?", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "a derived name matches in any case");
}

#[test]
fn another_mind_naming_a_project_wakes_its_steward_and_a_member_scores_her_without_waking_her() {
    let rig = quiet_rig();
    steward(&rig, BOB, "GameCult/BadDaySimulator", &["BDS"]);
    rig.say(BOB, delivery(1, "@ann:chat", "BDS needs a look", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "a Mind of the roster naming her project wakes her");
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    steward(&rig, BOB, "GameCult/BadDaySimulator", &["BDS"]);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.say(BOB, delivery(1, "@member:chat", "is BDS green?", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "a member does not start a turn");
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the member's naming scores her at full urgency");
}

#[test]
fn a_stewardship_is_read_only_when_chat_is_pending_and_a_failed_read_keeps_her_roster_names() {
    let rig = quiet_rig();
    rig.poll();
    assert_eq!(rig.stewardship_reads.get(), 0, "nothing pending, nothing read");
    rig.stewardship_down.set(true);
    rig.say(ANN, delivery(1, "@op:chat", "Ann, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "her own name still wakes her");
    assert_eq!(rig.stewardship_reads.get(), 1, "one read for her one poll");
    assert!(notes_of(&rig).iter().any(|note| note.contains("stewardship names unread")), "{:?}", notes_of(&rig));
}

fn table_row(rig: &Rig, id: &str) -> Addressee {
    rig.store.addressing.borrow().as_ref().expect("the heartbeat wrote a table").minds.iter().find(|row| row.matrix_id() == id).cloned().expect("her row")
}

#[test]
fn the_heartbeat_writes_the_table_it_read_and_only_when_it_changed() {
    let rig = quiet_rig();
    steward(&rig, BOB, "GameCult/BadDaySimulator", &["BDS"]);
    rig.poll();
    assert_eq!(rig.store.addressing_saves.get(), 1, "the roster is written at the first poll");
    assert!(!addresses(&table_row(&rig, "@bob:chat"), &delivery(9, "@op:chat", "BDS is down", &[])), "nothing pending, nothing read: no project name yet");
    rig.say(BOB, delivery(1, "@member:chat", "hello", &[]));
    rig.poll();
    assert_eq!(rig.store.addressing_saves.get(), 2);
    assert!(addresses(&table_row(&rig, "@bob:chat"), &delivery(9, "@op:chat", "BDS is down", &[])));
    assert!(addresses(&table_row(&rig, "@ann:chat"), &delivery(9, "@op:chat", "Ann?", &[])) && !addresses(&table_row(&rig, "@ann:chat"), &delivery(9, "@op:chat", "BDS", &[])));
    rig.poll();
    assert_eq!(rig.store.addressing_saves.get(), 2, "an identical poll writes nothing");
}

#[test]
fn a_failed_stewardship_read_keeps_her_last_read_names_and_roster_names_when_none_are_stored() {
    let rig = quiet_rig();
    steward(&rig, BOB, "GameCult/BadDaySimulator", &["BDS"]);
    rig.say(BOB, delivery(1, "@member:chat", "hello", &[]));
    rig.poll();
    rig.stewardship_down.set(true);
    rig.say(BOB, delivery(2, "@op:chat", "BDS is down", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "her stored names still wake her");
    assert!(notes_of(&rig).iter().any(|note| note.contains("stewardship names unread")), "{:?}", notes_of(&rig));
    assert!(addresses(&table_row(&rig, "@bob:chat"), &delivery(9, "@op:chat", "BDS", &[])), "the table keeps her names");
    let rig = quiet_rig();
    steward(&rig, BOB, "GameCult/BadDaySimulator", &["BDS"]);
    rig.stewardship_down.set(true);
    rig.say(BOB, delivery(1, "@op:chat", "BDS is down", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "nothing stored: roster names only");
    rig.say(BOB, delivery(2, "@op:chat", "bob, a word", &[]));
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
    rig.launcher.revive_as(&Slug("bob".into()), RunTurn::PersonaTurn);
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
    rig.launcher.revive_as(&Slug("ann".into()), RunTurn::PersonaTurn);
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

/// A document of the dollar-initiative days, standing in for what an old `heartbeat.cc` holds.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.initiative.v1", schema = "OldInitiative")]
struct OldInitiative {
    #[cultcache(key = 0)]
    initiative: String,
}

fn trace_at(seconds: i64, tempo_s: u32) -> PollTrace {
    PollTrace { at: crate::testkit::at(seconds), tempo_s, instances: vec![], launches: vec!["ann".into()] }
}

#[test]
fn the_file_store_keeps_the_state_and_the_traces_and_the_tempo_view_reads_the_newest_trace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("heartbeat.cc");
    let store = FileStore::new(&path);
    store.check().unwrap();
    assert_eq!((store.load().unwrap(), store.latest().unwrap()), (None, None), "no file is no state");
    let state = HeartbeatState { last_beat_at: crate::testkit::at(5), handled_mentions: vec!["ann:e1".into()] };
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap(), Some(state));
    store.record(&trace_at(20, 600)).unwrap();
    store.record(&trace_at(10, 900)).unwrap();
    assert_eq!(store.latest().unwrap().map(|trace| trace.tempo_s), Some(600), "the newest by time, not by write order");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644, "Persona units read it");
    }
}

#[test]
fn an_old_heartbeat_store_fails_closed_naming_the_file_and_is_never_migrated() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("heartbeat.cc");
    let mut cache = CultCache::new();
    cache.register_entry_type::<OldInitiative>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put("eureka", &OldInitiative { initiative: "3.5".into() }).unwrap();
    let store = FileStore::new(&path);
    for result in [store.check(), store.load().map(|_| ()), store.latest().map(|_| ()), store.save(&HeartbeatState { last_beat_at: crate::testkit::at(1), handled_mentions: vec![] }), store.record(&trace_at(1, 900))] {
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains(&path.display().to_string()) && error.contains("archive it, nothing migrates it"), "{error}");
    }
}

/// How many queries a poll asks of Ann's mind once she holds `pulses` pulses of
/// one kind (`vote` or `urgency`): the count must not depend on the history.
fn ann_queries_with(pulses: usize, beat: Option<u32>, urgency: Option<u16>) -> u32 {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    rig.turn(BOB, 100_000);
    let mut done = 0;
    while done < pulses {
        let batch = (pulses - done).min(50);
        rig.minds[ANN].admit_at(rig.now() - Duration::seconds(90_000));
        rig.minds[ANN].committed(
            (0..batch)
                .map(|n| {
                    eureka_pipeline::PipelineDocument::Pulse(eureka_pipeline::PipelinePulse {
                        instance: Slug("ann".into()),
                        label: eureka_pipeline::Label(format!("c{}", done + n)),
                        tempo_vote_beat_s: beat,
                        tempo_vote_withdrawn: false,
                        urgency_permille: urgency,
                        note: "cost".into(),
                    })
                })
                .collect(),
        );
        done += batch;
    }
    let before = rig.calls[ANN].get();
    rig.poll();
    rig.calls[ANN].get() - before
}

#[test]
fn a_poll_costs_the_same_queries_at_any_pulse_history_length() {
    for (beat, urgency) in [(Some(600), None), (None, Some(1))] {
        let sizes = [200, 800, 5000].map(|pulses| ann_queries_with(pulses, beat, urgency));
        assert_eq!(sizes[0], sizes[1], "{beat:?}/{urgency:?}: {sizes:?}");
        assert_eq!(sizes[1], sizes[2], "{beat:?}/{urgency:?}: {sizes:?}");
    }
}

#[test]
fn a_poll_costs_the_same_queries_after_a_hundred_withdrawn_launches() {
    let rig = Rig::new();
    rig.turn(ANN, 5000);
    rig.turn(BOB, 1000);
    *rig.launcher.fail_for.borrow_mut() = Some(Slug("ann".into()));
    let mut asked = Vec::new();
    for beat in 1..=101 {
        rig.advance(1000);
        let before = rig.calls[ANN].get();
        rig.poll();
        rig.end_run(BOB);
        if beat == 1 || beat == 101 {
            asked.push(rig.calls[ANN].get() - before);
        }
    }
    assert_eq!(asked[0], asked[1], "a poll after 100 withdrawn launches asks as much as the first");
}

#[test]
fn her_standing_vote_is_the_newest_by_admission_not_by_the_clock() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 100, Some(600), None);
    rig.pulse(ANN, 200, Some(7200), None);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].vote_beat_s, Some(7200), "admitted second, stamped earlier");
}

#[test]
fn a_null_vote_withdraws_the_standing_one_and_an_urgency_only_pulse_leaves_it() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 300, Some(60), None);
    rig.pulse(ANN, 200, None, Some(5));
    rig.poll();
    let kept = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(kept.instances[ANN].vote_beat_s, Some(60), "an urgency-only pulse leaves her vote standing");
    rig.minds[ANN].admit_at(rig.now() - Duration::seconds(100));
    rig.minds[ANN].committed(vec![eureka_pipeline::PipelineDocument::Pulse(eureka_pipeline::PipelinePulse {
        instance: Slug("ann".into()),
        label: eureka_pipeline::Label("withdraw".into()),
        tempo_vote_beat_s: None,
        tempo_vote_withdrawn: true,
        urgency_permille: None,
        note: "test".into(),
    })]);
    rig.advance(5000);
    rig.poll();
    let withdrawn = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(withdrawn.instances[ANN].vote_beat_s, None, "an explicit null withdraws it");
    assert_eq!(withdrawn.tempo_s, 900, "she abstains again");
}

#[test]
fn an_urgency_is_consumed_by_a_turn_admitted_after_it_by_ordinal_not_by_the_clock() {
    // Same second, urgency first: the turn consumed it.
    let rig = Rig::new();
    rig.pulse(ANN, 3000, None, Some(900));
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].urgency_permille, 0, "a declaration admitted before her turn is consumed");
    // Same second, turn first: the urgency is pending.
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 3000, None, Some(900));
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].urgency_permille, 900, "a declaration admitted after her turn is pending, whatever second it is stamped");
    // Admitted after her turn but stamped earlier than it: still pending.
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    rig.pulse(ANN, 4000, None, Some(700));
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].urgency_permille, 700, "the clock stepping back does not consume it");
}

#[test]
fn an_urgency_admitted_in_the_batch_of_her_turn_is_consumed_by_it() {
    let rig = Rig::new();
    rig.turn(BOB, 3000);
    rig.minds[ANN].admit_at(rig.now() - Duration::seconds(3000));
    rig.minds[ANN].committed(vec![
        run_in(NAMES[ANN], "joint", RunTurn::PersonaTurn, RunOperator::Mind, &[]),
        eureka_pipeline::PipelineDocument::Pulse(eureka_pipeline::PipelinePulse {
            instance: Slug(NAMES[ANN].into()),
            label: eureka_pipeline::Label("joint-p".into()),
            tempo_vote_beat_s: None,
            tempo_vote_withdrawn: false,
            urgency_permille: Some(800),
            note: "test".into(),
        }),
    ]);
    rig.minds[ANN].committed(vec![close(reference(K::Run, &format!("{}:run:joint", NAMES[ANN])), recorded())]);
    rig.poll();
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.instances[ANN].urgency_permille, 0, "one receipt is one ordinal: a declaration is pending only when admitted strictly after her turn");
}

#[test]
fn every_newest_read_asks_for_exactly_one_header() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 3000);
    for n in 0..5 {
        rig.pulse(ANN, 2000 - n, Some(600 + u32::try_from(n).unwrap()), None);
        rig.pulse(ANN, 1000 - n, None, Some(3));
    }
    rig.poll();
    assert_eq!(rig.unbounded[ANN].get(), 0, "the ran, voted and urged reads each take limit 1");
    let queries = rig.calls[ANN].get();
    assert!(queries >= 3, "the poll did read her mind ({queries} queries)");
}

// ---- the beat gives a Self run to a Mind whose Self holds briefs ----------------------

use eureka_pipeline::PipelineDocument;

/// A self-run follow-up for `who`, admitted under `agent`'s receipt.
fn brief_by(rig: &Rig, who: usize, agent: &str, label: &str, source: &PipelineRef) {
    let PipelineDocument::FollowUp(mut document) = crate::testkit::follow_up(label, source) else { unreachable!() };
    document.owner = "self-run".into();
    let outcome = rig.minds[who].admit(agent, "session-a", vec![PipelineDocument::FollowUp(document)]).unwrap();
    assert!(matches!(outcome, huginn_mind::PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
}

/// Two Minds that have both acted, ann longest ago, and a Persona turn of hers to cite.
fn rig_with_a_persona_turn() -> (Box<Rig>, PipelineRef) {
    let rig = Rig::new();
    let label = rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    let source = reference(K::Run, &format!("ann:run:{label}"));
    (rig, source)
}

fn self_runs(rig: &Rig, who: usize) -> Vec<eureka_pipeline::PipelineRun> {
    rig.minds[who]
        .ids(K::Run)
        .iter()
        .filter_map(|id| match rig.minds[who].view(K::Run, id)?.document {
            PipelineDocument::Run(run) if run.turn == RunTurn::SelfRun => Some(run),
            _ => None,
        })
        .collect()
}

#[test]
fn the_beat_won_by_a_mind_whose_self_holds_briefs_is_one_self_run_claiming_them_and_the_outcome_says_so() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "first", &source);
    brief_by(&rig, ANN, crate::persona::AGENT, "second", &source);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "the launch is in the outcome, not only the trace");
    let runs = self_runs(&rig, ANN);
    assert_eq!(runs.len(), 1, "one Self run for both briefs");
    let claimed: Vec<String> = runs[0].claims.iter().map(|claim| claim.id.0.clone()).collect();
    assert_eq!(claimed, vec!["eureka-body:follow_up:first".to_string(), "eureka-body:follow_up:second".to_string()]);
    assert_eq!(rig.launcher.started.borrow().iter().filter(|(_, turn)| *turn == RunTurn::SelfRun).count(), 1);
    assert!(self_runs(&rig, BOB).is_empty(), "bob holds no brief");
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!(trace.launches.len(), 1);
    assert!(!trace.instances[ANN].busy, "a Self run is not her Persona's turn: she is not busy");
}

#[test]
fn a_beat_won_by_a_mind_with_no_brief_is_a_persona_turn_and_marks_her_busy() {
    let (rig, _source) = rig_with_a_persona_turn();
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert!(self_runs(&rig, ANN).is_empty());
    assert!(rig.store.traces.borrow().last().unwrap().instances[ANN].busy);
}

#[test]
fn a_self_run_waits_for_the_beat_like_any_launch() {
    let (rig, source) = rig_with_a_persona_turn();
    rig.poll();
    brief_by(&rig, ANN, crate::persona::AGENT, "waiting", &source);
    rig.end_run(ANN);
    rig.advance(60);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the beat has not passed: no Self run starts");
    assert!(self_runs(&rig, ANN).is_empty());
    rig.advance(900);
    let Outcome::Polled { launched } = rig.poll() else { panic!("held") };
    assert_eq!(launched.len(), 1, "one launch for the beat");
}

#[test]
fn a_live_self_run_holds_the_briefs_and_the_beat_is_a_persona_turn() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "taken", &source);
    rig.poll();
    assert_eq!(self_runs(&rig, ANN).len(), 1);
    rig.advance(1000);
    rig.poll();
    assert_eq!(self_runs(&rig, ANN).len(), 1, "a live Self run is never given a second");
}

#[test]
fn a_follow_up_her_persona_did_not_admit_launches_no_self_run_whatever_source_it_names() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::self_run::AGENT, "hers", &source);
    brief_by(&rig, ANN, "test", "by-hand", &source);
    rig.poll();
    assert!(self_runs(&rig, ANN).is_empty());
}

#[test]
fn a_braked_mind_launches_no_self_run() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "waiting", &source);
    rig.control.braked.borrow_mut().push("ann".into());
    rig.poll();
    assert!(self_runs(&rig, ANN).is_empty());
}

#[test]
fn a_brief_that_keeps_failing_trips_once_and_a_trip_is_a_stop_until_the_question_is_answered() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "stuck", &source);
    let item = reference(K::FollowUp, "eureka-body:follow_up:stuck");
    // Three Self runs have ended on it, each leaving nothing.
    for n in 0..3 {
        let label = format!("failed-{n}");
        rig.minds[ANN].committed(vec![run_in("ann", &label, RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
        rig.minds[ANN].committed(vec![close(reference(K::Run, &format!("ann:run:{label}")), recorded())]);
    }
    let before = self_runs(&rig, ANN).len();
    // Beat after beat, whoever launches: no new Self run on the brief, and the breaker asks once.
    for _ in 0..8 {
        rig.advance(1000);
        rig.poll();
        for who in [ANN, BOB] {
            if rig.minds[who].ids(K::Run).last().is_some_and(|id| rig.minds[who].view(K::Run, id).is_some_and(|view| matches!(view.status, huginn_mind::PipelineStatus::InForce))) {
                rig.end_run(who);
            }
        }
    }
    assert_eq!(self_runs(&rig, ANN).len(), before, "no Self run after the trip");
    assert_eq!(rig.minds[ANN].ids(K::Question).len(), 1, "the breaker asked once; an open question is not asked again nor a fresh count");
}

/// Three Self runs that each claimed every one of `items` and ended leaving nothing.
fn fail_bundle(rig: &Rig, items: &[PipelineRef]) {
    for n in 0..3 {
        let label = format!("failed-{n}");
        rig.minds[ANN].committed(vec![run_in("ann", &label, RunTurn::SelfRun, RunOperator::Mind, items)]);
        rig.minds[ANN].committed(vec![close(reference(K::Run, &format!("ann:run:{label}")), recorded())]);
    }
}

/// Beats on which every live run ends at once; the Self runs and questions the Mind has after them.
fn beats(rig: &Rig, count: usize) {
    for _ in 0..count {
        rig.advance(1000);
        rig.poll();
        for who in [ANN, BOB] {
            if rig.minds[who].ids(K::Run).last().is_some_and(|id| rig.minds[who].view(K::Run, id).is_some_and(|view| matches!(view.status, huginn_mind::PipelineStatus::InForce))) {
                rig.end_run(who);
            }
        }
    }
}

#[test]
fn one_cause_failing_a_bundle_of_briefs_is_one_question_not_one_per_brief() {
    let (rig, source) = rig_with_a_persona_turn();
    let labels = ["a", "b", "c", "d", "e"];
    let items: Vec<PipelineRef> = labels.iter().map(|label| reference(K::FollowUp, &format!("eureka-body:follow_up:{label}"))).collect();
    for label in labels {
        brief_by(&rig, ANN, crate::persona::AGENT, label, &source);
    }
    fail_bundle(&rig, &items);
    let before = self_runs(&rig, ANN).len();
    beats(&rig, 10);
    assert_eq!(rig.minds[ANN].ids(K::Question).len(), 1, "five briefs, one cause, one question on the desk");
    assert_eq!(self_runs(&rig, ANN).len(), before, "and no Self run launches while it is open");
    assert!(rig.minds[BOB].ids(K::Question).is_empty(), "another Mind is not stopped");
    let question = rig.minds[ANN].ids(K::Question).pop().unwrap();
    assert_eq!(question_raised_in(&rig, &question), Some(reference(K::Run, "ann:run:failed-2")), "raised in the run whose end tripped it, not in a brief");
}

/// Five briefs a..e of ann's Self, and the three Self runs that each claimed all of them and ended empty.
fn failed_bundle(rig: &Rig, source: &PipelineRef) -> Vec<PipelineRef> {
    let items: Vec<PipelineRef> = ["a", "b", "c", "d", "e"].iter().map(|label| brief_ref(label)).collect();
    for label in ["a", "b", "c", "d", "e"] {
        brief_by(rig, ANN, crate::persona::AGENT, label, source);
    }
    fail_bundle(rig, &items);
    items
}

fn brief_ref(label: &str) -> PipelineRef {
    reference(K::FollowUp, &format!("eureka-body:follow_up:{label}"))
}

fn question_raised_in(rig: &Rig, question: &str) -> Option<PipelineRef> {
    let PipelineDocument::Question(asked) = rig.minds[ANN].view(K::Question, question)?.document else { return None };
    asked.raised_in
}

fn claims_of_run(run: &eureka_pipeline::PipelineRun) -> Vec<String> {
    run.claims.iter().map(|claim| claim.id.0.clone()).collect()
}

/// Beats until `done`, at most `limit`; which beat a Mind wins is not the test's business.
fn beats_until(rig: &Rig, limit: usize, done: impl Fn() -> bool) {
    for _ in 0..limit {
        if done() {
            return;
        }
        beats(rig, 1);
    }
}

/// Answers a breaker question of ann by a ruling, or closes it Withdrawn.
fn lift(rig: &Rig, question: &str, withdraw: bool) {
    let outcome = if withdraw {
        close(reference(K::Question, question), crate::testkit::withdrawn())
    } else {
        crate::testkit::ruling("answer", Some((question, "reopen")))
    };
    rig.minds[ANN].committed(vec![outcome]);
}

#[test]
fn an_answer_or_a_withdrawal_reopens_the_whole_bundle_with_a_fresh_count() {
    for withdraw in [false, true] {
        let (rig, source) = rig_with_a_persona_turn();
        let items = failed_bundle(&rig, &source);
        beats(&rig, 3);
        let question = rig.minds[ANN].ids(K::Question).pop().expect("the trip asked");
        assert_eq!(self_runs(&rig, ANN).len(), 3, "the trip launched nothing");
        lift(&rig, &question, withdraw);
        // The next Self run claims all five; three empty ones after the boundary trip once more, never before.
        beats_until(&rig, 12, || self_runs(&rig, ANN).len() == 4);
        let runs = self_runs(&rig, ANN);
        assert_eq!(runs.len(), 4, "one Self run after the answer (withdrawn: {withdraw})");
        assert_eq!(claims_of_run(&runs[3]), items.iter().map(|item| item.id.0.clone()).collect::<Vec<_>>(), "all five in one run");
        beats_until(&rig, 24, || self_runs(&rig, ANN).len() == 6);
        assert_eq!((self_runs(&rig, ANN).len(), rig.minds[ANN].ids(K::Question).len()), (6, 1), "three empty runs since the answer, and no second question yet");
        beats_until(&rig, 24, || rig.minds[ANN].ids(K::Question).len() == 2);
        beats(&rig, 6);
        assert_eq!((self_runs(&rig, ANN).len(), rig.minds[ANN].ids(K::Question).len()), (6, 2), "the fourth launch trips once more, once, and launches nothing");
    }
}

#[test]
fn an_open_question_raised_elsewhere_stops_no_brief() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "b1", &source);
    brief_by(&rig, ANN, crate::persona::AGENT, "b2", &source);
    // A follow-up that is no brief, a cut spec, and a run that claimed only other items.
    rig.minds[ANN].committed(vec![crate::testkit::follow_up("plain", &source), crate::testkit::spec("c1")]);
    rig.minds[ANN].committed(vec![run_in("ann", "other", RunTurn::SelfRun, RunOperator::Mind, &[crate::testkit::spec_ref("c1")])]);
    rig.minds[ANN].committed(vec![close(reference(K::Run, "ann:run:other"), recorded())]);
    for (label, place) in [("q1", reference(K::FollowUp, "eureka-body:follow_up:plain")), ("q2", crate::testkit::spec_ref("c1")), ("q3", reference(K::Run, "ann:run:other"))] {
        rig.minds[ANN].committed(vec![crate::testkit::question_in(label, &place)]);
    }
    let before = self_runs(&rig, ANN).len();
    beats_until(&rig, 6, || self_runs(&rig, ANN).len() > before);
    let runs = self_runs(&rig, ANN);
    assert_eq!(runs.len(), before + 1, "the successor launches as usual");
    assert_eq!(claims_of_run(runs.last().unwrap()), vec![brief_ref("b1").id.0, brief_ref("b2").id.0], "both briefs");
}

#[test]
fn a_question_raised_in_one_brief_holds_only_that_brief() {
    let (rig, source) = rig_with_a_persona_turn();
    for label in ["b1", "b2", "b3"] {
        brief_by(&rig, ANN, crate::persona::AGENT, label, &source);
    }
    rig.minds[ANN].committed(vec![crate::testkit::question_in("fork", &brief_ref("b2"))]);
    let told: Vec<String> = inbox::derive(&rig.minds[ANN], &HashSet::new()).unwrap().into_iter().map(|brief| brief.id).collect();
    assert_eq!(told, vec![brief_ref("b1").id.0, brief_ref("b3").id.0], "derive omits b2");
    beats_until(&rig, 6, || !self_runs(&rig, ANN).is_empty());
    assert_eq!(claims_of_run(&self_runs(&rig, ANN)[0]), vec![brief_ref("b1").id.0, brief_ref("b3").id.0]);
}

#[test]
fn a_resolved_tripped_brief_drops_out_and_the_question_holds_the_rest_and_nothing_new() {
    let (rig, source) = rig_with_a_persona_turn();
    let items = failed_bundle(&rig, &source);
    beats(&rig, 3);
    assert_eq!(rig.minds[ANN].ids(K::Question).len(), 1);
    rig.minds[ANN].committed(vec![close(items[0].clone(), recorded())]);
    assert!(inbox::derive(&rig.minds[ANN], &HashSet::new()).unwrap().is_empty(), "the rest stay held and untold");
    for item in &items[1..] {
        rig.minds[ANN].committed(vec![close(item.clone(), recorded())]);
    }
    brief_by(&rig, ANN, crate::persona::AGENT, "new", &source);
    beats_until(&rig, 6, || self_runs(&rig, ANN).len() > 3);
    let runs = self_runs(&rig, ANN);
    assert_eq!(runs.len(), 4, "a new brief launched");
    assert_eq!(claims_of_run(&runs[3]), vec![brief_ref("new").id.0], "no Self run claims any of the five");
    beats(&rig, 2);
    assert_eq!(rig.minds[ANN].ids(K::Question).len(), 1, "the question stays in force and no second is asked");
}

#[test]
fn a_brief_admitted_while_a_question_is_open_is_claimed_and_told_alone() {
    let (rig, source) = rig_with_a_persona_turn();
    failed_bundle(&rig, &source);
    beats(&rig, 3);
    brief_by(&rig, ANN, crate::persona::AGENT, "fresh", &source);
    let told: Vec<String> = inbox::derive(&rig.minds[ANN], &HashSet::new()).unwrap().into_iter().map(|brief| brief.id).collect();
    assert_eq!(told, vec![brief_ref("fresh").id.0], "the start tell and the mid-session tells read this: none of the held briefs");
    beats_until(&rig, 6, || self_runs(&rig, ANN).len() > 3);
    let runs = self_runs(&rig, ANN);
    assert_eq!(runs.len(), 4);
    assert_eq!(claims_of_run(&runs[3]), vec![brief_ref("fresh").id.0], "claimed alone");
}

#[test]
fn a_trip_and_a_wait_say_so_in_the_trace() {
    let (rig, source) = rig_with_a_persona_turn();
    failed_bundle(&rig, &source);
    let notes = || rig.store.traces.borrow().iter().flat_map(|trace| trace.instances.iter().map(|instance| instance.note.clone())).collect::<Vec<_>>();
    beats_until(&rig, 6, || !rig.minds[ANN].ids(K::Question).is_empty());
    let question = rig.minds[ANN].ids(K::Question).pop().unwrap();
    let expected = format!("tripped: {question} holds the 5 claims of ann:run:failed-2");
    assert!(notes().contains(&expected), "{:?}", notes());
    let waiting = "5 briefs held by open questions".to_string();
    beats_until(&rig, 12, || notes().contains(&waiting));
    assert!(notes().contains(&waiting), "{:?}", notes());
}

#[test]
fn a_beat_with_no_held_brief_writes_no_held_note() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "free", &source);
    rig.poll();
    let notes: Vec<String> = rig.store.traces.borrow().iter().flat_map(|trace| trace.instances.iter().map(|instance| instance.note.clone())).collect();
    assert!(notes.iter().all(|note| !note.contains("held") && !note.contains("closing")), "{notes:?}: a free brief and no dead holder write no note");
}

/// Her Self run in force whose unit is gone, claiming `claims`, admitted `ago` seconds before now.
fn crashed_self_run(rig: &Rig, claims: &[PipelineRef], ago: i64) {
    rig.minds[ANN].admit_at(rig.now() - Duration::seconds(ago));
    rig.minds[ANN].committed(vec![run_in(
        "ann",
        "crashed",
        RunTurn::SelfRun,
        RunOperator::Mind,
        claims,
    )]);
}

/// Polls on, a beat apart, leaving every run to its unit: nothing here ends one.
fn polls(rig: &Rig, count: usize) {
    for _ in 0..count {
        rig.advance(1000);
        rig.poll();
    }
}

fn notes_of(rig: &Rig) -> Vec<String> {
    rig.store
        .traces
        .borrow()
        .iter()
        .flat_map(|trace| trace.instances.iter().map(|instance| instance.note.clone()))
        .collect()
}

#[test]
fn a_self_run_left_in_force_by_a_crash_is_closed_by_the_next_beat_and_the_successor_launches() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "orphan", &source);
    crashed_self_run(&rig, &[brief_ref("orphan")], 3600);
    assert_eq!(self_runs(&rig, ANN).len(), 1);
    polls(&rig, 8);
    let runs = self_runs(&rig, ANN);
    assert_eq!(runs.len(), 2, "one successor, once");
    assert!(
        matches!(
            rig.minds[ANN]
                .view(K::Run, "ann:run:crashed")
                .unwrap()
                .status,
            huginn_mind::PipelineStatus::Resolved { .. }
        ),
        "the dead holder was closed by the launch"
    );
    assert_eq!(
        claims_of_run(&runs[1]),
        vec![brief_ref("orphan").id.0],
        "claiming the brief the dead run left"
    );
    assert!(
        notes_of(&rig)
            .contains(&"closing the Self run whose unit is gone: ann:run:crashed".to_string()),
        "{:?}",
        notes_of(&rig)
    );
}

#[test]
fn a_self_run_whose_unit_may_still_be_starting_or_is_running_gets_no_successor() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "orphan", &source);
    crashed_self_run(&rig, &[brief_ref("orphan")], 30);
    rig.advance(10);
    rig.poll();
    assert_eq!(
        self_runs(&rig, ANN).len(),
        1,
        "inside the start grace the unit may still be starting"
    );
    rig.launcher
        .revive_as(&Slug("ann".into()), RunTurn::SelfRun);
    rig.advance(3600);
    rig.poll();
    polls(&rig, 4);
    assert_eq!(
        self_runs(&rig, ANN).len(),
        1,
        "a run whose unit is active holds the briefs however old it is"
    );
    assert!(matches!(
        rig.minds[ANN]
            .view(K::Run, "ann:run:crashed")
            .unwrap()
            .status,
        huginn_mind::PipelineStatus::InForce
    ));
}

#[test]
fn one_held_brief_says_so_in_the_trace() {
    let (rig, source) = rig_with_a_persona_turn();
    brief_by(&rig, ANN, crate::persona::AGENT, "solo", &source);
    fail_bundle(&rig, &[brief_ref("solo")]);
    beats_until(&rig, 12, || {
        notes_of(&rig).contains(&"1 briefs held by open questions".to_string())
    });
    assert!(
        notes_of(&rig).contains(&"1 briefs held by open questions".to_string()),
        "{:?}",
        notes_of(&rig)
    );
}

#[test]
fn an_answer_of_retire_resolves_nothing_and_the_hold_stays_until_the_answerer_resolves() {
    let (rig, source) = rig_with_a_persona_turn();
    let items = failed_bundle(&rig, &source);
    beats(&rig, 3);
    let question = rig.minds[ANN].ids(K::Question).pop().expect("the trip asked");
    assert_eq!(self_runs(&rig, ANN).len(), 3);
    rig.minds[ANN].committed(vec![crate::testkit::ruling("answer", Some((&question, "retire")))]);
    beats(&rig, 6);
    assert_eq!(self_runs(&rig, ANN).len(), 3, "no Self run after the answer: retire does not relaunch");
    for item in &items {
        let view = rig.minds[ANN].view(K::FollowUp, &item.id.0).unwrap();
        assert!(matches!(view.status, huginn_mind::PipelineStatus::InForce), "{item:?}: the heartbeat resolves nothing on an answer");
    }
    assert_eq!(rig.minds[ANN].ids(K::Question).len(), 1, "and no second question");
    assert!(inbox::derive(&rig.minds[ANN], &HashSet::new()).unwrap().is_empty(), "the retired claims stay held and untold");
}

/// Bob's Persona turn admitted `ago` seconds back and left in force; the operator names him.
fn bob_in_a_turn_and_named(ago: i64) -> Box<Rig> {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig.minds[BOB].admit_at(rig.now() - Duration::seconds(ago));
    rig.minds[BOB].committed(vec![run_in("bob", "held", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    rig.minds[BOB].admit_at(rig.now());
    rig.say(BOB, delivery(1, "@op:chat", "Bob, a word", &[]));
    rig
}

#[test]
fn a_persona_run_whose_unit_is_gone_is_not_busy_and_the_next_launch_closes_it() {
    // A brake hold stopped her unit mid-turn: the run is in force, no unit is active.
    let rig = bob_in_a_turn_and_named(3600);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "a dead holder does not make her busy");
    assert!(matches!(rig.minds[BOB].view(K::Run, "bob:run:held").unwrap().status, huginn_mind::PipelineStatus::Resolved { .. }), "slot step 0 closed the dead run");
}

#[test]
fn a_persona_run_whose_unit_is_active_is_busy() {
    let rig = bob_in_a_turn_and_named(3600);
    rig.launcher.revive_as(&Slug("bob".into()), RunTurn::PersonaTurn);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert_eq!(rig.launched(), vec![]);
    assert!(matches!(rig.minds[BOB].view(K::Run, "bob:run:held").unwrap().status, huginn_mind::PipelineStatus::InForce));
}

#[test]
fn the_file_store_reads_back_the_addressing_it_wrote_to_the_view() {
    let dir = TempDir::new().unwrap();
    let store = FileStore::new(dir.path().join("heartbeat.cc"));
    assert_eq!(store.addressing().unwrap(), None);
    let table = Addressing { minds: vec![Addressee::new("@bob:chat", ["Bob"]).with_projects([Name { text: "BDS".into(), exact: true }])] };
    store.save_addressing(&table).unwrap();
    assert_eq!(store.addressing().unwrap(), Some(table.clone()));
    assert_eq!(store.load_addressing().unwrap(), Some(table));
}

/// A night over the rig's clock, 13:31 UTC: the window holds it, `ann` sleeps.
fn nights(window: &str, minds: &str) -> Box<Rig> {
    let mut rig = Rig::new();
    rig.config.night = Some(Night::parse(window, minds).unwrap());
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    rig
}

/// Ends the run the heartbeat launched for `who` (the newest by admission, not by key).
fn end_launched(rig: &Rig, who: usize) {
    let launched = rig.minds[who].ids(K::Run).into_iter().find(|id| id.contains(":run:mind-")).expect("a launched run");
    rig.minds[who].committed(vec![close(reference(K::Run, &launched), recorded())]);
}

fn started_turns(rig: &Rig) -> Vec<(String, RunTurn)> {
    rig.launcher.started.borrow().iter().map(|(slug, turn)| (slug.0.clone(), *turn)).collect()
}

#[test]
fn the_window_wraps_midnight_and_names_when_it_opened() {
    let night = Night::parse("22:00-04:00", "ann").unwrap();
    let on = |hour, minute| Utc.with_ymd_and_hms(2026, 10, 9, hour, minute, 0).unwrap();
    assert_eq!(night.opened(on(23, 0)), Some(on(22, 0)));
    assert_eq!(night.opened(on(2, 0)), Some(on(22, 0) - Duration::days(1)), "after midnight it opened yesterday");
    assert_eq!(night.opened(on(4, 0)), None, "the end is outside");
    assert_eq!(night.opened(on(13, 0)), None);
    assert_eq!(Night::parse("13:00-14:00", "ann").unwrap().opened(on(13, 0)), Some(on(13, 0)), "the start is inside");
    assert!(Night::parse("13:00-13:00", "ann").is_err() && Night::parse("25:00-03:00", "ann").is_err() && Night::parse("13:00-14:00", " , ").is_err());
}

#[test]
fn a_window_keeps_its_minutes_and_its_bounds_are_the_clock() {
    let on = |hour, minute| Utc.with_ymd_and_hms(2026, 10, 9, hour, minute, 0).unwrap();
    let night = Night::parse("22:30-04:15", "ann").unwrap();
    assert_eq!((night.from_min, night.to_min), (22 * 60 + 30, 4 * 60 + 15));
    assert_eq!(night.opened(on(22, 29)), None);
    assert_eq!(night.opened(on(22, 30)), Some(on(22, 30)));
    assert_eq!(night.opened(on(4, 14)), Some(on(22, 30) - Duration::days(1)));
    assert_eq!(night.opened(on(4, 15)), None);
    // The last minute of the day is a time, and the first one past it is not.
    let late = Night::parse("23:59-00:00", "ann").unwrap();
    assert_eq!((late.from_min, late.to_min), (23 * 60 + 59, 0));
    assert_eq!(Night::parse("00:00-23:00", "ann").unwrap().to_min, 23 * 60);
    assert_eq!(Night::parse("12:00-12:59", "ann").unwrap().to_min, 12 * 60 + 59);
    for bad in ["24:00-03:00", "22:00-24:00", "22:60-03:00", "22:00-03:60", "22:00", "22-03", "aa:bb-03:00"] {
        assert!(Night::parse(bad, "ann").is_err(), "{bad}");
    }
}

#[test]
fn the_nightly_sleep_is_both_settings_or_neither() {
    assert_eq!(Night::from_settings(None, None).unwrap(), None);
    let night = Night::from_settings(Some("22:30-04:00"), Some("ann, bob")).unwrap().unwrap();
    assert_eq!((night.from_min, night.to_min, night.minds), (22 * 60 + 30, 4 * 60, vec![Slug("ann".into()), Slug("bob".into())]));
    for only in [Night::from_settings(Some("22:30-04:00"), None), Night::from_settings(None, Some("ann"))] {
        let fault = only.expect_err("a half-set night is refused at start").to_string();
        assert!(fault.contains("HEARTBEAT_SLEEP_UTC") && fault.contains("HEARTBEAT_SLEEP_MINDS"), "{fault}");
    }
    assert!(Night::from_settings(Some("nonsense"), Some("ann")).is_err(), "both set, but not a window");
}

#[test]
fn a_listed_idle_mind_sleeps_once_per_window_and_a_missed_night_is_not_made_up() {
    let rig = nights("13:00-14:00", "ann");
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(started_turns(&rig), vec![("ann".to_string(), RunTurn::Sleep)], "bob is not listed, and the beat has just fallen");
    assert!(rig.minds[ANN].view(K::Run, rig.minds[ANN].ids(K::Run).last().unwrap()).is_some());
    // Busy while the night lasts: nothing more. Then her night ends (failed or slept, it counts) and the window is spent for her.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    end_launched(&rig, ANN);
    // Her unit is gone as well, so she is idle and only her last Sleep run says the window is spent.
    rig.launcher.kill_as(&Slug("ann".into()), RunTurn::Sleep);
    rig.advance(60);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "one night per window, whatever became of it");
    assert_eq!(started_turns(&rig).len(), 1);
    // The next window opens a day later and she sleeps again.
    rig.advance(24 * 3600);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn no_night_outside_the_window_for_an_unlisted_mind_or_after_the_window_passed_busy() {
    let outside = nights("22:00-04:00", "ann,bob");
    assert_eq!(outside.poll(), Outcome::Polled { launched: vec![] });
    assert!(started_turns(&outside).is_empty());
    // She is awake (a live Persona turn) when the window opens and until it closes: the night is not made up after.
    let rig = nights("13:00-14:00", "ann");
    rig.minds[ANN].committed(vec![run_in("ann", "awake", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    rig.launcher.revive_as(&Slug("ann".into()), RunTurn::PersonaTurn);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    rig.advance(3600);
    rig.launcher.kill_as(&Slug("ann".into()), RunTurn::PersonaTurn);
    rig.end_run(ANN);
    rig.store.save(&HeartbeatState { last_beat_at: rig.now(), handled_mentions: vec![] }).unwrap();
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert!(started_turns(&rig).is_empty());
}

#[test]
fn the_operator_goes_first_and_the_night_waits_for_her_to_be_idle() {
    let rig = nights("13:00-14:00", "ann");
    rig.say(ANN, delivery(1, "@op:chat", "Ann, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(started_turns(&rig), vec![("ann".to_string(), RunTurn::PersonaTurn)], "the mention is answered awake, no Sleep that poll");
    end_launched(&rig, ANN);
    rig.chats[ANN].deliveries.borrow_mut().clear();
    rig.advance(60);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(started_turns(&rig).last(), Some(&("ann".to_string(), RunTurn::Sleep)));
}

#[test]
fn a_sleeping_mind_is_not_eligible_for_the_beat() {
    let rig = nights("13:00-14:00", "ann");
    rig.store.save(&HeartbeatState { last_beat_at: DateTime::<Utc>::UNIX_EPOCH, handled_mentions: vec![] }).unwrap();
    rig.pulse(ANN, 5, None, Some(1000));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into()), Slug("bob".into())] });
    assert_eq!(started_turns(&rig), vec![("ann".to_string(), RunTurn::Sleep), ("bob".to_string(), RunTurn::PersonaTurn)], "the beat went to the Mind who was awake, though ann's urgency was the higher");
}
