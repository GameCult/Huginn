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
pub const REFUSAL: &str = "gamecult.persona.chat_refusal.v1";

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
    /// The Matrix user ids the event's `m.mentions` names, as the mouth carries them.
    pub mentions: Vec<String>,
    /// Whether the room is a direct message.
    pub is_direct: bool,
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

/// Why the mouth refused a request, as its typed names (Bifrost
/// `tools/persona-chat-documents.mjs` `refusals`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Brake,
    PersonaMismatch,
    UnknownRoom,
    EmptyText,
    SendFailed,
    UnknownPersona,
}

impl Refusal {
    /// The mouth's spelling: a MessagePack string, whatever the codec does with unit variants.
    pub fn name(self) -> &'static str {
        match self {
            Self::Brake => "Brake",
            Self::PersonaMismatch => "PersonaMismatch",
            Self::UnknownRoom => "UnknownRoom",
            Self::EmptyText => "EmptyText",
            Self::SendFailed => "SendFailed",
            Self::UnknownPersona => "UnknownPersona",
        }
    }
}

impl Serialize for Refusal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Refusal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        [Self::Brake, Self::PersonaMismatch, Self::UnknownRoom, Self::EmptyText, Self::SendFailed, Self::UnknownPersona]
            .into_iter()
            .find(|refusal| refusal.name() == name)
            .ok_or_else(|| serde::de::Error::custom("the mouth named a refusal this side does not know"))
    }
}

/// The mouth's typed answer to a request that names a Persona it does not
/// serve: the pending request, the ack and the send alike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRefusal {
    pub persona: String,
    pub refusal: Refusal,
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
    /// Deliveries after `after_seq`, oldest first. A `chat_refusal` answer is an
    /// error that names the refusal.
    fn pending(&self, after_seq: u64) -> Result<Vec<ChatDelivery>>;
    fn send(&self, request: &ChatSendRequest) -> Result<ChatSendReceipt>;
    /// Every delivery up to and including `through_seq` is answered; the mouth
    /// advances its sync position only now, so a start that dies first has its
    /// deliveries again.
    fn ack(&self, through_seq: u64) -> Result<()>;
}

#[cfg(test)]
mod tests;
