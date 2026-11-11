//! Chat ingest against a scripted room history, a fake vector store and the fake embedder.
//! The refusals come first: what must never be read, embedded or stored.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::{Result, bail};
use huginn_mind::ChatMessage;
use huginn_mind::redact::ScrubKind;

use super::chat::*;
use super::chat_chunk::{CHAT_BODY_CHARS_MAX, chat_version, chunk_id};
use super::chat_purge::{PURGE_WINDOWS_MAX, PurgeWindow, request_purge};
use super::room_scope::{ESTATE_ALIASES, Estate, Refusal, RoomFacts};
use super::*;
use crate::index::MANAGED_BY;
use crate::index::fakes::{FakeEmbedder, FakeSources};

// ---- the rig ----

fn plain(id: &str) -> RoomFacts {
    RoomFacts { room_id: id.into(), alias: None, alt_aliases: vec![], name: None, encrypted: false, direct: false, bridge: None }
}

fn estate(ids: &[&str]) -> Estate {
    Estate::new(ids.iter().chain(["!pad1:s", "!pad2:s", "!pad3:s"].iter()).take(3).map(|id| (*id).to_owned()).collect()).unwrap()
}

struct Scripted {
    facts: RoomFacts,
    events: Vec<RoomEvent>,
}

#[derive(Default)]
struct FakeHistory {
    joined: Vec<String>,
    rooms: BTreeMap<String, Scripted>,
    calls: Vec<String>,
    /// Rooms whose state cannot be read.
    facts_fail: BTreeSet<String>,
    /// Serve an event a redaction targets as the server does, without its content.
    prune: bool,
}

impl FakeHistory {
    fn room(&mut self, id: &str, events: Vec<RoomEvent>) -> &mut Self {
        self.rooms.insert(id.into(), Scripted { facts: plain(id), events });
        self.joined.push(id.into());
        self
    }

    fn facts(&mut self, id: &str, edit: impl FnOnce(&mut RoomFacts)) -> &mut Self {
        edit(&mut self.rooms.get_mut(id).unwrap().facts);
        self
    }

    fn push(&mut self, id: &str, events: Vec<RoomEvent>) {
        self.rooms.get_mut(id).unwrap().events.extend(events);
    }

    fn called(&self, prefix: &str, room: &str) -> bool {
        self.calls.iter().any(|call| call == &format!("{prefix}:{room}"))
    }
}

impl RoomHistory for FakeHistory {
    fn joined_rooms(&mut self) -> Result<Vec<String>> {
        self.calls.push("joined".into());
        Ok(self.joined.clone())
    }

    fn facts(&mut self, room_id: &str) -> Result<RoomFacts> {
        self.calls.push(format!("facts:{room_id}"));
        if self.facts_fail.contains(room_id) {
            bail!("the server did not answer for the state of {room_id}");
        }
        match self.rooms.get(room_id) {
            Some(room) => Ok(room.facts.clone()),
            None => bail!("no such room"),
        }
    }

    fn events_after(&mut self, room_id: &str, after: Option<&str>, limit: u32) -> Result<Option<EventPage>> {
        self.calls.push(format!("events:{room_id}"));
        let Some(room) = self.rooms.get(room_id) else { bail!("no such room") };
        let from = match after {
            None => 0,
            Some(after) => match room.events.iter().position(|event| event.event_id == after) {
                Some(at) => at + 1,
                None => return Ok(None),
            },
        };
        let redacted: BTreeSet<&str> = room.events.iter().filter_map(|event| if let EventKind::Redaction { target } = &event.kind { Some(target.as_str()) } else { None }).collect();
        let served: Vec<RoomEvent> = room.events[from..]
            .iter()
            .map(|event| if self.prune && redacted.contains(event.event_id.as_str()) { RoomEvent { kind: EventKind::Other, content_keys: vec![], ..event.clone() } } else { event.clone() })
            .collect();
        let take = served.len().min(limit as usize);
        Ok(Some(EventPage { events: served[..take].to_vec(), more: take < served.len() }))
    }
}

fn event(id: &str, sender: &str, ts_ms: u64, kind: EventKind) -> RoomEvent {
    RoomEvent { event_id: id.into(), sender: sender.into(), ts_ms, content_keys: vec!["body".into(), "msgtype".into()], kind }
}

fn said(id: &str, ts_ms: u64, body: &str) -> RoomEvent {
    event(id, "@a:s", ts_ms, EventKind::Message { body: body.into() })
}

fn edit(id: &str, sender: &str, ts_ms: u64, target: &str, body: &str) -> RoomEvent {
    event(id, sender, ts_ms, EventKind::Edit { target: target.into(), body: body.into() })
}

fn redaction(id: &str, ts_ms: u64, target: &str) -> RoomEvent {
    event(id, "@a:s", ts_ms, EventKind::Redaction { target: target.into() })
}

struct Rig {
    embedder: FakeEmbedder,
    index: FakeSources,
    estate: Estate,
}

impl Rig {
    fn new() -> Self {
        let mut embedder = FakeEmbedder::new("d1");
        let index = FakeSources::default();
        let identity = crate::index::Embedder::model_identity(&mut embedder, Duration::from_secs(1)).unwrap();
        let meta = SourcesMeta {
            managed_by: MANAGED_BY.into(),
            corpus: CORPUS.into(),
            model: identity.name,
            model_digest: identity.digest,
            dimensions: identity.dimensions,
            chunker_version: chunk::CHUNKER_VERSION,
        };
        index.plant(DescribedSources::Labelled { meta, cursors: BTreeMap::new() });
        Self { embedder, index, estate: estate(&[]) }
    }

    fn run(&mut self, history: &mut FakeHistory) -> (Result<ChatRefreshed>, Vec<String>) {
        let mut journal = Vec::new();
        let out = refresh_chat(&mut self.embedder, &mut self.index, history, &self.estate, &mut |line| journal.push(line));
        (out, journal)
    }

    fn ok(&mut self, history: &mut FakeHistory) -> ChatRefreshed {
        self.run(history).0.unwrap()
    }

    fn points(&self) -> Vec<ChatPoint> {
        self.index.state.lock().unwrap().chat.values().cloned().collect()
    }

    fn ids(&self) -> BTreeSet<String> {
        self.index.state.lock().unwrap().chat.keys().cloned().collect()
    }

    fn bodies(&self) -> Vec<String> {
        let mut bodies: Vec<String> = self.points().iter().flat_map(|point| point.payload.messages.iter().map(|message| message.body.clone())).collect();
        bodies.sort();
        bodies
    }

    fn held_ids(&self, room: &str) -> Vec<String> {
        let mut ids: Vec<String> =
            self.points().iter().filter(|point| point.payload.room_id == room).flat_map(|point| point.payload.messages.iter().map(|message| message.event_id.clone())).collect();
        ids.sort();
        ids
    }

    fn cursor(&self, room: &str) -> Option<String> {
        self.index.cursors().get(&cursor_key(room)).cloned()
    }

