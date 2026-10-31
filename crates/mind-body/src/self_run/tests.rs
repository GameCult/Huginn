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
use crate::lease_client::{Lease, LeaseRefused, token_holding};
use crate::testkit::*;

/// A cap no constant of the library equals, so that a request of it came from the argument.
const TEST_CAP_S: u64 = 5_400;

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
    /// A signal was attempted, delivered or not.
    Stopped(Stop),
    Asked(StdDuration),
    /// The connector was asked for a lease and answered one.
    Lent,
    /// A child was started.
    Spawned,
    /// The lease was settled with this.
    Settled(Settlement),
    /// The lease was dropped unsettled: the connector bills the reservation.
    Abandoned,
}

#[derive(Clone)]
enum Step {
    Line(String),
    /// Time passes with no output: the clock moves and the read times out.
    Idle(i64),
    /// The clock moves, then this line arrives.
    After(i64, String),
    /// The child is reaped with this code; its stream stays open.
    Exit(Option<i32>),
    /// Its stdout ends; the child is not reaped yet.
    Eof,
}

use Step::{Eof, Exit, Idle, Line as Out};

struct Scripted {
    steps: VecDeque<Step>,
    log: Rc<RefCell<Vec<Event>>>,
    clock: Rc<Fixed>,
    code: Option<i32>,
    /// The weakest signal that ends it; weaker ones are ignored.
    dies_on: Option<Stop>,
    /// Signals that cannot be delivered (no `kill` program, say).
    refuses: Vec<Stop>,
    /// When the steps run out it neither exits nor closes its stream: time just passes.
    hangs: bool,
    exited: bool,
    eof: bool,
    /// How many times it was asked: a loop that never ends is a failure, not a hang.
    asked: u32,
}

impl Child for Scripted {
    fn write_line(&mut self, line: &str) {
        self.log.borrow_mut().push(Event::Wrote(line.to_string()));
    }

    fn close_input(&mut self) {
        self.log.borrow_mut().push(Event::InputClosed);
    }

    fn next(&mut self, timeout: StdDuration) -> Read {
        self.log.borrow_mut().push(Event::Asked(timeout));
        self.asked += 1;
        assert!(self.asked < 500, "the loop never ended");
        match self.steps.pop_front() {
            Some(Step::Line(line)) => Read::Line(line),
            Some(Step::Idle(seconds)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                Read::Timeout
            }
            Some(Step::After(seconds, line)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                Read::Line(line)
            }
            Some(Step::Exit(code)) => {
                self.exited = true;
                Read::Exited(code)
            }
            Some(Step::Eof) => {
                self.eof = true;
                Read::Eof
            }
            // The stream is over: the child is reaped, then its stdout ends.
            None if self.hangs => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(timeout.as_secs() as i64));
                Read::Timeout
            }
            None if !self.exited => {
                self.exited = true;
                Read::Exited(self.code)
            }
            None if !self.eof => {
                self.eof = true;
                Read::Eof
            }
            None => panic!("the loop kept waiting on a child that ended"),
        }
    }

    fn stop(&mut self, how: Stop) -> bool {
        self.log.borrow_mut().push(Event::Stopped(how));
        if self.refuses.contains(&how) {
            return false;
        }
        if self.dies_on.is_some_and(|weakest| how >= weakest) {
            self.steps.clear();
            self.hangs = false;
        }
        true
    }
}

struct Fake {
    child: RefCell<Option<Scripted>>,
    spawns: RefCell<Vec<Vec<String>>>,
    refuse: bool,
}

impl Spawner for Fake {
    fn spawn(&self, args: &[String], _token: &TokenFd) -> Result<Box<dyn Child>> {
        self.spawns.borrow_mut().push(args.to_vec());
        if self.refuse {
            return Err(anyhow!("refused"));
        }
        let child = self.child.borrow_mut().take().expect("spawned twice");
        child.log.borrow_mut().push(Event::Spawned);
        Ok(Box::new(child))
    }
}

/// What the connector answers.
#[derive(Clone, Copy)]
enum Answer {
    Grant,
    /// Lends, then fails the settle.
    GrantSettleFails,
    Refuse(LeaseRefusalKind),
    Unreachable,
}

/// The connector, as the actuator asks it: it logs into the child's log, so the
/// order of lend, spawn, the child's end and settle is one sequence.
struct FakeLender {
    log: Rc<RefCell<Vec<Event>>>,
    asked: RefCell<Vec<LeaseRequest>>,
    answer: Answer,
}

impl FakeLender {
    fn new(answer: Answer) -> Self {
        Self::logging(Rc::new(RefCell::new(Vec::new())), answer)
    }

    fn logging(log: Rc<RefCell<Vec<Event>>>, answer: Answer) -> Self {
        Self { log, asked: RefCell::new(Vec::new()), answer }
    }
}

impl Lender for FakeLender {
    fn lend(&self, request: &LeaseRequest) -> Result<Lent> {
        self.asked.borrow_mut().push(request.clone());
        match self.answer {
            Answer::Grant | Answer::GrantSettleFails => {
                self.log.borrow_mut().push(Event::Lent);
                Ok(Lent::Granted(Box::new(FakeLease { log: self.log.clone(), token: token_holding("tok-canary"), settled: false, fails: matches!(self.answer, Answer::GrantSettleFails) })))
            }
            Answer::Refuse(kind) => Ok(Lent::Refused(LeaseRefused { kind, detail: "detail-canary".into() })),
            Answer::Unreachable => Err(anyhow!("down")),
        }
    }
}

