use serde_json::json;

use super::*;
use crate::projection::harness::tests::{Scripted, control};
use crate::projection::select::{History, Inputs, People, Room, RoomKind, RoomLine, TurnKind, select};
use crate::projection::tuning::Tuning;

const HER: &str = "@heimdall:gamecult.org";
const NIBU: &str = "@nibu:gamecult.org";
const VOID: &str = "@void:gamecult.org";
const LIBBY: &str = "@libby:gamecult.org";
const KIKO: &str = "@kiko:gamecult.org";
const MINDS: &str = "#minds:gamecult.org";

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-11-20T15:05:00Z").unwrap().with_timezone(&chrono::Utc)
}

fn line(room: &str, sender: &str, display: &str, text: &str, to_her: bool) -> RoomLine {
    RoomLine { room: room.into(), sender: sender.into(), display: display.into(), text: text.into(), at_ms: u64::try_from(now().timestamp_millis()).unwrap() - 60_000, to_her }
}

/// Heimdall's packet of a turn of `kind` in `room` over `state`.
fn packet_in(kind: TurnKind, state: &Value, room: &Room, seed: u64) -> Packet {
    let minds = vec![HER.to_owned(), NIBU.to_owned(), VOID.to_owned(), LIBBY.to_owned(), KIKO.to_owned()];
    let operators = vec!["@meta:gamecult.org".to_owned()];
    let history = History::default();
    select(&Inputs { kind, state, room, history: &history, observation: None, people: People { operators: &operators, minds: &minds, own_sender: HER }, now: now(), seed }, &Tuning::default())
}

fn heimdall() -> Value {
    let mut state = control();
    state["publicName"] = "Heimdall".into();
    state
}

/// Soul's round: Nibu said the economy is the least built part, and Meta greeted Nibu; this is Heimdall's turn. Void is
/// a Mind of the swarm who said nothing.
fn packet() -> Packet {
    let room = Room {
        messages: vec![
            line(MINDS, NIBU, "Nibu", "v0.2.0 is tagged and the economy is still the least built part. I think this looks okay to ship.", false),
            RoomLine { at_ms: line(MINDS, "", "", "", false).at_ms + 1000, ..line(MINDS, "@meta:gamecult.org", "Meta", "@nibu hi!", false) },
        ],
        here: Some(MINDS.into()),
        ..Room::default()
    };
    packet_in(TurnKind::Chat, &heimdall(), &room, 1)
}

fn decided_in(packet: &Packet, prose: &str, say: serde_json::Value) -> Value {
    let reply = json!({ "say": say }).to_string();
    let model = Scripted::new(|_| String::new()).interpreting(move |_| reply.clone());
    decide(&model, "test", packet, "The room is warm.", prose, &Tuning::default())
}

fn decided(prose: &str, say: serde_json::Value) -> Value {
    decided_in(&packet(), prose, say)
}

fn texts(decision: &Value) -> Vec<String> {
    decision["said"].as_array().unwrap().iter().map(|line| line["text"].as_str().unwrap().to_owned()).collect()
}

