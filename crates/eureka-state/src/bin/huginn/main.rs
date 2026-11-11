//! `huginn`: the agents' door to Huginn's mind, as shell commands.
//!
//! A caller of `HuginnClient` and nothing else. It owns argument parsing, the
//! strict decode of agent-written JSON into the leaf's types (`door`), the
//! recipe selections (`recipes`), the text rendering (`render`), the exit code
//! and the read-back after an admission. It owns no state, writes no file and
//! reads no configuration but these environment variables: `EUREKA_INSTANCE`
//! names the mind and `HUGINN_ENDPOINT` (`rudp://host:port`) the daemon that
//! holds it. The admitting verbs take `--session`, else `HUGINN_SESSION`. Each call is a new process; there is no retry inside it, the
//! caller's rule retries `Unavailable` once.
//!
//! Answers go to stdout and the exit code says what the answer was: 0 answered,
//! 1 the mind said no (a refusal, a conflict, an absent document). Troubles go
//! to stderr as `<Code>: <detail>`: 2 the call is wrong, 3 Unavailable, 4 no
//! answer and a retry cannot help, 5 the read-back after a committed admission
//! differs from what was sent or the verification walk of `import` finds a difference, 6 `admit` held the batch once (`admit_hold`). `import` also
//! exits 1 when the campaign is not moved to this mind or this mind holds records of its own under it, and 4 when the history cannot be replayed whole.

mod admit_hold;
mod config;
mod door;
mod import;
mod near;
mod recall;
mod recipes;
mod render;
mod schema;
mod trouble;

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use cultnet_rs::{PROJECTION_DOCUMENT, Selection};
use eureka_state::hold;
use huginn_mind::eureka_pipeline::{
    Line, PipelineDocument, PipelineRef, Short, Slug, pipeline_key,
};
use huginn_mind::{
    BATCH_MAX, Faculty, HuginnMindRequest, HuginnMindResponse, MindRefusal, PipelineAdmissionBatch, PipelineAdmissionOutcome,
    PersonaPutOutcome, PipelinePageItems, PipelineProvenance, PipelineSelectionPage, SemanticQuery, SourceQuery,
};
use serde_json::Value;

use config::{Settings, TOOL};
use recipes::Recipe;
use trouble::Trouble;

/// The page size when a selection names none.
const DEFAULT_LIMIT: u32 = 50;
/// The whole near-neighbour check of `admit`, all its queries together; it fails open when this runs out.
const HOLD_CHECK: Duration = Duration::from_secs(10);
/// The neighbours `--near` asks for when `--top-k` is not given.
const DEFAULT_TOP_K: u32 = 10;

const ADMIT_HELP: &str = "\
The file is a JSON array of 1 to 64 documents, each {\"kind\": <kind>, \"value\": {...}}, or - for stdin; one document object alone is a batch of one. \
`huginn schema` lists the kinds and `huginn schema <kind>` prints a value's JSON Schema. \
Every key the schema does not know is refused (exit 2) with its path; nothing is sent. \
After Committed or AlreadyAdmitted the batch is read back and compared with what was sent: `read back <n> equal`, or exit 5 with the ids and field paths that differ. A batch whose rulings or questions have in-force neighbours is held once: exit 6, the neighbours printed with a token, nothing sent. Run the same command with --seen <token> to admit it; a batch or --agent that changed needs a new token (a wrong one is exit 2).";

const IMPORT_HELP: &str = "Reads the giver's mind (it sends Query alone) and admits its receipts for the campaign into this one, one batch per source receipt, in order, each naming the giver's receipt in imported_from. It refuses unless the campaign is moved to this mind (exit 1), and when this mind already holds records under the campaign that it did not import from the giver, except follow-ups and ideas it admitted after the import began, which stay in place and are named (exit 1). A rerun after any stop resumes at the first batch this mind does not hold; it prints `imported <campaign> from <instance>: ... verified` only after comparing every document it holds with the giver's (exit 5 if one differs). Until it prints imported, this mind writes nothing else under the campaign; a write that collides stops the import with the refusal (exit 1). Exit 3 Unavailable (rerun), 4 the history cannot be replayed whole (a receipt over 64 documents, too many external referents, a receipt held in part). Only a Self run can run it: a Persona's traffic to a peer's huginn port is fenced.";

