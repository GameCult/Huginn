//! `eureka-state`, the client core: one call to a Huginn daemon.
//!
//! `HuginnClient::call` sends one `HuginnMindRequest` in CultNet's operation
//! envelope over RUDP, waits for the answer, and returns it as the mind's own
//! `HuginnMindResponse`. An answer too large for one send arrives as a
//! `Deferred` manifest; the client fetches the body on the same session,
//! verifies it, and decodes it, so the caller sees the answer the mind gave
//! and never the deferral.
//!
//! The client owns the transport and no state. A refusal is an answer, so it
//! comes back as `Ok(HuginnMindResponse::Refused(..))`. `ClientError` is the
//! only error, and it means the daemon could not be reached or did not answer
//! in the shape the protocol promises.
//!
//! Every call is its own session: it connects, asks, fetches what was
//! deferred, and disconnects, whether the call succeeded or failed. Nothing is
//! kept between calls.

use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, CultNetMessage,
    CultNetRudpSocketTransportConnection, CultNetWireContract, encode_cultnet_message_to_vec, fetch_content,
};
use huginn_mind::envelope::{OperationFailure, decode_response, encode_request};
use huginn_mind::epiphany_pipeline::Slug;
use huginn_mind::{HuginnMindRequest, HuginnMindResponse, MAX_DEFERRED_BODY_BYTES};

/// The correlation key of the one request a session carries.
const MESSAGE_ID: &str = "eureka-state-call";
const POLL: Duration = Duration::from_millis(2);

/// The largest encoded message one send carries. The client's RUDP session
/// does not fragment (`max_fragment_bytes` is unset), so a message is one UDP
/// datagram: at most 65,507 bytes over IPv4 (65,535 less the 8-byte UDP and
/// 20-byte IP headers), less RUDP's 36-byte fixed header (`RUDP_FIXED_HEADER_BYTES`
/// in `cultnet-rs`, private there) and the 6-byte `schema` channel id. A larger
/// message makes the operating system refuse the send ("Message too long").
pub const MAX_REQUEST_BYTES: usize = 65_507 - 36 - "schema".len();

/// Why a call produced no answer. `Unavailable` is the transport's failure and
/// the only one worth retrying: the daemon could not be reached, went quiet
/// past the call's deadline, or answered out of protocol. `Rejected` is the
/// daemon refusing the envelope itself, and `TooLarge` is the client refusing
/// a request that no single send can carry, before sending it; a retry of the
/// same call cannot change either.
#[derive(Debug)]
pub enum ClientError {
    Unavailable { endpoint: SocketAddr, detail: String },
    Rejected { endpoint: SocketAddr, code: String, detail: String },
    TooLarge { bytes: usize, limit: usize },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { endpoint, detail } => {
                write!(f, "the Huginn daemon at rudp://{endpoint} is unavailable: {detail}")
            }
            Self::Rejected { endpoint, code, detail } => {
                write!(f, "the Huginn daemon at rudp://{endpoint} rejected the request: {code}: {detail}")
            }
            Self::TooLarge { bytes, limit } => {
                write!(f, "the request encodes to {bytes} bytes and one send carries at most {limit}")
            }
        }
    }
}

impl std::error::Error for ClientError {}

/// What `exchange` can fail with, before the endpoint is attached.
enum Failure {
    Transport(anyhow::Error),
    Rejected(OperationFailure),
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        Self::Transport(error)
    }
}

/// A daemon's address, the instance the caller acts for, and the time one whole
/// call may take: the connection, the answer, and every chunk of a deferred
/// body together must finish within `timeout` of the call starting.
#[derive(Clone, Debug)]
pub struct HuginnClient {
    endpoint: SocketAddr,
    instance: Slug,
    timeout: Duration,
}

impl HuginnClient {
    pub fn new(endpoint: SocketAddr, instance: Slug, timeout: Duration) -> Self {
        Self { endpoint, instance, timeout }
    }

    /// The instance this client acts for. Requests name their own instance;
    /// this is the one the caller's tools fill in.
    pub fn instance(&self) -> &Slug {
        &self.instance
    }

