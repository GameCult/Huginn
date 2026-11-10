//! What her Self said: each main-thread result with words is one reply in her mind, answering the briefs the CLI took since the last one.

use eureka_pipeline::PipelineReply;

use super::*;

/// A turn that ended well, saying `text`.
fn saying(text: &str) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "terminal_reason": "completed", "result": text}).to_string()
}

fn reply_of(mind: &TestMind, n: u32) -> Option<PipelineReply> {
    match mind.view(K::Reply, &format!("{INSTANCE}:reply:selfrun-a-r{n}"))?.document {
        PipelineDocument::Reply(reply) => Some(reply),
        _ => None,
    }
}

fn answered(reply: &PipelineReply) -> Vec<String> {
    reply.answers.iter().map(|brief| brief.id.0.clone()).collect()
}

fn text_of(mind: &TestMind, n: u32) -> String {
    reply_of(mind, n).unwrap_or_else(|| panic!("reply {n} was not admitted")).text.0
}

#[test]
fn each_result_with_text_is_one_reply_in_order_answering_the_briefs_echoed_since_the_previous_result() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(at("m1", 10)),
        briefed(&mind, 31, vec![brief("b1", "Survey the noise.")]),
        Out(echo()),
        Out(saying("first")),
        Out(saying("second")),
        briefed(&mind, 31, vec![brief("b2", "Wire the uniforms.")]),
        Out(echo()),
        Out(saying("third")),
    ];
    let ran = mind_run(&mind, steps);
    assert!(ran.outcome.is_ok());
    assert_eq!(mind.ids(K::Reply).len(), 3);
    let (r1, r2, r3) = (reply_of(&mind, 1).unwrap(), reply_of(&mind, 2).unwrap(), reply_of(&mind, 3).unwrap());
    assert_eq!((r1.text.0.as_str(), r2.text.0.as_str(), r3.text.0.as_str()), ("first", "second", "third"));
    assert_eq!(answered(&r1), vec![brief_ref("b1").id.0], "the brief whose line the CLI took before the first result");
    assert_eq!(answered(&r2), vec![brief_ref("b1").id.0], "no line was taken between the first result and the second: it continues the turn before");
    assert_eq!(answered(&r3), vec![brief_ref("b2").id.0]);
    assert!(r1.run == run_ref("selfrun-a") && r1.instance.0 == INSTANCE);
    assert!(resolutions_of(&mind, &brief_ref("b1")).is_empty() && resolutions_of(&mind, &brief_ref("b2")).is_empty(), "the actuator never resolves a brief");
}

#[test]
fn lines_folded_into_one_echo_are_answered_by_one_reply() {
    let mind = live_mind();
    let steps = vec![
        Out(echo()),
        Out(at("m1", 10)),
        // Two briefs derived at one poll are one line, one echo, one answer.
        briefed(&mind, 31, vec![brief("b1", "Survey the noise."), brief("b2", "Wire the uniforms.")]),
        Out(echo()),
        Out(saying("both")),
    ];
    mind_run(&mind, steps).outcome.unwrap();
    assert_eq!(mind.ids(K::Reply).len(), 1);
    let only = reply_of(&mind, 1).unwrap();
    assert_eq!(answered(&only), vec![brief_ref("b1").id.0, brief_ref("b2").id.0], "in tell order");
}

#[test]
fn the_start_lines_briefs_are_answered_by_the_first_reply() {
    let mind = live_mind();
    persona_commit(&mind, vec![brief("b0", "Open at the start.")]);
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(saying("done"))];
    mind_run(&mind, steps).outcome.unwrap();
    assert_eq!(answered(&reply_of(&mind, 1).unwrap()), vec![brief_ref("b0").id.0], "the start line carried it");
}

#[test]
fn an_error_or_empty_result_makes_no_reply_and_leaves_its_briefs_for_the_next() {
    let mind = live_mind();
    persona_commit(&mind, vec![brief("b0", "Open at the start.")]);
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(saying("")), Out(saying("  \n")), Out(saying("now")), Out(result_line())];
    mind_run(&mind, steps).outcome.unwrap();
    assert_eq!(mind.ids(K::Reply).len(), 1, "{:?}", mind.ids(K::Reply));
    assert_eq!(answered(&reply_of(&mind, 1).unwrap()), vec![brief_ref("b0").id.0], "the silent turns answered nothing, so the brief waits for the first words");
}

#[test]
fn a_refused_admission_is_retried_at_the_next_result_and_the_session_does_not_end_for_it() {
    let mind = live_mind();
    let seen = Seen::of(&mind);
    seen.fail_admits.set(1);
    let steps = vec![Out(echo()), Out(at("m1", 10)), Out(saying("one")), briefed(&mind, 31, vec![brief("b1", "Survey the noise.")]), Out(echo()), Out(saying("two"))];
    let ran = carry_with(&seen, steps, Some(0), false);
    assert_eq!(ran.ended().0, Cause::Own, "{:?}", ran.outcome);
    assert_eq!((text_of(&mind, 1).as_str(), text_of(&mind, 2).as_str()), ("one", "two"), "no reply lost to one failed admission, and in order");
}

#[test]
fn a_reply_the_mind_did_not_take_is_tried_once_more_when_the_run_ends() {
    let mind = live_mind();
    let seen = Seen::of(&mind);
    seen.fail_admits.set(1);
    let ran = carry_with(&seen, vec![Out(echo()), Out(at("m1", 10)), Out(saying("last words"))], Some(0), false);
    assert!(ran.outcome.is_ok());
    assert_eq!(text_of(&mind, 1), "last words");
}

#[test]
fn a_refused_piece_of_a_long_result_holds_back_the_pieces_behind_it() {
    let mind = live_mind();
    let seen = Seen::of(&mind);
    seen.fail_admits.set(1);                                   // the first piece is refused
    let long = "w".repeat(REPLY_TEXT_LIMIT * 2 + 1);
    let ran = carry_with(&seen, vec![Out(echo()), Out(at("m1", 10)), Out(saying(&long))], Some(0), false);
    assert!(ran.outcome.is_ok());
    let ordinals: Vec<u64> = (1..=3).map(|n| mind.view(K::Reply, &format!("{INSTANCE}:reply:selfrun-a-r{n}")).unwrap_or_else(|| panic!("piece {n} lost")).admission.ordinal).collect();
    assert!(ordinals.windows(2).all(|w| w[0] < w[1]), "the pieces reached the mind out of order: {ordinals:?}");
}
