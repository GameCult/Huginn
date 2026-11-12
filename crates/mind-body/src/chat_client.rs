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
/// The pending request that asks for the reply link, the same tuple. Opt-in: only a client with the link on sends it.
pub const PENDING_REQUEST_V4: &str = "gamecult.persona.chat_pending_request.v3";
/// The answer a flag-on mouth gives it: a delivery with `reply_to` at tuple index 14. Never asked for by default.
pub const PENDING_V4: &str = "gamecult.persona.chat_pending.v4";
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
/// Her request that the mouth fetch one event's image, verify it and post the receipt as her, and its answer.
pub const FETCH_REQUEST: &str = "gamecult.persona.chat_fetch_request.v1";
pub const FETCH_RECEIPT: &str = "gamecult.persona.chat_fetch_receipt.v1";
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
    /// The earlier event this message replies to, as a `chat_pending.v4` mouth quotes it: tuple index 14. An answer of an older mouth
    /// ends before it and reads as absent, and so does an explicit nil. Display-only evidence: no addressing rule reads it.
    #[serde(default, skip_serializing_if = "ReplyLink::sends_nothing")]
    pub reply_to: ReplyLink,
}

/// The longest excerpt the mouth quotes, in Unicode code points (its `limits.replyExcerpt`).
pub const REPLY_EXCERPT_CHARS: usize = 240;

/// The quote of the event a message replies to: `[event_id, sender, excerpt]`. `excerpt` is untrusted text, none when the mouth sent
/// none (redacted, edited, or not text) or nothing but invisible characters. Built only by `ReplyLink`'s decode, which holds it to the mouth's contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReplyTo {
    pub event_id: String,
    pub sender: String,
    pub excerpt: Option<String>,
}

/// How a reply link out of the mouth's contract fails it, first by tuple order. A class and nothing the mouth sent, so no path can echo
/// what was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyViolation {
    /// Not an array, or an array of arity other than 3.
    NotATriple,
    /// An event id that is not a string, is empty, or is not the shape of one.
    BadEventId,
    /// A sender that is not a string, is empty, or is not the shape of a Matrix user id.
    BadSenderId,
    /// An excerpt that is neither a string nor nil.
    ExcerptNotText,
    /// An excerpt over `REPLY_EXCERPT_CHARS` code points as sent, before invisible characters are stripped.
    ExcerptTooLong,
}

impl ReplyViolation {
    /// The log token of the class.
    pub fn name(self) -> &'static str {
        match self {
            Self::NotATriple => "not_a_triple",
            Self::BadEventId => "bad_event_id",
            Self::BadSenderId => "bad_sender_id",
            Self::ExcerptNotText => "excerpt_not_text",
            Self::ExcerptTooLong => "excerpt_too_long",
        }
    }
}

/// Whether `text` has the shape of a Matrix user id: `@localpart:server`, at most 255 bytes. A shape check, no directory lookup. The
/// localpart is any ASCII graphic byte (historical and appservice ids such as `@_discord_123:host` are legal); the server name is a
/// hostname, IPv4 or `[IPv6]` with an optional port.
fn matrix_user_id(text: &str) -> bool {
    let Some((localpart, server)) = text.strip_prefix('@').and_then(|rest| rest.split_once(':')) else { return false };
    text.len() <= 255
        && !localpart.is_empty()
        && localpart.bytes().all(|byte| byte.is_ascii_graphic())
        && !server.is_empty()
        && server.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".:_[]-".contains(&byte))
}

/// Whether `text` has the shape of an event id: `$` and at least one ASCII graphic byte, at most 255 bytes. Deliberately not a base64
/// check: room versions 3 to 12 and the legacy `$opaque:server` ids all pass, because the mouth forwards whatever the homeserver named.
pub(crate) fn event_id_shape(text: &str) -> bool {
    text.len() <= 255 && text.strip_prefix('$').is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_graphic()))
}

/// The excerpt without the characters that show nothing: variation selectors, the tag block, and every format or private-use character.
/// Removal only; nothing is trimmed, replaced or reordered. Besides the selectors and the Cf/Co categories it removes the Mn/Lo/Zs/So
/// cells that render as nothing: U+034F, U+115F, U+1160, U+17B4, U+17B5, U+180B-180D, U+2800 and U+3164. Other whitespace, NBSP included, stays.
fn without_invisible(text: &str) -> String {
    use unicode_general_category::{GeneralCategory, get_general_category};
    text.chars()
        .filter(|&c| {
            !matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}' | '\u{E0000}'..='\u{E007F}' | '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}' | '\u{180B}'..='\u{180D}' | '\u{2800}' | '\u{3164}')
                && !matches!(get_general_category(c), GeneralCategory::Format | GeneralCategory::PrivateUse)
        })
        .collect()
}

