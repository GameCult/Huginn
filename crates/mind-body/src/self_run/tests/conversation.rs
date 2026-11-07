//! One Self turn answers one conversation: her Persona tells a reply only in the conversation of the briefs it answers, so the actuator
//! never writes a line holding two conversations' briefs while the first's are unanswered.

use eureka_pipeline::{MemoryOrigin, PipelineReply};

use super::*;

fn saying(text: &str) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "terminal_reason": "completed", "result": text}).to_string()
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

fn id_of(label: &str) -> String {
    brief_ref(label).id.0
}

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

/// Two conversations' briefs derived at one poll, and across polls while the first's are unanswered: the gate tells one conversation per
/// line and holds the other until a reply has answered.
#[test]
fn a_live_self_never_writes_a_line_holding_a_second_conversations_briefs_while_the_first_are_unanswered() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(at("m1", 10)),
        // The folded shape: both derived at one poll.
        briefed(&mind, 31, vec![aqua("a1"), libby("l1")]),
        Out(echo()),
        // Across polls, while the turn that took a1 runs: a third conversation's brief arrives too.
        briefed(&mind, 31, vec![brief_in("m1", "!mira:chat", "@mira:chat")]),
        Idle(31),
        Idle(31),
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
    for (line, (own, others)) in told.iter().zip([("a1", ["l1", "m1"]), ("l1", ["a1", "m1"]), ("m1", ["a1", "l1"])]) {
        assert!(line.contains(&id_of(own)), "{line}");
        assert!(others.iter().all(|other| !line.contains(&id_of(other))), "one conversation per line: {line}");
    }
    assert_eq!((answered(&mind, 1), answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("l1")], vec![id_of("m1")]));
}

