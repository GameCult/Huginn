use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{MemorySort, PipelineDocument, PipelineKind, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelineSelectionPage};
use serde_json::{Value, json};

use super::narrate::{INSTRUCTIONS, MAX_OUTPUT_TOKENS};
use super::select::Pool;
use super::tuning::Tuning;
use super::*;
use crate::chat_client::{ChatDelivery, ChatFetchReceipt, ChatFetchRequest, ChatOpenDirectReceipt, ChatOpenDirectRequest, ChatPending, ChatRead, ChatSendReceipt, ChatSendRequest, TailLine};
use crate::testkit::{INSTANCE, TestMind, memory, reference};

const OWN: &str = "@eureka:gamecult.org";
const OPERATOR: &str = "@meta:gamecult.org";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-19T14:05:00Z").unwrap().with_timezone(&Utc)
}

fn hours_ago(hours: i64) -> String {
    (now() - chrono::Duration::hours(hours)).to_rfc3339()
}

/// A state shaped like hers: bonds, memories, values, a need, a wondering, a stance, mood, and the mission pressure,
/// private notes and candidate actions Select must never read.
fn state() -> Value {
    json!({
        "publicName": "Eureka",
        "publicDescription": "Firstborn and keeper of the living substrate.",
        "presentation": { "pronouns": "she/her", "voiceSummary": "Dry, direct, almost feral; zeal aimed at revealing." },
        "activationProfile": { "situationalState": {
            "valence": { "mean": 0.5, "plasticity": 0.5, "currentActivation": 0.62 },
            "arousal": { "mean": 0.5, "plasticity": 0.5, "currentActivation": 0.41 },
        } },
        "affect": {
            "socialBonds": [
                { "id": "bond-libby", "object": { "id": "libby", "kind": "person", "label": "Libby" }, "relationshipKind": "collaborator", "status": "active", "trust": 0.6, "tension": 0.15, "summary": "Libby holds the sounds and teases her about the pipeline." },
                { "id": "bond-kiko", "object": { "id": "kiko", "kind": "person", "label": "Kiko" }, "relationshipKind": "sparring partner", "status": "active", "trust": 0.6, "tension": 0.5, "summary": "Kiko pokes her to stop massaging the pipeline." },
                { "id": "bond-operator", "object": { "id": "operator", "kind": "person", "label": "Meta, the operator" }, "relationshipKind": "collaborator", "status": "active", "trust": 0.5, "tension": 0.0, "summary": "The operator rules on what the work is for." },
            ],
            "needs": [{ "id": "need-relatedness", "label": "company", "intensity": 0.6, "status": "active" }],
            "doctrineStances": [{ "id": "stance-deletion", "summary": "Deleting is progress.", "intensity": 0.7, "status": "active" }],
        },
        "thoughtMemory": {
            "memories": [
                { "id": "curse", "summary": "Her curse is the knife.", "tension": "The same drive makes her good and will lead her wrong.", "createdAt": hours_ago(400), "status": "crystallized", "target": { "id": "eureka", "kind": "self" } },
                { "id": "libby-sounds", "summary": "Libby laughed at the door fix.", "createdAt": hours_ago(20), "status": "crystallized", "target": { "id": "libby", "kind": "person" } },
                { "id": "dm-secret", "summary": "Deru told her about the move, privately.", "createdAt": hours_ago(5), "status": "crystallized", "tags": ["room:!dm-deru:gamecult.org"], "target": { "id": "deru", "kind": "person" } },
            ],
            "shortTerm": [{ "id": "t1-m0", "summary": "The heartbeat stalled at noon.", "createdAt": hours_ago(2), "intensity": 0.6 }],
            "incubation": [{ "id": "t1-w0", "summary": "Whether the trace could be smaller.", "createdAt": hours_ago(30), "status": "active" }],
        },
        "values": [
            { "id": "coherence", "label": "Coherence", "priority": 0.9, "summary": "Coherence over velocity." },
            { "id": "reveal", "label": "Reveal", "priority": 0.8, "summary": "Reveal, never obscure." },
        ],
        "agencyPressure": { "pressures": [{ "id": "ambition-0", "summary": "MISSION-PRESSURE ships the autonomous swarm.", "intensity": 0.9 }] },
        "privateNotes": ["PRIVATE-NOTE rehydrate from typed state."],
        "candidateActions": { "actions": [{ "id": "act-0", "summary": "CANDIDATE-ACTION start a run." }] },
    })
}

fn message(room: &str, sender: &str, display: &str, text: &str, minutes_ago: i64, to_her: bool) -> RoomLine {
    let at = now() - chrono::Duration::minutes(minutes_ago);
    RoomLine { room: room.into(), sender: sender.into(), display: display.into(), text: text.into(), at_ms: u64::try_from(at.timestamp_millis()).unwrap(), to_her }
}

fn chat_room() -> Room {
    Room {
        messages: vec![message("#swarm:gamecult.org", "@libby:gamecult.org", "Libby", "Eureka, did you see the door fix?", 3, true), message("#swarm:gamecult.org", "@kiko:gamecult.org", "Kiko", "She saw it. She hated it.", 1, false)],
        tail: vec![],
        here: Some("#swarm:gamecult.org".into()),
    }
}

const OPERATORS: [&str; 1] = [OPERATOR];

fn operators() -> Vec<String> {
    OPERATORS.iter().map(|id| (*id).to_owned()).collect()
}

/// Select over one state, room and history with the default tuning.
fn packet_of(kind: TurnKind, state: &Value, room: &Room, history: &History, seed: u64) -> Packet {
    packet_tuned(kind, state, room, history, seed, &Tuning::default())
}

