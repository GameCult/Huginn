//! The Self run's credential lease, from the asking side (rulings
//! `eureka-body:ruling:self-runs-through-connector` and
//! `eureka-body:ruling:self-run-credential-lease`).
//!
//! The Claude connector is the only holder of the subscription token and the
//! one owner of Self-run admission, reservation and the spend ledger. A Self run
//! asks it over a unix socket for a lease. The connector reserves the run's
//! maximum cost, then answers with one frame and, with it, one descriptor: the
//! read end of a pipe that holds the token, its write end already closed. The
//! run settles with what it used, per model. A run that never
//! settles (killed by the brake, a crash, a dropped connection) is billed the
//! whole reservation by the connector, which is why [`Lease`] settles only by
//! being told to, and a dropped one just closes the connection.
//!
//! The token is never read here. It travels as a descriptor from the socket to
//! the child's descriptor table and is not in an environment, an argument, a
//! file, a log or any value this module can print.
//!
//! Wire: a `u32` big-endian length, then a MessagePack named map `{kind, value}`
//! of at most [`MAX_LEASE_FRAME_BYTES`]. The connector owns the format; the
//! golden frames under `lease_client/lease_v1` are copies of its fixtures.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

/// The variable the Self unit's environment names the connector's lease socket
/// in. The library reads no variable: the binary reads it and builds a
/// [`SocketLender`].
pub const LEASE_SOCKET_VARIABLE: &str = "CLAUDE_LEASE_SOCKET";

/// The largest frame either side sends or accepts, in bytes.
pub const MAX_LEASE_FRAME_BYTES: usize = 64 * 1024;

/// What a run asks for: a lease until `expires_at_unix_ms` (the connector bills a
/// lease that outlives it). The connector, not the caller, sets the reservation
/// and binds the lease to the requesting unit's invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseRequest {
    pub expires_at_unix_ms: u64,
}

/// Why the connector would not lend, as it names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseRefusalKind {
    SpendCeiling,
    CostBound,
    UnknownCaller,
    Expired,
    Busy,
}

/// A refusal: the kind is the typed answer, the detail the connector's words
/// (never the token, and not echoed by the actuator).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseRefused {
    pub kind: LeaseRefusalKind,
    pub detail: String,
}

/// Tokens a run used, as the connector prices them. Cache reads and writes are
/// counted apart from plain input; `output_tokens` already includes reasoning.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: Option<u64>,
}

/// The tokens one model used in a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub usage: TokenUsage,
}

