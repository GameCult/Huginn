//! The Self-run actuator: one `claude` child carries her one open Self run.
//!
//! `carry` finds her run (the one in-force Self run operated by the mind, read
//! through `launch::holders`: the run is the grant), starts `claude` with the
//! fixed argument list, writes the start message and watches the child's
//! stream-json output. It counts the main thread's requests, asks for the flush
//! once when the context prefix reaches the ceiling (ruling
//! `eureka-body:ruling:self-lifecycle-flush-rehydrate`: a Self run admits what
//! the mind lacks and ends, it never summarises), closes the child's input at
//! the first result, and stops a child that outlives its flush: an interrupt
//! after the grace, or once the requests of the flush's own turn have grown the
//! prefix by the headroom beyond the prefix the flush was asked at (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`), then SIGTERM and SIGKILL if
//! the child ignores that. Signals go to the child's process group.
//!
//! Every timing decision is one pure function, `decide` (the `decide` module,
//! ruling `eureka-body:ruling:self-run-decider`). `carry` is the loop that turns
//! the operating system's facts into events and executes the actions.
//!
//! The child's credential is a lease, not a variable (rulings
//! `eureka-body:ruling:self-runs-through-connector` and
//! `eureka-body:ruling:self-run-credential-lease`): before it starts `claude`,
//! `carry` asks the connector for a lease (`lease_client`) until the unit's
//! runtime cap, given to it in seconds (the connector sets the reservation and binds the lease to this
//! unit's invocation), and the granted descriptor is the only place the
//! token reaches the child. A refusal closes the run Recorded and starts nothing.
//! When the child has ended `carry` settles with the usage, per model, the
//! stream reported; a run that ends without settling (killed by the brake, a
//! crash) is billed its reservation by the connector.
//!
//! The actuator admits only the closure of a run whose lease was refused. It
//! reads neither the brake nor the environment and stops no unit: the gate and
//! the brake hold own those. It never echoes an argument, an endpoint, a
//! connector's refusal text or a stream line. Stream-json is Claude Code's wire, a
//! foreign owner's JSON: it is parsed into `serde_json::Value` here and nothing
//! parsed is stored.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineRef, ResolutionOutcome, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelinePageItems};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::launch::{Clock, closure, holders};
use crate::lease_client::{Lender, LeaseRefusalKind, LeaseRequest, Lent, ModelUsage, Settlement, TokenFd, TokenUsage};
use crate::queue::MindPort;

use decide::{Action, Event, State, decide, next_deadline};

/// The measured Self ceiling: at this prefix the flush is asked for.
pub const CEILING_TOKENS: u64 = 300_000;
/// What the prefix may grow by beyond the prefix the flush was asked at before
/// the actuator interrupts. Relative to the flush, never an absolute limit.
pub const FLUSH_HEADROOM_TOKENS: u64 = 60_000;
/// How long the child may take over the flush before it is interrupted.
pub const FLUSH_GRACE_S: i64 = 15 * 60;
/// How long a child may ignore a signal before the next, stronger one.
pub const ESCALATE_AFTER_S: i64 = 120;
/// How long a stream may stay open after the child has exited (a descendant
/// holding its stdout) before the run is over without it.
pub const EXIT_DRAIN_S: i64 = 2;
/// A backstop `claude` enforces; the ceiling is reached first.
pub const MAX_TURNS: u32 = 500;
/// Names the variable the Self unit sets to its own `RuntimeMaxSec`, in seconds:
/// the one length of a run. The unit owns the number; the lease asked for and
/// the connector's `--lease-max-ms` follow it.
pub const RUN_CAP_VARIABLE: &str = "MIND_SELF_RUN_CAP_S";
/// Names the directory `claude` keeps its configuration, settings, hooks and skills in.
const CONFIG_DIR_VARIABLE: &str = "CLAUDE_CONFIG_DIR";
/// The unit's per-invocation scratch directory (systemd's `RuntimeDirectory=`,
/// removed when the unit stops): where each child's configuration is made.
pub const SCRATCH_VARIABLE: &str = "RUNTIME_DIRECTORY";
/// Optional: a directory root owns, copied into each child's fresh configuration
/// (its skills and settings). The child may change its copy; no run sees another's.
pub const SEED_VARIABLE: &str = "MIND_SELF_CONFIG_SEED";
/// Names the descriptor the child reads its credential from.
const TOKEN_FD_VARIABLE: &str = "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR";
/// Tells `claude` to scrub credentials from the environment of what it starts.
const ENV_SCRUB_VARIABLE: &str = "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB";
/// The name the start message tells her to admit as.
pub const AGENT: &str = "mind-self-run";
/// The namespace of the session id: the run id is the name in it.
pub const SESSION_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;

