//! The chat documents against the mouth's golden bytes (`tests/fixtures/persona-chat-v1/` for the pending, ack and refusal
//! kinds, Bifrost `eureka-body/persona-mouth` ab2bdc6; `persona-chat-v2/` for the send and its receipt, copied from Bifrost `eureka-body/optimistic-send-mouth`):
//! each decodes into the typed document and the encoder reproduces it byte for
//! byte. The mouth reads positional tuples and refuses maps.

use super::*;

const REFUSAL_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_refusal.v1.msgpack");
const ACK_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_ack.v1.msgpack");
const PENDING_V1_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending.v1.msgpack");
const PENDING_NO_IMAGE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v3/gamecult.persona.chat_pending.v2.no-image.msgpack");
const PENDING_IMAGE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v3/gamecult.persona.chat_pending.v2.image.msgpack");
const PENDING_REQUEST_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v5/gamecult.persona.chat_pending_request.v2.msgpack");
const PENDING_REQUEST_V1_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending_request.v1.msgpack");
const PENDING_TAG_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v5/gamecult.persona.chat_pending.v3.tag.msgpack");
const PENDING_UNPROVEN_NOTICE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v5/gamecult.persona.chat_pending.v3.unproven-notice.msgpack");
const SENT_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_receipt.v2.sent.msgpack");
const ROOM_MOVED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_receipt.v2.room-moved.msgpack");
const NIL_BASIS_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.nil-basis.msgpack");
const SEEN_AND_SINCE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.seen-and-since.msgpack");
const SINCE_ONLY_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.since-only.msgpack");
const OPEN_REQUEST_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_request.v1.msgpack");
const OPENED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_receipt.v1.opened.msgpack");
const EXISTING_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_receipt.v1.existing.msgpack");
const DECLINED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_receipt.v1.declined.msgpack");
const UNKNOWN_KIND_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v4/gamecult.persona.chat_refusal.v1.unknown-kind.msgpack");

fn golden<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(bytes: &[u8], expected: &T) {
    assert_eq!(&decode::<T>(bytes).unwrap(), expected);
    assert_eq!(encode(expected).unwrap(), bytes, "the encoder reproduces the mouth's bytes");
    assert_eq!(bytes[0] & 0xf0, 0x90, "a positional tuple, not a map");
}

#[test]
fn the_pending_request_and_ack_are_the_mouths_bytes() {
    golden(PENDING_REQUEST_BYTES, &ChatPendingRequest { persona: "eureka".into(), after_seq: 3 });
    // The v1 kind carries the same tuple: asking by the v2 kind is the only difference.
    assert_eq!(PENDING_REQUEST_V1_BYTES, PENDING_REQUEST_BYTES);
    golden(ACK_BYTES, &ChatAck { persona: "eureka".into(), through_seq: 7 });
}

fn delivery(image: Option<ChatImage>) -> ChatDelivery {
    ChatDelivery {
        seq: 7,
        room_alias: "#operator:gamecult.org".into(),
        room_id: "!abc:gamecult.org".into(),
        event_id: "$e1".into(),
        sender: "@meta:gamecult.org".into(),
        sender_display: "Meta".into(),
        text: "hello eureka".into(),
        at_unix_ms: 1_761_700_000_000,
        context: vec![ContextLine { sender: "@eureka:gamecult.org".into(), text: "earlier".into(), at_unix_ms: 1_761_699_999_000 }],
        mentions: vec!["@eureka:gamecult.org".into(), "@nova:gamecult.org".into()],
        is_direct: true,
        image,
        basis: None,
        form: Form::Message,
    }
}

fn image() -> ChatImage {
    let sha256 = "ab".repeat(32);
    ChatImage { path: format!("{}/{sha256}.png", "cd".repeat(32)), mime: "image/png".into(), size: 2048, sha256 }
}

#[test]
fn a_v1_pending_answer_still_decodes_with_no_image() {
    let pending: ChatPending = decode(PENDING_V1_BYTES).unwrap();
    assert_eq!(pending, ChatPending { persona: "eureka".into(), deliveries: vec![delivery(None)] });
    assert_eq!(PENDING_V1_BYTES[0] & 0xf0, 0x90, "a positional tuple");
}

