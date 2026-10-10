//! `eureka-state`: Huginn's mind as six MCP tools over stdio.
//!
//! One tool per wire operation (`whoami`, `admit`, `view`, `query`,
//! `persona_get`, `persona_put`), each a `HuginnClient` call and nothing else. The declared identity is this
//! process's configuration, not a per-call argument: `EUREKA_INSTANCE` names
//! the mind and `HUGINN_ENDPOINT` (`rudp://host:port`) the daemon that holds
//! it. The server fills in `instance` and `provenance.tool`, so a caller can
//! name neither.
//!
//! Bad configuration does not stop the server: an MCP server that dies at
//! startup vanishes from Claude Code without a word. `whoami` reports it and
//! every other tool refuses with it. A refusal from the mind is an answer and
//! comes back as a normal result; a daemon that cannot be reached or that
//! rejects the envelope, or a request too large for one send, is a result with
//! `isError: true`, typed, so a caller can tell a down daemon from a permanent
//! mismatch. Arguments that do not fit a tool's input are typed the same way
//! (`InvalidInput`), as a tool error and not a protocol one.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cultnet_rs::Selection;
use eureka_state::{ClientError, HuginnClient};
use huginn_mind::eureka_pipeline::{PipelineDocument, PipelineRef, Short, Slug};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, PipelineAdmissionBatch, PipelineProvenance, SemanticQuery,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;
use serde_json::{Value, json};

/// The `provenance.tool` every admission from this server carries.
const TOOL: &str = "eureka-state";
/// How long one call may take, connection to last byte.
const TIMEOUT: Duration = Duration::from_secs(15);

/// CultLib's published shape of a selection, vendored because the selection
/// type carries no schema of its own. `selection_schema_matches_cultlib` pins
/// it to the CultLib revision `cultnet-rs` is built from.
const SELECTION_SCHEMA: &str = include_str!("../schemas/cultnet.selection.schema.json");

/// The selection's schema, self-contained: its own `$defs` move into the
/// tool's, so every `$ref` in the input schema is `#/$defs/...`.
fn selection_schema(generator: &mut SchemaGenerator) -> Schema {
    let Ok(Value::Object(mut selection)) = serde_json::from_str(SELECTION_SCHEMA) else {
        unreachable!("the vendored selection schema is a JSON object");
    };
    selection.remove("$schema");
    selection.remove("$id");
    if let Some(Value::Object(defs)) = selection.remove("$defs") {
        generator.definitions_mut().extend(defs);
    }
    Schema::try_from(Value::Object(selection)).expect("an object is a schema")
}

#[derive(Deserialize, JsonSchema)]
struct AdmitInput {
    /// The faculty the batch is admitted as. Attribution, not authority.
    faculty: Faculty,
    /// The admitting agent's name.
    agent: Short,
    /// The admitting session, recorded on the receipt so that several sessions
    /// sharing one mind stay tellable apart.
    session: Short,
    /// The documents to admit as one batch. The mind admits them whole or refuses them.
    documents: Vec<PipelineDocument>,
}

#[derive(Deserialize, JsonSchema)]
struct PersonaPutInput {
    /// The faculty the put is attributed to. Attribution, not authority.
    faculty: Faculty,
    /// The putting agent's name.
    agent: Short,
    /// The putting session, recorded on the receipt.
    session: Short,
    /// The whole `gamecult.persona_state.v0` document. It replaces the stored
    /// one; there is no partial write.
    state: Value,
    /// The stored document's `updatedAt`; omit it for the first put.
    expected_updated_at: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ViewInput {
    /// The document's id, as a query or an admission returned it.
    id: PipelineRef,
}

#[derive(Deserialize, JsonSchema)]
struct QueryInput {
    /// A CultNet typed selection over the mind's documents.
    #[schemars(schema_with = "selection_schema")]
    selection: Selection,
    /// Optional: rank by nearness to this text instead of ordering by key.
    semantic: Option<SemanticQuery>,
}

/// Why a call produced no answer. Each is a typed body with `isError: true`.
enum Trouble {
    Misconfigured(String),
    Client(ClientError),
    Internal(String),
}

impl Trouble {
    fn body(&self) -> Value {
        match self {
            Self::Misconfigured(detail) => json!({ "error": "Misconfigured", "detail": detail }),
            Self::Client(ClientError::Unavailable { endpoint, detail }) => {
                json!({ "error": "Unavailable", "endpoint": format!("rudp://{endpoint}"), "detail": detail })
            }
            Self::Client(ClientError::Rejected { endpoint, code, detail }) => {
                json!({ "error": "Rejected", "endpoint": format!("rudp://{endpoint}"), "code": code, "detail": detail })
            }
            Self::Client(ClientError::TooLarge { bytes, limit }) => {
                json!({ "error": "TooLarge", "bytes": bytes, "limit": limit })
            }
            Self::Client(ClientError::Unencodable { detail }) => json!({ "error": "Unencodable", "detail": detail }),
            Self::Internal(detail) => json!({ "error": "Internal", "detail": detail }),
        }
    }
}

/// The two environment values as given, and the client they make when both are
/// valid.
struct Settings {
    instance: Option<String>,
    endpoint: Option<String>,
    client: Result<HuginnClient, String>,
}

impl Settings {
    fn from_env() -> Self {
        let instance = variable("EUREKA_INSTANCE");
        let endpoint = variable("HUGINN_ENDPOINT");
        let shown = |value: &Result<Option<String>, String>| value.as_ref().ok().cloned().flatten();
        let (shown_instance, shown_endpoint) = (shown(&instance), shown(&endpoint));
        Self { instance: shown_instance, endpoint: shown_endpoint, client: client(instance, endpoint) }
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
    let address = endpoint
        .strip_prefix("rudp://")
        .and_then(|address| address.parse::<SocketAddr>().ok())
        .ok_or_else(|| format!("HUGINN_ENDPOINT is not rudp://<ip>:<port>: {endpoint}"))?;
    Ok(HuginnClient::new(address, slug, TIMEOUT))
}

#[derive(Clone)]
struct EurekaState {
    settings: Arc<Settings>,
    tool_router: ToolRouter<Self>,
}

impl EurekaState {
    fn new(settings: Settings) -> Self {
        Self { settings: Arc::new(settings), tool_router: Self::tool_router() }
    }

