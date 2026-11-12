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
use mind_body::chat_client::{self, ReplyLink, ReplyViolation, Basis, Form, ChatRefusal, MouthRefused, Refusal, REFUSAL, ACK, Chat, ChatAck, ChatFetchReceipt, ChatFetchRequest, ChatOpenDirectReceipt, ChatOpenDirectRequest, DirectOutcome, FetchOutcome, FETCH_RECEIPT, FETCH_REQUEST, ReceiptMode, OPEN_DIRECT_RECEIPT, OPEN_DIRECT_REQUEST, ChatPending, ChatPendingRequest, ChatRead, ChatSendReceipt, ChatSendRequest, PENDING, PENDING_REQUEST, PENDING_REQUEST_V1, PENDING_REQUEST_V4, PENDING_V1, PENDING_V2, PENDING_V4, READ, SEND_RECEIPT, SEND_REQUEST};
use mind_body::chat_rudp::{RudpChat, connection_id};

const ACK_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_ack.v1.msgpack");
const PENDING_V1_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_pending.v1.msgpack");
const PENDING_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v5/gamecult.persona.chat_pending.v3.tag.msgpack");
const PENDING_TAILED_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v6/gamecult.persona.chat_pending.v3.tag.msgpack");
const PENDING_TAIL_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v6/gamecult.persona.chat_pending.v3.tail.msgpack");
// The four-field `chat_pending.v3` of a mouth with `tail_gaps`: Bifrost b2e0bb2's own golden bytes, copied unchanged.
const PENDING_GAPS_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v7/gamecult.persona.chat_pending.v3.tail-gaps.msgpack");
const PENDING_V2_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v3/gamecult.persona.chat_pending.v2.image.msgpack");
const PENDING_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v5/gamecult.persona.chat_pending_request.v2.msgpack");
const PENDING_REQUEST_V1_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_pending_request.v1.msgpack");
// Bifrost dc35c4a2's own golden bytes for the reply link, copied unchanged as `persona-chat-v8/`.
const PENDING_V4_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v8/gamecult.persona.chat_pending.v4.reply-to.msgpack");
const SEND_RECEIPT_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v2/gamecult.persona.chat_send_receipt.v2.sent.msgpack");
const SEND_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v2/gamecult.persona.chat_send_request.v2.nil-basis.msgpack");
const OPEN_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_request.v1.msgpack");
const UNKNOWN_KIND_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v4/gamecult.persona.chat_refusal.v1.unknown-kind.msgpack");
const REFUSAL_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v1/gamecult.persona.chat_refusal.v1.msgpack");
const OPENED_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v4/gamecult.persona.chat_open_direct_receipt.v1.opened.msgpack");

// Bifrost 3a6b45d's own golden bytes for the fetch, copied unchanged as `persona-chat-v9/` (`tests/fixtures/persona-chat-v7/` there).
const FETCH_REQUEST_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v9/gamecult.persona.chat_fetch_request.v1.msgpack");
const FETCH_POSTED_BYTES: &[u8] = include_bytes!("fixtures/persona-chat-v9/gamecult.persona.chat_fetch_receipt.v1.stored-posted.msgpack");

const KEY: &str = "connection-key-for-test";
const TAG: &str = "cultnet.transport.rudp.v0";

/// What a fixture mouth answers an open with: the document kind, a record key (none: the request's own), and its bytes.
type OpenReply = (&'static str, Option<&'static str>, &'static [u8]);

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
    mouth_at(key, "127.0.0.1:0")
}

/// A mouth that has no image field yet: it answers a pending request in the v1 kind, and does not serve the v2 request.
fn legacy_mouth(key: &str) -> Mouth {
    serve(key, "127.0.0.1:0", PENDING_V1, PENDING_V1_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES))
}

/// A mouth that has the image field and not the addressing facts: it answers the v1 request in the v2 kind, and does not serve the v2 request.
fn imaged_mouth(key: &str) -> Mouth {
    serve(key, "127.0.0.1:0", PENDING_V2, PENDING_V2_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES))
}

