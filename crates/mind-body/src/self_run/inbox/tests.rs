//! The inbox over a real spool directory and a mind in memory: what is derived,
//! the exactly-once claim, and the hook that prints it.

use eureka_pipeline::{PipelineDocument, PipelineKind as K, PipelineRef, RunOperator, RunTurn};

use super::*;
use crate::testkit::{TestMind, close as closing, follow_up, id, recorded, reference, run, run_ref, spec, spec_ref, withdrawn};

fn spool_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn brief_doc(label: &str, owner: &str) -> PipelineDocument {
    let PipelineDocument::FollowUp(mut document) = follow_up(label, &spec_ref("x")) else { unreachable!() };
    document.owner = owner.into();
    document.item = format!("Do {label}.").as_str().into();
    PipelineDocument::FollowUp(document)
}

fn brief_ref(label: &str) -> PipelineRef {
    reference(K::FollowUp, &id("follow_up", label))
}

fn mind_with(documents: Vec<PipelineDocument>) -> TestMind {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x")]);
    mind.committed(documents);
    mind
}

fn entry(n: u64) -> Brief {
    Brief { id: format!("eureka-body:follow_up:b{n}"), campaign: "eureka-body".into(), repo: "GameCult/Huginn".into(), item: format!("Item {n}."), ordinal: n }
}

#[test]
fn the_inbox_is_the_open_self_run_briefs_that_no_closed_run_claims_and_the_session_does_not_hold() {
    let mind = mind_with(vec![brief_doc("open", OWNER), brief_doc("claimed-by-old", OWNER), brief_doc("someone-elses", "Hands"), brief_doc("answered", OWNER), brief_doc("held", OWNER)]);
    mind.committed(vec![run("old", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("claimed-by-old")])]);
    mind.committed(vec![closing(run_ref("old"), recorded()), closing(brief_ref("answered"), withdrawn())]);
    let spool = spool_dir();
    mark_told(spool.path(), &[Brief { id: brief_ref("held").id.0, ..entry(9) }]).unwrap();
    let derived: Vec<String> = derive(&mind, spool.path()).unwrap().into_iter().map(|brief| brief.id).collect();
    assert_eq!(derived, vec![brief_ref("open").id.0], "only the open one nobody answered and nobody holds");
    // A run that is still open claims without answering: its brief is still asked.
    mind.committed(vec![run("live", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("open")])]);
    assert_eq!(derive(&mind, spool.path()).unwrap().len(), 1);
}

#[test]
fn spooled_entries_are_claimed_in_order_and_hand_back_what_was_written() {
    let spool = spool_dir();
    spool_briefs(spool.path(), &[entry(2), entry(1)]);
    assert!(pending(spool.path()).unwrap());
    let claimed = claim(spool.path()).unwrap();
    assert_eq!(claimed, vec![entry(1), entry(2)], "oldest first");
    assert!(!pending(spool.path()).unwrap() && claim(spool.path()).unwrap().is_empty(), "a claim empties the spool");
    assert_eq!(entries(&spool.path().join("told")).unwrap(), vec![entry(1), entry(2)], "the claimed entries are the session's record");
}

fn spool_briefs(dir: &Path, briefs: &[Brief]) {
    spool(dir, briefs).unwrap();
}

#[test]
fn a_hook_and_the_actuator_claiming_at_once_deliver_each_entry_to_exactly_one_of_them() {
    for _ in 0..20 {
        let spool = spool_dir();
        let briefs: Vec<Brief> = (1..=60).map(entry).collect();
        spool_briefs(spool.path(), &briefs);
        let path = spool.path().to_path_buf();
        let racers: Vec<_> = (0..4)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || claim(&path).unwrap())
            })
            .collect();
        let mut seen: Vec<u64> = racers.into_iter().flat_map(|racer| racer.join().unwrap()).map(|brief| brief.ordinal).collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=60).collect::<Vec<u64>>(), "every entry once, none twice, none lost");
    }
}

#[test]
fn the_hook_prints_data_only_and_only_for_the_main_thread() {
    let spool = spool_dir();
    write_plan(spool.path(), "Plan usage: unobserved.").unwrap();
    assert_eq!(inject(spool.path(), "{}", false).unwrap(), Injection { stdout: String::new(), exit: 0 }, "an empty spool prints nothing");
    spool_briefs(spool.path(), &[entry(1)]);
    // A subagent's call (the input carries an agent id) prints nothing and claims nothing.
    let subagent = inject(spool.path(), "{\"hook_event_name\":\"PostToolBatch\",\"agent_id\":\"a1\"}", false).unwrap();
    assert_eq!(subagent, Injection { stdout: String::new(), exit: 0 });
    assert!(pending(spool.path()).unwrap());
    let printed = inject(spool.path(), "{\"hook_event_name\":\"PostToolBatch\"}", false).unwrap();
    assert_eq!(printed.exit, 0);
    let output: Value = serde_json::from_str(&printed.stdout).unwrap();
    assert_eq!(output["hookSpecificOutput"]["hookEventName"], "PostToolBatch");
    let context = output["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    for needle in ["eureka-body:follow_up:b1", "campaign eureka-body", "repo GameCult/Huginn", "Item 1.", "Plan usage: unobserved."] {
        assert!(context.contains(needle), "{needle}: {context}");
    }
    assert_eq!(inject(spool.path(), "{}", false).unwrap().stdout, "", "claimed once");
    // Unreadable input is not a reason to lose a brief or to fail the tool call.
    spool_briefs(spool.path(), &[entry(2)]);
    assert!(inject(spool.path(), "not json", false).unwrap().stdout.contains("Item 2."));
}

#[test]
fn the_pre_compact_hook_blocks_with_exit_2_and_leaves_one_marker() {
    let spool = spool_dir();
    spool_briefs(spool.path(), &[entry(1)]);
    assert_eq!(inject(spool.path(), "{}", true).unwrap(), Injection { stdout: String::new(), exit: 2 });
    assert!(pending(spool.path()).unwrap(), "blocking claims nothing");
    assert!(take_pre_compact(spool.path()) && !take_pre_compact(spool.path()), "the marker is consumed once");
}

#[test]
fn the_settings_name_the_two_hooks_with_absolute_quoted_commands() {
    let hooks = Hooks { spool: PathBuf::from("/run/mind-x-self/inbox"), injector: PathBuf::from("/opt/it's/mind-self-run") };
    let settings: Value = serde_json::from_str(&hooks.settings()).unwrap();
    let tell = settings["hooks"]["PostToolBatch"][0]["hooks"][0]["command"].as_str().unwrap();
    assert_eq!(tell, "'/opt/it'\\''s/mind-self-run' inject --spool '/run/mind-x-self/inbox'");
    let compact = &settings["hooks"]["PreCompact"][0];
    assert_eq!(compact["matcher"], "auto");
    assert_eq!(compact["hooks"][0]["command"].as_str().unwrap(), format!("{tell} --pre-compact"));
}