    fn embedded(&self) -> Vec<String> {
        self.embedder.texts()
    }

    fn seed(&mut self, room: &str, event_id: &str, ts: u64) {
        let mut seeded = vec![ChatPoint {
            id: chunk_id(room, event_id),
            vector: vec![1.0; 4],
            payload: chunk_payload_of(room, &[(event_id, ts)]),
        }];
        self.index.upsert_chat(&seeded.split_off(0)).unwrap();
        let mut cursors = self.index.cursors();
        cursors.insert(cursor_key(room), format!("{}:{event_id}", chat_version()));
        self.index.set_cursors(&cursors).unwrap();
    }
}

fn chunk_payload_of(room: &str, messages: &[(&str, u64)]) -> ChatPayload {
    let messages: Vec<ChatMessage> = messages.iter().map(|(id, ts)| ChatMessage { event_id: (*id).into(), sender: "@a:s".into(), ts_ms: *ts, body: format!("seeded {id}") }).collect();
    ChatPayload { room_id: room.into(), room_alias: None, text: "seeded".into(), messages }
}

fn tombstone(rig: &Rig, room: &str) -> Option<String> {
    rig.index.cursors().get(&format!("{CHAT_PURGE_PREFIX}{room}")).cloned()
}

// ---- scope: refusals first ----

#[test]
fn only_joined_rooms_are_looked_at_and_a_join_rule_is_not_the_test() {
    let mut history = FakeHistory::default();
    history.room("!a:s", vec![said("$a1", 1, "in a")]).room("!b:s", vec![said("$b1", 2, "in b")]);
    // A room the reader has not joined: scripted, but never listed.
    history.rooms.insert("!c:s".into(), Scripted { facts: plain("!c:s"), events: vec![said("$c1", 3, "in c")] });
    let mut rig = Rig::new();
    let done = rig.ok(&mut history);
    assert!(done.is_clean());
    assert!(!history.calls.iter().any(|call| call.contains("!c:s")), "{:?}", history.calls);
    assert_eq!(rig.held_ids("!a:s"), ["$a1"]);
    assert_eq!(rig.held_ids("!b:s"), ["$b1"]);
    assert!(rig.held_ids("!c:s").is_empty() && done.rooms.iter().all(|report| report.room_id != "!c:s"));
    let code = include_str!("room_scope.rs");
    assert!(!code[..code.find("#[cfg(test)]").unwrap()].contains(&["join", "_rule"].concat()));
}

#[test]
fn a_dm_room_is_refused_unread_unindexed_and_purged() {
    let mut history = FakeHistory::default();
    history.room("!dm:s", vec![said("$d1", 1, "private")]).facts("!dm:s", |facts| facts.direct = true);
    let mut rig = Rig::new();
    rig.seed("!dm:s", "$old", 5);
    let (done, journal) = rig.run(&mut history);
    let done = done.unwrap();
    assert!(!history.called("events", "!dm:s"));
    assert!(rig.points().is_empty() && rig.cursor("!dm:s").is_none() && rig.embedded().is_empty());
    assert_eq!(done.refused, [RefusedRoom { room_id: "!dm:s".into(), reason: Refusal::Direct, event_id: None }]);
    assert!(!done.is_clean() && journal.iter().any(|line| line == "refused: !dm:s direct room"), "{journal:?}");
}

#[test]
fn each_estate_room_is_refused_by_id_and_by_alias_and_an_excluded_room_in_the_index_is_purged() {
    let mut rig = Rig::new();
    rig.estate = estate(&["!byid:s"]);
    let mut history = FakeHistory::default();
    history.room("!byid:s", vec![said("$i1", 1, "x")]);
    for (n, alias) in ESTATE_ALIASES.iter().map(|alias| alias.to_string()).chain(["#USERS:GAMECULT.ORG".to_owned()]).enumerate() {
        history.room(&format!("!al{n}:s"), vec![said("$x", 1, "x")]).facts(&format!("!al{n}:s"), |facts| facts.alias = Some(alias));
    }
    history.room("!alt:s", vec![said("$x", 1, "x")]).facts("!alt:s", |facts| facts.alt_aliases = vec!["#other:s".into(), "#patrons:gamecult.org".into()]);
    history.room("!ok:s", vec![said("$ok1", 1, "fine")]);
    history.room("!guide:s", vec![said("$g1", 1, "fine")]).facts("!guide:s", |facts| facts.alias = Some("#users-guide:gamecult.org".into()));
    history.room("!club:s", vec![said("$c1", 1, "fine")]).facts("!club:s", |facts| facts.alias = Some("#patrons-club:gamecult.org".into()));
    for room in ["!byid:s", "!al0:s", "!al1:s", "!al2:s", "!al3:s", "!alt:s"] {
        rig.seed(room, "$old", 5);
    }
    let mut cursors = rig.index.cursors();
    cursors.insert(format!("{CHAT_PURGE_PREFIX}!al0:s"), "1..2".into());
    rig.index.set_cursors(&cursors).unwrap();
    let done = rig.ok(&mut history);
    assert!(!history.called("facts", "!byid:s") && !history.called("events", "!byid:s"));
    for room in ["!al0:s", "!al1:s", "!al2:s", "!al3:s", "!alt:s"] {
        assert!(history.called("facts", room) && !history.called("events", room), "{room}");
    }
    let refused: BTreeSet<&str> = done.refused.iter().map(|refused| refused.room_id.as_str()).collect();
    assert_eq!(refused, ["!al0:s", "!al1:s", "!al2:s", "!al3:s", "!alt:s", "!byid:s"].into());
    assert!(done.refused.iter().all(|refused| refused.reason == Refusal::Estate) && !done.is_clean());
    let held: BTreeSet<String> = rig.points().iter().map(|point| point.payload.room_id.clone()).collect();
    assert_eq!(held, ["!guide:s".to_owned(), "!club:s".to_owned(), "!ok:s".to_owned()].into(), "lookalikes and plain rooms are in; every estate room is out");
    assert!(rig.index.cursors().keys().all(|key| !key.starts_with("chat:!al") && !key.starts_with("chat:!byid") && !key.starts_with("chat:!alt")));
    assert_eq!(tombstone(&rig, "!al0:s").as_deref(), Some("1..2"), "a tombstone is never touched by a refusal");
}

#[test]
fn an_encrypted_room_is_refused_and_an_encrypted_event_in_a_plain_room_is_never_stored() {
    let mut history = FakeHistory::default();
    history.room("!enc:s", vec![said("$e1", 1, "x")]).facts("!enc:s", |facts| facts.encrypted = true);
    history.room("!plain:s", vec![said("$p1", 1, "visible"), event("$p2", "@a:s", 2, EventKind::Other), said("$p3", 3, "also visible")]);
    let mut rig = Rig::new();
    let done = rig.ok(&mut history);
    assert_eq!(done.refused[0].reason, Refusal::Encrypted);
    assert!(!history.called("events", "!enc:s"));
    assert_eq!(rig.held_ids("!plain:s"), ["$p1", "$p3"]);
    assert_eq!(rig.cursor("!plain:s").unwrap(), format!("{}:$p3", chat_version()), "the cursor passes an Other event");
}