/// A mouth that answers the reply-link request (`chat_pending_request.v3`) with `chat_pending.v4`, and does not serve the older kinds.
fn linked_mouth(key: &str) -> Mouth {
    serve(key, "127.0.0.1:0", PENDING_V4, PENDING_V4_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES))
}

/// The same mouth on a chosen address: a restart binds the address the old one held.
fn mouth_at(key: &str, bind: &str) -> Mouth {
    serve(key, bind, PENDING, PENDING_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES))
}

fn serve(key: &str, bind: &str, pending_kind: &'static str, pending_bytes: &'static [u8], open: OpenReply) -> Mouth {
    let socket = UdpSocket::bind(bind).unwrap();
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
                PENDING_REQUEST if for_eureka(&chat_client::decode::<ChatPendingRequest>(&document.payload).unwrap().persona) => {
                    Some(if pending_kind == PENDING { answer(pending_kind, "eureka", pending_bytes) } else { answer(REFUSAL, "eureka", UNKNOWN_KIND_BYTES) })
                }
                PENDING_REQUEST_V4 if for_eureka(&chat_client::decode::<ChatPendingRequest>(&document.payload).unwrap().persona) => {
                    Some(if pending_kind == PENDING_V4 { answer(pending_kind, "eureka", pending_bytes) } else { answer(REFUSAL, "eureka", UNKNOWN_KIND_BYTES) })
                }
                PENDING_REQUEST_V1 if pending_kind != PENDING && for_eureka(&chat_client::decode::<ChatPendingRequest>(&document.payload).unwrap().persona) => Some(answer(pending_kind, "eureka", pending_bytes)),
                SEND_REQUEST if for_eureka(&chat_client::decode::<ChatSendRequest>(&document.payload).unwrap().persona) => Some(answer(SEND_RECEIPT, &document.record_key, SEND_RECEIPT_BYTES)),
                OPEN_DIRECT_REQUEST if for_eureka(&chat_client::decode::<ChatOpenDirectRequest>(&document.payload).unwrap().persona) => Some(answer(open.0, open.1.unwrap_or(&document.record_key), open.2)),
                FETCH_REQUEST if for_eureka(&chat_client::decode::<ChatFetchRequest>(&document.payload).unwrap().persona) => Some(answer(FETCH_RECEIPT, &document.record_key, FETCH_POSTED_BYTES)),
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

    let deliveries = chat.pending(3).unwrap().deliveries;
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
fn a_read_reaches_the_mouth_as_the_positional_tuple_persona_room_event_instant() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "eureka", Duration::from_secs(5));
    let read = ChatRead { persona: "eureka".into(), room: "!abc:gamecult.org".into(), event_id: "$e1".into(), at_unix_ms: 1_761_700_000_000 };
    chat.read(&read).unwrap();
    let seen = mouth.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, READ);
    assert_eq!(chat_client::decode::<ChatRead>(&seen[0].1).unwrap(), read);
    let tuple: serde_json::Value = rmp_serde::from_slice(&seen[0].1).unwrap();
    assert_eq!(tuple, serde_json::json!(["eureka", "!abc:gamecult.org", "$e1", 1_761_700_000_000u64]), "a positional tuple, not a map");
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
    let sent = ChatSendRequest { request_id: "r".into(), persona: "someone-else".into(), room_alias: "#a:b".into(), text: "x".into(), basis: None };
    assert!(chat.send(&sent).is_err());
}

#[test]
fn a_client_recovers_from_a_restarted_mouth_on_its_next_call() {
    let first = mouth(KEY);
    let address = first.address;
    let chat = client(&first, KEY, "eureka", Duration::from_millis(1500));
    assert!(chat.pending(0).is_ok());
    drop(first);
    let second = mouth_at(KEY, &address.to_string());
    assert!(chat.pending(0).is_err(), "the restarted mouth ignores a link it never saw: the call times out");
    assert_eq!(chat.pending(0).unwrap(), chat_client::decode::<ChatPending>(PENDING_BYTES).unwrap(), "the next call opens a fresh link");
    assert_eq!(chat.pending(0).unwrap().deliveries.len(), 1, "and keeps it");
    let reads = second.seen.lock().unwrap().iter().filter(|(kind, _)| kind == PENDING_REQUEST).count();
    assert_eq!(reads, 2, "the restarted mouth read the two requests that reached it on a fresh link");
}

