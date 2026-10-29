//! The Persona organ's model transport: ModelConnector's kit client.
//!
//! `KitConnector` is the `Connector` port over `model_connector::ConnectorClient`,
//! the contract-v3 client of the connector daemon (CultNet operation frames,
//! AES-GCM sealed, one request per connection). A turn is one tool-less
//! `ProviderRequest` for the instance as caller, its output constrained to the
//! turn's schema, and its answer the assistant text of a `Completed` result; a
//! refusal, a failure or a stop other than the end of the turn is an error. The
//! readout is the kit's spend readout for the same caller.
//!
//! The connector reads its connection key as the file's text without the
//! trailing line break; the same rule applies here (`key`). The endpoint is the
//! connector's loopback TCP listener, `host:port` or `tcp://host:port`.

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use model_connector::{
    ConnectorClient, InputItem, ProviderRequest, StopReason, TransportDisposition, TransportEventPayload, TransportInvocation, TransportOutcome,
    provider_request_sha256,
};

use crate::persona::{Connector, SpendReadout, TurnRequest};

/// The largest frame either side of a connection may carry.
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
/// How long a connection waits for the connector: a turn runs a model.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(600);
/// How long an invocation may wait to be admitted.
const EXPIRES_IN_MS: u64 = 60_000;
/// The longest answer one turn may ask for.
pub const MAX_OUTPUT_TOKENS: u32 = 4096;

/// The connection key as the connector reads it: the file's text without its
/// trailing line break.
pub fn key(key_file_text: &str) -> String {
    key_file_text.trim_end_matches(['\r', '\n']).to_string()
}

/// The connector's address from its configured endpoint: `host:port` or
/// `tcp://host:port`, an IP literal. A `rudp://` endpoint is refused: the
/// connector speaks framed TCP.
pub fn endpoint(text: &str) -> Result<SocketAddr> {
    text.strip_prefix("tcp://").unwrap_or(text).parse::<SocketAddr>().map_err(|_| anyhow!("the connector endpoint is not <ip>:<port> or tcp://<ip>:<port>"))
}

/// `Connector` over the kit's client.
pub struct KitConnector {
    client: ConnectorClient,
    model: String,
    caller: String,
}

impl KitConnector {
    /// `key_text` is the connection key file's text (`key` is applied here);
    /// `caller` is the instance the key belongs to, whose spend the readout asks for.
    pub fn new(endpoint: SocketAddr, key_text: &str, model: impl Into<String>, caller: impl Into<String>) -> Result<Self> {
        let client = ConnectorClient::new(endpoint, key(key_text), MAX_FRAME_BYTES, Some(RESPONSE_TIMEOUT)).map_err(|_| anyhow!("the connector client could not be configured"))?;
        Ok(Self { client, model: model.into(), caller: caller.into() })
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

impl Connector for KitConnector {
    fn turn(&self, request: &TurnRequest) -> Result<String> {
        let mut provider = ProviderRequest::new(request.request_id.as_str(), request.caller.as_str(), self.model.as_str(), request.instructions.as_str(), MAX_OUTPUT_TOKENS);
        provider.input = vec![InputItem::UserText { text: request.input.clone() }];
        provider.output_schema_json = Some(request.output_schema_json.clone());
        // The caller has no provider-native form of its request: the native digest is the request's own.
        let digest = provider_request_sha256(&provider).map_err(|_| anyhow!("the connector request does not digest"))?;
        let invocation = TransportInvocation::new(request.caller.as_str(), now_ms() + EXPIRES_IN_MS, digest, request.max_cost_micro_usd, provider)
            .map_err(|_| anyhow!("the connector invocation is not valid"))?;
        let result = self.client.execute(&invocation).map_err(|error| anyhow!("the connector call failed: {error}"))?;
        match result.disposition {
            TransportDisposition::Refused(reason) => bail!("the connector refused the call: {reason:?}"),
            TransportDisposition::Transported { events, receipt } => {
                match &receipt.outcome {
                    TransportOutcome::Completed { stop_reason: StopReason::EndTurn, .. } => {}
                    TransportOutcome::Completed { stop_reason, .. } => bail!("the connector stopped the turn early: {stop_reason:?}"),
                    TransportOutcome::Failed { kind, .. } => bail!("the connector's provider failed: {kind:?}"),
                }
                let mut text = String::new();
                for event in events {
                    if let TransportEventPayload::TextDelta { text: piece } = event.payload {
                        text.push_str(&piece);
                    }
                }
                Ok(text)
            }
        }
    }

    fn spend_readout(&self) -> Result<SpendReadout> {
        self.client.spend_readout(&self.caller).map_err(|error| anyhow!("the spend readout failed: {error}"))
    }
}