/// A delivery's reply link, tuple index 14: absent, a quote within the mouth's contract, or dropped whole for leaving it. A defective link
/// never fails the poll and is never repaired or clipped; `reply_warnings` names it, and only `quote` reaches a model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ReplyLink {
    #[default]
    Absent,
    Quote(ReplyTo),
    Dropped(ReplyViolation),
}

impl ReplyLink {
    /// The quote, only when the link was within the contract.
    pub fn quote(&self) -> Option<&ReplyTo> {
        match self {
            Self::Quote(quote) => Some(quote),
            _ => None,
        }
    }

    fn sends_nothing(&self) -> bool {
        !matches!(self, Self::Quote(_))
    }
}

impl Serialize for ReplyLink {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Quote(quote) => quote.serialize(serializer),
            _ => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for ReplyLink {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Whatever one element of the document holds, read whole so it can be judged after: an element that errors mid-read cannot be resumed.
        // Nesting past rmp-serde's depth limit (1024) is the one input that still fails the document: the decoder abandons the read and
        // serde cannot resume it.
        enum Element {
            Nil,
            Text(String),
            List(Vec<Element>),
            Other,
        }

        // `top` is the reply link itself, the only level whose list is read item by item; a list inside it is Other and its contents are
        // skipped, so no input can make this read recurse past two levels of its own.
        struct ElementVisitor {
            top: bool,
        }

        struct Item;

        impl<'de> serde::de::DeserializeSeed<'de> for Item {
            type Value = Element;

            fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Element, D::Error> {
                deserializer.deserialize_any(ElementVisitor { top: false })
            }
        }

        impl<'de> serde::de::Visitor<'de> for ElementVisitor {
            type Value = Element;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a reply link element")
            }

            fn visit_unit<E>(self) -> Result<Element, E> {
                Ok(Element::Nil)
            }

            fn visit_none<E>(self) -> Result<Element, E> {
                Ok(Element::Nil)
            }

            fn visit_str<E>(self, text: &str) -> Result<Element, E> {
                Ok(Element::Text(text.to_string()))
            }

            fn visit_string<E>(self, text: String) -> Result<Element, E> {
                Ok(Element::Text(text))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Element, A::Error> {
                if !self.top {
                    while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                    return Ok(Element::Other);
                }
                let mut items = Vec::new();
                while let Some(item) = seq.next_element_seed(Item)? {
                    items.push(item);
                }
                Ok(Element::List(items))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Element, A::Error> {
                while map.next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?.is_some() {}
                Ok(Element::Other)
            }

            fn visit_bool<E>(self, _: bool) -> Result<Element, E> {
                Ok(Element::Other)
            }

            fn visit_i64<E>(self, _: i64) -> Result<Element, E> {
                Ok(Element::Other)
            }

            fn visit_u64<E>(self, _: u64) -> Result<Element, E> {
                Ok(Element::Other)
            }

            fn visit_f64<E>(self, _: f64) -> Result<Element, E> {
                Ok(Element::Other)
            }

            fn visit_bytes<E>(self, _: &[u8]) -> Result<Element, E> {
                Ok(Element::Other)
            }

            // A msgpack ext value reaches the visitor as a newtype struct or an enum (rmp-serde's timestamp and ext shapes), and an
            // `Option` wrapper as `visit_some`: all are out of contract, so read them as Other and let the delivery's link drop.
            fn visit_newtype_struct<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Element, D::Error> {
                serde::de::IgnoredAny::deserialize(deserializer)?;
                Ok(Element::Other)
            }

            fn visit_some<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Element, D::Error> {
                serde::de::IgnoredAny::deserialize(deserializer)?;
                Ok(Element::Other)
            }

            fn visit_enum<A: serde::de::EnumAccess<'de>>(self, data: A) -> Result<Element, A::Error> {
                let (_, variant): (serde::de::IgnoredAny, _) = data.variant()?;
                serde::de::VariantAccess::newtype_variant::<serde::de::IgnoredAny>(variant)?;
                Ok(Element::Other)
            }
        }

        impl<'de> Deserialize<'de> for Element {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_any(ElementVisitor { top: true })
            }
        }

