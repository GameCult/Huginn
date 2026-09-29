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
//! deferred, and drops the session. Nothing is kept between calls.

use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, CultNetMessage,
    CultNetRudpSocketTransportConnection, fetch_content,
};
use huginn_mind::envelope::{decode_response, encode_request};
use huginn_mind::epiphany_pipeline::Slug;
use huginn_mind::{HuginnMindRequest, HuginnMindResponse, MAX_DEFERRED_BODY_BYTES};

/// The correlation key of the one request a session carries.
const MESSAGE_ID: &str = "eureka-state-call";
const POLL: Duration = Duration::from_millis(2);

/// The daemon could not be reached, went quiet, or did not answer as the
/// protocol says it must. `detail` says which.
#[derive(Debug)]
pub enum ClientError {
    Unavailable { endpoint: SocketAddr, detail: String },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Unavailable { endpoint, detail } = self;
        write!(f, "the Huginn daemon at rudp://{endpoint} is unavailable: {detail}")
    }
}

impl std::error::Error for ClientError {}

/// A daemon's address, the instance the caller acts for, and how long any one
/// wait may last: the connection, the answer, and each chunk of a deferred
/// body each get `timeout`.
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
        self.exchange(&request)
            .map_err(|error| ClientError::Unavailable { endpoint: self.endpoint, detail: format!("{error:#}") })
    }

    fn exchange(&self, request: &HuginnMindRequest) -> Result<HuginnMindResponse> {
        let mut session = self.connect()?;
        let reply = self.ask(&mut session, &encode_request(MESSAGE_ID, request, None)?)?;
        let (_, answer) = decode_response(&reply)?;
        match answer.map_err(|failure| anyhow!("the daemon rejected the envelope: {}: {}", failure.code, failure.message))? {
            HuginnMindResponse::Deferred(deferred) => {
                let body = fetch_content(&deferred.manifest, MAX_DEFERRED_BODY_BYTES, |chunk| {
                    self.ask(&mut session, &chunk)
                })?;
                match rmp_serde::from_slice::<HuginnMindResponse>(&body).context("the deferred body is not a response")? {
                    HuginnMindResponse::Deferred(_) => bail!("a deferred body is itself deferred"),
                    answered => Ok(answered),
                }
            }
            answered => Ok(answered),
        }
    }

    fn connect(&self) -> Result<CultNetRudpSocketTransportConnection> {
        let mut session = CultMesh::create_rudp_client_for_endpoint(
            format!("eureka-state-{}", self.instance.0),
            CULTNET_OPERATION_CONNECTION_ID,
            &format!("rudp://{}", self.endpoint),
            CultMeshRudpSocketOptions::default(),
        )?;
        session.connect(Vec::new())?;
        let deadline = Instant::now() + self.timeout;
        loop {
            session.poll_resends()?;
            let _ = session.receive_once()?;
            if session.connected() {
                return Ok(session);
            }
            if Instant::now() >= deadline {
                bail!("no accept within {:?}", self.timeout);
            }
            std::thread::sleep(POLL);
        }
    }

    /// One message on the session and the next message back, within `timeout`.
    fn ask(&self, session: &mut CultNetRudpSocketTransportConnection, message: &CultNetMessage) -> Result<CultNetMessage> {
        session.send_schema_message(message)?;
        let deadline = Instant::now() + self.timeout;
        loop {
            session.poll_resends()?;
            if let Some(reply) = session.receive_schema_message_once()? {
                return Ok(reply);
            }
            if Instant::now() >= deadline {
                bail!("no answer within {:?}", self.timeout);
            }
            std::thread::sleep(POLL);
        }
    }
}