#[test]
fn the_same_conversations_briefs_are_still_told_while_its_turn_is_unanswered() {
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(at("m1", 10)), briefed(&mind, 31, vec![aqua("a1")]), Out(echo()), briefed(&mind, 31, vec![aqua("a2")]), Out(echo()), Out(saying("both"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(tells(&ran).len(), 2, "{:?}", ran.written());
    assert_eq!(answered(&mind, 1), vec![id_of("a1"), id_of("a2")]);
}

/// A task notification is a turn that took in no brief: while the run has told one conversation only, it continues the turn before.
#[test]
fn a_result_that_took_in_no_brief_answers_the_previous_replys_briefs() {
    let mind = live_mind();
    let steps = vec![Out(echo()), Out(saying("nothing yet")), briefed(&mind, 31, vec![aqua("a1")]), Out(echo()), Out(saying("first")), Out(saying("after a notification"))];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert!(answered(&mind, 1).is_empty(), "a run's first reply with nothing echoed answers none");
    assert_eq!(answered(&mind, 2), vec![id_of("a1")]);
    assert_eq!(answered(&mind, 3), vec![id_of("a1")], "continues the turn before");
}

/// The actuator cannot tell whose work a notification is, so once the run has told two conversations a result that took in no brief
/// answers nothing (her Persona tells such a reply in no session).
#[test]
fn a_result_that_took_in_no_brief_answers_nothing_once_two_conversations_were_told() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(saying("nothing yet")),
        briefed(&mind, 31, vec![aqua("a1")]),
        Out(echo()),
        Out(saying("aqua turn: started a background task")),
        briefed(&mind, 31, vec![libby("l1")]),
        Out(echo()),
        Out(saying("libby turn")),
        Out(saying("the background task for aqua finished")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 2), vec![id_of("a1")]);
    assert_eq!(answered(&mind, 3), vec![id_of("l1")]);
    assert!(answered(&mind, 4).is_empty(), "the notification is told in neither conversation");
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

/// A result arrives while a line of its conversation is written and not yet echoed, after another conversation's brief was told first:
/// the gate holds on the written line's conversation, so the other conversation's brief waits until that line is answered.
#[test]
fn the_gate_holds_across_a_result_while_a_line_of_its_conversation_is_unechoed() {
    use crate::self_run::session::SessionInbox;
    let mind = live_mind();
    persona_commit(&mind, vec![libby("l0")]);
    let mut told = inbox::derive(&*mind, &HashSet::new()).unwrap();
    let conversation = told.first().map(|brief| brief.conversation.clone());
    let (run, claims) = her_run(&*mind).unwrap().unwrap();
    let mut heard = Heard::default();
    let mut live = SelfInbox::new(&*mind, (&run, &claims), "", (&[], 0), (&mut told, conversation), &mut heard);
    live.echoed();
    live.result(&saying("l0 done"), true, false).unwrap();
    persona_commit(&mind, vec![aqua("a1")]);
    assert!(live.poll().unwrap().pending);
    live.tell().unwrap();
    live.echoed();
    persona_commit(&mind, vec![aqua("a2")]);
    assert!(live.poll().unwrap().pending);
    live.tell().unwrap();
    live.result(&saying("a1 done"), true, false).unwrap();
    persona_commit(&mind, vec![libby("l2")]);
    assert!(!live.poll().unwrap().pending, "a2 is written and unanswered: another conversation's brief waits");
    live.echoed();
    live.result(&saying("a2 done"), true, false).unwrap();
    assert!(live.poll().unwrap().pending, "a2 is answered: the gate is free");
}

fn task_started(tool_use: &str) -> String {
    json!({"type": "system", "subtype": "task_started", "task_id": "bg1", "tool_use_id": tool_use, "description": "synthetic task", "task_type": "local_bash"}).to_string()
}

fn task_notification(tool_use: &str) -> String {
    json!({"type": "system", "subtype": "task_notification", "task_id": "bg1", "tool_use_id": tool_use, "status": "completed", "summary": "synthetic task"}).to_string()
}

fn written_with(ran: &Ran, id: &str) -> bool {
    ran.written().iter().any(|line| line.contains(id))
}

/// A task notification's result is attributed by the `tool_use_id` the stream carries to the turn that started the task, not to the
/// conversation answered last (stream shape: the `task_started` and `task_notification` system events of the 2.1.312 captures).
#[test]
fn a_notifications_result_answers_the_briefs_of_the_turn_that_started_its_task() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(saying("nothing yet")),
        briefed(&mind, 31, vec![aqua("a1")]),
        Out(echo()),
        Out(task_started("toolu_aqua")),
        Out(saying("aqua turn: started a task")),
        briefed(&mind, 31, vec![libby("l1")]),
        Out(echo()),
        Out(task_started("toolu_libby")),
        Out(saying("libby turn: started a task")),
        Out(task_notification("toolu_aqua")),
        Out(saying("the aqua task finished")),
        Out(task_notification("toolu_libby")),
        Out(saying("the libby task finished")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!((answered(&mind, 2), answered(&mind, 3)), (vec![id_of("a1")], vec![id_of("l1")]));
    assert_eq!(answered(&mind, 4), vec![id_of("a1")], "the aqua task's result is aqua's, though libby was answered last");
    assert_eq!(answered(&mind, 5), vec![id_of("l1")]);
}

/// A notification the stream gives no start for, in a run that told two conversations, cannot be placed: it answers nothing.
#[test]
fn a_notification_with_no_known_task_answers_nothing_once_two_conversations_were_told() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(saying("nothing yet")),
        briefed(&mind, 31, vec![aqua("a1")]),
        Out(echo()),
        Out(saying("aqua turn")),
        briefed(&mind, 31, vec![libby("l1")]),
        Out(echo()),
        Out(saying("libby turn")),
        Out(task_notification("toolu_unknown")),
        Out(saying("some task finished")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert!(answered(&mind, 4).is_empty());
}

/// While a notification's turn is open no other conversation's brief is written into it: it waits for the turn's result.
#[test]
fn another_conversations_brief_waits_for_an_open_notification_turn() {
    let before = |mind: &Rc<TestMind>| {
        vec![
            Out(echo()),
            Out(saying("nothing yet")),
            briefed(mind, 31, vec![aqua("a1")]),
            Out(echo()),
            Out(task_started("toolu_aqua")),
            Out(saying("aqua turn: started a task")),
            Out(task_notification("toolu_aqua")),
            briefed(mind, 31, vec![libby("l1")]),
            Idle(31),
        ]
    };
    let mind = live_mind();
    let ran = mind_run(&mind, before(&mind));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, &id_of("l1")), "the open notification turn is aqua's: {:?}", ran.written());

    let mind = live_mind();
    let steps = [before(&mind), vec![Out(saying("the aqua task finished")), Idle(31), Out(echo()), Out(saying("libby done"))]].concat();
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 3), vec![id_of("a1")], "the outcome is aqua's");
    assert_eq!(answered(&mind, 4), vec![id_of("l1")], "libby's brief is told after, on its own line");
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

