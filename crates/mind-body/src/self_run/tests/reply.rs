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

// A canary stands where a forge token would: no byte of it may reach the journal or her mind.
const CANARY: &str = "CANARY-NOT-A-REAL-TOKEN-0000";

#[test]
fn url_userinfo_is_redacted_where_it_is_a_secret_and_kept_where_it_is_a_username() {
    let said = format!(
        "a https://user:{CANARY}@host.example/r.git b https://{CANARY}@host.example/r.git c ssh://u:{CANARY}@h/x d http://{CANARY}@h e HTTPS://x:{CANARY}@h \
         f https://u:p@ss{CANARY}@h/x g ssh://git@host.example/r.git h git://git@h/x i https://host.example/no-userinfo j (https://u:{CANARY}@h) k ://@x"
    );
    let want = "a https://<redacted>@host.example/r.git b https://<redacted>@host.example/r.git c ssh://<redacted>@h/x d http://<redacted>@h e HTTPS://<redacted>@h \
         f https://<redacted>@h/x g ssh://git@host.example/r.git h git://git@h/x i https://host.example/no-userinfo j (https://<redacted>@h) k ://@x";
    assert_eq!(redact_url_userinfo(&said), want);
    assert!(!redact_url_userinfo(&said).contains(CANARY));
}

#[test]
fn a_credential_in_a_url_reaches_neither_the_journal_nor_the_admitted_reply() {
    let mind = live_mind();
    let said = format!("origin https://user:{CANARY}@host.example/r.git and https://{CANARY}@host.example/s.git");
    let ran = mind_run(&mind, vec![Out(echo()), Out(saying(&said))]);
    assert!(ran.outcome.is_ok());
    assert!(!ran.out.contains(CANARY), "{}", ran.out);
    assert!(ran.out.contains("https://<redacted>@host.example/r.git"), "{}", ran.out);
    let text = text_of(&mind, 1);
    assert!(!text.contains(CANARY), "{text}");
    assert_eq!(text, "origin https://<redacted>@host.example/r.git and https://<redacted>@host.example/s.git");
}

#[test]
fn a_credential_split_by_the_reply_limit_is_redacted_before_the_cut() {
    let mind = live_mind();
    let said = format!("{}https://u:{CANARY}@h/x{}", "a".repeat(REPLY_TEXT_LIMIT - 25), "b".repeat(100));
    mind_run(&mind, vec![Out(echo()), Out(saying(&said))]).outcome.unwrap();
    let first = reply_of(&mind, 1).expect("the long result was admitted in pieces");
    assert!(reply_of(&mind, 2).is_some(), "the credential's place straddles the cut");
    assert!(!first.text.0.contains("CANARY") && !reply_of(&mind, 2).unwrap().text.0.contains("CANARY"));
}

#[test]
fn delimiters_inside_a_secret_leak_none_of_it_and_the_bare_and_scheme_less_forms_are_covered() {
    let r = redact_url_userinfo;
    for secret in ["pa/ssT", "pa?ssT", "pa#ssT", "pa\"ssT", "pa'ssT", "pa)ssT", "pa<ssT", "pa]ssT", "[u]"] {
        let out = r(&format!("x https://u:{secret}@h/x"));
        assert!(out.starts_with("x https://<redacted>@h"), "{secret:?} -> {out:?}");
        assert!(!out.contains("ssT"), "{secret:?} -> {out:?}");
    }
    // Whitespace of any kind ends a candidate run, so a secret holding it is cut there.
    assert_eq!(r("https://u:pa\u{a0}ssT@h/x"), "https://u:pa\u{a0}ssT@h/x");
    assert_eq!(r("HTTPS://TOKEN@h"), "HTTPS://<redacted>@h");
    assert_eq!(r("[l](https://T@h/x) <https://T@h> `https://u:T@h`"), "[l](https://<redacted>@h/x) <https://<redacted>@h> `https://<redacted>@h`");
    assert_eq!(r("a //u:T@host/x b //T@host c"), "a //<redacted>@host/x b //<redacted>@host c");
    assert_eq!(r("user:T@host.tld/path and a:b@c and mail me@x.org/ and https://h/a//b"), "<redacted>@host.tld/path and a:b@c and mail me@x.org/ and https://h/a//b");
    assert_eq!(r("ssh://git@host/r.git ssh://git@h:22/r"), "ssh://git@host/r.git ssh://git@h:22/r");
}

#[test]
fn multibyte_text_never_panics_the_redaction() {
    let r = redact_url_userinfo;
    for case in ["see \u{201c}https://h/x", "\u{2192}https://h/x", "docs\u{ff1a}https://h/x", "\u{e9}://x", "\u{65e5}\u{672c}://u:T@h", "https://x/\u{e9}://", "\u{e9}//u:T@h", "\u{e9}:u:T@h.tld/"] {
        let _ = r(case);
    }
    assert_eq!(r("\u{65e5}\u{672c}://u:T@h"), "\u{65e5}\u{672c}://<redacted>@h");
    assert_eq!(r("\u{2192}https://u:T@h/x"), "\u{2192}https://<redacted>@h/x");
    let seed = "\u{201c}\u{2192}\u{ff1a}\u{e9}\u{65e5}\u{a0}https://u:T@h/x//y:z@a.b/ ://@ //";
    let chars: Vec<char> = seed.chars().collect();
    for start in 0..=chars.len() {
        for end in start..=chars.len() {
            let _ = r(&chars[start..end].iter().collect::<String>());
        }
    }
}

#[test]
fn persona_and_sleep_structured_output_is_redacted_string_by_string() {
    let line = json!({"type": "result", "structured_output": {"say": format!("https://u:{CANARY}@h/x,"), "n": 3, "list": [format!("https://{CANARY}@h")]}}).to_string();
    let text = crate::persona::structured_output(&line).unwrap();
    assert!(!text.contains(CANARY), "{text}");
    let back: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(back, json!({"say": "https://<redacted>@h/x,", "n": 3, "list": ["https://<redacted>@h"]}));
}
