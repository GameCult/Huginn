//! Chat ingest: joined-room history into the shared source collection, beside
//! the forge's repos. `refresh_chat` is the only writer of `corpus=chat` points
//! and of the cursors object's `chat:` keys; `chat_purge::request_purge` is the
//! only writer of `chat-purge:` keys, and `refresh_chat` the only applier.
//!
//! The Matrix client that implements `RoomHistory` is the next cut's and holds no
//! policy. What is in scope is `room_scope`'s; what may be embedded is
//! `huginn_mind::redact::scrub`'s; this file folds events into chunks and writes
//! them, write-then-delete and cursor-last, so any crash replays to the same state.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use huginn_mind::ChatMessage;
use huginn_mind::redact::{ScrubKind, scrub};

use super::chat_chunk::{chat_version, chunk_id, chunk_messages, chunk_payload, cut_body};
use super::chat_purge::{PurgeWindow, covers, parse_windows, purge_key};
use super::room_scope::{Estate, Refusal, RoomFacts, admit_room, refuse_event};
use super::{CORPUS, ChatPoint, DescribedSources, SourceIndex, StoredChat};
use crate::index::{BATCH, BULK_EMBED_TIMEOUT, Embedder, INDEX_CALL_TIMEOUT};

/// Events asked of the port per page.
pub const CHAT_PAGE: u32 = 200;

/// A chat cursor is the key `chat:<room_id>`; a forge repo name cannot contain
/// a colon. The only definition of the prefix.
pub const CHAT_CURSOR_PREFIX: &str = "chat:";

/// A steward's purge tombstone is the key `chat-purge:<room_id>`. The only
/// definition of the prefix.
pub const CHAT_PURGE_PREFIX: &str = "chat-purge:";

/// Whether a cursors-object key is chat's (a room cursor or a purge tombstone)
/// and not a forge repo name. The only test that tells the two apart.
pub fn is_chat_key(key: &str) -> bool {
    key.starts_with(CHAT_CURSOR_PREFIX) || key.starts_with(CHAT_PURGE_PREFIX)
}

/// What one room event is to the ingest. Only text, notice and emote messages
/// become `Message`; everything else (encrypted, media, state, server-redacted)
/// is `Other` and is never stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Message { body: String },
    /// A replacement of `target`'s body.
    Edit { target: String, body: String },
    Redaction { target: String },
    Other,
}

/// One event of a room, as the port hands it over: the Matrix client maps its
/// JSON to this and filters nothing. `content_keys` are the top-level keys of
/// the event's content (the scope reads them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomEvent {
    pub event_id: String,
    pub sender: String,
    pub ts_ms: u64,
    pub content_keys: Vec<String>,
    pub kind: EventKind,
}

/// One page of a room's events after a cursor event, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventPage {
    pub events: Vec<RoomEvent>,
    /// More events follow this page.
    pub more: bool,
}

/// The reader user's view of Synapse as ingest uses it. Holds no policy: it maps
/// the server's answers to these types and filters nothing.
pub trait RoomHistory {
    /// The rooms the reader has joined. The only source of rooms; there is no allow list.
    fn joined_rooms(&mut self) -> Result<Vec<String>>;
    fn facts(&mut self, room_id: &str) -> Result<RoomFacts>;
    /// Up to `limit` events after the event `after` (from the room's start when `None`).
    /// `Ok(None)` means `after` is unknown to the server.
    fn events_after(&mut self, room_id: &str, after: Option<&str>, limit: u32) -> Result<Option<EventPage>>;
}

/// The cursors-object key of a room's chat cursor.
pub fn cursor_key(room_id: &str) -> String {
    format!("{CHAT_CURSOR_PREFIX}{room_id}")
}

/// The room a chat cursor key belongs to; `None` for any other key (a purge tombstone included).
pub fn cursor_room(key: &str) -> Option<&str> {
    key.strip_prefix(CHAT_CURSOR_PREFIX)
}

