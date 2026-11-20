//! Behaviour of the shared heartbeat through its ports: a fake brake, fake
//! mouths, real minds over temporary directories, a recording launcher and a
//! fixed clock. Every rule is pinned by a test that fails when the rule breaks.
//!
//! The rig's beat is 900 s. When a Mind last took a turn is the heartbeat's own
//! state (`HeartbeatState::last_turn_at`), so a test sets it; whether she is in a
//! turn is her unit, which the recording launcher answers.

use std::cell::{Cell, RefCell};

use chrono::Duration;
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineKind as K};
use huginn_mind::PipelineSelectionPage;
use tempfile::TempDir;

use super::*;
use crate::address::addresses;
use crate::control::{Brake, ControlState, SwarmState};
use crate::testkit::{Fixed, Recording, TestMind, at, reference};

/// The rig's beat, in seconds.
const BEAT_S: u32 = 900;

struct Counting<'a> {
    inner: &'a TestMind,
    calls: &'a Cell<u32>,
    /// Every admission asked of her mind: the heartbeat asks none.
    admits: &'a Cell<u32>,
    /// Reads of the stewardship kind, and whether they fail while everything else answers.
    stewardship_reads: &'a Cell<u32>,
    stewardship_down: &'a Cell<bool>,
}

impl MindPort for Counting<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.calls.set(self.calls.get() + 1);
        if selection.schemas.iter().flatten().any(|schema| schema == K::Stewardship.type_id()) {
            self.stewardship_reads.set(self.stewardship_reads.get() + 1);
            if self.stewardship_down.get() {
                return Err(anyhow!("the stewardship read failed"));
            }
        }
        self.inner.query(selection)
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<huginn_mind::PipelineAdmissionOutcome> {
        self.calls.set(self.calls.get() + 1);
        self.admits.set(self.admits.get() + 1);
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
    fn pending(&self, after_seq: u64) -> Result<crate::chat_client::ChatPending> {
        self.pending_calls.set(self.pending_calls.get() + 1);
        Ok(crate::chat_client::ChatPending { persona: "eureka".into(), deliveries: self.deliveries.borrow().iter().filter(|delivery| delivery.seq > after_seq).cloned().collect(), tail: None, tail_gaps: vec![] })
    }

    fn send(&self, _: &crate::chat_client::ChatSendRequest) -> Result<crate::chat_client::ChatSendReceipt> {
        panic!("the heartbeat sends nothing")
    }

    fn open_direct(&self, _: &crate::chat_client::ChatOpenDirectRequest) -> Result<crate::chat_client::ChatOpenDirectReceipt> {
        panic!("the heartbeat opens nothing")
    }

    fn fetch(&self, _: &crate::chat_client::ChatFetchRequest) -> Result<crate::chat_client::ChatFetchReceipt> {
        panic!("the heartbeat fetches nothing")
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
        basis: None,
        form: crate::chat_client::Form::Message,
        reply_to: crate::chat_client::ReplyLink::Absent,
    }
}

const ANN: usize = 0;
const BOB: usize = 1;
const NAMES: [&str; 2] = ["ann", "bob"];

struct Rig {
    minds: Vec<TestMind>,
    calls: Vec<Cell<u32>>,
    admits: Cell<u32>,
    stewardship_reads: Cell<u32>,
    stewardship_down: Cell<bool>,
    chats: Vec<FakeChat>,
    control: FakeControl,
    launcher: Recording,
    clock: Fixed,
    store: MemStore,
    config: Config,
}