#[test]
fn a_bridged_room_is_refused_whole_before_any_event_is_read_and_its_chunks_are_purged() {
    let mut history = FakeHistory::default();
    for (n, state_type) in ["m.bridge", "uk.half-shot.bridge"].into_iter().enumerate() {
        let room = format!("!br{n}:s");
        history.room(&room, vec![said("$x", 1, "x")]).facts(&room, |facts| facts.bridge = Some(state_type.into()));
    }
    let mut rig = Rig::new();
    rig.seed("!br0:s", "$old", 5);
    let done = rig.ok(&mut history);
    assert_eq!(done.refused.len(), 2);
    assert!(done.refused.iter().all(|refused| matches!(refused.reason, Refusal::Bridged(_))));
    assert!(!history.calls.iter().any(|call| call.starts_with("events:")));
    assert!(rig.points().is_empty() && rig.index.cursors().is_empty());
}

#[test]
fn a_bridged_platforms_sender_or_content_refuses_the_room_before_anything_is_embedded_and_purges_what_it_held() {
    let cases = [
        event("$bad", "@discord_123:s", 2, EventKind::Message { body: "SECRET-BODY-TEXT".into() }),
        event("$bad", "@_discord_9:s", 2, EventKind::Message { body: "SECRET-BODY-TEXT".into() }),
        event("$bad", "@discordbot:s", 2, EventKind::Message { body: "SECRET-BODY-TEXT".into() }),
        event("$bad", "@mautrix_discord:s", 2, EventKind::Message { body: "SECRET-BODY-TEXT".into() }),
        event("$bad", "@_bridge_discordbot:s", 2, EventKind::Message { body: "SECRET-BODY-TEXT".into() }),
        RoomEvent { content_keys: vec!["body".into(), "fi.mau.discord".into()], ..said("$bad", 2, "SECRET-BODY-TEXT") },
        RoomEvent { content_keys: vec!["uk.half-shot.discord".into()], ..said("$bad", 2, "SECRET-BODY-TEXT") },
    ];
    for bad in cases {
        let mut history = FakeHistory::default();
        history.room("!r:s", vec![said("$ok", 1, "before"), bad]);
        history.room("!other:s", vec![said("$o1", 1, "other room")]);
        let mut rig = Rig::new();
        rig.seed("!r:s", "$prev", 0);
        let (done, journal) = rig.run(&mut history);
        let done = done.unwrap();
        assert!(!rig.embedded().iter().any(|text| text.contains("SECRET-BODY-TEXT") || text.contains("before")), "nothing of the page is embedded");
        assert!(rig.held_ids("!r:s").is_empty() && rig.cursor("!r:s").is_none());
        assert_eq!(rig.held_ids("!other:s"), ["$o1"]);
        assert_eq!(done.refused.len(), 1);
        assert_eq!((done.refused[0].room_id.as_str(), done.refused[0].event_id.as_deref()), ("!r:s", Some("$bad")));
        assert!(!done.is_clean());
        assert!(journal.iter().all(|line| !line.contains("SECRET-BODY-TEXT")), "{journal:?}");
    }
}

#[test]
fn a_lookalike_of_the_bridged_platform_is_kept() {
    let mut history = FakeHistory::default();
    let kept = ["@discourse:s", "@disco:s", "@eureka:s"].map(|sender| event(&format!("${}", &sender[1..6]), sender, 1, EventKind::Message { body: "hi".into() }));
    let mut keyed = said("$k", 2, "hi");
    keyed.content_keys.push("discourse".into());
    let mut events = kept.to_vec();
    events.push(keyed);
    history.room("!r:s", events);
    let mut rig = Rig::new();
    assert!(rig.ok(&mut history).is_clean());
    assert_eq!(rig.held_ids("!r:s").len(), 4);
}

#[test]
fn the_collection_is_never_created_by_chat() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 1, "x")]);
    for described in [None, Some(DescribedSources::Absent), Some(DescribedSources::Unlabelled)] {
        let mut rig = Rig::new();
        match described {
            None => rig.index.state.lock().unwrap().described = None,
            Some(described) => rig.index.plant(described),
        }
        let error = format!("{:#}", rig.run(&mut history).0.unwrap_err());
        assert!(error.contains("forge refresh creates it"), "{error}");
        assert_eq!(rig.index.state.lock().unwrap().recreated, 0);
        assert!(rig.points().is_empty() && rig.embedded().is_empty());
    }
}

// ---- redactions, edits, replays ----

#[test]
fn a_redacted_event_is_absent_after_the_next_refresh_and_a_first_or_only_message_changes_or_drops_its_chunk() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "alpha"), said("$b", 20, "beta"), said("$c", 30, "gamma")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let first_id = chunk_id("!r:s", "$a");
    assert_eq!(rig.ids(), [first_id.clone()].into());
    history.push("!r:s", vec![redaction("$r1", 40, "$b")]);
    rig.ok(&mut history);
    assert_eq!(rig.held_ids("!r:s"), ["$a", "$c"]);
    assert!(!rig.points()[0].payload.text.contains("beta"));
    history.push("!r:s", vec![redaction("$r2", 50, "$a")]);
    rig.ok(&mut history);
    assert_eq!(rig.ids(), [chunk_id("!r:s", "$c")].into(), "the first message left, so the id changed and the old one is gone");
    history.push("!r:s", vec![redaction("$r3", 60, "$c")]);
    rig.ok(&mut history);
    assert!(rig.points().is_empty(), "a chunk the redaction empties is deleted");
}

#[test]
fn a_redaction_in_the_same_page_as_its_message_never_reaches_the_embedder() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "keep me"), said("$gone", 20, "REDACTED-BODY"), redaction("$r", 30, "$gone")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert!(rig.embedded().iter().all(|text| !text.contains("REDACTED-BODY")));
    assert_eq!(rig.held_ids("!r:s"), ["$a"]);
}

#[test]
fn an_edit_replaces_the_body_in_place_and_a_stranger_an_unknown_target_or_an_edit_of_an_edit_change_nothing() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "old words"), said("$b", 20, "other")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let before = rig.points();
    history.push("!r:s", vec![edit("$e1", "@a:s", 30, "$a", "new words")]);
    rig.ok(&mut history);
    let after = rig.points();
    assert_eq!(after.len(), 1);
    let first = &after[0].payload.messages[0];
    assert_eq!((first.event_id.as_str(), first.sender.as_str(), first.ts_ms, first.body.as_str()), ("$a", "@a:s", 10, "new words"));
    assert_eq!(after[0].id, before[0].id);
    assert!(after[0].payload.text.contains("new words") && !after[0].payload.text.contains("old words"));
    assert!(rig.embedded().last().unwrap().contains("new words"));
    let calls = rig.embedder.state.lock().unwrap().embedded.len();
    history.push("!r:s", vec![edit("$e2", "@stranger:s", 40, "$a", "hijack"), edit("$e3", "@a:s", 41, "$nothing", "lost"), edit("$e4", "@a:s", 42, "$e1", "edit of an edit")]);
    rig.ok(&mut history);
    assert_eq!(rig.points(), after);
    assert_eq!(rig.embedder.state.lock().unwrap().embedded.len(), calls, "no embed call for edits that change nothing");
    assert_eq!(rig.cursor("!r:s").unwrap(), format!("{}:$e4", chat_version()), "the cursor is the last event of any kind");
}