struct FakeLease {
    log: Rc<RefCell<Vec<Event>>>,
    token: TokenFd,
    settled: bool,
    /// The settle reaches the connector's side and fails there.
    fails: bool,
}

impl Lease for FakeLease {
    fn token(&self) -> &TokenFd {
        &self.token
    }

    fn settle(mut self: Box<Self>, settlement: Settlement) -> Result<u64> {
        self.settled = true;
        self.log.borrow_mut().push(Event::Settled(settlement));
        if self.fails { Err(anyhow!("detail-canary")) } else { Ok(7) }
    }
}

impl Drop for FakeLease {
    fn drop(&mut self) {
        if !self.settled {
            self.log.borrow_mut().push(Event::Abandoned);
        }
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
    /// Every event, the lease's included.
    all: Vec<Event>,
    spawns: Vec<Vec<String>>,
    admits: usize,
    asked: Vec<LeaseRequest>,
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

    fn count_all(&self, wanted: &Event) -> usize {
        self.all.iter().filter(|event| *event == wanted).count()
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

/// `dies_on_interrupt`: the child ends at the first signal, else it ignores all.
fn carry_with(seen: &Seen, steps: Vec<Step>, code: Option<i32>, dies_on_interrupt: bool) -> Ran {
    carry_signalled(seen, steps, code, dies_on_interrupt.then_some(Stop::Interrupt), &[])
}

fn carry_signalled(seen: &Seen, steps: Vec<Step>, code: Option<i32>, dies_on: Option<Stop>, refuses: &[Stop]) -> Ran {
    carry_scripted(seen, steps, code, dies_on, refuses, false)
}

fn carry_scripted(seen: &Seen, steps: Vec<Step>, code: Option<i32>, dies_on: Option<Stop>, refuses: &[Stop], hangs: bool) -> Ran {
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let fake = Fake {
        child: RefCell::new(Some(Scripted { steps: steps.into(), log: log.clone(), clock: clock.clone(), code, dies_on, refuses: refuses.to_vec(), hangs, exited: false, eof: false, asked: 0 })),
        spawns: RefCell::new(Vec::new()),
        refuse: false,
    };
    let lender = FakeLender::logging(log.clone(), Answer::Grant);
    let mut out = Vec::new();
    let outcome = carry(seen, &*clock, &lender, &fake, TEST_CAP_S, &mut out);
    let all = log.borrow().clone();
    // The child's own events; the lease's are in `all`, in order with them.
    let events = all.iter().filter(|event| !matches!(event, Event::Lent | Event::Spawned | Event::Settled(_) | Event::Abandoned)).cloned().collect();
    let spawns = fake.spawns.borrow().clone();
    Ran { outcome, out: String::from_utf8(out).unwrap(), events, all, spawns, admits: seen.admits.get(), asked: lender.asked.borrow().clone() }
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
    assert!(carry(&Seen::of(&mind), &*clock, &FakeLender::new(Answer::Grant), &refusing, TEST_CAP_S, &mut Vec::new()).is_err());
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
    assert!(carry(&Down, &*clock, &FakeLender::new(Answer::Grant), &never, TEST_CAP_S, &mut Vec::new()).is_err());
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
            "--setting-sources",
            "user",
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
    // Trailing zeros are significant: "10" is ten dollars, not one.
    for budget in ["0.5", "5", "12.25", "10", "100"] {
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
    for needle in ["Use the eureka skill.", "yggdrasil", "yggdrasil:run:selfrun-a", AGENT] {
        assert!(written[0].contains(needle), "{needle}: {}", written[0]);
    }
    assert_eq!(written.len(), 1, "no flush without the ceiling");
}

#[test]
fn the_start_message_opens_with_the_skill_the_seed_ships() {
    let message = start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"));
    assert!(message.starts_with("Use the eureka skill. You are Self for the instance `yggdrasil`;"), "{message}");
    assert!(!message.contains("mind-self skill"), "{message}");
}

#[cfg(unix)]
mod seed_links {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn a_relative_in_tree_link_is_copied_as_a_link_that_resolves() {
        let seed = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(seed.path().join("skills/eureka/agents")).unwrap();
        std::fs::write(seed.path().join("skills/eureka/agents/hands.md"), "hands").unwrap();
        symlink("skills/eureka/agents", seed.path().join("agents")).unwrap();
        std::fs::create_dir(seed.path().join("sub")).unwrap();
        symlink("../skills/eureka", seed.path().join("sub/eureka")).unwrap();
        let out = tempfile::tempdir().unwrap();
        copy_tree(seed.path(), out.path()).unwrap();
        assert!(out.path().join("agents").symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_link(out.path().join("agents")).unwrap(), Path::new("skills/eureka/agents"));
        assert_eq!(std::fs::read_to_string(out.path().join("agents/hands.md")).unwrap(), "hands");
        assert_eq!(std::fs::read_to_string(out.path().join("sub/eureka/agents/hands.md")).unwrap(), "hands");
    }

    #[test]
    fn an_absolute_or_escaping_link_is_refused_with_the_typed_error() {
        for target in ["/etc/passwd", "../outside", "a/../../outside", "missing"] {
            let seed = tempfile::tempdir().unwrap();
            symlink(target, seed.path().join("bad")).unwrap();
            let out = tempfile::tempdir().unwrap();
            let error = copy_tree(seed.path(), out.path()).unwrap_err();
            let refused = error.get_ref().and_then(|inner| inner.downcast_ref::<SeedLinkRefused>());
            assert_eq!(refused.map(|r| r.target.as_path()), Some(Path::new(target)), "{target}: {error}");
            assert!(out.path().join("bad").symlink_metadata().is_err(), "{target} was copied");
        }
    }

    #[test]
    fn a_link_through_another_in_tree_link_cannot_escape() {
        let parent = tempfile::tempdir().unwrap();
        let seed = parent.path().join("seed");
        std::fs::create_dir(&seed).unwrap();
        symlink(".", seed.join("c")).unwrap();
        symlink("c/..", seed.join("a")).unwrap();
        let out = tempfile::tempdir().unwrap();
        let error = copy_tree(&seed, out.path()).unwrap_err();
        let refused = error.get_ref().and_then(|inner| inner.downcast_ref::<SeedLinkRefused>());
        assert_eq!(refused.map(|r| r.target.as_path()), Some(Path::new("c/..")), "{error}");
    }

    /// A seed whose parent is not the config dir's parent, and a sibling of the
    /// config dir named `seed` holding the same file name: the copy's link text
    /// resolves there, though in the source it lands under the seed root.
    fn refused_in_the_copy(plant: impl Fn(&Path), expected: &str) {
        let source = tempfile::tempdir().unwrap();
        let seed = source.path().join("seed");
        std::fs::create_dir(&seed).unwrap();
        std::fs::write(seed.join("f"), "seed").unwrap();
        plant(&seed);
        let runtime = tempfile::tempdir().unwrap();
        std::fs::create_dir(runtime.path().join("seed")).unwrap();
        std::fs::write(runtime.path().join("seed/f"), "planted").unwrap();
        let out = runtime.path().join("claude-config");
        std::fs::create_dir(&out).unwrap();
        let error = copy_tree(&seed, &out).unwrap_err();
        let refused = error.get_ref().and_then(|inner| inner.downcast_ref::<SeedLinkRefused>());
        assert_eq!(refused.map(|r| r.target.as_path()), Some(Path::new(expected)), "{error}");
    }

    #[test]
    fn a_link_that_leaves_the_seed_and_returns_by_its_name_is_refused() {
        refused_in_the_copy(|seed| symlink("../seed/f", seed.join("x")).unwrap(), "../seed/f");
    }

    #[test]
    fn a_link_through_a_dot_link_to_the_seeds_name_is_refused() {
        refused_in_the_copy(
            |seed| {
                symlink(".", seed.join("c")).unwrap();
                symlink("c/../seed/f", seed.join("a")).unwrap();
            },
            "c/../seed/f",
        );
    }
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
        "request 1 prefix 84210\nrequest 2 prefix 100751\nrequest 3 prefix 117174\nrequest 4 prefix 118336\nrequest 5 prefix 126746\nrequest 6 prefix 127250\nrequest 7 prefix 129853\nsettled: billed 7 micro-USD\n"
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
    assert_eq!(at_it.ended().0, Cause::FlushRequested);
    assert_eq!(CEILING_TOKENS, 300_000);
}

#[test]
fn a_result_before_the_ceiling_means_no_flush_ever() {
    let ran = go(vec![Out(at("m1", 10)), Out(result_line()), Out(at("m2", CEILING_TOKENS + 1))]);
    assert_eq!(flushes(&ran), 0);
    assert_eq!(ran.ended().0, Cause::Own);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
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
    assert_eq!(early.count(&Event::Stopped(Stop::Interrupt)), 0);
    assert_eq!(early.ended().0, Cause::FlushRequested);

    // At the grace, and long past it: one interrupt.
    let on_time = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(grace), Idle(grace), Idle(grace)], Some(130), true);
    assert_eq!(on_time.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(on_time.ended().0, Cause::Signalled(Stop::Interrupt));

    // Exactly at the grace is the grace: the interrupt comes then, not on the next wait.
    let exact = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(grace)], Some(130), true);
    assert_eq!(exact.ended().0, Cause::Signalled(Stop::Interrupt));

    // A child that ignores it is interrupted once and still waited for.
    let stubborn = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(grace), Idle(grace), Idle(grace)], Some(0), false);
    assert_eq!(stubborn.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(stubborn.ended().1, Some(0), "its own end is waited for");
    assert_eq!(FLUSH_GRACE_S, 900);
}