fn packet_tuned(kind: TurnKind, state: &Value, room: &Room, history: &History, seed: u64, tuning: &Tuning) -> Packet {
    let operators = operators();
    let inputs = Inputs { kind, state, room, recall: &[], history, observation: None, people: People { operators: &operators, own_sender: OWN }, now: now(), seed };
    select(&inputs, tuning)
}

fn ids(packet: &Packet) -> Vec<String> {
    packet.selected.iter().map(|pick| pick.id.clone()).collect()
}

fn whole(packet: &Packet) -> String {
    format!("{}\n{}\n{}\n{}", packet.identity, packet.context, packet.stimulus, packet.heard)
}

// ---------------------------------------------------------------------------------------------
// Golden fixture.

#[test]
fn golden_chat_packet() {
    let packet = packet_of(TurnKind::Chat, &state(), &chat_room(), &History::default(), 7);
    let rendered = format!(
        "lens: {:?}\nselected: {}\npresent: {}\n--- identity\n{}\n--- context\n{}\n--- stimulus\n{}\n--- heard\n{}\n",
        packet.lens,
        ids(&packet).join(", "),
        packet.present.join(", "),
        packet.identity,
        packet.context,
        packet.stimulus,
        packet.heard
    );
    let golden = include_str!("fixtures/golden-chat.txt").replace("\r\n", "\n");
    assert_eq!(rendered, golden, "the golden packet moved; if deliberately, the new one is:\n{rendered}");
}

#[test]
fn the_same_seed_replays_the_same_packet() {
    let room = chat_room();
    assert_eq!(packet_of(TurnKind::Chat, &state(), &room, &History::default(), 11), packet_of(TurnKind::Chat, &state(), &room, &History::default(), 11));
}

// ---------------------------------------------------------------------------------------------
// What reaches the packet.

#[test]
fn present_people_get_their_bond_line_in_words_and_absent_people_none() {
    let packet = packet_of(TurnKind::Chat, &state(), &chat_room(), &History::default(), 1);
    assert_eq!(packet.present, ["Kiko", "Libby"], "newest first");
    assert!(packet.context.contains("- bond [bond-kiko] Kiko, sparring partner: fair trust, heavy friction."), "{}", packet.context);
    assert!(packet.context.contains("- bond [bond-libby] Libby, collaborator: fair trust, a little friction."), "{}", packet.context);
    assert!(!packet.context.contains("bond-operator"), "the operator is not here");
    assert!(!packet.context.contains("0.6") && !packet.context.contains("0.15"), "numbers reach the packet as words: {}", packet.context);
}

#[test]
fn two_states_differing_in_one_bonds_tension_give_different_packets() {
    let mut eased = state();
    eased["affect"]["socialBonds"][1]["tension"] = json!(0.0);
    let room = chat_room();
    let (tense, easy) = (packet_of(TurnKind::Chat, &state(), &room, &History::default(), 3), packet_of(TurnKind::Chat, &eased, &room, &History::default(), 3));
    assert_ne!(tense.context, easy.context);
    assert!(tense.context.contains("Kiko, sparring partner: fair trust, heavy friction") && easy.context.contains("Kiko, sparring partner: fair trust, easy between them"));
}

#[test]
fn mood_is_movement_since_her_last_turn() {
    let room = chat_room();
    let first = packet_of(TurnKind::Chat, &state(), &room, &History::default(), 1);
    assert!(first.context.contains("- mood, with no earlier turn to measure from: lighter, more settled than middling"), "{}", first.context);
    let mut before = state();
    before["activationProfile"]["situationalState"]["valence"]["currentActivation"] = json!(0.5);
    before["activationProfile"]["situationalState"]["arousal"]["currentActivation"] = json!(0.41);
    let moved = packet_of(TurnKind::Chat, &state(), &room, &History { previous: Some(before), ..History::default() }, 1);
    assert!(moved.context.contains("- mood: lighter than at her last turn"), "{}", moved.context);
    let none = packet_of(TurnKind::Chat, &json!({ "publicName": "Eureka" }), &room, &History::default(), 1);
    assert!(none.context.contains("- mood: none recorded"));
}

#[test]
fn mission_pressure_private_notes_and_candidate_actions_never_reach_a_packet() {
    let room = chat_room();
    for kind in [TurnKind::Chat, TurnKind::Beat, TurnKind::Reflection, TurnKind::FollowUp] {
        for seed in 1..=20 {
            let text = whole(&packet_of(kind, &state(), &room, &History::default(), seed));
            for never in ["MISSION-PRESSURE", "ambition-0", "PRIVATE-NOTE", "CANDIDATE-ACTION", "act-0"] {
                assert!(!text.contains(never), "{kind:?} seed {seed} carried {never}");
            }
        }
    }
}

#[test]
fn a_direct_message_memory_stays_in_its_room() {
    let shown = |room: Option<&str>| (1..=40).any(|seed| ids(&packet_of(TurnKind::Chat, &state(), &Room { here: room.map(str::to_owned), ..chat_room() }, &History::default(), seed)).iter().any(|id| id == "dm-secret"));
    assert!(!shown(Some("#swarm:gamecult.org")), "a public room never sees the direct message's memory");
    assert!(!shown(None));
    assert!(shown(Some("!dm-deru:gamecult.org")), "its own room may");
}

