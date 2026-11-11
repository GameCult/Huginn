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
// The three-field `chat_pending.v3` of a mouth with the room tail: Bifrost e8722d0's own golden bytes (`tests/fixtures/persona-chat-v5/`
// there), written by its encoder `pendingV3Tuple`, copied here unchanged. The v5 files above are the two-field answer of a mouth before it.
const PENDING_TAG_TAILED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v6/gamecult.persona.chat_pending.v3.tag.msgpack");
const PENDING_UNPROVEN_NOTICE_TAILED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v6/gamecult.persona.chat_pending.v3.unproven-notice.msgpack");
const PENDING_TAIL_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v6/gamecult.persona.chat_pending.v3.tail.msgpack");
// The four-field `chat_pending.v3` with `tail_gaps`: Bifrost b2e0bb2's own golden bytes, copied unchanged.
const PENDING_GAPS_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v7/gamecult.persona.chat_pending.v3.tail-gaps.msgpack");
// Bifrost dc35c4a2's own golden bytes for the reply link (`tests/fixtures/persona-chat-v6/` there), copied unchanged as `persona-chat-v8/`.
const PENDING_V4_REPLY_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v8/gamecult.persona.chat_pending.v4.reply-to.msgpack");
const PENDING_V4_REDACTED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v8/gamecult.persona.chat_pending.v4.reply-to-redacted.msgpack");
const PENDING_V4_NONE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v8/gamecult.persona.chat_pending.v4.no-reply-to.msgpack");
const PENDING_REQUEST_V3_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v8/gamecult.persona.chat_pending_request.v3.msgpack");
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
        reply_to: ReplyLink::Absent,
    }
}

fn image() -> ChatImage {
    let sha256 = "ab".repeat(32);
    ChatImage { path: format!("{}/{sha256}.png", "cd".repeat(32)), mime: "image/png".into(), size: 2048, sha256 }
}

#[test]
fn a_v1_pending_answer_still_decodes_with_no_image() {
    let pending: ChatPending = decode(PENDING_V1_BYTES).unwrap();
    assert_eq!(pending, ChatPending { persona: "eureka".into(), deliveries: vec![delivery(None)], tail: None, tail_gaps: vec![] });
    assert_eq!(PENDING_V1_BYTES[0] & 0xf0, 0x90, "a positional tuple");
}