    pub fn call(&self, request: HuginnMindRequest) -> Result<HuginnMindResponse, ClientError> {
        let deadline = Instant::now() + self.timeout;
        let unavailable =
            |error: anyhow::Error| ClientError::Unavailable { endpoint: self.endpoint, detail: format!("{error:#}") };
        let message = encode_request(MESSAGE_ID, &request, None).map_err(unavailable)?;
        let bytes = encode_cultnet_message_to_vec(&message, CultNetWireContract::CultNetSchemaV0)
            .map_err(unavailable)?
            .len();
        if bytes > MAX_REQUEST_BYTES {
            return Err(ClientError::TooLarge { bytes, limit: MAX_REQUEST_BYTES });
        }
        let mut session = self.connect(deadline).map_err(unavailable)?;
        let outcome = self.exchange(&mut session, &request, &message, deadline);
        // Best effort: the daemon drops a session it never hears from, only later.
        let _ = session.disconnect(Vec::new());
        outcome.map_err(|failure| match failure {
            Failure::Transport(error) => unavailable(error),
            Failure::Rejected(rejection) => ClientError::Rejected {
                endpoint: self.endpoint,
                code: rejection.code,
                detail: rejection.message,
            },
        })
    }

    fn exchange(
        &self,
        session: &mut CultNetRudpSocketTransportConnection,
        request: &HuginnMindRequest,
        message: &CultNetMessage,
        deadline: Instant,
    ) -> Result<HuginnMindResponse, Failure> {
        let reply = self.ask(session, message, deadline)?;
        let CultNetMessage::OperationResponse { operation, .. } = &reply else {
            return Err(anyhow!("the daemon answered with something other than an operation response").into());
        };
        if operation != request.operation() {
            return Err(anyhow!("the daemon answered {operation}, not the {} that was asked", request.operation()).into());
        }
        let (message_id, answer) = decode_response(&reply)?;
        if message_id != MESSAGE_ID {
            return Err(anyhow!("the daemon answered request {message_id}, not {MESSAGE_ID}").into());
        }
        match answer.map_err(Failure::Rejected)? {
            HuginnMindResponse::Deferred(deferred) => {
                let body = fetch_content(&deferred.manifest, MAX_DEFERRED_BODY_BYTES, |chunk| {
                    self.ask(session, &chunk, deadline)
                })?;
                match rmp_serde::from_slice::<HuginnMindResponse>(&body).context("the deferred body is not a response")? {
                    HuginnMindResponse::Deferred(_) => Err(anyhow!("a deferred body is itself deferred").into()),
                    answered => Ok(answered),
                }
            }
            answered => Ok(answered),
        }
    }

    fn connect(&self, deadline: Instant) -> Result<CultNetRudpSocketTransportConnection> {
        let mut session = CultMesh::create_rudp_client_for_endpoint(
            format!("eureka-state-{}", self.instance.0),
            CULTNET_OPERATION_CONNECTION_ID,
            &format!("rudp://{}", self.endpoint),
            CultMeshRudpSocketOptions::default(),
        )?;
        session.connect(Vec::new())?;
        loop {
            session.poll_resends()?;
            let _ = session.receive_once()?;
            if session.connected() {
                return Ok(session);
            }
            if Instant::now() >= deadline {
                bail!("no accept before the call's {:?} deadline", self.timeout);
            }
            std::thread::sleep(POLL);
        }
    }

    /// One message on the session and the next message back, before the call's
    /// `deadline`.
    fn ask(
        &self,
        session: &mut CultNetRudpSocketTransportConnection,
        message: &CultNetMessage,
        deadline: Instant,
    ) -> Result<CultNetMessage> {
        session.send_schema_message(message)?;
        loop {
            session.poll_resends()?;
            if let Some(reply) = session.receive_schema_message_once()? {
                return Ok(reply);
            }
            if Instant::now() >= deadline {
                bail!("no answer before the call's {:?} deadline", self.timeout);
            }
            std::thread::sleep(POLL);
        }
    }
}