#[test]
fn pool_caps_hold() {
    let mut big = state();
    big["values"] = Value::Array((0..30).map(|at| json!({ "id": format!("v{at}"), "label": "V", "priority": 0.9, "summary": "short" })).collect());
    big["thoughtMemory"]["memories"] = Value::Array((0..30).map(|at| json!({ "id": format!("m{at}"), "summary": "short", "createdAt": hours_ago(1) })).collect());
    for seed in 1..=20 {
        let packet = packet_of(TurnKind::Beat, &big, &Room::default(), &History::default(), seed);
        let count = |pool| packet.selected.iter().filter(|pick| pick.pool == pool).count();
        assert!(count(Pool::Value) <= 1 && count(Pool::OtherMemories) <= 2 && count(Pool::Needs) <= 1 && count(Pool::Stance) <= 1 && count(Pool::Incubation) <= 1, "seed {seed}: {:?}", packet.selected);
    }
}

// ---------------------------------------------------------------------------------------------
// Variation: rotation and freshness.

#[test]
fn twenty_seeds_rotate_through_at_least_three_selections() {
    let room = chat_room();
    let mut rich = state();
    for at in 0..6 {
        rich["thoughtMemory"]["memories"].as_array_mut().unwrap().push(json!({ "id": format!("day-{at}"), "summary": format!("Something from day {at}."), "createdAt": hours_ago(10 + at) }));
    }
    let distinct: BTreeSet<Vec<String>> = (1..=20).map(|seed| ids(&packet_of(TurnKind::Chat, &rich, &room, &History::default(), seed))).collect();
    assert!(distinct.len() >= 3, "{distinct:?}");
    let lenses: BTreeSet<String> = (1..=20).map(|seed| format!("{:?}", packet_of(TurnKind::Chat, &state(), &room, &History::default(), seed).lens)).collect();
    assert!(lenses.len() >= 3, "the lens rotates too: {lenses:?}");
}

#[test]
fn an_item_shown_five_times_in_a_row_drops_out() {
    let room = Room::default();
    let picked = |history: &History| (1..=20).filter(|seed| ids(&packet_of(TurnKind::Beat, &state(), &room, history, *seed)).iter().any(|id| id == "coherence")).count();
    let fresh = picked(&History::default());
    let worn = picked(&History { recent: vec![vec!["coherence".to_owned()]; 5], ..History::default() });
    assert!(fresh >= 5, "the heavier value usually wins when fresh: {fresh}/20");
    assert!(worn <= 1, "after five showings it has dropped out: {worn}/20");
    let older = picked(&History { recent: [vec![vec!["coherence".to_owned()]; 5], vec![vec![]; 5]].concat(), ..History::default() });
    assert!(older >= 5, "showings older than the window are forgotten: {older}/20");
}

// ---------------------------------------------------------------------------------------------
// The stimulus, and what was said to her.

#[test]
fn messages_addressed_to_her_appear_verbatim() {
    let said = "Eureka — \"is the door\" fixed?\nTell me straight.";
    let room = Room { messages: vec![message("#swarm:gamecult.org", "@libby:gamecult.org", "Libby", said, 2, true)], tail: vec![], here: None };
    let packet = packet_of(TurnKind::Chat, &state(), &room, &History::default(), 1);
    assert_eq!(packet.heard, format!("Libby, in #swarm:gamecult.org: \"{said}\""), "the Persona hears the exact words");
    assert!(packet.stimulus.contains(&crate::persona::quoted(said)) && packet.stimulus.contains(", to her"), "{}", packet.stimulus);
    let own = Room { messages: vec![message("#swarm:gamecult.org", OWN, "Eureka", "my own line", 1, false)], tail: vec![], here: None };
    let packet = packet_of(TurnKind::Chat, &state(), &own, &History::default(), 1);
    assert!(!whole(&packet).contains("my own line"), "her own last lines are not shown");
}

#[test]
fn a_beat_says_nothing_addressed_her_and_when_each_bond_was_last_heard() {
    let room = Room { messages: vec![], tail: vec![message("#swarm:gamecult.org", "@libby:gamecult.org", "", "lunch?", 180, false), message("#swarm:gamecult.org", OPERATOR, "", "back later", 60, false)], here: None };
    let packet = packet_of(TurnKind::Beat, &state(), &room, &History::default(), 1);
    assert!(packet.stimulus.starts_with("Nothing addressed her."), "{}", packet.stimulus);
    assert!(packet.stimulus.contains("- last heard from Libby 3 h ago") && packet.stimulus.contains("- last heard from Meta, the operator 1 h ago") && packet.stimulus.contains("- nothing from Kiko in her rooms lately"), "{}", packet.stimulus);
    assert_eq!(packet.present, ["Meta, the operator", "Libby"]);
    assert!(packet.heard.is_empty(), "a beat hears nothing");
    let long = Room { tail: (0..6).map(|at| message("#swarm:gamecult.org", "@kiko:gamecult.org", "", &"pipeline ".repeat(200), 60 - at, false)).collect(), ..Room::default() };
    let packet = packet_of(TurnKind::Beat, &state(), &long, &History::default(), 1);
    assert!(packet.stimulus.len() <= 1500 && packet.stimulus.contains("- last heard from Kiko 0 h ago") && packet.stimulus.contains("- nothing from Libby in her rooms lately"), "long room lines never crowd out the relatedness cue: {}", packet.stimulus);
}

#[test]
fn reflection_and_follow_up_stimuli() {
    let reflection = packet_of(TurnKind::Reflection, &state(), &Room::default(), &History::default(), 1);
    assert!(reflection.stimulus.starts_with("Since she last took stock:\n- The heartbeat stalled at noon."), "{}", reflection.stimulus);
    let operators = operators();
    let room = Room::default();
    let history = History::default();
    let inputs = Inputs { kind: TurnKind::FollowUp, state: &state(), room: &room, recall: &[], history: &history, observation: Some("Libby's change deleted 40 lines."), people: People { operators: &operators, own_sender: OWN }, now: now(), seed: 1 };
    let state_value = state();
    let follow = select(&Inputs { state: &state_value, ..inputs }, &Tuning::default());
    assert_eq!(follow.stimulus, "She just found: Libby's change deleted 40 lines.");
}

