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

fn read_total(settlement: &Settlement) -> u64 {
    settlement.usage.iter().map(|used| used.usage.cache_read_tokens).sum()
}