/// The longest the loop waits on the child when no deadline is nearer.
const POLL: StdDuration = StdDuration::from_secs(60);

/// What the child is told at the start. The contract with the `eureka`
/// skill: it names the skill, the instance and the run id.
pub fn start_message(instance: &Slug, run: &PipelineRef) -> String {
    format!(
        "Use the eureka skill. You are Self for the instance `{}`; your run is `{}`. Admit as faculty SelfFaculty, agent {AGENT}.",
        instance.0, run.id.0
    )
}

/// What the child is told once, at the context ceiling.
pub const FLUSH_MESSAGE: &str = "The context ceiling is reached. Admit to the mind what it lacks, close your run, and end.";

/// The session id of a run: UUIDv5 over the run's id. Derived, never stored.
pub fn session_id(run: &PipelineRef) -> Uuid {
    Uuid::new_v5(&SESSION_NAMESPACE, run.id.0.as_bytes())
}

/// The argument list of the child, and nothing else: no resume, continue or
/// fork, no model and no settings of its own belong to this actuator. It fixes
/// the permission mode: print mode starts in Manual when nothing on the command
/// line sets a mode (the managed `defaultMode` was not applied in print mode,
/// 2026-10-31), and `--permission-prompts none` then denies every tool that
/// needs approval, so a Mind's reach within her slice is granted here. Only the
/// user's settings load (the fresh configuration directory
/// the seed fills); the working directory's project and local settings, which a
/// run could have written, do not.
/// `budget_usd` is the run's, as stored.
pub fn claude_args(session: Uuid, budget_usd: &str) -> Vec<String> {
    [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--session-id",
        &session.to_string(),
        "--max-budget-usd",
        budget_usd,
        "--max-turns",
        &MAX_TURNS.to_string(),
        "--permission-mode",
        "bypassPermissions",
        "--permission-prompts",
        "none",
        "--setting-sources",
        "user",
    ]
    .map(String::from)
    .to_vec()
}

/// One line of the child's stream, as far as the actuator reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Usage {
    /// A request of the main thread: the message's id and the context it
    /// read (input, cache read and cache write tokens).
    Request { id: String, prefix: u64 },
    Result,
    /// Valid JSON that is none of the above, or a request of a subagent.
    Other,
    /// Not a JSON object.
    Unreadable,
}

/// Reads one stream line.
pub fn read_usage(line: &str) -> Usage {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return Usage::Unreadable };
    match event.get("type").and_then(Value::as_str) {
        Some("result") => Usage::Result,
        Some("assistant") if event.get("parent_tool_use_id").is_none_or(Value::is_null) => {
            let message = event.get("message");
            let Some(id) = message.and_then(|message| message.get("id")).and_then(Value::as_str) else { return Usage::Other };
            let usage = message.and_then(|message| message.get("usage"));
            let field = |name: &str| usage.and_then(|usage| usage.get(name)).and_then(Value::as_u64).unwrap_or(0);
            // Saturating: a sum that overflows is at or over the ceiling, never a panic or a wrap.
            let prefix = field("input_tokens").saturating_add(field("cache_read_input_tokens")).saturating_add(field("cache_creation_input_tokens"));
            Usage::Request { id: id.to_string(), prefix }
        }
        _ => Usage::Other,
    }
}