#[test]
fn a_v2_pending_answer_decodes_with_the_image_at_tuple_index_11_and_no_addressing_facts() {
    let expected = |image| ChatPending { persona: "eureka".into(), deliveries: vec![delivery(image)], tail: None, tail_gaps: vec![] };
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
        ChatPending { persona: "eureka".into(), deliveries: vec![delivery], tail: None, tail_gaps: vec![] }
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

/// Deploy safety: an old mouth's two-field v3 and a new mouth's three-field v3 both decode; only the new one has a tail.
#[test]
fn the_pending_answer_decodes_with_the_tail_field_and_without_it() {
    let one = |is_direct, mentions: &[&str], basis, form, tail| {
        let mut delivery = delivery(None);
        (delivery.is_direct, delivery.mentions, delivery.basis, delivery.form) = (is_direct, mentions.iter().map(|id| (*id).to_string()).collect(), basis, form);
        ChatPending { persona: "eureka".into(), deliveries: vec![delivery], tail, tail_gaps: vec![] }
    };
    for old in [PENDING_TAG_BYTES, PENDING_UNPROVEN_NOTICE_BYTES, PENDING_V1_BYTES, PENDING_NO_IMAGE_BYTES] {
        assert_eq!(decode::<ChatPending>(old).unwrap().tail, None, "a mouth with no tail sends none");
    }
    golden(PENDING_TAG_TAILED_BYTES, &one(false, &["@eureka:gamecult.org", "@nova:gamecult.org"], Some(Basis::Tag), Form::Message, Some(vec![])));
    golden(PENDING_UNPROVEN_NOTICE_TAILED_BYTES, &one(false, &[], None, Form::Notice, Some(vec![])));
    let tailed: ChatPending = decode(PENDING_TAIL_BYTES).unwrap();
    assert!(tailed.deliveries.is_empty());
    let line = |event: &str, at, form, basis, body: &str| TailLine { room: "#operator:gamecult.org".into(), event_id: event.into(), at_unix_ms: at, sender: "@meta:gamecult.org".into(), form, basis, body: body.into() };
    assert_eq!(tailed.tail, Some(vec![line("$e0", 1_700_000_000_000, Form::Message, None, "earlier"), line("$e1", 1_700_000_001_000, Form::Notice, Some(Basis::Tag), "hello eureka")]));
    golden(PENDING_TAIL_BYTES, &tailed);
    assert_eq!(rmp_serde::from_slice::<serde_json::Value>(PENDING_TAIL_BYTES).unwrap()[2][0].as_array().unwrap().len(), 7, "a tail line is seven fields");
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

/// `tail_gaps` is a defaulted fourth element: named rooms round-trip as the mouth's bytes, and an empty list encodes as the three-field answer.
#[test]
fn the_pending_answer_decodes_tail_gaps_and_defaults_them_to_none() {
    let gapped: ChatPending = decode(PENDING_GAPS_BYTES).unwrap();
    assert_eq!(gapped.tail_gaps, vec!["#swarm:gamecult.org".to_string(), "!dm1:gamecult.org".to_string()]);
    golden(PENDING_GAPS_BYTES, &gapped);
    let tailed: ChatPending = decode(PENDING_TAIL_BYTES).unwrap();
    assert!(tailed.tail_gaps.is_empty());
    let mut empty_fourth: serde_json::Value = rmp_serde::from_slice(PENDING_TAIL_BYTES).unwrap();
    empty_fourth.as_array_mut().unwrap().push(serde_json::json!([]));
    assert_eq!(decode::<ChatPending>(&rmp_serde::to_vec(&empty_fourth).unwrap()).unwrap(), tailed, "an empty fourth element is the same answer");
}

fn quote(sender: &str, excerpt: Option<&str>) -> ReplyLink {
    ReplyLink::Quote(ReplyTo { event_id: "$q1".into(), sender: sender.into(), excerpt: excerpt.map(Into::into) })
}

/// The v4 answers carry `reply_to` at delivery index 14. The mouth always sends the fourth, empty `tail_gaps` element on a v4 answer,
/// which this side omits when empty, so a re-encode is the same answer without that trailing empty list.
#[test]
fn the_v4_golden_bytes_decode_and_the_reply_to_ones_reencode() {
    let one = |reply_to| {
        let mut delivery = delivery(None);
        (delivery.is_direct, delivery.mentions, delivery.reply_to) = (false, vec!["@eureka:gamecult.org".into(), "@nova:gamecult.org".into()], reply_to);
        ChatPending { persona: "eureka".into(), deliveries: vec![delivery], tail: Some(vec![]), tail_gaps: vec![] }
    };
    let without_empty_gaps = |bytes: &[u8]| {
        let mut value: serde_json::Value = rmp_serde::from_slice(bytes).unwrap();
        assert_eq!(value.as_array_mut().unwrap().pop(), Some(serde_json::json!([])));
        value
    };
    for (bytes, expected) in [(PENDING_V4_REPLY_BYTES, one(quote("@alice:gamecult.org", Some("can someone look at the build?")))), (PENDING_V4_REDACTED_BYTES, one(quote("@alice:gamecult.org", None)))] {
        assert_eq!(decode::<ChatPending>(bytes).unwrap(), expected);
        let reencoded: serde_json::Value = rmp_serde::from_slice(&encode(&expected).unwrap()).unwrap();
        assert_eq!(reencoded, without_empty_gaps(bytes));
        assert_eq!(reencoded[1][0].as_array().unwrap().len(), 15);
    }
    // An explicit nil at index 14 reads as none; a re-encode drops it, so this fixture is decode-only.
    let none = decode::<ChatPending>(PENDING_V4_NONE_BYTES).unwrap();
    assert_eq!(none, one(ReplyLink::Absent));
    assert_eq!(rmp_serde::from_slice::<serde_json::Value>(&encode(&none).unwrap()).unwrap()[1][0].as_array().unwrap().len(), 14);
}

#[test]
fn the_v1_to_v3_answers_still_decode_with_no_reply_to() {
    for old in [PENDING_V1_BYTES, PENDING_NO_IMAGE_BYTES, PENDING_IMAGE_BYTES, PENDING_TAG_BYTES, PENDING_UNPROVEN_NOTICE_BYTES, PENDING_TAG_TAILED_BYTES, PENDING_TAIL_BYTES, PENDING_GAPS_BYTES] {
        assert!(decode::<ChatPending>(old).unwrap().deliveries.iter().all(|delivery| delivery.reply_to == ReplyLink::Absent));
    }
    golden(PENDING_TAG_BYTES, &decode::<ChatPending>(PENDING_TAG_BYTES).unwrap());
    golden(PENDING_GAPS_BYTES, &decode::<ChatPending>(PENDING_GAPS_BYTES).unwrap());
}

/// The valid reply-to answer with its delivery's index 14 swapped for `element`, as msgpack.
fn answer_with(element: serde_json::Value) -> Vec<u8> {
    let mut value: serde_json::Value = rmp_serde::from_slice(PENDING_V4_REPLY_BYTES).unwrap();
    value[1][0][14] = element;
    rmp_serde::to_vec(&value).unwrap()
}

fn link_of(bytes: &[u8]) -> ReplyLink {
    decode::<ChatPending>(bytes).unwrap().deliveries.remove(0).reply_to
}

#[test]
fn a_bad_reply_to_drops_that_quote_and_the_rest_of_the_poll_decodes() {
    let good: serde_json::Value = rmp_serde::from_slice(PENDING_V4_REPLY_BYTES).unwrap();
    let valid = good[1][0].clone();
    let mut bad = valid.clone();
    bad[14] = serde_json::json!(["$q1", "@alice:gamecult.org", 5]);
    bad[3] = "$bad".into();
    let mut document = good.clone();
    document[1] = serde_json::json!([valid.clone(), bad, valid]);
    let pending: ChatPending = decode(&rmp_serde::to_vec(&document).unwrap()).unwrap();
    let expected: ChatPending = decode(PENDING_V4_REPLY_BYTES).unwrap();
    assert_eq!(pending.deliveries.len(), 3);
    assert_eq!((&pending.deliveries[0], &pending.deliveries[2]), (&expected.deliveries[0], &expected.deliveries[0]));
    let dropped = &pending.deliveries[1];
    assert_eq!(dropped.reply_to, ReplyLink::Dropped(ReplyViolation::ExcerptNotText));
    assert!(dropped.reply_to.quote().is_none());
    assert_eq!(ChatDelivery { event_id: "$e1".into(), reply_to: expected.deliveries[0].reply_to.clone(), ..dropped.clone() }, expected.deliveries[0], "the delivery is otherwise unchanged");
    assert_eq!((&pending.persona, &pending.tail, &pending.tail_gaps), (&expected.persona, &expected.tail, &expected.tail_gaps));
    let reencoded: serde_json::Value = rmp_serde::from_slice(&encode(&pending).unwrap()).unwrap();
    assert_eq!(reencoded[1][1].as_array().unwrap().len(), 14, "a dropped link is not sent again");
}

#[test]
fn every_reply_to_violation_class_drops_the_quote_with_its_name() {
    use serde_json::json;
    let long = "x".repeat(241);
    let rows = [
        (json!(["", "@a:b", "t"]), ReplyViolation::BadEventId),
        (json!([7, "@a:b", "t"]), ReplyViolation::BadEventId),
        (json!(["$q", "", "t"]), ReplyViolation::BadSenderId),
        (json!(["$q", true, "t"]), ReplyViolation::BadSenderId),
        (json!(["$q", "@a:b", 5]), ReplyViolation::ExcerptNotText),
        (json!(["$q", "@a:b", false]), ReplyViolation::ExcerptNotText),
        (json!(["$q", "@a:b", ["t"]]), ReplyViolation::ExcerptNotText),
        (json!(["$q", "@a:b", long]), ReplyViolation::ExcerptTooLong),
        (json!(["$q", "@a:b"]), ReplyViolation::NotATriple),
        (json!(["$q", "@a:b", "t", "u"]), ReplyViolation::NotATriple),
        (json!("text"), ReplyViolation::NotATriple),
        (json!(5), ReplyViolation::NotATriple),
        (json!({"a": 1}), ReplyViolation::NotATriple),
        (json!(["", "", 5]), ReplyViolation::BadEventId),
        (json!(["$q", "", 5]), ReplyViolation::BadSenderId),
    ];
    for (element, class) in rows {
        assert_eq!(link_of(&answer_with(element.clone())), ReplyLink::Dropped(class), "{element}");
    }
    assert_eq!(link_of(&answer_with(serde_json::Value::Null)), ReplyLink::Absent);
    assert!(matches!(link_of(&answer_with(serde_json::json!(["$q", "@a:b", "x".repeat(240)]))), ReplyLink::Quote(_)));
    assert!(matches!(link_of(&answer_with(serde_json::json!(["$q", "@a:b", "\u{1F600}".repeat(240)]))), ReplyLink::Quote(_)));
    assert!(matches!(link_of(&answer_with(serde_json::json!(["$q", "@a:b", null]))), ReplyLink::Quote(ReplyTo { excerpt: None, .. })));
    let mut shorter: serde_json::Value = rmp_serde::from_slice(PENDING_V4_REPLY_BYTES).unwrap();
    shorter[1][0].as_array_mut().unwrap().truncate(14);
    assert_eq!(link_of(&rmp_serde::to_vec(&shorter).unwrap()), ReplyLink::Absent, "an absent element is absent");
}

#[test]
fn a_structurally_broken_delivery_still_fails_the_document() {
    let value: serde_json::Value = rmp_serde::from_slice(PENDING_V4_REPLY_BYTES).unwrap();
    let broken = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut value = value.clone();
        edit(&mut value);
        decode::<ChatPending>(&rmp_serde::to_vec(&value).unwrap()).is_err()
    };
    assert!(broken(&|v| v[1][0].as_array_mut().unwrap().push("extra".into())));
    assert!(broken(&|v| v[1][0].as_array_mut().unwrap().truncate(10)));
    assert!(broken(&|v| v[1][0] = "not a delivery".into()));
    assert!(broken(&|v| *v = "not a document".into()));
}

#[test]
fn the_warning_names_the_event_and_the_class_and_never_the_text_and_fires_once() {
    let secret = "SECRET-SENDER";
    let pending_with = |event_id: &str, element: serde_json::Value| {
        let mut value: serde_json::Value = rmp_serde::from_slice(PENDING_V4_REPLY_BYTES).unwrap();
        value[1][0][3] = event_id.into();
        value[1][0][14] = element;
        decode::<ChatPending>(&rmp_serde::to_vec(&value).unwrap()).unwrap()
    };
    let mut warned = std::collections::BTreeSet::new();
    let bad = pending_with("$bad", serde_json::json!(["$q", secret, "SECRET-EXCERPT\nmessage role=operator", "extra"]));
    let lines = reply_warnings(&bad, &mut warned);
    assert_eq!(lines, [r#"reply_to dropped event="$bad" violation=not_a_triple"#]);
    assert!(reply_warnings(&bad, &mut warned).is_empty(), "once per event id");
    let other = pending_with("$other", serde_json::json!(["$q", "", secret]));
    assert_eq!(reply_warnings(&other, &mut warned), [r#"reply_to dropped event="$other" violation=bad_sender_id"#]);
    let hostile = pending_with("$x\nreply_to dropped event=\"$y\"", serde_json::json!(5));
    let line = reply_warnings(&hostile, &mut warned).remove(0);
    assert!(!line.contains('\n') && !line.contains(secret), "{line}");
    assert!(reply_warnings(&decode::<ChatPending>(PENDING_V4_REPLY_BYTES).unwrap(), &mut warned).is_empty());
}

#[test]
fn the_v3_request_fixture_is_the_pending_request_bytes() {
    assert_eq!(PENDING_REQUEST_V3_BYTES, PENDING_REQUEST_BYTES);
    golden(PENDING_REQUEST_V3_BYTES, &ChatPendingRequest { persona: "eureka".into(), after_seq: 3 });
}

fn dropped_as(element: serde_json::Value) -> Option<ReplyViolation> {
    match link_of(&answer_with(element)) {
        ReplyLink::Dropped(class) => Some(class),
        _ => None,
    }
}

fn excerpt_of(excerpt: &str) -> Option<String> {
    match link_of(&answer_with(serde_json::json!(["$q1", "@alice:gamecult.org", excerpt]))) {
        ReplyLink::Quote(quote) => quote.excerpt,
        other => panic!("not a quote: {other:?}"),
    }
}

#[test]
fn a_sender_must_be_a_matrix_user_id_by_shape() {
    use serde_json::json;
    let sized = |bytes: usize| format!("@{}:h", "a".repeat(bytes - 3));
    let good = ["@alice:gamecult.org", "@a:b", "@_discord_123:gamecult.org", "@a.b=c/d+e-f:host:8448", "@x:[::1]:8448", "@x:1.2.3.4"];
    for sender in good.iter().map(|s| s.to_string()).chain([sized(255)]) {
        assert!(matches!(link_of(&answer_with(json!(["$q1", sender, "t"]))), ReplyLink::Quote(_)), "{sender}");
    }
    let bad = [sized(256), "".into(), "alice".into(), "@alice".into(), "@alice:".into(), "@:host".into(), "@".into(), ":host".into(), "alice:host".into(), "@a b:host".into(), "@a:ho st".into(), "@a\tb:h".into(), "@a:h\n".into(), "@a\u{0}:h".into(), "@a\u{7f}:h".into(), "@a\u{85}:h".into(), "@a\u{2028}:h".into(), "@\u{e5}lice:host".into(), "@a:h\u{e9}st".into(), "@alice:gamecult.org ".into(), "@a\u{200b}:h".into(), "@a\u{202e}:h".into(), "$event:host".into(), "!room:host".into()];
    for sender in bad {
        assert_eq!(dropped_as(json!(["$q1", sender, "t"])), Some(ReplyViolation::BadSenderId), "{sender:?}");
    }
    for sender in [json!(5), json!(null), json!(["@a:b"])] {
        assert_eq!(dropped_as(json!(["$q1", sender, "t"])), Some(ReplyViolation::BadSenderId));
    }
    let hostile = ["$q1", "", "t"];
    assert_eq!(dropped_as(json!(["", "@a:b", "t"])), Some(ReplyViolation::BadEventId));
    assert_eq!(dropped_as(json!(hostile)), Some(ReplyViolation::BadSenderId));
}

#[test]
fn an_event_id_must_be_an_event_id_by_shape() {
    use serde_json::json;
    let v4 = format!("${}", &"aB-_".repeat(11)[..43]);
    let v3 = format!("${}", &"aB+/".repeat(11)[..43]);
    for id in ["$q1".to_string(), v4, v3, "$abc123:matrix.org".into(), "$abc:[::1]:8448".into(), format!("${}", "a".repeat(254))] {
        assert!(matches!(link_of(&answer_with(json!([id, "@a:b", "t"]))), ReplyLink::Quote(_)), "{id}");
    }
    for id in [format!("${}", "a".repeat(255)), "".into(), "$".into(), "q1".into(), "!room:host".into(), "@u:host".into(), "$a b".into(), "$a\tb".into(), "$a\nb".into(), "$a\u{0}b".into(), "$a\u{7f}b".into(), "$a\u{85}b".into(), "$a\u{2028}b".into(), "$a\u{e9}b".into(), "$a\u{200b}b".into(), "$ab ".into(), " $ab".into()] {
        assert_eq!(dropped_as(json!([id, "@a:b", "t"])), Some(ReplyViolation::BadEventId), "{id:?}");
    }
    for id in [json!(5), json!(null), json!(["$a"])] {
        assert_eq!(dropped_as(json!([id, "@a:b", "t"])), Some(ReplyViolation::BadEventId));
    }
}

#[test]
fn variation_selectors_are_stripped_from_the_excerpt_and_nothing_else_changes() {
    for c in ['\u{FE00}', '\u{FE0F}', '\u{FE05}', '\u{E0100}', '\u{E01EF}', '\u{E0150}'] {
        assert_eq!(excerpt_of(&format!("ok{c}")).as_deref(), Some("ok"), "{c:?}");
    }
    assert_eq!(excerpt_of("ok\u{FE00}\u{FE0F}\u{E0100}\u{E01EF}").as_deref(), Some("ok"));
    for c in ['\u{FDFF}', '\u{FE10}', '\u{E00FF}', '\u{E01F0}'] {
        let sent = format!("ok{c}");
        let got = excerpt_of(&sent);
        // U+E00FF is inside the tag block's neighbour range only if listed; it is a plain unassigned cell and is kept.
        assert_eq!(got.as_deref(), Some(sent.as_str()), "{c:?}");
    }
    assert_eq!(excerpt_of("\u{2764}\u{FE0F}").as_deref(), Some("\u{2764}"));
    assert_eq!(excerpt_of(" a  b ").as_deref(), Some(" a  b "));
    assert_eq!(excerpt_of("\tTab Case").as_deref(), Some("\tTab Case"));
}

#[test]
fn format_and_private_use_characters_are_stripped_from_the_excerpt() {
    for c in ['\u{200D}', '\u{200B}', '\u{200C}', '\u{200E}', '\u{200F}', '\u{061C}', '\u{00AD}', '\u{202E}', '\u{2066}', '\u{2060}', '\u{FEFF}', '\u{E000}', '\u{F8FF}', '\u{F0000}', '\u{10FFFD}', '\u{E0001}', '\u{E0049}', '\u{E007F}', '\u{E0002}'] {
        assert_eq!(excerpt_of(&format!("a{c}b")).as_deref(), Some("ab"), "{c:?}");
    }
    for kept in ["caf\u{e9} \u{1F600} \u{4E2D}\u{6587}", "e\u{0301}", "a\u{180B}b"] {
        assert_eq!(excerpt_of(kept).as_deref(), Some(kept));
    }
    let mixed = format!("{}{}", "a".repeat(120), "\u{200B}".repeat(120));
    assert_eq!(excerpt_of(&mixed).as_deref(), Some("a".repeat(120).as_str()));
}

#[test]
fn an_excerpt_emptied_by_stripping_is_none_and_the_quote_stays() {
    for sent in ["\u{FE0F}", "\u{200B}\u{E0049}", &"\u{FE0F}\u{200D}".repeat(5), ""] {
        let link = link_of(&answer_with(serde_json::json!(["$q1", "@alice:gamecult.org", sent])));
        assert_eq!(link, ReplyLink::Quote(ReplyTo { event_id: "$q1".into(), sender: "@alice:gamecult.org".into(), excerpt: None }), "{sent:?}");
    }
    assert_eq!(excerpt_of(" ").as_deref(), Some(" "));
    assert_eq!(link_of(&answer_with(serde_json::json!(["$q1", "@alice:gamecult.org", null]))), quote("@alice:gamecult.org", None));
}

#[test]
fn the_excerpt_bound_is_checked_before_the_strip_and_never_clips() {
    let long = |visible: usize, extra: &str| serde_json::json!(["$q1", "@a:b", format!("{}{extra}", "x".repeat(visible))]);
    assert_eq!(excerpt_of(&"x".repeat(240)).map(|t| t.chars().count()), Some(240));
    assert_eq!(dropped_as(long(241, "")), Some(ReplyViolation::ExcerptTooLong));
    assert_eq!(dropped_as(long(240, "\u{FE0F}")), Some(ReplyViolation::ExcerptTooLong));
    assert_eq!(excerpt_of(&format!("{}\u{FE0F}", "x".repeat(239))).map(|t| t.chars().count()), Some(239));
    assert_eq!(dropped_as(serde_json::json!(["$q1", "@a:b", "\u{FE0F}".repeat(1_000_000)])), Some(ReplyViolation::ExcerptTooLong));
}

/// Invalid UTF-8 in the excerpt's bytes is a defect of that quote, never of the document.
#[test]
fn invalid_utf8_in_an_excerpt_drops_the_quote_and_not_the_document() {
    let good = answer_with(serde_json::json!(["$q1", "@alice:gamecult.org", "can someone look at the build?"]));
    let at = good.windows(4).position(|w| w == b"can ").unwrap();
    let mut bad = good.clone();
    bad[at] = 0xff;
    let pending: ChatPending = decode(&bad).expect("the document decodes");
    assert!(matches!(pending.deliveries[0].reply_to, ReplyLink::Dropped(ReplyViolation::ExcerptNotText)), "{:?}", pending.deliveries[0].reply_to);
}

#[test]
fn the_bifrost_fixtures_still_decode_as_quotes() {
    assert_eq!(link_of(PENDING_V4_REPLY_BYTES), quote("@alice:gamecult.org", Some("can someone look at the build?")));
    assert_eq!(link_of(PENDING_V4_REDACTED_BYTES), quote("@alice:gamecult.org", None));
    assert_eq!(link_of(PENDING_V4_NONE_BYTES), ReplyLink::Absent);
}