#[test]
fn replay_after_a_crash_between_the_upsert_and_the_cursor_neither_duplicates_nor_loses() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "alpha"), said("$b", 20, "beta"), edit("$e", "@a:s", 25, "$a", "alpha two"), said("$c", 30, "gamma")]);
    let mut rig = Rig::new();
    // One cursor write (the empty start) succeeds, then the page's upsert is acknowledged and the next write fails.
    rig.index.state.lock().unwrap().fail_cursor_writes_after = Some(1);
    assert!(!rig.run(&mut history).0.unwrap().failed.is_empty());
    let (ids, held) = (rig.ids(), rig.held_ids("!r:s"));
    assert_eq!(held, ["$a", "$b", "$c"]);
    rig.index.state.lock().unwrap().fail_cursor_writes_after = None;
    let done = rig.ok(&mut history);
    assert!(done.is_clean());
    assert_eq!((rig.ids(), rig.held_ids("!r:s")), (ids, held));
    assert!(rig.bodies().contains(&"alpha two".to_owned()));
    assert_eq!(rig.cursor("!r:s").unwrap(), format!("{}:$c", chat_version()));
}

#[test]
fn a_failed_upsert_leaves_the_cursor_and_a_failed_embed_or_delete_deletes_nothing_the_index_held() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "alpha"), said("$b", 20, "beta")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let (cursor, points) = (rig.cursor("!r:s"), rig.points());
    history.push("!r:s", vec![said("$c", 30, "gamma")]);
    rig.index.state.lock().unwrap().fail_chat_upserts = true;
    assert_eq!(rig.run(&mut history).0.unwrap().failed, ["!r:s"]);
    assert_eq!((rig.cursor("!r:s"), rig.points()), (cursor.clone(), points.clone()));
    rig.index.state.lock().unwrap().fail_chat_upserts = false;
    rig.embedder.state.lock().unwrap().embed_fails = true;
    assert_eq!(rig.run(&mut history).0.unwrap().failed, ["!r:s"]);
    assert_eq!((rig.cursor("!r:s"), rig.points()), (cursor.clone(), points.clone()));
    rig.embedder.state.lock().unwrap().embed_fails = false;
    // A redaction of the first message writes a new chunk, then fails to delete the old one: the cursor stays.
    history.push("!r:s", vec![redaction("$r", 40, "$a")]);
    rig.index.state.lock().unwrap().fail_chat_deletes = true;
    assert_eq!(rig.run(&mut history).0.unwrap().failed, ["!r:s"]);
    assert_eq!(rig.cursor("!r:s"), cursor);
    rig.index.state.lock().unwrap().fail_chat_deletes = false;
    assert!(rig.ok(&mut history).is_clean());
    assert_eq!(rig.held_ids("!r:s"), ["$b", "$c"]);
}

// ---- chunking through the job ----

#[test]
fn appending_to_the_tail_over_many_runs_equals_one_run_over_all() {
    let all: Vec<RoomEvent> = (0..80).map(|n| said(&format!("$m{n:03}"), 1_000 * n + if n >= 40 { 5_000_000 } else { 0 }, &format!("message number {n} {}", "x".repeat((n as usize * 37) % 400)))).collect();
    let mut whole_history = FakeHistory::default();
    whole_history.room("!r:s", all.clone());
    let mut whole = Rig::new();
    whole.ok(&mut whole_history);
    let mut grown_history = FakeHistory::default();
    grown_history.room("!r:s", vec![]);
    let mut grown = Rig::new();
    for batch in all.chunks(7) {
        grown_history.push("!r:s", batch.to_vec());
        grown.ok(&mut grown_history);
    }
    assert!(whole.points().len() > 2);
    let shape = |rig: &Rig| rig.points().into_iter().map(|point| (point.id, point.payload)).collect::<Vec<_>>();
    assert_eq!(shape(&grown), shape(&whole));
}

#[test]
fn a_long_message_is_cut_with_a_marker_and_every_point_satisfies_the_payload_contract() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, &"é".repeat(CHAT_BODY_CHARS_MAX * 2)), said("$b", 20, "short"), said("$c", 99_000_000, "later")]);
    history.facts("!r:s", |facts| {
        facts.alias = Some("#swarm:gamecult.org".into());
        facts.name = Some("Swarm".into());
    });
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let points = rig.points();
    assert_eq!(points.len(), 2);
    for point in &points {
        let payload = &point.payload;
        assert_eq!((payload.room_id.as_str(), payload.room_alias.as_deref()), ("!r:s", Some("#swarm:gamecult.org")));
        assert_eq!(point.id, chunk_id("!r:s", &payload.messages[0].event_id));
        assert_eq!(payload.text, super::chat_chunk::embedded_text("Swarm", &payload.messages));
        assert!(!payload.messages.is_empty());
    }
    let long = &points.iter().find(|point| point.payload.messages[0].event_id == "$a").unwrap().payload.messages[0];
    assert_eq!(long.body.chars().count(), CHAT_BODY_CHARS_MAX);
    assert!(long.body.ends_with("[cut]"));
}