impl Rig {
    fn new() -> Box<Self> {
        Box::new(Self {
            minds: vec![TestMind::seeded_as("ann"), TestMind::seeded_as("bob")],
            calls: vec![Cell::new(0), Cell::new(0)],
            admits: Cell::new(0),
            stewardship_reads: Cell::new(0),
            stewardship_down: Cell::new(false),
            chats: vec![FakeChat::default(), FakeChat::default()],
            control: FakeControl::new(),
            launcher: Recording::default(),
            clock: Fixed(Cell::new(at(1_000_000))),
            store: MemStore::default(),
            config: Config::new(vec![persona("ann", "Ann"), persona("bob", "Bob")], vec!["@op:chat".into()], BEAT_S).unwrap(),
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
        let ports: Vec<Counting> = self
            .minds
            .iter()
            .zip(&self.calls)
            .map(|(inner, calls)| Counting { inner, calls, admits: &self.admits, stewardship_reads: &self.stewardship_reads, stewardship_down: &self.stewardship_down })
            .collect();
        let seats: Vec<Seat> = self.config.personas.iter().enumerate().map(|(index, who)| Seat { who: who.clone(), mind: &ports[index], chat: &self.chats[index] }).collect();
        let organs = Organs { seats: &seats, control: &self.control, launcher: &self.launcher, clock: &self.clock, store: &self.store };
        poll(&organs, &self.config).unwrap()
    }

    /// The stored state, made if there is none, changed by `change`.
    fn with_state(&self, change: impl FnOnce(&mut HeartbeatState)) {
        let mut state = self.store.state.borrow().clone().unwrap_or(HeartbeatState { last_beat_at: DateTime::<Utc>::UNIX_EPOCH, handled_mentions: vec![], last_turn_at: BTreeMap::new() });
        change(&mut state);
        *self.store.state.borrow_mut() = Some(state);
    }

    /// The heartbeat last woke `who` for a turn `ago` seconds before now.
    fn turn(&self, who: usize, ago: i64) {
        let at = self.now() - Duration::seconds(ago);
        self.with_state(|state| {
            state.last_turn_at.insert(NAMES[who].into(), at);
        });
    }

    /// The beat fell now, so only an off-beat wake can start a unit.
    fn beat_fell(&self) {
        let now = self.now();
        self.with_state(|state| state.last_beat_at = now);
    }

    fn launched(&self) -> Vec<Slug> {
        self.launcher.started.borrow().iter().map(|(slug, _)| slug.clone()).collect()
    }

    fn started_turns(&self) -> Vec<(String, RunTurn)> {
        self.launcher.started.borrow().iter().map(|(slug, turn)| (slug.0.clone(), *turn)).collect()
    }

    /// Her unit ends, as her turn would.
    fn end_unit(&self, who: usize) {
        self.launcher.kill_as(&Slug(NAMES[who].into()), RunTurn::PersonaTurn);
    }

    /// Her unit is active.
    fn in_a_turn(&self, who: usize) {
        self.launcher.revive_as(&Slug(NAMES[who].into()), RunTurn::PersonaTurn);
    }

    fn total_calls(&self) -> u32 {
        self.calls.iter().map(Cell::get).sum::<u32>() + self.chats.iter().map(|chat| chat.pending_calls.get()).sum::<u32>()
    }

    fn notes(&self) -> Vec<String> {
        self.store.traces.borrow().iter().flat_map(|trace| trace.instances.iter().map(|instance| instance.note.clone())).collect()
    }
}

/// A rig on the beat's far side, so only an off-beat wake can start a unit.
fn quiet_rig() -> Box<Rig> {
    let rig = Rig::new();
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.beat_fell();
    rig
}

#[test]
fn each_persona_carries_her_own_huginn_and_a_missing_or_malformed_one_refuses_the_config() {
    let specs = "ann,ann,Ann,@ann:chat,rudp://127.0.0.1:4101; bob,bob,Bob Smith,@bob:chat,rudp://10.0.0.2:4102";
    let personas: Vec<PersonaId> = specs.split(';').map(PersonaId::parse).collect::<Result<_>>().unwrap();
    assert_eq!(personas.iter().map(|who| who.huginn.to_string()).collect::<Vec<_>>(), ["127.0.0.1:4101", "10.0.0.2:4102"]);
    for bad in ["ann,ann,Ann,@ann:chat", "ann,ann,Ann,@ann:chat,", "ann,ann,Ann,@ann:chat,127.0.0.1:4101", "ann,ann,Ann,@ann:chat,rudp://localhost:4101"] {
        assert!(PersonaId::parse(bad).is_err(), "{bad}");
    }
    assert!(Config::new(personas.clone(), vec![], 0).is_err(), "a beat of zero seconds is refused");
    let refused = Config::new(personas, vec!["@bob:chat".into()], 60).map(|_| ()).unwrap_err();
    assert!(format!("{refused:#}").contains("operator id is also a persona"), "a sender is the operator or a Mind, never both");
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
fn a_beat_wakes_exactly_one_persona_the_one_who_has_waited_longest() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "both are idle and the beat is due, but one beat is one turn");
    assert_eq!(rig.started_turns(), vec![("ann".to_string(), RunTurn::PersonaTurn)]);
    let state = rig.store.state.borrow().clone().unwrap();
    assert_eq!(state.last_turn_at.get("ann"), Some(&rig.now()), "the wake is her last turn");
    let trace = rig.store.traces.borrow().last().cloned().unwrap();
    assert_eq!((trace.tempo_s, trace.launches.clone()), (BEAT_S, vec!["ann".to_string()]));
    assert!(trace.instances[ANN].busy);
}

#[test]
fn the_next_beat_falls_a_beat_later_and_waiting_alone_round_robins() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    rig.poll();
    rig.end_unit(ANN);
    rig.advance(60);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the beat of 900 s has not passed");
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "bob has now waited longest");
    assert_eq!(rig.store.state.borrow().as_ref().unwrap().last_beat_at, rig.now());
}