/// What the child gave in one wait: the operating system's facts about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    Line(String),
    Timeout,
    /// Its stdout ended.
    Eof,
    /// It was reaped: its exit code, none when a signal ended it.
    Exited(Option<i32>),
}

/// The running `claude`. A test hands in a scripted one.
pub trait Child {
    /// Writes one line to its input. A dead child's stream closes, which is
    /// how `carry` learns of it, so a failed write is not an error here.
    fn write_line(&mut self, line: &str);
    /// Closes its input; later writes go nowhere.
    fn close_input(&mut self);
    fn next(&mut self, timeout: StdDuration) -> Read;
    /// Sends the signal to its whole process group; whether it was delivered.
    fn stop(&mut self, how: Stop) -> bool;
}

/// The signals the actuator sends, weakest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stop {
    /// SIGINT, as an operator's Ctrl-C.
    Interrupt,
    Terminate,
    Kill,
}

impl Stop {
    fn next(self) -> Self {
        match self {
            Self::Interrupt => Self::Terminate,
            Self::Terminate | Self::Kill => Self::Kill,
        }
    }
}

pub trait Spawner {
    /// Starts the child with `token` as its credential descriptor.
    fn spawn(&self, args: &[String], token: &TokenFd) -> Result<Box<dyn Child>>;
}

/// Why a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// The child ended with no flush asked for and no interrupt.
    Own,
    /// The ceiling was reached and the flush asked for; the child ended with no
    /// signal. Whether it ran the flush is for the mind to say, not this label.
    FlushRequested,
    /// A signal was delivered, and this was the strongest one delivered.
    Signalled(Stop),
    /// Not even SIGKILL could be delivered: the child was not waited for.
    Unstoppable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelfRunOutcome {
    /// She has not exactly one open Self run: nothing was started.
    NoRun,
    /// The connector would not lend: the run is closed Recorded and nothing was started.
    LeaseRefused { kind: LeaseRefusalKind },
    Ended { cause: Cause, code: Option<i32>, requests: u64, prefix: u64 },
}

impl SelfRunOutcome {
    /// 0 when the child exited 0, 3 when no child was started, 4 when the
    /// connector refused the lease, else 1 (a child killed by a signal has no code).
    pub fn exit_status(&self) -> i32 {
        match self {
            Self::NoRun => 3,
            Self::LeaseRefused { .. } => 4,
            Self::Ended { code: Some(0), .. } => 0,
            Self::Ended { .. } => 1,
        }
    }
}

impl fmt::Display for SelfRunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRun => write!(f, "no run: she has no single open Self run"),
            Self::LeaseRefused { kind } => write!(f, "no run: the connector refused the lease ({kind:?})"),
            Self::Ended { cause, code, requests, prefix } => {
                let cause = match cause {
                    Cause::Own => "own",
                    Cause::FlushRequested => "flush-requested",
                    Cause::Signalled(Stop::Interrupt) => "interrupted",
                    Cause::Signalled(Stop::Terminate) => "terminated",
                    Cause::Signalled(Stop::Kill) => "killed",
                    Cause::Unstoppable => "unstoppable",
                };
                match code {
                    Some(code) => write!(f, "ended: {cause} code {code} requests {requests} prefix {prefix}"),
                    None => write!(f, "ended: {cause} signal requests {requests} prefix {prefix}"),
                }
            }
        }
    }
}

/// Her one open Self run and its budget, read through the launch primitive's
/// holder reader. Anything but exactly one run is none.
fn her_run(mind: &dyn MindPort) -> Result<Option<(PipelineRef, String)>> {
    let mut held = holders(mind, mind.instance(), RunTurn::SelfRun)?;
    let (Some(run), None) = (held.pop(), held.pop()) else { return Ok(None) };
    let one = Selection { keys: Some(vec![run.id.id.0.clone()]), projection: "document".into(), ..Selection::default() };
    let PipelinePageItems::Documents(views) = mind.query(&one)?.items else { bail!("the mind answered headers to a document selection") };
    match views.into_iter().next().map(|view| view.document) {
        Some(PipelineDocument::Run(document)) => Ok(Some((run.id, document.budget_usd.0))),
        _ => Err(anyhow!("the run's document could not be read")),
    }
}