#[test]
fn a_lost_cursor_event_or_a_version_bump_re_ingests_that_room_alone() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 10, "alpha"), said("$b", 20, "beta")]).room("!q:s", vec![said("$q1", 10, "quiet")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let q_ids: BTreeSet<String> = rig.points().iter().filter(|point| point.payload.room_id == "!q:s").map(|point| point.id.clone()).collect();
    let recreated = rig.index.state.lock().unwrap().recreated;
    for bump in [Some("0.0".to_owned()), None] {
        let mut cursors = rig.index.cursors();
        let value = match &bump {
            Some(old) => format!("{old}:$b"),
            None => format!("{}:$gone-from-the-server", chat_version()),
        };
        cursors.insert(cursor_key("!r:s"), value);
        rig.index.set_cursors(&cursors).unwrap();
        let before = rig.embedded().len();
        assert!(rig.ok(&mut history).is_clean());
        assert_eq!(rig.held_ids("!r:s"), ["$a", "$b"]);
        assert!(rig.embedded().len() > before, "the room was re-embedded");
        assert_eq!(rig.points().iter().filter(|point| point.payload.room_id == "!q:s").map(|point| point.id.clone()).collect::<BTreeSet<_>>(), q_ids);
        assert_eq!(rig.cursor("!r:s").unwrap(), format!("{}:$b", chat_version()));
    }
    assert_eq!(rig.index.state.lock().unwrap().recreated, recreated);
}

#[test]
fn retire_chat_removes_rooms_no_longer_kept_and_given_none_all_chat_and_touches_nothing_else() {
    let mut rig = Rig::new();
    rig.index.upsert_sources(&[SourcePoint {
        id: "forge-1".into(),
        vector: vec![1.0; 4],
        payload: SourcePayload { repo: "Repo".into(), path: "a".into(), commit: "c".into(), blob: "b".into(), line_start: 1, line_end: 1, chunk_index: 0, ext: "rs".into(), text: "t".into() },
    }])
    .unwrap();
    for room in ["!a:s", "!b:s"] {
        rig.seed(room, "$e", 1);
    }
    let mut cursors = rig.index.cursors();
    cursors.insert("Repo".into(), "c".into());
    cursors.insert(format!("{CHAT_PURGE_PREFIX}!a:s"), "1..2".into());
    rig.index.set_cursors(&cursors).unwrap();
    let retired = retire_chat(&mut rig.index, &["!a:s".to_owned()].into()).unwrap();
    assert_eq!(retired.retired, ["!b:s"]);
    assert_eq!(rig.held_ids("!a:s"), ["$e"]);
    assert!(rig.held_ids("!b:s").is_empty() && rig.cursor("!b:s").is_none());
    assert_eq!(retire_chat(&mut rig.index, &BTreeSet::new()).unwrap().retired, ["!a:s"]);
    assert!(rig.points().is_empty());
    assert_eq!(rig.index.state.lock().unwrap().points.len(), 1, "a forge point stays");
    let kept: Vec<String> = rig.index.cursors().into_keys().collect();
    assert_eq!(kept, ["Repo".to_owned(), format!("{CHAT_PURGE_PREFIX}!a:s")]);
    // A room the reader left is retired by the refresh itself.
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 1, "x")]);
    rig.seed("!gone:s", "$e", 1);
    rig.ok(&mut history);
    assert!(rig.held_ids("!gone:s").is_empty());
}

// ---- the credential scrub through the job ----

fn planted() -> Vec<(ScrubKind, String, String)> {
    let t = |kind, secret: String, needle: &str| (kind, secret, needle.to_owned());
    vec![
        t(ScrubKind::AuthorizationHeader, format!("Authorization: Bearer {}", "a1B2".repeat(8)), "a1B2a1B2a1B2"),
        t(ScrubKind::Bearer, format!("Bearer {}", "z9Y8".repeat(6)), "z9Y8z9Y8z9Y8"),
        t(ScrubKind::Bearer, format!("bearer:{}", "z9Y8".repeat(6)), "z9Y8z9Y8z9Y8"),
        t(ScrubKind::KeyShape, format!("sk-{}", "q7W3".repeat(8)), "q7W3q7W3q7W3"),
        t(ScrubKind::KeyShape, format!("sk_live_{}", "m4N5".repeat(6)), "m4N5m4N5m4N5"),
        t(ScrubKind::KeyShape, format!("ghp_{}", "k2L9".repeat(9)), "k2L9k2L9k2L9"),
        t(ScrubKind::KeyShape, format!("xxx-ghp_{}", "k2L9".repeat(9)), "k2L9k2L9k2L9"),
        t(ScrubKind::KeyShape, format!("github_pat_{}_{}", "A1b2C3".repeat(4) + "ab", "d4E5f6".repeat(9) + "abcde"), "A1b2C3A1b2C3"),
        t(ScrubKind::KeyShape, format!("glpat-{}", "r6T7".repeat(5)), "r6T7r6T7r6T7"),
        t(ScrubKind::KeyShape, "xoxb-123456789012-987654321098-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL"),
        t(ScrubKind::KeyShape, "xoxa-2-123456789012-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL"),
        t(ScrubKind::KeyShape, "xoxp-123456789012-987654321098-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL"),
        t(ScrubKind::KeyShape, "xoxr-123456789012-987654321098-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL"),
        t(ScrubKind::KeyShape, "xoxs-123456789012-987654321098-aBcDeFgHiJkLmNoP".into(), "aBcDeFgHiJkL"),
        t(ScrubKind::KeyShape, "AKIA0123456789ABCDEF".into(), "0123456789ABCDEF"),
        t(ScrubKind::KeyShape, "xAKIA0123456789ABCDEF".into(), "0123456789ABCDEF"),
        t(ScrubKind::KeyShape, format!("AIza{}", "Sy1_".repeat(9)), "Sy1_Sy1_Sy1_"),
        t(ScrubKind::KeyShape, format!("syt_{}_{}_{}", "YWxpY2U", "aBcDeFgHiJkLmNoPqR", "0a1B2c"), "aBcDeFgHiJkLmNoPqR"),
        t(ScrubKind::KeyShape, format!("mct_{}", "w4X5".repeat(8)), "w4X5w4X5w4X5"),
        t(ScrubKind::AccessToken, "https://h/_matrix/client/v3/sync?access_token=abcDEF123456xyz&since=s1".into(), "abcDEF123456xyz"),
        t(ScrubKind::AccessToken, "{\"access_token\": \"tokenValue987654\", \"user\": 1}".into(), "tokenValue987654"),
        t(ScrubKind::Jwt, "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJmYWtlIn0.c2lnbmF0dXJlZmFrZQ".into(), "c2lnbmF0dXJlZmFrZQ"),
        t(ScrubKind::PemBlock, "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAfake\n-----END RSA PRIVATE KEY-----".into(), "MIIEowIBAAKCAQEAfake"),
        t(ScrubKind::PemBlock, "-----BEGIN PRIVATE KEY-----\nMIIBVQIBADANfakeNOEND".into(), "MIIBVQIBADANfake"),
        t(ScrubKind::PemBlock, "-----BEGIN PGP PRIVATE KEY BLOCK-----\nlQOYBFakeFakeFake\n-----END PGP PRIVATE KEY BLOCK-----".into(), "lQOYBFakeFake"),
        t(ScrubKind::UrlUserinfo, "https://user:hunter2hunter2@host.example/r.git".into(), "hunter2hunter2"),
        t(ScrubKind::UrlUserinfo, "https://tokenvalue12345@host.example/".into(), "tokenvalue12345"),
    ]
}