#[test]
fn the_wait_for_the_child_never_passes_the_grace_deadline_and_never_spins_after_the_interrupt() {
    let ran = go(vec![Out(at("m1", CEILING_TOKENS)), Idle(840), Idle(59), Idle(1)]);
    // 60 until the grace is near, the one second left, then the escalation's own wait, then the drain after the child's exit.
    assert_eq!(ran.asked(), [60, 60, 60, 1, 60, 2], "{:?}", ran.asked());
}

#[test]
fn the_headroom_counts_the_requests_after_the_first_result_that_follows_the_flush_and_one_short_of_it_does_not_interrupt() {
    let hard = CEILING_TOKENS + FLUSH_HEADROOM_TOKENS;
    let short = go(vec![Out(at("m1", CEILING_TOKENS)), Out(result_line()), Out(at("m2", hard - 1))]);
    assert_eq!(short.count(&Event::Stopped(Stop::Interrupt)), 0);
    assert_eq!(short.ended().0, Cause::FlushRequested);

    let at_hard = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Out(result_line()), Out(at("m2", hard)), Out(at("m3", hard + 1))], Some(130), true);
    assert_eq!(at_hard.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(at_hard.ended().0, Cause::Signalled(Stop::Interrupt));
    assert_eq!(FLUSH_HEADROOM_TOKENS, 60_000);

    // Before that result the running turn's own requests are not the flush's turn: however far they grow, no interrupt.
    let same_turn = go(vec![Out(at("m1", CEILING_TOKENS)), Out(at("m2", hard)), Out(at("m3", hard + 500_000))]);
    assert_eq!(same_turn.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn a_jump_past_the_ceiling_plus_the_headroom_writes_the_flush_and_the_running_turn_cannot_use_up_the_flushs_turn() {
    let jump = carry_with(
        &Seen::of(&with_run()),
        vec![Out(at("m1", 290_000)), Out(at("m2", 400_000)), Out(at("m3", 470_000)), Out(result_line()), Out(at("m4", 459_999)), Out(at("m5", 460_000)), Out(at("m6", 470_000))],
        Some(130),
        true,
    );
    let kinds: Vec<&str> = jump
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Wrote(line) if line.contains("context ceiling") => Some("flush"),
            Event::Stopped(_) => Some("interrupt"),
            Event::InputClosed => Some("closed"),
            Event::Asked(_) => Some("read"),
            _ => None,
        })
        .collect();
    // Reads: m1, m2 (the jump, which writes the flush), m3 (the running turn's 470k, past the headroom: no interrupt).
    assert_eq!(kinds[..5], ["read", "read", "flush", "read", "read"], "{kinds:?}");
    // The headroom is measured from the prefix the flush was asked at (400k) and counts the requests after the
    // first result only: 459_999 does not interrupt; 460_000 does, once, and m6 is never read.
    assert_eq!(jump.count(&Event::Stopped(Stop::Interrupt)), 1, "{kinds:?}");
    assert_eq!(jump.out, "request 1 prefix 290000\nrequest 2 prefix 400000\nrequest 3 prefix 470000\nrequest 4 prefix 459999\nrequest 5 prefix 460000\nsettled: billed 7 micro-USD\n");
    assert_eq!(jump.ended().0, Cause::Signalled(Stop::Interrupt));
    assert_eq!(flushes(&jump), 1);
}

