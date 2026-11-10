//! The desk door: the second door onto the one mind, a unix socket one peer may speak on.
//!
//! This module owns three things. Who may speak: the kernel's report of the connecting
//! process's uid must equal the configured one, and it is asked before a byte is read, so a
//! refused peer has nothing read and nothing written. The socket's life: bind, replace a stale
//! one, refuse a live one or a file that is not a socket, mode 0660, unlink on drop. And the
//! bounds: one request per connection, one reply, a header over `DESK_MAX_REQUEST_BYTES` refused
//! before anything is allocated, one `DESK_CONNECTION_DEADLINE` for the whole connection (every
//! read and write spends what is left of it, so a peer that trickles bytes is cut off at the
//! deadline like one that is silent), a reply past `DESK_MAX_RESPONSE_BYTES` refused by name.
//!
//! The trust boundary of the reads (ruling `desk-read-visibility-whole-record`). The peer-uid check
//! IS the whole authentication: Huginn must be reachable only by Bifrost's peer-checked socket, so
//! the configured uid is Bifrost's and no other process's. `Forks` and `Fork` return the WHOLE record
//! (cards, outcomes, every decider's note) to whoever passes that check; nothing in Huginn strips a
//! note or gates a read on `subject` or `deciders` (`deciders` only fills the card and the
//! showability gate). Bifrost applies visibility at its own door (a stripped read serves `note: null`,
//! CultLib `fork-card.md`). A second reader of this socket is therefore a note leak, and a deployment
//! that gives the socket to any uid but Bifrost's has no reader check left. The reads carry no
//! `authority` (a payload that names one is refused whole), and the door has no other way to ask who
//! is reading.
//!
//! It owns no rule. Which Mind method answers is `Daemon::handle_desk`'s; when the door is
//! served is `serve::run`'s, once per pass in the thread that owns the Mind. A peer that is
//! slow costs the loop at most `DESK_ACCEPTS_PER_PASS` connections of `DESK_CONNECTION_DEADLINE`
//! each, four seconds, and only the configured peer can reach that.
//!
//! The door keeps no request id: a retry after a lost reply is a new connection with the same
//! request, and the stored answer is what replays it, across a restart as well.

use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Utc};
use cultnet_rs::{
    CultNetMessage, CultNetSchemaKind, CultNetSchemaRegistration, CultNetSchemaRegistry, CultNetTransportProfile, CultNetWireContract,
    TcpFramedTransportConnection, TcpFramedTransportProfileOptions, create_tcp_framed_transport_profile, decode_cultnet_message_from_slice,
    encode_cultnet_message_to_vec,
};
use huginn_mind::desk_wire::{
    DESK_MAX_REQUEST_BYTES, DESK_MAX_RESPONSE_BYTES, DESK_REQUEST_SCHEMA, DESK_REQUEST_SCHEMA_JSON, DESK_RESPONSE_SCHEMA,
    DESK_RESPONSE_SCHEMA_JSON, DESK_SERVICE_ID, DeskResponse,
};
use huginn_mind::envelope::{OperationFailure, decode_desk_request, encode_desk_failure, encode_desk_response};
use huginn_mind::{MindRefusal, MindStore};
use rustix::net::sockopt::{Timeout, set_socket_timeout};

use crate::daemon::{Daemon, IndexSink};
use crate::serve::{MAX_ECHOED_FIELD_BYTES, overlong_echo};

/// How long one connection of the configured peer may hold the loop, reading and writing together.
pub const DESK_CONNECTION_DEADLINE: Duration = Duration::from_secs(1);

/// Connections one loop pass serves, so a peer that connects in a stream cannot hold the RUDP door off.
pub const DESK_ACCEPTS_PER_PASS: u32 = 4;

/// The listening socket, and the one uid that may speak on it.
pub struct DeskDoor {
    listener: UnixListener,
    path: PathBuf,
    peer_uid: u32,
    registry: CultNetSchemaRegistry,
}

/// What became of one connection. `Display` is the log line for the ones that are not answered.
#[derive(Debug, PartialEq, Eq)]
pub enum Served {
    Answered,
    /// The peer's uid is not the configured one: nothing was read or written.
    Refused { uid: u32, expected: u32 },
    /// The connection ended without a reply, and why.
    Dropped(String),
}

impl fmt::Display for Served {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Answered => formatter.write_str("desk door answered a peer"),
            Self::Refused { uid, expected } => write!(formatter, "desk door refused a peer with uid {uid} (expects {expected})"),
            Self::Dropped(why) => write!(formatter, "desk door dropped a connection: {why}"),
        }
    }
}