#[test]
fn planted_credentials_never_reach_the_chunk_the_embedder_or_the_journal_in_a_message_or_an_edit() {
    for (kind, secret, needle) in planted() {
        for shape in ["see {} now", "see {}", "{}\nnext line"] {
            let (dirty, clean) = (shape.replace("{}", &secret), "nothing here".to_owned());
            let mut history = FakeHistory::default();
            history.room("!r:s", vec![said("$m", 10, &dirty), said("$e", 20, &clean), edit("$ed", "@a:s", 30, "$e", &dirty)]);
            let mut rig = Rig::new();
            let (done, journal) = rig.run(&mut history);
            let done = done.unwrap();
            assert!(done.is_clean(), "a scrub hit does not make a run unclean");
            let stored = format!("{:?}", rig.points().iter().map(|point| (&point.payload.messages, &point.payload.text)).collect::<Vec<_>>());
            for seen in [stored, rig.embedded().join("\n"), journal.join("\n")] {
                assert!(!seen.contains(&needle), "{kind}: {secret:?} leaked into {seen}");
            }
            assert!(rig.bodies().iter().filter(|body| body.contains("<redacted>")).count() >= 2, "{kind}: both bodies show the replacement");
            let hits: Vec<(&str, ScrubKind)> = done.scrubbed.iter().map(|hit| (hit.event_id.as_str(), hit.kind)).collect();
            assert!(hits.contains(&("$m", kind)) && hits.contains(&("$ed", kind)), "{kind}: {hits:?}");
            assert!(done.scrubbed.iter().all(|hit| hit.room_id == "!r:s"));
            assert!(journal.contains(&format!("scrubbed: !r:s $m {kind}")), "{journal:?}");
        }
    }
}

#[test]
fn the_scrub_meets_edits_redactions_the_body_cut_replays_and_version_bumps() {
    let token = format!("ghp_{}", "k2L9".repeat(9));
    let mut history = FakeHistory::default();
    history.room(
        "!r:s",
        vec![
            said("$clean", 10, "all clean"),
            said("$dirty", 20, &format!("token {token} inside")),
            edit("$adds", "@a:s", 30, "$clean", &format!("now with {token}")),
            edit("$removes", "@a:s", 40, "$dirty", "token removed"),
            said("$gone", 50, &format!("secret {token}")),
            redaction("$r", 60, "$gone"),
            said("$straddle", 70, &format!("{} {token} {}", "x".repeat(CHAT_BODY_CHARS_MAX - 10), "y".repeat(100))),
        ],
    );
    let mut rig = Rig::new();
    let done = rig.ok(&mut history);
    assert!(rig.bodies().iter().all(|body| !body.contains("k2L9k2L9")));
    assert!(rig.embedded().iter().all(|text| !text.contains("k2L9k2L9")));
    let by_id: BTreeMap<String, String> = rig.points().iter().flat_map(|point| point.payload.messages.iter().map(|message| (message.event_id.clone(), message.body.clone()))).collect();
    assert_eq!(by_id["$clean"], "now with <redacted>");
    assert_eq!(by_id["$dirty"], "token removed");
    assert!(!by_id.contains_key("$gone"));
    assert!(by_id["$straddle"].chars().count() <= CHAT_BODY_CHARS_MAX && !by_id["$straddle"].contains("ghp_"));
    assert!(done.scrubbed.iter().any(|hit| hit.event_id == "$adds") && done.scrubbed.iter().any(|hit| hit.event_id == "$gone"));
    let (ids, texts) = (rig.ids(), rig.points().into_iter().map(|point| point.payload.text).collect::<Vec<_>>());
    // Replay from the start, as after a lost cursor: identical ids and text.
    let mut cursors = rig.index.cursors();
    cursors.remove(&cursor_key("!r:s"));
    rig.index.set_cursors(&cursors).unwrap();
    rig.ok(&mut history);
    assert_eq!((rig.ids(), rig.points().into_iter().map(|point| point.payload.text).collect::<Vec<_>>()), (ids, texts));
    // An older scrub version re-ingests the room and scrubs it again from the history.
    let mut cursors = rig.index.cursors();
    cursors.insert(cursor_key("!r:s"), format!("{}.0:$straddle", huginn_mind::redact::SCRUB_VERSION - 1));
    rig.index.set_cursors(&cursors).unwrap();
    rig.ok(&mut history);
    assert!(rig.bodies().iter().all(|body| !body.contains("k2L9k2L9")));
}

#[test]
fn ordinary_text_is_stored_byte_for_byte() {
    let mut history = FakeHistory::default();
    let same = "risk-averse task-force sk- 3f2b8c1d9e7a4b5c6d0e1f2a3b4c5d6e7f8a9b0c !abc:s user@example.com ssh://git@host/r.git the bearer of bad news";
    history.room("!r:s", vec![said("$a", 1, same)]);
    let mut rig = Rig::new();
    let done = rig.ok(&mut history);
    assert_eq!(rig.bodies(), [same]);
    assert!(done.scrubbed.is_empty());
}

// ---- steward purge ----

fn window(from_ms: u64, to_ms: u64) -> PurgeWindow {
    PurgeWindow { from_ms, to_ms }
}

fn purge_history() -> FakeHistory {
    let mut history = FakeHistory::default();
    // t1 = 1000, t2 = 3000: $b at t1 and $c inside are purged, $d at t2 is kept.
    history.room("!r:s", vec![said("$a", 500, "before"), said("$b", 1000, "PURGED-B"), said("$c", 2000, "PURGED-C"), said("$d", 3000, "after"), said("$e", 4000, "later")]);
    history.room("!other:s", vec![said("$o1", 1000, "same time other room"), said("$o2", 2000, "same time")]);
    history
}

#[test]
fn a_purge_window_removes_exactly_its_events_and_re_chunks_the_neighbours() {
    let mut history = purge_history();
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let other_ids: BTreeSet<String> = rig.points().iter().filter(|point| point.payload.room_id == "!other:s").map(|point| point.id.clone()).collect();
    assert_eq!(rig.held_ids("!r:s"), ["$a", "$b", "$c", "$d", "$e"]);
    assert!(request_purge(&mut rig.index, "!r:s", window(1000, 3000)).unwrap());
    let embedded_before = rig.embedded().len();
    rig.ok(&mut history);
    assert_eq!(rig.held_ids("!r:s"), ["$a", "$d", "$e"], "t1 is inside, t2 is not");
    assert!(rig.embedded()[embedded_before..].iter().all(|text| !text.contains("PURGED")));
    assert!(rig.points().iter().all(|point| !point.payload.text.contains("PURGED")));
    assert_eq!(rig.points().iter().filter(|point| point.payload.room_id == "!other:s").map(|point| point.id.clone()).collect::<BTreeSet<_>>(), other_ids);
    assert_eq!(rig.held_ids("!other:s"), ["$o1", "$o2"]);
    assert_eq!(tombstone(&rig, "!r:s").as_deref(), Some("1000..3000"));
}

#[test]
fn a_purged_first_message_changes_the_chunk_id_and_an_emptied_chunk_is_deleted() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 1000, "x"), said("$b", 2000, "y"), said("$c", 99_000_000, "far")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert_eq!(rig.ids().len(), 2);
    request_purge(&mut rig.index, "!r:s", window(900, 1500)).unwrap();
    rig.ok(&mut history);
    assert_eq!(rig.ids(), [chunk_id("!r:s", "$b"), chunk_id("!r:s", "$c")].into());
    request_purge(&mut rig.index, "!r:s", window(99_000_000, 99_000_001)).unwrap();
    rig.ok(&mut history);
    assert_eq!(rig.held_ids("!r:s"), ["$b"]);
}

