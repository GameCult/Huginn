//! The actuator's tests: a real mind over a temporary directory holds her run,
//! a scripted `Child` plays the stream and a settable clock plays time. The
//! stream lines are built from Claude Code 2.1.268's own records (the two
//! fixture files beside this one), varying only ids and usage numbers. The real
//! `ClaudeProcess` is driven through a fake `claude` script on Unix.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use chrono::Duration as ChronoDuration;
use eureka_pipeline::{PipelineDocument, PipelineKind as K, PipelineRef, RunOperator, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelineSelectionPage};

use super::*;
use crate::testkit::*;

const RECORDS: &str = include_str!("claude-2.1.268-assistant-usage-records.jsonl");
const PROBE: &str = include_str!("claude-2.1.268-stream-json-probe.jsonl");

// Pinned against Python's uuid.uuid5(uuid.NAMESPACE_OID, <run id>), not against the code under test.
const SESSION_OF_MIND_RUN: &str = "00cb82a4-c0ac-5ad4-83e2-8e685589fb67";
const SESSION_OF_RUN_A: &str = "d6db8da1-224c-5430-91b5-17aa4df8ae7a";
const SESSION_OF_RUN_B: &str = "546bec88-f7a9-5fa0-bf07-0fada54f071b";

fn foreign_run(id: &str) -> PipelineRef {
    PipelineRef { kind: K::Run, id: id.into() }
}

/// The first real assistant record with its id and usage replaced.
fn request(id: &str, input: u64, read: u64, write: u64) -> String {
    let mut event: Value = serde_json::from_str(RECORDS.lines().next().unwrap()).unwrap();
    event["message"]["id"] = json!(id);
    event["message"]["usage"]["input_tokens"] = json!(input);
    event["message"]["usage"]["cache_read_input_tokens"] = json!(read);
    event["message"]["usage"]["cache_creation_input_tokens"] = json!(write);
    event.to_string()
}

/// A request whose whole prefix is `prefix`, under a message id of its own.
fn at(id: &str, prefix: u64) -> String {
    request(id, 0, prefix, 0)
}

fn subagent(id: &str, prefix: u64) -> String {
    let mut event: Value = serde_json::from_str(&at(id, prefix)).unwrap();
    event["parent_tool_use_id"] = json!("toolu_0001");
    event.to_string()
}

fn result_line() -> String {
    PROBE.lines().nth(2).unwrap().to_string()
}

fn init_line() -> String {
    PROBE.lines().next().unwrap().to_string()
}

fn self_run(label: &str, budget: &str) -> PipelineDocument {
    let PipelineDocument::Run(mut document) = run(label, RunTurn::SelfRun, RunOperator::Mind, &[]) else { unreachable!() };
    document.budget_usd = budget.into();
    PipelineDocument::Run(document)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Wrote(String),
    InputClosed,
    Interrupted,
    Asked(StdDuration),
    Waited,
}

#[derive(Clone)]
enum Step {
    Line(String),
    /// Time passes with no output: the clock moves and the read times out.
    Idle(i64),
}

use Step::{Idle, Line as Out};

struct Scripted {
    steps: VecDeque<Step>,
    log: Rc<RefCell<Vec<Event>>>,
    clock: Rc<Fixed>,
    code: Option<i32>,
    dies_on_interrupt: bool,
}

impl Child for Scripted {
    fn write_line(&mut self, line: &str) {
        self.log.borrow_mut().push(Event::Wrote(line.to_string()));
    }

    fn close_input(&mut self) {
        self.log.borrow_mut().push(Event::InputClosed);
    }

    fn next_line(&mut self, timeout: StdDuration) -> Read {
        self.log.borrow_mut().push(Event::Asked(timeout));
        match self.steps.pop_front() {
            Some(Step::Line(line)) => Read::Line(line),
            Some(Step::Idle(seconds)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                Read::Timeout
            }
            None => Read::Closed,
        }
    }

    fn interrupt(&mut self) {
        self.log.borrow_mut().push(Event::Interrupted);
        if self.dies_on_interrupt {
            self.steps.clear();
        }
    }

    fn wait(&mut self) -> Option<i32> {
        self.log.borrow_mut().push(Event::Waited);
        self.code
    }
}

struct Fake {
    child: RefCell<Option<Scripted>>,
    spawns: RefCell<Vec<Vec<String>>>,
    refuse: bool,
}