/// A cursor value, `<version>:<last event id>`; the event is empty before a room's first upsert.
pub fn parse_cursor(value: &str) -> Option<(&str, &str)> {
    value.split_once(':')
}

/// What one admitted room's run did, for the journal. Never carries a body or a sender.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatReport {
    pub room_id: String,
    pub alias: Option<String>,
    pub first: Option<String>,
    pub last: Option<String>,
    pub events: u32,
    pub chunks: u32,
    pub scrubbed: u32,
    pub purged: u32,
    pub seconds: f64,
}

impl std::fmt::Display for ChatReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} ({}) {}..{} events={} chunks={} scrubbed={} purged={} seconds={:.1}",
            self.alias.as_deref().unwrap_or("-"),
            self.room_id,
            self.first.as_deref().unwrap_or("-"),
            self.last.as_deref().unwrap_or("-"),
            self.events,
            self.chunks,
            self.scrubbed,
            self.purged,
            self.seconds
        )
    }
}

/// A joined room the scope refused, whole, and why; `event_id` when an event did it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusedRoom {
    pub room_id: String,
    pub reason: Refusal,
    pub event_id: Option<String>,
}

/// A credential the scrub replaced: where, never what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrubHit {
    pub room_id: String,
    pub event_id: String,
    pub kind: ScrubKind,
}

/// What a chat refresh did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatRefreshed {
    pub rooms: Vec<ChatReport>,
    /// Rooms (or steps) that failed; the others went on.
    pub failed: Vec<String>,
    pub refused: Vec<RefusedRoom>,
    pub scrubbed: Vec<ScrubHit>,
}

impl ChatRefreshed {
    /// Nothing failed and no joined room had to be refused: a joined estate, direct,
    /// encrypted or bridged room means the reader's roster is wrong. A scrub hit does not make a run unclean.
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.refused.is_empty()
    }
}

fn read_cursors<S: SourceIndex>(index: &mut S) -> Result<BTreeMap<String, String>> {
    match index.describe_sources(INDEX_CALL_TIMEOUT).context("describing the source collection")? {
        DescribedSources::Labelled { cursors, .. } => Ok(cursors),
        _ => bail!("the source collection is gone; the forge refresh creates it"),
    }
}

fn put_cursor<S: SourceIndex>(index: &mut S, key: &str, value: &str) -> Result<()> {
    let mut cursors = read_cursors(index)?;
    if cursors.insert(key.to_owned(), value.to_owned()).as_deref() != Some(value) {
        index.set_cursors(&cursors).context("writing a chat cursor")?;
    }
    Ok(())
}

fn drop_cursor<S: SourceIndex>(index: &mut S, key: &str) -> Result<()> {
    let mut cursors = read_cursors(index)?;
    if cursors.remove(key).is_some() {
        index.set_cursors(&cursors).context("dropping a chat cursor")?;
    }
    Ok(())
}

/// A room's points, then its `chat:` cursor. Its purge tombstone stays.
fn purge_room<S: SourceIndex>(index: &mut S, room_id: &str) -> Result<()> {
    index.delete_chat(room_id, None).with_context(|| format!("deleting the chunks of {room_id}"))?;
    drop_cursor(index, &cursor_key(room_id))
}

/// Deletes the points and `chat:` cursor of every room not in `keep` (the reader left it,
/// or it is refused or excluded), and returns those rooms. Purge tombstones and forge
/// repos are never touched. With the empty set it removes the whole chat corpus.
pub fn retire_chat<S: SourceIndex>(index: &mut S, keep: &BTreeSet<String>) -> Result<Vec<String>> {
    let rooms: Vec<String> = read_cursors(index)?.keys().filter_map(|key| cursor_room(key)).filter(|room| !keep.contains(*room)).map(str::to_owned).collect();
    for room in &rooms {
        purge_room(index, room)?;
    }
    Ok(rooms)
}

/// A change to chunks already stored, in event order.
enum Op {
    /// An edit by `sender` of `target`'s body; applies only to the original's sender.
    Edit { target: String, sender: String, body: String },
    /// A held message's body is what the page says it is (a replay).
    Set { target: String, body: String },
    Redact { target: String },
}

