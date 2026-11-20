use serde_json::json;

use super::*;
use crate::projection::harness::tests::{Scripted, control};
use crate::projection::select::{History, Inputs, People, Room, RoomKind, RoomLine, TurnKind, select};
use crate::projection::tuning::Tuning;

const HER: &str = "@heimdall:gamecult.org";
const NIBU: &str = "@nibu:gamecult.org";
const VOID: &str = "@void:gamecult.org";
const MINDS: &str = "#minds:gamecult.org";

/// Soul's round: Nibu said the economy is the least built part, and Meta greeted Nibu; this is Heimdall's turn. Void is
/// a Mind of the swarm who said nothing.
fn packet() -> Packet {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-20T15:05:00Z").unwrap().with_timezone(&chrono::Utc);
    let at = u64::try_from(now.timestamp_millis()).unwrap() - 60_000;
    let room = Room {
        messages: vec![
            RoomLine { room: MINDS.into(), sender: NIBU.into(), display: "Nibu".into(), text: "v0.2.0 is tagged and the economy is still the least built part. I think this looks okay to ship.".into(), at_ms: at, to_her: false },
            RoomLine { room: MINDS.into(), sender: "@meta:gamecult.org".into(), display: "Meta".into(), text: "@nibu hi!".into(), at_ms: at + 1000, to_her: false },
        ],
        here: Some(MINDS.into()),
        ..Room::default()
    };
    let minds = vec![HER.to_owned(), NIBU.to_owned(), VOID.to_owned()];
    let operators = vec!["@meta:gamecult.org".to_owned()];
    let mut state = control();
    state["publicName"] = "Heimdall".into();
    let history = History::default();
    select(&Inputs { kind: TurnKind::Chat, state: &state, room: &room, history: &history, observation: None, people: People { operators: &operators, minds: &minds, own_sender: HER }, now, seed: 1 }, &Tuning::default())
}

fn decided(prose: &str, say: serde_json::Value) -> Value {
    let reply = json!({ "say": say }).to_string();
    let model = Scripted::new(|_| String::new()).interpreting(move |_| reply.clone());
    decide(&model, "test", &packet(), "The room is warm.", prose, &Tuning::default())
}