impl Spawner for Fake {
    fn spawn(&self, args: &[String]) -> Result<Box<dyn Child>> {
        self.spawns.borrow_mut().push(args.to_vec());
        if self.refuse {
            return Err(anyhow!("refused"));
        }
        Ok(Box::new(self.child.borrow_mut().take().expect("spawned twice")))
    }
}

/// The mind as the actuator sees it, over a `TestMind`: another instance's name,
/// doubled run headers, and a count of admissions it must never receive.
struct Seen<'a> {
    inner: &'a TestMind,
    instance: Slug,
    doubled: bool,
    admits: Cell<usize>,
}

impl<'a> Seen<'a> {
    fn of(inner: &'a TestMind) -> Self {
        Self { inner, instance: inner.instance().clone(), doubled: false, admits: Cell::new(0) }
    }
}

impl MindPort for Seen<'_> {
    fn instance(&self) -> &Slug {
        &self.instance
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        let mut page = self.inner.query(selection)?;
        if self.doubled
            && let PipelinePageItems::Headers(items) = &mut page.items
        {
            let again = items.clone();
            items.extend(again);
        }
        Ok(page)
    }

    fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.admits.set(self.admits.get() + 1);
        Err(anyhow!("the actuator admits nothing"))
    }
}

struct Ran {
    outcome: Result<SelfRunOutcome>,
    out: String,
    events: Vec<Event>,
    spawns: Vec<Vec<String>>,
    admits: usize,
}

impl Ran {
    fn ended(&self) -> (Cause, Option<i32>, u64, u64) {
        match self.outcome.as_ref().unwrap() {
            SelfRunOutcome::Ended { cause, code, requests, prefix } => (*cause, *code, *requests, *prefix),
            other => panic!("not ended: {other:?}"),
        }
    }

    /// The text of every user line written to the child, in order.
    fn written(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Wrote(line) => {
                    let line: Value = serde_json::from_str(line).unwrap();
                    assert_eq!((line["type"].as_str(), line["message"]["role"].as_str()), (Some("user"), Some("user")), "{line}");
                    Some(line["message"]["content"].as_str().unwrap().to_string())
                }
                _ => None,
            })
            .collect()
    }

    fn count(&self, wanted: &Event) -> usize {
        self.events.iter().filter(|event| *event == wanted).count()
    }

    fn asked(&self) -> Vec<u64> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Asked(timeout) => Some(timeout.as_secs()),
                _ => None,
            })
            .collect()
    }
}

fn carry_with(seen: &Seen, steps: Vec<Step>, code: Option<i32>, dies_on_interrupt: bool) -> Ran {
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let fake = Fake {
        child: RefCell::new(Some(Scripted { steps: steps.into(), log: log.clone(), clock: clock.clone(), code, dies_on_interrupt })),
        spawns: RefCell::new(Vec::new()),
        refuse: false,
    };
    let mut out = Vec::new();
    let outcome = carry(seen, &*clock, &fake, &mut out);
    let events = log.borrow().clone();
    let spawns = fake.spawns.borrow().clone();
    Ran { outcome, out: String::from_utf8(out).unwrap(), events, spawns, admits: seen.admits.get() }
}

fn at_start() -> chrono::DateTime<chrono::Utc> {
    crate::testkit::at(0)
}

/// A mind holding her one open Self run `selfrun-a` with budget 5.
fn with_run() -> TestMind {
    let mind = TestMind::seeded();
    mind.committed(vec![self_run("selfrun-a", "5")]);
    mind
}

fn go(steps: Vec<Step>) -> Ran {
    let mind = with_run();
    carry_with(&Seen::of(&mind), steps, Some(0), false)
}

type Seed = Box<dyn Fn(&TestMind)>;

fn flushes(ran: &Ran) -> usize {
    ran.written().iter().filter(|text| text.as_str() == FLUSH_MESSAGE).count()
}

// ---- who is carried -----------------------------------------------------

