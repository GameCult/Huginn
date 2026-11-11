//! A steward's purge of a room's time window. The tombstone is the one chat
//! state that is not derived from Synapse: a durable request, kept in the
//! collection's cursors object under `chat-purge:<room_id>`, that must outlive
//! the points it removes, so a re-ingest never brings the window back.
//!
//! `request_purge` is the only writer; `refresh_chat` reads the tombstone and
//! applies it at its next run. The code does not authenticate the steward: whoever
//! can run the job as the unit's user (the host admin) may ask, and the request
//! carries a ref in the job's journal for a trace. Tombstones live in the
//! collection, so a full collection rebuild forgets them; re-issue them from
//! their refs.

use anyhow::{Context, Result, bail};

use super::chat::CHAT_PURGE_PREFIX;
use super::{DescribedSources, SourceIndex};
use crate::index::INDEX_CALL_TIMEOUT;

/// The most windows a room's tombstone holds.
pub const PURGE_WINDOWS_MAX: usize = 32;

/// Messages with `from_ms <= ts_ms < to_ms` are purged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PurgeWindow {
    pub from_ms: u64,
    pub to_ms: u64,
}

impl PurgeWindow {
    pub fn covers(&self, ts_ms: u64) -> bool {
        self.from_ms <= ts_ms && ts_ms < self.to_ms
    }
}

/// The tombstone key of a room.
pub fn purge_key(room_id: &str) -> String {
    format!("{CHAT_PURGE_PREFIX}{room_id}")
}

/// A tombstone value, `<from>..<to>[,<from>..<to>...]`.
pub fn parse_windows(value: &str) -> Result<Vec<PurgeWindow>> {
    value
        .split(',')
        .map(|part| {
            let (from, to) = part.split_once("..").with_context(|| format!("purge window {part:?} is not <from>..<to>"))?;
            let window = PurgeWindow { from_ms: from.parse().context("a purge window's start is not milliseconds")?, to_ms: to.parse().context("a purge window's end is not milliseconds")? };
            if window.from_ms >= window.to_ms {
                bail!("purge window {part:?} is empty");
            }
            Ok(window)
        })
        .collect()
}

/// Windows sorted, with overlapping and touching ones joined.
pub fn merge(mut windows: Vec<PurgeWindow>) -> Vec<PurgeWindow> {
    windows.sort();
    let mut merged: Vec<PurgeWindow> = Vec::new();
    for window in windows {
        match merged.last_mut() {
            Some(last) if window.from_ms <= last.to_ms => last.to_ms = last.to_ms.max(window.to_ms),
            _ => merged.push(window),
        }
    }
    merged
}

pub fn encode(windows: &[PurgeWindow]) -> String {
    windows.iter().map(|window| format!("{}..{}", window.from_ms, window.to_ms)).collect::<Vec<_>>().join(",")
}

/// Whether any window covers `ts_ms`.
pub fn covers(windows: &[PurgeWindow], ts_ms: u64) -> bool {
    windows.iter().any(|window| window.covers(ts_ms))
}

/// Records that `window` of `room_id` must leave the index and never return. Writes the cursors
/// object only if the tombstone changed, and refuses a room's 33rd window. Writes no point and
/// embeds nothing; the next `refresh_chat` applies it. The job's `chat-purge` subcommand calls
/// it under the job's lock, because `set_cursors` replaces the cursors object whole.
/// Returns whether the tombstone changed.
pub fn request_purge<S: SourceIndex>(index: &mut S, room_id: &str, window: PurgeWindow) -> Result<bool> {
    if window.from_ms >= window.to_ms {
        bail!("a purge window must start before it ends");
    }
    let DescribedSources::Labelled { mut cursors, .. } = index.describe_sources(INDEX_CALL_TIMEOUT).context("describing the source collection")? else {
        bail!("the source collection is absent or not Huginn's; the forge refresh creates it");
    };
    let key = purge_key(room_id);
    let mut windows = cursors.get(&key).map(|held| parse_windows(held)).transpose()?.unwrap_or_default();
    windows.push(window);
    let windows = merge(windows);
    if windows.len() > PURGE_WINDOWS_MAX {
        bail!("room {room_id} already holds {PURGE_WINDOWS_MAX} purge windows; widen one instead");
    }
    let encoded = encode(&windows);
    if cursors.get(&key) == Some(&encoded) {
        return Ok(false);
    }
    cursors.insert(key, encoded);
    index.set_cursors(&cursors).context("writing the purge tombstone")?;
    Ok(true)
}
