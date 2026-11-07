//! A Self session serves one conversation at a time (ruling `eureka-body:ruling:self-serves-one-conversation-at-a-time`): another
//! conversation's brief is written only when the session is idle, and every result belongs to the conversation being served. The real-order
//! tests replay the claude 2.1.312 captures with briefs spliced in at named points of the stream, and carry synthetic words only.

use eureka_pipeline::{MemoryOrigin, PipelineReply};

use super::*;

const BG_IDLE: &str = include_str!("../claude-2.1.312-live-session-bg-idle.jsonl");
const BG_FOLD: &str = include_str!("../claude-2.1.312-live-session-bg-fold.jsonl");
const BG_OTHER: &str = include_str!("../claude-2.1.312-live-session-bg-other.jsonl");
const BG_RACE: &str = include_str!("../claude-2.1.312-live-session-bg-race.jsonl");
const BG_LATE: &str = include_str!("../claude-2.1.312-live-session-bg-late.jsonl");
const BG_FGTO: &str = include_str!("../claude-2.1.312-live-session-bg-fgto.jsonl");
const BG_KILL: &str = include_str!("../claude-2.1.312-live-session-bg-kill.jsonl");
const BG_MONITOR: &str = include_str!("../claude-2.1.312-live-session-bg-monitor.jsonl");

fn saying(text: &str) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "terminal_reason": "completed", "result": text}).to_string()
}

fn failing() -> String {
    json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "terminal_reason": "model_error", "result": "the API said no"}).to_string()
}

fn reply_of(mind: &TestMind, n: u32) -> Option<PipelineReply> {
    match mind.view(K::Reply, &format!("{INSTANCE}:reply:selfrun-a-r{n}"))?.document {
        PipelineDocument::Reply(reply) => Some(reply),
        _ => None,
    }
}

fn answered(mind: &TestMind, n: u32) -> Vec<String> {
    reply_of(mind, n).unwrap_or_else(|| panic!("reply {n} was not admitted")).answers.iter().map(|brief| brief.id.0.clone()).collect()
}

/// A brief her Persona gave from a DM with `member` in `room`.
fn brief_in(label: &str, room: &str, member: &str) -> PipelineDocument {
    let PipelineDocument::FollowUp(mut document) = brief(label, &format!("Walnut work for {member}.")) else { unreachable!() };
    document.origin = Some(MemoryOrigin::Direct { room: room.into(), with: vec![member.into()] });
    PipelineDocument::FollowUp(document)
}

fn aqua(label: &str) -> PipelineDocument {
    brief_in(label, "!aqua:chat", "@aqua:chat")
}

fn libby(label: &str) -> PipelineDocument {
    brief_in(label, "!libby:chat", "@libby:chat")
}

fn mira(label: &str) -> PipelineDocument {
    brief_in(label, "!mira:chat", "@mira:chat")
}

fn id_of(label: &str) -> String {
    brief_ref(label).id.0
}

fn written_with(ran: &Ran, text: &str) -> bool {
    ran.written().iter().any(|line| line.contains(text))
}

/// A mind whose Self run has one open brief of Aqua's: Aqua's conversation is the one served from the start.
fn serving_aqua() -> Rc<TestMind> {
    let mind = live_mind();
    persona_commit(&mind, vec![aqua("a1")]);
    mind
}

// ---- the captures, with briefs spliced in at named points -----------------

/// The lines of a capture, in order: what the CLI said. The probe's own writes are dropped.
fn stream(jsonl: &str) -> Vec<String> {
    jsonl.lines().filter_map(|entry| serde_json::from_str::<Value>(entry).ok()?.get("line")?.as_str().map(String::from)).collect()
}

fn label_of(line: &str) -> &'static str {
    match read_usage(line) {
        Usage::TurnBegan => "init",
        Usage::Request { .. } => "request",
        Usage::Echo => "echo",
        Usage::Result { .. } => "result",
        Usage::BackgroundTasks { .. } => "level",
        Usage::TaskBackgrounded { .. } => "backgrounded",
        Usage::TaskNotified { .. } => "notified",
        Usage::NotificationTaken { .. } => "taken",
        _ => "other",
    }
}