#[test]
fn without_exactly_one_open_self_run_of_hers_it_is_no_run_and_no_child_is_spawned() {
    let cases: Vec<(&str, Seed)> = vec![
        ("no run at all", Box::new(|_| {})),
        ("a Self run the operator operates", Box::new(|mind| mind.committed(vec![run("op", RunTurn::SelfRun, RunOperator::Operator, &[])]))),
        ("her Persona turn", Box::new(|mind| mind.committed(vec![run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]))),
        (
            "a Self run of hers that is closed",
            Box::new(|mind| {
                mind.committed(vec![self_run("selfrun-a", "5")]);
                mind.committed(vec![close(run_ref("selfrun-a"), recorded())]);
            }),
        ),
    ];
    for (name, seed) in cases {
        let mind = TestMind::seeded();
        seed(&mind);
        let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
        assert_eq!(ran.outcome.unwrap(), SelfRunOutcome::NoRun, "{name}");
        assert!(ran.spawns.is_empty() && ran.events.is_empty() && ran.out.is_empty(), "{name}");
    }

    // Another instance's mind holds none of this instance's runs.
    let mind = with_run();
    let mut other = Seen::of(&mind);
    other.instance = Slug("another".into());
    assert_eq!(carry_with(&other, vec![], Some(0), false).outcome.unwrap(), SelfRunOutcome::NoRun);

    // Two open runs are not one: nothing is spawned for either.
    let mut two = Seen::of(&mind);
    two.doubled = true;
    let ran = carry_with(&two, vec![], Some(0), false);
    assert_eq!(ran.outcome.unwrap(), SelfRunOutcome::NoRun);
    assert!(ran.spawns.is_empty());
}

#[test]
fn a_mind_that_cannot_be_read_or_a_child_that_cannot_start_is_an_error_not_an_outcome() {
    let mind = with_run();
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let refusing = Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: true };
    assert!(carry(&Seen::of(&mind), &*clock, &refusing, &mut Vec::new()).is_err());
    assert_eq!(refusing.spawns.borrow().len(), 1, "it tried exactly once");

    struct Down;
    impl MindPort for Down {
        fn instance(&self) -> &Slug {
            static INSTANCE: std::sync::OnceLock<Slug> = std::sync::OnceLock::new();
            INSTANCE.get_or_init(|| Slug("yggdrasil".into()))
        }
        fn query(&self, _selection: &Selection) -> Result<PipelineSelectionPage> {
            Err(anyhow!("down"))
        }
        fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
            unreachable!()
        }
    }
    let never = Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: false };
    assert!(carry(&Down, &*clock, &never, &mut Vec::new()).is_err());
    assert!(never.spawns.borrow().is_empty());
}

// ---- what is started ------------------------------------------------------

#[test]
fn claude_args_is_exactly_the_fixed_list_for_a_session_and_a_budget() {
    let session = Uuid::parse_str(SESSION_OF_RUN_A).unwrap();
    let args = claude_args(session, "12.25");
    assert_eq!(
        args,
        [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--session-id",
            SESSION_OF_RUN_A,
            "--max-budget-usd",
            "12.25",
            "--max-turns",
            "500",
            "--permission-prompts",
            "none",
        ]
    );
    // Those belong to the run measure or to the unit's managed settings.
    for forbidden in ["--resume", "--continue", "--fork-session", "--model", "--permission-mode", "--dangerously-skip-permissions", "--settings", "--system-prompt"] {
        assert!(!args.iter().any(|arg| arg.starts_with(forbidden) || arg.contains("dangerously") || arg == "bypassPermissions"), "{forbidden}");
    }
    assert_eq!(MAX_TURNS, 500);
}

#[test]
fn the_session_id_is_uuid_v5_of_the_run_id_and_differs_per_run() {
    assert_eq!(session_id(&foreign_run("eureka:run:mind-20261022T120000000Z")).to_string(), SESSION_OF_MIND_RUN);
    assert_eq!(session_id(&run_ref("selfrun-a")).to_string(), SESSION_OF_RUN_A);
    assert_eq!(session_id(&run_ref("selfrun-b")).to_string(), SESSION_OF_RUN_B);
}

#[test]
fn the_child_is_started_for_her_run_with_the_runs_own_budget_verbatim() {
    for budget in ["0.5", "5", "12.25"] {
        let mind = TestMind::seeded();
        mind.committed(vec![self_run("selfrun-b", budget)]);
        let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
        assert_eq!(ran.spawns.len(), 1, "{budget}");
        assert_eq!(ran.spawns[0], claude_args(Uuid::parse_str(SESSION_OF_RUN_B).unwrap(), budget), "{budget}");
    }
}

