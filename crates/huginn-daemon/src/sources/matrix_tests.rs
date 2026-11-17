//! The Matrix client against a fake Synapse that speaks the client API's JSON.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::chat::{EventKind, RoomHistory};
use super::matrix::HttpMatrix;
use super::test_http::{Asked, Reply, forge_of};

pub(super) const TOKEN: &str = "syt_reader_9f8e7d6c5b4a";

/// A room as the fake server holds it: its state events and its timeline.
pub(super) struct Room {
    pub state: Vec<Value>,
    pub timeline: Vec<Value>,
}

pub(super) type Rooms = Arc<Mutex<BTreeMap<String, Room>>>;

pub(super) fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            out.push(u8::from_str_radix(&raw[at + 1..at + 3], 16).unwrap());
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

pub(super) fn message(id: &str, sender: &str, ts: u64, body: &str) -> Value {
    json!({ "type": "m.room.message", "event_id": id, "sender": sender, "origin_server_ts": ts, "content": { "msgtype": "m.text", "body": body } })
}

pub(super) fn state(kind: &str, content: Value) -> Value {
    json!({ "type": kind, "state_key": "", "content": content })
}

/// A Synapse over `rooms`, joined to the rooms in the map: GET only, bearer required.
pub(super) fn synapse(rooms: &Rooms) -> (String, Asked) {
    let rooms = Arc::clone(rooms);
    forge_of(move |raw| {
        let (path, query) = raw.split_once('?').unwrap_or((raw, ""));
        let path = decode(path);
        let rooms = rooms.lock().unwrap();
        let rest = path.strip_prefix("/_matrix/client/v3/")?;
        if rest == "joined_rooms" {
            return Some(Reply::from(json!({ "joined_rooms": rooms.keys().collect::<Vec<_>>() }).to_string()));
        }
        let rest = rest.strip_prefix("rooms/")?;
        let (id, rest) = rest.split_once('/')?;
        let Some(room) = rooms.get(id) else { return Some(Reply::status(404, r#"{"errcode":"M_NOT_FOUND"}"#)) };
        let query: BTreeMap<String, String> = query.split('&').filter_map(|pair| pair.split_once('=')).map(|(k, v)| (k.to_owned(), decode(v))).collect();
        if rest == "state" {
            return Some(Reply::from(Value::Array(room.state.clone()).to_string()));
        }
        if let Some(event) = rest.strip_prefix("context/") {
            return Some(match room.timeline.iter().any(|held| held["event_id"] == event) {
                true => Reply::from(json!({ "end": format!("t_{event}"), "events_before": [], "events_after": [] }).to_string()),
                false => Reply::status(404, r#"{"errcode":"M_NOT_FOUND"}"#),
            });
        }
        if rest == "messages" {
            let start = match query.get("from") {
                Some(from) => 1 + room.timeline.iter().position(|held| format!("t_{}", held["event_id"].as_str().unwrap()) == *from)?,
                None => 0,
            };
            let limit: usize = query["limit"].parse().unwrap();
            let chunk: Vec<&Value> = room.timeline.iter().skip(start).take(limit).collect();
            let end = chunk.last().map(|last| format!("t_{}", last["event_id"].as_str().unwrap()));
            return Some(Reply::from(json!({ "chunk": chunk, "end": end }).to_string()));
        }
        None
    })
}

fn one_room(state: Vec<Value>, timeline: Vec<Value>) -> Rooms {
    Arc::new(Mutex::new(BTreeMap::from([("!r:s".to_owned(), Room { state, timeline })])))
}

#[test]
fn state_maps_to_facts_and_the_join_rule_is_not_read() {
    let plain = one_room(vec![state("m.room.join_rules", json!({ "join_rule": "invite" }))], vec![]);
    let (base, _) = synapse(&plain);
    let facts = HttpMatrix::new(&base, TOKEN).facts("!r:s").unwrap();
    assert_eq!((facts.alias, facts.name, facts.encrypted, facts.direct, facts.bridge), (None, None, false, false, None));
    for rule in ["public", "knock", "invite"] {
        let rooms = one_room(vec![state("m.room.join_rules", json!({ "join_rule": rule }))], vec![]);
        let (base, _) = synapse(&rooms);
        let facts = HttpMatrix::new(&base, TOKEN).facts("!r:s").unwrap();
        assert!(!facts.direct && !facts.encrypted && facts.bridge.is_none(), "{rule}");
    }
    let full = one_room(
        vec![
            state("m.room.canonical_alias", json!({ "alias": "#a:s", "alt_aliases": ["#b:s", "#c:s"] })),
            state("m.room.name", json!({ "name": "Swarm" })),
            state("m.room.encryption", json!({ "algorithm": "m.megolm.v1.aes-sha2" })),
            json!({ "type": "m.room.member", "state_key": "@x:s", "content": { "membership": "join" } }),
            json!({ "type": "m.room.member", "state_key": "@y:s", "content": { "membership": "join", "is_direct": true } }),
            state("uk.half-shot.bridge", json!({})),
        ],
        vec![],
    );
    let (base, _) = synapse(&full);
    let facts = HttpMatrix::new(&base, TOKEN).facts("!r:s").unwrap();
    assert_eq!(facts.alias.as_deref(), Some("#a:s"));
    assert_eq!(facts.alt_aliases, ["#b:s", "#c:s"]);
    assert_eq!(facts.name.as_deref(), Some("Swarm"));
    assert!(facts.encrypted && facts.direct);
    assert_eq!(facts.bridge.as_deref(), Some("uk.half-shot.bridge"));
    let relay = one_room(vec![state("m.bridge", json!({}))], vec![]);
    let (base, _) = synapse(&relay);
    assert_eq!(HttpMatrix::new(&base, TOKEN).facts("!r:s").unwrap().bridge.as_deref(), Some("m.bridge"));
}

#[test]
fn events_map_to_kinds_and_content_keys_are_names_only() {
    let mut other = message("$img", "@a:s", 3, "pic.png");
    other["content"]["msgtype"] = json!("m.image");
    let mut pruned = message("$pruned", "@a:s", 4, "was here");
    pruned["unsigned"] = json!({ "redacted_because": { "event_id": "$x" } });
    let mut notice = message("$n", "@a:s", 5, "n");
    notice["content"]["msgtype"] = json!("m.notice");
    let mut emote = message("$e", "@a:s", 6, "e");
    emote["content"]["msgtype"] = json!("m.emote");
    let edit = json!({ "type": "m.room.message", "event_id": "$edit", "sender": "@a:s", "origin_server_ts": 7, "content": {
        "msgtype": "m.text", "body": "* new", "m.new_content": { "msgtype": "m.text", "body": "new" }, "m.relates_to": { "rel_type": "m.replace", "event_id": "$m1" } } });
    let redaction = json!({ "type": "m.room.redaction", "event_id": "$red", "sender": "@a:s", "origin_server_ts": 8, "content": { "redacts": "$m1" } });
    let old_redaction = json!({ "type": "m.room.redaction", "event_id": "$red2", "sender": "@a:s", "origin_server_ts": 9, "redacts": "$m2", "content": {} });
    let encrypted = json!({ "type": "m.room.encrypted", "event_id": "$enc", "sender": "@a:s", "origin_server_ts": 10, "content": { "ciphertext": "xx" } });
    let timeline = vec![message("$m1", "@a:s", 1, "hello"), other, pruned, notice, emote, edit, redaction, old_redaction, encrypted];
    let rooms = one_room(vec![], timeline);
    let (base, _) = synapse(&rooms);
    let page = HttpMatrix::new(&base, TOKEN).events_after("!r:s", None, 50).unwrap().unwrap();
    let kinds: Vec<EventKind> = page.events.iter().map(|event| event.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            EventKind::Message { body: "hello".into() },
            EventKind::Other,
            EventKind::Other,
            EventKind::Message { body: "n".into() },
            EventKind::Message { body: "e".into() },
            EventKind::Edit { target: "$m1".into(), body: "new".into() },
            EventKind::Redaction { target: "$m1".into() },
            EventKind::Redaction { target: "$m2".into() },
            EventKind::Other,
        ]
    );
    assert_eq!(page.events[0].content_keys, ["body", "msgtype"]);
    assert_eq!(page.events[5].content_keys, ["body", "m.new_content", "m.relates_to", "msgtype"], "names, never values");
    assert_eq!((page.events[0].event_id.as_str(), page.events[0].sender.as_str(), page.events[0].ts_ms), ("$m1", "@a:s", 1));
}

#[test]
fn resume_uses_the_cursor_events_end_token_and_a_missing_event_is_none() {
    let rooms = one_room(vec![], vec![message("$1", "@a:s", 1, "a"), message("$2", "@a:s", 2, "b"), message("$3", "@a:s", 3, "c")]);
    let (base, asked) = synapse(&rooms);
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    let page = matrix.events_after("!r:s", Some("$1"), 1).unwrap().unwrap();
    assert_eq!(page.events.iter().map(|event| event.event_id.as_str()).collect::<Vec<_>>(), ["$2"]);
    assert!(page.more);
    assert_eq!(asked.paths(), ["/_matrix/client/v3/rooms/%21r%3As/context/%241?limit=0", "/_matrix/client/v3/rooms/%21r%3As/messages?dir=f&limit=1&from=t_%241"]);
    assert!(matrix.events_after("!r:s", Some("$unknown"), 5).unwrap().is_none());
    let tail = matrix.events_after("!r:s", Some("$3"), 5).unwrap().unwrap();
    assert!(tail.events.is_empty() && !tail.more, "an empty chunk ends the run");
    let first = matrix.events_after("!r:s", None, 2).unwrap().unwrap();
    assert_eq!(first.events.len(), 2);
    assert_eq!(asked.paths().last().unwrap(), "/_matrix/client/v3/rooms/%21r%3As/messages?dir=f&limit=2", "no from, no context call");
}

#[test]
fn only_get_always_the_bearer_and_the_token_is_never_printed() {
    let rooms = one_room(vec![], vec![message("$1", "@a:s", 1, "a")]);
    let (base, asked) = synapse(&rooms);
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    matrix.joined_rooms().unwrap();
    matrix.facts("!r:s").unwrap();
    matrix.events_after("!r:s", Some("$1"), 3).unwrap();
    let seen = asked.seen();
    assert!(seen.len() >= 4 && seen.iter().all(|seen| seen.method == "GET" && seen.authorization.as_deref() == Some(&format!("Bearer {TOKEN}"))), "{seen:?}");
    let (base, _) = forge_of(|_| Some(Reply::status(401, &format!(r#"{{"errcode":"M_UNKNOWN_TOKEN","error":"bad {TOKEN}"}}"#))));
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    let error = matrix.joined_rooms().unwrap_err();
    assert!(format!("{error:#}").contains("401"));
    assert!(!format!("{error:#}{error:?}").contains(TOKEN));
    assert!(!format!("{matrix:?}").contains(TOKEN));
}

#[test]
fn ids_are_percent_encoded_in_paths() {
    let weird = "$a+b/c=d:s";
    let rooms: Rooms = Arc::new(Mutex::new(BTreeMap::from([("!ro om:s".to_owned(), Room { state: vec![], timeline: vec![message(weird, "@a:s", 1, "x"), message("$2", "@a:s", 2, "y")] })])));
    let (base, asked) = synapse(&rooms);
    let mut matrix = HttpMatrix::new(&base, TOKEN);
    let page = matrix.events_after("!ro om:s", Some(weird), 5).unwrap().unwrap();
    assert_eq!(page.events.len(), 1, "the server decoded the room and event ids to the originals");
    let raw = asked.paths();
    assert!(raw.iter().all(|path| !path.contains('+') && !path.contains(' ') && !path.contains("!ro")), "{raw:?}");
}
