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
use cultnet_rs::Selection;
use huginn_mind::PipelinePageItems;
use crate::lease_client::token_holding;
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

/// The replay echo of a user line the CLI took into a request.
fn echo() -> String {
    json!({"type": "user", "isReplay": true, "uuid": "00000000-0000-4000-8000-000000000001", "message": {"role": "user", "content": "a line"}}).to_string()
}

/// A turn that ended well, with an answer.
fn ok_result() -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "terminal_reason": "completed", "result": "The answer."}).to_string()
}

fn init_line() -> String {
    PROBE.lines().next().unwrap().to_string()
}

fn self_run(label: &str, budget: &str) -> PipelineDocument {
    let PipelineDocument::Run(mut document) = run(label, RunTurn::SelfRun, RunOperator::Mind, &[]) else { unreachable!() };
    document.budget_usd = budget.into();
    PipelineDocument::Run(document)
}

use Step::{After, Eof, Exit, Idle, Line as Out};

/// The mind as the actuator sees it, over a `TestMind`: another instance's name,
/// admissions counted, and optionally refused as a mind that cannot be reached would.
struct Seen<'a> {
    inner: &'a TestMind,
    instance: Slug,
    doubled: bool,
    admit_fails: bool,
    /// The next this many admissions fail as an unreachable mind would, then the mind answers again.
    fail_admits: Cell<usize>,
    /// A query for this key fails.
    query_fails_on: Option<String>,
    admits: Cell<usize>,
}

impl<'a> Seen<'a> {
    fn of(inner: &'a TestMind) -> Self {
        Self { inner, instance: inner.instance().clone(), doubled: false, admit_fails: false, fail_admits: Cell::new(0), query_fails_on: None, admits: Cell::new(0) }
    }
}

impl MindPort for Seen<'_> {
    fn instance(&self) -> &Slug {
        &self.instance
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        if let (Some(key), Some(keys)) = (&self.query_fails_on, &selection.keys)
            && keys.contains(key)
        {
            return Err(anyhow!("mind-down-canary"));
        }
        let mut page = self.inner.query(selection)?;
        if self.doubled
            && let PipelinePageItems::Headers(items) = &mut page.items
        {
            let again = items.clone();
            items.extend(again);
        }
        Ok(page)
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.admits.set(self.admits.get() + 1);
        if self.admit_fails || self.fail_admits.get() > 0 {
            self.fail_admits.set(self.fail_admits.get().saturating_sub(1));
            return Err(anyhow!("down"));
        }
        self.inner.admit(agent, session, documents)
    }
}

struct Ran {
    outcome: Result<SelfRunOutcome>,
    out: String,
    events: Vec<Event>,
    /// Every event, the lease's included.
    all: Vec<Event>,
    spawns: Vec<Vec<String>>,
    sessions: Vec<Option<String>>,
    admits: usize,
    asked: Vec<LeaseRequest>,
}