#[derive(Parser, Debug)]
#[command(name = "huginn", about = "Huginn's mind: whoami, view, query, recipes, persona, admit, import, recall, schema", arg_required_else_help = true)]
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
    /// Search the public forge repos' code and docs by meaning.
    Sources {
        /// What to find, in words: one to eight texts. Each is a topic; the
        /// ranked lists of several are fused, so a brief with many topics
        /// retrieves by every one.
        #[arg(required = true, num_args = 1..=8)]
        texts: Vec<String>,
        /// Look in this repo alone (its forge name).
        #[arg(long)]
        repo: Option<String>,
        /// Look only in repos' front-page files (README.md, README, readme.md,
        /// README.markdown, CLAUDE.md, AGENTS.md at the repo root).
        #[arg(long)]
        front: bool,
        /// How many chunks (default 10, at most 50).
        #[arg(long)]
        top_k: Option<u32>,
        /// Print the answer as JSON.
        #[arg(long)]
        json: bool,
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
        /// The admitting session; else the environment's HUGINN_SESSION, which a Self run sets to its run.
        #[arg(long)]
        session: Option<String>,
        /// The token of the hold this exact batch got: admit it.
        #[arg(long)]
        seen: Option<String>,
        /// The documents file, or - for stdin.
        file: String,
    },
    /// The JSON Schema of a document kind or of a selection; no name lists them.
    Schema { name: Option<String> },
    /// Replay a moved campaign's history from the giver's mind into this one.
    #[command(after_help = IMPORT_HELP)]
    Import {
        /// The campaign's name: the root of its ids.
        campaign: String,
        /// The giver's instance.
        #[arg(long = "from")]
        from: String,
        /// The giver's daemon, rudp://<ip>:<port>.
        #[arg(long)]
        from_endpoint: String,
        #[arg(long, value_parser = faculty)]
        faculty: Faculty,
        /// The importing agent's name.
        #[arg(long)]
        agent: String,
        /// The importing session; else the environment's HUGINN_SESSION.
        #[arg(long)]
        session: Option<String>,
    },
    /// Recall into a dispatched brief.
    Recall {
        #[command(subcommand)]
        command: RecallCommand,
    },
    /// The mind's persona document (gamecult.persona_state.v0).
    Persona {
        #[command(subcommand)]
        command: PersonaCommand,
    },
}

#[derive(Subcommand, Debug)]
enum RecallCommand {
    /// The Agent PreToolUse hook, installed as `<huginn> recall dispatch || true`: reads Claude Code's hook JSON on stdin and, when the brief cites rulings, questions or follow-ups, prints the hook JSON that appends their in-force neighbours to it. Silent when it has nothing to add; never blocks and never decides a permission, and every failure exits 0, usage errors aside.
    Dispatch {
        /// The neighbours kept per cited record.
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..))]
        top_k: u32,
        /// The cited records looked up (the first this many, in order of appearance).
        #[arg(long, default_value_t = 6, value_parser = clap::value_parser!(u32).range(1..))]
        max_anchors: u32,
        /// The characters appended at most, capped at 9000.
        #[arg(long, default_value_t = 4000, value_parser = clap::value_parser!(u32).range(1..))]
        budget: u32,
        /// The whole run's bound, from start to the last byte written.
        #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..))]
        deadline_ms: u64,
    },
}