/// What a run used, from its stream. `claude`'s own account comes first: a
/// result record's `modelUsage` is its cumulative per-model total as of that
/// result, so the last result that has one is the run's usage up to then, plus
/// the tokens of every assistant message after it (the flush turn, or a turn the
/// child was cut off in: those requests are billed too). A stream with no such
/// result falls back to the tokens of every assistant message (once per message
/// id, subagents included), by model.
#[derive(Default)]
struct Spent {
    requests: BTreeMap<String, (String, TokenUsage)>,
    reported: Option<BTreeMap<String, TokenUsage>>,
    /// The message ids heard when `reported` was stated: its total already holds them.
    counted: BTreeSet<String>,
}

/// The model of a message that names none; the connector prices it as it sees fit.
const UNNAMED_MODEL: &str = "unknown";

impl Spent {
    fn hear(&mut self, line: &str) {
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return };
        match event.get("type").and_then(Value::as_str) {
            Some("assistant") => {}
            Some("result") => return self.hear_result(&event),
            _ => return,
        }
        let message = event.get("message");
        let Some(id) = message.and_then(|message| message.get("id")).and_then(Value::as_str) else { return };
        let model = message.and_then(|message| message.get("model")).and_then(Value::as_str).unwrap_or(UNNAMED_MODEL);
        let usage = message.and_then(|message| message.get("usage"));
        let field = |name: &str| usage.and_then(|usage| usage.get(name)).and_then(Value::as_u64).unwrap_or(0);
        let (named, seen) = self.requests.entry(id.to_string()).or_insert_with(|| (model.to_string(), TokenUsage::default()));
        *named = model.to_string();
        // Records that share an id are one request, each with the usage so far.
        seen.input_tokens = seen.input_tokens.max(field("input_tokens"));
        seen.cache_read_tokens = seen.cache_read_tokens.max(field("cache_read_input_tokens"));
        seen.cache_write_tokens = seen.cache_write_tokens.max(field("cache_creation_input_tokens"));
        seen.output_tokens = seen.output_tokens.max(field("output_tokens"));
    }

    /// A result's own per-model total, when it states one.
    fn hear_result(&mut self, result: &serde_json::Map<String, Value>) {
        let Some(Value::Object(models)) = result.get("modelUsage") else { return };
        let counted = |used: &Value, name: &str| used.get(name).and_then(Value::as_u64).unwrap_or(0);
        let stated: BTreeMap<String, TokenUsage> = models
            .iter()
            .map(|(model, used)| {
                let usage = TokenUsage {
                    input_tokens: counted(used, "inputTokens"),
                    cache_read_tokens: counted(used, "cacheReadInputTokens"),
                    cache_write_tokens: counted(used, "cacheCreationInputTokens"),
                    output_tokens: counted(used, "outputTokens"),
                    reasoning_tokens: None,
                };
                (model.clone(), usage)
            })
            .collect();
        if !stated.is_empty() {
            self.reported = Some(stated);
            self.counted = self.requests.keys().cloned().collect();
        }
    }

    /// The settle: the usage per model, models in name order.
    fn settlement(&self) -> Settlement {
        let mut by_model: BTreeMap<String, TokenUsage> = self.reported.clone().unwrap_or_default();
        for (id, (model, request)) in &self.requests {
            if self.reported.is_some() && self.counted.contains(id) {
                continue;
            }
            let total = by_model.entry(model.clone()).or_default();
            total.input_tokens = total.input_tokens.saturating_add(request.input_tokens);
            total.cache_read_tokens = total.cache_read_tokens.saturating_add(request.cache_read_tokens);
            total.cache_write_tokens = total.cache_write_tokens.saturating_add(request.cache_write_tokens);
            total.output_tokens = total.output_tokens.saturating_add(request.output_tokens);
        }
        Settlement { usage: by_model.into_iter().map(|(model, usage)| ModelUsage { model, usage }).collect() }
    }
}

