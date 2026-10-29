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
fn a_lease_is_asked_for_before_claude_starts_for_the_runs_budget_and_the_units_runtime_cap() {
    let ran = go(vec![Out(result_line())]);
    assert_eq!(ran.asked, vec![LeaseRequest { expires_at_unix_ms: u64::try_from(at_start().timestamp_millis()).unwrap() + TEST_CAP_S * 1000 }], "the connector sets the reservation: the request is only the expiry, the cap the unit gave");
    assert_eq!(ran.all.first(), Some(&Event::Lent), "nothing happens before the lease");
    assert_eq!(ran.all.get(1), Some(&Event::Spawned), "the child is the next thing");
}

#[test]
fn no_lease_is_asked_for_when_there_is_no_run_to_carry() {
    let ran = carry_with(&Seen::of(&TestMind::seeded()), vec![], Some(0), false);
    assert_eq!(ran.outcome.unwrap(), SelfRunOutcome::NoRun);
    assert!(ran.asked.is_empty() && ran.all.is_empty());
}

#[test]
fn refused_lease_spawns_nothing() {
    for kind in [LeaseRefusalKind::SpendCeiling, LeaseRefusalKind::CostBound, LeaseRefusalKind::UnknownCaller, LeaseRefusalKind::Busy] {
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
fn a_connector_that_cannot_be_reached_is_an_error_that_starts_nothing_and_leaves_the_run_open() {
    let mind = with_run();
    let lender = FakeLender::new(Answer::Unreachable);
    let never = never_spawns();
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    assert!(carry(&mind, &*clock, &lender, &never, TEST_CAP_S, &mut Vec::new()).is_err());
    assert!(never.spawns.borrow().is_empty());
    assert_eq!(mind.ids(K::Resolution), Vec::<String>::new(), "an unreachable connector is no refusal: the run stays open");
}

#[test]
fn settle_sends_carry_usage() {
    let haiku = |line: String| {
        let mut event: Value = serde_json::from_str(&line).unwrap();
        event["message"]["model"] = json!("claude-haiku-5-1");
        event.to_string()
    };
    let ran = go(vec![
        Out(billed("m1", 1, 100, 10, 5)),
        // Records that share a message id are one request, at the usage it had grown to.
        Out(billed("m1", 1, 100, 10, 9)),
        // A subagent's request is billed too, under its own model.
        Out(haiku(subagent_billed("m2", 2, 0, 0, 4))),
        Out(billed("m3", 3, 0, 0, 1)),
        Out("not json".to_string()),
        Out(result_line()),
    ]);
    let used = |model: &str, input, read, write, output| ModelUsage { model: model.into(), usage: TokenUsage { input_tokens: input, cache_read_tokens: read, cache_write_tokens: write, output_tokens: output, reasoning_tokens: None } };
    let expected = Settlement { usage: vec![used("claude-haiku-5-1", 2, 0, 0, 4), used("claude-opus-5-5", 4, 100, 10, 10)] };
    let settled: Vec<&Event> = ran.all.iter().filter(|event| matches!(event, Event::Settled(_))).collect();
    assert_eq!(settled, vec![&Event::Settled(expected)], "settled exactly once, with the stream's sums per model");
    assert!(matches!(ran.all.last(), Some(Event::Settled(_))), "settled after the child ended, not before");
    assert_eq!(ran.count_all(&Event::Abandoned), 0);
}

#[test]
fn a_run_that_reported_nothing_settles_an_empty_report_which_the_connector_bills_as_the_reservation() {
    // Not free: an empty report is the connector's cue to bill the reservation, exactly as for a dropped lease.
    let ran = go(vec![Out(result_line())]);
    assert!(ran.all.contains(&Event::Settled(Settlement::default())));
}

#[test]
fn a_child_that_cannot_be_stopped_is_left_to_the_connector_to_bill_in_full() {
    // Not even SIGKILL is delivered: the child may still be spending, so the lease is dropped unsettled.
    let stuck = carry_scripted(&Seen::of(&with_run()), vec![Out(at("m1", CEILING_TOKENS)), Idle(FLUSH_GRACE_S)], Some(0), None, &[Stop::Interrupt, Stop::Terminate, Stop::Kill], true);
    assert_eq!(stuck.ended().0, Cause::Unstoppable);
    assert_eq!(stuck.count_all(&Event::Abandoned), 1);
    assert!(!stuck.all.iter().any(|event| matches!(event, Event::Settled(_))));
}

#[test]
fn a_run_cut_off_before_its_child_ended_is_billed_the_reservation_not_a_guess() {
    // The output stream fails mid-run: `carry` unwinds with the child still alive. The lease is dropped
    // unsettled, as when the brake stops the unit, and the connector bills the reservation.
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
    let lender = FakeLender::logging(log.clone(), Answer::Grant);
    assert!(carry(&Seen::of(&mind), &*clock, &lender, &fake, TEST_CAP_S, &mut Broken).is_err());
    assert_eq!(log.borrow().last(), Some(&Event::Abandoned));
    assert!(!log.borrow().iter().any(|event| matches!(event, Event::Settled(_))));
}

#[test]
fn a_child_that_cannot_start_settles_an_empty_report_which_the_connector_bills_as_the_reservation() {
    let mind = with_run();
    let log = Rc::new(RefCell::new(Vec::new()));
    let lender = FakeLender::logging(log.clone(), Answer::Grant);
    let refusing = Fake { child: RefCell::new(None), spawns: RefCell::new(Vec::new()), refuse: true };
    let clock = Rc::new(Fixed(Cell::new(at_start())));
    assert!(carry(&Seen::of(&mind), &*clock, &lender, &refusing, TEST_CAP_S, &mut Vec::new()).is_err());
    assert_eq!(*log.borrow(), vec![Event::Lent, Event::Settled(Settlement::default())]);
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
fn claudes_own_per_model_total_in_the_result_is_what_settles_and_the_last_result_wins() {
    let used = |model: &str, input, read, write, output| ModelUsage { model: model.into(), usage: TokenUsage { input_tokens: input, cache_read_tokens: read, cache_write_tokens: write, output_tokens: output, reasoning_tokens: None } };
    let ran = go(vec![
        Out(billed("m1", 1, 100, 10, 5)),
        // An earlier result is superseded by the cumulative one after it (the flush turn's).
        Out(result_stating(json!({"claude-opus-5-5": stated(7, 8, 9, 10)}))),
        Out(billed("m2", 3, 0, 0, 1)),
        Out(result_stating(json!({"claude-opus-5-5": stated(11, 12, 13, 14), "claude-haiku-5-1": stated(1, 2, 3, 4)}))),
    ]);
    let expected = Settlement { usage: vec![used("claude-haiku-5-1", 1, 2, 3, 4), used("claude-opus-5-5", 11, 12, 13, 14)] };
    let settled: Vec<&Event> = ran.all.iter().filter(|event| matches!(event, Event::Settled(_))).collect();
    assert_eq!(settled, vec![&Event::Settled(expected)], "the stream's own sums are not added to claude's total");
}

#[test]
fn a_result_that_states_no_model_usage_leaves_the_streams_sums_to_settle() {
    let ran = go(vec![Out(billed("m1", 1, 100, 10, 5)), Out(result_stating(json!({}))), Out(result_stating(json!("not a map")))]);
    let settled: Vec<&Event> = ran.all.iter().filter(|event| matches!(event, Event::Settled(_))).collect();
    let used = ModelUsage { model: UNNAMED_MODEL.into(), usage: TokenUsage { input_tokens: 1, cache_read_tokens: 100, cache_write_tokens: 10, output_tokens: 5, reasoning_tokens: None } };
    assert!(matches!(settled.as_slice(), [Event::Settled(Settlement { usage })] if usage.len() == 1 && usage[0].usage == used.usage), "{settled:?}");
}

#[test]
fn the_journal_says_what_was_billed_and_that_a_failed_settle_leaves_the_reservation_billed() {
    let run = |answer: Answer| {
        let mind = with_run();
        let log = Rc::new(RefCell::new(Vec::new()));
        let clock = Rc::new(Fixed(Cell::new(at_start())));
        let child = Scripted { steps: vec![Out(at("m1", 10)), Out(result_line())].into(), log: log.clone(), clock: clock.clone(), code: Some(0), dies_on: None, refuses: vec![], hangs: false, exited: false, eof: false, asked: 0 };
        let fake = Fake { child: RefCell::new(Some(child)), spawns: RefCell::new(Vec::new()), refuse: false };
        let mut out = Vec::new();
        let outcome = carry(&Seen::of(&mind), &*clock, &FakeLender::logging(log, answer), &fake, TEST_CAP_S, &mut out).unwrap();
        (outcome, String::from_utf8(out).unwrap())
    };
    let (outcome, out) = run(Answer::Grant);
    assert_eq!(out, "request 1 prefix 10\nsettled: billed 7 micro-USD\n");
    let (failed, out) = run(Answer::GrantSettleFails);
    assert_eq!(out, "request 1 prefix 10\nsettle failed: the connector bills the reservation\n", "the connector's words are not echoed");
    assert_eq!(failed, outcome, "the run's own outcome stands");
}

fn read_total(settlement: &Settlement) -> u64 {
    settlement.usage.iter().map(|used| used.usage.cache_read_tokens).sum()
}

#[test]
fn requests_after_the_last_result_are_settled_on_top_of_claudes_total() {
    // The flush turn runs after the first result and is stopped before it states a result of its own.
    let ran = go(vec![
        Out(billed("m1", 1, 100, 10, 5)),
        Out(result_stating(json!({"claude-opus-5-5": stated(41, 812_345, 20_480, 9_123)}))),
        Out(billed("m2", 1000, 900_000, 0, 500)),
        Out(billed("m2", 1000, 900_000, 0, 700)),
    ]);
    let total = |input, read, write, output| Settlement { usage: vec![ModelUsage { model: "claude-opus-5-5".into(), usage: TokenUsage { input_tokens: input, cache_read_tokens: read, cache_write_tokens: write, output_tokens: output, reasoning_tokens: None } }] };
    let settled: Vec<&Event> = ran.all.iter().filter(|event| matches!(event, Event::Settled(_))).collect();
    assert_eq!(settled, vec![&Event::Settled(total(1_041, 1_712_345, 20_480, 9_823))], "claude's total plus the tail request once");
}

#[test]
fn two_results_and_a_tail_settle_the_last_total_plus_only_what_came_after_it() {
    let mut spent = Spent::default();
    spent.hear(&billed("m1", 1, 100, 0, 1));
    spent.hear(&result_stating(json!({"claude-opus-5-5": stated(1, 100, 0, 1)})));
    spent.hear(&billed("m2", 2, 200, 0, 2));
    spent.hear(&result_stating(json!({"claude-opus-5-5": stated(3, 300, 0, 3)})));
    assert_eq!(read_total(&spent.settlement()), 300, "m2 is inside the second result's total");
    spent.hear(&billed("m3", 4, 400, 0, 4));
    let settled = spent.settlement();
    assert_eq!(read_total(&settled), 700);
    assert_eq!(settled.usage.len(), 1, "the tail joins its model's total");
    assert_eq!(settled.usage[0].usage.input_tokens, 7);
    // A later empty result neither erases the total nor folds the tail in.
    spent.hear(&result_stating(json!({})));
    assert_eq!(read_total(&spent.settlement()), 700);
}