/// A turn that took in no brief is running (a request with nothing echoed) and the stream gave no notification event: it continues the turn
/// before, so another conversation's brief is not written into it.
#[test]
fn another_conversations_brief_waits_for_any_turn_that_took_in_no_brief() {
    let before = |mind: &Rc<TestMind>| {
        vec![
            Out(echo()),
            Out(saying("nothing yet")),
            briefed(mind, 31, vec![aqua("a1")]),
            Out(echo()),
            Out(saying("aqua turn: started a background task")),
            Out(at("n1", 10)),
            briefed(mind, 31, vec![libby("l1")]),
            Idle(31),
        ]
    };
    let mind = live_mind();
    let ran = mind_run(&mind, before(&mind));
    assert!(ran.outcome.is_ok());
    assert!(!written_with(&ran, &id_of("l1")), "{:?}", ran.written());

    let mind = live_mind();
    let ran = mind_run(&mind, [before(&mind), vec![Out(saying("SYNTHETIC aqua task outcome")), Idle(31), Out(echo()), Out(saying("libby done"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 3), vec![id_of("a1")], "the outcome continues aqua's turn, not libby's");
    assert_eq!(answered(&mind, 4), vec![id_of("l1")]);
}

/// The start message states the routing rules her Self works under, in place of the old continuation sentence.
#[test]
fn the_start_message_states_the_routing_rules() {
    let text = start_message(&Slug("yggdrasil".into()), &run_ref("selfrun-a"), &[], 0);
    for rule in ["one conversation at a time", "the conversation of the turn that started it", "no other conversation's brief is told into that turn", "pointer to it by id, without its text"] {
        assert!(text.contains(rule), "{rule}: {text}");
    }
    assert!(!text.contains("continues the one before"));
}

/// The brief steps up to a notification of an aqua task: aqua's turn started the task and ended, then the notification arrived.
fn after_aqua_started_a_task(mind: &Rc<TestMind>) -> Vec<Step> {
    vec![Out(echo()), Out(saying("nothing yet")), briefed(mind, 31, vec![aqua("a1")]), Out(echo()), Out(task_started("toolu_aqua")), Out(saying("aqua turn: started a task"))]
}

/// A brief of the notification's own conversation joins its turn, and takes the notification in with it: the result answers that brief, the
/// notification is no longer pending, and another conversation's brief is not starved.
#[test]
fn the_notified_conversations_brief_joins_its_turn_and_takes_the_notification_in() {
    let steps = |mind: &Rc<TestMind>| [after_aqua_started_a_task(mind), vec![Out(task_notification("toolu_aqua")), briefed(mind, 31, vec![aqua("a2")]), Idle(31)]].concat();
    let mind = live_mind();
    let ran = mind_run(&mind, steps(&mind));
    assert!(written_with(&ran, &id_of("a2")), "the notification's own conversation may be told into its turn: {:?}", ran.written());

    let mind = live_mind();
    let ran = mind_run(&mind, [steps(&mind), vec![Out(echo()), Out(saying("outcome and a2")), briefed(&mind, 31, vec![libby("l1")]), Idle(31), Out(echo()), Out(saying("libby done"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 3), vec![id_of("a2")]);
    assert_eq!(answered(&mind, 4), vec![id_of("l1")], "the notification was taken in, so libby's brief is told");
}

/// A notification of aqua's task arrives while libby's turn runs: libby's result does not answer it, so the turn that follows is still aqua's.
#[test]
fn a_notification_of_another_conversation_stays_pending_through_a_turn_that_took_in_briefs() {
    let mind = live_mind();
    let steps = [
        after_aqua_started_a_task(&mind),
        vec![briefed(&mind, 31, vec![libby("l1")]), Out(echo()), Out(task_notification("toolu_aqua")), Out(saying("libby turn")), Out(at("n1", 10)), Out(saying("the aqua task finished"))],
    ]
    .concat();
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 3), vec![id_of("l1")]);
    assert_eq!(answered(&mind, 4), vec![id_of("a1")], "the notification turn is aqua's, though libby was answered last");
}

/// Notifications of two conversations' tasks pending together cannot be placed: no brief is written into their turn and the result answers both, which her Persona places in no conversation.
#[test]
fn notifications_of_two_conversations_open_a_turn_no_brief_may_join() {
    let steps = |mind: &Rc<TestMind>| {
        vec![
            Out(echo()),
            Out(saying("nothing yet")),
            briefed(mind, 31, vec![aqua("a1")]),
            Out(echo()),
            Out(task_started("toolu_aqua")),
            Out(saying("aqua turn")),
            briefed(mind, 31, vec![libby("l1")]),
            Out(echo()),
            Out(task_started("toolu_libby")),
            Out(saying("libby turn")),
            Out(task_notification("toolu_aqua")),
            Out(task_notification("toolu_libby")),
            briefed(mind, 31, vec![aqua("a2")]),
            Idle(31),
        ]
    };
    let mind = live_mind();
    let ran = mind_run(&mind, steps(&mind));
    assert!(!written_with(&ran, &id_of("a2")), "{:?}", ran.written());

    let mind = live_mind();
    let ran = mind_run(&mind, [steps(&mind), vec![Out(saying("both tasks finished")), Idle(31), Out(echo()), Out(saying("a2 done"))]].concat());
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 4), vec![id_of("a1"), id_of("l1")], "two conversations' work in one turn answers both: her Persona places it in neither and points at it");
    assert_eq!(answered(&mind, 5), vec![id_of("a2")]);
}

/// A line that is no request, between turns, opens no turn: the next conversation's brief is told at once.
#[test]
fn a_line_between_turns_that_is_no_request_opens_no_turn() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(saying("nothing yet")),
        briefed(&mind, 31, vec![aqua("a1")]),
        Out(echo()),
        Out(saying("aqua done")),
        Out(json!({"type": "system", "subtype": "status", "status": "idle"}).to_string()),
        briefed(&mind, 31, vec![libby("l1")]),
        Idle(31),
        Out(echo()),
        Out(saying("libby done")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(answered(&mind, 3), vec![id_of("l1")]);
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
