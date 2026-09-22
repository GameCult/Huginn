//! The loop: one UDP socket, one reliable session per peer, one reply per
//! frame on the frame's own session. The hub is CultLib's; what is here is the
//! routing, the schema catalog and the order the process starts in.
//!
//! Stated limits. A socket error and a hostile datagram are indistinguishable
//! at `receive_event_once`, so both are logged and served past: a dead socket
//! spins with logging rather than exiting, and Cut 14's health check is the
//! observer.
//!
//! One answer may be `MAX_RESPONSE_BYTES` encoded, 1,228,800 today: the
//! reliable window one session holds, `MAX_PENDING_RELIABLE_PACKETS` packets
//! of `MAX_FRAGMENT_BYTES`. A larger answer is refused by name,
//! `MindRefusal::ResponseTooLarge`, carrying the encoded size and the limit,
//! and the caller narrows its own request. This is the current bound and not a
//! design target: a single cut spec at the leaf's own bounds does not fit it,
//! and what a list read should return is an open question, so a client author
//! should expect the number to move.
//!
//! Four things this cut does not do, which a client has nowhere else to learn:
//!
//! - A session idle for `ServeOptions::session_timeout`, 30 seconds, is
//!   removed here while the client still believes it is connected; its next
//!   request is answered by nothing. A client that intends to stay must speak
//!   inside that window or reconnect.
//! - An envelope CultNet's own `validate_message` rejects is dropped by the
//!   hub before this module sees it, so it is never answered.
//! - Shutdown neither drains what is in flight nor disconnects its peers: the
//!   loop stops and the sockets close under whatever was queued.
//! - `response-not-encodable` is authored here and is unreachable in practice;
//!   every response type encodes.
//!
//! Two reads pipelined on one session are not both answered: the window is
//! per session and counts what is unacknowledged, so a second large reply sent
//! before the first is acknowledged fails to send even when each passes the
//! size gate on its own. The gate measures one answer against the window, not
//! against what the session still has room for.
//!
//! The same counting makes the window one packet short until a session has
//! settled: the hub's own accept is unacknowledged reliable traffic, so a
//! session that has just connected holds 1023 packets rather than 1024 until
//! the client's acknowledgement arrives. On loopback that happens before the
//! first request, and a lost acknowledgement leaves the hole open one resend
//! longer. Nothing here depends on it; a test that measures the boundary
//! exactly does, and settles the session first.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultNetMessage, CultNetRudpServerEvent, CultNetRudpServerHub,
    CultNetRudpServerHubOptions, CultNetSchemaKind, CultNetSchemaRegistration, CultNetSchemaRegistry,
    CultNetWireContract, decode_cultnet_message_from_slice, encode_cultnet_message_to_vec,
};
use huginn_mind::epiphany_pipeline::Slug;
use huginn_mind::wire::{
    HuginnMindResponse, MIND_REQUEST_SCHEMA, MIND_REQUEST_SCHEMA_JSON, MIND_RESPONSE_SCHEMA,
    MIND_RESPONSE_SCHEMA_JSON, MIND_SERVICE_ID,
};
use huginn_mind::{MindRefusal, MindStore, OwnedRedbMessagePackBackingStore};

use crate::daemon::{Daemon, IndexSink, NoIndex};
use crate::envelope::{decode_request, encode_failure, encode_response};

/// What the loop does when nothing is waiting.
pub struct ServeOptions {
    pub session_timeout: Duration,
    pub idle_sleep: Duration,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self { session_timeout: Duration::from_secs(30), idle_sleep: Duration::from_millis(2) }
    }
}

/// The three things the operator supplies. Nothing here reads the environment:
/// how Idunn supplies `--bind` is Cut 14's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub state_root: PathBuf,
    pub instance: Slug,
    pub bind: SocketAddr,
}

/// Exactly `--state-root`, `--instance` and `--bind`, each required, each once.
pub fn parse_options(args: impl Iterator<Item = String>) -> Result<Options> {
    let mut args = args;
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    while let Some(name) = args.next() {
        let name = name.strip_prefix("--").with_context(|| format!("expected --option, got {name:?}"))?;
        ensure!(matches!(name, "state-root" | "instance" | "bind"), "unsupported Huginn option --{name}");
        let value = args.next().with_context(|| format!("missing value for --{name}"))?;
        ensure!(values.insert(name.to_owned(), value).is_none(), "duplicate Huginn option --{name}");
    }
    let take = |name: &str| -> Result<String> {
        values.get(name).cloned().with_context(|| format!("--{name} is required"))
    };
    let state_root = PathBuf::from(take("state-root")?);
    ensure!(state_root.is_absolute(), "--state-root must be an absolute path");
    let bind = take("bind")?;
    Ok(Options {
        state_root,
        instance: Slug(take("instance")?),
        bind: bind.parse().with_context(|| format!("--bind must be an ip:port, got {bind:?}"))?,
    })
}

/// Ruling 15, as an order: the mind opens first, and only a mind that opened
/// gets a socket. This is the one place both happen, so nothing can bind ahead
/// of the refusal.
pub fn startup(
    options: &Options,
) -> Result<(Daemon<OwnedRedbMessagePackBackingStore, NoIndex>, CultNetRudpServerHub, CultNetSchemaRegistry)> {
    let daemon = Daemon::open(&options.state_root, &options.instance)?;
    let hub = bind(options.bind, &daemon.runtime_id())?;
    Ok((daemon, hub, schema_registry()?))
}

