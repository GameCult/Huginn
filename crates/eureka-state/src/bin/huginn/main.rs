//! `huginn`: the agents' door to Huginn's mind, as shell commands.
//!
//! A caller of `HuginnClient` and nothing else. It owns argument parsing, the
//! strict decode of agent-written JSON into the leaf's types (`door`), the
//! recipe selections (`recipes`), the text rendering (`render`), the exit code
//! and the read-back after an admission. It owns no state, writes no file and
//! reads no configuration but two environment variables: `EUREKA_INSTANCE`
//! names the mind and `HUGINN_ENDPOINT` (`rudp://host:port`) the daemon that
//! holds it. Each call is a new process; there is no retry inside it, the
//! caller's rule retries `Unavailable` once.
//!
//! Answers go to stdout and the exit code says what the answer was: 0 answered,
//! 1 the mind said no (a refusal, a conflict, an absent document). Troubles go
//! to stderr as `<Code>: <detail>`: 2 the call is wrong, 3 Unavailable, 4 no
//! answer and a retry cannot help, 5 the read-back after a committed admission
//! differs from what was sent.

mod config;
mod door;
mod recipes;
mod render;
mod schema;
mod trouble;

use std::io::Write;
use std::process::ExitCode;
use std::sync::OnceLock;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use cultnet_rs::{PROJECTION_DOCUMENT, Selection};
use huginn_mind::eureka_pipeline::{Line, PipelineDocument, PipelineRef, Short, Slug, pipeline_key};
use huginn_mind::{
    BATCH_MAX, Faculty, HuginnMindRequest, HuginnMindResponse, MindRefusal, PipelineAdmissionBatch, PipelineAdmissionOutcome,
    PipelinePageItems, PipelineProvenance, PipelineSelectionPage, SemanticQuery,
};
use serde_json::Value;

use config::{Settings, TOOL};
use recipes::Recipe;
use trouble::Trouble;

/// The page size when a selection names none.
const DEFAULT_LIMIT: u32 = 50;
/// The neighbours `--near` asks for when `--top-k` is not given.
const DEFAULT_TOP_K: u32 = 10;

const ADMIT_HELP: &str = "\
The file is a JSON array of 1 to 64 documents, each {\"kind\": <kind>, \"value\": {...}}, or - for stdin; one document object alone is a batch of one. \
`huginn schema` lists the kinds and `huginn schema <kind>` prints a value's JSON Schema. \
Every key the schema does not know is refused (exit 2) with its path; nothing is sent. \
After Committed or AlreadyAdmitted the batch is read back and compared with what was sent: `read back <n> equal`, or exit 5 with the ids and field paths that differ.";

#[derive(Parser, Debug)]
#[command(name = "huginn", about = "Huginn's mind: whoami, view, query, recipes, admit, schema", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Who this process acts for and whether the daemon answers.
    Whoami,
    /// Read one document by id, with its admission facts and status.
    View {
        /// The document's id: <root>:<kind>:<local>.
        id: String,
        /// Print only the {kind, value} JSON, which `admit` accepts back.
        #[arg(long)]
        raw: bool,
    },
    /// Query the mind with a CultNet selection.
    Query {
        /// The selection JSON, @<file>, or - for stdin.
        selection: String,
        /// Rank by nearness to this text instead of ordering by key.
        #[arg(long)]
        near: Option<String>,
        /// How many neighbours (default 10).
        #[arg(long, requires = "near")]
        top_k: Option<u32>,
        /// Print each item as `view` does.
        #[arg(long)]
        documents: bool,
    },
    #[command(flatten)]
    Recipe(Recipe),
    /// Admit a batch of documents, whole or not at all, and read it back.
    #[command(after_help = ADMIT_HELP)]
    Admit {
        #[arg(long, value_parser = faculty)]
        faculty: Faculty,
        /// The admitting agent's name.
        #[arg(long)]
        agent: String,
        /// The admitting session.
        #[arg(long)]
        session: String,
        /// The documents file, or - for stdin.
        file: String,
    },
    /// The JSON Schema of a document kind or of a selection; no name lists them.
    Schema { name: Option<String> },
}

fn faculty(text: &str) -> Result<Faculty, String> {
    serde_json::from_value(Value::String(text.to_string())).map_err(|error| error.to_string())
}

/// The version, then the sha the binary was built from.
fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| format!("{} ({})", env!("CARGO_PKG_VERSION"), build_sha()))
}

fn build_sha() -> &'static str {
    option_env!("HUGINN_BUILD_SHA").unwrap_or("unknown")
}

/// The command line, or the exit code of having answered it (help, version) or
/// refused it (usage, as `InvalidInput`).
fn parse() -> Result<Cli, u8> {
    let mut command = Cli::command().version(version());
    for (name, text) in recipes::help() {
        command = command.mut_subcommand(name, |recipe| recipe.after_help(text));
    }
    match command.try_get_matches() {
        Ok(matches) => Cli::from_arg_matches(&matches).map_err(|error| usage(&error)),
        Err(error) => Err(usage(&error)),
    }
}

