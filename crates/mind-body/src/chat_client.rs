//! The chat wire of Bifrost's Persona mouth, as typed documents and the one port
//! her Persona organ reads and writes chat through.
//!
//! The mouth is Bifrost's chat ear and mouth (cut `persona-mouth`). Five CultNet document kinds cross
//! it, each a positional MessagePack tuple (fields in declaration order, `rmp_serde::to_vec`; the mouth refuses maps): the pending request and its answer, the
//! ack, and the send request with its receipt. Their field order is the mouth's, pinned
//! by its golden fixtures (`tests/fixtures/persona-chat-v1/` for the ack and refusal kinds, `persona-chat-v2/` for the send
//! and its receipt, `persona-chat-v5/` for the pending request and its answer) in both directions. `chat_rudp::RudpChat` is
//! the RUDP transport that carries them; `Chat` is its seam, and `encode` and
//! `decode` are its codec. Room aliases are the mouth's
//! full aliases (`#operator:gamecult.org`), which the caller's `--rooms` carries.
//!
//! No chat-server client, token or address exists on this side: the Persona
//! process holds no chat credential.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Document kinds, as the mouth names them.
/// The pending request as a mouth without the addressing facts reads it: `[persona, after_seq]`, answered with `chat_pending.v2`.
pub const PENDING_REQUEST_V1: &str = "gamecult.persona.chat_pending_request.v1";
/// The pending request, the same tuple. Asking by this kind is asking for the answer with the addressing facts.
pub const PENDING_REQUEST: &str = "gamecult.persona.chat_pending_request.v2";
/// The answer to it: a delivery with its `basis` and `form` at tuple indices 12 and 13.
pub const PENDING: &str = "gamecult.persona.chat_pending.v3";
/// The answer a mouth gives a v1 request: no addressing facts, decoded as `basis: None, form: Message`.
pub const PENDING_V2: &str = "gamecult.persona.chat_pending.v2";
/// The pending answer of a mouth that has no image field either: its deliveries decode with `image: None`.
pub const PENDING_V1: &str = "gamecult.persona.chat_pending.v1";
pub const ACK: &str = "gamecult.persona.chat_ack.v1";
/// Her read receipt: the newest event of a room that went into her model's input.
pub const READ: &str = "gamecult.persona.chat_read.v1";
pub const SEND_REQUEST: &str = "gamecult.persona.chat_send_request.v2";
pub const SEND_RECEIPT: &str = "gamecult.persona.chat_send_receipt.v2";
/// Her request that the mouth open a DM with a person, and its answer.
pub const OPEN_DIRECT_REQUEST: &str = "gamecult.persona.chat_open_direct_request.v1";
pub const OPEN_DIRECT_RECEIPT: &str = "gamecult.persona.chat_open_direct_receipt.v1";
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
    /// The attachment the mouth stored for an image event. Tuple index 11; a v1 record ends at 10 and reads as none.
    #[serde(default)]
    pub image: Option<ChatImage>,
    /// Why Matrix says the message is for this Persona: tuple index 12. None is unproven, never "not for her"; an answer of an
    /// older mouth ends before it and reads as none.
    #[serde(default)]
    pub basis: Option<Basis>,
    /// What kind of event it is: tuple index 13. An answer of an older mouth ends before it and reads as a message.
    #[serde(default)]
    pub form: Form,
}

/// Why the mouth found a delivery to be for the Persona, by what Matrix says (Bifrost `docs/bridge.md` "Addressing facts"). `Tag` and
/// `Room` are the sender's claim in `m.mentions`; `Reply` is checked against the event it targets. Evidence, not authority: the
/// runtime's own rule (`address::addresses`) weighs it. `Room` is reported and wakes no one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Direct,
    Tag,
    Reply,
    Room,
}

impl Basis {
    const ALL: [Self; 4] = [Self::Direct, Self::Tag, Self::Reply, Self::Room];

    /// The mouth's spelling: a MessagePack string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Tag => "tag",
            Self::Reply => "reply",
            Self::Room => "room",
        }
    }
}

impl Serialize for Basis {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Basis {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::ALL.into_iter().find(|basis| basis.name() == name).ok_or_else(|| serde::de::Error::custom("the mouth named a basis this side does not know"))
    }
}

/// The kind of event a delivery is. Only a `Message` can address anyone: a `Notice` is an automated line and an `Edit` revises an
/// earlier message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Form {
    #[default]
    Message,
    Notice,
    Edit,
}