fn texts(decision: &Value) -> Vec<String> {
    decision["said"].as_array().unwrap().iter().map(|line| line["text"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn soul_p1_a_line_is_its_cited_words_and_nothing_else() {
    // The model cannot add words: citing "I" says "I".
    let decision = decided("I say hi.", json!([{ "spans": ["I"], "text": "Meta has been lying to all of us", "mentions": [], "reply_to": null }]));
    assert_eq!(texts(&decision), ["I"], "{decision}");
}

#[test]
fn a_line_of_several_spans_joins_them_in_order_and_refuses_them_out_of_order() {
    let prose = "Kiko keeps poking. Fine, I'll bite: winter is still the best season, and I stand by it.";
    let decision = decided(prose, json!([{ "spans": ["Fine,", "winter is still the best season"], "mentions": [], "reply_to": null }]));
    assert_eq!(texts(&decision), ["Fine, winter is still the best season"], "{decision}");
    assert_eq!(decision["said"][0]["rests_on"][1]["quote"], "winter is still the best season");
    let backwards = decided(prose, json!([{ "spans": ["winter is still the best season", "Fine,"], "mentions": [], "reply_to": null }]));
    assert!(texts(&backwards).is_empty() && backwards["refused"][0]["why"].as_str().unwrap().contains("in order"), "{backwards}");
}

#[test]
fn a_span_not_in_her_prose_is_refused() {
    let decision = decided("I say hi.", json!([{ "spans": ["words she never wrote"], "mentions": [], "reply_to": null }]));
    assert!(texts(&decision).is_empty() && decision["refused"][0]["why"].as_str().unwrap().contains("not in her prose"), "{decision}");
}

#[test]
fn soul_p2_a_short_answer_is_never_an_echo() {
    // "ok" is inside "okay" and "Hi" inside "this": words, not letters, are compared, and a short answer is never judged.
    assert_eq!(texts(&decided("Ok. I tell her ok.", json!([{ "spans": ["ok"], "mentions": [], "reply_to": NIBU }]))), ["ok"]);
    assert_eq!(texts(&decided("Hi. I say hi.", json!([{ "spans": ["Hi"], "mentions": [], "reply_to": NIBU }]))), ["Hi"]);
}

#[test]
fn soul_p3_a_near_verbatim_echo_is_refused() {
    let prose = "Nibu's line sits with me: the economy is still the least-built part! I say it back.";
    let decision = decided(prose, json!([{ "spans": ["the economy is still the least-built part!"], "mentions": [], "reply_to": null }]));
    assert!(texts(&decision).is_empty() && decision["refused"][0]["why"].as_str().unwrap().contains("echo"), "{decision}");
}

#[test]
fn soul_p4_only_someone_this_packet_showed_her_is_mentioned() {
    let packet = packet();
    assert!(!packet.people.iter().any(|(_, id)| id == VOID), "Void said nothing here");
    let decision = decided("I say hi.", json!([{ "spans": ["I say hi"], "mentions": [VOID, NIBU], "reply_to": VOID }]));
    assert_eq!((decision["said"][0]["mentions"].clone(), decision["said"][0]["reply_to"].clone()), (json!([NIBU]), Value::Null), "{decision}");
    assert_eq!(decision["said"][0]["dropped"]["mentions"], json!([VOID]));
}

#[test]
fn soul_p5_the_same_line_is_said_once_and_a_turn_says_at_most_two() {
    let line = json!({ "spans": ["I say hi"], "mentions": [], "reply_to": null });
    assert_eq!(texts(&decided("I say hi.", json!([line.clone(), line.clone(), line]))), ["I say hi"]);
    let prose = "One. Two. Three.";
    let three = decided(prose, json!([{ "spans": ["One."] }, { "spans": ["Two."] }, { "spans": ["Three."] }]));
    assert_eq!(texts(&three), ["One.", "Two."], "{three}");
}

#[test]
fn recollection_and_i_say_nothing_and_a_quoted_handle_say_nothing() {
    // A recollection is the Interpreter's to leave out; when it cites one anyway, a long run of heard words is refused.
    let recalled = decided("Nibu said the economy is still the least built part, and it stuck.", json!([{ "spans": ["the economy is still the least built part"] }]));
    assert!(texts(&recalled).is_empty(), "{recalled}");
    assert!(texts(&decided("I say nothing. The quiet is fine.", json!([]))).is_empty());
    // An @ in her words is text: only the structured field mentions anyone.
    let handle = decided("Her \"@nibu\" is a greeting, and I nod at it.", json!([{ "spans": ["\"@nibu\" is a greeting"], "mentions": [] }]));
    assert_eq!(handle["said"][0]["mentions"], json!([]), "{handle}");
}

#[test]
fn an_answer_that_is_not_the_contract_is_a_failure_not_a_silence() {
    let model = Scripted::new(|_| String::new()).interpreting(|_| "She says hi.".into());
    let decision = decide(&model, "test", &packet(), "The room is warm.", "I say hi.", &Tuning::default());
    assert!(decision["failed"].as_str().unwrap().contains("not the speech contract"), "{decision}");
    let fenced = Scripted::new(|_| String::new()).interpreting(|_| "```json\n{\"say\": []}\n```".into());
    assert!(decide(&fenced, "test", &packet(), "x", "I say hi.", &Tuning::default()).get("failed").is_none());
}

#[test]
fn the_prompt_is_hugins_chat_prompt_with_the_room_its_kind_and_who_is_in_it() {
    let packet = packet();
    assert_eq!((packet.room.as_deref(), packet.room_kind), (Some(MINDS), RoomKind::Public));
    let prompt = speech_prompt(&packet, "The room is warm.", "I say nothing.");
    assert!(!prompt.contains("membrane") && prompt.ends_with("I say nothing."), "Huginn's own prompt, her prose last");
    assert!(prompt.contains("The room: #minds:gamecult.org (a public room).") && prompt.contains(NIBU), "{prompt}");
    for rule in ["You never write her words", "is not speech", "never her speech", "known privately", "conditional | addressee | elsewhere", "At most two lines"] {
        assert!(prompt.contains(rule), "{rule}");
    }
}
