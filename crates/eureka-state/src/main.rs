//! `eureka-state`: Huginn's mind as four MCP tools over stdio.
//!
//! One tool per wire operation (`whoami`, `admit`, `view`, `query`), each a
//! `HuginnClient` call and nothing else. The declared identity is this
//! process's configuration, not a per-call argument: `EUREKA_INSTANCE` names
//! the mind and `HUGINN_ENDPOINT` (`rudp://host:port`) the daemon that holds
//! it. The server fills in `instance` and `provenance.tool`, so a caller can
//! name neither.
//!
//! Bad configuration does not stop the server: an MCP server that dies at
//! startup vanishes from Claude Code without a word. `whoami` reports it and
//! every other tool refuses with it. A refusal from the mind is an answer and
//! comes back as a normal result; a daemon that cannot be reached or that
//! rejects the envelope is a result with `isError: true`, typed, so a caller
//! can tell a down daemon from a permanent mismatch.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use cultnet_rs::Selection;
use eureka_state::{ClientError, HuginnClient};
use huginn_mind::epiphany_pipeline::{PipelineDocument, PipelineRef, Short, Slug};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, PipelineAdmissionBatch, PipelineProvenance, SemanticQuery,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
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
        let instance = std::env::var("EUREKA_INSTANCE").ok();
        let endpoint = std::env::var("HUGINN_ENDPOINT").ok();
        let client = client(instance.as_deref(), endpoint.as_deref());
        Self { instance, endpoint, client }
    }
}

fn client(instance: Option<&str>, endpoint: Option<&str>) -> Result<HuginnClient, String> {
    let instance = instance.ok_or("EUREKA_INSTANCE is not set")?;
    let slug = Slug(instance.to_string());
    slug.validate_slug().map_err(|refusal| format!("EUREKA_INSTANCE is not a valid instance name: {refusal:?}"))?;
    let endpoint = endpoint.ok_or("HUGINN_ENDPOINT is not set")?;
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
    async fn call(&self, request: impl FnOnce(Slug) -> HuginnMindRequest) -> Result<HuginnMindResponse, Trouble> {
        let client = self.settings.client.as_ref().map_err(|detail| Trouble::Misconfigured(detail.clone()))?.clone();
        let request = request(client.instance().clone());
        tokio::task::spawn_blocking(move || client.call(request))
            .await
            .map_err(|error| Trouble::Internal(error.to_string()))?
            .map_err(Trouble::Client)
    }

    async fn answer(&self, request: impl FnOnce(Slug) -> HuginnMindRequest) -> CallToolResult {
        match self.call(request).await {
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
}

#[tool_handler]
impl ServerHandler for EurekaState {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Huginn's typed mind for this instance: whoami, admit, view, query. Refusals are answers; isError means the daemon could not be reached or rejected the request.",
        )
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let server = EurekaState::new(Settings::from_env()).serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}