/// The index just after the `nth` (from 1) line of this label.
fn after(lines: &[String], label: &str, nth: usize) -> usize {
    lines.iter().enumerate().filter(|(_, line)| label_of(line) == label).nth(nth - 1).map(|(at, _)| at + 1).unwrap_or_else(|| panic!("no {label} number {nth}"))
}

/// The capture's lines as steps, with each splice's steps put in after the line at its index.
fn script(lines: &[String], splices: Vec<(usize, Vec<Step>)>) -> Vec<Step> {
    let mut steps = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        steps.push(Out(line.clone()));
        for (point, extra) in &splices {
            if *point == at + 1 {
                steps.extend(extra.iter().cloned());
            }
        }
    }
    steps
}

/// A message of the CLI's that the stream gives for a task the session never saw start.
fn notification_of(task: &str) -> String {
    json!({"type": "system", "subtype": "task_notification", "task_id": task, "status": "completed", "summary": "synthetic task"}).to_string()
}

// ---- who is served at the start --------------------------------------------

#[test]
fn the_start_line_carries_one_conversation_and_the_other_is_told_on_its_own_line() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1"), libby("l1"), aqua("a2")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("a1")])]);
    let steps = vec![Out(echo()), Out(saying("aqua done")), Idle(31), Out(echo()), Out(saying("libby done"))];
    let ran = carry_with(&Seen::of(&mind), steps, Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("a1")) && start.contains(&id_of("a2")), "the claimed brief and the open one of its conversation: {start}");
    assert!(!start.contains(&id_of("l1")), "the other conversation's brief is not on the start line: {start}");
    let told = tells(&ran);
    assert_eq!(told.len(), 1, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("l1")) && !told[0].contains(&id_of("a1")) && !told[0].contains(&id_of("a2")), "{}", told[0]);
    assert_eq!(answered(&mind, 1).len(), 2, "the start line's briefs");
    assert!(!answered(&mind, 1).contains(&id_of("l1")));
    assert_eq!(answered(&mind, 2), vec![id_of("l1")]);
}

#[test]
fn with_no_claimed_brief_the_start_set_is_the_oldest_open_briefs_conversation() {
    let mind = live_mind();
    for brief in [libby("l1"), aqua("a1"), libby("l2")] {
        persona_commit(&mind, vec![brief]);
    }
    let ran = mind_run(&mind, vec![Out(echo()), Out(saying("libby done")), Idle(31), Out(echo()), Out(saying("aqua done"))]);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("l1")) && start.contains(&id_of("l2")) && !start.contains(&id_of("a1")), "{start}");
    assert_eq!(answered(&mind, 2), vec![id_of("a1")]);
}

/// A restart whose claimed brief was resolved while another conversation's brief is open: the claim's conversation, read by key, is the start line's.
#[test]
fn a_restart_with_a_resolved_claim_opens_the_gate_for_its_conversation_only() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1")]);
    persona_commit(&mind, vec![libby("l1")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("a1")])]);
    mind.committed(vec![close(brief_ref("a1"), recorded())]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo()), Out(saying("aqua done")), Idle(31), Out(echo()), Out(saying("libby done"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("a1")) && !start.contains(&id_of("l1")), "the resolved claim's conversation, read by key, is the start line's: {start}");
    assert_eq!(answered(&mind, 1), vec![id_of("a1")]);
    assert_eq!(answered(&mind, 2), vec![id_of("l1")], "told in the right conversation, none stranded");
}

/// The claim is the first open brief and another conversation's brief is next: the start set follows the claim.
#[test]
fn the_start_set_follows_the_claim_not_the_next_open_brief() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1")]);
    persona_commit(&mind, vec![libby("l1")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("a1")])]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo()), Out(saying("aqua done"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("a1")) && !start.contains(&id_of("l1")), "{start}");
}