#[test]
fn a_send_that_timed_out_is_not_repeated_by_the_client() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "someone-else", Duration::from_millis(600));
    let sent = ChatSendRequest { request_id: "r-1".into(), persona: "someone-else".into(), room_alias: "#a:b".into(), text: "x".into(), basis: None };
    assert!(chat.send(&sent).is_err());
    chat.ack(1).unwrap();
    let sends: Vec<_> = mouth.seen.lock().unwrap().iter().filter(|(kind, _)| kind == SEND_REQUEST).map(|(_, payload)| chat_client::decode::<ChatSendRequest>(payload).unwrap().request_id).collect();
    assert_eq!(sends, ["r-1"], "the send reached the mouth once; a repeat is the caller's, under the same request id");
}

#[test]
fn a_mouth_without_the_v2_request_is_asked_the_v1_way_and_its_deliveries_carry_no_addressing_facts() {
    for (mouth, text) in [(legacy_mouth(KEY), "v1 answer"), (imaged_mouth(KEY), "v2 answer")] {
        let old = client(&mouth, KEY, "eureka", Duration::from_millis(1500)).pending(3).unwrap();
        assert_eq!(old.tail, None, "{text}: a v1 request is answered with no tail");
        let old = old.deliveries;
        assert_eq!(old.len(), 1, "{text}");
        assert_eq!((old[0].basis, old[0].form), (None, Form::Message), "{text}");
        let seen: Vec<String> = mouth.seen.lock().unwrap().iter().map(|(kind, _)| kind.clone()).collect();
        assert_eq!(seen, [PENDING_REQUEST, PENDING_REQUEST_V1], "{text}: the v2 request first, refused as an unknown kind, then the v1 request");
        assert_eq!(mouth.seen.lock().unwrap()[1].1, PENDING_REQUEST_V1_BYTES, "{text}");
    }
    assert!(client(&imaged_mouth(KEY), KEY, "eureka", Duration::from_millis(1500)).pending(0).unwrap().deliveries[0].image.is_some());
}

#[test]
fn a_current_mouth_answers_the_v2_request_with_basis_and_form() {
    let current = mouth(KEY);
    let new = client(&current, KEY, "eureka", Duration::from_millis(1500)).pending(3).unwrap().deliveries;
    assert_eq!((new[0].basis, new[0].form), (Some(Basis::Tag), Form::Message));
    let kinds: Vec<String> = current.seen.lock().unwrap().iter().map(|(kind, _)| kind.clone()).collect();
    assert_eq!(kinds, [PENDING_REQUEST], "no v1 request follows an answered v2 request");
}

fn kinds_seen(mouth: &Mouth) -> Vec<String> {
    mouth.seen.lock().unwrap().iter().map(|(kind, _)| kind.clone()).collect()
}

#[test]
fn a_flag_off_client_asks_only_the_v2_kind_and_a_flag_on_client_asks_v4_first() {
    let current = mouth(KEY);
    let off = client(&current, KEY, "eureka", Duration::from_millis(1500)).with_reply_link(false).pending(3).unwrap();
    assert!(off.deliveries[0].reply_to.quote().is_none());
    assert_eq!(kinds_seen(&current), [PENDING_REQUEST], "flag off sends exactly today's request");
    assert_eq!(current.seen.lock().unwrap()[0].1, PENDING_REQUEST_BYTES);

    let linked = linked_mouth(KEY);
    let on = client(&linked, KEY, "eureka", Duration::from_millis(1500)).with_reply_link(true).pending(3).unwrap();
    assert_eq!(kinds_seen(&linked), [PENDING_REQUEST_V4]);
    let quote = on.deliveries[0].reply_to.quote().cloned().expect("the reply link crossed the wire");
    assert_eq!((quote.event_id.as_str(), quote.sender.as_str(), quote.excerpt.as_deref()), ("$q1", "@alice:gamecult.org", Some("can someone look at the build?")));
}