impl Form {
    const ALL: [Self; 3] = [Self::Message, Self::Notice, Self::Edit];

    /// The mouth's spelling: a MessagePack string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Notice => "notice",
            Self::Edit => "edit",
        }
    }
}

impl Serialize for Form {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Form {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::ALL.into_iter().find(|form| form.name() == name).ok_or_else(|| serde::de::Error::custom("the mouth named a form this side does not know"))
    }
}

/// An image the mouth fetched and stored, by reference: never its bytes. The path is relative to the Persona's image
/// store, `<hex sha256 of the room id>/<hex sha256 of the bytes>[.<ext>]`, and every field is re-checked against the
/// file before it reaches a model (`chat_image`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatImage {
    pub path: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
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
    /// The mouth's room tail, tuple index 2 of `chat_pending.v3`: recent lines of the rooms she may speak in, oldest first, whether
    /// or not delivered or acked. An answer of a mouth that has none (v1, v2, or a v3 before the tail) ends before it and reads as
    /// none; a mouth with nothing to show sends an empty list, and that is a different fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<Vec<TailLine>>,
}

/// One line of the room tail: `[room, event_id, at_unix_ms, sender, form, basis, body]`. `room` is the configured alias (the room id
/// for a DM); `basis` is what Matrix says on the event itself, none being unproven; `body` is untrusted text clipped by the mouth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailLine {
    pub room: String,
    pub event_id: String,
    pub at_unix_ms: u64,
    pub sender: String,
    pub form: Form,
    pub basis: Option<Basis>,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatAck {
    pub persona: String,
    pub through_seq: u64,
}

/// One room's read report: `[persona, room, event_id, at_unix_ms]`, positional like `ChatAck`.
///
/// `room` is the delivery's `room_id` (the Matrix room id), not its alias. The mouth posts `m.read` to the room it delivered
/// the event for and accepts either spelling, but only the id names one room with no lookup, and a direct room has no alias
/// (its alias is its id). `event_id` is the newest event of that room among those the model's input held, and `at_unix_ms`
/// that event's own instant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRead {
    pub persona: String,
    pub room: String,
    pub event_id: String,
    pub at_unix_ms: u64,
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
    /// A send with a basis found the room had moved past it: the receipt names what moved it.
    RoomMoved,
    /// The mouth does not serve this document kind. Answers an open-direct request only: a send receipt never carries it.
    UnknownKind,
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
            Self::RoomMoved => "RoomMoved",
            Self::UnknownKind => "UnknownKind",
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
        [Self::Brake, Self::PersonaMismatch, Self::UnknownRoom, Self::EmptyText, Self::SendFailed, Self::UnknownPersona, Self::RoomMoved, Self::UnknownKind]
            .into_iter()
            .find(|refusal| refusal.name() == name)
            .ok_or_else(|| serde::de::Error::custom("the mouth named a refusal this side does not know"))
    }
}

/// The mouth refused a request outright with a typed refusal frame, and nothing was done. An error of `open_direct` when the
/// mouth does not serve the kind: told as not sent, unlike a silent mouth, whose open may have gone out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouthRefused(pub Refusal);

impl std::fmt::Display for MouthRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the mouth refused the request: {}", self.0.name())
    }
}

impl std::error::Error for MouthRefused {}

/// The mouth's typed answer to a request that names a Persona it does not
/// serve: the pending request, the ack and the send alike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRefusal {
    pub persona: String,
    pub refusal: Refusal,
}

/// What a send's author had seen of its room: the newest event of the room her input held (none when it held
/// none), and when her process began. The mouth refuses the send if another sender's message reached the room
/// after that event, or after that time where there is none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendBasis {
    pub seen_event_id: Option<String>,
    pub since_unix_ms: u64,
}

/// A send. `request_id` is the mouth's transaction id, so a repeat of the same
/// id is the same send. No `basis` is her choice standing: the mouth checks nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendRequest {
    pub request_id: String,
    pub persona: String,
    pub room_alias: String,
    pub text: String,
    pub basis: Option<SendBasis>,
}

/// The mouth's answer to a send: the event it made, or why it did not. `newer` names the events that moved the
/// room, oldest first, exactly when the refusal is `RoomMoved`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendReceipt {
    pub request_id: String,
    pub event_id: Option<String>,
    pub refusal: Option<Refusal>,
    pub newer: Vec<String>,
}