#[test]
fn the_start_message_is_the_first_thing_written_and_names_the_skill_the_instance_and_the_run() {
    let ran = go(vec![Out(result_line())]);
    assert!(matches!(&ran.events[0], Event::Wrote(_)), "the start precedes the first read: {:?}", ran.events[0]);
    let written = ran.written();
    assert_eq!(written[0], start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a")));
    for needle in ["mind-self", "yggdrasil", "yggdrasil:run:selfrun-a", AGENT] {
        assert!(written[0].contains(needle), "{needle}: {}", written[0]);
    }
    assert_eq!(written.len(), 1, "no flush without the ceiling");
}

// ---- what is read -----------------------------------------------------------

#[test]
fn read_usage_sums_input_cache_read_and_cache_write_and_reads_absent_fields_as_zero() {
    assert_eq!(read_usage(&request("m1", 1, 20, 300)), Usage::Request { id: "m1".into(), prefix: 321 });
    // Each term alone, so dropping or doubling any one is seen.
    assert_eq!(read_usage(&request("m", 7, 0, 0)), Usage::Request { id: "m".into(), prefix: 7 });
    assert_eq!(read_usage(&request("m", 0, 7, 0)), Usage::Request { id: "m".into(), prefix: 7 });
    assert_eq!(read_usage(&request("m", 0, 0, 7)), Usage::Request { id: "m".into(), prefix: 7 });
    for absent in ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"] {
        let mut event: Value = serde_json::from_str(&request("m", 1, 20, 300)).unwrap();
        event["message"]["usage"].as_object_mut().unwrap().remove(absent);
        let want = 321 - [("input_tokens", 1), ("cache_read_input_tokens", 20), ("cache_creation_input_tokens", 300)].iter().find(|(name, _)| *name == absent).unwrap().1;
        assert_eq!(read_usage(&event.to_string()), Usage::Request { id: "m".into(), prefix: want }, "{absent} absent");
    }
    let mut no_usage: Value = serde_json::from_str(&request("m", 1, 1, 1)).unwrap();
    no_usage["message"].as_object_mut().unwrap().remove("usage");
    assert_eq!(read_usage(&no_usage.to_string()), Usage::Request { id: "m".into(), prefix: 0 });
}

#[test]
fn read_usage_classifies_the_owners_real_records() {
    let sums = [84210, 100751, 117174, 118336, 126746, 127250, 129853];
    let mut seen = Vec::new();
    for line in RECORDS.lines() {
        let Usage::Request { id, prefix } = read_usage(line) else { panic!("an assistant record of the main thread is a request: {line}") };
        if !seen.iter().any(|(known, _)| *known == id) {
            seen.push((id, prefix));
        }
    }
    assert_eq!(seen.iter().map(|(_, prefix)| *prefix).collect::<Vec<_>>(), sums);
    assert_eq!(read_usage(&result_line()), Usage::Result);
    assert_eq!(read_usage(&init_line()), Usage::Other);
    assert_eq!(read_usage(&subagent("m", 5)), Usage::Other, "a subagent's request is not the main thread's");
    let mut no_id: Value = serde_json::from_str(&at("m", 5)).unwrap();
    no_id["message"].as_object_mut().unwrap().remove("id");
    assert_eq!(read_usage(&no_id.to_string()), Usage::Other);
    let mut null_parent: Value = serde_json::from_str(&at("m", 5)).unwrap();
    null_parent["parent_tool_use_id"] = Value::Null;
    assert_eq!(read_usage(&null_parent.to_string()), Usage::Request { id: "m".into(), prefix: 5 });
    let mut no_parent: Value = serde_json::from_str(&at("m", 5)).unwrap();
    no_parent.as_object_mut().unwrap().remove("parent_tool_use_id");
    assert_eq!(read_usage(&no_parent.to_string()), Usage::Request { id: "m".into(), prefix: 5 });
    for unreadable in ["", "not json", "[1,2]", "\"assistant\"", "12", "null", "{\"type\":"] {
        assert_eq!(read_usage(unreadable), Usage::Unreadable, "{unreadable:?}");
    }
}

#[test]
fn records_that_share_a_message_id_count_once_and_subagent_records_never_move_the_prefix() {
    let mut steps: Vec<Step> = RECORDS.lines().map(|line| Out(line.to_string())).collect();
    steps.insert(3, Out(subagent("msg_sub", 999_999)));
    let ran = go(steps);
    assert_eq!(
        ran.out,
        "request 1 prefix 84210\nrequest 2 prefix 100751\nrequest 3 prefix 117174\nrequest 4 prefix 118336\nrequest 5 prefix 126746\nrequest 6 prefix 127250\nrequest 7 prefix 129853\n"
    );
    let (cause, _, requests, prefix) = ran.ended();
    assert_eq!((cause, requests, prefix), (Cause::Own, 7, 129_853), "the newest prefix, 129853, not a subagent's 999999");
    assert_eq!(flushes(&ran), 0);
}