#[test]
fn the_beat_is_the_configured_one() {
    let mut rig = Rig::new();
    rig.config.beat_s = 3600;
    rig.turn(ANN, 10);
    rig.turn(BOB, 10);
    rig.beat_fell();
    rig.advance(3599);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    rig.advance(1);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(rig.store.traces.borrow().last().unwrap().tempo_s, 3600);
}

#[test]
fn a_mind_whose_start_keeps_failing_neither_starves_the_beat_nor_reads_as_never_acted() {
    let rig = Rig::new();
    rig.turn(ANN, 5000);
    rig.turn(BOB, 1000);
    *rig.launcher.fail_for.borrow_mut() = Some(Slug("ann".into()));
    for beat in 1..=12 {
        rig.advance(1000);
        assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "beat {beat}");
        rig.end_unit(BOB);
        let trace = rig.store.traces.borrow().last().cloned().unwrap();
        assert!(trace.instances[ANN].note.contains("start failed"), "beat {beat}: {:?}", trace.instances[ANN]);
        assert!(trace.instances[ANN].idle_s >= 5000, "beat {beat}: a failed start is no turn: {:?}", trace.instances[ANN]);
    }
    assert_eq!(rig.launched().iter().filter(|slug| slug.0 == "bob").count(), 12);
}