        Ok(match Element::deserialize(deserializer)? {
            Element::Nil => Self::Absent,
            Element::List(items) if items.len() == 3 => {
                let mut items = items.into_iter();
                let (event_id, sender, excerpt) = (items.next(), items.next(), items.next());
                match (event_id, sender, excerpt) {
                    (Some(Element::Text(event_id)), _, _) if !event_id_shape(&event_id) => Self::Dropped(ReplyViolation::BadEventId),
                    (Some(Element::Text(_)), Some(Element::Text(sender)), _) if !matrix_user_id(&sender) => Self::Dropped(ReplyViolation::BadSenderId),
                    (Some(Element::Text(event_id)), Some(Element::Text(sender)), Some(excerpt)) => match excerpt {
                        Element::Nil => Self::Quote(ReplyTo { event_id, sender, excerpt: None }),
                        Element::Text(text) if text.chars().take(REPLY_EXCERPT_CHARS + 1).count() > REPLY_EXCERPT_CHARS => Self::Dropped(ReplyViolation::ExcerptTooLong),
                        Element::Text(text) => {
                            let shown = without_invisible(&text);
                            Self::Quote(ReplyTo { event_id, sender, excerpt: if shown.is_empty() { None } else { Some(shown) } })
                        }
                        _ => Self::Dropped(ReplyViolation::ExcerptNotText),
                    },
                    (Some(Element::Text(_)), _, _) => Self::Dropped(ReplyViolation::BadSenderId),
                    _ => Self::Dropped(ReplyViolation::BadEventId),
                }
            }
            _ => Self::Dropped(ReplyViolation::NotATriple),
        })
    }
}

/// The log line for each delivery whose reply link was dropped and whose event id is not yet in `warned`, which it grows: one warning per
/// event however often the answer is polled. The event id is escaped (`{:?}`) so it cannot start another line, and the line holds nothing
/// else the mouth sent.
pub fn reply_warnings(pending: &ChatPending, warned: &mut std::collections::BTreeSet<String>) -> Vec<String> {
    pending
        .deliveries
        .iter()
        .filter_map(|delivery| match &delivery.reply_to {
            ReplyLink::Dropped(violation) if warned.insert(delivery.event_id.clone()) => Some(format!("reply_to dropped event={:?} violation={}", delivery.event_id, violation.name())),
            _ => None,
        })
        .collect()
}

/// Why the mouth found a delivery to be for the Persona, by what Matrix says (Bifrost `docs/bridge.md` "Addressing facts"). `Tag` and
/// `Room` are the sender's claim in `m.mentions`; `Reply` is checked against the event it targets. Evidence, not authority: the
/// runtime's own rule (`address::addresses`) weighs it. `Room` is reported and wakes no one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Direct,
    Tag,
    Reply,
    /// A reply in a conversation she opened within the mouth's window. The window and its cap are the mouth's; Huginn keeps no state for it.
    FollowUp,
    Room,
}

/// The wire spelling of `Basis::FollowUp`, defined once: decode, encode and the rendered `basis=` all read it. The mouth's word
/// (Bifrost PR #74, `CHAT_FOLLOW=1`); the parser accepts it before the mouth's flag turns on.
pub const FOLLOW_UP_BASIS: &str = "follow";

impl Basis {
    const ALL: [Self; 5] = [Self::Direct, Self::Tag, Self::Reply, Self::FollowUp, Self::Room];

    /// The mouth's spelling: a MessagePack string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Tag => "tag",
            Self::Reply => "reply",
            Self::FollowUp => FOLLOW_UP_BASIS,
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
    /// The rooms whose lines are not all in `tail`, tuple index 3: a room the mouth could not read in time, or whose lines it trimmed. A
    /// reader renders such a room as unknown, never quiet. An answer that ends before the field, or an empty list, names none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tail_gaps: Vec<String>,
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

/// A request that the mouth fetch the image of `event_id` in `room` (an alias or room id) for `persona`: the reference is room and
/// event only. `request_id` makes a repeat the same fetch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFetchRequest {
    pub request_id: String,
    pub persona: String,
    pub room: String,
    pub event_id: String,
}