/// A restart whose claim is resolved and which has no open brief: a brief of another conversation arriving is not written while the
/// claim's start line is unanswered.
#[test]
fn a_restart_with_a_resolved_claim_and_no_open_brief_holds_the_gate_for_the_claims_conversation() {
    let setup = || {
        let mind = Rc::new(TestMind::seeded());
        mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
        persona_commit(&mind, vec![aqua("a1")]);
        mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("a1")])]);
        mind.committed(vec![close(brief_ref("a1"), recorded())]);
        mind
    };
    let steps = |mind: &Rc<TestMind>| vec![Out(echo()), briefed(mind, 31, vec![libby("l1")]), Idle(31)];
    let mind = setup();
    let ran = mind_run(&mind, steps(&mind));
    assert!(ran.outcome.is_ok());
    assert!(ran.written()[0].contains(&id_of("a1")), "{:?}", ran.written());
    assert!(!written_with(&ran, &id_of("l1")), "{:?}", ran.written());

    let mind = setup();
    let steps = [steps(&mind), vec![Out(saying("aqua done")), Idle(31), Out(echo()), Out(saying("libby done"))]].concat();
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("a1")], vec![id_of("l1")]));
}

/// The resolved claim is libby's and an aqua brief that was admitted before it is open: the claim's own conversation, read by its key, is the start line's.
#[test]
fn a_resolved_claims_conversation_is_read_by_its_own_key() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1")]);
    persona_commit(&mind, vec![libby("l1")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("l1")])]);
    mind.committed(vec![close(brief_ref("l1"), recorded())]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo()), Out(saying("libby start done")), Idle(31), Out(echo()), Out(saying("aqua done"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("l1")) && !start.contains(&id_of("a1")), "{start}");
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("l1")], vec![id_of("a1")]));
}

/// A claimed cut spec is no brief and has no conversation: it stays on the start line beside the open briefs of any conversation.
#[test]
fn a_claimed_cut_spec_stays_on_the_start_line_beside_open_briefs() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[spec_ref("x")])]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo()), Out(saying("done"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&format!("claimed cut spec `{}`", spec_ref("x").id.0)) && start.contains(&id_of("a1")), "{start}");
}

// ---- the real order of a background task -----------------------------------

/// The notification arrives inside the starting turn; after its result the CLI begins a turn of its own for it (init, a request, no echo).
/// Libby's brief, derived inside Aqua's turn and in every window after, is written into none of it: only after the notification turn's result.
#[test]
fn a_notified_task_holds_other_conversations_until_the_turn_the_cli_begins_for_it_ends() {
    let lines = stream(BG_IDLE);
    let (request1, result1, init2, request3, result2) = (after(&lines, "request", 1), after(&lines, "result", 1), after(&lines, "init", 2), after(&lines, "request", 3), after(&lines, "result", 2));
    let splices = |mind: &Rc<TestMind>| vec![(request1, vec![briefed(mind, 31, vec![libby("l1")])]), (result1, vec![Idle(31)]), (init2, vec![Idle(31)]), (request3, vec![Idle(31)])];
    // Held: every window up to the notification turn's last line, with the turn still open.
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines[..result2 - 1], splices(&mind)));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, "@libby:chat") && !written_with(&ran, &id_of("l1")), "no line of Libby's reaches Aqua's task turns: {:?}", ran.written());
    // The notification turn ends: the session is idle and Libby's brief is told on its own line; the task's result was Aqua's.
    let mind = serving_aqua();
    let mut steps = splices(&mind);
    steps.push((result2, vec![Idle(31), Out(echo()), Out(saying("libby done"))]));
    let ran = mind_run(&mind, script(&lines, steps));
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 1, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("l1")) && !told[0].contains(&id_of("a1")), "{}", told[0]);
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("a1")], vec![id_of("l1")]));
}

/// Libby is served and idle; Aqua's brief switches the session; Aqua's turn starts a task. Its notification turn belongs to Aqua, not to the
/// conversation answered before the switch.
#[test]
fn a_background_tasks_result_reaches_the_conversation_that_started_it_after_another_conversation_was_served() {
    let mind = live_mind();
    persona_commit(&mind, vec![libby("l1")]);
    let steps = [vec![Out(echo()), Out(saying("libby done")), briefed(&mind, 31, vec![aqua("a1")])], script(&stream(BG_IDLE), vec![])].concat();
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    assert_eq!(answered(&mind, 1), vec![id_of("l1")]);
    assert_eq!(answered(&mind, 2), vec![id_of("a1")], "the turn that started the task");
    assert_eq!(answered(&mind, 3), vec![id_of("a1")], "the notification turn: never none, never Libby's");
}

