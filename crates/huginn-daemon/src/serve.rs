//! The loop: one UDP socket, one reliable session per peer, one reply per
//! frame on the frame's own session. The hub is CultLib's; what is here is the
//! routing, the schema catalog and the order the process starts in.
//!
//! Stated limits. A socket error and a hostile datagram are indistinguishable
//! at `receive_event_once`, so both are logged and served past: a dead socket
//! spins with logging rather than exiting, and Cut 14's health check is the
//! observer.
//!
//! One send carries `MAX_RESPONSE_BYTES` encoded, 1,228,800 today: the
//! reliable window one session holds, `MAX_PENDING_RELIABLE_PACKETS` packets
//! of `MAX_FRAGMENT_BYTES`. `deliver` decides every answer's plane: one that
//! fits goes whole; a larger one whose encoded payload is at most
//! `MAX_DEFERRED_BODY_BYTES` is held by `DeferredBodies` and answered with
//! `HuginnMindResponse::Deferred`, a `CultMeshCdnArtifactManifest`, and the
//! client fetches the payload with `cultnet_rs::fetch_content`, one
//! `cultmesh.content_chunk_request.v1` at a time on the session it already
//! holds, exactly as the C# reference's `CultMeshLegacyRudpContentServer` is
//! asked. The fetched bytes are the named MessagePack of the
//! `HuginnMindResponse` the mind gave. A body the daemon will not defer is
//! refused by name, `MindRefusal::ResponseTooLarge`, carrying the payload size
//! and `MAX_DEFERRED_BODY_BYTES`, and the caller narrows its own request. A
//! deferred body is memory only: it expires untouched after
//! `ServeOptions::deferred_ttl`, is evicted past `DEFERRED_BUDGET_BYTES`, and
//! is gone when the process exits, so a chunk request that finds it gone is
//! answered `found: false` and the client asks the operation again.
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
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Utc};
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultNetMessage, CultNetRudpServerEvent, CultNetRudpServerHub,
    CultNetRudpServerHubOptions, CultNetRudpServerSessionContext, CultNetSchemaKind, CultNetSchemaRegistration, CultNetSchemaRegistry,
    CultNetWireContract, answer_content_chunk_request, decode_cultnet_message_from_slice,
    encode_cultnet_message_to_vec, pack_content,
};
use huginn_mind::epiphany_pipeline::Slug;
use huginn_mind::wire::{
    DeferredAnswer, HuginnMindResponse, MIND_REQUEST_SCHEMA, MIND_REQUEST_SCHEMA_JSON, MIND_RESPONSE_SCHEMA,
    MIND_RESPONSE_SCHEMA_JSON, MIND_SERVICE_ID,
};
use huginn_mind::{Mind, MindRefusal, MindStore, OwnedRedbMessagePackBackingStore};

use crate::bodies::{
    DEFERRED_BUDGET_BYTES, DEFERRED_CHUNK_BYTES, DeferredBodies, MAX_DEFERRED_BODY_BYTES,
};
use crate::daemon::{Daemon, Handled, IndexSink, Search, SearchTicket, runtime_id};
use crate::envelope::{OperationFailure, decode_request, encode_failure, encode_response};
use crate::index::{Backoff, Embedder, VectorIndex, WorkerSink};

/// What the loop does when nothing is waiting. A deferred body untouched for
/// `deferred_ttl`, twice the session timeout, is dropped.
pub struct ServeOptions {
    pub session_timeout: Duration,
    pub idle_sleep: Duration,
    pub deferred_ttl: Duration,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            session_timeout: Duration::from_secs(30),
            idle_sleep: Duration::from_millis(2),
            deferred_ttl: Duration::from_secs(60),
        }
    }
}

/// The six things the operator supplies. Nothing here reads the environment:
/// how Idunn supplies `--bind` is Cut 14's. The index has no off switch: an
/// unreachable Qdrant or Ollama is a degraded projection, reported by
/// `whoami`, not a configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub state_root: PathBuf,
    pub instance: Slug,
    pub bind: SocketAddr,
    pub qdrant_url: String,
    pub ollama_url: String,
    pub embedding_model: String,
}

/// Exactly `--state-root`, `--instance`, `--bind`, `--qdrant-url`,
/// `--ollama-url` and `--embedding-model`, each required, each once. The two
/// URLs are plain `http://`: this build carries no TLS.
pub fn parse_options(args: impl Iterator<Item = String>) -> Result<Options> {
    let mut args = args;
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    while let Some(name) = args.next() {
        let name = name.strip_prefix("--").with_context(|| format!("expected --option, got {name:?}"))?;
        ensure!(
            matches!(name, "state-root" | "instance" | "bind" | "qdrant-url" | "ollama-url" | "embedding-model"),
            "unsupported Huginn option --{name}"
        );
        let value = args.next().with_context(|| format!("missing value for --{name}"))?;
        ensure!(values.insert(name.to_owned(), value).is_none(), "duplicate Huginn option --{name}");
    }
    let take = |name: &str| -> Result<String> {
        values.get(name).cloned().with_context(|| format!("--{name} is required"))
    };
    let state_root = PathBuf::from(take("state-root")?);
    ensure!(state_root.is_absolute(), "--state-root must be an absolute path");
    let bind = take("bind")?;
    let url = |name: &str| -> Result<String> {
        let value = take(name)?;
        ensure!(value.starts_with("http://"), "--{name} must be a plain http:// address, got {value:?}");
        Ok(value)
    };
    let embedding_model = take("embedding-model")?;
    ensure!(!embedding_model.is_empty(), "--embedding-model must name a model");
    Ok(Options {
        state_root,
        instance: Slug(take("instance")?),
        bind: bind.parse().with_context(|| format!("--bind must be an ip:port, got {bind:?}"))?,
        qdrant_url: url("qdrant-url")?,
        ollama_url: url("ollama-url")?,
        embedding_model,
    })
}