#[test]
fn a_purged_window_does_not_come_back() {
    let mut history = purge_history();
    let mut rig = Rig::new();
    rig.ok(&mut history);
    request_purge(&mut rig.index, "!r:s", window(1000, 3000)).unwrap();
    rig.ok(&mut history);
    let want = ["$a", "$d", "$e"];
    // New events, an in-window edit of an outside message, a redaction.
    history.push("!r:s", vec![said("$f", 5000, "new"), edit("$ed", "@a:s", 2500, "$a", "EDIT-IN-WINDOW"), redaction("$rd", 6000, "$e")]);
    rig.ok(&mut history);
    assert_eq!(rig.held_ids("!r:s"), ["$a", "$d", "$f"]);
    assert!(rig.bodies().contains(&"before".to_owned()) && !rig.bodies().iter().any(|body| body.contains("EDIT-IN-WINDOW")), "the in-window edit is dropped and the original stays");
    // A lost cursor re-ingests from the start; a version bump re-ingests too.
    for cursor in [None, Some("0.0:$f".to_owned())] {
        let mut cursors = rig.index.cursors();
        match cursor {
            None => cursors.remove(&cursor_key("!r:s")),
            Some(old) => cursors.insert(cursor_key("!r:s"), old),
        };
        rig.index.set_cursors(&cursors).unwrap();
        rig.ok(&mut history);
        assert!(rig.bodies().iter().all(|body| !body.contains("PURGED")), "{:?}", rig.held_ids("!r:s"));
        assert_eq!(rig.held_ids("!r:s"), ["$a", "$d", "$f"], "$e stays redacted by the history; {want:?} before the new events");
    }
    // The reader leaves the room and joins again.
    let mut left = FakeHistory::default();
    left.room("!other:s", vec![said("$o1", 1000, "x")]);
    rig.ok(&mut left);
    assert!(rig.held_ids("!r:s").is_empty() && tombstone(&rig, "!r:s").is_some());
    rig.ok(&mut history);
    assert!(rig.bodies().iter().all(|body| !body.contains("PURGED")));
}

#[test]
fn a_purge_of_an_unindexed_window_writes_only_the_tombstone_and_still_holds() {
    let mut rig = Rig::new();
    let writes = rig.index.state.lock().unwrap().cursor_writes.len();
    assert!(request_purge(&mut rig.index, "!never:s", window(10, 20)).unwrap());
    assert_eq!(rig.index.state.lock().unwrap().cursor_writes.len(), writes + 1);
    assert!(rig.points().is_empty() && rig.embedded().is_empty());
    let mut history = FakeHistory::default();
    history.room("!never:s", vec![said("$a", 5, "ok"), said("$b", 15, "PURGED"), said("$c", 25, "ok")]);
    rig.ok(&mut history);
    assert_eq!(rig.held_ids("!never:s"), ["$a", "$c"]);
    assert!(rig.embedded().iter().all(|text| !text.contains("PURGED")));
}

#[test]
fn a_repeated_or_overlapping_request_changes_nothing_or_merges_and_the_33rd_window_is_refused() {
    let mut rig = Rig::new();
    assert!(request_purge(&mut rig.index, "!r:s", window(10, 20)).unwrap());
    let writes = rig.index.state.lock().unwrap().cursor_writes.len();
    assert!(!request_purge(&mut rig.index, "!r:s", window(10, 20)).unwrap());
    assert!(!request_purge(&mut rig.index, "!r:s", window(12, 18)).unwrap());
    assert_eq!(rig.index.state.lock().unwrap().cursor_writes.len(), writes, "no write for a request the tombstone already holds");
    assert!(request_purge(&mut rig.index, "!r:s", window(15, 30)).unwrap());
    assert_eq!(tombstone(&rig, "!r:s").as_deref(), Some("10..30"));
    for n in 0..(PURGE_WINDOWS_MAX as u64 - 1) {
        request_purge(&mut rig.index, "!r:s", window(100 + n * 10, 105 + n * 10)).unwrap();
    }
    let error = format!("{:#}", request_purge(&mut rig.index, "!r:s", window(10_000, 10_001)).unwrap_err());
    assert!(error.contains("32"), "{error}");
    assert!(request_purge(&mut Rig::new().index, "!r:s", window(5, 5)).is_err());
}

// ---- the source stays in its lane ----

#[test]
fn the_ingest_code_is_pure_over_its_ports_and_names_no_credential_shape_no_second_literal_and_no_join_rule() {
    let sources = [("chat.rs", include_str!("chat.rs")), ("chat_chunk.rs", include_str!("chat_chunk.rs")), ("chat_purge.rs", include_str!("chat_purge.rs")), ("room_scope.rs", include_str!("room_scope.rs"))];
    let process = ["Command", "::new"].concat();
    for (name, source) in sources {
        let code = source.split("#[cfg(test)]").next().unwrap();
        assert!(!code.contains(&process) && !code.contains("std::process"), "{name}");
        assert!(!code.to_ascii_lowercase().contains("join_rule"), "{name}");
        if name != "room_scope.rs" {
            assert!(!code.to_ascii_lowercase().contains("discord"), "{name}");
        }
        for shape in ["sk-", "sk_live_", "ghp_", "github_pat_", "glpat-", "AKIA", "xoxb-", "PRIVATE KEY", "Bearer"] {
            assert!(!code.contains(shape), "{name} names {shape}");
        }
        let defined = usize::from(name == "chat.rs");
        assert_eq!((code.matches("\"chat:").count(), code.matches("\"chat-purge:").count()), (defined, defined), "{name}: each prefix is defined once");
    }
    let chat = include_str!("chat.rs");
    assert_eq!(chat.matches("scrub(").count(), 2, "scrub is called for a body and for the room name, nowhere else");
}

// ---- redacted edits, redacted originals, late edits ----

const EDIT_SECRET: &str = "EDIT-SECRET";

fn nothing_secret_stored(rig: &Rig) {
    let stored = format!("{:?}", rig.points().iter().map(|point| (&point.payload.messages, &point.payload.text)).collect::<Vec<_>>());
    assert!(!stored.contains(EDIT_SECRET), "{stored}");
}

#[test]
fn a_redacted_edit_in_the_same_page_reverts_the_message_to_its_earlier_body() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$m", 10, "original words"), edit("$e1", "@a:s", 20, "$m", "first edit"), edit("$e2", "@a:s", 30, "$m", EDIT_SECRET), redaction("$r", 40, "$e2")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert_eq!(rig.bodies(), ["first edit"]);
    assert!(rig.embedded().iter().all(|text| !text.contains(EDIT_SECRET)));
    history.room("!q:s", vec![said("$m", 10, "orig"), edit("$e", "@a:s", 20, "$m", EDIT_SECRET), redaction("$r", 30, "$e")]);
    rig.ok(&mut history);
    assert!(rig.bodies().contains(&"orig".to_owned()));
    nothing_secret_stored(&rig);
}