/// The CLI replays the notification to the model in the running turn (a user line starting <task-notification>) after the ceiling flush was
/// written: the replay is no echo of ours, so the flush is not taken by it and its result is not mistaken for the flush's.
#[test]
fn a_replayed_task_notification_is_not_our_echo() {
    let lines = stream(BG_FOLD);
    let ceiling = after(&lines, "request", 2);
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines, vec![(ceiling, vec![Out(at("ceiling", CEILING_TOKENS))])]));
    assert!(ran.outcome.is_ok());
    assert_eq!(ran.written().len(), 2, "the start line and the flush: {:?}", ran.written());
    assert_eq!(flushes(&ran), 1);
    assert_eq!(ran.count(&Event::InputClosed), 0, "the flush has not been taken: the replay is the CLI's line, not its echo");
    assert_eq!(answered(&mind, 1), vec![id_of("a1")], "Aqua's answer is a reply, not the flush's note");
    assert!(reply_of(&mind, 2).is_none());
    // The flush's own echo takes it; its result is the note to the successor and is no reply.
    let mind = serving_aqua();
    let ran = mind_run(&mind, [script(&lines, vec![(ceiling, vec![Out(at("ceiling", CEILING_TOKENS))])]), vec![Out(echo()), Out(saying("note to my successor"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(ran.count(&Event::InputClosed), 1);
    assert_eq!(answered(&mind, 1), vec![id_of("a1")]);
    assert!(reply_of(&mind, 2).is_none(), "the note is in the journal only");
}

/// The notification turn runs a tool, so a line written into it is folded: the served conversation's brief joins it and its result answers
/// that brief; another conversation's brief, briefed at the same point, waits for idle.
#[test]
fn the_served_conversations_brief_joins_the_clis_notification_turn_and_another_waits() {
    let lines = stream(BG_OTHER);
    let request3 = after(&lines, "request", 3);
    let mind = serving_aqua();
    let held = script(&lines[..lines.len() - 1], vec![(request3, vec![briefed(&mind, 31, vec![aqua("a2"), libby("l1")])])]);
    let ran = mind_run(&mind, held);
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    assert!(tells(&ran)[0].contains(&id_of("a2")) && !written_with(&ran, &id_of("l1")), "{:?}", ran.written());
    assert_eq!(answered(&mind, 1), vec![id_of("a1")]);
    let mind = serving_aqua();
    let steps = script(&lines, vec![(request3, vec![briefed(&mind, 31, vec![aqua("a2"), libby("l1")])]), (lines.len(), vec![Idle(31), Out(echo()), Out(saying("libby done"))])]);
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 2, "{:?}", ran.written());
    assert!(told[1].contains(&id_of("l1")) && !told[1].contains(&id_of("a2")), "{}", told[1]);
    assert_eq!(answered(&mind, 2), vec![id_of("a2")], "the folded brief's result answers it");
    assert_eq!(answered(&mind, 3), vec![id_of("l1")]);
}

/// A brief arrives while the CLI's notification turn (no tool) runs: written into it, it would be queued behind and answered by the wrong
/// result. It is written after the turn's result.
#[test]
fn another_conversations_brief_waits_behind_a_notification_turn_with_no_tool() {
    let lines = stream(BG_RACE);
    let (init2, request3, result2) = (after(&lines, "init", 2), after(&lines, "request", 3), after(&lines, "result", 2));
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines[..result2 - 1], vec![(init2, vec![briefed(&mind, 31, vec![libby("l1")])]), (request3, vec![Idle(31)])]));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, &id_of("l1")) && !written_with(&ran, "@libby:chat"), "{:?}", ran.written());
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines, vec![(init2, vec![briefed(&mind, 31, vec![libby("l1")])]), (request3, vec![Idle(31)]), (result2, vec![Idle(31)])]));
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    assert_eq!((answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("l1")]), "the notification turn is Aqua's; Libby's brief is answered by its own result");
}