    /// One wire call for the configured instance. The request is built from
    /// that instance, never from anything the caller sent.
    async fn call(
        &self,
        request: impl FnOnce(Slug) -> HuginnMindRequest + Send + 'static,
    ) -> Result<HuginnMindResponse, Trouble> {
        self.ask(move |client| client.call(request(client.instance().clone()))).await
    }

    /// One `HuginnClient` call for the configured instance, off the async
    /// thread.
    async fn ask(
        &self,
        call: impl FnOnce(&HuginnClient) -> Result<HuginnMindResponse, ClientError> + Send + 'static,
    ) -> Result<HuginnMindResponse, Trouble> {
        let client = self.settings.client.as_ref().map_err(|detail| Trouble::Misconfigured(detail.clone()))?.clone();
        tokio::task::spawn_blocking(move || call(&client))
            .await
            .map_err(|error| Trouble::Internal(error.to_string()))?
            .map_err(Trouble::Client)
    }

    async fn answer(&self, request: impl FnOnce(Slug) -> HuginnMindRequest + Send + 'static) -> CallToolResult {
        Self::replied(self.call(request).await)
    }

    fn replied(result: Result<HuginnMindResponse, Trouble>) -> CallToolResult {
        match result {
            Ok(response) => reply(payload(response), false),
            Err(trouble) => reply(trouble.body(), true),
        }
    }
}

/// The answer as the tool's output: the operation's own payload, or the typed
/// refusal, tagged, when the mind refused the request.
fn payload(response: HuginnMindResponse) -> Value {
    let value = match response {
        HuginnMindResponse::Whoami(status) => serde_json::to_value(status),
        HuginnMindResponse::Admit(outcome) => serde_json::to_value(outcome),
        HuginnMindResponse::View(view) => serde_json::to_value(view),
        HuginnMindResponse::Query(page) => serde_json::to_value(page),
        HuginnMindResponse::Persona(view) => serde_json::to_value(view),
        HuginnMindResponse::PersonaPut(outcome) => serde_json::to_value(outcome),
        other => serde_json::to_value(other),
    };
    value.expect("wire types serialize")
}

fn reply(body: Value, is_error: bool) -> CallToolResult {
    let content = vec![ContentBlock::text(body.to_string())];
    if is_error { CallToolResult::error(content) } else { CallToolResult::success(content) }
}

#[tool_router]
impl EurekaState {
    #[tool(
        description = "Who this server acts for and whether the Huginn daemon answers: the configured instance and endpoint, reachable, and the mind's status when it does. Reports bad configuration instead of failing."
    )]
    async fn whoami(&self) -> CallToolResult {
        let mut out = json!({
            "instance": self.settings.instance,
            "endpoint": self.settings.endpoint,
            "reachable": false,
            "status": null,
        });
        match self.call(|_| HuginnMindRequest::Whoami).await {
            Ok(HuginnMindResponse::Whoami(status)) => {
                out["reachable"] = json!(true);
                out["status"] = serde_json::to_value(status).expect("wire types serialize");
            }
            Ok(other) => out["error"] = json!({ "error": "Internal", "detail": format!("unexpected answer: {other:?}") }),
            Err(trouble) => out["error"] = trouble.body(),
        }
        reply(out, false)
    }