/// How the mouth answered a fetch, as its fourteen typed names. Whether the file may be fetched is the mouth's, derived from Matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchOutcome {
    /// Fetched, verified against the event's hash, stored. The receipt, if any, says whether it was posted.
    Stored,
    /// Fetched and stored but the event carried no hash: not verified, no receipt, never marked read.
    StoredUnverified,
    NotAMember,
    BridgedRoom,
    TooLarge,
    TypeRefused,
    NoAttachment,
    HashMismatch,
    RateLimited,
    Brake,
    PersonaMismatch,
    UnknownPersona,
    FetchFailed,
    StoreFailed,
}

impl FetchOutcome {
    const ALL: [Self; 14] = [
        Self::Stored,
        Self::StoredUnverified,
        Self::NotAMember,
        Self::BridgedRoom,
        Self::TooLarge,
        Self::TypeRefused,
        Self::NoAttachment,
        Self::HashMismatch,
        Self::RateLimited,
        Self::Brake,
        Self::PersonaMismatch,
        Self::UnknownPersona,
        Self::FetchFailed,
        Self::StoreFailed,
    ];

    /// The mouth's spelling: a MessagePack string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Stored => "Stored",
            Self::StoredUnverified => "StoredUnverified",
            Self::NotAMember => "NotAMember",
            Self::BridgedRoom => "BridgedRoom",
            Self::TooLarge => "TooLarge",
            Self::TypeRefused => "TypeRefused",
            Self::NoAttachment => "NoAttachment",
            Self::HashMismatch => "HashMismatch",
            Self::RateLimited => "RateLimited",
            Self::Brake => "Brake",
            Self::PersonaMismatch => "PersonaMismatch",
            Self::UnknownPersona => "UnknownPersona",
            Self::FetchFailed => "FetchFailed",
            Self::StoreFailed => "StoreFailed",
        }
    }

    /// Whether the mouth stored a file for this outcome: the two that carry a room id and an image.
    pub fn stored(self) -> bool {
        matches!(self, Self::Stored | Self::StoredUnverified)
    }
}

impl Serialize for FetchOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for FetchOutcome {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::ALL.into_iter().find(|outcome| outcome.name() == name).ok_or_else(|| serde::de::Error::custom("the mouth named a fetch outcome this side does not know"))
    }
}

/// The receipt the mouth posted as the Persona for a `Stored` fetch, or found already posted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptMode {
    Posted,
    Existing,
}

impl ReceiptMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Posted => "Posted",
            Self::Existing => "Existing",
        }
    }
}

impl Serialize for ReceiptMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for ReceiptMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        [Self::Posted, Self::Existing].into_iter().find(|mode| mode.name() == name).ok_or_else(|| serde::de::Error::custom("the mouth named a receipt mode this side does not know"))
    }
}

/// The mouth's answer to a fetch: `[request_id, outcome, room_id, image, receipt]`. `room_id` and `image` are present exactly for
/// `Stored` and `StoredUnverified`; `receipt` only with `Stored` (`Stored` with none is valid: nothing was posted, so it is not
/// marked read). A receipt that says otherwise does not decode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawFetchReceipt")]
pub struct ChatFetchReceipt {
    pub request_id: String,
    pub outcome: FetchOutcome,
    pub room_id: Option<String>,
    pub image: Option<ChatImage>,
    pub receipt: Option<ReceiptMode>,
}

#[derive(Deserialize)]
struct RawFetchReceipt {
    request_id: String,
    outcome: FetchOutcome,
    room_id: Option<String>,
    image: Option<ChatImage>,
    receipt: Option<ReceiptMode>,
}

impl TryFrom<RawFetchReceipt> for ChatFetchReceipt {
    type Error = &'static str;

    fn try_from(raw: RawFetchReceipt) -> Result<Self, Self::Error> {
        if raw.outcome.stored() != (raw.room_id.is_some() && raw.image.is_some()) || (!raw.outcome.stored() && (raw.room_id.is_some() || raw.image.is_some())) {
            return Err("the fetch receipt's room and image do not match its outcome");
        }
        if raw.receipt.is_some() && raw.outcome != FetchOutcome::Stored {
            return Err("the fetch receipt posts a receipt for an outcome that is not Stored");
        }
        Ok(Self { request_id: raw.request_id, outcome: raw.outcome, room_id: raw.room_id, image: raw.image, receipt: raw.receipt })
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
    /// Ask the mouth to fetch, verify and receipt one event's image. A refusal is a receipt; only a transport fault or a mouth
    /// that does not serve the kind is an error.
    fn fetch(&self, request: &ChatFetchRequest) -> Result<ChatFetchReceipt>;
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