impl Ran {
    fn ended(&self) -> (Cause, Option<i32>, u64, u64) {
        match self.outcome.as_ref().unwrap() {
            SelfRunOutcome::Ended { cause, code, requests, prefix, .. } => (*cause, *code, *requests, *prefix),
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

    /// What the unit's stdout holds apart from the decisions the actuator journals.
    fn lines(&self) -> String {
        self.out.lines().filter(|line| !line.starts_with("decision ") && !line.starts_with("session ")).map(|line| format!("{line}\n")).collect()
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
    carry_one(seen, steps, (code, dies_on, refuses, hangs))
}

fn carry_one(seen: &Seen, steps: Vec<Step>, how: (Option<i32>, Option<Stop>, &[Stop], bool)) -> Ran {
    carry_kept(seen, steps, how, None)
}

/// The same, with `kept` the conversation the spawner reports as kept.
fn carry_kept(seen: &Seen, steps: Vec<Step>, (code, dies_on, refuses, hangs): (Option<i32>, Option<Stop>, &[Stop], bool), kept: Option<Kept>) -> Ran {
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let fake = Fake::with(Some(Scripted { steps: steps.into(), log: log.clone(), clock: clock.clone(), code, dies_on, refuses: refuses.to_vec(), hangs, exited: false, eof: false, asked: 0 }), false);
    *fake.kept.borrow_mut() = kept;
    let lender = FakeLender::logging(log.clone(), Answer::Grant);
    let mut out = Vec::new();
    let outcome = carry(seen, &*clock, &lender, &fake, TEST_CAP_S, &mut out);
    let all = log.borrow().clone();
    // The child's own events; the lease's are in `all`, in order with them.
    let events = all.iter().filter(|event| !matches!(event, Event::Lent | Event::Spawned | Event::Abandoned | Event::Kept(_))).cloned().collect();
    let spawns = fake.spawns.borrow().clone();
    let sessions = fake.sessions.borrow().clone();
    Ran { outcome, out: String::from_utf8(out).unwrap(), events, all, spawns, sessions, admits: seen.admits.get(), asked: lender.asked.borrow().clone() }
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
    ran.written().iter().filter(|text| text.contains(FLUSH_MESSAGE)).count()
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
    let refusing = Fake::with(None, true);
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
    let never = Fake::with(None, false);
    assert!(carry(&Down, &*clock, &FakeLender::new(Answer::Grant), &never, TEST_CAP_S, &mut Vec::new()).is_err());
    assert!(never.spawns.borrow().is_empty());
}

// ---- what is started ------------------------------------------------------

#[test]
fn claude_args_is_exactly_the_fixed_list_for_a_session() {
    let session = Uuid::parse_str(SESSION_OF_RUN_A).unwrap();
    let args = claude_args(session, None);
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
            "--max-turns",
            "500",
            "--permission-mode",
            "bypassPermissions",
            "--permission-prompts",
            "none",
            "--setting-sources",
            "user",
            "--replay-user-messages",
            "--disallowedTools",
            "ScheduleWakeup,CronCreate",
        ]
    );
    // Those belong to the run measure or to the unit's managed settings.
    for forbidden in ["--continue", "--model", "--dangerously-skip-permissions", "--system-prompt", "--max-budget"] {
        assert!(!args.iter().any(|arg| arg.starts_with(forbidden) || arg.contains("dangerously")), "{forbidden}");
    }
    // Print mode starts in Manual without a mode, and prompts none then denies.
    let mode = args.iter().position(|arg| arg == "--permission-mode").expect("mode flag");
    assert_eq!(args[mode + 1], "bypassPermissions");
    assert_eq!(MAX_TURNS, 500);
}

#[test]
fn the_session_id_is_uuid_v5_of_the_run_id_and_differs_per_run() {
    assert_eq!(session_id(&foreign_run("eureka:run:mind-20261022T120000000Z")).to_string(), SESSION_OF_MIND_RUN);
    assert_eq!(session_id(&run_ref("selfrun-a")).to_string(), SESSION_OF_RUN_A);
    assert_eq!(session_id(&run_ref("selfrun-b")).to_string(), SESSION_OF_RUN_B);
}

#[test]
fn the_child_is_started_for_her_run_with_the_fixed_list_whatever_the_run_record_says() {
    // The run's budget field is a dead record; nothing in what is started depends on it.
    for budget in ["0", "0.5", "10", "100"] {
        let mind = TestMind::seeded();
        mind.committed(vec![self_run("selfrun-b", budget)]);
        let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
        assert_eq!(ran.spawns.len(), 1, "{budget}");
        assert_eq!(ran.spawns[0], claude_args(Uuid::parse_str(SESSION_OF_RUN_B).unwrap(), None), "{budget}");
    }
}

#[test]
fn the_child_is_given_its_runs_label_as_huginn_session_fresh_resumed_and_in_the_next_run_only() {
    // Fresh: the bare label, the spelling the run's opening receipt carries, not the id and not the derived UUID.
    let a = with_run();
    let fresh = carry_with(&Seen::of(&a), vec![Out(result_line())], Some(0), false);
    assert_eq!(fresh.sessions, [Some("selfrun-a".to_string())]);
    // Resumed: the same child, the new run's label.
    let resumed = carry_kept(&Seen::of(&with_run()), vec![Out(result_line())], (Some(0), None, &[], false), Some(resume::kept_for(1800, true)));
    assert_eq!(resumed.sessions, [Some("selfrun-a".to_string())]);
    // The rotation's new run sets its own; nothing of the previous run is carried.
    let b = TestMind::seeded();
    b.committed(vec![self_run("selfrun-b", "5")]);
    let next = carry_kept(&Seen::of(&b), vec![Out(result_line())], (Some(0), None, &[], false), Some(resume::kept_for(1800, true)));
    assert_eq!(next.sessions, [Some("selfrun-b".to_string())]);
    for ran in [&fresh, &resumed, &next] {
        assert!(ran.sessions.iter().flatten().all(|label| !label.contains(':') && label.parse::<Uuid>().is_err()), "{:?}", ran.sessions);
    }
    assert!(!next.sessions.contains(&Some("selfrun-a".to_string())));
}

#[test]
fn the_start_message_is_the_first_thing_written_and_names_the_skill_the_instance_and_the_run() {
    let ran = go(vec![Out(result_line())]);
    assert!(matches!(&ran.events[0], Event::Wrote(_)), "the start precedes the first read: {:?}", ran.events[0]);
    let written = ran.written();
    for needle in ["Plan usage: five_hour 42.3% allowed, resets at 2026-10-29T20:50:00Z, read at 2026-10-29T20:33Z", "about 90% of the weekly allowance", "nothing enforces it"] {
        assert!(written[0].contains(needle), "the lease grant plan windows reach the child: {needle}: {}", written[0]);
    }
    for needle in ["Use the eureka skill.", "yggdrasil", "yggdrasil:run:selfrun-a", AGENT] {
        assert!(written[0].contains(needle), "{needle}: {}", written[0]);
    }
    assert_eq!(written.len(), 1, "no flush without the ceiling");
}

#[test]
fn the_start_message_opens_with_the_skill_the_seed_ships() {
    let message = start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[plan_window()], 1_793_306_100_000);
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

#[test]
fn a_result_events_text_reaches_the_journal_with_its_subtype_and_error_flag() {
    let line = json!({"type": "result", "subtype": "success", "is_error": false, "result": "all done"}).to_string();
    let ran = go(vec![Out(line)]);
    assert!(ran.out.contains("result subtype=success is_error=false: all done\n"), "{}", ran.out);
}

/// What the journal holds for one stream line.
fn journal_of(line: &str) -> String {
    let mut out = Vec::new();
    write_result(line, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

/// The result envelopes of the real two-turn background-agent capture.
fn bg_agent_results() -> Vec<String> {
    include_str!("claude-2.1.312-live-session-bg-agent.jsonl")
        .lines()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok()?.get("line")?.as_str().map(str::to_string))
        .filter(|line| serde_json::from_str::<Value>(line).is_ok_and(|event| event["type"] == "result"))
        .collect()
}

fn usage_lines(journal: &str) -> Vec<&str> {
    journal.lines().filter(|line| line.starts_with("usage turn")).collect()
}

#[test]
fn a_result_writes_its_per_turn_usage_and_not_the_sessions_total() {
    let results = bg_agent_results();
    assert_eq!(results.len(), 2);
    let lines: Vec<String> = results.iter().map(|line| journal_of(line)).collect();
    // The capture's modelUsage runs 20/10 then 40/20; the turn's own usage is 20/10 both times.
    assert_eq!(usage_lines(&lines[0]), ["usage turn 0: claude-opus-5-5 input 20 cache_read 0 cache_creation 0 output 10 subagents_spawned_total 1"]);
    assert_eq!(usage_lines(&lines[1]), ["usage turn 1: claude-opus-5-5 input 20 cache_read 0 cache_creation 0 output 10 subagents_spawned_total 1"]);
}

#[test]
fn each_model_in_the_turns_usage_is_its_own_segment() {
    let line = json!({
        "type": "result", "result_index": 3,
        "model_usage_since_last_result": {
            "claude-haiku-4-5": {"inputTokens": 3, "cacheReadInputTokens": 4, "cacheCreationInputTokens": 5, "outputTokens": 6},
            "claude-opus-5-5": {"inputTokens": 7, "cacheReadInputTokens": 8, "cacheCreationInputTokens": 9, "outputTokens": 10}
        }
    })
    .to_string();
    assert_eq!(
        usage_lines(&journal_of(&line)),
        ["usage turn 3: claude-haiku-4-5 input 3 cache_read 4 cache_creation 5 output 6; claude-opus-5-5 input 7 cache_read 8 cache_creation 9 output 10"]
    );
}

#[test]
fn an_unreported_usage_is_said_never_zero() {
    let whole = json!({"inputTokens": 1, "cacheReadInputTokens": 2, "cacheCreationInputTokens": 3, "outputTokens": 4});
    let mut cases = vec![json!({"type": "result", "result_index": 2}), json!({"type": "result", "result_index": 2, "model_usage_since_last_result": {}})];
    for missing in ["inputTokens", "cacheReadInputTokens", "cacheCreationInputTokens", "outputTokens"] {
        let mut entry = whole.clone();
        entry.as_object_mut().unwrap().remove(missing);
        // The whole entry is unknown, even beside a model that reported everything.
        cases.push(json!({"type": "result", "result_index": 2, "model_usage_since_last_result": {"a": whole, "b": entry}}));
    }
    for case in cases {
        let journal = journal_of(&case.to_string());
        assert_eq!(usage_lines(&journal), ["usage turn 2: not reported"], "{case}");
    }
}

#[test]
fn the_result_line_is_unchanged_and_first_and_the_usage_line_carries_none_of_its_text() {
    let line = json!({
        "type": "result", "subtype": "success", "is_error": false, "result": "all done", "result_index": 0,
        "model_usage_since_last_result": {"m": {"inputTokens": 1, "cacheReadInputTokens": 2, "cacheCreationInputTokens": 3, "outputTokens": 4}}
    })
    .to_string();
    let journal = journal_of(&line);
    let lines: Vec<&str> = journal.lines().collect();
    assert_eq!(lines, ["result subtype=success is_error=false: all done", "usage turn 0: m input 1 cache_read 2 cache_creation 3 output 4"]);
    // A line that is not a result writes nothing.
    assert_eq!(journal_of(&json!({"type": "assistant"}).to_string()), "");
}

#[test]
fn a_long_result_reaches_the_journal_whole() {
    let text = format!("a{}", "é".repeat(5000));
    let line = json!({"type": "result", "result": text}).to_string();
    let ran = go(vec![Out(line)]);
    let written = ran.out.lines().find(|l| l.starts_with("result")).unwrap();
    assert_eq!(written, format!("result: {text}"));
    assert!(!ran.out.contains("[truncated]"), "{}", ran.out);
}

// ---- what is read -----------------------------------------------------------

#[test]
fn read_usage_sums_input_cache_read_and_cache_write_and_reads_absent_fields_as_zero() {
    assert_eq!(read_usage(&request("m1", 1, 20, 300)), Usage::Request { id: "m1".into(), prefix: 321, stops: vec![] });
    // Each term alone, so dropping or doubling any one is seen.
    assert_eq!(read_usage(&request("m", 7, 0, 0)), Usage::Request { id: "m".into(), prefix: 7, stops: vec![] });
    assert_eq!(read_usage(&request("m", 0, 7, 0)), Usage::Request { id: "m".into(), prefix: 7, stops: vec![] });
    assert_eq!(read_usage(&request("m", 0, 0, 7)), Usage::Request { id: "m".into(), prefix: 7, stops: vec![] });
    for absent in ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"] {
        let mut event: Value = serde_json::from_str(&request("m", 1, 20, 300)).unwrap();
        event["message"]["usage"].as_object_mut().unwrap().remove(absent);
        let want = 321 - [("input_tokens", 1), ("cache_read_input_tokens", 20), ("cache_creation_input_tokens", 300)].iter().find(|(name, _)| *name == absent).unwrap().1;
        assert_eq!(read_usage(&event.to_string()), Usage::Request { id: "m".into(), prefix: want, stops: vec![] }, "{absent} absent");
    }
    let mut no_usage: Value = serde_json::from_str(&request("m", 1, 1, 1)).unwrap();
    no_usage["message"].as_object_mut().unwrap().remove("usage");
    assert_eq!(read_usage(&no_usage.to_string()), Usage::Request { id: "m".into(), prefix: 0, stops: vec![] });
}

#[test]
fn read_usage_classifies_the_owners_real_records() {
    let sums = [84210, 100751, 117174, 118336, 126746, 127250, 129853];
    let mut seen = Vec::new();
    for line in RECORDS.lines() {
        let Usage::Request { id, prefix, .. } = read_usage(line) else { panic!("an assistant record of the main thread is a request: {line}") };
        if !seen.iter().any(|(known, _)| *known == id) {
            seen.push((id, prefix));
        }
    }
    assert_eq!(seen.iter().map(|(_, prefix)| *prefix).collect::<Vec<_>>(), sums);
    assert_eq!(read_usage(&result_line()), Usage::Result { ok: false, blocked: false });
    assert_eq!(read_usage(&ok_result()), Usage::Result { ok: true, blocked: false });
    let blocked = json!({"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "blocking_limit", "result": "Prompt is too long"}).to_string();
    assert_eq!(read_usage(&blocked), Usage::Result { ok: false, blocked: true });
    // An echo is a main-thread user line the CLI replayed, with string content; nothing else is.
    assert_eq!(read_usage(&echo()), Usage::Echo);
    let tool_result = json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "x"}]}}).to_string();
    assert_eq!(read_usage(&tool_result), Usage::Other, "a tool result is not one of our lines");
    let not_replayed = json!({"type": "user", "isReplay": false, "message": {"role": "user", "content": "a line"}}).to_string();
    assert_eq!(read_usage(&not_replayed), Usage::Other);
    let subagent_echo = json!({"type": "user", "isReplay": true, "parent_tool_use_id": "toolu_1", "message": {"role": "user", "content": "a line"}}).to_string();
    assert_eq!(read_usage(&subagent_echo), Usage::Other, "a subagent's line is not ours");
    assert_eq!(read_usage(&init_line()), Usage::TurnBegan);
    let replayed = json!({"type": "user", "isReplay": true, "message": {"role": "user", "content": "<task-notification>
<task-id>t</task-id>
</task-notification>"}}).to_string();
    assert_eq!(read_usage(&replayed), Usage::NotificationTaken { id: "t".into() }, "the CLI's replay of a notification is never our echo");
    assert_eq!(read_usage(&subagent("m", 5)), Usage::Other, "a subagent's request is not the main thread's");
    let mut no_id: Value = serde_json::from_str(&at("m", 5)).unwrap();
    no_id["message"].as_object_mut().unwrap().remove("id");
    assert_eq!(read_usage(&no_id.to_string()), Usage::Other);
    let mut null_parent: Value = serde_json::from_str(&at("m", 5)).unwrap();
    null_parent["parent_tool_use_id"] = Value::Null;
    assert_eq!(read_usage(&null_parent.to_string()), Usage::Request { id: "m".into(), prefix: 5, stops: vec![] });
    let mut no_parent: Value = serde_json::from_str(&at("m", 5)).unwrap();
    no_parent.as_object_mut().unwrap().remove("parent_tool_use_id");
    assert_eq!(read_usage(&no_parent.to_string()), Usage::Request { id: "m".into(), prefix: 5, stops: vec![] });
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
        ran.lines(),
        "request 1 prefix 84210
request 2 prefix 100751
request 3 prefix 117174
request 4 prefix 118336
request 5 prefix 126746
request 6 prefix 127250
request 7 prefix 129853
"
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

    let at_it = go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS - 1)), Out(at("m2", CEILING_TOKENS)), Out(at("m3", CEILING_TOKENS + 10_000))]);
    assert_eq!(flushes(&at_it), 1, "{:?}", at_it.written());
    assert_eq!(at_it.written().len(), 2, "the start and the flush");
    assert_eq!(at_it.ended().0, Cause::FlushRequested);
    assert_eq!(CEILING_TOKENS, 300_000);
}

