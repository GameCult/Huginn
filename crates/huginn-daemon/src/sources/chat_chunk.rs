//! The chat chunker: greedy, deterministic and local. A chunk is consecutive
//! messages of one room; it closes at a time gap, at a message count, or when
//! its embedded text would pass a character bound. A message is never split.

use chrono::DateTime;
use huginn_mind::redact::SCRUB_VERSION;
use huginn_mind::ChatMessage;

use super::ChatPayload;
use crate::index::point_id;

/// Bumped whenever chunk boundaries or the embedded text would change for the
/// same messages. Part of `chat_version`.
pub const CHAT_CHUNKER_VERSION: u32 = 1;

/// A chunk closes when the next message is more than this much later.
pub const CHAT_GAP_MS: u64 = 30 * 60 * 1000;
/// The most messages a chunk holds.
pub const CHAT_MESSAGES_MAX: usize = 24;
/// The most characters a chunk's embedded text holds.
pub const CHAT_CHARS_MAX: usize = 1_600;
/// The most characters a stored message body holds, marker included.
pub const CHAT_BODY_CHARS_MAX: usize = 1_200;

/// Ends a body that was cut.
const CUT_MARKER: &str = "…[cut]";

/// What the room's cursor and chunk ids are stamped with: the chunker and the
/// scrub. Either changing re-ingests the room.
pub fn chat_version() -> String {
    format!("{CHAT_CHUNKER_VERSION}.{SCRUB_VERSION}")
}

/// A chunk's id: where it starts and under which version it was cut.
pub fn chunk_id(room_id: &str, first_event_id: &str) -> String {
    point_id(&format!("{room_id}\0{first_event_id}\0{}", chat_version()))
}

/// A scrubbed body cut to `CHAT_BODY_CHARS_MAX` characters on a character boundary, with a marker.
pub fn cut_body(body: &str) -> String {
    if body.chars().count() <= CHAT_BODY_CHARS_MAX {
        return body.to_owned();
    }
    let keep = CHAT_BODY_CHARS_MAX - CUT_MARKER.chars().count();
    body.chars().take(keep).chain(CUT_MARKER.chars()).collect()
}

fn line_of(message: &ChatMessage) -> String {
    let when = DateTime::from_timestamp_millis(i64::try_from(message.ts_ms).unwrap_or(i64::MAX)).map_or_else(String::new, |at| at.format("%Y-%m-%d %H:%M").to_string());
    format!("{when} {}: {}", message.sender, message.body)
}

/// What is embedded for a chunk: the room's label (its name, else its alias,
/// else its id), then one `YYYY-MM-DD HH:MM sender: body` line per message, UTC.
pub fn embedded_text(label: &str, messages: &[ChatMessage]) -> String {
    let mut text = label.to_owned();
    for message in messages {
        text.push('\n');
        text.push_str(&line_of(message));
    }
    text
}

/// `messages` (one room, in order) cut into chunks, each a run of consecutive messages.
pub fn chunk_messages(label: &str, messages: &[ChatMessage]) -> Vec<Vec<ChatMessage>> {
    let mut chunks: Vec<Vec<ChatMessage>> = Vec::new();
    let mut chars = 0_usize;
    for message in messages {
        let line = line_of(message).chars().count() + 1;
        let closes = chunks.last().is_some_and(|open| {
            let last = open.last().map_or(0, |last| last.ts_ms);
            message.ts_ms.saturating_sub(last) > CHAT_GAP_MS || open.len() >= CHAT_MESSAGES_MAX || chars + line > CHAT_CHARS_MAX
        });
        if closes || chunks.is_empty() {
            chunks.push(Vec::new());
            chars = label.chars().count();
        }
        chunks.last_mut().expect("a chunk is open").push(message.clone());
        chars += line;
    }
    chunks
}