impl Op {
    fn target(&self) -> &str {
        match self {
            Self::Edit { target, .. } | Self::Set { target, .. } | Self::Redact { target } => target,
        }
    }

    /// Applies to `messages`; whether they changed.
    fn apply(&self, messages: &mut Vec<ChatMessage>) -> bool {
        let Some(at) = messages.iter().position(|message| message.event_id == self.target()) else { return false };
        match self {
            Self::Edit { sender, body, .. } if messages[at].sender == *sender && messages[at].body != *body => messages[at].body.clone_from(body),
            Self::Set { body, .. } if messages[at].body != *body => messages[at].body.clone_from(body),
            Self::Redact { .. } => {
                messages.remove(at);
            }
            _ => return false,
        }
        true
    }
}

struct Folded {
    pending: Vec<ChatMessage>,
    ops: Vec<Op>,
}

struct Room<'a> {
    facts: &'a RoomFacts,
    label: String,
    windows: Vec<PurgeWindow>,
}

/// The page folded in order: the purge filter, then the credential scrub, then the
/// event's effect. After this no raw body exists.
fn fold(room: &Room, events: &[RoomEvent], report: &mut ChatReport, out: &mut ChatRefreshed) -> Folded {
    let mut folded = Folded { pending: Vec::new(), ops: Vec::new() };
    let mut redacted: HashSet<&str> = HashSet::new();
    let mut scrubbed = |event: &RoomEvent, body: &str| {
        let clean = scrub(body);
        for kind in clean.hits {
            report.scrubbed += 1;
            out.scrubbed.push(ScrubHit { room_id: room.facts.room_id.clone(), event_id: event.event_id.clone(), kind });
        }
        cut_body(&clean.text)
    };
    for event in events {
        match &event.kind {
            EventKind::Message { body } => {
                if covers(&room.windows, event.ts_ms) {
                    report.purged += 1;
                } else if !redacted.contains(event.event_id.as_str()) {
                    let body = scrubbed(event, body);
                    if !body.trim().is_empty() {
                        folded.pending.push(ChatMessage { event_id: event.event_id.clone(), sender: event.sender.clone(), ts_ms: event.ts_ms, body });
                    }
                }
            }
            EventKind::Edit { target, body } => {
                if covers(&room.windows, event.ts_ms) {
                    report.purged += 1;
                    continue;
                }
                let body = scrubbed(event, body);
                match folded.pending.iter_mut().find(|message| message.event_id == *target) {
                    Some(message) if message.sender == event.sender && !body.trim().is_empty() => message.body = body,
                    Some(_) => {}
                    None => folded.ops.push(Op::Edit { target: target.clone(), sender: event.sender.clone(), body }),
                }
            }
            EventKind::Redaction { target } => {
                folded.pending.retain(|message| message.event_id != *target);
                redacted.insert(target);
                folded.ops.push(Op::Redact { target: target.clone() });
            }
            EventKind::Other => {}
        }
    }
    folded
}

/// Chunks to rewrite: the stored chunk ids they replace and the messages that now make them.
type Rewrite = (Vec<String>, Vec<ChatMessage>);