#[test]
fn a_closed_input_is_never_asked_for_a_flush_whatever_the_prefix_grows_to() {
    // An error before any answer closes the input; a request that comes after it asks nothing.
    let ran = go(vec![Out(echo()), Out(result_line()), Out(at("m2", CEILING_TOKENS + 1))]);
    assert_eq!(ran.count(&Event::InputClosed), 1);
    assert_eq!(flushes(&ran), 0);
    assert_eq!(ran.ended().0, Cause::Own);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn stdin_closes_once_at_the_result_after_the_flushs_echo_folded_or_queued_and_nothing_is_written_after() {
    let writes_before_close = |ran: &Ran| {
        assert_eq!(ran.count(&Event::InputClosed), 1, "{:?}", ran.events);
        let closed_at = ran.events.iter().position(|event| *event == Event::InputClosed).unwrap();
        assert_eq!(ran.events[..closed_at].iter().filter(|event| matches!(event, Event::Wrote(_))).count(), 2, "start and flush before the close");
        assert!(ran.events[closed_at..].iter().all(|event| !matches!(event, Event::Wrote(_))));
        assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0, "never a grace interrupt");
    };
    // Folded: the flush lands in the running turn; its one result answers it.
    writes_before_close(&go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(echo()), Out(at("m2", 11)), Out(ok_result())]));
    // Queued: the first result ends the turn it was queued behind; the second is the flush's own.
    writes_before_close(&go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(ok_result()), Out(echo()), Out(at("m2", 11)), Out(ok_result())]));
    // The first result of a queued flush closes nothing: the flush is still to have its turn.
    let one = go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(ok_result())]);
    assert_eq!(one.count(&Event::InputClosed), 0, "{:?}", one.events);

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
    // The inbox poll's 30 until the grace is near, the one second left, then the escalation's own wait, then the drain after the child's exit.
    assert_eq!(ran.asked(), [30, 30, 30, 1, 29, 2], "{:?}", ran.asked());
}