/// The bytes one reliable packet carries, and the packets one session may hold
/// unacknowledged. Their product is the largest answer a single send can
/// carry, and `answer` measures every reply against it rather than handing the
/// hub a send it can already see will fail.
pub const MAX_FRAGMENT_BYTES: u32 = 1200;
pub const MAX_PENDING_RELIABLE_PACKETS: u32 = 1024;
/// 1,228,800 bytes.
pub const MAX_RESPONSE_BYTES: u64 = MAX_FRAGMENT_BYTES as u64 * MAX_PENDING_RELIABLE_PACKETS as u64;

/// One non-blocking socket serving the operation connection id. The hub drops
/// every datagram carrying another one.
pub fn bind(addr: SocketAddr, runtime_id: &str) -> Result<CultNetRudpServerHub> {
    let socket = UdpSocket::bind(addr).with_context(|| format!("binding {addr}"))?;
    socket.set_nonblocking(true)?;
    let mut options = CultNetRudpServerHubOptions::new(runtime_id, socket, CULTNET_OPERATION_CONNECTION_ID);
    options.max_fragment_bytes = Some(MAX_FRAGMENT_BYTES);
    options.max_pending_reliable_packets = Some(MAX_PENDING_RELIABLE_PACKETS);
    options.max_peers = 256;
    CultNetRudpServerHub::new(options)
}

/// The organ's two contracts, as the catalog advertises them.
pub fn schema_registry() -> Result<CultNetSchemaRegistry> {
    let mut registry = CultNetSchemaRegistry::new();
    for (schema_id, title, schema_json) in [
        (MIND_REQUEST_SCHEMA, "Huginn mind request", MIND_REQUEST_SCHEMA_JSON),
        (MIND_RESPONSE_SCHEMA, "Huginn mind response", MIND_RESPONSE_SCHEMA_JSON),
    ] {
        registry.register(CultNetSchemaRegistration {
            schema_id: schema_id.to_string(),
            kind: CultNetSchemaKind::WireMessage,
            wire_contracts: vec![CultNetWireContract::CultNetSchemaV0],
            schema_version: Some(schema_id.to_string()),
            document_type: None,
            title: Some(title.to_string()),
            schema_json: Some(schema_json.to_string()),
        })?;
    }
    Ok(registry)
}

/// One message in, one message out. Nothing here reaches a mind except through
/// `Daemon::handle`, and an envelope that does not decode never gets that far.
pub fn answer<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    registry: &CultNetSchemaRegistry,
    message: CultNetMessage,
    now: DateTime<Utc>,
) -> CultNetMessage {
    let runtime_id = daemon.runtime_id();
    match &message {
        CultNetMessage::OperationRequest { message_id, operation, .. } => match decode_request(&message) {
            Ok((message_id, request)) => {
                let operation = request.operation();
                let response = daemon.handle(request, now);
                let reply = encode_or_fail(&message_id, operation, &response, &runtime_id);
                within_window(reply, &message_id, operation, &runtime_id)
            }
            Err(failure) => encode_failure(message_id, operation, &failure, &runtime_id),
        },
        CultNetMessage::SchemaCatalogRequest { .. } => match registry.create_catalog_response(&message) {
            Ok(response) => response,
            Err(error) => CultNetMessage::Error { error: format!("{error:#}") },
        },
        _ => CultNetMessage::Error {
            error: format!(
                "{MIND_SERVICE_ID} answers cultnet.operation_request.v0 and cultnet.schema_catalog_request.v0"
            ),
        },
    }
}

/// One response in its envelope, or the failure that says it did not encode.
fn encode_or_fail(
    message_id: &str,
    operation: &str,
    response: &HuginnMindResponse,
    runtime_id: &str,
) -> CultNetMessage {
    match encode_response(message_id, operation, response, runtime_id) {
        Ok(message) => message,
        Err(error) => encode_failure(
            message_id,
            operation,
            &crate::envelope::OperationFailure {
                code: "response-not-encodable".into(),
                message: format!("{error:#}"),
            },
            runtime_id,
        ),
    }
}

/// The reply, or a typed refusal saying it will not fit. The hub cuts a reply
/// into `MAX_FRAGMENT_BYTES` packets and refuses a send needing more than
/// `MAX_PENDING_RELIABLE_PACKETS` of them; that refusal is a transport error
/// the client never sees, so it waits for an answer nothing will send. The
/// size is therefore measured here, against the window this module configured,
/// before a send that can already be seen to fail is attempted, and the client
/// is told by name how large the answer was and how large one may be.
///
/// Nothing is truncated, paginated or retried: the caller narrows its own
/// request. The refusal rides the response schema like every other refusal, so
/// it is not a second thing for a client to parse.
fn within_window(reply: CultNetMessage, message_id: &str, operation: &str, runtime_id: &str) -> CultNetMessage {
    let encoded = match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) => bytes.len() as u64,
        Err(error) => {
            return encode_failure(
                message_id,
                operation,
                &crate::envelope::OperationFailure {
                    code: "response-not-encodable".into(),
                    message: format!("{error:#}"),
                },
                runtime_id,
            );
        }
    };
    if encoded <= MAX_RESPONSE_BYTES {
        return reply;
    }
    let refusal = HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge {
        bytes: encoded,
        limit: MAX_RESPONSE_BYTES,
    });
    encode_or_fail(message_id, operation, &refusal, runtime_id)
}