#[test]
fn the_longest_waiter_wins() {
    let rig = Rig::new();
    rig.turn(ANN, 1500);
    rig.turn(BOB, 1000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn the_operator_naming_a_persona_wakes_her_at_once_off_the_beat_and_once_per_message() {
    let rig = quiet_rig();
    rig.say(BOB, delivery(1, "@op:chat", "Bob, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the beat has just fallen, and the operator does not wait for the next");
    assert_eq!(rig.store.state.borrow().as_ref().unwrap().last_beat_at, rig.now(), "an off-beat wake does not move the beat");
    // Her turn ends without acking; the same message does not wake her again.
    rig.end_unit(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert_eq!(rig.launched(), vec![Slug("bob".into())]);
    // A tag in m.mentions counts as naming.
    rig.say(ANN, delivery(2, "@op:chat", "please look", &["@ann:chat"]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
}

#[test]
fn another_mind_naming_a_persona_wakes_her_once_off_the_beat() {
    let rig = quiet_rig();
    rig.say(BOB, delivery(1, "@ann:chat", "Bob, a word", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "a Mind of the roster wakes her at once");
    rig.end_unit(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the same message polled again wakes nobody");
}

#[test]
fn a_mind_naming_no_one_or_herself_wakes_nobody() {
    let rig = quiet_rig();
    rig.say(BOB, delivery(1, "@ann:chat", "nothing to anyone", &[]));
    rig.say(BOB, delivery(2, "@bob:chat", "Bob reminds Bob", &["@bob:chat"]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
}

#[test]
fn a_member_naming_a_persona_wakes_nobody_off_the_beat_but_counts_as_full_urgency_on_it() {
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    rig.beat_fell();
    rig.say(BOB, delivery(1, "@member:chat", "Bob, are you there?", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "a member does not start a turn");
    rig.advance(900);
    // Bob waited 990 s x ten for the member addressing him; Ann waited 1900 s x one: Bob wins the beat.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
    assert_eq!(rig.store.traces.borrow().last().unwrap().instances[BOB].urgency_permille, 1000);
}

#[test]
fn a_member_message_naming_nobody_of_hers_scores_nobody() {
    let rig = Rig::new();
    rig.turn(ANN, 1000);
    rig.turn(BOB, 90);
    rig.beat_fell();
    rig.say(BOB, delivery(1, "@member:chat", "good morning, swarm", &[]));
    rig.say(BOB, delivery(2, "@member:chat", "is BDS green?", &[]));
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] }, "BDS is nobody's here: no one stewards it");
}

/// `who` also stewards `repo`, with `names` listed or none.
fn steward(rig: &Rig, who: usize, repo: &str, names: &[&str]) {
    rig.minds[who].committed(vec![PipelineDocument::Stewardship(eureka_pipeline::PipelineStewardship {
        instance: Slug(NAMES[who].into()),
        repo: repo.into(),
        sequence: 1,
        assigned_on: eureka_pipeline::Date("2026-11-06".into()),
        note: "assigned".into(),
        names: names.iter().map(|name| (*name).into()).collect(),
    })]);
}

#[test]
fn the_operator_naming_a_project_untagged_wakes_its_idle_steward_once_off_the_beat() {
    let rig = quiet_rig();
    steward(&rig, ANN, "GameCult/BadDaySimulator", &["BDS"]);
    rig.say(ANN, delivery(1, "@op:chat", "BDS is down", &[]));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    rig.end_unit(ANN);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the same message polled again wakes nobody");
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
    rig.beat_fell();
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
    assert!(rig.notes().iter().any(|note| note.contains("stewardship names unread")), "{:?}", rig.notes());
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
    assert!(rig.notes().iter().any(|note| note.contains("stewardship names unread")), "{:?}", rig.notes());
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
fn an_operator_mention_wakes_no_busy_or_braked_persona() {
    let rig = quiet_rig();
    rig.control.braked.borrow_mut().push("ann".into());
    rig.say(ANN, delivery(1, "@op:chat", "Ann, now", &[]));
    rig.say(BOB, delivery(2, "@op:chat", "Bob, now", &[]));
    rig.in_a_turn(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] });
    assert_eq!(rig.launched(), vec![], "nothing started");
    rig.end_unit(BOB);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the mention waits for her turn to end, and is answered once");
}

#[test]
fn a_braked_or_busy_persona_is_never_the_beat_winner() {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    rig.turn(BOB, 100);
    rig.control.braked.borrow_mut().push("ann".into());
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the longest waiter is braked");
    rig.end_unit(BOB);
    rig.control.braked.borrow_mut().clear();
    rig.in_a_turn(ANN);
    rig.advance(900);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] }, "the longest waiter is in a turn");
}

#[test]
fn a_persona_that_has_never_acted_goes_first() {
    let rig = Rig::new();
    rig.turn(ANN, 100_000);
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into())] });
}

// ---- C2 of the Persona-only map: the heartbeat writes no mind and starts no Self run ----

#[test]
fn a_mind_whose_self_holds_briefs_gets_a_persona_wake_and_no_self_run() {
    let rig = Rig::new();
    rig.turn(ANN, 3000);
    rig.turn(BOB, 1000);
    rig.minds[ANN].committed(vec![crate::testkit::run_in("ann", "old", RunTurn::PersonaTurn, eureka_pipeline::RunOperator::Mind, &[])]);
    let source = reference(K::Run, "ann:run:old");
    rig.minds[ANN].committed(vec![crate::testkit::close(source.clone(), crate::testkit::recorded())]);
    for label in ["first", "second"] {
        let PipelineDocument::FollowUp(mut brief) = crate::testkit::follow_up(label, &source) else { unreachable!() };
        brief.owner = "self-run".into();
        rig.minds[ANN].committed(vec![PipelineDocument::FollowUp(brief)]);
    }
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("ann".into())] });
    assert_eq!(rig.started_turns(), vec![("ann".to_string(), RunTurn::PersonaTurn)], "her Persona's unit, never mind-self@");
}

#[test]
fn no_poll_admits_anything_to_any_mind_whatever_it_wakes() {
    let rig = Rig::new();
    let runs_before: Vec<usize> = rig.minds.iter().map(|mind| mind.ids(K::Run).len()).collect();
    rig.say(BOB, delivery(1, "@op:chat", "Bob, a word", &[]));
    // One poll wakes bob off the beat and ann on it; the next beat wakes one of them again.
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![Slug("bob".into()), Slug("ann".into())] });
    rig.end_unit(ANN);
    rig.end_unit(BOB);
    rig.advance(900);
    rig.poll();
    assert!(rig.launched().len() >= 3, "{:?}", rig.started_turns());
    assert_eq!(rig.admits.get(), 0, "no admission was asked of a mind");
    assert_eq!(rig.minds.iter().map(|mind| mind.ids(K::Run).len()).collect::<Vec<_>>(), runs_before, "no run was opened");
}