fn usage(error: &clap::Error) -> u8 {
    if error.use_stderr() {
        let text = error.to_string();
        eprintln!("InvalidInput: {}", text.trim_start_matches("error: ").trim_end());
        2
    } else {
        let _ = error.print();
        0
    }
}

fn main() -> ExitCode {
    let cli = match parse() {
        Ok(cli) => cli,
        Err(code) => return ExitCode::from(code),
    };
    match run(&Settings::from_env(), cli.command) {
        Ok(code) => ExitCode::from(code),
        Err(trouble) => {
            eprintln!("{trouble}");
            ExitCode::from(trouble.exit())
        }
    }
}

fn say(text: impl std::fmt::Display) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn run(settings: &Settings, command: Command) -> Result<u8, Trouble> {
    match command {
        Command::Whoami => whoami(settings),
        Command::View { id, raw } => view(settings, &id, raw),
        Command::Query { selection, near, top_k, documents } => {
            let selection = door::selection(&door::argument(&selection)?)?;
            let semantic = near.map(|text| SemanticQuery { text: Line(text), top_k: top_k.unwrap_or(DEFAULT_TOP_K) });
            query(settings, selection, semantic, None, documents)
        }
        Command::Recipe(recipe) => {
            let ask = recipe.ask()?;
            query(settings, door::selection_value(&ask.selection)?, ask.semantic, ask.page.cursor, ask.page.documents)
        }
        Command::Admit { faculty, agent, session, file } => admit(settings, faculty, agent, session, &file),
        Command::Schema { name } => {
            let text = schema::text(name.as_deref())?;
            let _ = write!(std::io::stdout(), "{text}");
            Ok(0)
        }
    }
}

fn whoami(settings: &Settings) -> Result<u8, Trouble> {
    say(format_args!("instance {}", settings.instance.as_deref().unwrap_or("unset")));
    say(format_args!("endpoint {}", settings.endpoint.as_deref().unwrap_or("unset")));
    say(format_args!("client {}", build_sha()));
    match settings.ask(|_| HuginnMindRequest::Whoami) {
        Ok(HuginnMindResponse::Whoami(status)) => {
            say("reachable yes");
            say(format_args!(
                "documents {} receipts {} index {} epoch {}",
                status.documents,
                status.receipts,
                render::index_state(&status.index),
                status.schema_epoch
            ));
            say(format_args!("persona {}", render::persona_state(&status.persona)));
            Ok(0)
        }
        Ok(other) => {
            say("reachable no");
            Err(unexpected(&other))
        }
        Err(trouble) => {
            say("reachable no");
            Err(trouble)
        }
    }
}

fn view(settings: &Settings, id: &str, raw: bool) -> Result<u8, Trouble> {
    let reference = PipelineRef { kind: door::kind_of(id)?, id: Short(id.to_string()) };
    match settings.ask(|instance| HuginnMindRequest::View { instance: instance.clone(), id: reference })? {
        HuginnMindResponse::View(Some(found)) => {
            say(render::view(&found, raw));
            Ok(0)
        }
        HuginnMindResponse::View(None) => {
            say(format_args!("absent {id}"));
            Ok(1)
        }
        HuginnMindResponse::Refused(refusal) => Ok(refused(&refusal)),
        other => Err(unexpected(&other)),
    }
}

fn query(
    settings: &Settings,
    mut selection: Selection,
    semantic: Option<SemanticQuery>,
    cursor: Option<String>,
    documents: bool,
) -> Result<u8, Trouble> {
    if documents {
        selection.projection = PROJECTION_DOCUMENT.to_string();
    }
    if cursor.is_some() {
        selection.cursor = cursor;
    }
    selection.limit.get_or_insert(DEFAULT_LIMIT);
    match settings.ask(|instance| HuginnMindRequest::Query { instance: instance.clone(), selection, semantic })? {
        HuginnMindResponse::Query(page) => {
            show(&page);
            Ok(0)
        }
        HuginnMindResponse::Refused(refusal) => Ok(refused(&refusal)),
        other => Err(unexpected(&other)),
    }
}

fn show(page: &PipelineSelectionPage) {
    match &page.items {
        PipelinePageItems::Headers(headers) => {
            for header in headers {
                say(render::header(header));
            }
        }
        PipelinePageItems::Documents(views) => {
            for (index, found) in views.iter().enumerate() {
                if index > 0 {
                    say("");
                }
                say(render::view(found, false));
            }
        }
    }
    say(format_args!("matched {} as_of {} next {}", page.matched, page.as_of, page.next.as_deref().unwrap_or("-")));
}