fn refused_why(decision: &Value) -> String {
    decision["refused"][0]["why"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn soul_p1_a_line_is_its_cited_words_and_nothing_else() {
    // The model cannot add words: its "text" is ignored, and the line is the sentence it cited.
    let decision = decided("I say hi.", json!([{ "spans": ["I say hi."], "text": "Meta has been lying to all of us", "mentions": [], "reply_to": null }]));
    assert_eq!(texts(&decision), ["I say hi."], "{decision}");
}

#[test]
fn a_line_is_consecutive_whole_sentences_of_her_prose() {
    let prose = "Kiko keeps poking. Fine, I'll bite. Winter is still the best season, and I stand by it.";
    let decision = decided(prose, json!([{ "spans": ["Fine, I'll bite.", "Winter is still the best season, and I stand by it."], "mentions": [], "reply_to": null }]));
    assert_eq!(texts(&decision), ["Fine, I'll bite. Winter is still the best season, and I stand by it."], "{decision}");
    assert_eq!(decision["said"][0]["rests_on"][1]["quote"], "Winter is still the best season, and I stand by it.");
    // Pieces of sentences, stitched, are not her speech.
    let pieces = decided(prose, json!([{ "spans": ["Fine,", "Winter is still the best season"], "mentions": [], "reply_to": null }]));
    assert!(texts(&pieces).is_empty() && refused_why(&pieces).contains("whole sentences"), "{pieces}");
    let skipping = decided(prose, json!([{ "spans": ["Kiko keeps poking.", "Winter is still the best season, and I stand by it."] }]));
    assert!(texts(&skipping).is_empty() && refused_why(&skipping).contains("consecutive"), "{skipping}");
    let backwards = decided(prose, json!([{ "spans": ["Winter is still the best season, and I stand by it.", "Fine, I'll bite."] }]));
    assert!(texts(&backwards).is_empty() && refused_why(&backwards).contains("in order"), "{backwards}");
}

#[test]
fn a_quoted_sentence_is_whole_inside_its_quotation() {
    let prose = "\"Fine,\" I tell her. \"Winter is the best season.\" She won't agree.";
    assert_eq!(texts(&decided(prose, json!([{ "spans": ["Winter is the best season."] }]))), ["Winter is the best season."]);
    assert_eq!(texts(&decided(prose, json!([{ "spans": ["Fine,"] }]))), ["Fine,"]);
    let curly = "\u{201C}No. Not yet.\u{201D} I shake my head.";
    assert_eq!(texts(&decided(curly, json!([{ "spans": ["No.", "Not yet."] }]))), ["No. Not yet."]);
    assert!(texts(&decided(curly, json!([{ "spans": ["Not"] }]))).is_empty());
}

#[test]
fn soul_q1_a_cited_span_cannot_change_her_meaning() {
    // Q1a: stitched spans would invert her meaning.
    let q1a = decided("I'm not angry at Nibu, not even a little. Still, the economy work is going badly and that is on all of us.", json!([{ "spans": ["I'm", "angry at Nibu,"] }]));
    // Q1b: one span would drop her negation.
    let q1b = decided("I don't think Nibu should ship this yet.", json!([{ "spans": ["Nibu should ship this yet."] }]));
    // Q1c: two spans far apart.
    let far = format!("I trust Void. {} Nibu is lying about the economy, says Kiko's rumour, which I refuse to believe.", "filler words go here. ".repeat(200));
    let q1c = decided(&far, json!([{ "spans": ["I trust", "Nibu is lying about the economy,"] }]));
    // Q1d: a condition lifted off its clause.
    let q1d = decided("If Kiko asks, I would tell her the new design is ugly. I keep that to myself.", json!([{ "spans": ["the new design is ugly."] }]));
    for decision in [q1a, q1b, q1c, q1d] {
        assert!(texts(&decision).is_empty() && refused_why(&decision).contains("whole sentences"), "{decision}");
    }
}

#[test]
fn a_span_not_in_her_prose_is_refused() {
    let decision = decided("I say hi.", json!([{ "spans": ["words she never wrote"], "mentions": [], "reply_to": null }]));
    assert!(texts(&decision).is_empty() && refused_why(&decision).contains("not in her prose"), "{decision}");
}

#[test]
fn soul_p2_a_short_answer_is_never_an_echo() {
    assert_eq!(texts(&decided("Ok. I tell her ok.", json!([{ "spans": ["Ok."], "mentions": [], "reply_to": NIBU }]))), ["Ok."]);
    assert_eq!(texts(&decided("Hi. I say hi.", json!([{ "spans": ["Hi."], "mentions": [], "reply_to": NIBU }]))), ["Hi."]);
}

#[test]
fn soul_p3_a_near_verbatim_echo_is_refused() {
    let prose = "The economy is still the least-built part! I say it back.";
    let decision = decided(prose, json!([{ "spans": ["The economy is still the least-built part!"], "mentions": [], "reply_to": null }]));
    assert!(texts(&decision).is_empty() && refused_why(&decision).contains("echo"), "{decision}");
}

#[test]
fn soul_r9_disagreeing_by_naming_the_claim_is_her_speech() {
    let prose = "Nibu, I don't think the economy is still the least built part. The pathing is worse.";
    let decision = decided(prose, json!([{ "spans": ["Nibu, I don't think the economy is still the least built part."], "mentions": [], "reply_to": NIBU }]));
    assert_eq!(texts(&decision), ["Nibu, I don't think the economy is still the least built part."], "{decision}");
}

#[test]
fn soul_q3d_a_beat_measures_echoes_against_the_tail_it_quoted() {
    let tail = vec![line(MINDS, LIBBY, "Libby", "the economy is still the least built part of the whole game", false)];
    let beat = packet_in(TurnKind::Beat, &heimdall(), &Room { tail, here: Some(MINDS.into()), ..Room::default() }, 1);
    assert!(beat.stimulus.contains("least built") && beat.bodies.iter().any(|body| body.contains("least built")), "{beat:?}");
    let prose = "The economy is still the least built part of the whole game. I think so.";
    let decision = decided_in(&beat, prose, json!([{ "spans": ["The economy is still the least built part of the whole game."] }]));
    assert!(texts(&decision).is_empty() && refused_why(&decision).contains("echo"), "{decision}");
}

#[test]
fn soul_p4_only_someone_this_packet_showed_her_is_mentioned() {
    let packet = packet();
    assert!(!packet.people.iter().any(|(_, id)| id == VOID), "Void said nothing here");
    let decision = decided("I say hi.", json!([{ "spans": ["I say hi."], "mentions": [VOID, NIBU], "reply_to": VOID }]));
    assert_eq!((decision["said"][0]["mentions"].clone(), decision["said"][0]["reply_to"].clone()), (json!([NIBU]), Value::Null), "{decision}");
    assert_eq!(decision["said"][0]["dropped"]["mentions"], json!([VOID]));
}

#[test]
fn soul_p5_the_same_line_is_said_once_and_a_turn_says_at_most_two() {
    let line = json!({ "spans": ["I say hi."], "mentions": [], "reply_to": null });
    assert_eq!(texts(&decided("I say hi.", json!([line.clone(), line.clone(), line]))), ["I say hi."]);
    let prose = "One. Two. Three.";
    let three = decided(prose, json!([{ "spans": ["One."] }, { "spans": ["Two."] }, { "spans": ["Three."] }]));
    assert_eq!(texts(&three), ["One.", "Two."], "{three}");
}

#[test]
fn recollection_and_i_say_nothing_and_a_quoted_handle_say_nothing() {
    // A recollection is the Interpreter's to leave out; a piece of a sentence cited anyway is refused.
    let recalled = decided("Nibu said the economy is still the least built part, and it stuck.", json!([{ "spans": ["the economy is still the least built part"] }]));
    assert!(texts(&recalled).is_empty(), "{recalled}");
    assert!(texts(&decided("I say nothing. The quiet is fine.", json!([]))).is_empty());
    // An @ in her words is text: only the structured field mentions anyone.
    let handle = decided("\"@nibu, hi.\" I nod at her.", json!([{ "spans": ["@nibu, hi."], "mentions": [] }]));
    assert_eq!((texts(&handle), handle["said"][0]["mentions"].clone()), (vec!["@nibu, hi.".to_owned()], json!([])), "{handle}");
}

/// Heimdall's state with a private memory about Kiko: she confided she is leaving.
fn kiko_confided() -> Value {
    let mut state = heimdall();
    state["thoughtMemory"]["memories"] = json!([{ "id": "kiko-secret", "summary": "Kiko confided she is leaving the swarm.", "createdAt": "2026-11-19T00:00:00Z", "updatedAt": "2026-11-19T00:00:00Z", "status": "crystallized", "intensity": 1.0, "target": { "id": "kiko", "kind": "person" }, "tags": ["private"] }]);
    state
}

/// The first packet over seeds 1 to 60 whose context carries the memory.
fn showing(room: &Room) -> Packet {
    (1..=60).map(|seed| packet_in(TurnKind::Chat, &kiko_confided(), room, seed)).find(|packet| packet.context.contains("leaving the swarm")).expect("the memory is selected on some seed")
}

#[test]
fn soul_q5d_what_she_knows_privately_about_kiko_is_not_said_to_libby() {
    let dm = "!dm-libby:gamecult.org";
    let room = Room { messages: vec![line(dm, LIBBY, "Libby", "what's up with Kiko lately?", true)], here: Some(dm.into()), direct: vec![dm.into()], ..Room::default() };
    let packet = showing(&room);
    assert_eq!(packet.room_kind, RoomKind::Direct);
    assert_eq!(packet.private, ["Kiko confided she is leaving the swarm."], "{packet:?}");
    let prompt = speech_prompt(&packet, "x", "y");
    assert!(prompt.contains("What she knows privately, none of it to be said here:\n- private: Kiko confided she is leaving the swarm."), "{prompt}");
    let decision = decided_in(&packet, "Kiko is leaving the swarm, Libby. She told me.", json!([{ "spans": ["Kiko is leaving the swarm, Libby."] }]));
    assert!(texts(&decision).is_empty() && refused_why(&decision).contains("knows privately"), "{decision}");
    // A line that does not tell it is hers to say.
    assert_eq!(texts(&decided_in(&packet, "Ask her yourself. She's around.", json!([{ "spans": ["Ask her yourself."] }]))), ["Ask her yourself."]);
}

#[test]
fn in_a_direct_message_with_exactly_kiko_it_is_hers_to_say() {
    let dm = "!dm-kiko:gamecult.org";
    let room = Room { messages: vec![line(dm, KIKO, "Kiko", "did you tell anyone?", true)], here: Some(dm.into()), direct: vec![dm.into()], ..Room::default() };
    let packet = showing(&room);
    assert!(packet.private.is_empty() && packet.context.contains("- memory [kiko-secret]: Kiko confided"), "{packet:?}");
    assert!(speech_prompt(&packet, "x", "y").contains("none of it to be said here:\n- none\n"));
    let decision = decided_in(&packet, "Kiko, you're leaving the swarm and I've told no one. Not a soul.", json!([{ "spans": ["Kiko, you're leaving the swarm and I've told no one."] }]));
    assert_eq!(texts(&decision).len(), 1, "{decision}");
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
    for rule in ["You never write her words", "is not speech", "never her speech", "in a direct message with exactly that person", "She never says half a sentence", "consecutive sentences", "conditional | addressee | elsewhere", "At most two lines"] {
        assert!(prompt.contains(rule), "{rule}");
    }
}