fn unix_ms(at: DateTime<Utc>) -> u64 {
    u64::try_from(at.timestamp_millis()).unwrap_or(0)
}

/// Closes a run whose lease was refused: Recorded with the refusal's kind, so a
/// run that cannot be paid for counts as an empty run and trips the breaker.
fn close_refused(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef, kind: LeaseRefusalKind) -> Result<SelfRunOutcome> {
    let reason = format!("the connector refused the credential lease: {kind:?}");
    let document = closure(mind, run, clock.now(), ResolutionOutcome::Recorded { reason: reason.as_str().into() }, "The Self run's credential lease was refused, so no claude was started.")?;
    let label = run.id.0.rsplit(':').next().unwrap_or(&run.id.0);
    match mind.admit(AGENT, label, vec![document])? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(SelfRunOutcome::LeaseRefused { kind }),
        _ => Err(anyhow!("the run whose lease was refused could not be closed")),
    }
}

/// Closes a run whose lease was refused for its length: Withdrawn, because no
/// claude ever ran and a misconfiguration is not an empty run; the breaker
/// counts nothing and a launch that finds the unit dead has no open run to close.
fn withdraw_misconfigured(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef) -> Result<()> {
    let outcome = ResolutionOutcome::Withdrawn { reason: "the connector refused the lease as too long".into() };
    let document = closure(mind, run, clock.now(), outcome, "The unit's runtime cap exceeds the connector's lease length, so no claude was started.")?;
    let label = run.id.0.rsplit(':').next().unwrap_or(&run.id.0);
    match mind.admit(AGENT, label, vec![document])? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(()),
        _ => Err(anyhow!("the run whose lease was too long could not be withdrawn")),
    }
}

/// Carries her one open Self run: one `claude` child from the lease to its
/// settle. The request lines go to `out`. A mind that cannot be reached, a
/// connector that cannot be reached or a child that cannot be started is an
/// error; every other end is an outcome. The lease comes first: with none
/// granted no child exists. It is settled once, after the child is known to
/// have ended; a lease dropped any other way is billed in full by the connector.
/// `run_cap_s` is the unit's `RuntimeMaxSec`: a connector that finds the lease
/// too long is misconfigured, which is an error naming the mismatch and not a
/// refusal of the run: the run is withdrawn, so the breaker counts nothing.
pub fn carry(mind: &dyn MindPort, clock: &dyn Clock, lender: &dyn Lender, spawner: &dyn Spawner, run_cap_s: u64, out: &mut dyn Write) -> Result<SelfRunOutcome> {
    let Some((run, budget)) = her_run(mind)? else { return Ok(SelfRunOutcome::NoRun) };
    let request = LeaseRequest { expires_at_unix_ms: unix_ms(clock.now()).saturating_add(run_cap_s.saturating_mul(1000)) };
    let lease = match lender.lend(&request)? {
        Lent::Granted(lease) => lease,
        Lent::Refused(refused) if refused.kind == LeaseRefusalKind::Expired => {
            withdraw_misconfigured(mind, clock, &run)?;
            bail!("the connector refused a {run_cap_s} s lease as too long: raise its --lease-max-ms to at least the unit's RuntimeMaxSec ({RUN_CAP_VARIABLE}) plus slack");
        }
        Lent::Refused(refused) => return close_refused(mind, clock, &run, refused.kind),
    };
    let child = match spawner.spawn(&claude_args(session_id(&run), &budget), lease.token()) {
        Ok(child) => child,
        Err(error) => {
            // No child ever held the token; the empty report is the connector's cue to bill the reservation.
            let _ = lease.settle(Settlement::default());
            return Err(error);
        }
    };
    let mut spent = Spent::default();
    let outcome = drive(mind, clock, child, &run, &mut spent, out)?;
    // A child that could not even be killed may still be spending: that lease is
    // dropped, and the connector bills the reservation.
    if !matches!(outcome, SelfRunOutcome::Ended { cause: Cause::Unstoppable, .. }) {
        // A settle that fails leaves the connector to bill the reservation; the run's outcome stands.
        // Either way the journal says what the connector billed, or that it will bill the reservation.
        let report = match lease.settle(spent.settlement()) {
            Ok(billed) => format!("settled: billed {billed} micro-USD"),
            Err(_) => "settle failed: the connector bills the reservation".to_string(),
        };
        let _ = writeln!(out, "{report}");
    }
    Ok(outcome)
}