/// The two desk contracts, as the door's catalog advertises them and nothing else.
pub fn desk_registry() -> Result<CultNetSchemaRegistry> {
    let mut registry = CultNetSchemaRegistry::new();
    for (schema_id, title, schema_json) in [
        (DESK_REQUEST_SCHEMA, "Huginn desk request", DESK_REQUEST_SCHEMA_JSON),
        (DESK_RESPONSE_SCHEMA, "Huginn desk response", DESK_RESPONSE_SCHEMA_JSON),
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

/// Binds the desk socket. A path that exists and is not a socket is refused; a socket that
/// accepts a connection is a live server and is refused; a socket that refuses the connection is
/// stale and is replaced (`Mind::open`'s exclusive lock has already shown no live daemon of this
/// instance). The socket is then mode 0660 and non-blocking.
pub fn bind_desk(path: &Path, peer_uid: u32) -> Result<DeskDoor> {
    ensure!(path.is_absolute(), "the desk socket path must be absolute, got {}", path.display());
    ensure!(peer_uid != 0, "the desk peer uid must not be 0");
    match fs::symlink_metadata(path) {
        Ok(found) => {
            ensure!(found.file_type().is_socket(), "{} exists and is not a socket", path.display());
            match UnixStream::connect(path) {
                Ok(_) => bail!("{} is a live socket; another server holds it", path.display()),
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(path).with_context(|| format!("removing the stale socket {}", path.display()))?;
                }
                Err(error) => return Err(error).with_context(|| format!("probing the socket {}", path.display())),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    }
    let listener = UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    // The door owns the path from here, so a failure below still unlinks it on drop.
    let door = DeskDoor { listener, path: path.to_path_buf(), peer_uid, registry: desk_registry()? };
    fs::set_permissions(path, fs::Permissions::from_mode(0o660)).with_context(|| format!("setting the mode of {}", path.display()))?;
    door.listener.set_nonblocking(true)?;
    Ok(door)
}

impl DeskDoor {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Serves at most `DESK_ACCEPTS_PER_PASS` waiting connections, each to completion, and says
    /// how many it took. A refusal or a dropped connection is logged and counted: the loop was
    /// busy with it.
    pub fn serve_ready<S: MindStore, I: IndexSink<S>>(&mut self, daemon: &mut Daemon<S, I>, now: DateTime<Utc>) -> u32 {
        let mut taken = 0;
        while taken < DESK_ACCEPTS_PER_PASS {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    eprintln!("huginn: the desk socket could not accept: {error}");
                    break;
                }
            };
            taken += 1;
            let served = serve_connection(stream, self.peer_uid, daemon, &self.registry, now, |stream| stream.set_nonblocking(false));
            if served != Served::Answered {
                eprintln!("huginn: {served}");
            }
        }
        taken
    }
}

impl Drop for DeskDoor {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn profile() -> CultNetTransportProfile {
    create_tcp_framed_transport_profile(
        "huginn-desk",
        TcpFramedTransportProfileOptions { max_payload_bytes: Some(DESK_MAX_REQUEST_BYTES as u32), ..Default::default() },
    )
}

/// A stream whose every read and write is bounded by what is left of one deadline: the socket's
/// timeout is reset to the remainder before each call, so bytes arriving do not extend it.
pub(crate) struct Deadlined<St> {
    inner: St,
    deadline: Instant,
}

impl<St: AsFd> Deadlined<St> {
    pub(crate) fn new(inner: St, within: Duration) -> Self {
        Self { inner, deadline: Instant::now() + within }
    }

    fn remaining(&self, which: Timeout) -> io::Result<()> {
        // Whole milliseconds: rustix rounds a sub-microsecond tail up into an out-of-range tv_usec.
        let left = Duration::from_millis(self.deadline.saturating_duration_since(Instant::now()).as_millis() as u64);
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        set_socket_timeout(&self.inner, which, Some(left)).map_err(io::Error::from)
    }
}

impl<St: Read + AsFd> Read for Deadlined<St> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.remaining(Timeout::Recv)?;
        self.inner.read(buf)
    }
}