    #[tool(
        description = "Admit one batch of pipeline documents into the mind, whole or not at all. Returns the outcome: committed with a receipt, already admitted, conflict, or a typed refusal. The instance is this server's configuration; do not pass one."
    )]
    async fn admit(&self, Parameters(input): Parameters<AdmitInput>) -> CallToolResult {
        self.answer(|instance| {
            HuginnMindRequest::Admit(PipelineAdmissionBatch {
                instance,
                provenance: PipelineProvenance {
                    faculty: input.faculty,
                    agent: input.agent,
                    session: input.session,
                    tool: Short(TOOL.into()),
                },
                documents: input.documents,
            })
        })
        .await
    }

    #[tool(description = "Read one document by id, with its admission facts and status. null when the mind has no such document.")]
    async fn view(&self, Parameters(input): Parameters<ViewInput>) -> CallToolResult {
        self.answer(|instance| HuginnMindRequest::View { instance, id: input.id }).await
    }

    #[tool(
        description = "Query the mind with a CultNet typed selection, optionally ranked by semantic nearness. Returns one page; pass its cursor back in the selection for the next."
    )]
    async fn query(&self, Parameters(input): Parameters<QueryInput>) -> CallToolResult {
        self.answer(|instance| HuginnMindRequest::Query {
            instance,
            selection: input.selection,
            semantic: input.semantic,
        })
        .await
    }

    #[tool(
        description = "The mind's persona document (gamecult.persona_state.v0) with its updatedAt and the receipt of the put that wrote it. null until the first put."
    )]
    async fn persona_get(&self) -> CallToolResult {
        Self::replied(self.ask(HuginnClient::persona_get).await)
    }

    #[tool(
        description = "Replace the mind's persona document whole. The document must pass the published schema, name this instance as personaId, carry provenance.authority canonical, and have an updatedAt later than the stored one; expected_updated_at must be the stored updatedAt (omit it for the first put). A refusal is an answer; a stale put names the stored updatedAt to retry against."
    )]
    async fn persona_put(&self, Parameters(input): Parameters<PersonaPutInput>) -> CallToolResult {
        let provenance = PipelineProvenance {
            faculty: input.faculty,
            agent: input.agent,
            session: input.session,
            tool: Short(TOOL.into()),
        };
        Self::replied(
            self.ask(move |client| client.persona_put(provenance, input.state, input.expected_updated_at)).await,
        )
    }
}

/// Every tool error is a typed JSON body. The ones this server writes already
/// are; the one rmcp writes, for arguments that do not deserialize into the
/// tool's input, is plain text, so it is put into the same shape.
fn typed_error(result: CallToolResult) -> CallToolResult {
    if result.is_error != Some(true) {
        return result;
    }
    let Some(text) = result.content.first().and_then(ContentBlock::as_text).map(|text| text.text.as_str()) else {
        return result;
    };
    if serde_json::from_str::<Value>(text).is_ok_and(|body| body.is_object()) {
        return result;
    }
    reply(json!({ "error": "InvalidInput", "detail": text }), true)
}

#[tool_handler]
impl ServerHandler for EurekaState {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(typed_error(self.tool_router.call(ToolCallContext::new(self, request, context)).await?))
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Huginn's typed mind for this instance: whoami, admit, view, query, persona_get, persona_put. Refusals are answers; isError means the daemon could not be reached or rejected the request.",
        )
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let server = EurekaState::new(Settings::from_env()).serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_of(result: &CallToolResult) -> (bool, String) {
        (result.is_error == Some(true), result.content[0].as_text().unwrap().text.clone())
    }

    /// Reaching `Internal` for real takes a panic in a blocking task, so its
    /// body is pinned where it is built.
    #[test]
    fn every_trouble_has_its_typed_body() {
        assert_eq!(Trouble::Internal("task panicked".into()).body(), json!({ "error": "Internal", "detail": "task panicked" }));
        assert_eq!(
            Trouble::Client(ClientError::TooLarge { bytes: 70_000, limit: 65_000 }).body(),
            json!({ "error": "TooLarge", "bytes": 70_000, "limit": 65_000 })
        );
        assert_eq!(
            Trouble::Client(ClientError::Unencodable { detail: "no".into() }).body(),
            json!({ "error": "Unencodable", "detail": "no" })
        );
        assert_eq!(Trouble::Misconfigured("bad".into()).body(), json!({ "error": "Misconfigured", "detail": "bad" }));
    }

    /// Only the text rmcp writes is rewritten: our own typed bodies and every
    /// success pass through untouched.
    #[test]
    fn typed_error_rewrites_only_rmcp_plain_text() {
        let plain = CallToolResult::error(vec![ContentBlock::text("failed to deserialize parameters: missing field `faculty`")]);
        let (is_error, text) = body_of(&typed_error(plain));
        assert!(is_error);
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            json!({ "error": "InvalidInput", "detail": "failed to deserialize parameters: missing field `faculty`" })
        );

        let typed = reply(json!({ "error": "Unavailable", "detail": "down" }), true);
        assert_eq!(body_of(&typed_error(typed.clone())), body_of(&typed));

        let success = CallToolResult::success(vec![ContentBlock::text("not json, and fine")]);
        assert_eq!(body_of(&typed_error(success)), (false, "not json, and fine".to_string()));

        let quoted = CallToolResult::error(vec![ContentBlock::text("[1, 2]")]);
        assert_eq!(serde_json::from_str::<Value>(&body_of(&typed_error(quoted)).1).unwrap()["error"], json!("InvalidInput"));
    }
}
