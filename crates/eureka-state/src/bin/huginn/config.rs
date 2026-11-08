//! The CLI's configuration: the mind is the session's environment, never an
//! argument. `EUREKA_INSTANCE` names the instance and `HUGINN_ENDPOINT`
//! (`rudp://<ip>:<port>`) the daemon that holds it.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use eureka_state::HuginnClient;
use huginn_mind::eureka_pipeline::Slug;
use huginn_mind::{HuginnMindRequest, HuginnMindResponse};

use crate::trouble::Trouble;

/// The `provenance.tool` every admission from this binary carries.
pub const TOOL: &str = "huginn";
/// How long one call may take, connection to last byte.
const TIMEOUT: Duration = Duration::from_secs(15);
/// What `ask_until` leaves of its deadline for the caller to print and exit.
const RESERVE: Duration = Duration::from_millis(100);

/// The two environment values as given, and the client they make when both
/// are valid.
pub struct Settings {
    pub instance: Option<String>,
    pub endpoint: Option<String>,
    client: Result<HuginnClient, String>,
}

impl Settings {
    pub fn from_env() -> Self {
        let instance = variable("EUREKA_INSTANCE");
        let endpoint = variable("HUGINN_ENDPOINT");
        let shown = |value: &Result<Option<String>, String>| value.as_ref().ok().cloned().flatten();
        let (shown_instance, shown_endpoint) = (shown(&instance), shown(&endpoint));
        Self { instance: shown_instance, endpoint: shown_endpoint, client: client(instance, endpoint) }
    }

    pub fn client(&self) -> Result<&HuginnClient, Trouble> {
        self.client.as_ref().map_err(|detail| Trouble::Misconfigured(detail.clone()))
    }

    /// A client for another mind, read from: `instance` at `endpoint` (`rudp://<ip>:<port>`), both
    /// given as arguments. It refuses this process's own instance before any call, so the one
    /// mind a verb writes is never also its source. Every request through it declares `instance`,
    /// so an endpoint that holds a different mind answers ForeignInstance.
    pub fn source(&self, instance: &str, endpoint: &str) -> Result<HuginnClient, Trouble> {
        let slug = Slug(instance.to_string());
        slug.validate_slug().map_err(|refusal| Trouble::invalid(format!("--from is not a valid instance name: {refusal:?}")))?;
        if self.instance.as_deref() == Some(instance) {
            return Err(Trouble::invalid(format!("--from {instance} is this process's own instance (EUREKA_INSTANCE)")));
        }
        let address = address(endpoint).ok_or_else(|| Trouble::invalid(format!("--from-endpoint is not rudp://<ip>:<port>: {endpoint}")))?;
        Ok(HuginnClient::new(address, slug, TIMEOUT))
    }

    /// One wire call for the configured instance. The request is built from
    /// that instance, never from anything the caller sent. No retry.
    pub fn ask(&self, request: impl FnOnce(&Slug) -> HuginnMindRequest) -> Result<HuginnMindResponse, Trouble> {
        let client = self.client()?;
        client.call(request(client.instance())).map_err(Trouble::from)
    }

    /// `ask` bounded by a deadline instead of the fixed call time: the call
    /// may take what is left of `deadline` less a short reserve, and is
    /// `Unavailable` without being sent when nothing is left.
    pub fn ask_until(
        &self,
        deadline: Instant,
        request: impl FnOnce(&Slug) -> HuginnMindRequest,
    ) -> Result<HuginnMindResponse, Trouble> {
        let client = self.client()?;
        let left = deadline.saturating_duration_since(Instant::now()).saturating_sub(RESERVE);
        if left.is_zero() {
            return Err(Trouble::Unavailable("the deadline has no time left for a call".to_string()));
        }
        let client = client.with_timeout(left);
        client.call(request(client.instance())).map_err(Trouble::from)
    }
}

/// An environment variable: its value, `None` when unset, or why it cannot be read.
fn variable(name: &str) -> Result<Option<String>, String> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

fn client(instance: Result<Option<String>, String>, endpoint: Result<Option<String>, String>) -> Result<HuginnClient, String> {
    let instance = instance?.ok_or("EUREKA_INSTANCE is not set")?;
    let slug = Slug(instance);
    slug.validate_slug().map_err(|refusal| format!("EUREKA_INSTANCE is not a valid instance name: {refusal:?}"))?;
    let endpoint = endpoint?.ok_or("HUGINN_ENDPOINT is not set")?;
    let address = address(&endpoint).ok_or_else(|| format!("HUGINN_ENDPOINT is not rudp://<ip>:<port>: {endpoint}"))?;
    Ok(HuginnClient::new(address, slug, TIMEOUT))
}

/// The socket address of a `rudp://<ip>:<port>` endpoint.
fn address(endpoint: &str) -> Option<SocketAddr> {
    endpoint.strip_prefix("rudp://").and_then(|address| address.parse::<SocketAddr>().ok())
}
