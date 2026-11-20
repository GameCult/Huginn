use serde_json::json;

use super::*;
use crate::projection::harness::tests::{Scripted, control};
use crate::projection::select::{History, Inputs, People, Room, RoomLine, TurnKind, select};
use crate::projection::tuning::Tuning;

const HER: &str = "@heimdall:gamecult.org";
const NIBU: &str = "@nibu:gamecult.org";
const MINDS: &str = "#minds:gamecult.org";

/// The round Soul probed: Nibu said "least built part" and greeted with "@nibu"; this is Heimdall's turn.
fn packet() -> Packet {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-20T15:05:00Z").unwrap().with_timezone(&chrono::Utc);
    let at = u64::try_from(now.timestamp_millis()).unwrap() - 60_000;
    let room = Room {
        messages: vec![
            RoomLine { room: MINDS.into(), sender: NIBU.into(), display: "Nibu".into(), text: "v0.2.0 is tagged and the economy is still the least built part.".into(), at_ms: at, to_her: false },
            RoomLine { room: MINDS.into(), sender: "@meta:gamecult.org".into(), display: "Meta".into(), text: "@nibu hi!".into(), at_ms: at + 1000, to_her: false },
        ],
        here: Some(MINDS.into()),
        ..Room::default()
    };
    let minds = vec![HER.to_owned(), NIBU.to_owned()];
    let operators = vec!["@meta:gamecult.org".to_owned()];
    let mut state = control();
    state["publicName"] = "Heimdall".into();
    let history = History::default();
    select(&Inputs { kind: TurnKind::Chat, state: &state, room: &room, history: &history, observation: None, people: People { operators: &operators, minds: &minds, own_sender: HER }, now, seed: 1 }, &Tuning::default())
}

fn decided(prose: &str, answer: serde_json::Value) -> Value {
    let reply = answer.to_string();
    let model = Scripted::new(|_| String::new()).interpreting(move |_| reply.clone());
    decide(&model, "test", &packet(), "The room is warm.", prose, &Tuning::default())
}

#[test]
fn a_recollection_is_never_her_speech() {
    let prose = "Nibu's 'least built part' snags. I turn it over and say nothing.";
    let answer = json!({ "say": [{ "room": MINDS, "text": "least built part", "source_quote": "least built part", "mentions": [], "reply_to": null }] });
    let decision = decided(prose, answer);
    assert_eq!(decision["said"], json!([]), "{decision}");
    assert!(decision["refused"][0]["why"].as_str().unwrap().contains("recollection"), "{decision}");
}

#[test]
fn i_say_nothing_says_nothing() {
    let decision = decided("I say nothing. The quiet is fine.", json!({ "say": [] }));
    assert_eq!((decision["said"].as_array().map(Vec::len), decision["refused"].as_array().map(Vec::len)), (Some(0), Some(0)));
}

#[test]
fn a_quoted_handle_is_no_mention() {
    // Her "@nibu" is a greeting: even if the model posts it, its text carries no mention, and the quote of Meta's line
    // is someone else's words.
    let prose = "Her \"@nibu\" is a greeting, not mine to answer.";
    let echoed = decided(prose, json!({ "say": [{ "room": MINDS, "text": "@nibu", "source_quote": "@nibu", "mentions": [], "reply_to": null }] }));
    assert_eq!(echoed["said"], json!([]), "the handle is Meta's words: {echoed}");
    let posted = decided(prose, json!({ "say": [{ "room": MINDS, "text": "@nibu is a greeting", "source_quote": "is a greeting", "mentions": [], "reply_to": null }] }));
    assert_eq!(posted["said"][0]["mentions"], json!([]), "only the structured field mentions anyone: {posted}");
}

#[test]
fn a_line_rests_on_her_prose_in_a_room_she_may_speak_in_and_names_only_people_shown() {
    let prose = "I'd like Nibu to know the economy can wait. I tell her so.";
    let answer = json!({ "say": [
        { "room": MINDS, "text": "the economy can wait", "source_quote": "I tell her so", "mentions": [NIBU, "@stranger:x"], "reply_to": NIBU },
        { "room": MINDS, "text": "hello", "source_quote": "words she never wrote", "mentions": [], "reply_to": null },
        { "room": "#elsewhere:gamecult.org", "text": "hi there", "source_quote": "I tell her so", "mentions": [], "reply_to": null },
    ] });
    let decision = decided(prose, answer);
    let said = &decision["said"];
    assert_eq!(said.as_array().unwrap().len(), 1, "{decision}");
    assert_eq!((said[0]["mentions"].clone(), said[0]["reply_to"].clone(), said[0]["rests_on"]["quote"].clone()), (json!([NIBU]), json!(NIBU), json!("I tell her so")));
    assert_eq!(said[0]["dropped"]["mentions"], json!(["@stranger:x"]), "an unknown id is dropped and recorded");
    let whys: Vec<&str> = decision["refused"].as_array().unwrap().iter().filter_map(|line| line["why"].as_str()).collect();
    assert!(whys.iter().any(|why| why.contains("not in her prose")) && whys.iter().any(|why| why.contains("not a room")), "{whys:?}");
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
fn the_prompt_carries_her_prose_the_rooms_and_the_people_by_id_and_the_rules() {
    let prompt = speech_prompt(&packet(), "The room is warm.", "I say nothing.", &Tuning::default());
    assert!(prompt.contains(":interpreter -->") && prompt.ends_with("I say nothing."), "Ghostlight's Interpreter membrane, her prose last");
    assert!(prompt.contains("Rooms she may speak in: #minds:gamecult.org") && prompt.contains(NIBU) && prompt.contains("People she may address"), "{prompt}");
    for rule in ["never her speech", "is not speech", "character for character", "No tool is available"] {
        assert!(prompt.contains(rule), "{rule}");
    }
}
