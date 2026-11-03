//! The chat documents against the mouth's golden bytes (`tests/fixtures/persona-chat-v1/` for the pending, ack and refusal
//! kinds, Bifrost `eureka-body/persona-mouth` ab2bdc6; `persona-chat-v2/` for the send and its receipt, copied from Bifrost `eureka-body/optimistic-send-mouth`):
//! each decodes into the typed document and the encoder reproduces it byte for
//! byte. The mouth reads positional tuples and refuses maps.

use super::*;

const REFUSAL_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_refusal.v1.msgpack");
const ACK_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_ack.v1.msgpack");
const PENDING_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending.v1.msgpack");
const PENDING_REQUEST_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending_request.v1.msgpack");
const SENT_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_receipt.v2.sent.msgpack");
const ROOM_MOVED_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_receipt.v2.room-moved.msgpack");
const NIL_BASIS_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.nil-basis.msgpack");
const SEEN_AND_SINCE_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.seen-and-since.msgpack");
const SINCE_ONLY_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.since-only.msgpack");

fn golden<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(bytes: &[u8], expected: &T) {
    assert_eq!(&decode::<T>(bytes).unwrap(), expected);
    assert_eq!(encode(expected).unwrap(), bytes, "the encoder reproduces the mouth's bytes");
    assert_eq!(bytes[0] & 0xf0, 0x90, "a positional tuple, not a map");
}

#[test]
fn the_pending_request_and_ack_are_the_mouths_bytes() {
    golden(PENDING_REQUEST_BYTES, &ChatPendingRequest { persona: "eureka".into(), after_seq: 3 });
    golden(ACK_BYTES, &ChatAck { persona: "eureka".into(), through_seq: 7 });
}

#[test]
fn a_pending_answer_is_the_mouths_bytes_with_its_context() {
    let delivery = ChatDelivery {
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
    };
    golden(PENDING_BYTES, &ChatPending { persona: "eureka".into(), deliveries: vec![delivery] });
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