/// The most of a result's text that reaches the journal.
const RESULT_TEXT_LIMIT: usize = 4096;

/// What `claude` concluded: a result event's text, with its subtype and error
/// flag when it has them, written to the unit's stdout so it reaches the journal
/// before the transcript goes with the runtime directory. Bounded, marked when cut.
fn write_result(line: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return Ok(()) };
    if event.get("type").and_then(Value::as_str) != Some("result") {
        return Ok(());
    }
    let mut head = String::from("result");
    if let Some(subtype) = event.get("subtype").and_then(Value::as_str) {
        head.push_str(&format!(" subtype={subtype}"));
    }
    if let Some(is_error) = event.get("is_error").and_then(Value::as_bool) {
        head.push_str(&format!(" is_error={is_error}"));
    }
    let text = event.get("result").and_then(Value::as_str).unwrap_or("");
    let mut end = text.len().min(RESULT_TEXT_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let marker = if end < text.len() { "\n[truncated]" } else { "" };
    writeln!(out, "{head}: {}{marker}", &text[..end])
}

/// The loop: facts in, one clock read per fact, actions out.
fn drive(mind: &dyn MindPort, clock: &dyn Clock, mut child: Box<dyn Child>, run: &PipelineRef, spent: &mut Spent, out: &mut dyn Write) -> Result<SelfRunOutcome> {
    let user_line = |text: &str| json!({"type": "user", "message": {"role": "user", "content": text}}).to_string();
    child.write_line(&user_line(&start_message(mind.instance(), run)));

    let mut state = State::default();
    let mut now = clock.now();
    loop {
        let wait = next_deadline(&state).map_or(POLL, |at| (at - now).to_std().unwrap_or_default().min(POLL));
        let fact = match child.next(wait) {
            Read::Line(line) => {
                spent.hear(&line);
                write_result(&line, out)?;
                match read_usage(&line) {
                    Usage::Request { id, prefix } => Event::Request { id, prefix },
                    Usage::Result => Event::Result,
                    Usage::Other | Usage::Unreadable => Event::Tick,
                }
            }
            Read::Timeout => Event::Tick,
            Read::Eof => Event::StdoutEof,
            Read::Exited(code) => Event::ChildExited(code),
        };
        now = clock.now();
        let mut event = Some(fact);
        while let Some(heard) = event.take() {
            let counted = state.requests;
            let (next, action) = decide(state, heard, now);
            state = next;
            if state.requests != counted {
                writeln!(out, "request {} prefix {}", state.requests, state.prefix)?;
            }
            match action {
                Action::None => {}
                Action::WriteFlush => child.write_line(&user_line(FLUSH_MESSAGE)),
                Action::CloseStdin => child.close_input(),
                Action::Signal(how) => event = Some(Event::Signalled { how, delivered: child.stop(how) }),
                Action::Finish(outcome) => return Ok(outcome),
            }
        }
    }
}

/// The real spawner: the `claude` on the unit's `PATH`, its stderr in the
/// journal. Its credential is the lease's descriptor, named in the one variable
/// that says where to read it; the token is in no environment, argument or file.
/// It holds the unit's environment, which the library never reads: the binary
/// hands it over, and [`child_environment`] decides what of it the child sees.
pub struct ClaudeSpawner {
    unit_environment: Vec<(OsString, OsString)>,
    scratch: PathBuf,
    seed: Option<PathBuf>,
}