// ---- The store ----

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

fn state_at(milliseconds: i64) -> HeartbeatState {
    HeartbeatState { last_beat_at: crate::testkit::at(milliseconds), handled_mentions: vec!["ann:e1".into()], last_turn_at: BTreeMap::from([("ann".to_string(), crate::testkit::at(3))]) }
}

#[test]
fn the_file_store_keeps_the_state_and_the_traces_and_reads_the_newest_trace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("heartbeat.cc");
    let store = FileStore::new(&path);
    store.check().unwrap();
    assert_eq!((store.load().unwrap(), store.latest().unwrap()), (None, None), "no file is no state");
    let state = state_at(5);
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

/// The state as the heartbeat before this cut wrote it: two slots.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.heartbeat.state.v1", schema = "HeartbeatState")]
struct StateBeforeTheCut {
    #[cultcache(key = 0)]
    last_beat_at: DateTime<Utc>,
    #[cultcache(key = 1)]
    handled_mentions: Vec<String>,
}

#[test]
fn a_state_written_before_the_cut_reads_with_no_turns() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("heartbeat.cc");
    let mut cache = CultCache::new();
    cache.register_entry_type::<StateBeforeTheCut>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put(STATE_KEY, &StateBeforeTheCut { last_beat_at: crate::testkit::at(7), handled_mentions: vec!["ann:e1".into()] }).unwrap();
    let state = FileStore::new(&path).load().unwrap().expect("the old state reads");
    assert_eq!(state, HeartbeatState { last_beat_at: crate::testkit::at(7), handled_mentions: vec!["ann:e1".into()], last_turn_at: BTreeMap::new() });
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
    for result in [store.check(), store.load().map(|_| ()), store.latest().map(|_| ()), store.save(&state_at(1)), store.record(&trace_at(1, 900))] {
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains(&path.display().to_string()) && error.contains("archive it, nothing migrates it"), "{error}");
    }
}

#[test]
fn the_file_store_reads_back_the_addressing_it_wrote_to_the_view() {
    let dir = TempDir::new().unwrap();
    let store = FileStore::new(dir.path().join("heartbeat.cc"));
    assert_eq!(store.addressing().unwrap(), None);
    let table = Addressing { minds: vec![Addressee::new("@bob:chat", ["Bob"]).with_stewardships([Name { text: "BDS".into(), exact: true }], [])] };
    store.save_addressing(&table).unwrap();
    assert_eq!(store.addressing().unwrap(), Some(table.clone()));
    assert_eq!(store.load_addressing().unwrap(), Some(table));
}

/// Matrix's word that a message is her DM, a tag or a reply wakes her off the beat from the operator; a notice or an edit never does,
/// whatever its basis, and a stranger's direct message is delivered but wakes no one.
#[test]
fn matrixs_addressing_facts_wake_her_off_the_beat_only_on_a_message_from_the_roster() {
    use crate::chat_client::{Basis, Form};
    let with = |seq, sender: &str, basis, form| ChatDelivery { basis, form, ..delivery(seq, sender, "no name here", &[]) };
    for (basis, form, wakes) in [
        (Some(Basis::Direct), Form::Message, true),
        (Some(Basis::Tag), Form::Message, true),
        (Some(Basis::Reply), Form::Message, true),
        (Some(Basis::FollowUp), Form::Message, true),
        (Some(Basis::FollowUp), Form::Notice, false),
        (Some(Basis::Room), Form::Message, false),
        (None, Form::Message, false),
        (Some(Basis::Direct), Form::Notice, false),
        (Some(Basis::Tag), Form::Edit, false),
    ] {
        let rig = quiet_rig();
        rig.say(BOB, with(1, "@op:chat", basis, form));
        let launched = if wakes { vec![Slug("bob".into())] } else { vec![] };
        assert_eq!(rig.poll(), Outcome::Polled { launched }, "{basis:?} {form:?}");
    }
    let rig = quiet_rig();
    rig.say(BOB, with(1, "@stranger:chat", Some(Basis::Direct), Form::Message));
    assert_eq!(rig.poll(), Outcome::Polled { launched: vec![] }, "the sender gate stays: a stranger's DM is delivered and wakes no one off the beat");
}