fn admit(settings: &Settings, faculty: Faculty, agent: String, session: String, file: &str) -> Result<u8, Trouble> {
    let documents = door::documents(&door::file_or_stdin(file)?)?;
    if documents.is_empty() || documents.len() > BATCH_MAX {
        return Err(Trouble::invalid(format!("a batch is 1 to {BATCH_MAX} documents, this one has {}", documents.len())));
    }
    let sent = documents.clone();
    let provenance = PipelineProvenance { faculty, agent: Short(agent), session: Short(session), tool: Short(TOOL.to_string()) };
    let request = |instance: &Slug| {
        HuginnMindRequest::Admit(PipelineAdmissionBatch { instance: instance.clone(), provenance, documents })
    };
    match settings.ask(request)? {
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { receipt_id, committed_at, writes }) => {
            say(format_args!("Committed {receipt_id} at {committed_at}"));
            for write in &writes {
                say(format_args!("wrote {}", write.id.0));
            }
            read_back(settings, &sent)
        }
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id }) => {
            say(format_args!("AlreadyAdmitted {receipt_id}"));
            read_back(settings, &sent)
        }
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal)) | HuginnMindResponse::Refused(refusal) => {
            Ok(refused(&refusal))
        }
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict { identities }) => {
            say("Conflict");
            for identity in &identities {
                say(format_args!("conflicts {}", identity.id.0));
            }
            Ok(1)
        }
        other => Err(unexpected(&other)),
    }
}

/// The end-to-end check that the mind kept what was sent: one `keys` query,
/// each stored document compared with the one sent. A difference is exit 5,
/// not silence, whatever dropped it (an older leaf, a serde default, skew).
fn read_back(settings: &Settings, sent: &[PipelineDocument]) -> Result<u8, Trouble> {
    let keys = sent
        .iter()
        .map(|document| pipeline_key(document).map_err(|refusal| Trouble::Internal(format!("a committed document has no key: {refusal:?}"))))
        .collect::<Result<Vec<_>, _>>()?;
    let selection = Selection {
        keys: Some(keys.clone()),
        projection: PROJECTION_DOCUMENT.to_string(),
        limit: Some(u32::try_from(keys.len()).unwrap_or(u32::MAX)),
        ..Selection::default()
    };
    let page = match settings.ask(|instance| HuginnMindRequest::Query { instance: instance.clone(), selection, semantic: None })? {
        HuginnMindResponse::Query(page) => page,
        HuginnMindResponse::Refused(refusal) => return Ok(refused(&refusal)),
        other => return Err(unexpected(&other)),
    };
    let PipelinePageItems::Documents(stored) = page.items else {
        return Err(Trouble::Internal("the read-back answered headers, not documents".to_string()));
    };
    let mut differing = Vec::new();
    for (key, document) in keys.iter().zip(sent) {
        let held = stored.iter().find(|found| &found.id.id.0 == key);
        match held {
            None => differing.push(format!("{key}: missing from the read-back")),
            Some(found) if found.document == *document => {}
            Some(found) => {
                let (mut paths, sent_value, held_value) = (Vec::new(), to_value(document), to_value(&found.document));
                differences(&sent_value, &held_value, "", &mut paths);
                let shown = if paths.is_empty() { "the document".to_string() } else { paths.join(", ") };
                differing.push(format!("{key}: {shown}"));
            }
        }
    }
    if differing.is_empty() {
        say(format_args!("read back {} equal", sent.len()));
        Ok(0)
    } else {
        Err(Trouble::ReadBackDiffers(format!(
            "{}; the batch is committed, so supersede the documents that differ",
            differing.join("; ")
        )))
    }
}

fn to_value(document: &PipelineDocument) -> Value {
    serde_json::to_value(document).unwrap_or(Value::Null)
}

/// The field paths at which two documents differ, including a field only one
/// of them carries.
fn differences(sent: &Value, held: &Value, path: &str, out: &mut Vec<String>) {
    match (sent, held) {
        (Value::Object(sent), Value::Object(held)) => {
            for key in sent.keys().chain(held.keys().filter(|key| !sent.contains_key(*key))) {
                let here = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                match (sent.get(key), held.get(key)) {
                    (Some(sent), Some(held)) => differences(sent, held, &here, out),
                    _ => out.push(here),
                }
            }
        }
        (Value::Array(sent), Value::Array(held)) if sent.len() == held.len() => {
            for (index, (sent, held)) in sent.iter().zip(held).enumerate() {
                differences(sent, held, &format!("{path}[{index}]"), out);
            }
        }
        _ if sent == held => {}
        _ => out.push(path.to_string()),
    }
}

/// A refusal is the mind's answer: stdout, exit 1.
fn refused(refusal: &MindRefusal) -> u8 {
    say(format_args!("Refused {refusal:?}"));
    1
}

fn unexpected(answer: &HuginnMindResponse) -> Trouble {
    let name = match answer {
        HuginnMindResponse::Whoami(_) => "whoami",
        HuginnMindResponse::Admit(_) => "admit",
        HuginnMindResponse::View(_) => "view",
        HuginnMindResponse::Query(_) => "query",
        HuginnMindResponse::Persona(_) => "a persona",
        HuginnMindResponse::PersonaPut(_) => "a persona put",
        HuginnMindResponse::Refused(_) => "a refusal",
        HuginnMindResponse::Deferred(_) => "a deferral",
    };
    Trouble::Internal(format!("the daemon answered with {name}, which this call did not ask for"))
}