#[test]
fn a_child_that_ends_before_the_grace_is_never_interrupted() {
    let ran = go(vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())]);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
    assert_eq!(ran.ended().0, Cause::FlushRequested);
}

#[test]
fn the_grace_is_the_full_grace_from_the_moment_the_flush_was_asked_whichever_request_that_was() {
    let grace = FLUSH_GRACE_S;
    // Five requests, time passing before the ceiling, then the flush on the sixth.
    let before: Vec<Step> = vec![Out(at("m1", 10)), Idle(500), Out(at("m2", 20)), Out(at("m3", 30)), Out(at("m4", 40)), Out(at("m5", 50)), Out(at("m6", CEILING_TOKENS))];
    let early = carry_with(&Seen::of(&with_run()), [before.clone(), vec![Idle(grace - 1)]].concat(), Some(0), true);
    assert_eq!((flushes(&early), early.count(&Event::Stopped(Stop::Interrupt))), (1, 0), "one second short of the grace, flush on request 6");
    let on_time = carry_with(&Seen::of(&with_run()), [before, vec![Idle(grace - 1), Idle(1)]].concat(), Some(0), true);
    assert_eq!(on_time.count(&Event::Stopped(Stop::Interrupt)), 1, "the grace is the same after six requests as after one");
}

#[test]
fn the_message_ids_of_a_long_run_are_all_remembered() {
    let ids: Vec<String> = (1..=12).map(|n| format!("m{n}")).collect();
    let mut steps: Vec<Step> = ids.iter().map(|id| Out(at(id, 1_000))).collect();
    // Every earlier request seen again, the oldest and the newest among them: none counts twice.
    steps.extend(ids.iter().map(|id| Out(at(id, 2_000))));
    let ran = go(steps);
    let (_, _, requests, prefix) = ran.ended();
    assert_eq!((requests, prefix), (12, 1_000));
    assert_eq!(ran.out.lines().filter(|line| line.starts_with("request ")).count(), 12);
}