#[test]
fn the_prefix_is_the_newest_requests_not_the_largest() {
    let ran = go(vec![Out(at("m1", 200_000)), Out(at("m2", 150_000))]);
    assert_eq!(ran.ended().3, 150_000);
}

// ---- the flush --------------------------------------------------------------

#[test]
fn below_the_ceiling_no_flush_is_written_and_at_it_exactly_one() {
    let below = go(vec![Out(at("m1", CEILING_TOKENS - 1))]);
    assert_eq!(flushes(&below), 0);
    assert_eq!(below.ended().0, Cause::Own);

    let at_it = go(vec![Out(at("m1", CEILING_TOKENS - 1)), Out(at("m2", CEILING_TOKENS)), Out(at("m3", CEILING_TOKENS + 10_000))]);
    assert_eq!(flushes(&at_it), 1, "{:?}", at_it.written());
    assert_eq!(at_it.written().len(), 2, "the start and the flush");
    assert_eq!(at_it.ended().0, Cause::Flushed);
    assert_eq!(CEILING_TOKENS, 300_000);
}

#[test]
fn a_result_before_the_ceiling_means_no_flush_ever() {
    let ran = go(vec![Out(at("m1", 10)), Out(result_line()), Out(at("m2", CEILING_TOKENS + 1))]);
    assert_eq!(flushes(&ran), 0);
    assert_eq!(ran.ended().0, Cause::Own);
    assert_eq!(ran.count(&Event::Interrupted), 0);
}

#[test]
fn stdin_closes_once_at_the_first_result_flush_or_no_flush_and_nothing_is_written_after() {
    let plain = go(vec![Out(at("m1", 10)), Out(result_line()), Out(result_line())]);
    assert_eq!(plain.count(&Event::InputClosed), 1);
    let closed_at = plain.events.iter().position(|event| *event == Event::InputClosed).unwrap();
    assert!(plain.events[closed_at..].iter().all(|event| !matches!(event, Event::Wrote(_))), "no input after the close");
    assert!(plain.events[..closed_at].iter().any(|event| matches!(event, Event::Wrote(_))), "the start went before");

    let flushed = go(vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())]);
    assert_eq!(flushed.count(&Event::InputClosed), 1);
    let closed_at = flushed.events.iter().position(|event| *event == Event::InputClosed).unwrap();
    assert_eq!(flushed.events[..closed_at].iter().filter(|event| matches!(event, Event::Wrote(_))).count(), 2, "start and flush before the close");
    assert!(flushed.events[closed_at..].iter().all(|event| !matches!(event, Event::Wrote(_))));

    let never = go(vec![Out(at("m1", 10))]);
    assert_eq!(never.count(&Event::InputClosed), 0, "no result, no close");
}

// ---- the interrupt ------------------------------------------------------------

#[test]
fn after_the_flush_the_grace_interrupts_exactly_once_and_not_a_second_early() {
    let grace = FLUSH_GRACE_S;
    // Just short of the grace: no interrupt, and the child ended on its own.
    let early = go(vec![Out(at("m1", CEILING_TOKENS)), Idle(grace - 1)]);
    assert_eq!(early.count(&Event::Interrupted), 0);
    assert_eq!(early.ended().0, Cause::Flushed);

    // At the grace, and long past it: one interrupt.
    let on_time = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(grace), Idle(grace), Idle(grace)], Some(130), true);
    assert_eq!(on_time.count(&Event::Interrupted), 1);
    assert_eq!(on_time.ended().0, Cause::Interrupted);

    // A child that ignores it is interrupted once and still waited for.
    let stubborn = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(grace), Idle(grace), Idle(grace)], Some(0), false);
    assert_eq!(stubborn.count(&Event::Interrupted), 1);
    assert_eq!(stubborn.count(&Event::Waited), 1);
    assert_eq!(FLUSH_GRACE_S, 900);
}

#[test]
fn the_wait_for_the_child_never_passes_the_grace_deadline_and_never_spins_after_the_interrupt() {
    let ran = go(vec![Out(at("m1", CEILING_TOKENS)), Idle(840), Idle(59), Idle(1)]);
    assert_eq!(ran.asked(), [60, 60, 60, 1, 60], "{:?}", ran.asked());
}