/// A task that was never started backgrounded, and a notification of a task never seen, owe nothing: no wedge.
#[test]
fn a_notification_of_a_foreground_or_unknown_task_holds_nothing() {
    // The capture's task is foreground now: not announced backgrounded and in no level.
    let lines: Vec<String> = stream(BG_IDLE).into_iter().map(|line| line.replace("\"is_backgrounded\":true", "\"is_backgrounded\":false")).filter(|line| label_of(line) != "level").collect();
    assert!(lines.iter().all(|line| !matches!(read_usage(line), Usage::TaskBackgrounded { .. } | Usage::BackgroundTasks { .. })), "the capture's task is foreground now");
    let (request1, result1) = (after(&lines, "request", 1), after(&lines, "result", 1));
    let mind = serving_aqua();
    let steps = script(&lines[..result1], vec![(request1, vec![briefed(&mind, 31, vec![libby("l1")])])]);
    let ran = mind_run(&mind, [steps, vec![Out(notification_of("unseen")), Idle(31), Out(echo()), Out(saying("libby done"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "the first idle tells Libby's brief: {:?}", ran.written());
    assert_eq!(answered(&mind, 2), vec![id_of("l1")]);
}

/// A task that never ends holds other conversations; the rotation flush is the bound and never switches.
#[test]
fn a_backgrounded_task_that_is_never_notified_holds_other_conversations_to_the_rotation_flush_only() {
    let lines = stream(BG_LATE);
    let (request1, result1) = (after(&lines, "request", 1), after(&lines, "result", 1));
    let mind = serving_aqua();
    let steps = script(&lines[..result1], vec![(request1, vec![briefed(&mind, 31, vec![libby("l1")])])]);
    let rotation = i64::try_from(TEST_CAP_S).unwrap() - ROTATE_MARGIN_S;
    let ran = mind_run(&mind, [steps, vec![Idle(60), Idle(rotation), Out(echo()), Out(saying("note to my successor"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(flushes(&ran), 1, "{:?}", ran.written());
    assert!(!written_with(&ran, &id_of("l1")), "the flush never carries another conversation's brief: {:?}", ran.written());
    assert_eq!(ran.count(&Event::InputClosed), 1, "the flush's result closes stdin");
    assert_eq!(answered(&mind, 1), vec![id_of("a1")]);
}

/// A task of the main session that the CLI moved to the background at its timeout is in the level from then on, with no `task_started` that says
/// so. Libby's brief, derived after Aqua's turn ended, is written into none of the CLI's turn for the task: not while the level holds it, and
/// not in the gap after the level empties and before the CLI's init line; only after that turn's result, which was Aqua's.
#[test]
fn a_task_moved_to_the_background_at_its_timeout_holds_other_conversations_until_the_clis_turn_for_it_ends() {
    let lines = stream(BG_FGTO);
    let (result1, level2, init2, request3, result2) = (after(&lines, "result", 1), after(&lines, "level", 2), after(&lines, "init", 2), after(&lines, "request", 3), after(&lines, "result", 2));
    let splices = |mind: &Rc<TestMind>| vec![(result1, vec![briefed(mind, 31, vec![libby("l1")]), Idle(31)]), (level2, vec![Idle(31)]), (init2, vec![Idle(31)]), (request3, vec![Idle(31)])];
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines[..result2 - 1], splices(&mind)));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, "@libby:chat") && !written_with(&ran, &id_of("l1")), "no line of Libby's reaches the CLI's turn for Aqua's task: {:?}", ran.written());
    let mind = serving_aqua();
    let mut steps = splices(&mind);
    steps.push((result2, vec![Idle(31), Out(echo()), Out(saying("libby done"))]));
    let ran = mind_run(&mind, script(&lines, steps));
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 1, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("l1")) && !told[0].contains(&id_of("a1")), "{}", told[0]);
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("a1")], vec![id_of("l1")]));
}

/// A task the session stops with TaskStop is notified inside the turn that stops it and the CLI gives it no turn: the next conversation is
/// told at the first idle after that turn's result.
#[test]
fn a_task_the_session_stops_with_taskstop_owes_no_turn() {
    let lines = stream(BG_KILL);
    let (request1, result1) = (after(&lines, "request", 1), after(&lines, "result", 1));
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines, vec![(request1, vec![briefed(&mind, 31, vec![libby("l1")])]), (result1, vec![Idle(31), Out(echo()), Out(saying("libby done"))])]));
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 1, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("l1")) && !told[0].contains(&id_of("a1")), "{}", told[0]);
    assert_eq!(ran.count(&Event::InputClosed), 0);
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("a1")], vec![id_of("l1")]), "no CLI turn was awaited");
}