/// Chunks `rewrites`, embeds and writes the new chunks (acknowledged), and only then deletes the old
/// ids that are not among the new. Returns how many chunks were written.
///
/// A chunk written by an earlier run whose delete failed can still sit beside the chunk that replaced
/// it, and a rewrite of the stale one then names the same first event, hence the same id, as the
/// newer chunk. The newer chunk (`standing`, the other chunks the page found) stays; the stale one is deleted.
fn write_rewrites<E: Embedder, S: SourceIndex>(
    embedder: &mut E,
    index: &mut S,
    dimensions: u32,
    room: &Room,
    rewrites: Vec<Rewrite>,
    standing: &BTreeSet<String>,
) -> Result<u32> {
    let mut old: BTreeSet<String> = BTreeSet::new();
    let mut points: Vec<(String, super::ChatPayload)> = Vec::new();
    for (ids, messages) in rewrites {
        for group in chunk_messages(&room.label, &messages) {
            let id = chunk_id(&room.facts.room_id, &group[0].event_id);
            if standing.contains(&id) && !ids.contains(&id) {
                continue;
            }
            points.push((id, chunk_payload(&room.facts.room_id, room.facts.alias.as_deref(), &room.label, group)));
        }
        old.extend(ids);
    }
    let new: BTreeSet<&String> = points.iter().map(|(id, _)| id).collect();
    let stale: Vec<String> = old.iter().filter(|id| !new.contains(id)).cloned().collect();
    let written = u32::try_from(points.len()).unwrap_or(u32::MAX);
    for slice in points.chunks(BATCH) {
        let texts: Vec<String> = slice.iter().map(|(_, payload)| payload.text.clone()).collect();
        let vectors = embedder.embed(&texts, BULK_EMBED_TIMEOUT).context("embedding chat chunks")?;
        ensure!(vectors.len() == texts.len(), "the embedder returned {} vectors for {} chunks", vectors.len(), texts.len());
        let mut batch = Vec::with_capacity(slice.len());
        for ((id, payload), vector) in slice.iter().zip(vectors) {
            ensure!(vector.len() == dimensions as usize, "the embedder returned a vector of {} where the collection holds {dimensions}", vector.len());
            batch.push(ChatPoint { id: id.clone(), vector, payload: payload.clone() });
        }
        index.upsert_chat(&batch).context("writing chat chunks")?;
    }
    if !stale.is_empty() {
        index.delete_chat(&room.facts.room_id, Some(&stale)).context("deleting superseded chat chunks")?;
    }
    Ok(written)
}

/// Applies the room's purge windows to the chunks it already holds.
fn apply_purge<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, dimensions: u32, room: &Room, report: &mut ChatReport) -> Result<()> {
    let mut hit: BTreeMap<String, StoredChat> = BTreeMap::new();
    for window in &room.windows {
        for chunk in index.chat_chunks_overlapping(&room.facts.room_id, window.from_ms, window.to_ms)? {
            hit.insert(chunk.id.clone(), chunk);
        }
    }
    let mut rewrites = Vec::new();
    for chunk in hit.into_values() {
        let kept: Vec<ChatMessage> = chunk.payload.messages.iter().filter(|message| !covers(&room.windows, message.ts_ms)).cloned().collect();
        let removed = chunk.payload.messages.len() - kept.len();
        if removed > 0 {
            report.purged += u32::try_from(removed).unwrap_or(u32::MAX);
            rewrites.push((vec![chunk.id], kept));
        }
    }
    report.chunks += write_rewrites(embedder, index, dimensions, room, rewrites, &BTreeSet::new())?;
    Ok(())
}

/// Folds one page into the stored chunks: edits and redactions rewrite the chunk that held
/// the target alone, new messages extend the open tail.
fn write_page<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, dimensions: u32, room: &Room, mut folded: Folded) -> Result<u32> {
    let room_id = &room.facts.room_id;
    let mut asked: Vec<String> = folded.pending.iter().map(|message| message.event_id.clone()).collect();
    asked.extend(folded.ops.iter().map(|op| op.target().to_owned()));
    if asked.is_empty() {
        return Ok(0);
    }
    let holding = index.chat_chunks_holding(room_id, &asked)?;
    // A message the index already holds is a replay: its page body is asserted, never appended again.
    let held: HashSet<&str> = holding.iter().flat_map(|chunk| chunk.payload.messages.iter().map(|message| message.event_id.as_str())).collect();
    let (replayed, fresh): (Vec<ChatMessage>, Vec<ChatMessage>) = folded.pending.drain(..).partition(|message| held.contains(message.event_id.as_str()));
    let mut ops: Vec<Op> = replayed.into_iter().map(|message| Op::Set { target: message.event_id, body: message.body }).collect();
    ops.append(&mut folded.ops);

    let tail = index.chat_tail(room_id)?;
    let tail_id = tail.as_ref().map(|tail| tail.id.clone());
    let standing: BTreeSet<String> = holding.iter().map(|chunk| chunk.id.clone()).chain(tail_id.clone()).collect();
    let mut tail_messages = tail.map(|tail| tail.payload.messages).unwrap_or_default();
    let mut tail_changed = false;
    let mut rewrites: Vec<Rewrite> = Vec::new();
    for chunk in holding {
        let mut messages = chunk.payload.messages.clone();
        let mut changed = false;
        for op in &ops {
            changed |= op.apply(&mut messages);
        }
        if Some(&chunk.id) == tail_id.as_ref() {
            (tail_messages, tail_changed) = (messages, changed);
        } else if changed {
            rewrites.push((vec![chunk.id], messages));
        }
    }
    if !fresh.is_empty() || tail_changed {
        tail_messages.extend(fresh);
        rewrites.push((tail_id.into_iter().collect(), tail_messages));
    }
    write_rewrites(embedder, index, dimensions, room, rewrites, &standing)
}