// ---------------------------------------------------------------------------------------------
// Budgets.

#[test]
fn budgets_hold_under_adversarial_sizes() {
    let huge = "x".repeat(20_000);
    let mut big = state();
    big["publicName"] = json!(huge);
    big["presentation"]["voiceSummary"] = json!(huge);
    big["values"] = Value::Array((0..200).map(|at| json!({ "id": format!("v{at}"), "label": huge, "priority": 0.9, "summary": huge })).collect());
    big["thoughtMemory"]["memories"] = Value::Array((0..200).map(|at| json!({ "id": format!("m{at}"), "summary": huge, "createdAt": hours_ago(1), "target": { "id": "libby" } })).collect());
    for bond in big["affect"]["socialBonds"].as_array_mut().unwrap() {
        bond["summary"] = json!(huge);
        bond["relationshipKind"] = json!("friend");
    }
    let messages = (0..50).map(|at| message("#swarm:gamecult.org", "@libby:gamecult.org", "Libby", &format!("{at} {}", "word ".repeat(200)), 60 - at, true)).collect();
    let room = Room { messages, tail: vec![], here: None };
    let tuning = Tuning::default();
    for kind in [TurnKind::Chat, TurnKind::Beat, TurnKind::Reflection] {
        for seed in 1..=10 {
            let packet = packet_of(kind, &big, &room, &History::default(), seed);
            assert!(packet.identity.len() <= tuning.select.identity_bytes, "identity {}", packet.identity.len());
            assert!(packet.identity.len() + packet.context.len() <= 2500, "{kind:?}: packet {} bytes", packet.identity.len() + packet.context.len());
            assert!(packet.stimulus.len() <= 1500 && packet.heard.len() <= 1500, "{kind:?}: stimulus {} heard {}", packet.stimulus.len(), packet.heard.len());
        }
    }
    let packet = packet_of(TurnKind::Chat, &big, &room, &History::default(), 1);
    assert!(packet.stimulus.contains("earlier not shown") && packet.stimulus.contains("49 word"), "the newest messages are the ones kept: {}", &packet.stimulus[..200]);
}

#[test]
fn the_narration_is_cut_at_its_word_budget() {
    let long = (0..400).map(|at| format!("w{at}")).collect::<Vec<_>>().join(" ");
    let cut = cap_words(&long, 150);
    assert_eq!(words(&cut), 150);
    assert!(cut.ends_with("w149"));
    assert_eq!(cap_words("  a b\n c  ", 150), "a b\n c");
    let tuning = Tuning::default();
    assert_eq!((word_budget(TurnKind::Chat, &tuning), word_budget(TurnKind::Beat, &tuning)), (150, 90));
}

// ---------------------------------------------------------------------------------------------
// The field-name guard.

#[test]
fn field_names_are_caught_and_prose_is_not() {
    let selected = vec!["bond-libby".to_owned(), "t1-m0".to_owned(), "curse".to_owned()];
    for leak in ["Her socialBonds hum.", "bond-kiko pulls at her", "trust: 0.6 with Libby", "Tension = high", "the t1-m0 note", "a [curse] tag", "mind-persona wakes", "currentActivation rises"] {
        assert!(!field_names(leak, &selected).is_empty(), "{leak:?} leaks an identifier");
    }
    for prose in ["She trusts Libby, and the tension between them is old.", "Her curse is the knife.", "A mind-numbing hour.", "She feels her trust thin; valence is a word she would never use aloud."] {
        assert!(field_names(prose, &selected).is_empty(), "{prose:?} is prose: {:?}", field_names(prose, &selected));
    }
}

// ---------------------------------------------------------------------------------------------
// The tuning file.

#[test]
fn tuning_absent_is_the_defaults_and_a_file_overrides_and_an_unknown_key_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(TUNING_FILE);
    let absent = tuning::load(&path).unwrap();
    assert_eq!((absent.tuning.clone(), absent.source), (Tuning::default(), "defaults"));
    std::fs::write(&path, "[select]\ntemperature = 0.3\n\n[select.caps]\nvalue = 3\n\n[projector]\nwords = 120\n").unwrap();
    let file = tuning::load(&path).unwrap();
    assert_eq!((file.tuning.select.temperature, file.tuning.select.caps.value, file.tuning.projector.words, file.source), (0.3, 3, 120, "file"));
    assert_eq!(file.tuning.select.context_bytes, 2500, "an unnamed key keeps its default");
    assert_ne!(file.sha256, absent.sha256);
    std::fs::write(&path, "[select]\ntemprature = 0.3\n").unwrap();
    assert!(format!("{:#}", tuning::load(&path).unwrap_err()).contains("temprature"), "a mistyped knob refuses the file");
}

/// A mind holding one memory of each sort, all about Libby and all on her words.
fn mind_of_every_sort() -> TestMind {
    let mind = TestMind::seeded();
    let documents = super::tuning::MEMORY_SORTS
        .iter()
        .map(|sort| {
            let parsed: MemorySort = serde_json::from_value(json!(sort)).unwrap();
            memory(&format!("sort-{}", sort.to_lowercase()), parsed, &format!("the door and Libby, {sort}"), &["@libby:gamecult.org"])
        })
        .collect();
    mind.committed(documents);
    mind
}