/// The same stream with the stop made by something else: the TaskStop call is renamed, so the stop was not the session's and the CLI gives the
/// notification its turn, which follows the result.
#[test]
fn a_stop_the_session_did_not_ask_for_owes_its_turn() {
    let lines: Vec<String> = stream(BG_KILL).into_iter().map(|line| line.replace("\"name\":\"TaskStop\"", "\"name\":\"Other\"")).collect();
    assert!(lines.iter().all(|line| !matches!(read_usage(line), Usage::Request { stops, .. } if !stops.is_empty())), "no stop of ours is left in the capture");
    let (request1, result1) = (after(&lines, "request", 1), after(&lines, "result", 1));
    let cli_turn = |result: Vec<Step>| [vec![Idle(31), Out(init_line()), Out(at("c1", 10)), Idle(31)], result].concat();
    let splices = |mind: &Rc<TestMind>, rest: Vec<Step>| vec![(request1, vec![briefed(mind, 31, vec![libby("l1")])]), (result1, [vec![Idle(31)], cli_turn(rest)].concat())];
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines, splices(&mind, vec![])));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, "@libby:chat") && !written_with(&ran, &id_of("l1")), "the CLI's turn for the stopped task is running: {:?}", ran.written());
    let mind = serving_aqua();
    let ran = mind_run(&mind, script(&lines, splices(&mind, vec![Out(saying("the task was stopped")), Idle(31), Out(echo()), Out(saying("libby done"))])));
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("a1")], vec![id_of("l1")]), "the CLI's turn answers Aqua's brief, never Libby's");
}

/// A persistent Monitor stays in the level through several notice turns the CLI begins; Libby's brief waits through all of them and the final turn
/// of the notification.
#[test]
fn a_persistent_monitor_holds_other_conversations_through_its_notice_turns() {
    let lines = stream(BG_MONITOR);
    let result1 = after(&lines, "result", 1);
    let windows = |mind: &Rc<TestMind>, upto: usize| {
        let mut splices: Vec<(usize, Vec<Step>)> = (result1..=upto).map(|at| (at, vec![Idle(31)])).collect();
        splices[0].1.insert(0, briefed(mind, 31, vec![libby("l1")]));
        splices
    };
    let mind = serving_aqua();
    let last_result = lines.len();
    let ran = mind_run(&mind, script(&lines[..last_result - 1], windows(&mind, last_result - 1)));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, "@libby:chat") && !written_with(&ran, &id_of("l1")), "no line of Libby's reaches a notice turn: {:?}", ran.written());
    let mind = serving_aqua();
    let mut splices = windows(&mind, last_result - 1);
    splices.push((last_result, vec![Idle(31), Out(echo()), Out(saying("libby done"))]));
    let ran = mind_run(&mind, script(&lines, splices));
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "{:?}", ran.written());
    let results = lines.iter().filter(|line| label_of(line) == "result").count();
    for n in 1..=results {
        assert_eq!(answered(&mind, u32::try_from(n).unwrap()), vec![id_of("a1")], "reply {n}: the turns of the monitor are Aqua's");
    }
    assert_eq!(answered(&mind, u32::try_from(results + 1).unwrap()), vec![id_of("l1")]);
}