#[test]
fn a_bad_reply_to_over_rudp_delivers_the_poll_warns_once_and_counts() {
    let mut value: serde_json::Value = rmp_serde::from_slice(PENDING_V4_BYTES).unwrap();
    let valid = value[1][0].clone();
    let mut bad = valid.clone();
    bad[3] = "$bad".into();
    bad[14] = serde_json::json!(["$q1", "@alice:gamecult.org", 5]);
    value[1] = serde_json::json!([valid.clone(), bad, valid]);
    let bytes: &'static [u8] = Box::leak(rmp_serde::to_vec(&value).unwrap().into_boxed_slice());
    let check = |pending: chat_client::ChatPending| {
        assert_eq!(pending.deliveries.len(), 3);
        assert!(pending.deliveries[0].reply_to.quote().is_some() && pending.deliveries[2].reply_to.quote().is_some());
        assert_eq!(pending.deliveries[1].reply_to, ReplyLink::Dropped(ReplyViolation::ExcerptNotText));
    };
    let mouth = serve(KEY, "127.0.0.1:0", PENDING_V4, bytes, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES));
    let chat = client(&mouth, KEY, "eureka", Duration::from_millis(1500)).with_reply_link(true);
    check(chat.pending(3).unwrap());
    assert_eq!(chat.reply_defects(), 1);
    check(chat.pending(3).unwrap());
    assert_eq!(chat.reply_defects(), 1, "the same event is not counted again");
    // The lenient decode does not depend on the request kind: a flag-off client reads the same bytes.
    let current = serve(KEY, "127.0.0.1:0", PENDING, bytes, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES));
    let off = client(&current, KEY, "eureka", Duration::from_millis(1500));
    check(off.pending(3).unwrap());
    assert_eq!(off.reply_defects(), 1);
}

#[test]
fn a_flag_on_client_steps_down_from_a_mouth_that_refuses_the_v4_kind() {
    let current = mouth(KEY);
    let pending = client(&current, KEY, "eureka", Duration::from_millis(1500)).with_reply_link(true).pending(3).unwrap();
    assert_eq!(kinds_seen(&current), [PENDING_REQUEST_V4, PENDING_REQUEST], "a flag-off mouth refuses v4 and answers v2");
    assert!(pending.deliveries[0].reply_to.quote().is_none() && pending.deliveries[0].basis == Some(Basis::Tag));
    for old in [legacy_mouth(KEY), imaged_mouth(KEY)] {
        let pending = client(&old, KEY, "eureka", Duration::from_millis(1500)).with_reply_link(true).pending(3).unwrap();
        assert_eq!(kinds_seen(&old), [PENDING_REQUEST_V4, PENDING_REQUEST, PENDING_REQUEST_V1]);
        assert!(pending.deliveries[0].reply_to.quote().is_none());
    }
}

#[test]
fn an_open_crosses_a_real_rudp_session_in_the_mouths_bytes_and_a_receipt_for_another_open_is_refused() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "eureka", Duration::from_secs(5));
    let request = chat_client::decode::<ChatOpenDirectRequest>(OPEN_REQUEST_BYTES).unwrap();
    let receipt = chat.open_direct(&request).unwrap();
    assert_eq!(receipt, chat_client::decode::<ChatOpenDirectReceipt>(OPENED_BYTES).unwrap());
    assert_eq!(receipt.outcome, DirectOutcome::Opened);

    let another = ChatOpenDirectRequest { request_id: "req-other".into(), ..request };
    assert!(chat.open_direct(&another).is_err(), "the answer names req-1, not req-other");
    let seen = mouth.seen.lock().unwrap().clone();
    assert_eq!(seen[0], (OPEN_DIRECT_REQUEST.to_string(), OPEN_REQUEST_BYTES.to_vec()));
}