#[test]
fn the_headroom_counts_the_requests_after_the_first_result_that_follows_the_flush_and_one_short_of_it_does_not_interrupt() {
    let hard = CEILING_TOKENS + FLUSH_HEADROOM_TOKENS;
    let short = go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(ok_result()), Out(echo()), Out(at("m2", hard - 1))]);
    assert_eq!(short.count(&Event::Stopped(Stop::Interrupt)), 0);
    assert_eq!(short.ended().0, Cause::FlushRequested);

    let at_hard = carry_with(&Seen::of(&with_run()), vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(ok_result()), Out(echo()), Out(at("m2", hard)), Out(at("m3", hard + 1))], Some(130), true);
    assert_eq!(at_hard.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(at_hard.ended().0, Cause::Signalled(Stop::Interrupt));
    assert_eq!(FLUSH_HEADROOM_TOKENS, 60_000);

    // Before the flush's echo the running turn's own requests are not the flush's turn: however far they grow, no interrupt.
    let same_turn = go(vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(at("m2", hard)), Out(at("m3", hard + 500_000))]);
    assert_eq!(same_turn.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn a_jump_past_the_ceiling_plus_the_headroom_writes_the_flush_and_the_running_turn_cannot_use_up_the_flushs_turn() {
    let jump = carry_with(
        &Seen::of(&with_run()),
        vec![Out(echo()), Out(at("m1", 290_000)), Out(at("m2", 400_000)), Out(at("m3", 470_000)), Out(ok_result()), Out(echo()), Out(at("m4", 459_999)), Out(at("m5", 460_000)), Out(at("m6", 470_000))],
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
    // Reads: the start echo, m1, m2 (the jump, which writes the flush), then m3 (the running turn's 470k, past the headroom: no interrupt).
    assert_eq!(kinds[..6], ["read", "read", "read", "flush", "read", "read"], "{kinds:?}");
    // The headroom is measured from the prefix the flush was asked at (400k) and counts the requests after the
    // flush's echo only: 459_999 does not interrupt; 460_000 does, once, and m6 is never read.
    assert_eq!(jump.count(&Event::Stopped(Stop::Interrupt)), 1, "{kinds:?}");
    assert_eq!(jump.lines(), "request 1 prefix 290000
request 2 prefix 400000
request 3 prefix 470000
result subtype=success is_error=false: The answer.
request 4 prefix 459999
request 5 prefix 460000
");
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
    let before: Vec<Step> = vec![Out(echo()), Out(at("m1", 10)), Idle(500), Out(at("m2", 20)), Out(at("m3", 30)), Out(at("m4", 40)), Out(at("m5", 50)), Out(at("m6", CEILING_TOKENS))];
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
    assert_eq!(read_usage(&line), Usage::Request { id: "m".into(), prefix: u64::MAX, stops: vec![] });
    assert_eq!(read_usage(&request("m", u64::MAX, u64::MAX, u64::MAX)), Usage::Request { id: "m".into(), prefix: u64::MAX, stops: vec![] });
    assert_eq!(read_usage(&request("m", 1, u64::MAX, 1)), Usage::Request { id: "m".into(), prefix: u64::MAX, stops: vec![] });
    // Through the actuator: the flush is asked for, and the headroom beyond a saturated prefix does not overflow
    // either, and the pass that wrote the flush does not interrupt.
    let ran = go(vec![Out(echo()), Out(request("m1", u64::MAX, 7, 0)), Out(request("m2", 0, 0, 0))]);
    assert_eq!(flushes(&ran), 1);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn an_error_result_before_any_answer_closes_the_input_at_once_and_the_run_ends_with_a_typed_outcome() {
    // The shape of an API or authentication failure: an error result before any answer. A flush would fail the same way.
    let ran = carry_with(&Seen::of(&with_run()), vec![Out(echo()), Out(result_line())], Some(1), false);
    assert_eq!(ran.count(&Event::InputClosed), 1);
    assert_eq!(flushes(&ran), 0);
    assert_eq!(ran.ended(), (Cause::Own, Some(1), 0, 0));
    assert_eq!(ran.outcome.as_ref().unwrap().exit_status(), 1);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 0);
}

#[test]
fn without_a_flush_nothing_is_ever_signalled_however_large_the_prefix_grows() {
    // An error before any answer closed the input: a request far over the ceiling asks for no flush and nothing is signalled.
    let ran = go(vec![Out(echo()), Out(result_line()), Out(at("m2", 900_000)), Idle(100_000)]);
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
    let steps: VecDeque<Step> = vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(at("m2", CEILING_TOKENS + 1)), Idle(0)].into();
    let child = Scripted { steps, log: log.clone(), clock: Rc::new(Fixed(Cell::new(at_start()))), code: Some(0), dies_on: None, refuses: vec![], hangs: false, exited: false, eof: false, asked: 0 };
    let fake = Fake::with(Some(child), false);
    let outcome = carry(&Seen::of(&mind), &Stepped(clock.clone()), &FakeLender::new(Answer::Grant), &fake, TEST_CAP_S, &mut Vec::new()).unwrap();
    let events = log.borrow().clone();
    let reads = events.iter().filter(|event| matches!(event, Event::Asked(_))).count() as u32;
    // One read for the lease's expiry, one before the first wait, then one per fact heard, and one for the closure:
    // never a second read inside a pass.
    assert_eq!(clock.1.get(), 3 + reads, "{events:?}");
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
    let ran = carry_scripted(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Step::After(FLUSH_GRACE_S + 5, ok_result())], Some(130), Some(Stop::Interrupt), &[], true);
    assert_eq!(ran.count(&Event::Stopped(Stop::Interrupt)), 1);
    assert_eq!(ran.count(&Event::InputClosed), 0, "the flush is queued behind the turn that just ended: its echo has not come");
    let stop_at = ran.events.iter().position(|event| matches!(event, Event::Stopped(_))).unwrap();
    let reads_before = ran.events[..stop_at].iter().filter(|event| matches!(event, Event::Asked(_))).count();
    assert_eq!(reads_before, 2, "the interrupt is in the pass that heard the result, not a poll away: {:?}", ran.events);
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
    assert_eq!(own.outcome.as_ref().unwrap(), &SelfRunOutcome::Ended { cause: Cause::Own, code: Some(0), requests: 1, prefix: 10, warm_until: None });
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


// ---- the real child -----------------------------------------------------------

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
    assert_eq!(ran.lines(), "request 1 prefix 10
result subtype=success is_error=true: Failed to authenticate: OAuth session expired and could not be refreshed
");
    let outcome = ran.outcome.as_ref().unwrap().to_string();
    assert!(!ran.out.contains(canary) && !outcome.contains(canary));
    assert!(!ran.written().iter().any(|text| text.contains(canary)), "nothing from the stream is written back");
}

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
            let config = FreshConfig::create(program.parent().unwrap(), None, None).unwrap();
            match spawn_program(program, args, token, (&unit_environment(), None), config) {
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
        let program = script(&dir, "trap 'echo interrupted; exit 130' INT\necho ready\nwhile :; do read -t 0.05 -r _; [ $? -le 128 ] && break; done");
        let mut child = start(&program, &[]);
        assert_eq!(child.next(StdDuration::from_secs(20)), Read::Line("ready".into()));
        child.stop(Stop::Interrupt);
        assert_eq!(drain(&mut *child), (vec!["interrupted".to_string()], Some(130)));
    }

    #[test]
    fn a_program_that_cannot_start_is_an_error_that_names_no_argument() {
        let canary = "canary-argument-6b0d";
        let scratch = tempfile::tempdir().unwrap();
        let config = FreshConfig::create(scratch.path(), None, None).unwrap();
        let error = spawn_program(Path::new("/nonexistent/claude"), &[canary.to_string()], &token_holding("tok-canary"), (&[], None), config).err().unwrap();
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
        fn spawn(&self, args: &[String], token: &TokenFd, _session: Option<&str>) -> Result<Box<dyn Child>> {
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
        fn spawn(&self, args: &[String], token: &TokenFd, _session: Option<&str>) -> Result<Box<dyn Child>> {
            Ok(Box::new(Bounded(start_with(&self.0, args, token), std::time::Instant::now())))
        }
    }

    #[test]
    fn a_whole_run_through_the_real_child_flushes_at_the_ceiling_and_ends_on_the_flushs_own_result() {
        let dir = tempfile::tempdir().unwrap();
        let lines = [at("m1", 10), at("m2", CEILING_TOKENS)].join("
");
        let (echo, result) = (echo(), ok_result());
        // Prints two requests, waits for the start and the flush on stdin, echoes both, then answers, as a stand-in for a Self that obeys.
        let body = format!("cat <<'EOF'
{lines}
EOF
read -t 5 start
read -t 5 flush
cat <<'EOF'
{echo}
{echo}
{result}
EOF
while read more; do :; done
exit 0");
        let program = script(&dir, &body);
        let mind = with_run();
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&mind), &SystemClock, &FakeLender::new(Answer::Grant), &Spawn(program), TEST_CAP_S, &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::Ended { cause: Cause::FlushRequested, code: Some(0), requests: 2, prefix: 300_000, warm_until: None });
        let printed: String = String::from_utf8(out).unwrap().lines().filter(|line| !line.starts_with("decision ") && !line.starts_with("session ")).map(|line| format!("{line}\n")).collect();
        assert_eq!(printed, "request 1 prefix 10
request 2 prefix 300000
result subtype=success is_error=false: The answer.
");
    }

    #[test]
    fn a_child_that_ends_after_a_failure_result_ends_the_run_promptly_through_the_real_child() {
        let dir = tempfile::tempdir().unwrap();
        let result = result_line();
        // An authentication or API failure: the error result is the first thing it prints, then it exits 1.
        let program = script(&dir, &format!("read -t 5 start\ncat <<'EOF'\n{result}\nEOF\nexit 1"));
        let started = std::time::Instant::now();
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&with_run()), &SystemClock, &FakeLender::new(Answer::Grant), &Spawn(program), TEST_CAP_S, &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::Ended { cause: Cause::Own, code: Some(1), requests: 0, prefix: 0, warm_until: None });
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
            assert_eq!(outcome, SelfRunOutcome::Ended { cause, code: Some(0), requests, prefix, warm_until: None }, "flushed: {flushed}");
            assert!(alive(&holder), "the actuator signals no one after the child is gone");
            assert!(started.elapsed() < StdDuration::from_secs(20), "it ended on the child's exit: {:?}", started.elapsed());
            Command::new("kill").arg(&holder).status().unwrap();
        }
    }

    #[test]
    fn each_signal_reaches_a_child_that_ignores_the_weaker_ones_and_one_for_a_gone_child_is_not_delivered() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(&dir, "trap '' INT TERM\necho ready\nwhile :; do read -t 0.05 -r _; [ $? -le 128 ] && break; done");
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
mod closure;
mod conversation;
mod reply;
mod resume;

// ---- the task in the start message ---------------------------------------

fn follow_up_saying(label: &str, item: &str) -> (PipelineDocument, PipelineRef) {
    let PipelineDocument::FollowUp(mut document) = follow_up(label, &spec_ref("x")) else { unreachable!() };
    document.item = item.into();
    (PipelineDocument::FollowUp(document), reference(K::FollowUp, &id("follow_up", label)))
}

fn claiming(claims: &[PipelineRef], documents: Vec<PipelineDocument>) -> TestMind {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x")]);
    if !documents.is_empty() {
        mind.committed(documents);
    }
    let PipelineDocument::Run(mut document) = self_run("selfrun-a", "5") else { unreachable!() };
    document.claims = claims.to_vec();
    mind.committed(vec![PipelineDocument::Run(document)]);
    mind
}

#[test]
fn the_start_message_carries_the_claimed_follow_ups_item_and_id_verbatim() {
    let (brief, claim) = follow_up_saying("brief-a", "Fix the thing.\nThen report \u{e9}.");
    let mind = claiming(std::slice::from_ref(&claim), vec![brief]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
    let first = &ran.written()[0];
    assert!(first.starts_with(&start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[plan_window()], at_start().timestamp_millis() as u64)), "{first}");
    for needle in ["Fix the thing.\nThen report \u{e9}.", claim.id.0.as_str(), "Use the eureka skill."] {
        assert!(first.contains(needle), "{needle}: {first}");
    }
    assert!(first.contains(&format!("(campaign `{}`, repo `{}`)", follow_up_campaign(), follow_up_repo())), "{first}");
}

fn follow_up_repo() -> String {
    let PipelineDocument::FollowUp(document) = follow_up("c", &spec_ref("x")) else { unreachable!() };
    document.repo.0
}

fn follow_up_campaign() -> String {
    let PipelineDocument::FollowUp(document) = follow_up("c", &spec_ref("x")) else { unreachable!() };
    document.campaign.0
}

#[test]
fn a_claim_that_is_no_brief_is_named_not_dropped() {
    // A claimed cut spec is the task: named with its id, to be read with huginn view.
    let mind = claiming(&[spec_ref("x")], vec![]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
    let first = &ran.written()[0];
    assert!(first.contains(&format!("claimed cut spec `{}`", spec_ref("x").id.0)) && first.contains("huginn view"), "{first}");
    assert!(!first.contains("unknown"), "{first}");
    // A claim the mind does not hold at all.
    let absent = reference(K::FollowUp, &id("follow_up", "absent"));
    let task = task_section(&mind, std::slice::from_ref(&absent)).unwrap();
    assert!(task.contains(&absent.id.0) && task.contains("does not hold"), "{task}");
    // A document of another kind is named by id and kind.
    let other = run_ref("selfrun-a");
    let task = task_section(&mind, std::slice::from_ref(&other)).unwrap();
    assert!(task.contains(&other.id.0) && task.contains("no brief"), "{task}");
}

#[test]
fn every_claim_of_sixty_four_full_items_is_named_by_id() {
    let big = "\u{e9}".repeat(500);
    let docs: Vec<_> = (0..64).map(|n| follow_up_saying(&format!("big-{n}"), &big)).collect();
    let claims: Vec<_> = docs.iter().map(|(_, claim)| claim.clone()).collect();
    let mind = claiming(&claims, docs.into_iter().map(|(document, _)| document).collect());
    let task = task_section(&mind, &claims).unwrap();
    for claim in &claims {
        assert!(task.contains(&claim.id.0), "{}", claim.id.0);
    }
    assert_eq!(task.matches(&big).count(), 64, "every item verbatim");
    assert!(!task.contains("truncated"));
    // Claim order is kept.
    assert!(task.find(&claims[0].id.0) < task.find(&claims[63].id.0));
}

#[test]
fn a_mind_query_failure_on_a_claim_fails_the_run_with_the_cause_and_starts_no_child() {
    let (brief, claim) = follow_up_saying("brief-a", "Fix the thing.");
    let mind = claiming(std::slice::from_ref(&claim), vec![brief]);
    let mut seen = Seen::of(&mind);
    seen.query_fails_on = Some(claim.id.0.clone());
    let ran = carry_with(&seen, vec![Out(result_line())], Some(0), false);
    let error = format!("{:#}", ran.outcome.as_ref().unwrap_err());
    assert!(error.contains("mind-down-canary") && error.contains(&claim.id.0), "{error}");
    assert!(ran.spawns.is_empty() && ran.events.is_empty() && !ran.all.iter().any(|event| matches!(event, Event::Lent)), "no lease, no child");
    assert!(!ran.out.contains("unknown"));
}

#[test]
fn a_run_without_claims_gets_the_identity_lines_alone() {
    let ran = go(vec![Out(result_line())]);
    assert_eq!(ran.written()[0], start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[plan_window()], at_start().timestamp_millis() as u64));
}

// ---- the live session -----------------------------------------------------

use eureka_pipeline::PipelineResolution;

fn live_mind() -> Rc<TestMind> {
    let mind = Rc::new(with_run());
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    mind
}

/// The mind gains these documents as her Persona turn's admission: the provenance that makes a self-run follow-up a brief.
fn persona_commit(mind: &TestMind, documents: Vec<PipelineDocument>) {
    let outcome = mind.admit(crate::persona::AGENT, "persona-a", documents).unwrap();
    assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
}

/// Her Persona admits these briefs, `seconds` into the run.
fn briefed(mind: &Rc<TestMind>, seconds: i64, documents: Vec<PipelineDocument>) -> Step {
    let mind = mind.clone();
    Step::Do(seconds, Rc::new(move || persona_commit(&mind, documents.clone())))
}

/// A brief from her Persona: a follow-up with owner `self-run`, sourced from the Persona turn's run.
fn brief(label: &str, item: &str) -> PipelineDocument {
    let PipelineDocument::FollowUp(mut document) = follow_up(label, &run_ref("persona")) else { unreachable!() };
    document.item = item.into();
    document.owner = inbox::OWNER.into();
    PipelineDocument::FollowUp(document)
}

fn brief_ref(label: &str) -> PipelineRef {
    reference(K::FollowUp, &id("follow_up", label))
}

/// The mind gains these documents, `seconds` into the run.
fn admits(mind: &Rc<TestMind>, seconds: i64, documents: Vec<PipelineDocument>) -> Step {
    let mind = mind.clone();
    Step::Do(seconds, Rc::new(move || mind.committed(documents.clone())))
}

/// Her Self resolves the brief herself, through her mind tool.
fn resolves(mind: &Rc<TestMind>, label: &str) -> Step {
    admits(mind, 0, vec![close(brief_ref(label), recorded())])
}

fn resolutions_of(mind: &TestMind, subject: &PipelineRef) -> Vec<PipelineResolution> {
    mind.ids(K::Resolution)
        .iter()
        .filter_map(|resolution| match mind.view(K::Resolution, resolution)?.document {
            PipelineDocument::Resolution(document) if document.subject == *subject => Some(document),
            _ => None,
        })
        .collect()
}

fn five_minute_request(id: &str, prefix: u64) -> String {
    let mut event: Value = serde_json::from_str(&at(id, prefix)).unwrap();
    event["message"]["usage"]["cache_creation"] = json!({"ephemeral_1h_input_tokens": 0, "ephemeral_5m_input_tokens": 100});
    event.to_string()
}

fn tells(ran: &Ran) -> Vec<String> {
    ran.written().into_iter().filter(|text| text.contains("A brief from your Persona")).collect()
}

fn mind_run(mind: &Rc<TestMind>, steps: Vec<Step>) -> Ran {
    carry_with(&Seen::of(mind), steps, Some(0), false)
}

#[test]
fn a_result_never_ends_the_session_the_child_ends_it_or_the_cache_does() {
    let ran = go(vec![Out(echo()), Out(at("m1", 10)), Out(ok_result())]);
    assert_eq!(ran.count(&Event::InputClosed), 0, "a result is the end of a turn: {:?}", ran.events);
    assert_eq!(ran.ended().0, Cause::Own);
}

#[test]
fn a_brief_derived_while_a_turn_runs_is_written_at_once_counted_at_its_echo_and_never_told_twice() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(at("m1", 10)),
        // The brief arrives while the first turn runs: the next poll writes it at once, mid-turn.
        briefed(&mind, 31, vec![brief("b1", "Survey the noise functions.")]),
        Out(echo()),
        // Later polls do not tell it again.
        Idle(31),
        Idle(31),
        Out(ok_result()),
    ];
    let ran = mind_run(&mind, steps);
    let told = tells(&ran);
    assert_eq!(told.len(), 1, "{:?}", ran.written());
    assert!(told[0].contains(&brief_ref("b1").id.0) && told[0].contains("Survey the noise functions."), "{}", told[0]);
    assert!(told[0].contains(&inbox::resolving_sentence(&brief_ref("b1").id.0)) && !told[0].contains(inbox::CLOSING_RULE), "she is reminded of the closing rule, not told it again: {}", told[0]);
    // The plan line is restated under every brief for the run's life, so it names the instant of the reading, never an age that stops being true.
    assert!(told[0].contains("read at 2026-10-29T20:33Z") && !told[0].contains(" ago"), "{}", told[0]);
    assert_eq!(ran.written().len(), 2, "the start message and one tell: {:?}", ran.written());
    assert!(ran.out.contains("decision Write { tells: true, switch: false, flush: None } prefix 10 ttl_s 0 idle_s working inbox true written 2 consumed 1\n"), "{}", ran.out);
    assert_eq!(ran.count(&Event::InputClosed), 0);
    // A result is not an answer: nothing closes the brief by its timing.
    assert!(resolutions_of(&mind, &brief_ref("b1")).is_empty());
}