#[test]
fn a_v2_pending_answer_decodes_with_the_image_at_tuple_index_11_and_no_addressing_facts() {
    let expected = |image| ChatPending { persona: "eureka".into(), deliveries: vec![delivery(image)] };
    assert_eq!(decode::<ChatPending>(PENDING_NO_IMAGE_BYTES).unwrap(), expected(None));
    assert_eq!(decode::<ChatPending>(PENDING_IMAGE_BYTES).unwrap(), expected(Some(image())));
    // Index 11 of the delivery tuple, by position: 12 elements, the last an array of [path, mime, size, sha256].
    assert_eq!(shape::delivery_tuple(PENDING_IMAGE_BYTES), shape::Probe { delivery_len: 12, image_len: Some(4) });
    assert_eq!(shape::delivery_tuple(PENDING_NO_IMAGE_BYTES), shape::Probe { delivery_len: 12, image_len: None });
    // A v1 request is answered with that 12-field tuple: no basis is unproven, and the event is a message.
    let old = decode::<ChatPending>(PENDING_IMAGE_BYTES).unwrap().deliveries.remove(0);
    assert_eq!((old.basis, old.form), (None, Form::Message));
}

#[test]
fn a_v3_pending_answer_is_the_golden_bytes_with_basis_and_form_at_tuple_indices_12_and_13() {
    let one = |is_direct, mentions: &[&str], basis, form| {
        let mut delivery = delivery(None);
        (delivery.is_direct, delivery.mentions, delivery.basis, delivery.form) = (is_direct, mentions.iter().map(|id| (*id).to_string()).collect(), basis, form);
        ChatPending { persona: "eureka".into(), deliveries: vec![delivery] }
    };
    golden(PENDING_TAG_BYTES, &one(false, &["@eureka:gamecult.org", "@nova:gamecult.org"], Some(Basis::Tag), Form::Message));
    golden(PENDING_UNPROVEN_NOTICE_BYTES, &one(false, &[], None, Form::Notice));
    let fields = |bytes: &[u8]| {
        let value: serde_json::Value = rmp_serde::from_slice(bytes).unwrap();
        let tuple = value[1][0].as_array().unwrap().clone();
        (tuple.len(), tuple[12].clone(), tuple[13].clone())
    };
    assert_eq!(fields(PENDING_TAG_BYTES), (14, "tag".into(), "message".into()));
    assert_eq!(fields(PENDING_UNPROVEN_NOTICE_BYTES), (14, serde_json::Value::Null, "notice".into()));
}

#[test]
fn a_basis_or_form_this_side_does_not_know_does_not_decode() {
    let mut value: serde_json::Value = rmp_serde::from_slice(PENDING_TAG_BYTES).unwrap();
    value[1][0][12] = "everyone".into();
    assert!(decode::<ChatPending>(&rmp_serde::to_vec(&value).unwrap()).is_err());
    value[1][0][12] = "tag".into();
    value[1][0][13] = "reaction".into();
    assert!(decode::<ChatPending>(&rmp_serde::to_vec(&value).unwrap()).is_err());
}

/// Reads the tuple's shape from the raw bytes, independent of the serde types under test.
mod shape {
    #[derive(Debug, PartialEq, Eq)]
    pub struct Probe {
        pub delivery_len: usize,
        pub image_len: Option<usize>,
    }

    pub fn delivery_tuple(bytes: &[u8]) -> Probe {
        let value: serde_json::Value = rmp_serde::from_slice(bytes).unwrap();
        let delivery = &value[1][0];
        let array = delivery.as_array().expect("a delivery is a tuple");
        Probe { delivery_len: array.len(), image_len: array[11].as_array().map(Vec::len) }
    }
}

#[test]
fn a_refusal_is_the_mouths_bytes_and_a_typed_name() {
    golden(REFUSAL_BYTES, &ChatRefusal { persona: "ghost".into(), refusal: Refusal::UnknownPersona });
}

#[test]
fn a_send_and_its_receipt_are_the_mouths_bytes() {
    let request = |basis: Option<SendBasis>| ChatSendRequest { request_id: "req-1".into(), persona: "eureka".into(), room_alias: "#swarm:gamecult.org".into(), text: "hi".into(), basis };
    golden(NIL_BASIS_BYTES, &request(None));
    golden(SEEN_AND_SINCE_BYTES, &request(Some(SendBasis { seen_event_id: Some("$seen".into()), since_unix_ms: 1_793_698_425_998 })));
    golden(SINCE_ONLY_BYTES, &request(Some(SendBasis { seen_event_id: None, since_unix_ms: 1_793_698_425_998 })));
    golden(SENT_BYTES, &ChatSendReceipt { request_id: "req-1".into(), event_id: Some("$sent".into()), refusal: None, newer: vec![] });
    golden(ROOM_MOVED_BYTES, &ChatSendReceipt { request_id: "req-1".into(), event_id: None, refusal: Some(Refusal::RoomMoved), newer: vec!["$a".into(), "$b".into()] });
}

