//! The chat wire of Bifrost's Persona mouth, as typed documents and the one port
//! her Persona organ reads and writes chat through.
//!
//! The mouth is Bifrost's chat ear and mouth (cut `persona-mouth`). Five CultNet document kinds cross
//! it, each a positional MessagePack tuple (fields in declaration order, `rmp_serde::to_vec`; the mouth refuses maps): the pending request and its answer, the
//! ack, and the send request with its receipt. Their field order is the mouth's, pinned
//! by its golden fixtures (`tests/fixtures/persona-chat-v1/`, Bifrost
//! `eureka-body/persona-mouth` ab2bdc6) in both directions. `chat_rudp::RudpChat` is
//! the RUDP transport that carries them; `Chat` is its seam, and `encode` and
//! `decode` are its codec. Room aliases are the mouth's
//! full aliases (`#operator:gamecult.org`), which the caller's `--rooms` carries.
//!
//! No chat-server client, token or address exists on this side: the Persona
//! process holds no chat credential.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Document kinds, as the mouth names them.
pub const PENDING_REQUEST: &str = "gamecult.persona.chat_pending_request.v1";
pub const PENDING: &str = "gamecult.persona.chat_pending.v1";
pub const ACK: &str = "gamecult.persona.chat_ack.v1";
pub const SEND_REQUEST: &str = "gamecult.persona.chat_send_request.v1";
pub const SEND_RECEIPT: &str = "gamecult.persona.chat_send_receipt.v1";

/// One earlier line of a room, delivered with a message for its context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLine {
    pub sender: String,
    pub text: String,
    pub at_unix_ms: u64,
}

/// One message from another sender in a joined room.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatDelivery {
    pub seq: u64,
    pub room_alias: String,
    pub room_id: String,
    pub event_id: String,
    pub sender: String,
    pub sender_display: String,
    pub text: String,
    pub at_unix_ms: u64,
    pub context: Vec<ContextLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPendingRequest {
    pub persona: String,
    pub after_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPending {
    pub persona: String,
    pub deliveries: Vec<ChatDelivery>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatAck {
    pub persona: String,
    pub through_seq: u64,
}

/// A send. `request_id` is the mouth's transaction id, so a repeat of the same
/// id is the same send.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendRequest {
    pub request_id: String,
    pub persona: String,
    pub room_alias: String,
    pub text: String,
}

/// The mouth's answer to a send: the event it made, or why it did not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendReceipt {
    pub request_id: String,
    pub event_id: Option<String>,
    pub refusal: Option<String>,
}

/// A document as the mouth reads it: a positional tuple.
pub fn encode<T: Serialize>(document: &T) -> Result<Vec<u8>> {
    rmp_serde::to_vec(document).context("the chat document does not encode")
}

/// A document as the mouth writes it.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    rmp_serde::from_slice(bytes).context("the chat document does not decode")
}

/// The Persona organ's chat: what is waiting, what to say, what is done.
pub trait Chat {
    /// Deliveries after `after_seq`, oldest first.
    fn pending(&self, after_seq: u64) -> Result<Vec<ChatDelivery>>;
    fn send(&self, request: &ChatSendRequest) -> Result<ChatSendReceipt>;
    /// Every delivery up to and including `through_seq` is answered; the mouth
    /// advances its sync position only now, so a start that dies first has its
    /// deliveries again.
    fn ack(&self, through_seq: u64) -> Result<()>;
}

#[cfg(test)]
mod tests;