#[test]
fn a_brief_derived_while_the_session_is_idle_is_written_and_the_session_is_busy_until_its_echo() {
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), briefed(&mind, 31, vec![brief("b2", "Wire the shader uniforms.")]), Out(echo()), Out(ok_result())];
    let ran = mind_run(&mind, steps);
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    assert!(ran.out.contains("idle_s 31 inbox true written 2 consumed 1\n"), "idle when it was written: {}", ran.out);
    assert_eq!(ran.count(&Event::InputClosed), 0);
    assert!(resolutions_of(&mind, &brief_ref("b2")).is_empty());
}

#[test]
fn the_start_message_carries_the_closing_rule_once_and_names_what_closes_a_change() {
    let text = start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[], 0);
    assert_eq!(text.matches(inbox::CLOSING_RULE).count(), 1, "{text}");
    for word in ["default branch", "Recorded", "Fixed", "Superseded", "Withdrawn", "merge-gate-own-soul", "An open PR, a pushed branch or a local commit is not done"] {
        assert!(text.contains(word), "{word}: {text}");
    }
}

#[test]
fn the_flush_keeps_a_brief_whose_change_has_not_landed_open() {
    for why in [Why::Ceiling, Why::Rotation, Why::Idle] {
        let text = flush_text(why);
        assert!(text.contains(FLUSH_MESSAGE), "{text}");
        assert_eq!(text.contains("context ceiling"), why == Why::Ceiling, "each flush names its own reason: {text}");
        assert!(text.contains("stays open") && text.contains("default branch") && text.contains("closing rule"), "{text}");
        assert!(!text.contains("Recorded, your answer"), "{text}");
    }
}

#[test]
fn the_closing_rule_covers_an_act_without_a_commit_and_names_the_commit_fixed_takes() {
    let rule = inbox::CLOSING_RULE;
    for word in ["no commit", "Recorded with what you did and where it can be seen", "merge or squash commit", "not the PR head"] {
        assert!(rule.contains(word), "{word}: {rule}");
    }
    let reminder = inbox::resolving_sentence("x");
    for word in ["Recorded only if it asked just for an answer", "an act with no commit", "a pushed branch or a local commit is not done"] {
        assert!(reminder.contains(word), "{word}: {reminder}");
    }
}

#[test]
fn no_source_of_the_actuator_tells_her_a_brief_closes_by_recorded_with_her_answer() {
    for source in [include_str!("../self_run.rs"), include_str!("inbox.rs")] {
        for old in ["Recorded with your answer", "Recorded, your answer", "Resolve each brief you answered"] {
            assert!(!source.contains(old), "{old}");
        }
    }
}

#[test]
fn a_brief_open_at_start_is_in_the_start_message_with_how_to_resolve_it_and_is_not_told_again() {
    let mind = live_mind();
    persona_commit(&mind, vec![brief("p1", "Still open.")]);
    let ran = mind_run(&mind, vec![Out(echo()), Idle(31), Idle(31)]);
    let start = &ran.written()[0];
    assert!(start.contains("Also open for you") && start.contains(&brief_ref("p1").id.0) && start.contains("Still open."), "{start}");
    assert!(start.contains(&inbox::resolving_sentence(&brief_ref("p1").id.0)), "{start}");
    assert_eq!(start.matches(inbox::CLOSING_RULE).count(), 1, "the rule is told once, beside the reminder: {start}");
    assert_eq!(ran.written().len(), 1, "never told again: {:?}", ran.written());
}

#[test]
fn a_follow_up_her_self_filed_for_herself_is_never_told() {
    let mind = live_mind();
    let PipelineDocument::FollowUp(mut hers) = follow_up("hers", &run_ref("selfrun-a")) else { unreachable!() };
    hers.owner = inbox::OWNER.into();
    // Naming the Persona turn's run as the source does not make it hers to be told: the receipt records who admitted it.
    let PipelineDocument::FollowUp(mut citing) = follow_up("citing", &run_ref("persona")) else { unreachable!() };
    citing.owner = inbox::OWNER.into();
    mind.committed(vec![PipelineDocument::FollowUp(hers), PipelineDocument::FollowUp(citing)]);
    let ran = mind_run(&mind, vec![Out(echo()), Idle(31)]);
    assert_eq!(ran.written().len(), 1, "{:?}", ran.written());
    assert!(!ran.written()[0].contains("Also open"), "{}", ran.written()[0]);
}

#[test]
fn an_idle_end_closes_no_brief_the_ones_she_left_open_stay_open_and_her_resolutions_stand() {
    let mind = live_mind();
    persona_commit(&mind, vec![brief("p1", "Left open."), brief("p2", "Answered by her.")]);
    let steps = vec![Out(echo()), Out(at("m1", 10)), resolves(&mind, "p2"), Out(ok_result()), Idle(3539), Idle(1), Out(echo()), Out(ok_result())];
    let ran = mind_run(&mind, steps);
    assert_eq!(ran.count(&Event::InputClosed), 1, "{:?}", ran.events);
    assert!(resolutions_of(&mind, &brief_ref("p1")).is_empty(), "the actuator resolved nothing she left open");
    assert_eq!(resolutions_of(&mind, &brief_ref("p2")).len(), 1, "her own resolution stands alone");
}

#[test]
fn a_rotation_a_ceiling_an_error_or_a_crash_leaves_the_told_briefs_open_for_the_successor() {
    let cap = i64::try_from(TEST_CAP_S).unwrap();
    let ends: Vec<(&str, Vec<Step>)> = vec![
        ("rotation", vec![Out(echo()), After(cap - 1400, at("m1", 10)), Out(echo()), Out(ok_result())]),
        ("ceiling", vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(echo()), Out(ok_result())]),
        ("error before an answer", vec![Out(echo()), Out(result_line())]),
        ("idle flush answered by an error", vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), Idle(3539), Idle(1), Out(echo()), Out(result_line())]),
        ("crash", vec![Out(echo()), Out(at("m1", 10)), Exit(Some(137))]),
    ];
    for (name, steps) in ends {
        let mind = live_mind();
        persona_commit(&mind, vec![brief("p1", "Left open.")]);
        let ran = mind_run(&mind, steps);
        assert!(ran.outcome.is_ok(), "{name}");
        assert!(resolutions_of(&mind, &brief_ref("p1")).is_empty(), "{name}: the brief stays open");
    }
}

#[test]
fn a_brief_admitted_after_the_flush_is_written_stays_in_force_and_untold() {
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), briefed(&mind, 31, vec![brief("late", "Too late.")]), Idle(31), Out(echo()), Out(ok_result())];
    let ran = mind_run(&mind, steps);
    assert!(tells(&ran).is_empty(), "{:?}", ran.written());
    assert_eq!(ran.written().len(), 2, "the start and the flush; no line after the flush");
    assert!(resolutions_of(&mind, &brief_ref("late")).is_empty());
}

#[test]
fn the_session_ends_where_the_cache_would_be_evicted_with_a_flush_and_a_waiting_brief_keeps_it_open() {
    // An hour's cache: warm at 3539 s, the flush at 3540, the input closed at the result after the flush's echo.
    let hour = go(vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), Idle(3539), Idle(1), Out(echo()), Out(ok_result()), Idle(1)]);
    assert_eq!(flushes(&hour), 1);
    let flush_at = hour.events.iter().position(|event| matches!(event, Event::Wrote(line) if line.contains(FLUSH_MESSAGE))).unwrap();
    let closed_at = hour.events.iter().position(|event| *event == Event::InputClosed).unwrap();
    assert!(flush_at < closed_at && hour.count(&Event::InputClosed) == 1, "{:?}", hour.events);
    assert!(hour.out.contains("decision Write { tells: false, switch: false, flush: Some(Idle) } prefix 10 ttl_s 3600 idle_s 3540 inbox false written 2 consumed 1\n"), "{}", hour.out);
    assert!(hour.written().iter().any(|text| text.contains("nothing is waiting and its cache would lapse") && text.contains("by the closing rule")), "{:?}", hour.written());
    // Five minutes' cache: the flush at 240 s.
    let short = go(vec![Out(echo()), Out(five_minute_request("m1", 10)), Out(ok_result()), Idle(239), Idle(1), Out(echo()), Out(ok_result()), Idle(1)]);
    assert_eq!(flushes(&short), 1);
    // Not a second early.
    let early = go(vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), Idle(3539)]);
    assert_eq!(flushes(&early), 0);
    // A brief waiting at the cache's end is told, not flushed: the session stays open.
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), briefed(&mind, 3539, vec![brief("b5", "One more.")]), Idle(1), Out(echo()), Out(ok_result()), Idle(1)];
    let ran = mind_run(&mind, steps);
    assert_eq!((flushes(&ran), tells(&ran).len(), ran.count(&Event::InputClosed)), (0, 1, 0), "{:?}
{}", ran.written(), ran.out);
}