#[test]
fn a_redaction_of_an_edit_a_later_run_stored_rebuilds_the_room_and_the_secret_does_not_return() {
    let mut history = FakeHistory { prune: true, ..FakeHistory::default() };
    history.room("!r:s", vec![said("$m", 10, "original"), edit("$e", "@a:s", 20, "$m", EDIT_SECRET)]);
    history.room("!q:s", vec![said("$q", 10, "quiet")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert!(rig.bodies().iter().any(|body| body == EDIT_SECRET), "the edit applied while it stood");
    let recreated = rig.index.state.lock().unwrap().recreated;
    history.push("!r:s", vec![redaction("$r", 30, "$e")]);
    let done = rig.ok(&mut history);
    assert!(done.is_clean());
    nothing_secret_stored(&rig);
    assert!(rig.bodies().contains(&"original".to_owned()));
    assert_eq!(rig.held_ids("!q:s"), ["$q"]);
    assert_eq!(rig.index.state.lock().unwrap().recreated, recreated);
    assert_eq!(rig.cursor("!r:s").unwrap(), format!("{}:$r", chat_version()));
    // The rebuild does not loop: the redaction it replays is not a trigger the second time, and nothing changes.
    let ids = rig.ids();
    rig.ok(&mut history);
    assert_eq!(rig.ids(), ids);
}

#[test]
fn a_redacted_original_takes_its_live_and_late_edits_with_it_in_a_page_across_runs_and_across_a_rebuild() {
    let script = |history: &mut FakeHistory| {
        history.room("!r:s", vec![said("$m", 10, "original"), edit("$e1", "@a:s", 20, "$m", "live edit"), redaction("$re", 25, "$e1"), redaction("$rm", 30, "$m"), edit("$late", "@a:s", 40, "$m", EDIT_SECRET)]);
        history.prune = true;
    };
    // One page.
    let (mut history, mut rig) = (FakeHistory::default(), Rig::new());
    script(&mut history);
    rig.ok(&mut history);
    nothing_secret_stored(&rig);
    assert!(rig.points().is_empty());
    // Across runs: the original and a live edit stand, then it is redacted, then a late edit arrives.
    let (mut history, mut rig) = (FakeHistory { prune: true, ..FakeHistory::default() }, Rig::new());
    history.room("!r:s", vec![said("$m", 10, "original"), edit("$e1", "@a:s", 20, "$m", EDIT_SECRET)]);
    rig.ok(&mut history);
    history.push("!r:s", vec![redaction("$rm", 30, "$m")]);
    rig.ok(&mut history);
    nothing_secret_stored(&rig);
    history.push("!r:s", vec![edit("$late", "@a:s", 40, "$m", EDIT_SECRET)]);
    rig.ok(&mut history);
    nothing_secret_stored(&rig);
    assert!(rig.points().is_empty());
    // Across a rebuild: a lost cursor re-reads the whole history.
    let mut cursors = rig.index.cursors();
    cursors.remove(&cursor_key("!r:s"));
    rig.index.set_cursors(&cursors).unwrap();
    rig.ok(&mut history);
    nothing_secret_stored(&rig);
    assert!(rig.points().is_empty());
}

#[test]
fn a_stranger_cannot_edit_a_message_of_the_same_page() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$m", 10, "mine"), edit("$e", "@stranger:s", 20, "$m", "hijacked")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert_eq!(rig.bodies(), ["mine"]);
}

#[test]
fn the_room_name_is_scrubbed_before_it_is_embedded_or_stored() {
    let mut history = FakeHistory::default();
    let token = format!("ghp_{}", "k2L9".repeat(9));
    history.room("!r:s", vec![said("$m", 10, "hello")]).facts("!r:s", |facts| facts.name = Some(format!("Swarm {token}")));
    let mut rig = Rig::new();
    let (done, journal) = rig.run(&mut history);
    let done = done.unwrap();
    assert!(rig.embedded().iter().all(|text| !text.contains("k2L9k2L9")));
    assert!(rig.points().iter().all(|point| !point.payload.text.contains("k2L9k2L9") && point.payload.text.starts_with("Swarm <redacted>")));
    assert!(done.scrubbed.iter().any(|hit| hit.event_id == "(name)") && journal.iter().all(|line| !line.contains("k2L9k2L9")));
}

#[test]
fn a_token_split_by_zero_width_characters_leaves_no_tail_in_a_stored_body() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$m", 10, &format!("key sk-{}\u{200b}{} done", "q7W3".repeat(4), "q7W3".repeat(4)))]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    assert_eq!(rig.bodies(), ["key <redacted> done"]);
}

#[test]
fn retire_goes_on_past_a_failed_delete_and_names_each_failure() {
    let mut rig = Rig::new();
    for room in ["!a:s", "!b:s", "!c:s"] {
        rig.seed(room, "$e", 1);
    }
    rig.index.state.lock().unwrap().fail_chat_deletes_for = ["!a:s".to_owned()].into();
    let done = retire_chat(&mut rig.index, &BTreeSet::new()).unwrap();
    assert_eq!(done.retired, ["!b:s", "!c:s"]);
    assert_eq!(done.failed.len(), 1);
    assert_eq!(done.failed[0].0, "!a:s");
    assert_eq!(rig.held_ids("!a:s"), ["$e"]);
    assert!(rig.cursor("!a:s").is_some(), "the cursor stays so the next run finds it again");
    // The refresh reports it too.
    let mut history = FakeHistory::default();
    history.room("!z:s", vec![said("$z", 1, "x")]);
    let done = rig.ok(&mut history);
    assert!(done.failed.contains(&"!a:s".to_owned()) && !done.is_clean());
}

#[test]
fn a_facts_error_on_a_joined_room_keeps_its_points_and_cursor_and_is_reported() {
    let mut history = FakeHistory::default();
    history.room("!r:s", vec![said("$a", 1, "x")]);
    let mut rig = Rig::new();
    rig.ok(&mut history);
    let (cursor, points) = (rig.cursor("!r:s"), rig.points());
    history.facts_fail.insert("!r:s".into());
    let (done, journal) = rig.run(&mut history);
    let done = done.unwrap();
    assert_eq!((rig.cursor("!r:s"), rig.points()), (cursor, points));
    assert_eq!(done.failed, ["!r:s"]);
    assert!(done.rooms.iter().any(|report| report.room_id == "!r:s" && report.failure.is_some()));
    assert!(journal.iter().any(|line| line.contains("!r:s") && line.contains("failed")));
}

#[test]
fn the_tail_on_equal_ends_is_the_greatest_chunk_id() {
    let mut rig = Rig::new();
    let (a, b) = (chunk_id("!r:s", "$a"), chunk_id("!r:s", "$b"));
    rig.index.upsert_chat(&[
        ChatPoint { id: a.clone(), vector: vec![1.0; 4], payload: chunk_payload_of("!r:s", &[("$a", 5)]) },
        ChatPoint { id: b.clone(), vector: vec![1.0; 4], payload: chunk_payload_of("!r:s", &[("$b", 5)]) },
    ])
    .unwrap();
    assert_eq!(rig.index.chat_tail("!r:s").unwrap().unwrap().id, a.max(b));
}