#[derive(Subcommand, Debug)]
enum PersonaCommand {
    /// The stored document with its updatedAt and receipt; `absent` (exit 1) until the first put.
    Get,
    /// Replace the persona document whole.
    Put {
        #[arg(long, value_parser = faculty)]
        faculty: Faculty,
        /// The writing agent's name.
        #[arg(long)]
        agent: String,
        /// The writing session; else the environment's HUGINN_SESSION.
        #[arg(long)]
        session: Option<String>,
        /// The stored updatedAt this put replaces; omit for the first put.
        #[arg(long)]
        expected_updated_at: Option<String>,
        /// The whole document file, or - for stdin.
        file: String,
    },
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
        Command::Sources {
            texts,
            repo,
            front,
            top_k,
            json,
        } => sources(settings, texts, repo, front, top_k, json),
        Command::Recipe(Recipe::Blocked { spec_id, page }) => {
            blocked(settings, &spec_id, page.cursor, page.documents)
        }
        Command::Recipe(recipe) => {
            let ask = recipe.ask()?;
            query(settings, door::selection_value(&ask.selection)?, ask.semantic, ask.page.cursor, ask.page.documents)
        }
        Command::Admit { faculty, agent, session, seen, file } => admit(settings, faculty, agent, session, seen, &file),
        Command::Import { campaign, from, from_endpoint, faculty, agent, session } => {
            let session = settings.session(session)?;
            import::run(settings, import::Flags { campaign, from, from_endpoint, faculty, agent, session })
        }
        Command::Persona { command: PersonaCommand::Get } => persona_get(settings),
        Command::Persona { command: PersonaCommand::Put { faculty, agent, session, expected_updated_at, file } } => {
            persona_put(settings, faculty, agent, session, expected_updated_at, &file)
        }
        Command::Recall { command: RecallCommand::Dispatch { top_k, max_anchors, budget, deadline_ms } } => Ok(recall::dispatch(
            settings,
            &recall::Flags { top_k: top_k as usize, max_anchors: max_anchors as usize, budget: budget as usize, deadline_ms },
        )),
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

/// The open questions that hold a spec, and the answered-`retire` ones whose
/// claims are not yet resolved: `eureka_state::hold`'s answer, the one the
/// heartbeat's gate reads, printed as a selection of those questions.
fn blocked(
    settings: &Settings,
    spec_id: &str,
    cursor: Option<String>,
    documents: bool,
) -> Result<u8, Trouble> {
    let spec = PipelineRef {
        kind: door::kind_of(spec_id)?,
        id: Short(spec_id.to_string()),
    };
    let failure: RefCell<Option<Trouble>> = RefCell::new(None);
    let as_of = Cell::new(0);
    let ask = |selection: &Selection| -> anyhow::Result<PipelineSelectionPage> {
        let selection = selection.clone();
        match settings.ask(|instance| HuginnMindRequest::Query {
            instance: instance.clone(),
            selection,
            semantic: None,
        }) {
            Ok(HuginnMindResponse::Query(page)) => {
                as_of.set(page.as_of);
                Ok(page)
            }
            Ok(_) => Err(anyhow::anyhow!(
                "the mind answered a query with something else"
            )),
            Err(trouble) => {
                *failure.borrow_mut() = Some(trouble);
                Err(anyhow::anyhow!("the mind could not be asked"))
            }
        }
    };
    let holding = match hold::holds(&ask) {
        Ok(holds) => holds,
        Err(error) => {
            return Err(failure
                .take()
                .unwrap_or_else(|| Trouble::Internal(error.to_string())));
        }
    };
    let keys: Vec<String> = holding
        .iter()
        .filter(|held| held.items.contains(&spec))
        .map(|held| held.question.id.id.0.clone())
        .collect();
    if keys.is_empty() {
        say(format_args!("matched 0 as_of {} next -", as_of.get()));
        return Ok(0);
    }
    let selection = Selection {
        keys: Some(keys),
        ..Selection::default()
    };
    query(settings, selection, None, cursor, documents)
}

fn sources(
    settings: &Settings,
    texts: Vec<String>,
    repo: Option<String>,
    front: bool,
    top_k: Option<u32>,
    json: bool,
) -> Result<u8, Trouble> {
    let query = SourceQuery {
        text: Line::default(),
        texts: texts.into_iter().map(Line).collect(),
        top_k: top_k.unwrap_or(DEFAULT_TOP_K),
        repo: repo.map(Short),
        front,
    };
    match settings.ask(|instance| HuginnMindRequest::SearchSources {
        instance: instance.clone(),
        query,
    })? {
        HuginnMindResponse::Sources(found) => {
            if json {
                say(serde_json::to_string_pretty(&found).map_err(|error| Trouble::Internal(error.to_string()))?);
            } else {
                say(render::source_hits(&found));
            }
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

fn admit(
    settings: &Settings,
    faculty: Faculty,
    agent: String,
    session: Option<String>,
    seen: Option<String>,
    file: &str,
) -> Result<u8, Trouble> {
    let session = settings.session(session)?;
    let documents = door::documents(&door::file_or_stdin(file)?)?;
    if documents.is_empty() || documents.len() > BATCH_MAX {
        return Err(Trouble::invalid(format!("a batch is 1 to {BATCH_MAX} documents, this one has {}", documents.len())));
    }
    let token = admit_hold::token(&documents, &agent);
    match seen {
        Some(seen) if seen != token => {
            return Err(Trouble::invalid("--seen is not the token of this batch and agent; run without it to get one"));
        }
        Some(_) => {}
        None => {
            let deadline = Instant::now() + HOLD_CHECK;
            let call = |selection, semantic| {
                settings.ask_until(deadline, |instance| HuginnMindRequest::Query { instance: instance.clone(), selection, semantic: Some(semantic) })
            };
            match admit_hold::check(call, &documents, &token) {
                Ok(Some(held)) => {
                    say(held.trim_end());
                    return Ok(6);
                }
                Ok(None) => {}
                Err(trouble) => eprintln!("near-neighbour check did not run ({}); this admit was not held", trouble.code()),
            }
        }
    }
    let sent = documents.clone();
    let provenance = PipelineProvenance { faculty, agent: Short(agent), session: Short(session), tool: Short(TOOL.to_string()), imported_from: None };
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

fn persona_get(settings: &Settings) -> Result<u8, Trouble> {
    match settings.client()?.persona_get().map_err(Trouble::from)? {
        HuginnMindResponse::Persona(Some(view)) => {
            say(serde_json::to_string_pretty(&view.value).map_err(|error| Trouble::Internal(error.to_string()))?);
            say(format_args!("updatedAt {} receipt {}", view.updated_at, view.receipt_id));
            Ok(0)
        }
        HuginnMindResponse::Persona(None) => {
            say("absent");
            Ok(1)
        }
        HuginnMindResponse::Refused(refusal) => Ok(refused(&refusal)),
        other => Err(unexpected(&other)),
    }
}

fn persona_put(
    settings: &Settings,
    faculty: Faculty,
    agent: String,
    session: Option<String>,
    expected_updated_at: Option<String>,
    file: &str,
) -> Result<u8, Trouble> {
    let session = settings.session(session)?;
    let text = door::file_or_stdin(file)?;
    let state: Value = serde_json::from_str(&text)
        .map_err(|error| Trouble::invalid(format!("the persona document is not JSON: {error}")))?;
    if !state.is_object() {
        return Err(Trouble::invalid("the persona document is one JSON object, the whole gamecult.persona_state.v0"));
    }
    let provenance = PipelineProvenance { faculty, agent: Short(agent), session: Short(session), tool: Short(TOOL.to_string()), imported_from: None };
    match settings.client()?.persona_put(provenance, state, expected_updated_at).map_err(Trouble::from)? {
        HuginnMindResponse::PersonaPut(PersonaPutOutcome::Committed { receipt_id }) => {
            say(format_args!("Committed {receipt_id}"));
            Ok(0)
        }
        HuginnMindResponse::PersonaPut(PersonaPutOutcome::Refused(refusal)) | HuginnMindResponse::Refused(refusal) => {
            Ok(refused(&refusal))
        }
        other => Err(unexpected(&other)),
    }
}

fn unexpected(answer: &HuginnMindResponse) -> Trouble {
    let name = match answer {
        HuginnMindResponse::Whoami(_) => "whoami",
        HuginnMindResponse::Admit(_) => "admit",
        HuginnMindResponse::View(_) => "view",
        HuginnMindResponse::Query(_) => "query",
        HuginnMindResponse::Persona(_) => "a persona",
        HuginnMindResponse::PersonaPut(_) => "a persona put",
        HuginnMindResponse::Sources(_) => "source hits",
        HuginnMindResponse::Chat(_) => "chat hits",
        HuginnMindResponse::Activity(_) => "repo activity",
        HuginnMindResponse::RootsTouched(_) => "roots touched",
        HuginnMindResponse::Refused(_) => "a refusal",
        HuginnMindResponse::Deferred(_) => "a deferral",
    };
    Trouble::Internal(format!("the daemon answered with {name}, which this call did not ask for"))
}