/// Ruling 15, as an order: the mind opens first, and only a mind that opened
/// gets a socket, and only a daemon that is going to serve gets an index
/// worker. This is the one place all three happen, so nothing can bind ahead
/// of the refusal and nothing reaches the vector store for a process that
/// never listens. The worker's first act is to reconcile the collection
/// against every indexable document the mind holds now.
pub fn startup<E, V>(
    options: &Options,
    embedder: E,
    index: V,
    backoff: Backoff,
) -> Result<(Daemon<OwnedRedbMessagePackBackingStore, WorkerSink>, CultNetRudpServerHub, CultNetSchemaRegistry)>
where
    E: Embedder + Send + 'static,
    V: VectorIndex + Send + 'static,
{
    let mind = Mind::open(&options.state_root, &options.instance)?;
    let hub = bind(options.bind, &runtime_id(&options.instance))?;
    let startup_entries = mind.index_entries(None)?;
    let sink = WorkerSink::spawn(embedder, index, &options.instance, mind.genesis_receipt_id()?, startup_entries, backoff);
    Ok((Daemon::new(mind, sink), hub, schema_registry()?))
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

/// The longest client-supplied string a reply may echo, in bytes. A reply
/// echoes the request's `message_id` (and, on a failure, its `service_id`,
/// `operation` and `payload_schema`; on a chunk answer, its `chunk_hash`), so an
/// unbounded field would make the reply unbounded, and the hub would refuse a
/// send the client then waits on forever. Every echoed field is refused past
/// this at the door, with a transport `Error` that carries none of them. With
/// it, every reply is within the window: the largest, a deferred answer or a
/// refusal, is one 256-byte id, a real operation name, the daemon's own runtime
/// id and, for a deferred answer, a manifest of at most
/// `MAX_DEFERRED_BODY_BYTES / DEFERRED_CHUNK_BYTES` chunk references. Real ids
/// are UUID-sized.
pub const MAX_ECHOED_FIELD_BYTES: usize = 256;

/// The first field of a request that a reply would echo and that is over
/// `MAX_ECHOED_FIELD_BYTES`.
fn overlong_echo(message: &CultNetMessage) -> Option<&'static str> {
    let echoed: Vec<(&'static str, &str)> = match message {
        CultNetMessage::OperationRequest { message_id, service_id, operation, payload_schema, .. } => {
            vec![
                ("message_id", message_id),
                ("service_id", service_id),
                ("operation", operation),
                ("payload_schema", payload_schema),
            ]
        }
        CultNetMessage::SchemaCatalogRequest { message_id, .. } => vec![("message_id", message_id)],
        CultNetMessage::ContentChunkRequest { message_id, chunk_hash, record_key, .. } => {
            vec![("message_id", message_id), ("chunk_hash", chunk_hash), ("record_key", record_key)]
        }
        _ => return None,
    };
    echoed.into_iter().find(|(_, value)| value.len() > MAX_ECHOED_FIELD_BYTES).map(|(name, _)| name)
}

/// What one message came to: a message to send back now, or a semantic query
/// the index has yet to answer, which the loop holds against the session it
/// came in on.
pub enum Routed {
    Reply(CultNetMessage),
    Search { message_id: String, operation: &'static str, search: Search },
}

impl Routed {
    /// The reply, for a message that cannot be a search.
    #[cfg(test)]
    pub(crate) fn reply(self) -> CultNetMessage {
        match self {
            Self::Reply(reply) => reply,
            Self::Search { .. } => panic!("expected a reply, got a search"),
        }
    }
}

/// One message in, one message out, or a search to wait for. Nothing here
/// reaches a mind except through `Daemon::handle`, and an envelope that does
/// not decode never gets that far. A chunk request is answered from `bodies`
/// alone, on the session it came in on; every operation's reply leaves through
/// `respond`.
pub fn answer<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    registry: &CultNetSchemaRegistry,
    bodies: &mut DeferredBodies,
    message: CultNetMessage,
    now: DateTime<Utc>,
) -> Routed {
    let runtime_id = daemon.runtime_id();
    bodies.expire(now);
    if let Some(field) = overlong_echo(&message) {
        return Routed::Reply(CultNetMessage::Error {
            error: format!("{MIND_SERVICE_ID} refuses a {field} over {MAX_ECHOED_FIELD_BYTES} bytes"),
            code: None,
            details: None,
        });
    }
    Routed::Reply(match &message {
        CultNetMessage::OperationRequest { message_id, operation, .. } => match decode_request(&message) {
            Ok((message_id, request)) => {
                let operation = request.operation();
                match daemon.handle(request, now) {
                    Handled::Answered(response) => respond(&response, &message_id, operation, &runtime_id, bodies, now),
                    Handled::Searching(search) => return Routed::Search { message_id, operation, search },
                }
            }
            Err(failure) => encode_failure(message_id, operation, &failure, &runtime_id),
        },
        CultNetMessage::SchemaCatalogRequest { .. } => match registry.create_catalog_response(&message) {
            Ok(response) => response,
            Err(error) => CultNetMessage::Error { error: format!("{error:#}"), code: None, details: None },
        },
        CultNetMessage::ContentChunkRequest { .. } => {
            answer_content_chunk_request(&message, |hash| bodies.chunk(hash, now))
        }
        _ => CultNetMessage::Error {
            error: format!(
                "{MIND_SERVICE_ID} answers cultnet.operation_request.v0, cultnet.schema_catalog_request.v0 \
                 and cultmesh.content_chunk_request.v1"
            ),
            code: None,
            details: None,
        },
    })
}

