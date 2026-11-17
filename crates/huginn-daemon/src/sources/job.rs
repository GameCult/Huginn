//! What one `huginn-sources` run does with the pieces it is handed: the forge
//! refresh, then the chat refresh (or, with chat off, the retirement of every
//! chat point), one exit status for both, and the steward-purge request. The
//! binary supplies the real forge, embedder, vector store and Matrix client and
//! owns the flags and the lock; this file decides nothing about scope or
//! scrubbing (`room_scope` and `redact` do) and never sees a credential.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Utc};

use super::chat::{Entry, RoomHistory, refresh_chat, retire_chat};
use super::chat_purge::{PurgeWindow, parse_windows, purge_key, request_purge};
use super::room_scope::Estate;
use super::{DescribedSources, Forge, SourceIndex, refresh};
use crate::index::{Embedder, INDEX_CALL_TIMEOUT};

/// Chat on: the reader's rooms and the estate ids the scope refuses.
pub struct Chat<'a, H> {
    pub history: &'a mut H,
    pub estate: &'a Estate,
}

/// One run: the forge refresh, then chat (`Some`) or the retirement of every chat point (`None`).
/// `say` takes the journal's report lines (stdout), `complain` the failures, refusals and scrub hits (stderr).
/// A forge failure never skips the chat run and a chat failure never hides a forge one.
/// Returns the exit status: 0 only when both were clean.
pub fn run<E: Embedder, S: SourceIndex, F: Forge, H: RoomHistory>(
    embedder: &mut E,
    index: &mut S,
    forge: &mut F,
    state_root: &Path,
    chat: Option<Chat<'_, H>>,
    say: &mut dyn FnMut(String),
    complain: &mut dyn FnMut(String),
) -> u8 {
    let mut clean = true;
    match refresh(embedder, index, forge, state_root, &mut |report| say(report.to_string())) {
        Ok(refreshed) if refreshed.is_clean() => {}
        Ok(refreshed) => {
            clean = false;
            if !refreshed.failed.is_empty() {
                complain(format!("huginn-sources: {} repos failed: {}", refreshed.failed.len(), refreshed.failed.join(", ")));
            }
            if let Some(why) = &refreshed.retirement_withheld {
                complain(format!("huginn-sources: no repo was retired: {why}"));
            }
        }
        Err(error) => {
            clean = false;
            complain(format!("huginn-sources: {error:#}"));
        }
    }
    match chat {
        Some(Chat { history, estate }) => {
            let mut line = |entry: Entry| if entry.is_alarm() { complain(entry.to_string()) } else { say(entry.to_string()) };
            match refresh_chat(embedder, index, history, estate, &mut line) {
                Ok(refreshed) => clean &= refreshed.is_clean(),
                Err(error) => {
                    clean = false;
                    complain(format!("huginn-sources: chat: {error:#}"));
                }
            }
        }
        None => match retire_chat(index, &BTreeSet::new()) {
            Ok(done) => {
                if !done.retired.is_empty() {
                    say(format!("chat retired: {} rooms", done.retired.len()));
                }
                for (room, why) in done.failed {
                    clean = false;
                    complain(format!("huginn-sources: retiring {room} failed: {why}"));
                }
            }
            Err(error) => {
                clean = false;
                complain(format!("huginn-sources: chat: {error:#}"));
            }
        },
    }
    u8::from(!clean)
}

/// The longest span one request may name.
const PURGE_SPAN_DAYS: i64 = 366;
const REQUEST_BYTES_MAX: usize = 120;

/// A steward's purge request as the admin typed it, checked. Each error names its option.
#[derive(Debug, PartialEq, Eq)]
pub struct Purge {
    pub room_id: String,
    pub from: String,
    pub to: String,
    pub request: String,
    pub window: PurgeWindow,
}

fn instant(option: &str, text: &str) -> Result<DateTime<Utc>> {
    ensure!(text.ends_with('Z'), "--{option} must be an RFC 3339 UTC time ending in Z, got {text:?}");
    let at = DateTime::parse_from_rfc3339(text).map_err(|_| anyhow::anyhow!("--{option} must be an RFC 3339 UTC time ending in Z, got {text:?}"))?;
    Ok(at.with_timezone(&Utc))
}

impl Purge {
    pub fn check(room: &str, from: &str, to: &str, request: &str, now: DateTime<Utc>) -> Result<Self> {
        ensure!(room.starts_with('!'), "--room must be a room id beginning with !, got {room:?}: an alias needs Matrix, and the journal prints both #alias (!id)");
        let (start, end) = (instant("from", from)?, instant("to", to)?);
        ensure!(start < end, "--from must be before --to");
        ensure!(end <= now, "--to must not be after now");
        ensure!(end - start <= chrono::Duration::days(PURGE_SPAN_DAYS), "the window must be at most {PURGE_SPAN_DAYS} days");
        ensure!(!request.is_empty() && request.len() <= REQUEST_BYTES_MAX, "--request must be 1 to {REQUEST_BYTES_MAX} bytes");
        let ms = |at: DateTime<Utc>| u64::try_from(at.timestamp_millis()).context("a purge time before 1970");
        Ok(Self { room_id: room.to_owned(), from: from.to_owned(), to: to.to_owned(), request: request.to_owned(), window: PurgeWindow { from_ms: ms(start)?, to_ms: ms(end)? } })
    }
}

/// Writes the tombstone and returns the line the admin sees; the free-text request is quoted and escaped so it cannot forge a journal line. Touches no point and makes no Matrix call;
/// the caller holds the job's lock. The next run applies it.
pub fn purge<S: SourceIndex>(index: &mut S, purge: &Purge) -> Result<String> {
    request_purge(index, &purge.room_id, purge.window)?;
    let DescribedSources::Labelled { cursors, .. } = index.describe_sources(INDEX_CALL_TIMEOUT).context("describing the source collection")? else {
        bail!("the source collection is absent or not Huginn's");
    };
    let windows = cursors.get(&purge_key(&purge.room_id)).map(|held| parse_windows(held)).transpose()?.map_or(0, |held| held.len());
    Ok(format!("chat-purge requested: {} {}..{} request={} windows={windows}", purge.room_id, purge.from, purge.to, purge.request))
}