/// The point payload of one chunk.
pub fn chunk_payload(room_id: &str, alias: Option<&str>, label: &str, messages: Vec<ChatMessage>) -> ChatPayload {
    ChatPayload { room_id: room_id.to_owned(), room_alias: alias.map(str::to_owned), text: embedded_text(label, &messages), messages }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(n: u64, ts_ms: u64, body: &str) -> ChatMessage {
        ChatMessage { event_id: format!("$e{n}"), sender: "@a:s".into(), ts_ms, body: body.into() }
    }

    fn ids(chunks: &[Vec<ChatMessage>]) -> Vec<Vec<&str>> {
        chunks.iter().map(|chunk| chunk.iter().map(|message| message.event_id.as_str()).collect()).collect()
    }

    #[test]
    fn a_gap_a_count_and_a_character_bound_each_close_a_chunk() {
        let gap = [message(1, 0, "a"), message(2, CHAT_GAP_MS, "b"), message(3, 2 * CHAT_GAP_MS + 1, "c")];
        assert_eq!(ids(&chunk_messages("r", &gap)), [["$e1", "$e2"].as_slice(), ["$e3"].as_slice()], "a gap of exactly the bound does not close");
        let many: Vec<ChatMessage> = (0..25).map(|n| message(n, n, "x")).collect();
        let chunks = chunk_messages("r", &many);
        assert_eq!((chunks[0].len(), chunks[1].len()), (CHAT_MESSAGES_MAX, 1));
        let long: Vec<ChatMessage> = (0..4).map(|n| message(n, n, &"w".repeat(500))).collect();
        let chunks = chunk_messages("r", &long);
        assert!(chunks.iter().all(|chunk| embedded_text("r", chunk).chars().count() <= CHAT_CHARS_MAX), "{:?}", ids(&chunks));
        assert!(chunks.len() > 1);
    }

    #[test]
    fn the_embedded_text_is_the_label_then_utc_lines() {
        let text = embedded_text("Swarm", &[message(1, 1_700_000_000_000, "hello")]);
        assert_eq!(text, "Swarm\n2023-11-14 22:13 @a:s: hello");
    }

    #[test]
    fn a_long_body_is_cut_at_the_bound_with_a_marker_on_a_char_boundary_and_a_short_one_is_whole() {
        let cut = cut_body(&"é".repeat(CHAT_BODY_CHARS_MAX + 50));
        assert_eq!(cut.chars().count(), CHAT_BODY_CHARS_MAX);
        assert!(cut.ends_with(CUT_MARKER));
        assert_eq!(cut_body(&"é".repeat(CHAT_BODY_CHARS_MAX)), "é".repeat(CHAT_BODY_CHARS_MAX));
        let one = chunk_messages("r", &[message(1, 0, &cut)]);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn a_chunk_id_is_its_room_first_event_and_version() {
        assert_eq!(chunk_id("!r:s", "$a"), chunk_id("!r:s", "$a"));
        assert_ne!(chunk_id("!r:s", "$a"), chunk_id("!r:s", "$b"));
        assert_ne!(chunk_id("!r:s", "$a"), chunk_id("!q:s", "$a"));
        assert_eq!(chat_version(), format!("{CHAT_CHUNKER_VERSION}.{SCRUB_VERSION}"));
    }

    /// Chunking the whole sequence equals re-chunking the open tail with each new batch appended.
    #[test]
    fn appending_to_the_open_tail_gives_the_boundaries_of_chunking_all_at_once() {
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        let mut ts = 0;
        let all: Vec<ChatMessage> = (0..300)
            .map(|n| {
                ts += if next(7) == 0 { CHAT_GAP_MS + 1 } else { next(60_000) };
                message(n, ts, &"z".repeat(next(700) as usize + 1))
            })
            .collect();
        let whole = chunk_messages("r", &all);
        for step in [1_usize, 5, 17, 100] {
            let mut held: Vec<Vec<ChatMessage>> = Vec::new();
            for batch in all.chunks(step) {
                let mut tail: Vec<ChatMessage> = held.pop().unwrap_or_default();
                tail.extend_from_slice(batch);
                held.extend(chunk_messages("r", &tail));
            }
            assert_eq!(held, whole, "step {step}");
        }
    }
}