#[test]
fn a_fetch_crosses_a_real_rudp_session_in_the_mouths_bytes_and_a_receipt_for_another_fetch_is_refused() {
    let mouth = mouth(KEY);
    let chat = client(&mouth, KEY, "eureka", Duration::from_secs(5));
    let request = chat_client::decode::<ChatFetchRequest>(FETCH_REQUEST_BYTES).unwrap();
    let receipt = chat.fetch(&request).unwrap();
    assert_eq!(receipt, chat_client::decode::<ChatFetchReceipt>(FETCH_POSTED_BYTES).unwrap());
    assert_eq!((receipt.outcome, receipt.receipt), (FetchOutcome::Stored, Some(ReceiptMode::Posted)));

    let another = ChatFetchRequest { request_id: "req-other".into(), ..request };
    assert!(chat.fetch(&another).is_err(), "the answer names req-f1, not req-other");
    let seen = mouth.seen.lock().unwrap().clone();
    assert_eq!(seen[0], (FETCH_REQUEST.to_string(), FETCH_REQUEST_BYTES.to_vec()));
}

/// A mouth that answers every open with `reply`.
fn opening_with(reply: OpenReply) -> Mouth {
    serve(KEY, "127.0.0.1:0", PENDING, PENDING_BYTES, reply)
}

#[test]
fn a_mouth_that_does_not_serve_the_open_kind_refuses_it_and_the_open_is_told_not_sent_at_once() {
    let mouth = opening_with((REFUSAL, Some("eureka"), UNKNOWN_KIND_BYTES));
    let chat = client(&mouth, KEY, "eureka", Duration::from_secs(20));
    let request = chat_client::decode::<ChatOpenDirectRequest>(OPEN_REQUEST_BYTES).unwrap();
    let started = std::time::Instant::now();
    let error = chat.open_direct(&request).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5), "the refusal ends the wait: {:?}", started.elapsed());
    assert_eq!(error.downcast_ref::<MouthRefused>(), Some(&MouthRefused(Refusal::UnknownKind)), "{error}");
}

#[test]
fn a_refusal_for_another_persona_or_another_kind_does_not_satisfy_the_open() {
    let wrong_persona = opening_with((REFUSAL, Some("someone-else"), UNKNOWN_KIND_BYTES));
    let request = chat_client::decode::<ChatOpenDirectRequest>(OPEN_REQUEST_BYTES).unwrap();
    let error = client(&wrong_persona, KEY, "eureka", Duration::from_millis(800)).open_direct(&request).unwrap_err();
    assert!(error.downcast_ref::<MouthRefused>().is_none(), "a refusal keyed by another persona is not heard: {error}");
    let wrong_kind = opening_with((SEND_RECEIPT, Some("eureka"), SEND_RECEIPT_BYTES));
    let error = client(&wrong_kind, KEY, "eureka", Duration::from_millis(800)).open_direct(&request).unwrap_err();
    assert!(error.downcast_ref::<MouthRefused>().is_none(), "another kind is not heard: {error}");
}

#[test]
fn a_refusal_of_another_kind_ends_the_wait_but_is_not_told_as_not_sent() {
    // The fixture refusal is persona "ghost", UnknownPersona: the mouth refuses this Persona's open for a reason other than the kind.
    let mouth = opening_with((REFUSAL, Some("ghost"), REFUSAL_BYTES));
    let request = chat_client::decode::<ChatOpenDirectRequest>(OPEN_REQUEST_BYTES).unwrap();
    let started = std::time::Instant::now();
    let error = client(&mouth, KEY, "ghost", Duration::from_secs(20)).open_direct(&request).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5), "the refusal ends the wait: {:?}", started.elapsed());
    assert!(error.downcast_ref::<MouthRefused>().is_none(), "only UnknownKind is a known not-sent: {error}");
    assert!(error.to_string().contains("UnknownPersona"), "{error}");
}