#[test]
fn a_tell_that_is_not_yet_echoed_keeps_the_session_busy_so_the_cache_end_does_not_flush_over_it() {
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(ok_result()), briefed(&mind, 31, vec![brief("b6", "Pending.")]), Idle(3600)];
    let ran = mind_run(&mind, steps);
    assert_eq!((tells(&ran).len(), flushes(&ran)), (1, 0), "{:?}", ran.written());
}

#[test]
fn near_the_cap_the_session_is_asked_for_its_flush_idle_or_working_and_closes_at_the_result_after_its_echo() {
    let cap = i64::try_from(TEST_CAP_S).unwrap();
    // Idle at cap - 1700 s: the flush at cap - 1500, not a second before.
    let before = go(vec![Out(echo()), After(cap - 1700, at("m1", 10)), Out(ok_result()), Idle(199)]);
    assert_eq!(flushes(&before), 0);
    let idle = go(vec![Out(echo()), After(cap - 1700, at("m1", 10)), Out(ok_result()), Idle(199), Idle(1), Out(echo()), Out(ok_result())]);
    assert_eq!((flushes(&idle), idle.count(&Event::InputClosed)), (1, 1), "{:?}", idle.events);
    assert!(idle.written().iter().any(|text| text.contains("the unit's time is nearly up")), "{:?}", idle.written());
    // Mid-turn: the flush at cap - 1500, once.
    let working = go(vec![Out(echo()), After(cap - 1500, at("m1", 10))]);
    assert_eq!(flushes(&working), 1);
    let not_yet = go(vec![Out(echo()), After(cap - 1501, at("m1", 10))]);
    assert_eq!(flushes(&not_yet), 0);
}

#[test]
fn no_stdin_line_is_written_after_the_flush_and_nothing_but_the_writer_writes() {
    let mind = live_mind();
    persona_commit(&mind, vec![brief("b7", "Early.")]);
    let steps = vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), briefed(&mind, 31, vec![brief("b8", "After the flush.")]), Idle(31), Idle(31), Out(echo()), Out(at("m2", 11)), Out(ok_result())];
    let ran = mind_run(&mind, steps);
    let written = ran.written();
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(written[1].contains(FLUSH_MESSAGE), "{}", written[1]);
    assert_eq!(ran.spawns.len(), 1);
    assert!(ran.spawns[0].iter().all(|arg| arg != "--settings"), "no hooks on argv: {:?}", ran.spawns[0]);
}

/// The committed 2.1.312 streams, replayed through the real parser and the decision.
mod live_session {
    use crate::self_run::decide::{Action as Do, Event as Fact, Why};
    use super::*;

    fn flush(why: Why) -> Do {
        Do::Write { tells: false, switch: false, flush: Some(why) }
    }

    /// One recorded line as the loop would hear it: a label for the stream, the decision's action.
    fn replay(jsonl: &str) -> Vec<(String, Do)> {
        replay_state(jsonl).0
    }

    /// The same, with the state the decision ended in. A `want` entry (the probe asked its writer to send a line that waits) is a brief waiting; the flush is the decision's own.
    fn replay_state(jsonl: &str) -> (Vec<(String, Do)>, State) {
        let mut state = State::started(crate::testkit::at(0), 100_000);
        let mut ttl_1h = false;
        let mut trace = Vec::new();
        for entry in jsonl.lines() {
            let entry: Value = serde_json::from_str(entry).unwrap();
            let at = crate::testkit::at((entry["t"].as_f64().unwrap() * 1000.0) as i64);
            if let Some(text) = entry.get("want").and_then(Value::as_str) {
                let (label, fact) = if text.contains(FLUSH_MESSAGE) { ("flush-asked", Fact::Tick) } else { ("want", Fact::Inbox { pending: true, other: false, other_first: false }) };
                let (next, action) = decide(state, fact, at);
                state = next;
                trace.push((label.to_string(), action));
                continue;
            }
            let Some(line) = entry.get("line").and_then(Value::as_str) else { continue };
            if let Some(hour) = writes_hour_cache(line) {
                ttl_1h = hour;
            }
            let (label, fact) = match read_usage(line) {
                Usage::Request { id, prefix, stops } => (format!("request:{prefix}"), Fact::Request { id, prefix, stops }),
                Usage::Echo => ("echo".to_string(), Fact::Echo),
                Usage::TurnBegan => ("turn".to_string(), Fact::TurnBegan),
                Usage::BackgroundTasks { live, main, shell } => ("level".to_string(), Fact::BackgroundTasks { live, main, shell }),
                Usage::TaskBackgrounded { id, shell } => ("backgrounded".to_string(), Fact::TaskBackgrounded { id, shell }),
                Usage::TaskNotified { id, stopped } => ("notified".to_string(), Fact::TaskNotified { id, stopped }),
                Usage::NotificationTaken { id } => ("taken".to_string(), Fact::NotificationTaken { id }),
                Usage::Result { ok, blocked } => (format!("result:{}", if blocked { "blocked" } else if ok { "ok" } else { "error" }), Fact::Result { ok, blocked, ttl_1h }),
                Usage::Other | Usage::Unreadable => ("other".to_string(), Fact::Tick),
            };
            let (next, action) = decide(state, fact, at);
            state = next;
            trace.push((label, action));
        }
        (trace, state)
    }

    fn kinds(trace: &[(String, Do)]) -> Vec<&str> {
        trace.iter().map(|(label, _)| label.as_str()).collect()
    }

    /// The indexes at which the decision acted, and how.
    fn acted(trace: &[(String, Do)]) -> Vec<(usize, Do)> {
        trace.iter().enumerate().filter(|(_, (_, action))| *action != Do::None).map(|(at, (_, action))| (at, action.clone())).collect()
    }

    const FOLDED: &str = include_str!("claude-2.1.312-live-session-folded.jsonl");
    const QUEUED: [&str; 2] = [include_str!("claude-2.1.312-live-session-queued.jsonl"), include_str!("claude-2.1.312-live-session-queued-scrub.jsonl")];
    const THREE_QUEUED: [&str; 2] = [include_str!("claude-2.1.312-live-session-three-queued.jsonl"), include_str!("claude-2.1.312-live-session-three-queued-scrub.jsonl")];
    const SERIALIZED: [&str; 2] = [include_str!("claude-2.1.312-live-session-serialized.jsonl"), include_str!("claude-2.1.312-live-session-serialized-scrub.jsonl")];
    const CEILING_FOLDED: &str = include_str!("claude-2.1.312-live-session-ceiling-flush-folded.jsonl");
    const CEILING_QUEUED: [&str; 2] = [include_str!("claude-2.1.312-live-session-ceiling-flush-queued.jsonl"), include_str!("claude-2.1.312-live-session-ceiling-flush-queued-scrub.jsonl")];
    const API_ERROR: [&str; 2] = [include_str!("claude-2.1.312-live-session-api-error-first.jsonl"), include_str!("claude-2.1.312-live-session-api-error-first-scrub.jsonl")];
    const BLOCKING: [&str; 2] = [include_str!("claude-2.1.312-live-session-blocking-limit.jsonl"), include_str!("claude-2.1.312-live-session-blocking-limit-scrub.jsonl")];
    const COMPACTION_OFF: [&str; 2] = [include_str!("claude-2.1.312-live-session-compaction-off-360k.jsonl"), include_str!("claude-2.1.312-live-session-compaction-off-360k-scrub.jsonl")];
    const BG_IDLE: &str = include_str!("claude-2.1.312-live-session-bg-idle.jsonl");
    const BG_FOLD: &str = include_str!("claude-2.1.312-live-session-bg-fold.jsonl");
    const BG_OTHER: &str = include_str!("claude-2.1.312-live-session-bg-other.jsonl");
    const BG_RACE: &str = include_str!("claude-2.1.312-live-session-bg-race.jsonl");
    const BG_AGENT: &str = include_str!("claude-2.1.312-live-session-bg-agent.jsonl");
    const BG_LATE: &str = include_str!("claude-2.1.312-live-session-bg-late.jsonl");
    const BG_FGTO: &str = include_str!("claude-2.1.312-live-session-bg-fgto.jsonl");
    const BG_KILL: &str = include_str!("claude-2.1.312-live-session-bg-kill.jsonl");
    const BG_MONITOR: &str = include_str!("claude-2.1.312-live-session-bg-monitor.jsonl");
    const BG_KILLSHELL: &str = include_str!("claude-2.1.312-live-session-bg-killshell.jsonl");
    const BG_DEADLINE_RACE: &str = include_str!("claude-2.1.312-live-session-bg-deadline-race.jsonl");

    #[test]
    fn a_line_written_during_a_tool_call_is_folded_one_result_and_its_echo_before_the_request_that_carries_it() {
        let trace = replay(FOLDED);
        // The tool_result's own user line is no echo: it is a foreign owner's array content.
        assert_eq!(kinds(&trace), ["turn", "echo", "request:10", "other", "notified", "other", "echo", "request:10", "result:ok"]);
        assert!(acted(&trace).is_empty(), "{:?}", acted(&trace));
    }

    #[test]
    fn a_line_written_while_the_last_request_is_in_flight_is_queued_two_results_and_its_echo_after_the_first() {
        for stream in QUEUED {
            let trace = replay(stream);
            assert_eq!(kinds(&trace), ["turn", "echo", "request:10", "result:ok", "turn", "echo", "request:10", "result:ok"]);
            assert!(acted(&trace).is_empty(), "{:?}", acted(&trace));
        }
    }

    fn sent(jsonl: &str) -> usize {
        jsonl.lines().filter(|entry| serde_json::from_str::<Value>(entry).unwrap().get("sent").is_some()).count()
    }

