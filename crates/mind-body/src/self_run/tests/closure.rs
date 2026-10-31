//! Every exit of `carry` resolves her run Recorded exactly once, and a run left
//! open by a process that died is closed at the next start.

use eureka_pipeline::ResolutionOutcome;

use super::*;

/// The reason of the run's one closure; fails if it was closed other than once.
fn reason_of(mind: &TestMind, label: &str) -> String {
    let id = format!("{INSTANCE}:resolution:run.{label}.n1");
    let PipelineDocument::Resolution(closure) = mind.view(K::Resolution, &id).unwrap_or_else(|| panic!("{label} was not closed")).document else { panic!("not a resolution") };
    assert!(mind.view(K::Resolution, &format!("{INSTANCE}:resolution:run.{label}.n2")).is_none(), "{label} was closed twice");
    match closure.outcome {
        ResolutionOutcome::Recorded { reason } => reason.0,
        other => panic!("not Recorded: {other:?}"),
    }
}

fn result_with(text: &str) -> String {
    let mut event: Value = serde_json::from_str(&result_line()).unwrap();
    event["result"] = json!(text);
    event.to_string()
}

#[test]
fn a_normal_exit_resolves_the_run_once_with_the_results_subtype_error_flag_and_text() {
    let mind = with_run();
    let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
    assert!(ran.outcome.is_ok());
    assert_eq!(ran.admits, 1);
    let reason = reason_of(&mind, "selfrun-a");
    assert!(reason.contains("subtype=success is_error=true") && reason.contains("Failed to authenticate: OAuth session expired"), "{reason}");
    assert_eq!(mind.ids(K::Resolution).len(), 1);
}

#[test]
fn the_excerpt_is_bounded_and_cut_at_a_character_boundary() {
    let mind = with_run();
    // Two bytes a character, so the limit falls inside one when it is odd; here it is even, so add a one-byte head.
    let text = format!("x{}", "é".repeat(3000));
    carry_with(&Seen::of(&mind), vec![Out(result_with(&text))], Some(0), false).outcome.unwrap();
    let reason = reason_of(&mind, "selfrun-a");
    let excerpt = reason.split_once(": ").unwrap().1;
    assert!(excerpt.contains("[truncated]") && excerpt.len() <= REASON_EXCERPT_LIMIT + "\n[truncated]".len() + 120, "{}", reason.len());
    assert!(excerpt.matches('é').count() <= 400);
}

#[test]
fn a_child_that_ends_with_no_result_resolves_the_run_and_says_so() {
    let mind = with_run();
    let ran = carry_with(&Seen::of(&mind), vec![Out(at("m1", 10))], Some(2), false);
    assert_eq!(ran.ended().1, Some(2));
    let reason = reason_of(&mind, "selfrun-a");
    assert!(reason.contains("claude gave no result") && reason.contains("code 2"), "{reason}");
}

#[test]
fn a_child_stopped_by_the_actuator_resolves_the_run_with_how_it_ended() {
    let mind = with_run();
    carry_with(&Seen::of(&mind), vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], None, true).outcome.unwrap();
    assert!(reason_of(&mind, "selfrun-a").contains("interrupted"));
}

#[test]
fn a_child_that_cannot_start_resolves_the_run_and_the_error_stands() {
    let mind = with_run();
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let refusing = Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: true };
    let error = carry(&Seen::of(&mind), &*clock, &FakeLender::new(Answer::Grant), &refusing, TEST_CAP_S, &mut Vec::new()).unwrap_err();
    assert_eq!(error.to_string(), "refused");
    assert!(reason_of(&mind, "selfrun-a").contains("the actuator failed"));
}

#[test]
fn an_error_after_the_run_opened_resolves_it() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mind = with_run();
    let log = Rc::new(RefCell::new(Vec::new()));
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let child = Scripted { steps: vec![Out(at("m1", 10))].into(), log: log.clone(), clock: clock.clone(), code: Some(0), dies_on: None, refuses: vec![], hangs: true, exited: false, eof: false, asked: 0 };
    let fake = Fake { child: RefCell::new(Some(child)), spawns: RefCell::new(Vec::new()), refuse: false };
    assert!(carry(&Seen::of(&mind), &*clock, &FakeLender::logging(log, Answer::Grant), &fake, TEST_CAP_S, &mut Broken).is_err());
    assert!(reason_of(&mind, "selfrun-a").contains("the actuator failed"));
}

#[test]
fn a_run_the_child_closed_itself_is_not_closed_again() {
    let mind = with_run();
    mind.committed(vec![close(run_ref("selfrun-a"), recorded())]);
    // Closed, so it is not hers to carry or to close: nothing is admitted.
    let seen = Seen::of(&mind);
    assert_eq!(carry_with(&seen, vec![Out(result_line())], Some(0), false).outcome.unwrap(), SelfRunOutcome::NoRun);
    assert_eq!(seen.admits.get(), 0);
}

#[test]
fn a_run_still_open_at_the_next_start_is_an_orphan_and_is_closed_before_the_newer_one_is_carried() {
    let mind = TestMind::seeded();
    mind.committed(vec![self_run("selfrun-old", "5")]);
    mind.committed(vec![self_run("selfrun-a", "5")]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(result_line())], Some(0), false);
    assert!(ran.outcome.is_ok());
    assert_eq!(ran.spawns.len(), 1);
    assert!(ran.spawns[0].contains(&SESSION_OF_RUN_A.to_string()), "the newer run is the one carried");
    assert_eq!(reason_of(&mind, "selfrun-old"), "orphaned: the run's process ended without closing it");
    assert!(reason_of(&mind, "selfrun-a").contains("subtype=success"));
    assert_eq!(mind.ids(K::Resolution).len(), 2);
}

#[test]
fn a_resolve_that_fails_is_logged_and_tried_once() {
    let mind = with_run();
    let mut seen = Seen::of(&mind);
    seen.admit_fails = true;
    let ran = carry_with(&seen, vec![Out(result_line())], Some(0), false);
    assert!(format!("{:#}", ran.outcome.unwrap_err()).contains("could not be closed"));
    assert_eq!(ran.admits, 1, "one try, no loop");
    assert!(ran.out.contains("the run could not be closed"));
    assert!(mind.ids(K::Resolution).is_empty());

    // The same when the run's own error is what is being reported: that error stands.
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let refusing = Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: true };
    let again = Seen { admit_fails: true, ..Seen::of(&mind) };
    let error = carry(&again, &*clock, &FakeLender::new(Answer::Grant), &refusing, TEST_CAP_S, &mut Vec::new()).unwrap_err();
    assert_eq!(error.to_string(), "refused");
    assert_eq!(again.admits.get(), 1);
}