#[test]
fn a_prefix_at_the_ceiling_plus_the_allowance_interrupts_at_once_and_one_short_of_it_does_not() {
    let hard = CEILING_TOKENS + FLUSH_ALLOWANCE_TOKENS;
    let short = go(vec![Out(at("m1", CEILING_TOKENS)), Out(at("m2", hard - 1))]);
    assert_eq!(short.count(&Event::Interrupted), 0);
    assert_eq!(short.ended().0, Cause::Flushed);

    let at_hard = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Out(at("m2", hard)), Out(at("m3", hard + 1))], Some(130), true);
    assert_eq!(at_hard.count(&Event::Interrupted), 1);
    assert_eq!(at_hard.ended().0, Cause::Interrupted);
    assert_eq!(FLUSH_ALLOWANCE_TOKENS, 60_000);

    // The first request already past both: the flush is written, then the interrupt.
    let jump = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", hard + 5))], Some(130), true);
    assert_eq!((flushes(&jump), jump.count(&Event::Interrupted)), (1, 1));
}

#[test]
fn a_child_that_ends_before_the_grace_is_never_interrupted() {
    let ran = go(vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())]);
    assert_eq!(ran.count(&Event::Interrupted), 0);
    assert_eq!(ran.count(&Event::Waited), 1);
}

// ---- how it ends --------------------------------------------------------------

#[test]
fn the_outcome_names_how_it_ended_and_the_exit_status_follows_the_childs_code() {
    let own = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10)), Out(result_line())], Some(0), false);
    assert_eq!(own.outcome.as_ref().unwrap(), &SelfRunOutcome::Ended { cause: Cause::Own, code: Some(0), requests: 1, prefix: 10 });
    assert_eq!(own.outcome.as_ref().unwrap().to_string(), "ended: own code 0 requests 1 prefix 10");
    assert_eq!(own.outcome.unwrap().exit_status(), 0);

    let flushed = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())], Some(0), false);
    assert_eq!(flushed.outcome.as_ref().unwrap().to_string(), "ended: flushed code 0 requests 1 prefix 300000");
    assert_eq!(flushed.outcome.unwrap().exit_status(), 0);

    let nonzero = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10))], Some(2), false);
    assert_eq!(nonzero.outcome.as_ref().unwrap().to_string(), "ended: own code 2 requests 1 prefix 10");
    assert_eq!(nonzero.outcome.unwrap().exit_status(), 1);

    let signalled = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS + FLUSH_ALLOWANCE_TOKENS))], None, true);
    assert_eq!(signalled.outcome.as_ref().unwrap().to_string(), "ended: interrupted signal requests 1 prefix 360000");
    assert_eq!(signalled.outcome.unwrap().exit_status(), 1, "a child killed by a signal has no code");

    assert_eq!(SelfRunOutcome::NoRun.exit_status(), 3);
    assert_eq!(SelfRunOutcome::NoRun.to_string(), "no run: she has no single open Self run");
}

#[test]
fn unreadable_and_foreign_lines_are_skipped_and_never_printed() {
    let canary = "canary-stream-line-8c41";
    let mut foreign: Value = serde_json::from_str(&init_line()).unwrap();
    foreign["cwd"] = json!(canary);
    foreign["type"] = json!(canary);
    let ran = go(vec![
        Out(format!("not json {canary}")),
        Out(foreign.to_string()),
        Out(format!("[\"{canary}\"]")),
        Out(at("m1", 10)),
        Out(result_line()),
    ]);
    assert_eq!(ran.out, "request 1 prefix 10\n");
    let outcome = ran.outcome.as_ref().unwrap().to_string();
    assert!(!ran.out.contains(canary) && !outcome.contains(canary));
    assert!(!ran.written().iter().any(|text| text.contains(canary)), "nothing from the stream is written back");
}

#[test]
fn the_actuator_admits_nothing_to_the_mind_through_a_whole_flushed_run() {
    let mind = with_run();
    let seen = Seen::of(&mind);
    let ran = carry_with(&seen, vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())], Some(0), false);
    assert_eq!(ran.admits, 0);
    assert!(ran.outcome.is_ok());
    // And the run is exactly as she left it: still in force until her flush, or the next launch, closes it.
    assert_eq!(mind.ids(K::Resolution), Vec::<String>::new());
}

