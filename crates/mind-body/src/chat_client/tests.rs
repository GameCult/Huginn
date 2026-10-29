//! The chat documents against the mouth's own golden bytes (Bifrost
//! `eureka-body/persona-mouth` ab2bdc6, `tests/fixtures/persona-chat-v1/`):
//! each decodes into the typed document and the encoder reproduces it byte for
//! byte. The mouth reads positional tuples and refuses maps.

use super::*;

const ACK_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_ack.v1.msgpack");
const PENDING_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending.v1.msgpack");
const PENDING_REQUEST_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_pending_request.v1.msgpack");
const SEND_RECEIPT_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_send_receipt.v1.msgpack");
const SEND_REQUEST_BYTES: &[u8] = include_bytes!("../../tests/fixtures/persona-chat-v1/gamecult.persona.chat_send_request.v1.msgpack");

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
    };
    golden(PENDING_BYTES, &ChatPending { persona: "eureka".into(), deliveries: vec![delivery] });
}

#[test]
fn a_send_and_its_receipt_are_the_mouths_bytes() {
    golden(
        SEND_REQUEST_BYTES,
        &ChatSendRequest { request_id: "req-1".into(), persona: "eureka".into(), room_alias: "#operator:gamecult.org".into(), text: "hi".into() },
    );
    golden(SEND_RECEIPT_BYTES, &ChatSendReceipt { request_id: "req-1".into(), event_id: Some("$sent".into()), refusal: None });
}