#[test]
fn an_unknown_kind_refusal_keyed_by_this_persona_but_naming_another_is_not_heard_as_not_sent() {
    let other = chat_client::encode(&ChatRefusal { persona: "someone-else".into(), refusal: Refusal::UnknownKind }).unwrap();
    let mouth = opening_with((REFUSAL, Some("eureka"), Box::leak(other.into_boxed_slice())));
    let request = chat_client::decode::<ChatOpenDirectRequest>(OPEN_REQUEST_BYTES).unwrap();
    let error = client(&mouth, KEY, "eureka", Duration::from_secs(20)).open_direct(&request).unwrap_err();
    assert!(error.downcast_ref::<MouthRefused>().is_none(), "the payload names another Persona: {error}");
}

/// The wire, not only the decoder: the three-field `chat_pending.v3` of Bifrost e8722d0 crosses a real RUDP session as `chat_pending.v3`,
/// and the tail arrives; the two-field answer of the mouth before it (the fixture the other tests serve) arrives with none.
#[test]
fn the_three_field_v3_crosses_real_rudp_with_its_tail_and_the_two_field_one_with_none() {
    let tailed = serve(KEY, "127.0.0.1:0", PENDING, PENDING_TAIL_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES));
    let got = client(&tailed, KEY, "eureka", Duration::from_secs(5)).pending(0).unwrap();
    assert_eq!(got, chat_client::decode::<ChatPending>(PENDING_TAIL_BYTES).unwrap());
    assert_eq!(got.tail.as_ref().map(Vec::len), Some(2));
    let with_delivery = serve(KEY, "127.0.0.1:0", PENDING, PENDING_TAILED_BYTES, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES));
    let got = client(&with_delivery, KEY, "eureka", Duration::from_secs(5)).pending(0).unwrap();
    assert_eq!((got.deliveries.len(), got.tail), (1, Some(vec![])), "a delivery and an empty tail: the mouth had nothing to add");
    let old = client(&mouth(KEY), KEY, "eureka", Duration::from_secs(5)).pending(0).unwrap();
    assert_eq!((old.deliveries.len(), old.tail), (1, None), "the older mouth's two-field answer carries no tail");
}

/// The three-field answer with a fourth element appended, as a four-field mouth would send it.
fn four_field(three: &[u8], gaps: &[&str]) -> &'static [u8] {
    let mut value: serde_json::Value = rmp_serde::from_slice(three).unwrap();
    value.as_array_mut().unwrap().push(serde_json::json!(gaps));
    Box::leak(rmp_serde::to_vec(&value).unwrap().into_boxed_slice())
}

/// Deploy safety for `tail_gaps` (Bifrost b2e0bb2): the three-field answer, the four-field one with gaps and the four-field one with
/// none all cross real RUDP and decode. Only a named room is a gap; a quiet room is never in the list.
#[test]
fn the_four_field_v3_crosses_real_rudp_with_its_gaps_and_the_three_field_one_with_none() {
    let fetch = |bytes: &'static [u8]| client(&serve(KEY, "127.0.0.1:0", PENDING, bytes, (OPEN_DIRECT_RECEIPT, None, OPENED_BYTES)), KEY, "eureka", Duration::from_secs(5)).pending(0).unwrap();
    let three = fetch(PENDING_TAIL_BYTES);
    assert_eq!((three.tail.as_ref().map(Vec::len), three.tail_gaps), (Some(2), Vec::<String>::new()), "a three-field answer names no gap");
    let gapped = fetch(PENDING_GAPS_BYTES);
    assert_eq!((gapped.deliveries.len(), gapped.tail.as_ref().map(Vec::len)), (0, Some(1)));
    assert_eq!(gapped.tail_gaps, vec!["#swarm:gamecult.org".to_string(), "!dm1:gamecult.org".to_string()], "the mouth's own bytes name two rooms");
    let empty = fetch(four_field(PENDING_TAIL_BYTES, &[]));
    assert_eq!((empty.tail.as_ref().map(Vec::len), empty.tail_gaps), (Some(2), Vec::<String>::new()), "an empty fourth element is every room read fully");
    let quiet = fetch(four_field(PENDING_TAIL_BYTES, &["#swarm:gamecult.org"]));
    assert!(!quiet.tail_gaps.contains(&"#operator:gamecult.org".to_string()), "a room the mouth did not name is not a gap, however few lines it has");
}