/// Until `stopping`: expire what timed out, resend what was not acknowledged,
/// then answer every frame waiting on the socket. A hostile datagram and a
/// departed session are logged and served past, never fatal. This is the
/// crate's only clock read, once per frame.
pub fn run<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    hub: &mut CultNetRudpServerHub,
    registry: &CultNetSchemaRegistry,
    stopping: &AtomicBool,
    options: &ServeOptions,
) -> Result<()> {
    let timeout_ms = options.session_timeout.as_millis() as u64;
    while !stopping.load(Ordering::Relaxed) {
        hub.remove_timed_out_sessions(timeout_ms);
        if let Err(error) = hub.poll_resends() {
            eprintln!("huginn: resending to a peer failed: {error:#}");
        }
        let mut served = 0_u32;
        loop {
            let event = match hub.receive_event_once() {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("huginn: a datagram was not readable and was discarded: {error:#}");
                    break;
                }
            };
            let CultNetRudpServerEvent::Frame { session, frame } = event else {
                continue;
            };
            if frame.channel_id != "schema" {
                continue;
            }
            let message = match decode_cultnet_message_from_slice(&frame.payload, CultNetWireContract::CultNetSchemaV0)
            {
                Ok(message) => message,
                Err(error) => {
                    eprintln!("huginn: a frame was not a CultNet message and was discarded: {error:#}");
                    continue;
                }
            };
            let reply = answer(daemon, registry, message, Utc::now());
            if let Err(error) = hub.send_schema_message(&session, &reply) {
                eprintln!("huginn: {} did not receive its reply: {error:#}", session.remote_addr);
            }
            served += 1;
        }
        if served == 0 {
            std::thread::sleep(options.idle_sleep);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::tests::{
        CAMPAIGN, FITTING_CHANGES, FITTING_CUT, INSTANCE, OTHER, WIDE_CHANGES, WIDE_CUT, batch, identity, now,
        seeded_wide, slug,
    };
    use huginn_mind::epiphany_pipeline::{PipelineKind, PipelineRef};
    use crate::envelope::{FAILURE_SCHEMA, decode_response, encode_request};
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use cultnet_rs::{CultMesh, CultMeshRudpSocketOptions};
    use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse, MindStatus};
    use huginn_mind::{MindRefusal, PipelineAdmissionOutcome, PipelineQuery};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn documents(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, NoIndex>) -> u32 {
        match daemon.handle(HuginnMindRequest::Whoami, now()) {
            HuginnMindResponse::Whoami(MindStatus { documents, .. }) => documents,
            other => panic!("expected a status, got {other:?}"),
        }
    }

    fn request(
        message_id: &str,
        service_id: &str,
        operation: &str,
        payload_schema: &str,
        payload: String,
    ) -> CultNetMessage {
        CultNetMessage::OperationRequest {
            message_id: message_id.into(),
            service_id: service_id.into(),
            operation: operation.into(),
            payload_schema: payload_schema.into(),
            payload_encoding: "messagepack-base64".into(),
            payload,
            source_runtime_id: None,
            target_runtime_id: None,
        }
    }

    fn rejected(message: &CultNetMessage) -> (String, String, String) {
        let CultNetMessage::OperationResponse { message_id, status, payload_schema, payload, .. } = message else {
            panic!("expected an operation response, got {message:?}");
        };
        assert_eq!(status, "rejected");
        assert_eq!(payload_schema, FAILURE_SCHEMA);
        let failure = decode_response(message).unwrap().1.unwrap_err();
        (message_id.clone(), failure.code, payload.clone())
    }

    /// Six malformed envelopes, each with its own code, none of which reaches
    /// a mind; a message that is not an operation at all is answered rather
    /// than dropped; and the catalog answers with both schemas.
    #[test]
    fn a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind() {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let registry = schema_registry().unwrap();
        let admit = STANDARD.encode(
            rmp_serde::to_vec_named(&HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)]))).unwrap(),
        );

        let cases = [
            ("m-1", request("m-1", "odin.catalog", "admit", MIND_REQUEST_SCHEMA, admit.clone()), "wrong-service"),
            ("m-2", request("m-2", MIND_SERVICE_ID, "admit", "odin.topology.v1", admit.clone()), "wrong-payload-schema"),
            (
                "m-3",
                request("m-3", MIND_SERVICE_ID, "admit", MIND_REQUEST_SCHEMA, "not base64!!".into()),
                "payload-not-base64",
            ),
            (
                "m-4",
                request("m-4", MIND_SERVICE_ID, "admit", MIND_REQUEST_SCHEMA, STANDARD.encode([0xc1_u8, 0xc1])),
                "payload-not-a-request",
            ),
            ("m-5", request("m-5", MIND_SERVICE_ID, "query", MIND_REQUEST_SCHEMA, admit.clone()), "operation-mismatch"),
            // The same operation under another casing is another name, not this
            // one: the comparison is of bytes, so `Admit` is no more `admit`
            // than `query` is.
            ("m-5b", request("m-5b", MIND_SERVICE_ID, "Admit", MIND_REQUEST_SCHEMA, admit), "operation-mismatch"),
        ];
        for (id, message, code) in cases {
            let reply = answer(&mut daemon, &registry, message, now());
            assert_eq!(rejected(&reply).0, id, "the client's own correlation key is echoed");
            assert_eq!(rejected(&reply).1, code);
        }
        // The sixth code, `not-an-operation-request`, is the codec's and is
        // pinned in `envelope::tests`: `answer` never produces it, because a
        // message of another family is answered with `Error` instead.
        let reply = answer(&mut daemon, &registry, CultNetMessage::Error { error: "hello".into() }, now());
        let CultNetMessage::Error { error } = &reply else { panic!("expected an error, got {reply:?}") };
        assert!(
            !error.is_empty() && error.contains("cultnet.operation_request.v0") && error.contains("cultnet.schema_catalog_request.v0"),
            "{error}"
        );

        assert_eq!(documents(&mut daemon), 0, "no malformed envelope reached the mind");

        // A mind's refusal is an answer on the response schema, not a failure
        // of the envelope: the client decodes it as the typed refusal it is.
        let read = HuginnMindRequest::Query { instance: slug(OTHER), query: PipelineQuery::default() };
        let reply = answer(&mut daemon, &registry, encode_request("m-r", &read, None).unwrap(), now());
        let CultNetMessage::OperationResponse { status, payload_schema, .. } = &reply else {
            panic!("expected an operation response, got {reply:?}");
        };
        assert_eq!((status.as_str(), payload_schema.as_str()), ("rejected", MIND_RESPONSE_SCHEMA));
        let (correlation, answered) = decode_response(&reply).unwrap();
        assert_eq!(correlation, "m-r");
        assert_eq!(
            answered.unwrap(),
            HuginnMindResponse::Refused(MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() })
        );

        let catalog = CultNetMessage::SchemaCatalogRequest {
            message_id: "m-6".into(),
            include_schema_json: Some(true),
            schema_ids: None,
            kinds: None,
        };
        let reply = answer(&mut daemon, &registry, catalog, now());
        let CultNetMessage::SchemaCatalogResponse { message_id, schemas } = reply else {
            panic!("expected a catalog response");
        };
        assert_eq!(message_id, "m-6");
        let ids: Vec<&str> = schemas.iter().map(|schema| schema.schema_id.as_str()).collect();
        assert_eq!(ids, [MIND_REQUEST_SCHEMA, MIND_RESPONSE_SCHEMA]);
        for schema in &schemas {
            assert_eq!(schema.content_hash, registry.get(&schema.schema_id, true).unwrap().content_hash);
            assert!(schema.schema_json.as_deref().unwrap().contains("HuginnMind"));
        }
    }

    /// Two clients over loopback, each answered on its own session; a client on
    /// another connection id is not served at all; the stop flag ends the loop.
    #[test]
    fn two_clients_get_their_own_replies_over_loopback() {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
        let endpoint = format!("rudp://{}", hub.local_addr().unwrap());
        let registry = schema_registry().unwrap();
        let stopping = Arc::new(AtomicBool::new(false));

        let mut clients = Vec::new();
        for (name, connection_id) in
            [("a", CULTNET_OPERATION_CONNECTION_ID), ("b", CULTNET_OPERATION_CONNECTION_ID), ("c", 0x4355_4c55)]
        {
            let mut client = CultMesh::create_rudp_client_for_endpoint(
                format!("eureka-state-{name}"),
                connection_id,
                &endpoint,
                CultMeshRudpSocketOptions::default(),
            )
            .unwrap();
            client.connect(Vec::new()).unwrap();
            clients.push(client);
        }
        // The two admitted peers connect; the third's packets are dropped on
        // their connection id and it never becomes a session.
        for _ in 0..200 {
            while hub.receive_event_once().unwrap().is_some() {}
            for client in clients.iter_mut() {
                let _ = client.receive_once();
            }
            if clients[0].connected() && clients[1].connected() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(clients[0].connected() && clients[1].connected());
        assert!(!clients[2].connected(), "another connection id is not served");
        assert_eq!(hub.sessions().len(), 2);

        let requests = [
            ("m-a", HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)]))),
            ("m-b", HuginnMindRequest::Query { instance: slug(INSTANCE), query: PipelineQuery::default() }),
        ];
        for (client, (message_id, request)) in clients.iter_mut().zip(requests.iter()) {
            client.send_schema_message(&encode_request(message_id, request, None).unwrap()).unwrap();
        }
        // The third client cannot even send: the hub dropped every packet
        // carrying its connection id, so it never became a session.
        let whoami = encode_request("m-c", &HuginnMindRequest::Whoami, None).unwrap();
        assert!(clients[2].send_schema_message(&whoami).is_err(), "an unserved connection id has no session");

        let loop_stopping = Arc::clone(&stopping);
        let serving = std::thread::spawn(move || {
            let result = run(&mut daemon, &mut hub, &registry, &loop_stopping, &ServeOptions::default());
            (result, daemon)
        });

        for (index, message_id) in ["m-a", "m-b"].iter().enumerate() {
            let mut reply = None;
            for _ in 0..500 {
                clients[index].poll_resends().unwrap();
                if let Some(message) = clients[index].receive_schema_message_once().unwrap() {
                    reply = Some(message);
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let reply = reply.unwrap_or_else(|| panic!("client {index} got no reply"));
            let (correlation, answer) = decode_response(&reply).unwrap();
            assert_eq!(&correlation, message_id, "each session gets its own reply");
            match (index, answer.unwrap()) {
                (0, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })) => {}
                (1, HuginnMindResponse::Query(page)) => assert_eq!(page.matched, 1),
                (index, other) => panic!("client {index} got {other:?}"),
            }
        }
        // The third client, on its own connection id, is not answered.
        for _ in 0..50 {
            clients[2].poll_resends().unwrap();
            assert!(clients[2].receive_schema_message_once().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(2));
        }

        stopping.store(true, Ordering::Relaxed);
        let (result, mut daemon) = serving.join().unwrap();
        result.unwrap();
        assert_eq!(documents(&mut daemon), 1, "the admission that crossed the wire landed");
    }

    /// An answer larger than one send can carry is a refusal by name, not a
    /// transport failure the client waits out. The wide document is one cut spec
    /// with every list the leaf bounds filled: about a megabyte of field
    /// content, which base64 in the operation envelope carries past the window
    /// this module configures, so a single read of it is already over. The
    /// fitting one is the same document narrowed until its answer lands inside
    /// the window, and it is delivered whole.
    ///
    /// Both halves are load-bearing. Without the refused reads a daemon that
    /// never measured would pass; without the delivered one a daemon that
    /// refused everything, or configured a window smaller than the number it
    /// measures against, would pass too. The delivered answer sits within one
    /// packet-count's slack of the limit, so shrinking the configured window
    /// breaks it.
    #[test]
    fn an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client() {
        let (_root, mut daemon) = seeded_wide();
        let registry = schema_registry().unwrap();

        let sized = |daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, NoIndex>, id: PipelineRef| {
            let view = daemon.handle(HuginnMindRequest::View { instance: slug(INSTANCE), id }, now());
            let message = encode_response("m-0", "view", &view, "huginn-yggdrasil").unwrap();
            encode_cultnet_message_to_vec(&message, CultNetWireContract::CultNetSchemaV0).unwrap().len() as u64
        };
        let wide = spec_ref(&mut daemon, WIDE_CUT);
        let fitting = spec_ref(&mut daemon, FITTING_CUT);
        let (over, under) = (sized(&mut daemon, wide.clone()), sized(&mut daemon, fitting.clone()));
        eprintln!("a view of {WIDE_CHANGES} file changes encodes to {over} bytes, of {FITTING_CHANGES} to {under}");
        assert!(over > MAX_RESPONSE_BYTES, "a cut spec at the leaf's bounds is {over} bytes, over {MAX_RESPONSE_BYTES}");
        assert!(under <= MAX_RESPONSE_BYTES, "the narrowed spec is {under} bytes and must fit");
        assert!(
            MAX_RESPONSE_BYTES - under < 64 * MAX_FRAGMENT_BYTES as u64,
            "the delivered answer must sit close enough to the limit that the window's value is load-bearing"
        );

        let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
        let endpoint = format!("rudp://{}", hub.local_addr().unwrap());
        let mut client = CultMesh::create_rudp_client_for_endpoint(
            "eureka-state-wide".to_string(),
            CULTNET_OPERATION_CONNECTION_ID,
            &endpoint,
            CultMeshRudpSocketOptions::default(),
        )
        .unwrap();
        client.connect(Vec::new()).unwrap();
        for _ in 0..200 {
            while hub.receive_event_once().unwrap().is_some() {}
            let _ = client.receive_once();
            if client.connected() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(client.connected());

        let reads = [
            ("m-q", HuginnMindRequest::Query { instance: slug(INSTANCE), query: PipelineQuery::default() }),
            ("m-v", HuginnMindRequest::View { instance: slug(INSTANCE), id: wide }),
            ("m-o", HuginnMindRequest::OpenItems { instance: slug(INSTANCE), campaign: slug(CAMPAIGN) }),
        ];
        let stopping = Arc::new(AtomicBool::new(false));
        let loop_stopping = Arc::clone(&stopping);
        let serving = std::thread::spawn(move || {
            let result = run(&mut daemon, &mut hub, &registry, &loop_stopping, &ServeOptions::default());
            (result, daemon)
        });

        // One read at a time: the window is per session and counts what is
        // still unacknowledged, so two large answers in flight together are a
        // separate limit this gate does not measure.
        for (message_id, request) in reads {
            client.send_schema_message(&encode_request(message_id, &request, None).unwrap()).unwrap();
            let mut reply = None;
            for _ in 0..500 {
                client.poll_resends().unwrap();
                if let Some(message) = client.receive_schema_message_once().unwrap() {
                    reply = Some(message);
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let reply = reply.unwrap_or_else(|| panic!("{message_id} got no reply"));
            let (correlation, answered) = decode_response(&reply).unwrap();
            assert_eq!(&correlation, message_id);
            let HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge { bytes, limit }) =
                answered.expect("a refusal is an answer, not an envelope failure")
            else {
                panic!("{message_id} was not refused for its size");
            };
            assert_eq!(limit, MAX_RESPONSE_BYTES);
            assert!(bytes > limit, "{message_id} answered {bytes} bytes against a {limit} limit");
        }

        // A real-sized answer that does fit is delivered whole, over the same
        // session that was just refused twice: the gate measures the answer, and
        // the window it measures against is one the hub actually carries.
        let delivered = [
            ("m-f", HuginnMindRequest::View { instance: slug(INSTANCE), id: fitting.clone() }),
            ("m-w", HuginnMindRequest::Whoami),
        ];
        for (message_id, request) in delivered {
            client.send_schema_message(&encode_request(message_id, &request, None).unwrap()).unwrap();
            let mut reply = None;
            for _ in 0..2000 {
                client.poll_resends().unwrap();
                if let Some(message) = client.receive_schema_message_once().unwrap() {
                    reply = Some(message);
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let reply = reply.unwrap_or_else(|| panic!("{message_id} got no reply"));
            let (correlation, answered) = decode_response(&reply).unwrap();
            assert_eq!(&correlation, message_id);
            match answered.unwrap() {
                HuginnMindResponse::View(Some(view)) => assert_eq!(view.id, fitting),
                HuginnMindResponse::Whoami(status) => assert_eq!(status.documents, 5),
                other => panic!("{message_id} got {other:?}"),
            }
        }

        stopping.store(true, Ordering::Relaxed);
        serving.join().unwrap().0.unwrap();
    }

    fn encoded_len(message: &CultNetMessage) -> u64 {
        encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap().len() as u64
    }

    /// A reply carrying `payload_len` bytes of payload under a chosen
    /// `message_id`: the envelope a view answer travels in, with the document
    /// replaced by filler, so a size can be chosen instead of found. The
    /// `message_id` is part of the encoded envelope too, so pinning the gate
    /// against one length alone (every other fixture uses `m-0`) hides a gate
    /// that hard-codes that length's contribution instead of measuring it.
    fn synthetic_with_id(message_id: &str, payload_len: usize) -> CultNetMessage {
        synthetic_full(message_id, "view", "huginn-yggdrasil", payload_len)
    }

    /// N4: `operation` and `source_runtime_id` are in the encoded envelope
    /// too, exactly as `message_id` is, so a boundary fixture that only ever
    /// varies `message_id` cannot tell a gate that genuinely measures the
    /// encoded bytes from one that hard-codes a formula shaped to fit that
    /// one dimension. `synthetic_with_id` is this with the operation and
    /// runtime id `the_gates_boundary_*` tests were pinned to before N4.
    fn synthetic_full(message_id: &str, operation: &str, runtime_id: &str, payload_len: usize) -> CultNetMessage {
        CultNetMessage::OperationResponse {
            message_id: message_id.into(),
            service_id: MIND_SERVICE_ID.into(),
            operation: operation.into(),
            status: "accepted".into(),
            payload_schema: MIND_RESPONSE_SCHEMA.into(),
            payload_encoding: "messagepack-base64".into(),
            payload: "A".repeat(payload_len),
            diagnostics: vec![],
            source_runtime_id: Some(runtime_id.into()),
        }
    }

    fn synthetic(payload_len: usize) -> CultNetMessage {
        synthetic_with_id("m-0", payload_len)
    }

    /// One whose encoded envelope is exactly `target` bytes, under a chosen
    /// `message_id`. The envelope's own overhead is measured rather than
    /// assumed: the length prefix is the same width on either side of a
    /// megabyte, so one probe fixes it.
    fn sized_exactly_with_id(message_id: &str, target: u64) -> CultNetMessage {
        sized_exactly_full(message_id, "view", "huginn-yggdrasil", target)
    }

    /// N4's own probe: `sized_exactly_with_id` generalised over the
    /// operation and the runtime id as well as the message id, so a fixture
    /// can pin the gate's boundary at a combination a formula shaped to fit
    /// `view`/`huginn-yggdrasil` alone was never measured against.
    fn sized_exactly_full(message_id: &str, operation: &str, runtime_id: &str, target: u64) -> CultNetMessage {
        let probe = 1_200_000_usize;
        let at_probe = encoded_len(&synthetic_full(message_id, operation, runtime_id, probe));
        let message =
            synthetic_full(message_id, operation, runtime_id, (probe as i64 + (target as i64 - at_probe as i64)) as usize);
        assert_eq!(encoded_len(&message), target);
        message
    }

    fn sized_exactly(target: u64) -> CultNetMessage {
        sized_exactly_with_id("m-0", target)
    }

    /// A connected session whose accept has been acknowledged, so its window
    /// holds `MAX_PENDING_RELIABLE_PACKETS` and not one less. See the module
    /// documentation: a test that measures the window exactly and does not
    /// settle first measures 1023.
    fn settled_session(
        hub: &mut CultNetRudpServerHub,
    ) -> (cultnet_rs::CultNetRudpSocketTransportConnection, cultnet_rs::CultNetRudpServerSessionContext) {
        let endpoint = format!("rudp://{}", hub.local_addr().unwrap());
        let mut client = CultMesh::create_rudp_client_for_endpoint(
            "eureka-state-boundary".to_string(),
            CULTNET_OPERATION_CONNECTION_ID,
            &endpoint,
            CultMeshRudpSocketOptions::default(),
        )
        .unwrap();
        client.connect(Vec::new()).unwrap();
        let mut session = None;
        for _ in 0..500 {
            while let Some(event) = hub.receive_event_once().unwrap() {
                if let CultNetRudpServerEvent::Connected { session: connected } = event {
                    session = Some(connected);
                }
            }
            let _ = client.receive_once();
            if client.connected() && session.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(client.connected());
        for _ in 0..100 {
            hub.poll_resends().unwrap();
            while hub.receive_event_once().unwrap().is_some() {}
            client.poll_resends().unwrap();
            while client.receive_once().unwrap().is_some() {}
            std::thread::sleep(Duration::from_millis(2));
        }
        (client, session.unwrap())
    }

    /// The gate's boundary is the transport's, byte for byte. The test above
    /// proves the two agree at the sizes real documents reach, which leaves
    /// the width of a whole document between the largest answer it delivers
    /// and the smallest it refuses; this one leaves nothing. A reply whose
    /// encoded envelope is exactly `MAX_RESPONSE_BYTES` passes the gate
    /// unchanged and is accepted by a hub `bind` configured; one byte more is
    /// refused by both.
    ///
    /// So the gate measures the encoded envelope, which is what the hub
    /// fragments, and not the payload string inside it, which is 231 bytes
    /// smaller; and it admits the limit rather than stopping one short of it.
    /// Neither is visible to a test whose answers sit tens of thousands of
    /// bytes from the boundary.
    ///
    /// The refusal is measured on the same scale: it is one packet, so the
    /// answer that says an answer did not fit always fits itself. And the
    /// accepted reply is shown to have filled the window it was measured
    /// against, because a one-byte reply after it is refused.
    #[test]
    fn the_gates_boundary_is_the_transports_boundary_byte_for_byte() {
        let at = sized_exactly(MAX_RESPONSE_BYTES);
        let over = sized_exactly(MAX_RESPONSE_BYTES + 1);
        assert_eq!(within_window(at.clone(), "m-0", "view", "huginn-yggdrasil"), at, "exactly the limit passes");

        let refused = within_window(over.clone(), "m-0", "view", "huginn-yggdrasil");
        let (correlation, answered) = decode_response(&refused).unwrap();
        assert_eq!(correlation, "m-0");
        let HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge { bytes, limit }) =
            answered.expect("a refusal is an answer, not an envelope failure")
        else {
            panic!("one byte over the limit was not refused: {refused:?}");
        };
        assert_eq!((bytes, limit), (MAX_RESPONSE_BYTES + 1, MAX_RESPONSE_BYTES));
        let refusal_bytes = encoded_len(&refused);
        assert!(
            refusal_bytes <= MAX_FRAGMENT_BYTES as u64,
            "the refusal is {refusal_bytes} bytes and must fit one packet"
        );

        let mut hub = bind("127.0.0.1:0".parse().unwrap(), "huginn-yggdrasil").unwrap();
        let (_client, session) = settled_session(&mut hub);
        let error = hub.send_schema_message(&session, &over).unwrap_err();
        assert!(format!("{error:#}").contains("queue is full"), "limit + 1 was accepted: {error:#}");
        hub.send_schema_message(&session, &at).expect("exactly the limit is accepted on an empty window");
        hub.send_schema_message(&session, &synthetic(1))
            .expect_err("the accepted answer filled the window, so nothing follows it");
    }

    /// The boundary above is pinned for one envelope shape: every reply in it
    /// carries `message_id` `m-0`, three bytes, so a gate that measures
    /// `payload.len() + 231` — exactly the encoded envelope's overhead for
    /// that one id — passes it unnoticed. The reply echoes the client's own
    /// `message_id`, so that overhead is under the client's control: at a
    /// longer id, such a gate is wrong, and wrong on the side that lets an
    /// oversize answer through the size check to a send that then fails
    /// silently. Forty bytes is used because it is far from `m-0`'s length in
    /// either direction, not chosen to sit near some other boundary.
    #[test]
    fn the_gates_boundary_holds_for_a_second_message_id_length() {
        let long_id = format!("m-{}", "0".repeat(38));
        assert_eq!(long_id.len(), 40, "a message_id far from m-0's own length");

        let at = sized_exactly_with_id(&long_id, MAX_RESPONSE_BYTES);
        let over = sized_exactly_with_id(&long_id, MAX_RESPONSE_BYTES + 1);
        assert_eq!(
            within_window(at.clone(), &long_id, "view", "huginn-yggdrasil"),
            at,
            "exactly the limit passes at this id's length too"
        );

        let refused = within_window(over.clone(), &long_id, "view", "huginn-yggdrasil");
        let (correlation, answered) = decode_response(&refused).unwrap();
        assert_eq!(correlation, long_id);
        let HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge { bytes, limit }) =
            answered.expect("a refusal is an answer, not an envelope failure")
        else {
            panic!("one byte over the limit was not refused: {refused:?}");
        };
        assert_eq!((bytes, limit), (MAX_RESPONSE_BYTES + 1, MAX_RESPONSE_BYTES));
    }

    /// N4: the boundary above is pinned at one `message_id` length, but
    /// `operation` and `source_runtime_id` are in the encoded envelope the
    /// same way `message_id` is, and every fixture before this one held both
    /// fixed at `"view"` and `"huginn-yggdrasil"`. A gate whose measurement is
    /// a formula of `message_id`'s length alone -- shaped to pass at exactly
    /// `m-0`'s three bytes and the other fixture's forty -- has no way to be
    /// wrong there and every way to be wrong once the operation or the
    /// runtime id it never looked at changes length too. This holds the
    /// boundary at `open_items` (ten bytes, not `view`'s four) over a second
    /// runtime id, so such a formula is measured somewhere it was never
    /// fitted.
    #[test]
    fn the_gates_boundary_holds_across_operation_and_runtime_id() {
        for (operation, runtime_id) in [("view", "huginn-yggdrasil"), ("open_items", "huginn-thought-cage")] {
            let at = sized_exactly_full("m-g", operation, runtime_id, MAX_RESPONSE_BYTES);
            let over = sized_exactly_full("m-g", operation, runtime_id, MAX_RESPONSE_BYTES + 1);
            assert_eq!(
                within_window(at.clone(), "m-g", operation, runtime_id),
                at,
                "{operation}/{runtime_id}: exactly the limit passes"
            );

            let refused = within_window(over.clone(), "m-g", operation, runtime_id);
            let (correlation, answered) = decode_response(&refused).unwrap();
            assert_eq!(correlation, "m-g");
            let HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge { bytes, limit }) =
                answered.expect("a refusal is an answer, not an envelope failure")
            else {
                panic!("{operation}/{runtime_id}: one byte over the limit was not refused: {refused:?}");
            };
            assert_eq!((bytes, limit), (MAX_RESPONSE_BYTES + 1, MAX_RESPONSE_BYTES), "{operation}/{runtime_id}");
        }
    }

    /// One cut spec's own reference, by its cut label.
    fn spec_ref(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, NoIndex>, cut: &str) -> PipelineRef {
        let query = PipelineQuery {
            kinds: vec![PipelineKind::CutSpec],
            cut: Some(cut.into()),
            ..PipelineQuery::default()
        };
        let HuginnMindResponse::Query(page) =
            daemon.handle(HuginnMindRequest::Query { instance: slug(INSTANCE), query }, now())
        else {
            panic!("expected a page");
        };
        assert_eq!(page.matched, 1, "one cut spec is labelled {cut}");
        page.items[0].id.clone()
    }

    /// Ruling 15 as an order in the one place both happen. Both gates are shut
    /// at once: the mind is held and the port is held. A startup that opens
    /// first says why the mind refused; one that binds first can only say the
    /// port was taken, and never reaches the refusal that matters.
    #[test]
    fn startup_opens_the_mind_before_it_binds_anything() {
        let root = tempfile::tempdir().unwrap();
        let held = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let occupied = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = occupied.local_addr().unwrap();

        let options = Options { state_root: root.path().to_path_buf(), instance: slug(INSTANCE), bind: port };
        let error = format!("{:#}", startup(&options).err().expect("a held mind refuses"));
        assert!(error.contains("MindAlreadyOwned"), "the refusal is the mind's, not the socket's: {error}");
        assert!(!error.contains("binding"), "the socket was never reached: {error}");

        // With the mind free and the port still held, the socket is the only
        // gate left, and startup reports it.
        drop(held);
        let error = format!("{:#}", startup(&options).err().expect("a held port refuses"));
        assert!(error.contains("binding"), "{error}");
        drop(occupied);
        let (_daemon, hub, _registry) = startup(&options).expect("both gates open");
        assert_eq!(hub.local_addr().unwrap(), port);
    }

    #[test]
    fn options_are_exactly_state_root_instance_and_bind() {
        let root = if cfg!(windows) { r"C:\state" } else { "/state" };
        let args = |values: &[&str]| values.iter().map(|value| value.to_string()).collect::<Vec<_>>().into_iter();
        assert_eq!(
            parse_options(args(&["--state-root", root, "--instance", "yggdrasil", "--bind", "127.0.0.1:17872"]))
                .unwrap(),
            Options {
                state_root: PathBuf::from(root),
                instance: slug("yggdrasil"),
                bind: "127.0.0.1:17872".parse().unwrap(),
            }
        );
        for bad in [
            vec!["--state-root", root, "--instance", "yggdrasil"],
            vec!["--state-root", root, "--bind", "127.0.0.1:1"],
            vec!["--instance", "yggdrasil", "--bind", "127.0.0.1:1"],
            vec!["--state-root", root, "--instance", "yggdrasil", "--bind", "127.0.0.1:1", "--bind", "127.0.0.1:2"],
            vec!["--state-root", root, "--instance", "yggdrasil", "--bind", "not-an-address"],
            vec!["--state-root", "relative", "--instance", "yggdrasil", "--bind", "127.0.0.1:1"],
            vec!["--state-root", root, "--instance", "yggdrasil", "--bind", "127.0.0.1:1", "--idunn-anchor", "x"],
        ] {
            assert!(parse_options(args(&bad)).is_err(), "{bad:?}");
        }
    }
}