#[test]
fn a_usage_sum_that_overflows_is_at_or_over_the_ceiling_not_a_panic_or_a_wrap() {
    let line = request("m", u64::MAX, 1, 0);
    assert_eq!(read_usage(&line), Usage::Request { id: "m".into(), prefix: u64::MAX });
    assert_eq!(read_usage(&request("m", u64::MAX, u64::MAX, u64::MAX)), Usage::Request { id: "m".into(), prefix: u64::MAX });
    assert_eq!(read_usage(&request("m", 1, u64::MAX, 1)), Usage::Request { id: "m".into(), prefix: u64::MAX });
    // Through the actuator: the flush is asked for, and the headroom beyond a saturated prefix does not overflow
    // either, and the pass that wrote the flush does not interrupt.
    let ran = go(vec![Out(request("m1", u64::MAX, 7, 0)), Out(request("m2", 0, 0, 0))]);
    assert_eq!(flushes(&ran), 1);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn a_result_before_any_request_closes_the_input_once_and_the_run_ends_with_a_typed_outcome() {
    // The shape of an API or authentication failure: the child reports an error result and no request ever came.
    let ran = carry_with(&Seen::of(&with_run()), vec![Out(result_line())], Some(1), false);
    assert_eq!(ran.count(&Event::InputClosed), 1);
    assert_eq!(ran.ended(), (Cause::Own, Some(1), 0, 0));
    assert_eq!(ran.outcome.as_ref().unwrap().exit_status(), 1);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn without_a_flush_nothing_is_ever_signalled_however_large_the_prefix_grows() {
    let ran = go(vec![Out(at("m1", 10)), Out(result_line()), Out(at("m2", 900_000)), Idle(100_000)]);
    assert_eq!(ran.events.iter().filter(|event| matches!(event, Event::Stopped(_))).count(), 0);
    assert_eq!(ran.ended().0, Cause::Own);
}

#[test]
fn a_child_that_ignores_a_signal_gets_the_next_one_after_the_bound_and_the_outcome_names_the_one_that_ended_it() {
    let hung = |dies_on: Option<Stop>, refuses: &[Stop], rest: Vec<Step>| {
        carry_signalled(&Seen::of(&with_run()), [vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], rest].concat(), Some(0), dies_on, refuses)
    };
    let stops = |ran: &Ran| -> Vec<Event> { ran.events.iter().filter(|event| matches!(event, Event::Stopped(_))).cloned().collect() };
    let after = ESCALATE_AFTER_S;

    // Ignores SIGINT and SIGTERM: each next signal waits out the bound, SIGKILL ends it.
    let killed = hung(Some(Stop::Kill), &[], vec![Idle(after - 1), Idle(1), Idle(after - 1), Idle(1), Idle(after)]);
    assert_eq!(stops(&killed), [Event::Stopped(Stop::Interrupt), Event::Stopped(Stop::Terminate), Event::Stopped(Stop::Kill)]);
    assert_eq!(killed.ended().0, Cause::Signalled(Stop::Kill));

    // One second short of the bound there is no second signal.
    let waiting = hung(None, &[], vec![Idle(after - 1)]);
    assert_eq!(stops(&waiting), [Event::Stopped(Stop::Interrupt)]);

    // SIGTERM ends it: the outcome says terminated, and nothing stronger is sent.
    let terminated = hung(Some(Stop::Terminate), &[], vec![Idle(after), Idle(after)]);
    assert_eq!(stops(&terminated), [Event::Stopped(Stop::Interrupt), Event::Stopped(Stop::Terminate)]);
    assert_eq!(terminated.outcome.as_ref().unwrap().to_string(), "ended: terminated code 0 requests 1 prefix 300000");
}

#[test]
fn a_signal_that_could_not_be_delivered_is_never_reported_as_sent() {
    let hung = |refuses: &[Stop], rest: Vec<Step>| {
        carry_signalled(&Seen::of(&with_run()), [vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], rest].concat(), Some(0), None, refuses)
    };
    // SIGINT is refused and the child ends by itself afterwards: the flush was requested, nothing interrupted it.
    let refused = hung(&[Stop::Interrupt], vec![]);
    assert_eq!(refused.count(&Event::Stopped(Stop::Interrupt)), 1, "it was attempted");
    assert_eq!(refused.ended().0, Cause::FlushRequested);

    // SIGINT delivered but ignored, SIGTERM refused, the child ends on its own: the strongest delivered is SIGINT.
    let partly = hung(&[Stop::Terminate], vec![Idle(ESCALATE_AFTER_S)]);
    assert_eq!(partly.ended().0, Cause::Signalled(Stop::Interrupt));

    // Not even SIGKILL is delivered and the child never ends: after the bound the run is over without it, and the label says so.
    let stuck = carry_scripted(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], Some(0), None, &[Stop::Interrupt, Stop::Terminate, Stop::Kill], true);
    assert_eq!(stuck.ended(), (Cause::Unstoppable, None, 1, CEILING_TOKENS));
    assert_eq!(stuck.events.iter().filter(|event| matches!(event, Event::Stopped(_))).count(), 3);
    assert_eq!(stuck.outcome.unwrap().exit_status(), 1);
}

/// A clock that counts its reads.
struct Counting(Fixed, Cell<u32>);

impl Clock for Counting {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.1.set(self.1.get() + 1);
        self.0.now()
    }
}

#[test]
fn the_clock_is_read_once_per_event_so_a_step_of_the_wall_clock_cannot_split_a_pass() {
    let mind = with_run();
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Counting(Fixed(Cell::new(at_start())), Cell::new(0)));
    // The child's reads move the clock by far more than the grace, as a stepped wall clock would.
    struct Stepped(Rc<Counting>);
    impl Clock for Stepped {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            let now = self.0.now();
            self.0.0.0.set(now + ChronoDuration::seconds(FLUSH_GRACE_S + 1));
            now
        }
    }
    let steps: VecDeque<Step> = vec![Out(at("m1", CEILING_TOKENS)), Out(at("m2", CEILING_TOKENS + 1)), Idle(0)].into();
    let child = Scripted { steps, log: log.clone(), clock: Rc::new(Fixed(Cell::new(at_start()))), code: Some(0), dies_on: None, refuses: vec![], hangs: false, exited: false, eof: false, asked: 0 };
    let fake = Fake { child: RefCell::new(Some(child)), spawns: RefCell::new(Vec::new()), refuse: false };
    let outcome = carry(&Seen::of(&mind), &Stepped(clock.clone()), &FakeLender::new(Answer::Grant), &fake, TEST_CAP_S, &mut Vec::new()).unwrap();
    let events = log.borrow().clone();
    let reads = events.iter().filter(|event| matches!(event, Event::Asked(_))).count() as u32;
    // One read for the lease's expiry, one before the first wait, then one per fact heard:
    // never a second read inside a pass.
    assert_eq!(clock.1.get(), 2 + reads, "{events:?}");
    // The flush was written on the first request at time 0; the very next pass sees the stepped clock.
    // The flush pass itself has no interrupt, whatever the step: the interrupt is on a later event.
    let flush_at = events.iter().position(|event| matches!(event, Event::Wrote(line) if line.contains("context ceiling"))).unwrap();
    let stop_at = events.iter().position(|event| matches!(event, Event::Stopped(_))).unwrap();
    assert!(events[flush_at..stop_at].iter().any(|event| matches!(event, Event::Asked(_))), "a read between the flush and the interrupt: {events:?}");
    assert_eq!(outcome.exit_status(), 0);
}