impl<St: Write + AsFd> Write for Deadlined<St> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.remaining(Timeout::Send)?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// One connection: the peer first, before anything is read, then one frame in, one frame out,
/// all inside `DESK_CONNECTION_DEADLINE`. `ready` readies the stream for blocking I/O and runs
/// only after the peer is known to be the configured one.
pub fn serve_connection<St, S, I>(
    stream: St,
    peer_uid: u32,
    daemon: &mut Daemon<S, I>,
    registry: &CultNetSchemaRegistry,
    now: DateTime<Utc>,
    ready: impl FnOnce(&St) -> io::Result<()>,
) -> Served
where
    St: Read + Write + AsFd,
    S: MindStore,
    I: IndexSink<S>,
{
    let uid = match rustix::net::sockopt::socket_peercred(&stream) {
        Ok(credentials) => credentials.uid.as_raw(),
        Err(error) => return Served::Dropped(format!("the peer's credentials could not be read: {error}")),
    };
    if uid != peer_uid {
        return Served::Refused { uid, expected: peer_uid };
    }
    if let Err(error) = ready(&stream) {
        return Served::Dropped(format!("the stream could not be readied: {error}"));
    }
    let mut transport = TcpFramedTransportConnection::new(Deadlined::new(stream, DESK_CONNECTION_DEADLINE), profile());
    let frame = match transport.receive() {
        Ok(frame) => frame,
        Err(error) => return Served::Dropped(format!("no whole frame arrived: {error:#}")),
    };
    let message = match decode_cultnet_message_from_slice(&frame.payload, CultNetWireContract::CultNetSchemaV0) {
        Ok(message) => message,
        Err(error) => return Served::Dropped(format!("a frame was not a CultNet message: {error:#}")),
    };
    let reply = answer_desk_message(daemon, registry, message, now);
    let bytes = match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) => bytes,
        Err(error) => return Served::Dropped(format!("the reply could not be encoded: {error:#}")),
    };
    match transport.send("schema", &bytes) {
        Ok(()) => Served::Answered,
        Err(error) => Served::Dropped(format!("the reply was not delivered: {error:#}")),
    }
}

/// One message in, one message out. Nothing here reaches a mind except through
/// `Daemon::handle_desk`, and an envelope that does not decode never gets that far.
pub fn answer_desk_message<S: MindStore, I: IndexSink<S>>(
    daemon: &mut Daemon<S, I>,
    registry: &CultNetSchemaRegistry,
    message: CultNetMessage,
    now: DateTime<Utc>,
) -> CultNetMessage {
    let runtime_id = daemon.runtime_id();
    if let Some(field) = overlong_echo(&message) {
        return CultNetMessage::Error {
            error: format!("{DESK_SERVICE_ID} refuses a {field} over {MAX_ECHOED_FIELD_BYTES} bytes"),
            code: None,
            details: None,
        };
    }
    match &message {
        CultNetMessage::OperationRequest { message_id, operation, .. } => match decode_desk_request(&message) {
            Ok((message_id, request)) => {
                let operation = request.operation();
                let response = daemon.handle_desk(request, now);
                respond(&response, &message_id, operation, &runtime_id, DESK_MAX_RESPONSE_BYTES)
            }
            Err(failure) => encode_desk_failure(message_id, operation, &failure, &runtime_id),
        },
        CultNetMessage::SchemaCatalogRequest { .. } => match registry.create_catalog_response(&message) {
            Ok(reply) => bounded(reply, DESK_MAX_RESPONSE_BYTES, |size| CultNetMessage::Error {
                error: match size {
                    Ok(bytes) => format!("{DESK_SERVICE_ID} refuses a catalog reply of {bytes} bytes over {DESK_MAX_RESPONSE_BYTES}"),
                    Err(detail) => detail,
                },
                code: None,
                details: None,
            }),
            Err(error) => CultNetMessage::Error { error: format!("{error:#}"), code: None, details: None },
        },
        _ => CultNetMessage::Error {
            error: format!("{DESK_SERVICE_ID} answers cultnet.operation_request.v0 and cultnet.schema_catalog_request.v0"),
            code: None,
            details: None,
        },
    }
}

/// One answer as its message, or, past `limit` encoded, the refusal that names the size.
pub(crate) fn respond(response: &DeskResponse, message_id: &str, operation: &str, runtime_id: &str, limit: usize) -> CultNetMessage {
    let not_encodable = |detail: String| {
        encode_desk_failure(message_id, operation, &OperationFailure { code: "response-not-encodable".into(), message: detail }, runtime_id)
    };
    let reply = match encode_desk_response(message_id, operation, response, runtime_id) {
        Ok(reply) => reply,
        Err(error) => return not_encodable(format!("{error:#}")),
    };
    bounded(reply, limit, |size| match size {
        Ok(bytes) => {
            let refusal = DeskResponse::Refused(MindRefusal::ResponseTooLarge { bytes: bytes as u64, limit: limit as u64 });
            encode_desk_response(message_id, operation, &refusal, runtime_id).unwrap_or_else(|error| not_encodable(format!("{error:#}")))
        }
        Err(detail) => not_encodable(detail),
    })
}

/// `reply` itself when it encodes to at most `limit` bytes. Otherwise what `refuse` makes of the
/// encoded size, or of the encoding error. Every reply the door sends passes here.
fn bounded(reply: CultNetMessage, limit: usize, refuse: impl FnOnce(Result<usize, String>) -> CultNetMessage) -> CultNetMessage {
    match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) if bytes.len() <= limit => reply,
        Ok(bytes) => refuse(Ok(bytes.len())),
        Err(error) => refuse(Err(format!("{error:#}"))),
    }
}