    #[test]
    fn lines_written_behind_an_in_flight_request_come_back_as_one_echo_which_is_why_one_write_is_outstanding() {
        for stream in THREE_QUEUED {
            let trace = replay(stream);
            let echoes = kinds(&trace).iter().filter(|kind| **kind == "echo").count();
            assert_eq!((sent(stream), echoes), (3, 2), "three lines written, two echoes: counting echoes against lines written drifts for good");
        }
    }

    #[test]
    fn with_one_write_outstanding_every_write_is_echoed_once_and_the_flush_is_reached_and_answered() {
        for stream in SERIALIZED {
            let (trace, state) = replay_state(stream);
            assert_eq!(
                kinds(&trace),
                ["turn", "want", "want", "echo", "request:10", "result:ok", "turn", "echo", "request:300000", "result:ok", "flush-asked", "turn", "echo", "request:10", "result:ok"]
            );
            // Both briefs wait for the start message's echo and go as one line; the flush is written at the ceiling request; the result after its echo closes stdin.
            assert_eq!(acted(&trace), vec![(3, Do::Write { tells: true, switch: false, flush: None }), (8, flush(Why::Ceiling)), (14, Do::CloseStdin)]);
            assert_eq!((sent(stream), state.written, state.consumed), (3, 3, 3), "every line written has its own echo");
        }
    }

    #[test]
    fn the_flush_folded_into_a_running_turn_is_answered_at_its_one_result() {
        let trace = replay(CEILING_FOLDED);
        assert_eq!(kinds(&trace), ["turn", "echo", "request:300000", "other", "notified", "other", "echo", "request:10", "result:ok"]);
        assert_eq!(acted(&trace), vec![(2, flush(Why::Ceiling)), (8, Do::CloseStdin)], "never a grace interrupt");
    }

    #[test]
    fn the_flush_queued_behind_the_turn_that_reached_the_ceiling_is_answered_at_its_own_second_result() {
        for stream in CEILING_QUEUED {
            let trace = replay(stream);
            assert_eq!(kinds(&trace), ["turn", "echo", "request:300000", "result:ok", "turn", "echo", "request:10", "result:ok"]);
            // The first result ends the turn the flush was queued behind: it closes nothing.
            assert_eq!(acted(&trace), vec![(2, flush(Why::Ceiling)), (7, Do::CloseStdin)], "never a grace interrupt");
        }
    }

    #[test]
    fn an_api_error_before_any_answer_closes_the_session_at_once_with_no_flush() {
        for stream in API_ERROR {
            let trace = replay(stream);
            assert_eq!(kinds(&trace), ["turn", "echo", "other", "result:error"]);
            assert_eq!(acted(&trace), vec![(3, Do::CloseStdin)]);
        }
    }

    #[test]
    fn a_context_over_the_blocking_limit_is_a_blocked_result_that_closes_the_session() {
        for stream in BLOCKING {
            let trace = replay(stream);
            assert_eq!(kinds(&trace), ["turn", "echo", "request:990000", "result:ok", "turn", "echo", "other", "result:blocked"]);
            assert_eq!(acted(&trace), vec![(2, flush(Why::Ceiling)), (7, Do::CloseStdin)]);
        }
    }

    #[test]
    fn with_compaction_off_a_request_at_360k_is_sent_and_answered_not_compacted_or_blocked() {
        for stream in COMPACTION_OFF {
            let trace = replay(stream);
            assert_eq!(kinds(&trace), ["turn", "echo", "request:360000", "result:ok", "turn", "echo", "request:10", "result:ok"]);
            assert_eq!(acted(&trace), vec![(2, flush(Why::Ceiling)), (7, Do::CloseStdin)]);
        }
    }

    /// Whether the session is idle after each stream line, by the line's label.
    fn idleness(jsonl: &str) -> Vec<(String, bool)> {
        let lines: Vec<&str> = jsonl.lines().collect();
        (1..=lines.len())
            .filter_map(|n| {
                let entry: Value = serde_json::from_str(lines[n - 1]).unwrap();
                entry.get("line")?.as_str()?;
                let (trace, state) = replay_state(&lines[..n].join("\n"));
                Some((trace.last().unwrap().0.clone(), state.idle_s(crate::testkit::at(1_000_000)).is_some()))
            })
            .collect()
    }

    fn significant(trace: &[(String, Do)]) -> Vec<&str> {
        kinds(trace).into_iter().filter(|kind| *kind != "other").collect()
    }

    fn usages(jsonl: &str) -> Vec<Usage> {
        jsonl.lines().filter_map(|entry| serde_json::from_str::<Value>(entry).ok()?.get("line")?.as_str().map(read_usage)).collect()
    }

    #[test]
    fn read_usage_reads_the_level_the_backgrounded_lines_the_notifications_and_the_stops_the_captures_hold() {
        // A turn is begun by the init line, a task joins the level and is backgrounded, its notification comes by its event and the level
        // empties after it; the CLI's replay of that notification is taken, never our echo.
        let level = ["level", "backgrounded", "notified", "level"];
        assert_eq!(significant(&replay(BG_FOLD)), [vec!["turn", "echo", "request:10"], level.to_vec(), vec!["request:10", "taken", "request:10", "result:ok"]].concat());
        assert_eq!(significant(&replay(BG_IDLE)), [vec!["turn", "echo", "request:10"], level.to_vec(), vec!["request:10", "result:ok", "turn", "request:10", "result:ok"]].concat());
        assert_eq!(significant(&replay(BG_RACE)), [vec!["turn", "echo", "request:10"], level.to_vec(), vec!["request:10", "result:ok", "turn", "request:10", "result:ok", "turn", "echo", "request:10", "result:ok"]].concat());
        assert_eq!(significant(&replay(BG_OTHER)), [vec!["turn", "echo", "request:10"], level.to_vec(), vec!["request:10", "result:ok", "turn", "request:10", "echo", "request:10", "result:ok"]].concat());
        // A persistent monitor: notice turns the CLI begins while its task stays in the level, then the notification and the level's end.
        assert_eq!(significant(&replay(BG_MONITOR)).iter().filter(|kind| **kind == "level").count(), 2);
        let task = |usage: &Usage| match usage {
            Usage::TaskBackgrounded { id, .. } | Usage::TaskNotified { id, .. } | Usage::NotificationTaken { id } => Some(id.clone()),
            _ => None,
        };
        let fold = usages(BG_FOLD);
        let ids: Vec<String> = fold.iter().filter_map(task).collect();
        assert_eq!(ids.len(), 3, "backgrounded, notified, taken");
        assert!(ids.iter().all(|id| *id == ids[0]) && !ids[0].starts_with("toolu_"), "one task_id names the task at its three lines: {ids:?}");
        assert!(fold.contains(&Usage::BackgroundTasks { live: vec![ids[0].clone()], main: vec![ids[0].clone()], shell: vec![ids[0].clone()] }) && fold.contains(&Usage::BackgroundTasks { live: vec![], main: vec![], shell: vec![] }));
        // A task that went to the background at its timeout was announced foreground and is backgrounded by the update that moved it: the start is no reading.
        let fgto = usages(BG_FGTO);
        let moved: Vec<&Usage> = fgto.iter().filter(|usage| matches!(usage, Usage::TaskBackgrounded { .. })).collect();
        assert_eq!(moved.len(), 1, "{fgto:?}");
        assert!(fgto.iter().any(|usage| matches!(usage, Usage::TaskNotified { stopped: false, .. })));
        // A stop is a notification whose status is stopped, and the TaskStop call on the request line names the task.
        let kill = usages(BG_KILL);
        let Some(Usage::TaskBackgrounded { id: killed, shell: true }) = kill.iter().find(|usage| matches!(usage, Usage::TaskBackgrounded { .. })) else { panic!("{kill:?}") };
        assert!(kill.contains(&Usage::TaskNotified { id: killed.clone(), stopped: true }));
        assert!(kill.iter().any(|usage| matches!(usage, Usage::Request { stops, .. } if stops == std::slice::from_ref(killed))), "{kill:?}");
        assert_eq!(kill.iter().filter(|usage| matches!(usage, Usage::Request { stops, .. } if !stops.is_empty())).count(), 1);
        // The level's `main` leaves out a subagent's and an awaited task; a start under a parent task, or awaited, or in the foreground, is no reading.
        let changed = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "m", "task_type": "local_bash"}, {"task_id": "s", "parent_task_id": "p", "task_type": "local_agent"}, {"task_id": "w", "awaited": true}, {"task_id": "n", "parent_task_id": null, "awaited": false}]}).to_string();
        assert_eq!(read_usage(&changed), Usage::BackgroundTasks { live: vec!["m".into(), "s".into(), "w".into(), "n".into()], main: vec!["m".into(), "n".into()], shell: vec!["m".into()] });
        let started = |extra: Value| {
            let mut line = json!({"type": "system", "subtype": "task_started", "task_id": "t", "is_backgrounded": true});
            line.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            read_usage(&line.to_string())
        };
        assert_eq!(started(json!({})), Usage::TaskBackgrounded { id: "t".into(), shell: false }, "no task_type is no shell");
        assert_eq!(started(json!({"task_type": "local_bash"})), Usage::TaskBackgrounded { id: "t".into(), shell: true });
        assert_eq!(started(json!({"task_type": "local_agent"})), Usage::TaskBackgrounded { id: "t".into(), shell: false });
        // Every kind the CLI source shows queues nothing for the model when the session stops it is silent; the kinds that queue a message, and any unknown one, are not.
        for kind in ["local_bash", "local_workflow", "remote_agent", "mcp_task", "in_process_teammate", "dream", "auto_mode_scan", "local_memory_import"] {
            assert_eq!(started(json!({"task_type": kind})), Usage::TaskBackgrounded { id: "t".into(), shell: true }, "{kind}");
        }
        for kind in ["local_agent", "monitor_mcp", "monitor_ws", "some_new_kind"] {
            assert_eq!(started(json!({"task_type": kind})), Usage::TaskBackgrounded { id: "t".into(), shell: false }, "{kind}");
        }
        // The move to the background read from `task_updated` names no kind, so it claims no silence.
        let updated = json!({"type": "system", "subtype": "task_updated", "task_id": "t", "patch": {"is_backgrounded": true}}).to_string();
        assert_eq!(read_usage(&updated), Usage::TaskBackgrounded { id: "t".into(), shell: false });
        assert_eq!(started(json!({"is_backgrounded": false})), Usage::Other, "a foreground task is no reading");
        assert_eq!(started(json!({"parent_task_id": "p"})), Usage::Other);
        assert_eq!(started(json!({"awaited": true})), Usage::Other);
        // The notification's old key is no key: a line without a task_id is nothing.
        let keyed = json!({"type": "system", "subtype": "task_notification", "tool_use_id": "toolu_9", "status": "completed"}).to_string();
        assert_eq!(read_usage(&keyed), Usage::Other);
        // The stop is read under any name of the task's id the call carries, and only from a TaskStop call.
        let call = |name: &str, input: Value| {
            let mut event: Value = serde_json::from_str(RECORDS.lines().next().unwrap()).unwrap();
            event["message"]["content"] = json!([{"type": "text", "text": "stopping"}, {"type": "tool_use", "id": "toolu_5", "name": name, "input": input}]);
            let Usage::Request { stops, .. } = read_usage(&event.to_string()) else { panic!("a request") };
            stops
        };
        assert_eq!(call("TaskStop", json!({"task_id": "t"})), ["t"]);
        assert_eq!(call("TaskStop", json!({"shell_id": "u"})), ["u"]);
        for alias in ["KillShell", "KillBash"] {
            assert_eq!(call(alias, json!({"shell_id": "u"})), ["u"], "{alias} is the same stop under another name");
        }
        assert!(call("Bash", json!({"task_id": "t"})).is_empty());
    }

    #[test]
    fn a_shell_task_stopped_by_killshell_or_by_a_deadline_that_beat_our_taskstop_gives_no_cli_turn_and_leaves_the_session_idle() {
        for capture in [BG_KILLSHELL, BG_DEADLINE_RACE] {
            let usages = usages(capture);
            assert!(usages.iter().any(|usage| matches!(usage, Usage::TaskNotified { stopped: true, .. })), "{usages:?}");
            assert!(usages.iter().any(|usage| matches!(usage, Usage::Request { stops, .. } if !stops.is_empty())), "{usages:?}");
            assert_eq!(kinds(&replay(capture)).iter().filter(|kind| **kind == "turn").count(), 1, "no turn begins after ours");
            assert!(idleness(capture).last().unwrap().1, "nothing is owed after the stop");
        }
    }

    #[test]
    fn a_background_agent_announces_itself_as_a_backgrounded_task_and_the_session_is_not_idle_until_its_notification_turn_ends() {
        let usages = usages(BG_AGENT);
        // One task_id names the task at its start and at its notification.
        let ids: Vec<&Usage> = usages.iter().filter(|usage| matches!(usage, Usage::TaskBackgrounded { .. } | Usage::TaskNotified { .. })).collect();
        assert!(matches!(ids[..], [Usage::TaskBackgrounded { id: started, .. }, Usage::TaskNotified { id: ended, stopped: false }] if started == ended), "{ids:?}");
        assert!(usages.iter().position(|usage| matches!(usage, Usage::TaskBackgrounded { .. })) < usages.iter().position(|usage| matches!(usage, Usage::TaskNotified { .. })));
        let idle = idleness(BG_AGENT);
        let labels: Vec<&str> = idle.iter().map(|(label, _)| label.as_str()).collect();
        let idle_after = |label: &str, nth: usize| idle.iter().zip(&labels).filter(|(_, l)| **l == label).nth(nth).unwrap().0.1;
        assert!(!idle_after("backgrounded", 0), "the task runs");
        assert!(!idle_after("result:ok", 0), "the turn that started it ended, the task still runs");
        assert!(!idle_after("notified", 0), "the notification is owed its turn");
        assert!(idle_after("result:ok", 1), "the CLI's notification turn ended");
    }

    #[test]
    fn a_background_task_that_ends_while_no_turn_is_open_gets_a_turn_the_cli_begins_and_the_session_is_idle_after_its_result() {
        let idle = idleness(BG_LATE);
        let labels: Vec<&str> = idle.iter().map(|(label, _)| label.as_str()).collect();
        assert_eq!(significant(&replay(BG_LATE)), ["turn", "echo", "request:10", "level", "backgrounded", "request:10", "result:ok", "notified", "level", "turn", "request:10", "result:ok"]);
        let idle_after = |label: &str, nth: usize| idle.iter().zip(&labels).filter(|(_, l)| **l == label).nth(nth).unwrap().0.1;
        assert!(!idle_after("result:ok", 0), "idle but for the running task");
        assert!(!idle_after("notified", 0), "owed its turn");
        assert!(!idle_after("request:10", 2), "the CLI-begun turn runs");
        assert!(idle_after("result:ok", 1));
    }
}