// ---- the real child -----------------------------------------------------------

#[cfg(unix)]
mod process {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::*;
    use crate::launch::SystemClock;

    /// A fake `claude`: the shell script `body`, in a directory that outlives the child.
    fn script(dir: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// Spawns it, retrying the one error a freshly written executable can give
    /// while another test's fork still holds its write handle.
    fn start(program: &Path, args: &[String]) -> Box<dyn Child> {
        for _ in 0..50 {
            match spawn_program(program, args) {
                Ok(child) => return child,
                Err(error) if format!("{error:#}").contains("busy") => std::thread::sleep(StdDuration::from_millis(100)),
                Err(error) => panic!("{error:#}"),
            }
        }
        panic!("the script stayed busy");
    }

    fn drain(child: &mut dyn Child) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match child.next_line(StdDuration::from_secs(20)) {
                Read::Line(line) => lines.push(line),
                Read::Closed => return lines,
                Read::Timeout => panic!("the fake claude hung"),
            }
        }
    }

    #[test]
    fn the_child_gets_its_arguments_and_lines_in_order_and_ends_when_its_input_closes() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "echo \"args: $*\"\nwhile read line; do echo \"got: $line\"; done\necho bye\nexit 7");
        let mut child = start(&program, &["-p".to_string(), "--max-turns".to_string(), "500".to_string()]);
        child.write_line("first");
        child.write_line("second");
        child.close_input();
        assert_eq!(drain(&mut *child), ["args: -p --max-turns 500", "got: first", "got: second", "bye"]);
        assert_eq!(child.wait(), Some(7));
    }

    #[test]
    fn a_silent_child_times_out_instead_of_blocking_and_a_signalled_child_has_no_code() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "kill -9 $$");
        let mut child = start(&program, &[]);
        assert_eq!(child.next_line(StdDuration::from_secs(20)), Read::Closed);
        assert_eq!(child.wait(), None);

        let quiet = script(&dir, "exec sleep 5");
        let mut child = start(&quiet, &[]);
        assert_eq!(child.next_line(StdDuration::from_millis(100)), Read::Timeout);
        child.interrupt();
        assert_eq!(child.next_line(StdDuration::from_secs(20)), Read::Closed);
        assert_eq!(child.wait(), None, "SIGINT ended the unprepared child");
    }

    #[test]
    fn interrupt_is_sigint_the_childs_own_handler_can_catch() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "trap 'echo interrupted; exit 130' INT\necho ready\nwhile :; do sleep 0.05; done");
        let mut child = start(&program, &[]);
        assert_eq!(child.next_line(StdDuration::from_secs(20)), Read::Line("ready".into()));
        child.interrupt();
        assert_eq!(drain(&mut *child), ["interrupted"]);
        assert_eq!(child.wait(), Some(130));
    }

    #[test]
    fn a_program_that_cannot_start_is_an_error_that_names_no_argument() {
        let canary = "canary-argument-6b0d";
        let error = spawn_program(Path::new("/nonexistent/claude"), &[canary.to_string()]).err().unwrap();
        assert!(!format!("{error:#}").contains(canary) && !format!("{error:#}").contains("nonexistent"), "{error:#}");
    }

    #[test]
    fn a_whole_run_through_the_real_child_flushes_at_the_ceiling_and_ends_on_the_result() {
        let dir = tempfile::tempdir().unwrap();
        let lines = [at("m1", 10), at("m2", CEILING_TOKENS)].join("\n");
        let result = result_line();
        // Prints two requests, waits for the flush on stdin, then the result, as a stand-in for a Self that obeys.
        let body = format!("cat <<'EOF'\n{lines}\nEOF\nread start\nread flush\ncat <<'EOF'\n{result}\nEOF\nwhile read more; do :; done\nexit 0");
        let program = script(&dir, &body);
        let mind = with_run();
        struct Spawn(PathBuf);
        impl Spawner for Spawn {
            fn spawn(&self, args: &[String]) -> Result<Box<dyn Child>> {
                Ok(start(&self.0, args))
            }
        }
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&mind), &SystemClock, &Spawn(program), &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::Ended { cause: Cause::Flushed, code: Some(0), requests: 2, prefix: 300_000 });
        assert_eq!(String::from_utf8(out).unwrap(), "request 1 prefix 10\nrequest 2 prefix 300000\n");
    }
}