impl ClaudeSpawner {
    /// From the unit's environment: it must name [`SCRATCH_VARIABLE`], and may name [`SEED_VARIABLE`].
    pub fn new(unit_environment: impl IntoIterator<Item = (OsString, OsString)>) -> Result<Self> {
        let unit_environment: Vec<(OsString, OsString)> = unit_environment.into_iter().collect();
        let named = |variable: &str| unit_environment.iter().find(|(name, _)| name == variable).map(|(_, value)| PathBuf::from(value));
        // systemd lists several runtime directories colon-separated; the unit names one.
        let scratch = named(SCRATCH_VARIABLE).filter(|path| !path.as_os_str().is_empty()).ok_or_else(|| anyhow!("{SCRATCH_VARIABLE} is not set: the unit needs RuntimeDirectory="))?;
        let seed = named(SEED_VARIABLE).filter(|path| !path.as_os_str().is_empty());
        Ok(Self { unit_environment, scratch, seed })
    }
}

impl Spawner for ClaudeSpawner {
    fn spawn(&self, args: &[String], token: &TokenFd) -> Result<Box<dyn Child>> {
        let config = FreshConfig::create(&self.scratch, self.seed.as_deref())?;
        spawn_program(Path::new("claude"), args, token, &self.unit_environment, config)
    }
}

/// The directory name of a child's configuration inside the scratch directory.
const CONFIG_DIR_NAME: &str = "claude-config";

/// One child's configuration directory: made empty (or as the seed) for it,
/// owner-only, and removed when the child's handle goes. Whatever the run writes
/// there (settings that set an environment variable, an `apiKeyHelper`, a base
/// URL, a hook) lasts only as long as that run: the next one starts from the seed.
struct FreshConfig(PathBuf);

impl FreshConfig {
    fn create(scratch: &Path, seed: Option<&Path>) -> Result<Self> {
        let dir = scratch.join(CONFIG_DIR_NAME);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("a stale configuration directory could not be removed"),
        }
        let mut made = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut made, 0o700);
        made.create(&dir).context("the configuration directory could not be made")?;
        let fresh = Self(dir);
        if let Some(seed) = seed {
            copy_tree(seed, &fresh.0).context("the configuration seed could not be copied")?;
        }
        Ok(fresh)
    }
}

impl Drop for FreshConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A seed link the copy refuses: it would reach outside the seed.
#[derive(Debug)]
struct SeedLinkRefused {
    link: PathBuf,
    target: PathBuf,
}

impl fmt::Display for SeedLinkRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the seed link {} -> {} is absolute or leaves the seed", self.link.display(), self.target.display())
    }
}

impl std::error::Error for SeedLinkRefused {}

/// Copies directories and regular files; a link is reproduced as a link with
/// the same target, never followed into the copy. A link is refused with
/// [`SeedLinkRefused`] unless its target is relative and, resolved through
/// every link on its way in the source, lands under the canonical seed root.
/// The source check refuses early; but a link's text resolves from where the
/// copy sits, so once the copy is whole every reproduced link is resolved again
/// in `to`, and one that lands outside it is refused the same way.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    copy_level(&from.canonicalize()?, from, to)?;
    check_links(&to.canonicalize()?, to)
}

fn check_links(root: &Path, dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            check_links(root, &entry.path())?;
        } else if kind.is_symlink() && !entry.path().canonicalize().is_ok_and(|resolved| resolved.starts_with(root)) {
            return Err(std::io::Error::other(SeedLinkRefused { link: entry.path(), target: std::fs::read_link(entry.path())? }));
        }
    }
    Ok(())
}

fn copy_level(root: &Path, from: &Path, to: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir(&target)?;
            copy_level(root, &entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        } else if kind.is_symlink() {
            let points_at = std::fs::read_link(entry.path())?;
            let stays = !points_at.is_absolute() && entry.path().canonicalize().is_ok_and(|resolved| resolved.starts_with(root));
            if !stays {
                return Err(std::io::Error::other(SeedLinkRefused { link: entry.path(), target: points_at }));
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&points_at, &target)?;
            #[cfg(not(unix))]
            return Err(std::io::Error::other("a seed link cannot be reproduced on this platform"));
        }
    }
    Ok(())
}