/// One answer as the message that carries it: encoded, then delivered whole,
/// deferred or refused by `deliver`.
fn respond(
    response: &HuginnMindResponse,
    message_id: &str,
    operation: &str,
    runtime_id: &str,
    bodies: &mut DeferredBodies,
    now: DateTime<Utc>,
) -> CultNetMessage {
    let reply = encode_or_fail(message_id, operation, response, runtime_id);
    deliver(reply, message_id, operation, runtime_id, bodies, now)
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

/// How one answer leaves: whole, deferred or refused, in that order, and
/// nothing else decides. The hub cuts a reply into `MAX_FRAGMENT_BYTES` packets
/// and refuses a send needing more than `MAX_PENDING_RELIABLE_PACKETS` of them;
/// that refusal is a transport error the client never sees, so it waits for an
/// answer nothing will send. The envelope is therefore measured here, against
/// the window this module configured, before a send that can already be seen to
/// fail is attempted.
///
/// A reply that fits is returned as it is. A larger one whose payload, the
/// named MessagePack of the response, is at most `MAX_DEFERRED_BODY_BYTES` is
/// packed by CultNet's content plane, retained in `bodies`, and answered with
/// `Deferred` and the manifest. That answer is bounded, not fixed-shape: the id
/// it echoes is at most `MAX_ECHOED_FIELD_BYTES` and its manifest at most 256
/// references, so it always fits one send and is never itself measured or
/// deferred. Its envelope status is the deferred answer's own, read from the
/// reply before deferral, so routing on status is the same whole or deferred. A
/// larger payload is refused by name
/// with its own size and the deferral bound: nothing is truncated or paginated,
/// and the caller narrows its own request. Both answers ride the response
/// schema like every other, so a client parses no second vocabulary.
fn deliver(
    reply: CultNetMessage,
    message_id: &str,
    operation: &str,
    runtime_id: &str,
    bodies: &mut DeferredBodies,
    now: DateTime<Utc>,
) -> CultNetMessage {
    let not_encodable = |message: String| {
        encode_failure(
            message_id,
            operation,
            &OperationFailure { code: "response-not-encodable".into(), message },
            runtime_id,
        )
    };
    let encoded = match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) => bytes.len() as u64,
        Err(error) => return not_encodable(format!("{error:#}")),
    };
    if encoded <= MAX_RESPONSE_BYTES {
        return reply;
    }
    let (payload, answer_status) = match &reply {
        CultNetMessage::OperationResponse { payload, status, .. } => (STANDARD.decode(payload), status.clone()),
        _ => return not_encodable("only an operation response is deferred".into()),
    };
    let payload = match payload {
        Ok(bytes) => bytes,
        Err(error) => return not_encodable(format!("the payload is not base64: {error}")),
    };
    let size = payload.len() as u64;
    if size > MAX_DEFERRED_BODY_BYTES {
        let refusal = HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge {
            bytes: size,
            limit: MAX_DEFERRED_BODY_BYTES,
        });
        return encode_or_fail(message_id, operation, &refusal, runtime_id);
    }
    let (manifest, chunks) = pack_content(
        "huginn.mind_response",
        "package",
        "",
        "application/vnd.gamecult.huginn.mind-response+msgpack",
        &now.to_rfc3339(),
        &payload,
        DEFERRED_CHUNK_BYTES,
    )
    .expect("the deferral constants are a valid pack");
    bodies.retain(&manifest, chunks, now);
    let mut deferred =
        encode_or_fail(message_id, operation, &HuginnMindResponse::Deferred(DeferredAnswer { manifest }), runtime_id);
    if let CultNetMessage::OperationResponse { status, .. } = &mut deferred {
        *status = answer_status;
    }
    deferred
}

/// A semantic query on its way through the index: who asked, what to call the
/// answer, the search to finish, and when the client stops being promised one.
struct Waiting {
    session: CultNetRudpServerSessionContext,
    message_id: String,
    operation: &'static str,
    search: Search,
    until: Instant,
}

/// Until `stopping`: expire what timed out, resend what was not acknowledged,
/// answer every frame waiting on the socket, then answer every semantic query
/// the index has finished or that has waited out the index's own
/// `search_deadline`. A semantic
/// query is never answered inline: `answer` hands back a `Search`, the loop
/// holds it beside its session and goes on serving, and the embedder and the
/// vector store are the worker's to wait on. A hostile datagram and a departed
/// session are logged and served past, never fatal. The clock is read once per
/// frame, and once per pass for the searches' deadlines.
pub fn run<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    hub: &mut CultNetRudpServerHub,
    registry: &CultNetSchemaRegistry,
    stopping: &AtomicBool,
    options: &ServeOptions,
) -> Result<()> {
    let timeout_ms = options.session_timeout.as_millis() as u64;
    let mut bodies = DeferredBodies::new(DEFERRED_BUDGET_BYTES, options.deferred_ttl);
    let mut waiting: BTreeMap<SearchTicket, Waiting> = BTreeMap::new();
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
            match answer(daemon, registry, &mut bodies, message, Utc::now()) {
                Routed::Reply(reply) => send(hub, &session, &reply),
                Routed::Search { message_id, operation, search } => {
                    let until = Instant::now() + daemon.search_deadline();
                    waiting.insert(search.ticket, Waiting { session, message_id, operation, search, until });
                }
            }
            served += 1;
        }
        let finished = collect_searches(daemon, &waiting);
        let answered = !finished.is_empty();
        for (ticket, found) in finished {
            let Some(asked) = waiting.remove(&ticket) else { continue };
            let response = daemon.finish(asked.search, found);
            let reply = respond(&response, &asked.message_id, asked.operation, &daemon.runtime_id(), &mut bodies, Utc::now());
            send(hub, &asked.session, &reply);
        }
        if served == 0 && !answered {
            std::thread::sleep(options.idle_sleep);
        }
    }
    Ok(())
}

/// What the index has finished, then what has waited too long: the latter as
/// an `Unavailable` detail, and abandoned, so the index does not start a search
/// nobody is waiting for. A search that finishes after its deadline finds
/// nothing waiting for it and is dropped.
fn collect_searches<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    waiting: &BTreeMap<SearchTicket, Waiting>,
) -> Vec<(SearchTicket, Result<crate::daemon::Hits, String>)> {
    let deadline = daemon.search_deadline();
    let mut collected = daemon.searched();
    let now = Instant::now();
    for (ticket, asked) in waiting {
        if asked.until <= now {
            daemon.abandon(*ticket);
            collected.push((*ticket, Err(format!("the semantic search did not finish within {deadline:?}"))));
        }
    }
    collected
}