enum Outcome {
    Done(ChatReport),
    Refused(RefusedRoom),
}

#[allow(clippy::too_many_arguments)]
fn ingest_room<E: Embedder, S: SourceIndex, H: RoomHistory>(
    embedder: &mut E,
    index: &mut S,
    history: &mut H,
    dimensions: u32,
    facts: &RoomFacts,
    out: &mut ChatRefreshed,
) -> Result<Outcome> {
    let started = Instant::now();
    let room_id = facts.room_id.as_str();
    let (key, version) = (cursor_key(room_id), chat_version());
    let cursors = read_cursors(index)?;
    let mut after: Option<String> = match cursors.get(&key).map(|value| parse_cursor(value)) {
        None => None,
        Some(Some((held, event))) if held == version => Some(event.to_owned()).filter(|event| !event.is_empty()),
        // Another chunker or scrub version, or a value nobody here wrote: this room alone starts over.
        Some(_) => {
            index.delete_chat(room_id, None).with_context(|| format!("deleting the chunks of {room_id} for a re-ingest"))?;
            None
        }
    };
    if after.is_none() {
        put_cursor(index, &key, &format!("{version}:"))?;
    }
    let windows = cursors.get(&purge_key(room_id)).map(|value| parse_windows(value)).transpose()?.unwrap_or_default();
    let label = facts.name.clone().or_else(|| facts.alias.clone()).unwrap_or_else(|| room_id.to_owned());
    let room = Room { facts, label, windows };
    let mut report = ChatReport { room_id: room_id.to_owned(), alias: facts.alias.clone(), first: None, last: None, events: 0, chunks: 0, scrubbed: 0, purged: 0, seconds: 0.0 };
    apply_purge(embedder, index, dimensions, &room, &mut report)?;
    let mut restarted = false;
    loop {
        let Some(page) = history.events_after(room_id, after.as_deref(), CHAT_PAGE)? else {
            ensure!(after.is_some() && !restarted, "the server does not know the start of {room_id}'s history");
            restarted = true;
            purge_room(index, room_id)?;
            put_cursor(index, &key, &format!("{version}:"))?;
            after = None;
            continue;
        };
        let Some(last) = page.events.last().map(|event| event.event_id.clone()) else { break };
        // A refusing event refuses the room before anything of the page is chunked or embedded.
        if let Some((event, reason)) = page.events.iter().find_map(|event| refuse_event(event).map(|reason| (event, reason))) {
            purge_room(index, room_id)?;
            return Ok(Outcome::Refused(RefusedRoom { room_id: room_id.to_owned(), reason, event_id: Some(event.event_id.clone()) }));
        }
        report.first.get_or_insert_with(|| page.events[0].event_id.clone());
        report.events += u32::try_from(page.events.len()).unwrap_or(u32::MAX);
        let folded = fold(&room, &page.events, &mut report, out);
        report.chunks += write_page(embedder, index, dimensions, &room, folded)?;
        put_cursor(index, &key, &format!("{version}:{last}"))?;
        report.last = Some(last.clone());
        after = Some(last);
        if !page.more {
            break;
        }
    }
    report.seconds = started.elapsed().as_secs_f64();
    Ok(Outcome::Done(report))
}