/// A subagent's or an awaited task is in the CLI's level and holds the session while it runs; its notification is no one's turn.
#[test]
fn a_subagents_or_an_awaited_tasks_notification_owes_nothing() {
    let level = |tasks: Value| json!({"type": "system", "subtype": "background_tasks_changed", "tasks": tasks}).to_string();
    let ended = |task: &str| json!({"type": "system", "subtype": "task_notification", "task_id": task, "status": "completed", "summary": "synthetic task"}).to_string();
    let both = || json!([{"task_id": "sub", "parent_task_id": "agent"}, {"task_id": "waited", "awaited": true}]);
    let mind = serving_aqua();
    let steps = vec![Out(echo()), Out(level(both())), Out(saying("aqua done")), briefed(&mind, 31, vec![libby("l1")]), Idle(31), Out(ended("sub"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, &id_of("l1")), "while the level holds them: {:?}", ran.written());
    let mind = serving_aqua();
    let steps = vec![Out(echo()), Out(level(both())), Out(saying("aqua done")), briefed(&mind, 31, vec![libby("l1")]), Idle(31), Out(ended("sub")), Out(ended("waited")), Out(level(json!([]))), Idle(31), Out(echo()), Out(saying("libby done"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 1, "idle once the level is empty: {:?}", ran.written());
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("a1")], vec![id_of("l1")]));
}

// ---- who a result belongs to ------------------------------------------------