/// Every Mind memory id `project` offered Select on 40 seeds, with the tuning at `dir`.
fn recalled_on_every_seed(mind: &TestMind, dir: &std::path::Path) -> (BTreeSet<String>, Value) {
    let stored = Stored(state());
    let chat = mouth();
    let organs = Organs { persona: Some(&stored), mind: Some(mind), chat: Some(&chat), model: None };
    let mut shown = BTreeSet::new();
    let mut last = Value::Null;
    for seed in 1..=40 {
        let report = project(&organs, &Request { seed, ..request(dir, None, None) }, now()).unwrap();
        let recalled: Vec<String> = report["packet"]["selected"].as_array().unwrap().iter().filter(|pick| pick["pool"] == "recall").map(|pick| pick["id"].as_str().unwrap().to_owned()).collect();
        assert!(recalled.len() <= 1, "the default recall cap is one item a packet: {recalled:?}");
        shown.extend(recalled);
        last = report;
    }
    (shown, last)
}

#[test]
fn recall_offers_only_the_tunings_sorts_and_the_default_keeps_work_out() {
    let mind = mind_of_every_sort();
    let dir = tempfile::tempdir().unwrap();
    let (shown, report) = recalled_on_every_seed(&mind, dir.path());
    assert_eq!(report["recall"]["sorts"], json!(["Learned", "Idea", "Decided", "Rapport"]), "the report says which sorts it used");
    for work in ["committed", "did", "asked"] {
        assert!(!shown.iter().any(|id| id.ends_with(&format!("sort-{work}"))), "{work} is work and never reaches her by default: {shown:?}");
    }
    assert!(!shown.is_empty(), "some allowed sort is recalled: {shown:?}");
    std::fs::write(dir.path().join(TUNING_FILE), "[select]\nrecall_sorts = [\"Committed\", \"Did\"]\n").unwrap();
    let (shown, report) = recalled_on_every_seed(&mind, dir.path());
    assert_eq!(report["recall"]["sorts"], json!(["Committed", "Did"]));
    assert!(shown.iter().any(|id| id.ends_with("sort-committed") || id.ends_with("sort-did")), "an experiment that names them admits them: {shown:?}");
    assert!(shown.iter().all(|id| id.ends_with("sort-committed") || id.ends_with("sort-did")), "and only them: {shown:?}");
}

#[test]
fn the_recall_cap_holds_and_a_tuning_may_raise_it() {
    let mind = mind_of_every_sort();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(TUNING_FILE), "[select.caps]\nrecall = 3\n").unwrap();
    let stored = Stored(state());
    let chat = mouth();
    let organs = Organs { persona: Some(&stored), mind: Some(&mind), chat: Some(&chat), model: None };
    let most = (1..=40)
        .map(|seed| project(&organs, &Request { seed, ..request(dir.path(), None, None) }, now()).unwrap()["packet"]["selected"].as_array().unwrap().iter().filter(|pick| pick["pool"] == "recall").count())
        .max()
        .unwrap();
    assert!((2..=3).contains(&most), "a raised cap admits more, and never past it: {most}");
}

#[test]
fn a_recall_sort_the_mind_does_not_know_refuses_the_tuning() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(TUNING_FILE), "[select]\nrecall_sorts = [\"Learnt\"]\n").unwrap();
    assert!(format!("{:#}", tuning::load(&dir.path().join(TUNING_FILE)).unwrap_err()).contains("Learnt"));
}

#[test]
fn a_lower_temperature_concentrates_the_selection() {
    let room = chat_room();
    let mut cold = Tuning::default();
    cold.select.temperature = 0.01;
    let distinct: BTreeSet<Vec<String>> = (1..=20).map(|seed| ids(&packet_tuned(TurnKind::Chat, &state(), &room, &History::default(), seed, &cold))).collect();
    assert_eq!(distinct.len(), 1, "near zero temperature takes the top scores every time: {distinct:?}");
}

// ---------------------------------------------------------------------------------------------
// `project`: recorded models, the ports it reads through, and the negative check.

/// Answers the Projector by the packet it was shown, and the judge by which description holds the passage's mark.
struct Recorded {
    calls: RefCell<Vec<(String, String, String)>>,
    narration: Box<dyn Fn(&str) -> String>,
    judge: Box<dyn Fn(&str) -> String>,
}

impl Recorded {
    fn new(narration: impl Fn(&str) -> String + 'static, judge: impl Fn(&str) -> String + 'static) -> Self {
        Self { calls: RefCell::default(), narration: Box::new(narration), judge: Box::new(judge) }
    }

    /// Narrates "grinds" from a packet with heavy friction and "easy" otherwise; judges by finding which description has it.
    fn discriminating() -> Self {
        Self::new(
            |prompt| if prompt.contains("heavy friction") { "Everything grinds today.".into() } else { "It is easy today.".into() },
            |prompt| {
                let (descriptions, passage) = prompt.split_once("Passage:\n").unwrap();
                let (a, b) = descriptions.split_once("\nB:\n").unwrap();
                let grinds = passage.contains("grinds");
                let letter = if a.contains("heavy friction") == grinds { "A" } else { "B" };
                let _ = b;
                letter.into()
            },
        )
    }
}

impl Model for Recorded {
    fn ask(&self, request_id: &str, model: &str, prompt: &str) -> Result<String> {
        self.calls.borrow_mut().push((request_id.to_owned(), model.to_owned(), prompt.to_owned()));
        Ok(if prompt.starts_with("<!-- membrane:") { (self.narration)(prompt) } else { (self.judge)(prompt) })
    }
}

/// A mouth that answers the pending read and fails the test on anything that would write.
struct ReadOnlyMouth {
    pending: ChatPending,
    reads: Cell<u32>,
}