#[test]
fn a_brief_the_run_claims_is_in_the_task_once_and_not_again_under_also_open() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![brief("claimed", "Do the claimed thing.")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("claimed")])]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo())], Some(0), false);
    let start = &ran.written()[0];
    assert_eq!(start.matches("claimed follow-up").count(), 1, "{start}");
    assert!(start.contains(", repo `"), "the claimed task names the repo the work belongs to: {start}");
    assert!(start.contains("Do the claimed thing.") && !start.contains("Also open for you"), "{start}");
}

/// The Persona's boundary: the bubblewrap wrapper `ClaudeSpawner` runs when it has a sandbox.
#[cfg(unix)]
mod sandbox {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn the_sandbox_arguments_are_pinned() {
        let args = sandbox_args(Path::new("/var/lib/mind-x/sandbox"), Path::new("/run/mind-x"));
        let expected: Vec<String> = [
            "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp", "--tmpfs", "/run", "--ro-bind-try", "/run/systemd/resolve", "/run/systemd/resolve", "--ro-bind", "/run/mind-x", "/run/mind-x", "--bind",
            "/run/mind-x/claude-config", "/run/mind-x/claude-config", "--tmpfs", "/etc/gamecult", "--tmpfs", "/var/lib", "--bind", "/var/lib/mind-x/sandbox", "/var/lib/mind-x/sandbox", "--unshare-pid", "--die-with-parent", "--new-session", "--chdir", "/var/lib/mind-x/sandbox", "--",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(args, expected);
        // The credentials are hidden before the sandbox and the scratch are bound back, and the network is not unshared.
        let at = |needle: &str| args.iter().position(|arg| arg == needle).unwrap();
        assert!(at("/run") < at("/run/mind-x") && at("/var/lib") < at("/var/lib/mind-x/sandbox"));
        // The scratch is read-only to her: no plain bind of it, so she cannot plant a link where the organ writes.
        assert!(!args.windows(3).any(|w| w[0] == "--bind" && w[1] == "/run/mind-x"), "{args:?}");
        assert!(!args.iter().any(|arg| arg == "--unshare-net" || arg == "--unshare-all"));
    }

    #[test]
    fn a_sandboxed_spawner_runs_bwrap_with_its_arguments_then_claude_and_the_child_sees_no_credential() {
        let dir = tempfile::tempdir().unwrap();
        let (bin, scratch, sandbox) = (dir.path().join("bin"), dir.path().join("scratch"), dir.path().join("sandbox"));
        for made in [&bin, &scratch, &sandbox] {
            std::fs::create_dir(made).unwrap();
        }
        let (argv, environment) = (dir.path().join("argv"), dir.path().join("environment"));
        let fake = bin.join("bwrap");
        std::fs::write(&fake, format!("#!/bin/sh\nprintf '%s\n' \"$@\" > {}\nenv > {}\n", argv.display(), environment.display())).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut unit: Vec<(std::ffi::OsString, std::ffi::OsString)> = ["ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "UNLISTED_VARIABLE"].iter().map(|name| ((*name).into(), format!("canary-{name}").into())).collect();
        unit.push(("PATH".into(), format!("{}:/usr/bin:/bin", bin.display()).into()));
        unit.push((SCRATCH_VARIABLE.into(), scratch.clone().into_os_string()));
        let spawner = ClaudeSpawner::in_sandbox(unit, "MIND_PERSONA_CONFIG_SEED", Some(sandbox.clone())).unwrap();
        let mut child = spawner.spawn(&["--flag".to_string(), "value".to_string()], &token_holding("tok-canary"), None).unwrap();
        while !matches!(child.next(StdDuration::from_secs(20)), Read::Exited(_) | Read::Timeout) {}
        let recorded: Vec<String> = std::fs::read_to_string(&argv).unwrap().lines().map(String::from).collect();
        let mut expected = sandbox_args(&sandbox, &scratch);
        expected.extend(["claude", "--flag", "value"].map(String::from));
        assert_eq!(recorded, expected, "bwrap, its arguments, then claude and its own");
        let seen = std::fs::read_to_string(&environment).unwrap();
        assert!(!seen.contains("canary") && !seen.contains("UNLISTED"), "the child's environment is the allow-list: {seen}");
    }
}

#[test]
fn a_user_line_with_images_is_a_text_block_then_one_base64_image_block_each_and_without_them_is_the_bare_text() {
    use super::session::user_line;
    use crate::chat_image::Image;
    let bare: serde_json::Value = serde_json::from_str(&user_line("hello", vec![])).unwrap();
    assert_eq!(bare, json!({"type": "user", "message": {"role": "user", "content": "hello"}}));
    let images = vec![Image { media_type: "image/png", base64: "QUJD".into() }, Image { media_type: "image/jpeg", base64: "REVG".into() }];
    let with: serde_json::Value = serde_json::from_str(&user_line("hello", images)).unwrap();
    assert_eq!(
        with,
        json!({"type": "user", "message": {"role": "user", "content": [
            {"type": "text", "text": "hello"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "QUJD"}},
            {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": "REVG"}},
        ]}})
    );
}

#[test]
fn a_replayed_line_with_images_is_still_our_echo_and_a_tool_result_is_not() {
    let replayed = json!({"type": "user", "isReplay": true, "message": {"role": "user", "content": [
        {"type": "text", "text": "hello"},
        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "QUJD"}},
    ]}}).to_string();
    assert_eq!(read_usage(&replayed), Usage::Echo);
    let tool_result = json!({"type": "user", "isReplay": true, "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "x"}]}}).to_string();
    assert_eq!(read_usage(&tool_result), Usage::Other);
}
