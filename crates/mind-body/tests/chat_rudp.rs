//! The chat transport against a real cultnet-rs RUDP server speaking the mouth's
//! documents: the mouth's golden fixtures are what the server expects to read
//! and what it answers with, so the client is held to the mouth's bytes in both
//! directions. The server is the mouth's behaviour (Bifrost
//! `startChatEndpoint`): it answers a pending request with a pending document,
//! a send with a receipt, an ack with nothing, and a request for another
//! Persona with nothing.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cultnet_rs::{CultNetMessage, CultNetRawDocumentRecord, CultNetRawPayloadEncoding, CultNetRudpSocketTransportConnection, CultNetRudpSocketTransportOptions};
use mind_body::chat_client::{self, ACK, Chat, ChatAck, ChatPending, ChatPendingRequest, ChatSendReceipt, ChatSendRequest, PENDING, PENDING_REQUEST, SEND_RECEIPT, SEND_REQUEST};
use mind_body::chat_rudp::{RudpChat, connection_id};

const ACK_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_ack.v1.msgpack");
const PENDING_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_pending.v1.msgpack");
const PENDING_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_pending_request.v1.msgpack");
const SEND_RECEIPT_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_send_receipt.v1.msgpack");
const SEND_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_send_request.v1.msgpack");

const KEY: &str = "connection-key-for-test";
const TAG: &str = "cultnet.transport.rudp.v0";

type Seen = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

struct Mouth {
    address: SocketAddr,
    seen: Seen,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Mouth {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn answer(kind: &str, record_key: &str, payload: &[u8]) -> CultNetMessage {
    CultNetMessage::DocumentPutRaw {
        message_id: format!("{kind}:{record_key}"),
        document: CultNetRawDocumentRecord {
            schema_id: kind.into(),
            record_key: record_key.into(),
            stored_at: "2026-10-29T00:00:00.000Z".into(),
            payload_encoding: CultNetRawPayloadEncoding::Messagepack,
            payload: payload.to_vec(),
            source_runtime_id: Some("bifrost-persona-mouth-eureka".into()),
            source_agent_id: Some("eureka".into()),
            source_role: Some("bifrost-persona-matrix-mouth".into()),
            tags: Some(vec![TAG.into()]),
        },
    }
}

/// The mouth of the Persona `eureka`, whose connection id derives from `key`.
fn mouth(key: &str) -> Mouth {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let address = socket.local_addr().unwrap();
    let mut server = CultNetRudpSocketTransportConnection::new(CultNetRudpSocketTransportOptions::server("mouth-test", socket, connection_id(key))).unwrap();
    let (seen, stop) = (Seen::default(), Arc::new(AtomicBool::new(false)));
    let (log, halt) = (seen.clone(), stop.clone());
    let thread = std::thread::spawn(move || {
        while !halt.load(Ordering::SeqCst) {
            let Ok(Some(CultNetMessage::DocumentPutRaw { document, .. })) = server.receive_schema_message_once() else {
                let _ = server.poll_resends();
                continue;
            };
            assert_eq!(document.tags.as_deref(), Some(&[TAG.to_string()][..]), "the mouth refuses a document whose tag is not its own");
            assert_eq!(document.payload_encoding, CultNetRawPayloadEncoding::Messagepack);
            log.lock().unwrap().push((document.schema_id.clone(), document.payload.clone()));
            let for_eureka = |persona: &str| persona == "eureka";
            let reply = match document.schema_id.as_str() {
                PENDING_REQUEST if for_eureka(&chat_client::decode::<ChatPendingRequest>(&document.payload).unwrap().persona) => Some(answer(PENDING, "eureka", PENDING_BYTES)),
                SEND_REQUEST if for_eureka(&chat_client::decode::<ChatSendRequest>(&document.payload).unwrap().persona) => Some(answer(SEND_RECEIPT, &document.record_key, SEND_RECEIPT_BYTES)),
                _ => None,
            };
            if let Some(reply) = reply {
                server.send_schema_message(&reply).unwrap();
            }
        }
    });
    Mouth { address, seen, stop, thread: Some(thread) }
}

fn client(mouth: &Mouth, key: &str, persona: &str, timeout: Duration) -> RudpChat {
    RudpChat::new(mouth.address, connection_id(key), persona, timeout)
}

#[test]
fn the_connection_id_is_the_first_four_bytes_of_the_trimmed_keys_sha256_as_the_mouth_derives_it() {
    // Node: createHash("sha256").update("connection-key-for-test").digest().readUInt32BE(0)
    assert_eq!(connection_id(KEY), 3_188_925_986);
    assert_eq!(connection_id(&format!("  {KEY}\r\n")), 3_188_925_986, "the key file's whitespace is trimmed, as the mouth reads it");
    assert_ne!(connection_id("another-key"), connection_id(KEY));
}

#[test]
fn pending_send_and_ack_cross_a_real_rudp_session_in_the_mouths_bytes() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "eureka", Duration::from_secs(5));

    let deliveries = chat.pending(3).unwrap();
    assert_eq!(deliveries, chat_client::decode::<ChatPending>(PENDING_BYTES).unwrap().deliveries);
    assert_eq!(deliveries[0].seq, 7);

    let request = chat_client::decode::<ChatSendRequest>(SEND_REQUEST_BYTES).unwrap();
    let receipt = chat.send(&request).unwrap();
    assert_eq!(receipt, chat_client::decode::<ChatSendReceipt>(SEND_RECEIPT_BYTES).unwrap());

    chat.ack(chat_client::decode::<ChatAck>(ACK_BYTES).unwrap().through_seq).unwrap();

    // What the mouth read is its fixtures, byte for byte, in the order they were made.
    let seen = mouth.seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![
            (PENDING_REQUEST.to_string(), PENDING_REQUEST_BYTES.to_vec()),
            (SEND_REQUEST.to_string(), SEND_REQUEST_BYTES.to_vec()),
            (ACK.to_string(), ACK_BYTES.to_vec()),
        ]
    );
}

#[test]
fn a_peer_without_the_credential_cannot_open_the_session() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, "not-the-key", "eureka", Duration::from_millis(600));
    assert!(chat.pending(0).is_err());
    assert!(mouth.seen.lock().unwrap().is_empty(), "nothing reached the mouth");
}

#[test]
fn a_request_the_mouth_does_not_answer_ends_in_an_error_not_a_hang() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "someone-else", Duration::from_millis(600));
    assert!(chat.pending(0).is_err(), "the mouth answers only its own Persona");
    let sent = ChatSendRequest { request_id: "r".into(), persona: "someone-else".into(), room_alias: "#a:b".into(), text: "x".into() };
    assert!(chat.send(&sent).is_err());
}