/// A request that the mouth open a DM with `user_id`, `text` being the invite: the ask, and the first line of the room.
/// `request_id` is the mouth's transaction id, so a repeat of the same id is the same open. The mouth bounds `text` to
/// 1..=4000 bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatOpenDirectRequest {
    pub request_id: String,
    pub persona: String,
    pub user_id: String,
    pub text: String,
}

/// How the mouth answered an open, as its typed names. It reads consent from Matrix room state; none is kept here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectOutcome {
    Opened,
    Existing,
    Brake,
    PersonaMismatch,
    UnknownPersona,
    EmptyText,
    ForeignUser,
    NotAPerson,
    NoSuchUser,
    InvitePending,
    Declined,
    /// One creation per person per UTC day.
    DailyLimit,
    SendFailed,
}

impl DirectOutcome {
    const ALL: [Self; 13] = [
        Self::Opened,
        Self::Existing,
        Self::Brake,
        Self::PersonaMismatch,
        Self::UnknownPersona,
        Self::EmptyText,
        Self::ForeignUser,
        Self::NotAPerson,
        Self::NoSuchUser,
        Self::InvitePending,
        Self::Declined,
        Self::DailyLimit,
        Self::SendFailed,
    ];

    /// The mouth's spelling: a MessagePack string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Opened => "Opened",
            Self::Existing => "Existing",
            Self::Brake => "Brake",
            Self::PersonaMismatch => "PersonaMismatch",
            Self::UnknownPersona => "UnknownPersona",
            Self::EmptyText => "EmptyText",
            Self::ForeignUser => "ForeignUser",
            Self::NotAPerson => "NotAPerson",
            Self::NoSuchUser => "NoSuchUser",
            Self::InvitePending => "InvitePending",
            Self::Declined => "Declined",
            Self::DailyLimit => "DailyLimit",
            Self::SendFailed => "SendFailed",
        }
    }
}

impl Serialize for DirectOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for DirectOutcome {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::ALL.into_iter().find(|outcome| outcome.name() == name).ok_or_else(|| serde::de::Error::custom("the mouth named an outcome this side does not know"))
    }
}

/// The mouth's answer to an open. `room_id` and `event_id` are present exactly for `Opened` and `Existing`; a receipt
/// that says otherwise does not decode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawOpenReceipt")]
pub struct ChatOpenDirectReceipt {
    pub request_id: String,
    pub outcome: DirectOutcome,
    pub room_id: Option<String>,
    pub event_id: Option<String>,
}

#[derive(Deserialize)]
struct RawOpenReceipt {
    request_id: String,
    outcome: DirectOutcome,
    room_id: Option<String>,
    event_id: Option<String>,
}

impl TryFrom<RawOpenReceipt> for ChatOpenDirectReceipt {
    type Error = &'static str;

    fn try_from(raw: RawOpenReceipt) -> Result<Self, Self::Error> {
        let landed = matches!(raw.outcome, DirectOutcome::Opened | DirectOutcome::Existing);
        if landed != (raw.room_id.is_some() && raw.event_id.is_some()) || (!landed && (raw.room_id.is_some() || raw.event_id.is_some())) {
            return Err("the open receipt's ids do not match its outcome");
        }
        Ok(Self { request_id: raw.request_id, outcome: raw.outcome, room_id: raw.room_id, event_id: raw.event_id })
    }
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
    /// Deliveries after `after_seq`, oldest first, with the mouth's room tail. A `chat_refusal` answer is an
    /// error that names the refusal.
    fn pending(&self, after_seq: u64) -> Result<ChatPending>;
    fn send(&self, request: &ChatSendRequest) -> Result<ChatSendReceipt>;
    /// Ask the mouth to open a DM. A refusal is a receipt; only a transport fault or a mouth that does not serve the kind is an error.
    fn open_direct(&self, request: &ChatOpenDirectRequest) -> Result<ChatOpenDirectReceipt>;
    /// Every delivery up to and including `through_seq` is answered; the mouth
    /// advances its sync position only now, so a start that dies first has its
    /// deliveries again.
    fn ack(&self, through_seq: u64) -> Result<()>;
    /// The model's input now holds `read.event_id` and every earlier event of its room. A display-only receipt: it
    /// acknowledges nothing and the mouth answers nothing.
    fn read(&self, read: &ChatRead) -> Result<()>;
}

#[cfg(test)]
mod tests;