/// An error result makes no reply and leaves its briefs for the next result of the span, but never for another span.
#[test]
fn an_error_results_briefs_stay_in_their_own_span() {
    let common = |mind: &Rc<TestMind>| vec![Out(echo()), Out(saying("first")), briefed(mind, 31, vec![aqua("a2")]), Out(echo()), Out(failing())];
    let mind = serving_aqua();
    let ran = mind_run(&mind, [common(&mind), vec![Out(saying("second"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("a1")], vec![id_of("a2")]), "the failed turn's brief is answered by the next result of its span");
    let mind = serving_aqua();
    let ran = mind_run(&mind, [common(&mind), vec![briefed(&mind, 31, vec![libby("l1")]), Out(echo()), Out(saying("libby done"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 2, "{:?}", ran.written());
    assert_eq!(answered(&mind, 2), vec![id_of("l1")], "no turn of Libby's span answers Aqua's failed brief");
}

/// Libby, Mira and Aqua briefs waiting behind an Aqua turn: Aqua's own brief drops in at once, the others go oldest first at successive idles.
#[test]
fn at_idle_the_oldest_waiting_conversation_is_served_first() {
    let mind = serving_aqua();
    let steps = vec![
        Out(echo()),
        Out(at("m1", 10)),
        briefed(&mind, 31, vec![libby("l1")]),
        briefed(&mind, 31, vec![mira("m1")]),
        briefed(&mind, 31, vec![aqua("a2")]),
        Out(echo()),
        Out(saying("aqua done")),
        Idle(31),
        Out(echo()),
        Out(saying("libby done")),
        Idle(31),
        Out(echo()),
        Out(saying("mira done")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 3, "{:?}", ran.written());
    for (line, (own, others)) in told.iter().zip([("a2", ["l1", "m1"]), ("l1", ["a1", "m1"]), ("m1", ["a1", "l1"])]) {
        assert!(line.contains(&id_of(own)), "{line}");
        assert!(others.iter().all(|other| !line.contains(&id_of(other))), "one conversation per line: {line}");
    }
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1"), id_of("a2")], vec![id_of("l1")], vec![id_of("m1")]));
}

/// Aqua is served and idle; Libby's brief is the older of the two now waiting, Aqua's own the newer: at idle the oldest goes first, so Aqua's
/// brief does not outrun Libby's again and again.
#[test]
fn at_idle_an_older_brief_of_another_conversation_goes_before_the_served_ones_newer_brief() {
    let mind = serving_aqua();
    let steps = vec![Out(echo()), Out(saying("aqua done")), briefed(&mind, 0, vec![libby("l1")]), briefed(&mind, 31, vec![aqua("a2")]), Idle(31), Out(echo()), Out(saying("libby done")), Idle(31), Out(echo()), Out(saying("aqua again"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 2, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("l1")) && !told[0].contains(&id_of("a2")), "the older brief first: {}", told[0]);
    assert!(told[1].contains(&id_of("a2")) && !told[1].contains(&id_of("l1")), "{}", told[1]);
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("l1")], vec![id_of("a2")]));
}

/// A result that took in no brief answers the briefs of the span being served: never the span before.
#[test]
fn every_result_of_a_span_answers_the_spans_briefs() {
    let mind = live_mind();
    persona_commit(&mind, vec![libby("l1")]);
    let ran = mind_run(&mind, vec![Out(echo()), Out(saying("libby done")), briefed(&mind, 31, vec![aqua("a1")]), Out(echo()), Out(saying("aqua done")), Out(saying("a task of aqua's finished")), Out(saying("another task of aqua's"))]);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 1), vec![id_of("l1")]);
    for n in 2..=4 {
        assert_eq!(answered(&mind, n), vec![id_of("a1")], "reply {n}");
    }
    // A start that serves no one: its results answer none, and her Persona tells them in the public session.
    let mind = live_mind();
    let ran = mind_run(&mind, vec![Out(echo()), Out(saying("a wake of its own")), Out(saying("a task finished"))]);
    assert!(ran.outcome.is_ok());
    assert!(answered(&mind, 1).is_empty() && answered(&mind, 2).is_empty());
}

/// The same conversation's new brief is written at once, whatever runs; another conversation's waits for the turn to end.
#[test]
fn the_served_conversations_briefs_drop_in_mid_turn_and_another_conversations_wait() {
    let mind = serving_aqua();
    let steps = vec![Out(echo()), Out(at("m1", 10)), briefed(&mind, 31, vec![aqua("a2")]), Out(echo()), briefed(&mind, 31, vec![libby("l1")]), Idle(31), Out(saying("both")), Idle(31), Out(echo()), Out(saying("libby done"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    let told = tells(&ran);
    assert_eq!(told.len(), 2, "{:?}", ran.written());
    assert!(told[0].contains(&id_of("a2")) && told[1].contains(&id_of("l1")), "{told:?}");
    assert_eq!((answered(&mind, 1), answered(&mind, 2)), (vec![id_of("a1"), id_of("a2")], vec![id_of("l1")]));
}

/// The flush result is the session's note to its successor: it answers no brief, though one conversation was told and a notification would continue it.
#[test]
fn the_result_of_the_flush_answers_no_brief() {
    let mind = live_mind();
    persona_commit(&mind, vec![aqua("a1")]);
    let steps = vec![Out(echo()), Out(at("m1", CEILING_TOKENS)), Out(saying("aqua work")), Out(echo()), Out(saying("note to my successor"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 1), vec![id_of("a1")]);
    assert!(reply_of(&mind, 2).is_none(), "the flush is the note to the successor, kept in the journal: it is no reply, so nothing points at it");
}

/// The start message states the serialization her Self works under, in place of the routing rules of the earlier rulings.
#[test]
fn the_start_message_states_one_conversation_at_a_time_and_what_holds_it() {
    let text = start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[], 0);
    for rule in ["one conversation at a time", "only once you are idle", "in the conversation you are serving", "a long background task keeps other members waiting"] {
        assert!(text.contains(rule), "{rule}: {text}");
    }
    for gone in ["pointer", "continues the one before", "the conversation of the turn that started it"] {
        assert!(!text.contains(gone), "{gone}: {text}");
    }
}

/// A switch begins a span of its own: a result that took in no brief, before any reply of the span, answers the briefs of the line that began
/// it and never the span before.
#[test]
fn a_switch_seeds_its_span_with_its_own_briefs_so_no_result_inherits_the_span_before() {
    use crate::self_run::session::SessionInbox;
    let mind = live_mind();
    persona_commit(&mind, vec![libby("l1")]);
    let mut told = inbox::derive(&*mind, &HashSet::new()).unwrap();
    let conversation = told.first().map(|brief| brief.conversation.clone()).unwrap();
    let (run, claims) = her_run(&*mind).unwrap().unwrap();
    let mut heard = Heard::default();
    let mut live = SelfInbox::new(&*mind, (&run, &claims), "", (&[], 0), (&mut told, conversation), &mut heard);
    live.echoed();
    live.result(&saying("libby done"), true, false).unwrap();
    persona_commit(&mind, vec![aqua("a1")]);
    let polled = live.poll().unwrap();
    assert!(!polled.pending && polled.other, "Aqua's brief is another conversation's");
    live.tell(true).unwrap();
    live.result(&saying("a turn that took in no brief of Aqua's line"), true, false).unwrap();
    assert_eq!(answered(&mind, 1), vec![id_of("l1")]);
    assert_eq!(answered(&mind, 2), vec![id_of("a1")], "the span's own briefs, not Libby's");
}