/// What a run tells the connector when it ends: its usage, per model. The
/// connector trusts only its own observed plan quota.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settlement {
    pub usage: Vec<ModelUsage>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub(crate) enum LeaseMessage {
    #[serde(rename = "claude_lease_request")]
    Request(LeaseRequest),
    /// Travels with exactly one SCM_RIGHTS descriptor.
    #[serde(rename = "claude_lease_grant")]
    Grant(LeaseGranted),
    #[serde(rename = "claude_lease_refusal")]
    Refusal(LeaseRefused),
    #[serde(rename = "claude_lease_settle")]
    Settle(LeaseSettle),
    #[serde(rename = "claude_lease_settled")]
    Settled(LeaseSettled),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LeaseGranted {
    pub(crate) lease_id: String,
    pub(crate) reserved_micro_usd: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LeaseSettle {
    pub(crate) lease_id: String,
    pub(crate) usage: Vec<ModelUsage>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LeaseSettled {
    pub(crate) billed_micro_usd: u64,
    pub(crate) overrun_micro_usd: u64,
}

/// A frame: the length, then the named-map MessagePack of `message`.
pub(crate) fn encode_frame(message: &LeaseMessage) -> Result<Vec<u8>> {
    let body = rmp_serde::to_vec_named(message)?;
    if body.len() > MAX_LEASE_FRAME_BYTES {
        bail!("a lease frame is over the bound");
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&u32::try_from(body.len())?.to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

#[cfg(test)]
/// The message in one whole frame, or none when the bytes are not exactly one
/// well-formed message within the bound.
pub(crate) fn decode_frame(frame: &[u8]) -> Option<LeaseMessage> {
    let (length, body) = frame.split_first_chunk::<4>()?;
    let length = usize::try_from(u32::from_be_bytes(*length)).ok()?;
    if length == 0 || length > MAX_LEASE_FRAME_BYTES || body.len() != length {
        return None;
    }
    rmp_serde::from_slice(body).ok()
}

#[cfg(unix)]
type Held = std::os::fd::OwnedFd;
#[cfg(not(unix))]
type Held = ();
#[cfg(unix)]
type Placed = std::os::fd::OwnedFd;
#[cfg(not(unix))]
type Placed = std::convert::Infallible;

/// The token as the descriptor the connector sent. It is close-on-exec, so no
/// process the actuator starts inherits it by accident; the only way to hand it
/// to a child is [`TokenFd::inheritable`].
pub struct TokenFd(Held);

/// A duplicate of the token descriptor that survives exec, at a number above
/// stdio. It exists to be named to the child and dropped right after the spawn.
pub struct Inheritable(Placed);

impl std::fmt::Debug for TokenFd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenFd")
    }
}

#[cfg(unix)]
impl TokenFd {
    pub(crate) fn from_owned(fd: std::os::fd::OwnedFd) -> Self {
        Self(fd)
    }

    /// The duplicate to inherit: lowest free number from 3 up (never a stdio
    /// number, even if stdio were closed), close-on-exec cleared.
    pub fn inheritable(&self) -> Result<Inheritable> {
        use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_setfd};
        let placed = fcntl_dupfd_cloexec(&self.0, 3)?;
        fcntl_setfd(&placed, FdFlags::empty())?;
        Ok(Inheritable(placed))
    }
}

#[cfg(not(unix))]
impl TokenFd {
    pub fn inheritable(&self) -> Result<Inheritable> {
        bail!("the credential lease needs a unix host")
    }
}

impl Inheritable {
    /// The descriptor number the child is told to read the token from.
    #[cfg(unix)]
    pub fn number(&self) -> i32 {
        std::os::fd::AsRawFd::as_raw_fd(&self.0)
    }

    #[cfg(not(unix))]
    pub fn number(&self) -> i32 {
        match self.0 {}
    }
}

/// A held lease: the token, and the means to settle it exactly once.
pub trait Lease {
    fn token(&self) -> &TokenFd;
    /// Tells the connector what the run used, and answers what it billed in
    /// micro-USD. A lease dropped without this is billed its whole reservation.
    fn settle(self: Box<Self>, settlement: Settlement) -> Result<u64>;
}

/// The connector's answer to a request.
pub enum Lent {
    Granted(Box<dyn Lease>),
    Refused(LeaseRefused),
}

/// Where leases come from. A transport fault is an error; a refusal is an answer.
pub trait Lender {
    fn lend(&self, request: &LeaseRequest) -> Result<Lent>;
}

/// The connector's lease socket.
pub struct SocketLender {
    path: PathBuf,
}

impl SocketLender {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[cfg(not(unix))]
impl Lender for SocketLender {
    fn lend(&self, _request: &LeaseRequest) -> Result<Lent> {
        let _ = &self.path;
        Err(anyhow!("the credential lease needs a unix host"))
    }
}

#[cfg(unix)]
impl Lender for SocketLender {
    fn lend(&self, request: &LeaseRequest) -> Result<Lent> {
        socket::lend(&self.path, request)
    }
}

#[cfg(unix)]
mod socket {
    use std::io::{IoSliceMut, Read, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::Duration;

    use anyhow::{Context, Result, anyhow, bail};
    use rustix::io::Errno;
    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};

    use super::{LeaseMessage, LeaseRequest, Lease, Lent, Settlement, TokenFd, decode_frame_body, encode_frame};
    use super::{LeaseSettle, MAX_LEASE_FRAME_BYTES};

    /// How long the connector may take over any one frame.
    const IO_TIMEOUT: Duration = Duration::from_secs(30);

    pub(super) fn lend(path: &Path, request: &LeaseRequest) -> Result<Lent> {
        let mut stream = UnixStream::connect(path).context("the lease socket could not be reached")?;
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;
        stream.write_all(&encode_frame(&LeaseMessage::Request(request.clone()))?)?;
        let (message, mut fds) = receive(&stream)?;
        match message {
            LeaseMessage::Grant(grant) => {
                let (Some(fd), None) = (fds.pop(), fds.pop()) else { bail!("a lease grant carries exactly one descriptor") };
                if grant.lease_id.is_empty() {
                    bail!("a lease grant names no lease");
                }
                Ok(Lent::Granted(Box::new(Granted { token: TokenFd::from_owned(fd), lease_id: grant.lease_id, stream })))
            }
            LeaseMessage::Refusal(refused) if fds.is_empty() => Ok(Lent::Refused(refused)),
            _ => Err(anyhow!("the connector answered a lease request with something else")),
        }
    }

    /// One frame and any descriptors that arrived with it. The descriptors
    /// travel with the frame's first byte, so they come with the length; every
    /// receive asks for close-on-exec.
    fn receive(stream: &UnixStream) -> Result<(LeaseMessage, Vec<OwnedFd>)> {
        let mut fds = Vec::new();
        let mut length = [0_u8; 4];
        let mut filled = 0;
        while filled < length.len() {
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut control = RecvAncillaryBuffer::new(&mut space);
            let mut vector = [IoSliceMut::new(&mut length[filled..])];
            let got = match recvmsg(stream, &mut vector, &mut control, RecvFlags::CMSG_CLOEXEC) {
                Ok(got) => got,
                Err(Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            for message in control.drain() {
                if let RecvAncillaryMessage::ScmRights(received) = message {
                    fds.extend(received);
                }
            }
            if got.flags.contains(ReturnFlags::CTRUNC) {
                bail!("descriptors were dropped from a lease frame");
            }
            if got.bytes == 0 {
                bail!("the lease socket closed");
            }
            filled += got.bytes;
        }
        let size = usize::try_from(u32::from_be_bytes(length))?;
        if size == 0 || size > MAX_LEASE_FRAME_BYTES {
            bail!("a lease frame is outside its bound");
        }
        let mut body = vec![0_u8; size];
        let mut reader = stream;
        reader.read_exact(&mut body)?;
        Ok((decode_frame_body(&body)?, fds))
    }

    /// The lease on the wire: its token, and the connection a settle goes down.
    /// Dropping it closes the connection, which the connector bills in full.
    struct Granted {
        token: TokenFd,
        lease_id: String,
        stream: UnixStream,
    }

    impl Lease for Granted {
        fn token(&self) -> &TokenFd {
            &self.token
        }

        fn settle(self: Box<Self>, settlement: Settlement) -> Result<u64> {
            let mut stream = self.stream;
            let frame = encode_frame(&LeaseMessage::Settle(LeaseSettle {
                lease_id: self.lease_id,
                usage: settlement.usage,
            }))?;
            stream.write_all(&frame)?;
            let (message, fds) = receive(&stream)?;
            match message {
                LeaseMessage::Settled(settled) if fds.is_empty() => Ok(settled.billed_micro_usd),
                _ => Err(anyhow!("the connector answered a settle with something else")),
            }
        }
    }
}

/// One message from a frame body already read within the bound.
fn decode_frame_body(body: &[u8]) -> Result<LeaseMessage> {
    rmp_serde::from_slice(body).map_err(|_| anyhow!("a lease frame is not a lease message"))
}

#[cfg(test)]
mod tests;

/// A token descriptor holding `text`, as the connector's pipe does: written
/// whole, write end closed.
#[cfg(test)]
pub(crate) fn token_holding(text: &str) -> TokenFd {
    #[cfg(unix)]
    {
        let (read, mut write) = std::io::pipe().unwrap();
        std::io::Write::write_all(&mut write, text.as_bytes()).unwrap();
        TokenFd::from_owned(read.into())
    }
    #[cfg(not(unix))]
    {
        let _ = text;
        TokenFd(())
    }
}