/// Whether the child may see a unit variable. `claude` prefers an ambient
/// credential (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`,
/// `CLAUDE_CODE_OAUTH_TOKEN`) to the lease's descriptor and takes its endpoint
/// from `ANTHROPIC_BASE_URL`, so the child gets only what a shell needs to find
/// its way; nothing credential-shaped is on the list. Its `HOME` and
/// `CLAUDE_CONFIG_DIR` are its fresh configuration directory, set by the spawn.
fn child_may_see(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    matches!(name.as_ref(), "PATH" | "LANG" | "TERM") || name.starts_with("LC_")
}

/// The unit's environment as the child sees it: the allow-list, and nothing else.
fn child_environment(unit: &[(OsString, OsString)]) -> Vec<(&OsString, &OsString)> {
    unit.iter().filter(|(name, _)| child_may_see(name)).map(|(name, value)| (name, value)).collect()
}

/// Starts `program` with a duplicate of the token descriptor open in it, and
/// closes that duplicate here the moment the child exists. The lease's own
/// descriptor is close-on-exec, so this duplicate is the child's only way to the
/// token. `claude` reads it to the end once at startup, so what its own children
/// inherit is an emptied pipe whose write end exists nowhere. The child's
/// environment is cleared and rebuilt from `unit`'s allow-listed variables.
fn spawn_program(program: &Path, args: &[String], token: &TokenFd, unit: &[(OsString, OsString)], config: FreshConfig) -> Result<Box<dyn Child>> {
    let inheritable = token.inheritable().context("the credential descriptor could not be placed")?;
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    command.env_clear().envs(child_environment(unit));
    command.env("HOME", &config.0).env(CONFIG_DIR_VARIABLE, &config.0);
    command.env(TOKEN_FD_VARIABLE, inheritable.number().to_string()).env(ENV_SCRUB_VARIABLE, "1");
    // Its own process group, so that a signal reaches what the child started too.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut process = command.spawn().context("claude could not be started")?;
    drop(inheritable);
    let group = process.id();
    let stdin = process.stdin.take();
    let stdout = process.stdout.take().ok_or_else(|| anyhow!("claude has no stdout"))?;
    let (sender, facts) = channel();
    let lines = sender.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).split(b'\n') {
            let Ok(bytes) = line else { break };
            if lines.send(Read::Line(String::from_utf8_lossy(&bytes).into_owned())).is_err() {
                return;
            }
        }
        let _ = lines.send(Read::Eof);
    });
    // The one waitpid: the child's end is a fact of its own, not inferred from its stream.
    std::thread::spawn(move || {
        let _ = sender.send(Read::Exited(process.wait().ok().and_then(|status| status.code())));
    });
    Ok(Box::new(ClaudeProcess { stdin, facts, group, _config: config }))
}

struct ClaudeProcess {
    stdin: Option<ChildStdin>,
    facts: Receiver<Read>,
    /// The child's pid, which is its process group's id.
    group: u32,
    /// Its configuration, removed when the child's handle goes.
    _config: FreshConfig,
}

impl Child for ClaudeProcess {
    fn write_line(&mut self, line: &str) {
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = stdin.write_all(line.as_bytes()).and_then(|()| stdin.write_all(b"\n")).and_then(|()| stdin.flush());
        }
    }

    fn close_input(&mut self) {
        self.stdin = None;
    }

    fn next(&mut self, timeout: StdDuration) -> Read {
        match self.facts.recv_timeout(timeout) {
            Ok(fact) => fact,
            Err(RecvTimeoutError::Timeout) => Read::Timeout,
            Err(RecvTimeoutError::Disconnected) => Read::Eof,
        }
    }

    fn stop(&mut self, how: Stop) -> bool {
        // The `kill` program, not a libc call: the library names no FFI.
        let signal = match how {
            Stop::Interrupt => "INT",
            Stop::Terminate => "TERM",
            Stop::Kill => "KILL",
        };
        Command::new("kill").args(["-s", signal, "--", &format!("-{}", self.group)]).stderr(Stdio::null()).status().is_ok_and(|status| status.success())
    }
}

mod decide;

#[cfg(test)]
mod tests;