/// Brings the chat corpus to the reader's joined rooms. The collection must already exist and be
/// Huginn's, built under this embedder's model: the forge refresh creates and rebuilds it, and this never does.
/// A joined room the scope refuses is purged and named; a room the reader left is retired; the rest
/// are ingested from their cursors. One room's failure is journaled and the others go on.
/// `journal` receives every line the run prints: reports, scrub hits, refusals and failures.
pub fn refresh_chat<E: Embedder, S: SourceIndex, H: RoomHistory>(
    embedder: &mut E,
    index: &mut S,
    history: &mut H,
    estate: &Estate,
    journal: &mut dyn FnMut(String),
) -> Result<ChatRefreshed> {
    let DescribedSources::Labelled { meta, .. } = index.describe_sources(INDEX_CALL_TIMEOUT).context("describing the source collection")? else {
        bail!("the source collection is absent or not Huginn's; the forge refresh creates it and chat never does");
    };
    ensure!(meta.corpus == CORPUS, "the source collection was built for corpus {} and chat ingest writes {CORPUS}; the forge refresh rebuilds it", meta.corpus);
    embedder.built_under(Some(&meta.model_identity()));
    let identity = embedder.model_identity(BULK_EMBED_TIMEOUT).context("reading the embedding model's identity")?;
    ensure!(meta.model_identity() == identity, "the source collection was built under {} {} and chat embeds under {} {}", meta.model, meta.model_digest, identity.name, identity.digest);

    let mut out = ChatRefreshed::default();
    let mut joined = history.joined_rooms().context("listing the reader's joined rooms")?;
    let mut seen = HashSet::new();
    joined.retain(|room| seen.insert(room.clone()));
    let mut keep: BTreeSet<String> = joined.iter().cloned().collect();
    let mut admitted: Vec<RoomFacts> = Vec::new();
    for room_id in &joined {
        let verdict = if estate.holds_id(room_id) {
            Ok(Err(Refusal::Estate))
        } else {
            history.facts(room_id).map(|facts| admit_room(&facts, estate).map(|()| facts))
        };
        match verdict {
            Ok(Ok(facts)) => admitted.push(facts),
            Ok(Err(reason)) => {
                keep.remove(room_id);
                match purge_room(index, room_id) {
                    Ok(()) => {}
                    Err(error) => {
                        journal(format!("huginn-sources: chat {room_id} could not be purged, the others go on: {error:#}"));
                        out.failed.push(room_id.clone());
                    }
                }
                journal(format!("refused: {room_id} {reason}"));
                out.refused.push(RefusedRoom { room_id: room_id.clone(), reason, event_id: None });
            }
            Err(error) => {
                journal(format!("huginn-sources: chat {room_id} facts failed, the others go on: {error:#}"));
                out.failed.push(room_id.clone());
            }
        }
    }
    if let Err(error) = retire_chat(index, &keep) {
        journal(format!("huginn-sources: retiring rooms the reader left failed, the others go on: {error:#}"));
        out.failed.push("retire".to_owned());
    }
    for facts in admitted {
        match ingest_room(embedder, index, history, identity.dimensions, &facts, &mut out) {
            Ok(Outcome::Done(report)) => {
                journal(report.to_string());
                out.rooms.push(report);
            }
            Ok(Outcome::Refused(refused)) => {
                journal(format!("refused: {} {} [{}]", refused.room_id, refused.reason, refused.event_id.as_deref().unwrap_or("")));
                out.refused.push(refused);
            }
            Err(error) => {
                journal(format!("huginn-sources: chat {} failed, the others go on: {error:#}", facts.room_id));
                out.failed.push(facts.room_id.clone());
            }
        }
    }
    for hit in &out.scrubbed {
        journal(format!("scrubbed: {} {} {}", hit.room_id, hit.event_id, hit.kind));
    }
    Ok(out)
}