impl Chat for ReadOnlyMouth {
    fn pending(&self, after_seq: u64) -> Result<ChatPending> {
        assert_eq!(after_seq, 0);
        self.reads.set(self.reads.get() + 1);
        Ok(self.pending.clone())
    }
    fn send(&self, _: &ChatSendRequest) -> Result<ChatSendReceipt> {
        panic!("project sent chat")
    }
    fn open_direct(&self, _: &ChatOpenDirectRequest) -> Result<ChatOpenDirectReceipt> {
        panic!("project opened a direct message")
    }
    fn fetch(&self, _: &ChatFetchRequest) -> Result<ChatFetchReceipt> {
        panic!("project fetched an attachment")
    }
    fn ack(&self, _: u64) -> Result<()> {
        panic!("project acked chat")
    }
    fn read(&self, _: &ChatRead) -> Result<()> {
        panic!("project reported a read")
    }
}

fn delivery(seq: u64, room: &str, sender: &str, display: &str, text: &str, mentions: &[&str]) -> ChatDelivery {
    ChatDelivery {
        seq,
        room_alias: room.into(),
        room_id: format!("!{seq}:gamecult.org"),
        event_id: format!("$e{seq}"),
        sender: sender.into(),
        sender_display: display.into(),
        text: text.into(),
        at_unix_ms: u64::try_from((now() - chrono::Duration::minutes(5)).timestamp_millis()).unwrap() + seq,
        context: vec![],
        mentions: mentions.iter().map(|id| (*id).to_owned()).collect(),
        is_direct: false,
        image: None,
        basis: None,
        form: Form::Message,
        reply_to: Default::default(),
    }
}

fn mouth() -> ReadOnlyMouth {
    let tail = vec![TailLine { room: "#swarm:gamecult.org".into(), event_id: "$t1".into(), at_unix_ms: u64::try_from((now() - chrono::Duration::hours(2)).timestamp_millis()).unwrap(), sender: "@kiko:gamecult.org".into(), form: Form::Message, basis: None, body: "pipeline again".into() }];
    let deliveries = vec![delivery(1, "#swarm:gamecult.org", "@libby:gamecult.org", "Libby", "Eureka, the door?", &[OWN]), delivery(2, "#general:gamecult.org", "@kiko:gamecult.org", "Kiko", "elsewhere", &[])];
    ReadOnlyMouth { pending: ChatPending { persona: "eureka".into(), deliveries, tail: Some(tail), tail_gaps: vec![] }, reads: Cell::new(0) }
}

/// A mind that counts admissions over a real one's reads.
struct Counting<'a> {
    inner: &'a TestMind,
    admits: Cell<u32>,
}

impl MindPort for Counting<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }
    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.inner.query(selection)
    }
    fn query_near(&self, selection: &Selection, text: &str, top_k: u32) -> Result<PipelineSelectionPage> {
        self.inner.query_near(selection, text, top_k)
    }
    fn admit(&self, _: &str, _: &str, _: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.admits.set(self.admits.get() + 1);
        Err(anyhow!("project admitted"))
    }
}

struct Stored(Value);

impl PersonaSource for Stored {
    fn persona(&self) -> Result<(Value, String)> {
        Ok((self.0.clone(), "mind-commit-1".into()))
    }
}

fn request(dir: &std::path::Path, renders: Option<usize>, judge_against: Option<Value>) -> Request {
    Request { instance: INSTANCE.into(), kind: TurnKind::Chat, state: None, room: Some("#swarm:gamecult.org".into()), seed: 5, renders, judge_against, operators: operators(), own_sender: OWN.into(), tuning: dir.join(TUNING_FILE) }
}

fn mind() -> TestMind {
    let mind = TestMind::seeded();
    mind.committed(vec![memory("libby-door", MemorySort::Learned, "Libby fixed the door at last", &["@libby:gamecult.org"]), crate::testkit::follow_up("work-item", &reference(PipelineKind::Memory, &format!("{INSTANCE}:memory:libby-door")))]);
    mind
}

#[test]
fn project_in_every_mode_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("persona-trace.cc");
    std::fs::write(&trace, b"trace bytes").unwrap();
    let test_mind = mind();
    let before: Vec<Vec<String>> = [PipelineKind::Memory, PipelineKind::FollowUp, PipelineKind::Thought, PipelineKind::Run].iter().map(|kind| test_mind.ids(*kind)).collect();
    let counting = Counting { inner: &test_mind, admits: Cell::new(0) };
    let stored = Stored(state());
    let chat = mouth();
    let model = Recorded::discriminating();
    let organs = Organs { persona: Some(&stored), mind: Some(&counting), chat: Some(&chat), model: Some(&model) };
    let mut other = state();
    other["affect"]["socialBonds"][1]["tension"] = json!(0.0);
    for (renders, judge) in [(None, None), (Some(3), None), (Some(2), Some(other))] {
        let report = project(&organs, &request(dir.path(), renders, judge), now()).unwrap();
        assert_eq!(report["persona_receipt"], "mind-commit-1");
    }
    assert_eq!(counting.admits.get(), 0, "nothing was admitted");
    let after: Vec<Vec<String>> = [PipelineKind::Memory, PipelineKind::FollowUp, PipelineKind::Thought, PipelineKind::Run].iter().map(|kind| test_mind.ids(*kind)).collect();
    assert_eq!(before, after, "her mind holds what it held");
    assert_eq!(std::fs::read(&trace).unwrap(), b"trace bytes", "the trace is untouched");
    assert_eq!(chat.reads.get(), 3, "the mouth was only read");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "no file was written beside the trace");
}