#[test]
fn a_deadline_that_came_during_a_line_is_answered_without_waiting_for_the_next_poll() {
    // The result arrives after the grace has run out: the input closes, and the interrupt is due at once.
    let ran = carry_scripted(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Step::After(FLUSH_GRACE_S + 5, result_line())], Some(130), Some(Stop::Interrupt), &[], true);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(ran.count(&Event::InputClosed), 1);
    let asks = ran.asked();
    assert_eq!(asks[2], 0, "the wait after the result: the deadline is past, not a poll away: {asks:?}");
}

#[test]
fn a_child_that_exited_is_never_signalled_however_long_its_stream_stays_open() {
    // Flush asked for, the child exits, then the grace and more pass with its stream still open.
    let ran = carry_scripted(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Exit(Some(0)), Idle(1)], Some(0), None, &[], true);
    assert_eq!(ran.events.iter().filter(|event| matches!(event, Event::Stopped(_))).count(), 0);
    assert_eq!(ran.ended(), (Cause::FlushRequested, Some(0), 1, CEILING_TOKENS));
    // The stream ending first and the exit after: the same end.
    let eof_first = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10)), Eof], Some(0), false);
    assert_eq!(eof_first.ended(), (Cause::Own, Some(0), 1, 10));
}

// ---- how it ends --------------------------------------------------------------