fn send(hub: &mut CultNetRudpServerHub, session: &CultNetRudpServerSessionContext, reply: &CultNetMessage) {
    if let Err(error) = hub.send_schema_message(session, reply) {
        eprintln!("huginn: {} did not receive its reply: {error:#}", session.remote_addr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::tests::{
        FITTING_CHANGES, FITTING_CUT, INSTANCE, NoIndex, OTHER, WIDE_CHANGES, WIDE_CUT, batch, identity, now,
        open_unindexed, seeded_wide, slug,
    };
    use huginn_mind::epiphany_pipeline::{PipelineKind, PipelineRef};
    use crate::envelope::{FAILURE_SCHEMA, decode_response, encode_request};
    use crate::index::Backoff;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use cultnet_rs::{CultMesh, CultMeshCdnArtifactManifest, CultMeshRudpSocketOptions, fetch_content};
    use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse, MindStatus};
    use cultnet_rs::{FieldPredicate, Selection};
    use huginn_mind::{MindRefusal, PipelineAdmissionOutcome, PipelinePageItems};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn documents(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, NoIndex>) -> u32 {
        match daemon.handle(HuginnMindRequest::Whoami, now()).answered() {
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
        let mut daemon = open_unindexed(root.path(), &slug(INSTANCE)).unwrap();
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
            let reply = answer(&mut daemon, &registry, &mut fresh_bodies(), message, now()).reply();
            assert_eq!(rejected(&reply).0, id, "the client's own correlation key is echoed");
            assert_eq!(rejected(&reply).1, code);
        }
        // The sixth code, `not-an-operation-request`, is the codec's and is
        // pinned in `envelope::tests`: `answer` never produces it, because a
        // message of another family is answered with `Error` instead.
        let reply = answer(&mut daemon, &registry, &mut fresh_bodies(), CultNetMessage::Error { error: "hello".into(), code: None, details: None }, now()).reply();
        let CultNetMessage::Error { error, .. } = &reply else { panic!("expected an error, got {reply:?}") };
        assert!(
            !error.is_empty() && error.contains("cultnet.operation_request.v0") && error.contains("cultnet.schema_catalog_request.v0"),
            "{error}"
        );

        assert_eq!(documents(&mut daemon), 0, "no malformed envelope reached the mind");

        // A mind's refusal is an answer on the response schema, not a failure
        // of the envelope: the client decodes it as the typed refusal it is.
        let read = HuginnMindRequest::Query { instance: slug(OTHER), selection: Selection::default(), semantic: None };
        let reply = answer(&mut daemon, &registry, &mut fresh_bodies(), encode_request("m-r", &read, None).unwrap(), now()).reply();
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
        let reply = answer(&mut daemon, &registry, &mut fresh_bodies(), catalog, now()).reply();
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
        let mut daemon = open_unindexed(root.path(), &slug(INSTANCE)).unwrap();
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
            ("m-b", HuginnMindRequest::Query { instance: slug(INSTANCE), selection: Selection::default(), semantic: None }),
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

    fn encoded_len(message: &CultNetMessage) -> u64 {
        encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap().len() as u64
    }

    const TTL: Duration = Duration::from_secs(60);

    fn fresh_bodies() -> DeferredBodies {
        DeferredBodies::new(DEFERRED_BUDGET_BYTES, TTL)
    }

    type Client = cultnet_rs::CultNetRudpSocketTransportConnection;
    type BodyDaemon = Daemon<OwnedRedbMessagePackBackingStore, NoIndex>;

    /// One request on the client's own session and its reply, the way
    /// `fetch_content` is asked for each chunk.
    fn exchange(client: &mut Client, message: &CultNetMessage) -> Result<CultNetMessage> {
        client.send_schema_message(message)?;
        for _ in 0..3000 {
            client.poll_resends()?;
            if let Some(reply) = client.receive_schema_message_once()? {
                return Ok(reply);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        anyhow::bail!("no reply")
    }

    fn deferred_manifest(reply: &CultNetMessage) -> CultMeshCdnArtifactManifest {
        match decode_response(reply).unwrap().1.expect("an answer, not an envelope failure") {
            HuginnMindResponse::Deferred(DeferredAnswer { manifest }) => manifest,
            other => panic!("expected a deferred answer, got {other:?}"),
        }
    }

    fn chunk_request(manifest: &CultMeshCdnArtifactManifest, index: usize) -> CultNetMessage {
        let chunk = &manifest.chunks[index];
        CultNetMessage::ContentChunkRequest {
            message_id: format!("c-{index}"),
            chunk_hash: chunk.chunk_hash.clone(),
            record_key: chunk.record_key.clone(),
            expected_size_bytes: chunk.size_bytes,
        }
    }

    fn found(reply: &CultNetMessage) -> bool {
        let CultNetMessage::ContentChunkResponse { found, .. } = reply else {
            panic!("expected a content chunk response, got {reply:?}");
        };
        *found
    }

    fn document_query() -> HuginnMindRequest {
        HuginnMindRequest::Query {
            instance: slug(INSTANCE),
            selection: Selection { projection: "document".into(), ..Selection::default() },
            semantic: None,
        }
    }

    /// One operation asked of `answer` at an injected time, and the manifest of
    /// the deferred answer it gives back.
    fn ask_deferred(
        daemon: &mut BodyDaemon,
        bodies: &mut DeferredBodies,
        message_id: &str,
        read: &HuginnMindRequest,
        at: DateTime<Utc>,
    ) -> CultMeshCdnArtifactManifest {
        let registry = schema_registry().unwrap();
        deferred_manifest(&answer(daemon, &registry, bodies, encode_request(message_id, read, None).unwrap(), at).reply())
    }

    /// Whether the first chunk of a manifest is still served at an injected time.
    fn first_chunk_found(
        daemon: &mut BodyDaemon,
        bodies: &mut DeferredBodies,
        manifest: &CultMeshCdnArtifactManifest,
        at: DateTime<Utc>,
    ) -> bool {
        let registry = schema_registry().unwrap();
        found(&answer(daemon, &registry, bodies, chunk_request(manifest, 0), at).reply())
    }

    /// An answer larger than one send can carry is deferred to the body plane
    /// and fetches, on the same session, to the answer the mind gave. The wide
    /// document is one cut spec with every list the leaf bounds filled: about a
    /// megabyte of field content, which base64 in the operation envelope
    /// carries past the window this module configures, so a single read of it is
    /// already over. The fitting one is the same document narrowed until its
    /// envelope lands inside the window, and it is delivered whole.
    ///
    /// The client holds the `Deferred` answer before it fetches, so the size
    /// the daemon decided on is observed at the decision and not only
    /// downstream of the fetch.
    #[test]
    fn an_oversize_answer_arrives_deferred_and_fetches_to_the_same_answer() {
        let (_root, mut daemon) = seeded_wide();
        let registry = schema_registry().unwrap();

        let sized = |daemon: &mut BodyDaemon, id: PipelineRef| {
            let view = daemon.handle(HuginnMindRequest::View { instance: slug(INSTANCE), id }, now()).answered();
            let message = encode_response("m-0", "view", &view, "huginn-yggdrasil").unwrap();
            encoded_len(&message)
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

        let reads = [
            ("m-q", document_query()),
            ("m-v", HuginnMindRequest::View { instance: slug(INSTANCE), id: wide }),
        ];
        let expected: Vec<HuginnMindResponse> =
            reads.iter().map(|(_, read)| daemon.handle(read.clone(), now()).answered()).collect();

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

        let stopping = Arc::new(AtomicBool::new(false));
        let loop_stopping = Arc::clone(&stopping);
        let serving = std::thread::spawn(move || {
            let result = run(&mut daemon, &mut hub, &registry, &loop_stopping, &ServeOptions::default());
            (result, daemon)
        });

        // One read at a time: the window is per session and counts what is
        // still unacknowledged, so the fetch is one chunk in flight.
        for ((message_id, read), expected) in reads.into_iter().zip(expected) {
            let reply = exchange(&mut client, &encode_request(message_id, &read, None).unwrap()).unwrap();
            assert!(encoded_len(&reply) <= MAX_RESPONSE_BYTES, "the deferred answer fits one send");
            assert_eq!(decode_response(&reply).unwrap().0, message_id);
            let manifest = deferred_manifest(&reply);
            let bytes_expected = rmp_serde::to_vec_named(&expected).unwrap();
            assert_eq!(manifest.size_bytes as usize, bytes_expected.len(), "{message_id}: the manifest is of the answer");

            let bytes = fetch_content(&manifest, MAX_DEFERRED_BODY_BYTES, |request| exchange(&mut client, &request))
                .unwrap();
            assert_eq!(bytes, bytes_expected, "{message_id}: byte-identical to the answer the mind gave");
            assert_eq!(rmp_serde::from_slice::<HuginnMindResponse>(&bytes).unwrap(), expected);
            assert!(!matches!(expected, HuginnMindResponse::Deferred(_)));
        }

        // A real-sized answer that does fit is delivered whole, over the same
        // session that just fetched two bodies.
        let delivered = [
            ("m-f", HuginnMindRequest::View { instance: slug(INSTANCE), id: fitting.clone() }),
            ("m-w", HuginnMindRequest::Whoami),
        ];
        for (message_id, request) in delivered {
            let reply = exchange(&mut client, &encode_request(message_id, &request, None).unwrap()).unwrap();
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

    /// A reply carrying a chosen payload string, with every field the envelope
    /// echoes chosen too, so a size can be chosen instead of found.
    fn reply_with_payload(message_id: &str, operation: &str, runtime_id: &str, payload: String) -> CultNetMessage {
        CultNetMessage::OperationResponse {
            message_id: message_id.into(),
            service_id: MIND_SERVICE_ID.into(),
            operation: operation.into(),
            status: "accepted".into(),
            payload_schema: MIND_RESPONSE_SCHEMA.into(),
            payload_encoding: "messagepack-base64".into(),
            payload,
            diagnostics: vec![],
            source_runtime_id: Some(runtime_id.into()),
        }
    }

    /// One whose encoded envelope is exactly `target` bytes and whose payload is
    /// valid base64, under a `message_id` that is `stem` followed by as many
    /// zeros as make the payload a whole number of base64 quanta. The envelope's
    /// own overhead is measured, not assumed: the length prefix is the same
    /// width on either side of a megabyte, so one probe fixes it. The id is
    /// echoed, so it is part of the result.
    fn sized_exactly(stem: &str, operation: &str, runtime_id: &str, target: u64) -> (String, CultNetMessage) {
        let probe = 1_200_000_usize;
        for zeros in 0..4 {
            let message_id = format!("{stem}{}", "0".repeat(zeros));
            let at_probe = encoded_len(&reply_with_payload(&message_id, operation, runtime_id, "A".repeat(probe)));
            let payload_len = (probe as i64 + (target as i64 - at_probe as i64)) as usize;
            if payload_len % 4 == 0 {
                let message = reply_with_payload(&message_id, operation, runtime_id, "A".repeat(payload_len));
                assert_eq!(encoded_len(&message), target);
                return (message_id, message);
            }
        }
        unreachable!("one of four consecutive id lengths aligns the payload")
    }

    /// The boundary is the transport's, byte for byte, at every shape of
    /// envelope. A reply whose encoded envelope is exactly `MAX_RESPONSE_BYTES`
    /// is delivered unchanged and a hub `bind` configured accepts it; one byte
    /// more is deferred, not refused, and a hub refuses it.
    ///
    /// So the threshold measures the encoded envelope, which is what the hub
    /// fragments, and not the payload string inside it, which is a couple of
    /// hundred bytes smaller; it admits the limit rather than stopping one
    /// short of it; and it is the window itself and not a margin under it. The
    /// echoed ids, operations and runtime ids vary independently and in length,
    /// so a formula fitted to one envelope shape is measured where it was never
    /// fitted. The accepted reply is shown to have filled the window it was
    /// measured against, because a one-byte reply after it is refused.
    #[test]
    fn the_delivery_boundary_is_the_transports_boundary_byte_for_byte() {
        let long = format!("m-{}", "1".repeat(38));
        for (stem, operation, runtime_id) in [
            ("m-", "view", "huginn-yggdrasil"),
            (long.as_str(), "view", "huginn-yggdrasil"),
            ("m-", "whoami", "huginn-thought-cage"),
            ("m-", "whoami", "huginn-yggdrasil"),
            ("m-", "view", "huginn-thought-cage"),
        ] {
            let (id_at, at) = sized_exactly(stem, operation, runtime_id, MAX_RESPONSE_BYTES);
            let (id_over, over) = sized_exactly(stem, operation, runtime_id, MAX_RESPONSE_BYTES + 1);
            let mut bodies = fresh_bodies();
            let case = format!("{operation}/{runtime_id}/{}", stem.len());

            let delivered = deliver(at.clone(), &id_at, operation, runtime_id, &mut bodies, now());
            assert_eq!(delivered, at, "{case}: exactly the limit is delivered whole");
            assert_eq!(bodies.body_count(), 0, "{case}: nothing was retained for it");

            let deferred = deliver(over.clone(), &id_over, operation, runtime_id, &mut bodies, now());
            let (correlation, answered) = decode_response(&deferred).unwrap();
            assert_eq!(correlation, id_over, "{case}");
            let Some(HuginnMindResponse::Deferred(DeferredAnswer { manifest })) = answered.ok() else {
                panic!("{case}: one byte over the limit was not deferred: {deferred:?}");
            };
            assert_eq!(bodies.body_count(), 1, "{case}");
            assert!(
                encoded_len(&deferred) <= MAX_FRAGMENT_BYTES as u64 * 4,
                "{case}: the deferred answer is a few packets"
            );
            let CultNetMessage::OperationResponse { payload, .. } = &over else { unreachable!() };
            assert_eq!(manifest.size_bytes as usize, STANDARD.decode(payload).unwrap().len(), "{case}");

            if stem == "m-" && operation == "view" && runtime_id == "huginn-yggdrasil" {
                let mut hub = bind("127.0.0.1:0".parse().unwrap(), "huginn-yggdrasil").unwrap();
                let (_client, session) = settled_session(&mut hub);
                let error = hub.send_schema_message(&session, &over).unwrap_err();
                assert!(format!("{error:#}").contains("queue is full"), "limit + 1 was accepted: {error:#}");
                hub.send_schema_message(&session, &at).expect("exactly the limit is accepted on an empty window");
                hub.send_schema_message(&session, &reply_with_payload("m-0", "view", runtime_id, "A".into()))
                    .expect_err("the accepted answer filled the window, so nothing follows it");
            }
        }
    }

    /// The backstop stays, measured against the deferral bound and on the
    /// payload: a body of exactly `MAX_DEFERRED_BODY_BYTES` is deferred, one
    /// byte more is refused by name with its own size and that bound, and the
    /// deferred answer at the bound still fits one send, so it is never itself
    /// deferred.
    #[test]
    fn an_answer_over_the_deferral_bound_is_refused_by_name() {
        let payload_of = |len: u64| STANDARD.encode(vec![0_u8; len as usize]);
        let mut bodies = fresh_bodies();
        // The longest id a request may carry: the reply echoes it, so the
        // largest deferred answer and the largest refusal are measured at it.
        let id = "m".repeat(MAX_ECHOED_FIELD_BYTES);

        let at = reply_with_payload(&id, "view", "huginn-yggdrasil", payload_of(MAX_DEFERRED_BODY_BYTES));
        let deferred = deliver(at, &id, "view", "huginn-yggdrasil", &mut bodies, now());
        let manifest = deferred_manifest(&deferred);
        assert_eq!(manifest.size_bytes as u64, MAX_DEFERRED_BODY_BYTES);
        assert_eq!(manifest.chunks.len() as u64, MAX_DEFERRED_BODY_BYTES / DEFERRED_CHUNK_BYTES as u64);
        assert!(encoded_len(&deferred) <= MAX_RESPONSE_BYTES, "at the bound the deferred answer still fits one send");
        assert_eq!(bodies.body_count(), 1);
        drop(bodies);

        let over = reply_with_payload(&id, "view", "huginn-yggdrasil", payload_of(MAX_DEFERRED_BODY_BYTES + 1));
        let mut empty = fresh_bodies();
        let refused = deliver(over, &id, "view", "huginn-yggdrasil", &mut empty, now());
        let (correlation, answered) = decode_response(&refused).unwrap();
        assert_eq!(correlation, id);
        let HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge { bytes, limit }) =
            answered.expect("a refusal is an answer, not an envelope failure")
        else {
            panic!("one byte over the deferral bound was not refused: {refused:?}");
        };
        assert_eq!((bytes, limit), (MAX_DEFERRED_BODY_BYTES + 1, MAX_DEFERRED_BODY_BYTES));
        assert!(encoded_len(&refused) <= MAX_RESPONSE_BYTES, "the refusal fits one send at the longest id");
        assert_eq!(empty.body_count(), 0, "a refused answer retains nothing");
    }

    /// An oversize reply whose payload cannot be read as the response's bytes
    /// is a failure of the envelope, named, and never deferred.
    #[test]
    fn an_oversize_reply_that_is_not_a_response_is_not_deferred() {
        let mut bodies = fresh_bodies();
        let not_base64 = reply_with_payload("m-x", "view", "huginn-yggdrasil", "A".repeat(MAX_RESPONSE_BYTES as usize) + "!");
        let not_a_response =
            CultNetMessage::Error { error: "e".repeat(MAX_RESPONSE_BYTES as usize + 1), code: None, details: None };
        for reply in [not_base64, not_a_response] {
            assert!(encoded_len(&reply) > MAX_RESPONSE_BYTES);
            let answered = deliver(reply, "m-x", "view", "huginn-yggdrasil", &mut bodies, now());
            assert_eq!(rejected(&answered).1, "response-not-encodable");
        }
        assert_eq!(bodies.body_count(), 0);
    }

    /// Lifecycle end to end through the daemon's own `answer`, on an injected
    /// clock: a chunk served inside the TTL slides the expiry, a body untouched
    /// for the TTL is gone and its chunk is answered `found: false` by name,
    /// and the client asks again and fetches the same answer.
    #[test]
    fn an_expired_body_answers_found_false_and_the_client_can_ask_again() {
        let (_root, mut daemon) = seeded_wide();
        let registry = schema_registry().unwrap();
        let wide = spec_ref(&mut daemon, WIDE_CUT);
        let read = HuginnMindRequest::View { instance: slug(INSTANCE), id: wide };
        let expected = rmp_serde::to_vec_named(&daemon.handle(read.clone(), now()).answered()).unwrap();
        let t0 = now();
        let after = |seconds: i64| t0 + chrono::Duration::seconds(seconds);
        let mut bodies = fresh_bodies();

        let manifest = ask_deferred(&mut daemon, &mut bodies, "m-v", &read, t0);
        assert!(manifest.chunks.len() >= 2, "the fixture needs a second chunk to serve");
        let mut chunk = |index: usize, at: i64| {
            answer(&mut daemon, &registry, &mut bodies, chunk_request(&manifest, index), after(at)).reply()
        };
        assert!(found(&chunk(0, 59)), "inside the TTL");
        assert!(found(&chunk(1, 118)), "serving a chunk slid the expiry past the first TTL");
        let gone = chunk(0, 178);
        assert!(!found(&gone), "untouched for the TTL since t=118");
        let CultNetMessage::ContentChunkResponse { error, payload, .. } = &gone else { unreachable!() };
        assert!(error.starts_with("FileNotFoundException"), "{error}");
        assert!(payload.is_empty());

        let again = ask_deferred(&mut daemon, &mut bodies, "m-v2", &read, after(179));
        assert_eq!(again.content_hash, manifest.content_hash, "the same answer is the same body");
        let bytes = fetch_content(&again, MAX_DEFERRED_BODY_BYTES, |request| {
            Ok(answer(&mut daemon, &registry, &mut bodies, request, after(180)).reply())
        })
        .unwrap();
        assert_eq!(bytes, expected);
    }

    /// Past the budget the body touched longest ago is evicted, its chunks
    /// answer `found: false`, and asking again evicts the other in turn. Two
    /// identical answers are one body.
    #[test]
    fn eviction_and_sharing_through_the_daemons_answer() {
        let (_root, mut daemon) = seeded_wide();
        let wide = spec_ref(&mut daemon, WIDE_CUT);
        let view = HuginnMindRequest::View { instance: slug(INSTANCE), id: wide };
        let query = document_query();
        let size = |daemon: &mut BodyDaemon, read: &HuginnMindRequest| {
            rmp_serde::to_vec_named(&daemon.handle(read.clone(), now()).answered()).unwrap().len() as u64
        };
        let (view_bytes, query_bytes) = (size(&mut daemon, &view), size(&mut daemon, &query));
        // Room for the larger body alone, and not for both.
        let budget = view_bytes.max(query_bytes) + 1;
        let t0 = now();
        let after = |seconds: i64| t0 + chrono::Duration::seconds(seconds);
        let mut bodies = DeferredBodies::new(budget, TTL);

        let first = ask_deferred(&mut daemon, &mut bodies, "m-1", &view, after(0));
        let same = ask_deferred(&mut daemon, &mut bodies, "m-2", &view, after(1));
        assert_eq!(first.content_hash, same.content_hash, "two identical answers are one body");
        assert_eq!((bodies.body_count(), bodies.stored_bytes()), (1, view_bytes));

        let second = ask_deferred(&mut daemon, &mut bodies, "m-3", &query, after(2));
        assert_eq!(bodies.body_count(), 1, "the least recently touched body was evicted");
        assert!(bodies.stored_bytes() <= budget);
        assert!(!first_chunk_found(&mut daemon, &mut bodies, &first, after(3)), "the evicted body answers found: false");
        assert!(first_chunk_found(&mut daemon, &mut bodies, &second, after(3)), "the newer body stays");

        let again = ask_deferred(&mut daemon, &mut bodies, "m-4", &view, after(4));
        assert!(first_chunk_found(&mut daemon, &mut bodies, &again, after(5)));
        assert!(
            !first_chunk_found(&mut daemon, &mut bodies, &second, after(5)),
            "the client's re-ask evicted the one touched longest ago"
        );
    }

    /// Every field a reply echoes is bounded at the door, so no reply can
    /// outgrow the window: a request whose `message_id`, `service_id`,
    /// `operation`, `payload_schema` or `chunk_hash` is over the bound is
    /// answered with a transport `Error` that fits and carries none of them,
    /// and touches no mind; a field of exactly the bound is served as usual.
    /// The 1.3 MB id is the one that made a `Whoami` answer echo past the
    /// window, deferred it, and left the client waiting on a send the hub
    /// refused.
    #[test]
    fn an_overlong_echoed_field_is_refused_at_the_door_with_an_answer_that_fits() {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = open_unindexed(root.path(), &slug(INSTANCE)).unwrap();
        let registry = schema_registry().unwrap();
        let mut bodies = fresh_bodies();
        let huge = "x".repeat(1_300_000);
        let over = "x".repeat(MAX_ECHOED_FIELD_BYTES + 1);
        let at = "x".repeat(MAX_ECHOED_FIELD_BYTES);
        let whoami = |message_id: &str, service_id: &str, operation: &str, payload_schema: &str| {
            let payload = STANDARD.encode(rmp_serde::to_vec_named(&HuginnMindRequest::Whoami).unwrap());
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
        };
        let chunk = |message_id: &str, chunk_hash: &str, record_key: &str| CultNetMessage::ContentChunkRequest {
            message_id: message_id.into(),
            chunk_hash: chunk_hash.into(),
            record_key: record_key.into(),
            expected_size_bytes: 1,
        };
        let refused = [
            ("huge message_id", whoami(&huge, MIND_SERVICE_ID, "whoami", MIND_REQUEST_SCHEMA)),
            ("message_id", whoami(&over, MIND_SERVICE_ID, "whoami", MIND_REQUEST_SCHEMA)),
            ("service_id", whoami("m", &huge, "whoami", MIND_REQUEST_SCHEMA)),
            ("operation", whoami("m", MIND_SERVICE_ID, &huge, MIND_REQUEST_SCHEMA)),
            ("payload_schema", whoami("m", MIND_SERVICE_ID, "whoami", &huge)),
            (
                "catalog message_id",
                CultNetMessage::SchemaCatalogRequest {
                    message_id: huge.clone(),
                    include_schema_json: Some(true),
                    schema_ids: None,
                    kinds: None,
                },
            ),
            ("chunk message_id", chunk(&huge, "abc", "")),
            ("chunk_hash", chunk("m", &huge, "")),
            ("record_key", chunk("m", "abc", &huge)),
        ];
        for (case, message) in refused {
            let reply = answer(&mut daemon, &registry, &mut bodies, message, now()).reply();
            let CultNetMessage::Error { error, .. } = &reply else {
                panic!("{case}: expected a transport error, got {reply:?}");
            };
            assert!(error.contains("over 256 bytes"), "{case}: {error}");
            assert!(encoded_len(&reply) <= MAX_FRAGMENT_BYTES as u64, "{case}: the refusal is one packet");
        }
        assert_eq!(documents(&mut daemon), 0);

        let reply = answer(&mut daemon, &registry, &mut bodies, whoami(&at, MIND_SERVICE_ID, "whoami", MIND_REQUEST_SCHEMA), now()).reply();
        assert_eq!(decode_response(&reply).unwrap().0, at, "a field of exactly the bound is served");
        let reply = answer(&mut daemon, &registry, &mut bodies, chunk(&at, &at, &at), now()).reply();
        assert!(!found(&reply), "an unknown chunk is answered found: false, not refused at the door");
    }

    /// The envelope's status is the answer's own whether it is sent whole or
    /// deferred, so a client that routes on status routes the same way.
    #[test]
    fn a_deferred_answer_keeps_the_status_of_the_answer_it_stands_for() {
        for status in ["rejected", "accepted"] {
            let payload = STANDARD.encode(vec![0_u8; MAX_RESPONSE_BYTES as usize]);
            let mut reply = reply_with_payload("m-s", "view", "huginn-yggdrasil", payload);
            let CultNetMessage::OperationResponse { status: carried, .. } = &mut reply else { unreachable!() };
            *carried = status.into();
            let mut bodies = fresh_bodies();
            let deferred = deliver(reply, "m-s", "view", "huginn-yggdrasil", &mut bodies, now());
            let CultNetMessage::OperationResponse { status: envelope, .. } = &deferred else {
                panic!("expected an operation response, got {deferred:?}");
            };
            assert_eq!(envelope, status);
            deferred_manifest(&deferred);
        }
    }

    /// One cut spec's own reference, by its cut label.
    fn spec_ref(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, NoIndex>, cut: &str) -> PipelineRef {
        let selection = Selection {
            schemas: Some(vec![PipelineKind::CutSpec.type_id().into()]),
            fields: Some(vec![FieldPredicate {
                index: "cut".into(),
                op: "any_of".into(),
                values: Some(vec![cut.into()]),
                number: None,
            }]),
            ..Selection::default()
        };
        let HuginnMindResponse::Query(page) =
            daemon.handle(HuginnMindRequest::Query { instance: slug(INSTANCE), selection, semantic: None }, now()).answered()
        else {
            panic!("expected a page");
        };
        assert_eq!(page.matched, 1, "one cut spec is labelled {cut}");
        let PipelinePageItems::Headers(headers) = &page.items else { panic!("expected headers") };
        headers[0].id.clone()
    }

    /// Ruling 15 as an order in the one place both happen. Both gates are shut
    /// at once: the mind is held and the port is held. A startup that opens
    /// first says why the mind refused; one that binds first can only say the
    /// port was taken, and never reaches the refusal that matters.
    #[test]
    fn startup_opens_the_mind_before_it_binds_anything() {
        let root = tempfile::tempdir().unwrap();
        let held = open_unindexed(root.path(), &slug(INSTANCE)).unwrap();
        let occupied = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = occupied.local_addr().unwrap();

        let options = Options {
            state_root: root.path().to_path_buf(),
            instance: slug(INSTANCE),
            bind: port,
            qdrant_url: "http://127.0.0.1:1".into(),
            ollama_url: "http://127.0.0.1:1".into(),
            embedding_model: "a-model".into(),
        };
        let started = |options: &Options| {
            let (embedder, index) = crate::index::fakes::pair();
            startup(options, embedder, index, Backoff::default())
        };
        let error = format!("{:#}", started(&options).err().expect("a held mind refuses"));
        assert!(error.contains("MindAlreadyOwned"), "the refusal is the mind's, not the socket's: {error}");
        assert!(!error.contains("binding"), "the socket was never reached: {error}");

        // With the mind free and the port still held, the socket is the only
        // gate left, and startup reports it.
        drop(held);
        let error = format!("{:#}", started(&options).err().expect("a held port refuses"));
        assert!(error.contains("binding"), "{error}");
        drop(occupied);
        let (_daemon, hub, _registry) = started(&options).expect("both gates open");
        assert_eq!(hub.local_addr().unwrap(), port);
    }

    #[test]
    fn options_are_exactly_the_six_the_operator_supplies() {
        let root = if cfg!(windows) { r"C:\state" } else { "/state" };
        let args = |values: &[&str]| values.iter().map(|value| value.to_string()).collect::<Vec<_>>().into_iter();
        let good = [
            "--state-root", root, "--instance", "yggdrasil", "--bind", "127.0.0.1:17872", "--qdrant-url",
            "http://127.0.0.1:6333", "--ollama-url", "http://10.77.0.4:11434", "--embedding-model", "qwen3-embedding:0.6b",
        ];
        assert_eq!(
            parse_options(args(&good)).unwrap(),
            Options {
                state_root: PathBuf::from(root),
                instance: slug("yggdrasil"),
                bind: "127.0.0.1:17872".parse().unwrap(),
                qdrant_url: "http://127.0.0.1:6333".into(),
                ollama_url: "http://10.77.0.4:11434".into(),
                embedding_model: "qwen3-embedding:0.6b".into(),
            }
        );
        // Each of the six is required: there is no index-less mode.
        for name in ["--state-root", "--instance", "--bind", "--qdrant-url", "--ollama-url", "--embedding-model"] {
            let at = good.iter().position(|value| *value == name).unwrap();
            let mut without = good.to_vec();
            without.drain(at..at + 2);
            assert!(parse_options(args(&without)).is_err(), "{name} is required");
        }
        fn with<'a>(good: &[&'a str], name: &str, value: &'a str) -> Vec<&'a str> {
            let at = good.iter().position(|option| *option == name).unwrap();
            let mut changed = good.to_vec();
            changed[at + 1] = value;
            changed
        }
        for bad in [
            [good.to_vec(), vec!["--bind", "127.0.0.1:2"]].concat(),
            with(&good, "--bind", "not-an-address"),
            with(&good, "--state-root", "relative"),
            with(&good, "--qdrant-url", "https://127.0.0.1:6333"),
            with(&good, "--ollama-url", "10.77.0.4:11434"),
            with(&good, "--embedding-model", ""),
            [good.to_vec(), vec!["--idunn-anchor", "x"]].concat(),
        ] {
            assert!(parse_options(args(&bad)).is_err(), "{bad:?}");
        }
    }
}
