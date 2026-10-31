//! The credential lease as `carry` uses it: asked for before claude exists,
//! refusals as typed outcomes, settled once with what the stream reported.
//! The wire itself is `lease_client`'s tests; the real child is `process::lease_child`.

use eureka_pipeline::ResolutionOutcome;

use super::*;

/// A real assistant record with all four counters of its usage set.
fn billed(id: &str, input: u64, read: u64, write: u64, output: u64) -> String {
    let mut event: Value = serde_json::from_str(&request(id, input, read, write)).unwrap();
    event["message"]["usage"]["output_tokens"] = json!(output);
    event.to_string()
}

fn subagent_billed(id: &str, input: u64, read: u64, write: u64, output: u64) -> String {
    let mut event: Value = serde_json::from_str(&billed(id, input, read, write, output)).unwrap();
    event["parent_tool_use_id"] = json!("toolu_0001");
    event.to_string()
}

fn never_spawns() -> Fake {
    Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: false }
}

#[test]
fn no_lease_is_asked_for_when_there_is_no_run_to_carry() {
    let ran = carry_with(&Seen::of(&TestMind::seeded()), vec![], Some(0), false);
    assert_eq!(ran.outcome.unwrap(), SelfRunOutcome::NoRun);
    assert!(ran.asked.is_empty() && ran.all.is_empty());
}

#[test]
fn a_lease_refused_for_its_length_is_a_configuration_error_that_starts_nothing_and_withdraws_the_run() {
    let mind = with_run();
    let lender = FakeLender::new(Answer::Refuse(LeaseRefusalKind::Expired));
    let never = never_spawns();
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    let error = carry(&mind, &*clock, &lender, &never, TEST_CAP_S, &mut Vec::new()).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("--lease-max-ms") && message.contains("5400") && message.contains(RUN_CAP_VARIABLE), "{message}");
    assert!(!message.contains("detail-canary"), "the connector's words are not echoed");
    assert!(never.spawns.borrow().is_empty());
    // Withdrawn, never Recorded: the breaker counts no withdrawn run, so a misconfigured unit never trips it.
    let closure = mind.view(K::Resolution, &format!("{INSTANCE}:resolution:run.selfrun-a.n1")).expect("the run is withdrawn");
    let PipelineDocument::Resolution(closure) = closure.document else { panic!("not a resolution") };
    assert_eq!(closure.subject, run_ref("selfrun-a"));
    assert!(matches!(closure.outcome, ResolutionOutcome::Withdrawn { .. }), "{:?}", closure.outcome);
    let again = FakeLender::new(Answer::Grant);
    assert_eq!(carry(&mind, &*clock, &again, &never, TEST_CAP_S, &mut Vec::new()).unwrap(), SelfRunOutcome::NoRun, "no open run is left for a later launch to find dead and record");
}

#[test]
fn a_connector_that_cannot_be_reached_is_an_error_that_starts_nothing_and_closes_the_run_recorded() {
    let mind = with_run();
    let lender = FakeLender::new(Answer::Unreachable);
    let never = never_spawns();
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    assert!(carry(&mind, &*clock, &lender, &never, TEST_CAP_S, &mut Vec::new()).is_err());
    assert!(never.spawns.borrow().is_empty());
    // No refusal, so no refusal kind: the actuator failed after the run opened, and the run does not stay in force.
    let closure = mind.view(K::Resolution, &format!("{INSTANCE}:resolution:run.selfrun-a.n1")).expect("the run is closed");
    let PipelineDocument::Resolution(closure) = closure.document else { panic!("not a resolution") };
    assert!(matches!(&closure.outcome, ResolutionOutcome::Recorded { reason } if reason.0.contains("the actuator failed")), "{:?}", closure.outcome);
}

fn result_stating(models: Value) -> String {
    let mut event: Value = serde_json::from_str(&result_line()).unwrap();
    event["modelUsage"] = models;
    event.to_string()
}

fn stated(input: u64, read: u64, write: u64, output: u64) -> Value {
    json!({"inputTokens": input, "cacheReadInputTokens": read, "cacheCreationInputTokens": write, "outputTokens": output, "costUSD": 0.5, "contextWindow": 200000})
}

#[test]
fn a_lease_is_asked_for_before_claude_starts_for_the_units_runtime_cap() {
    let ran = go(vec![Out(result_line())]);
    assert_eq!(ran.asked, vec![LeaseRequest { expires_at_unix_ms: u64::try_from(at_start().timestamp_millis()).unwrap() + TEST_CAP_S * 1000 }], "the request is only the expiry, the cap the unit gave");
    assert_eq!(ran.all.first(), Some(&Event::Lent), "nothing happens before the lease");
    assert_eq!(ran.all.get(1), Some(&Event::Spawned), "the child is the next thing");
}

#[test]
fn refused_lease_spawns_nothing() {
    for kind in [LeaseRefusalKind::UnknownCaller, LeaseRefusalKind::Busy, LeaseRefusalKind::ProviderRejected] {
        let mind = with_run();
        let lender = FakeLender::new(Answer::Refuse(kind));
        let never = never_spawns();
        let clock = Rc::new(Fixed(Cell::new(at_start())));
        let mut out = Vec::new();
        let outcome = carry(&mind, &*clock, &lender, &never, TEST_CAP_S, &mut out).unwrap();
        assert_eq!(outcome, SelfRunOutcome::LeaseRefused { kind });
        assert_eq!(outcome.exit_status(), 4);
        assert!(never.spawns.borrow().is_empty(), "{kind:?}: the spawner is never called");
        assert!(!format!("{outcome} {}", String::from_utf8_lossy(&out)).contains("detail-canary"), "the connector's words are not echoed");

        // The run is closed Recorded, with the kind, so a run that cannot be paid for counts as an empty one.
        let closure = mind.view(K::Resolution, &format!("{INSTANCE}:resolution:run.selfrun-a.n1")).expect("the run is closed");
        let PipelineDocument::Resolution(closure) = closure.document else { panic!("not a resolution") };
        assert_eq!(closure.subject, run_ref("selfrun-a"));
        assert!(matches!(&closure.outcome, ResolutionOutcome::Recorded { reason } if reason.0.contains(&format!("{kind:?}"))), "{:?}", closure.outcome);

        // Closed, so it is no longer her open run: nothing is carried or leased for it again.
        let again = FakeLender::new(Answer::Grant);
        assert_eq!(carry(&mind, &*clock, &again, &never, TEST_CAP_S, &mut Vec::new()).unwrap(), SelfRunOutcome::NoRun);
        assert!(again.asked.borrow().is_empty());
    }
}

#[test]
fn the_lease_is_held_until_the_child_ends_and_then_dropped_with_nothing_settled() {
    let ran = go(vec![Out(result_line())]);
    assert_eq!(ran.all.last(), Some(&Event::Abandoned), "the lease ends by being dropped");
    assert!(!ran.out.contains("settle"), "no settle report exists");
}