#[test]
fn the_outcome_names_how_it_ended_and_the_exit_status_follows_the_childs_code() {
    let own = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10)), Out(result_line())], Some(0), false);
    assert_eq!(own.outcome.as_ref().unwrap(), &SelfRunOutcome::Ended { cause: Cause::Own, code: Some(0), requests: 1, prefix: 10 });
    assert_eq!(own.outcome.as_ref().unwrap().to_string(), "ended: own code 0 requests 1 prefix 10");
    assert_eq!(own.outcome.unwrap().exit_status(), 0);

    let flushed = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Out(result_line())], Some(0), false);
    assert_eq!(flushed.outcome.as_ref().unwrap().to_string(), "ended: flush-requested code 0 requests 1 prefix 300000");
    assert_eq!(flushed.outcome.unwrap().exit_status(), 0);

    let nonzero = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", 10))], Some(2), false);
    assert_eq!(nonzero.outcome.as_ref().unwrap().to_string(), "ended: own code 2 requests 1 prefix 10");
    assert_eq!(nonzero.outcome.unwrap().exit_status(), 1);

    let signalled = carry_with(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], None, true);
    assert_eq!(signalled.outcome.as_ref().unwrap().to_string(), "ended: interrupted signal requests 1 prefix 300000");
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
    assert_eq!(ran.out, "request 1 prefix 10\nsettled: billed 7 micro-USD\n");
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
    mod lease_child;

    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::*;
    use crate::launch::SystemClock;

    /// A fake `claude`: the shell script `body`, in a directory that outlives the child.
    fn script(dir: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("claude");
        std::fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// Spawns it, retrying the one error a freshly written executable can give
    /// while another test's fork still holds its write handle.
    fn start(program: &Path, args: &[String]) -> Box<dyn Child> {
        start_with(program, args, &token_holding("tok-canary"))
    }

    /// What the unit would hold: what a shell needs, and credential-shaped variables
    /// that live only here, never in the test process, so only the spawner can pass them on.
    fn unit_environment() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let mut unit: Vec<(std::ffi::OsString, std::ffi::OsString)> = vec![("PATH".into(), "/usr/local/bin:/usr/bin:/bin".into())];
        for name in CREDENTIAL_CANARIES {
            unit.push((name.into(), format!("canary-{name}").into()));
        }
        unit
    }

    const CREDENTIAL_CANARIES: [&str; 6] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_API_KEY_HELPER_TTL_MS", "UNLISTED_VARIABLE"];

    fn start_with(program: &Path, args: &[String], token: &TokenFd) -> Box<dyn Child> {
        for _ in 0..50 {
            let config = FreshConfig::create(program.parent().unwrap(), None).unwrap();
            match spawn_program(program, args, token, &unit_environment(), config) {
                Ok(child) => return child,
                Err(error) if format!("{error:#}").contains("busy") => std::thread::sleep(StdDuration::from_millis(100)),
                Err(error) => panic!("{error:#}"),
            }
        }
        panic!("the script stayed busy");
    }

    /// Everything the child gives until it has been reaped and its stdout has ended.
    fn drain(child: &mut dyn Child) -> (Vec<String>, Option<i32>) {
        let (mut lines, mut code, mut eof) = (Vec::new(), None, false);
        loop {
            match child.next(StdDuration::from_secs(20)) {
                Read::Line(line) => lines.push(line),
                Read::Eof => eof = true,
                Read::Exited(status) => code = Some(status),
                Read::Timeout => panic!("the fake claude hung"),
            }
            if let (true, Some(status)) = (eof, code) {
                return (lines, status);
            }
        }
    }

    /// Whether a process of this id still runs. A zombie is dead: with no init to reap
    /// it (cargo as PID 1 in a one-command job) it stays in the table after it is killed.
    fn alive(pid: &str) -> bool {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return false };
        stat.rsplit_once(')').is_some_and(|(_, rest)| !rest.trim_start().starts_with('Z'))
    }

    #[test]
    fn the_child_gets_its_arguments_and_lines_in_order_and_ends_when_its_input_closes() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "echo \"args: $*\"\nwhile read line; do echo \"got: $line\"; done\necho bye\nexit 7");
        let mut child = start(&program, &["-p".to_string(), "--max-turns".to_string(), "500".to_string()]);
        child.write_line("first");
        child.write_line("second");
        child.close_input();
        assert_eq!(drain(&mut *child), (["args: -p --max-turns 500", "got: first", "got: second", "bye"].map(String::from).to_vec(), Some(7)));
    }

    #[test]
    fn a_silent_child_times_out_instead_of_blocking_and_a_signalled_child_has_no_code() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "kill -9 $$");
        let mut child = start(&program, &[]);
        assert_eq!(drain(&mut *child), (Vec::new(), None));

        let quiet = script(&dir, "exec sleep 5");
        let mut child = start(&quiet, &[]);
        assert_eq!(child.next(StdDuration::from_millis(100)), Read::Timeout);
        child.stop(Stop::Interrupt);
        assert_eq!(drain(&mut *child), (Vec::new(), None), "SIGINT ended the unprepared child");
    }

    #[test]
    fn interrupt_is_sigint_the_childs_own_handler_can_catch() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "trap 'echo interrupted; exit 130' INT\necho ready\nwhile :; do sleep 0.05; done");
        let mut child = start(&program, &[]);
        assert_eq!(child.next(StdDuration::from_secs(20)), Read::Line("ready".into()));
        child.stop(Stop::Interrupt);
        assert_eq!(drain(&mut *child), (vec!["interrupted".to_string()], Some(130)));
    }

    #[test]
    fn a_program_that_cannot_start_is_an_error_that_names_no_argument() {
        let canary = "canary-argument-6b0d";
        let scratch = tempfile::tempdir().unwrap();
        let config = FreshConfig::create(scratch.path(), None).unwrap();
        let error = spawn_program(Path::new("/nonexistent/claude"), &[canary.to_string()], &token_holding("tok-canary"), &[], config).err().unwrap();
        assert!(!format!("{error:#}").contains(canary) && !format!("{error:#}").contains("nonexistent"), "{error:#}");
    }

    /// A read that times out here means the stream stopped, never a wait worth 60 s:
    /// fail at once instead of hanging the suite.
    struct Prompt(Box<dyn Child>);
    impl Child for Prompt {
        fn write_line(&mut self, line: &str) {
            self.0.write_line(line)
        }
        fn close_input(&mut self) {
            self.0.close_input()
        }
        fn next(&mut self, timeout: StdDuration) -> Read {
            match self.0.next(StdDuration::from_secs(20).min(timeout)) {
                Read::Timeout => panic!("the fake claude went quiet"),
                read => read,
            }
        }
        fn stop(&mut self, how: Stop) -> bool {
            self.0.stop(how)
        }
    }

    struct Spawn(PathBuf);
    impl Spawner for Spawn {
        fn spawn(&self, args: &[String], token: &TokenFd) -> Result<Box<dyn Child>> {
            Ok(Box::new(Prompt(start_with(&self.0, args, token))))
        }
    }

    /// The real child as it is: a read may time out, which is how the drain after an exit is waited.
    /// A run that has not ended by 15 s is a failure, not a hang.
    struct Bounded(Box<dyn Child>, std::time::Instant);
    impl Child for Bounded {
        fn write_line(&mut self, line: &str) {
            self.0.write_line(line)
        }
        fn close_input(&mut self) {
            self.0.close_input()
        }
        fn next(&mut self, timeout: StdDuration) -> Read {
            assert!(self.1.elapsed() < StdDuration::from_secs(15), "the run did not end");
            self.0.next(StdDuration::from_secs(1).min(timeout))
        }
        fn stop(&mut self, how: Stop) -> bool {
            self.0.stop(how)
        }
    }

    struct Patient(PathBuf);
    impl Spawner for Patient {
        fn spawn(&self, args: &[String], token: &TokenFd) -> Result<Box<dyn Child>> {
            Ok(Box::new(Bounded(start_with(&self.0, args, token), std::time::Instant::now())))
        }
    }

    #[test]
    fn a_whole_run_through_the_real_child_flushes_at_the_ceiling_and_ends_on_the_result() {
        let dir = tempfile::tempdir().unwrap();
        let lines = [at("m1", 10), at("m2", CEILING_TOKENS)].join("\n");
        let result = result_line();
        // Prints two requests, waits for the flush on stdin, then the result, as a stand-in for a Self that obeys.
        let body = format!("cat <<'EOF'\n{lines}\nEOF\nread -t 5 start\nread -t 5 flush\ncat <<'EOF'\n{result}\nEOF\nwhile read more; do :; done\nexit 0");
        let program = script(&dir, &body);
        let mind = with_run();
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&mind), &SystemClock, &FakeLender::new(Answer::Grant), &Spawn(program), TEST_CAP_S, &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::Ended { cause: Cause::FlushRequested, code: Some(0), requests: 2, prefix: 300_000 });
        assert_eq!(String::from_utf8(out).unwrap(), "request 1 prefix 10\nrequest 2 prefix 300000\nsettled: billed 7 micro-USD\n");
    }

    #[test]
    fn a_failure_result_before_any_request_closes_stdin_and_ends_the_run_promptly_through_the_real_child() {
        let dir = tempfile::tempdir().unwrap();
        let result = result_line();
        // An authentication or API failure: the error result is the first thing it prints, then it
        // waits for its input to end, as `claude -p` over stream-json does, and exits 1.
        let program = script(&dir, &format!("read -t 5 start\ncat <<'EOF'\n{result}\nEOF\nwhile read more; do :; done\nexit 1"));
        let started = std::time::Instant::now();
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&with_run()), &SystemClock, &FakeLender::new(Answer::Grant), &Spawn(program), TEST_CAP_S, &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::Ended { cause: Cause::Own, code: Some(1), requests: 0, prefix: 0 });
        assert_eq!(outcome.exit_status(), 1);
        assert!(!String::from_utf8_lossy(&out).contains("request "), "no request was heard");
        assert!(started.elapsed() < StdDuration::from_secs(20), "it ended promptly");
    }

    /// The Yggdrasil probe shape: the child prints its result and exits, and a background
    /// process it started still holds its stdout, so the stream never ends. The run is over
    /// when the child is, and the actuator signals nothing: the child is gone.
    #[test]
    fn a_child_that_exits_while_a_descendant_holds_its_stdout_ends_the_run_without_a_signal() {
        for flushed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let pids = dir.path().join("holder.pid");
            let (result, ceiling) = (result_line(), at("m1", CEILING_TOKENS));
            let before = if flushed { format!("cat <<'EOF'\n{ceiling}\nEOF\nread -t 5 start\nread -t 5 flush\n") } else { "read -t 5 start\n".to_string() };
            let program = script(&dir, &format!("{before}cat <<'EOF'\n{result}\nEOF\nsleep 30 &\necho $! > {}\nexit 0", pids.display()));
            let started = std::time::Instant::now();
            let mut out = Vec::new();
            let outcome = carry(&Seen::of(&with_run()), &SystemClock, &FakeLender::new(Answer::Grant), &Patient(program), TEST_CAP_S, &mut out).unwrap();
            let holder = std::fs::read_to_string(&pids).unwrap().trim().to_string();
            let cause = if flushed { Cause::FlushRequested } else { Cause::Own };
            let (requests, prefix) = if flushed { (1, CEILING_TOKENS) } else { (0, 0) };
            assert_eq!(outcome, SelfRunOutcome::Ended { cause, code: Some(0), requests, prefix }, "flushed: {flushed}");
            assert!(alive(&holder), "the actuator signals no one after the child is gone");
            assert!(started.elapsed() < StdDuration::from_secs(20), "it ended on the child's exit: {:?}", started.elapsed());
            Command::new("kill").arg(&holder).status().unwrap();
        }
    }

    #[test]
    fn each_signal_reaches_a_child_that_ignores_the_weaker_ones_and_one_for_a_gone_child_is_not_delivered() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "trap '' INT TERM\necho ready\nwhile :; do sleep 0.05; done");
        let mut child = start(&program, &[]);
        assert_eq!(child.next(StdDuration::from_secs(20)), Read::Line("ready".into()));
        assert!(child.stop(Stop::Interrupt) && child.stop(Stop::Terminate), "delivered, and ignored");
        assert_eq!(child.next(StdDuration::from_millis(300)), Read::Timeout, "still running");
        assert!(child.stop(Stop::Kill));
        assert_eq!(drain(&mut *child), (Vec::new(), None), "SIGKILL leaves no code");

        let quick = script(&dir, "exit 0");
        let mut gone = start(&quick, &[]);
        assert_eq!(drain(&mut *gone), (Vec::new(), Some(0)));
        assert!(!gone.stop(Stop::Interrupt), "no such process");
    }

    #[test]
    fn a_signal_reaches_the_whole_process_group_the_child_started() {
        // Not SIGINT: a background job of a non-interactive shell ignores it, which is the shell's rule, not ours.
        for how in [Stop::Terminate, Stop::Kill] {
            let dir = tempfile::tempdir().unwrap();
            // The child starts a descendant of its own and waits for it.
            let program = script(&dir, "sleep 300 &\necho $!\nwait");
            let mut child = start(&program, &[]);
            let Read::Line(holder) = child.next(StdDuration::from_secs(20)) else { panic!("no pid") };
            assert!(alive(&holder));
            assert!(child.stop(how), "{how:?} delivered to the group");
            drain(&mut *child);
            let mut waited = 0;
            while alive(&holder) && waited < 100 {
                std::thread::sleep(StdDuration::from_millis(50));
                waited += 1;
            }
            assert!(!alive(&holder), "{how:?} reached the descendant too");
        }
    }
}

mod lease;
