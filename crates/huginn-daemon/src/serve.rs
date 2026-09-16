//! The loop: one UDP socket, one reliable session per peer, one reply per
//! frame on the frame's own session. The hub is CultLib's; what is here is the
//! routing, the schema catalog and the order the process starts in.
//!
//! Stated limits. A socket error and a hostile datagram are indistinguishable
//! at `receive_event_once`, so both are logged and served past: a dead socket
//! spins with logging rather than exiting, and Cut 14's health check is the
//! observer. The reliable window is `max_pending_reliable_packets` times
//! `max_fragment_bytes` per session, about 1.2 MB in flight; a page larger
//! than that fails to send and the client times out rather than receiving a
//! typed answer.

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
    CultNetWireContract, decode_cultnet_message_from_slice,
};
use huginn_mind::epiphany_pipeline::Slug;
use huginn_mind::wire::{
    MIND_REQUEST_SCHEMA, MIND_REQUEST_SCHEMA_JSON, MIND_RESPONSE_SCHEMA, MIND_RESPONSE_SCHEMA_JSON, MIND_SERVICE_ID,
};
use huginn_mind::{MindStore, OwnedRedbMessagePackBackingStore};

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

/// One non-blocking socket serving the operation connection id. The hub drops
/// every datagram carrying another one.
pub fn bind(addr: SocketAddr, runtime_id: &str) -> Result<CultNetRudpServerHub> {
    let socket = UdpSocket::bind(addr).with_context(|| format!("binding {addr}"))?;
    socket.set_nonblocking(true)?;
    let mut options = CultNetRudpServerHubOptions::new(runtime_id, socket, CULTNET_OPERATION_CONNECTION_ID);
    options.max_fragment_bytes = Some(1200);
    options.max_pending_reliable_packets = Some(1024);
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
                match encode_response(&message_id, operation, &response, &runtime_id) {
                    Ok(message) => message,
                    Err(error) => encode_failure(
                        &message_id,
                        operation,
                        &crate::envelope::OperationFailure {
                            code: "response-not-encodable".into(),
                            message: format!("{error:#}"),
                        },
                        &runtime_id,
                    ),
                }
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
    use crate::daemon::tests::{INSTANCE, OTHER, batch, identity, now, slug};
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
