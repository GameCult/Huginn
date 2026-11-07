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
    assert!(answered(&mind, 2).is_empty(), "the flush is never told as a reply");
}

/// A restart whose claimed brief was resolved while another conversation's brief is open: the start line holds the open brief only.
#[test]
fn a_restart_with_a_resolved_claim_puts_one_conversation_on_the_start_line() {
    let mind = TestMind::seeded();
    mind.committed(vec![spec("x"), run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    persona_commit(&mind, vec![aqua("a1")]);
    persona_commit(&mind, vec![libby("l1")]);
    mind.committed(vec![run("selfrun-a", RunTurn::SelfRun, RunOperator::Mind, &[brief_ref("a1")])]);
    mind.committed(vec![close(brief_ref("a1"), recorded())]);
    let ran = carry_with(&Seen::of(&mind), vec![Out(echo()), Out(saying("libby done"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let start = &ran.written()[0];
    assert!(start.contains(&id_of("l1")) && !start.contains(&id_of("a1")), "{start}");
    assert_eq!(answered(&mind, 1), vec![id_of("l1")], "told in the right conversation, none stranded");
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