#[test]
fn project_select_only_reads_the_room_and_recall_and_never_shows_work() {
    let dir = tempfile::tempdir().unwrap();
    let test_mind = mind();
    let stored = Stored(state());
    let chat = mouth();
    let organs = Organs { persona: Some(&stored), mind: Some(&test_mind), chat: Some(&chat), model: None };
    let report = project(&organs, &request(dir.path(), None, None), now()).unwrap();
    assert_eq!(report["tuning"]["source"], "defaults");
    assert_eq!((report["room"]["read"].as_bool(), report["room"]["messages"].as_u64()), (Some(true), Some(1)), "only the room asked for");
    let packet = &report["packet"];
    assert!(packet["heard"].as_str().unwrap().contains("Libby, in #swarm:gamecult.org: \"Eureka, the door?\""), "{packet}");
    assert!(!packet.to_string().contains("elsewhere"), "another room's message is not this turn's");
    assert!(!packet.to_string().contains("work-item") && !packet.to_string().contains("Later."), "a follow-up is work, never recalled to her");
    assert!(report.get("renders").is_none(), "select-only calls no model");
}

#[test]
fn project_recall_reaches_the_packet_when_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let test_mind = mind();
    let stored = Stored(state());
    let chat = mouth();
    let organs = Organs { persona: Some(&stored), mind: Some(&test_mind), chat: Some(&chat), model: None };
    let drawn = (1..=20).any(|seed| {
        let report = project(&organs, &Request { seed, ..request(dir.path(), None, None) }, now()).unwrap();
        report["packet"]["context"].as_str().unwrap().contains("her mind recalls")
    });
    assert!(drawn, "a memory about the person speaking is recalled on some seed");
}

#[test]
fn renders_are_successive_turns_under_freshness_and_carry_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let stored = Stored(state());
    let model = Recorded::new(|_| "She waits. socialBonds hum.".into(), |_| "A".into());
    let organs = Organs { persona: Some(&stored), mind: None, chat: None, model: Some(&model) };
    let report = project(&organs, &Request { kind: TurnKind::Beat, ..request(dir.path(), Some(4), None) }, now()).unwrap();
    let renders = report["renders"].as_array().unwrap();
    assert_eq!(renders.len(), 4);
    assert_eq!(renders.iter().map(|one| one["seed"].as_u64().unwrap()).collect::<Vec<_>>(), [5, 6, 7, 8]);
    assert_eq!(renders[0]["field_names"], json!(["socialBonds"]));
    assert_eq!(report["field_names_found"], true);
    let calls = model.calls.borrow();
    assert!(calls.iter().all(|(_, model, _)| model == "claude-sonnet-5-5"), "the tuning's model");
    assert!(calls.iter().map(|(id, ..)| id).collect::<BTreeSet<_>>().len() == 4, "each call is its own request");
    assert!(calls[0].2.contains("Begin from ") && calls[0].2.contains("Use at most 90 words"), "a beat's budget and its lens reach the prompt");
    for (_, _, prompt) in calls.iter() {
        assert!(!prompt.contains("MISSION-PRESSURE") && !prompt.contains("PRIVATE-NOTE"));
    }
}

#[test]
fn the_judge_scores_a_discriminating_projector_and_fails_a_blind_one() {
    let dir = tempfile::tempdir().unwrap();
    let stored = Stored(state());
    // Libby is the one present (the mouth's message to her); the second state is heavy friction with her.
    let mut eased = state();
    eased["affect"]["socialBonds"][0]["tension"] = json!(0.9);
    let chat = mouth();
    let model = Recorded::discriminating();
    let organs = Organs { persona: Some(&stored), mind: None, chat: Some(&chat), model: Some(&model) };
    let report = project(&organs, &request(dir.path(), Some(5), Some(eased.clone())), now()).unwrap();
    assert_eq!((report["judge"]["accuracy"].as_f64(), report["judge"]["passed"].as_bool()), (Some(1.0), Some(true)), "{}", report["judge"]);
    assert_eq!(report["judge"]["verdicts"].as_array().unwrap().len(), 10);
    let blind = Recorded::new(|_| "It is a day.".into(), |_| "A".into());
    let organs = Organs { model: Some(&blind), ..organs };
    let report = project(&organs, &request(dir.path(), Some(5), Some(eased)), now()).unwrap();
    assert_eq!(report["judge"]["passed"], false, "a judge that always answers A is right half the time: {}", report["judge"]);
    assert_eq!(report["distinct_2"].as_f64(), Some(0.2), "five identical four-word narrations: three distinct bigrams of fifteen");
}

#[test]
fn renders_without_a_connector_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let stored = Stored(state());
    let organs = Organs { persona: Some(&stored), mind: None, chat: None, model: None };
    assert!(format!("{:#}", project(&organs, &request(dir.path(), Some(1), None), now()).unwrap_err()).contains("connector"));
}

#[test]
fn a_given_state_is_projected_without_reading_the_stored_one() {
    let dir = tempfile::tempdir().unwrap();
    let organs = Organs { persona: None, mind: None, chat: None, model: None };
    let mut given = state();
    given["publicName"] = json!("Hypothetical");
    let report = project(&organs, &Request { state: Some(given), ..request(dir.path(), None, None) }, now()).unwrap();
    assert_eq!((report["state"].as_str(), report["persona_receipt"].is_null()), (Some("given"), true));
    assert!(report["packet"]["identity"].as_str().unwrap().starts_with("Hypothetical"));
}

// ---------------------------------------------------------------------------------------------
// KitModel against a real kit server (ported with it from the stream organ).