#[test]
fn open_direct_fixtures_round_trip() {
    golden(OPEN_REQUEST_BYTES, &ChatOpenDirectRequest { request_id: "req-1".into(), persona: "eureka".into(), user_id: "@meta:gamecult.org".into(), text: "hello Meta".into() });
    golden(OPENED_BYTES, &ChatOpenDirectReceipt { request_id: "req-1".into(), outcome: DirectOutcome::Opened, room_id: Some("!dm1:gamecult.org".into()), event_id: Some("$line1".into()) });
    golden(EXISTING_BYTES, &ChatOpenDirectReceipt { request_id: "req-2".into(), outcome: DirectOutcome::Existing, room_id: Some("!dm1:gamecult.org".into()), event_id: Some("$line2".into()) });
    golden(DECLINED_BYTES, &ChatOpenDirectReceipt { request_id: "req-3".into(), outcome: DirectOutcome::Declined, room_id: None, event_id: None });
    golden(UNKNOWN_KIND_BYTES, &ChatRefusal { persona: "eureka".into(), refusal: Refusal::UnknownKind });
}

#[test]
fn every_outcome_is_spelled_as_the_mouth_spells_it() {
    let all = [
        (DirectOutcome::Opened, "Opened"),
        (DirectOutcome::Existing, "Existing"),
        (DirectOutcome::Brake, "Brake"),
        (DirectOutcome::PersonaMismatch, "PersonaMismatch"),
        (DirectOutcome::UnknownPersona, "UnknownPersona"),
        (DirectOutcome::EmptyText, "EmptyText"),
        (DirectOutcome::ForeignUser, "ForeignUser"),
        (DirectOutcome::NotAPerson, "NotAPerson"),
        (DirectOutcome::NoSuchUser, "NoSuchUser"),
        (DirectOutcome::InvitePending, "InvitePending"),
        (DirectOutcome::Declined, "Declined"),
        (DirectOutcome::DailyLimit, "DailyLimit"),
        (DirectOutcome::SendFailed, "SendFailed"),
    ];
    for (outcome, name) in all {
        assert_eq!(outcome.name(), name);
        let refused = outcome != DirectOutcome::Opened && outcome != DirectOutcome::Existing;
        let ids = if refused { (None, None) } else { (Some("!r:x".to_string()), Some("$e".to_string())) };
        let receipt = ChatOpenDirectReceipt { request_id: "q".into(), outcome, room_id: ids.0, event_id: ids.1 };
        assert_eq!(decode::<ChatOpenDirectReceipt>(&encode(&receipt).unwrap()).unwrap(), receipt, "{name}");
    }
}

#[test]
fn a_receipt_with_ids_on_a_refusal_does_not_decode() {
    let wire = |outcome: &str, room: Option<&str>, event: Option<&str>| rmp_serde::to_vec(&("q", outcome, room, event)).unwrap();
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Declined", Some("!r:x"), Some("$e"))).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Declined", Some("!r:x"), None)).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Declined", None, Some("$e"))).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Opened", None, None)).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Opened", Some("!r:x"), None)).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Existing", None, Some("$e"))).is_err());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Existing", Some("!r:x"), Some("$e"))).is_ok());
    assert!(decode::<ChatOpenDirectReceipt>(&wire("Declined", None, None)).is_ok());
}

#[test]
fn a_receipt_error_never_echoes_what_it_read() {
    let canary = "CANARY-9f3a";
    let bad_outcome = rmp_serde::to_vec(&("q", canary, None::<&str>, None::<&str>)).unwrap();
    let bad_ids = rmp_serde::to_vec(&(canary, "Declined", Some(canary), Some(canary))).unwrap();
    for bytes in [bad_outcome, bad_ids] {
        let error = format!("{:#}", decode::<ChatOpenDirectReceipt>(&bytes).unwrap_err());
        assert!(!error.contains(canary), "{error}");
    }
}
