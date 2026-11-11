//! Chat ingest: joined-room history into the shared source collection, beside
//! the forge's repos. `refresh_chat` is the only writer of `corpus=chat` points
//! and of the cursors object's chat keys.

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