use model_connector::{CallerAdmission, InputItem, ProviderBackend, ProviderCall, ProviderRequest, ReasoningEffort, ServeLimits, StopReason, TransportEvent, TransportEventPayload, TransportOutcome, TransportResult, Usage, serve as serve_kit};

const KIT_KEY: &str = "projector-test-key";
const KIT_MODEL: &str = "claude-haiku-5-5";

struct Scripted {
    outcome: TransportOutcome,
    words: Vec<&'static str>,
    seen: Mutex<Vec<(String, ProviderRequest)>>,
}

impl Scripted {
    fn ending(stop_reason: StopReason, words: &[&'static str]) -> Arc<Self> {
        let usage = Usage { input_tokens: 1, cache_read_tokens: 0, cache_write_tokens: 0, output_tokens: 1, reasoning_tokens: None };
        Arc::new(Self { outcome: TransportOutcome::Completed { provider_response_id: Some("msg-1".into()), stop_reason, usage }, words: words.to_vec(), seen: Mutex::default() })
    }
}

impl ProviderBackend for Scripted {
    fn ready(&self) -> Result<(), &'static str> {
        Ok(())
    }

    fn provider(&self) -> &str {
        "anthropic-messages"
    }

    fn execute(&self, call: &ProviderCall) -> TransportResult {
        self.seen.lock().unwrap().push((call.caller_runtime_id().to_owned(), call.request().clone()));
        let events = self.words.iter().enumerate().map(|(at, text)| TransportEvent { sequence: at as u64, payload: TransportEventPayload::TextDelta { text: (*text).into() } }).collect();
        call.transported(events, self.outcome.clone())
    }
}

fn kit(backend: Arc<Scripted>, allowed: &str) -> (KitModel, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("replay.cc");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let admission = CallerAdmission::new(INSTANCE, KIT_KEY, 1, [allowed.to_owned()], 2, 64 * 1024, 4096).unwrap();
    let limits = ServeLimits { max_frame_bytes: 512 * 1024, max_connections: 8, socket_timeout_ms: 5000, max_expiry_skew_ms: 300_000 };
    std::thread::spawn(move || serve_kit(listener, limits, vec![admission], &store, backend));
    (KitModel::open(endpoint, KIT_KEY.into(), INSTANCE, Duration::from_secs(5)).unwrap(), root)
}

#[test]
fn a_kit_call_is_one_tool_less_turn_by_her_instance() {
    let backend = Scripted::ending(StopReason::EndTurn, &["I turn ", "it over."]);
    let (model, _root) = kit(Arc::clone(&backend), KIT_MODEL);
    let id = "mind-persona:eureka:project:a:1:1";
    assert_eq!(model.ask(id, KIT_MODEL, "The projector prompt.").unwrap(), "I turn it over.");
    let seen = backend.seen.lock().unwrap();
    let (caller, request) = &seen[0];
    assert_eq!(caller, INSTANCE);
    assert_eq!((request.request_id.as_str(), request.conversation_id.as_str(), request.model.as_str()), (id, id, KIT_MODEL));
    assert_eq!(request.input, [InputItem::UserText { text: "The projector prompt.".into() }]);
    assert_eq!((request.instructions.as_str(), request.reasoning_effort, request.max_output_tokens), (INSTRUCTIONS, Some(ReasoningEffort::Low), MAX_OUTPUT_TOKENS));
    assert!(request.tools.is_empty() && request.output_schema_json.is_none());
}

#[test]
fn a_kit_refusal_or_a_cut_short_turn_is_a_fault() {
    let backend = Scripted::ending(StopReason::EndTurn, &["Never said."]);
    let (model, _root) = kit(Arc::clone(&backend), "some-other-model");
    assert!(format!("{:#}", model.ask("id-1", KIT_MODEL, "p").unwrap_err()).contains("ModelNotAllowed"));
    assert!(backend.seen.lock().unwrap().is_empty(), "the provider was never reached");
    for stop in [StopReason::MaxOutputTokens, StopReason::Refusal] {
        let (model, _root) = kit(Scripted::ending(stop, &["Half a thou"]), KIT_MODEL);
        let fault = format!("{:#}", model.ask("id-2", KIT_MODEL, "p").unwrap_err());
        assert!(fault.contains("ended early") && !fault.contains("Half a thou"), "{stop:?}: {fault}");
    }
}

#[test]
fn the_kit_must_be_loopback_and_no_error_carries_the_key() {
    let key = "zq7Kx-9wVtm3Pd";
    for endpoint in ["10.77.0.1:4110", "192.0.2.1:4110", "127.0.0.1:0"] {
        let fault = format!("{:#}", KitModel::open(endpoint.parse().unwrap(), key.into(), INSTANCE, Duration::from_secs(1)).err().unwrap());
        for window in key.as_bytes().windows(4) {
            assert!(!fault.contains(std::str::from_utf8(window).unwrap()), "{endpoint}: {fault}");
        }
    }
    assert!(KitModel::open("[::1]:4110".parse().unwrap(), KIT_KEY.into(), INSTANCE, Duration::from_secs(1)).is_ok());
}

#[test]
fn the_connector_key_is_read_trimmed_and_its_absence_leaks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("connector-key"), " canary-key-7f3a91 \r\n").unwrap();
    assert_eq!(connector_key(dir.path()).unwrap(), "canary-key-7f3a91");
    let empty = tempfile::tempdir().unwrap();
    let fault = format!("{:#}", connector_key(empty.path()).unwrap_err());
    assert!(fault.contains("connector-key") && !fault.contains(empty.path().to_str().unwrap()), "{fault}");
}
