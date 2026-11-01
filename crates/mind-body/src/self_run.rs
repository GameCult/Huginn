//! The Self-run actuator: one `claude` child carries her one open Self run.
//!
//! `carry` finds her run (the one in-force Self run operated by the mind, read
//! through `launch::holders`: the run is the grant), starts `claude` with the
//! fixed argument list, writes the start message and watches the child's
//! stream-json output. It counts the main thread's requests, asks for the flush
//! once when the context prefix reaches the ceiling (ruling
//! `eureka-body:ruling:self-lifecycle-flush-rehydrate`: a Self run admits what
//! the mind lacks and ends, it never summarises), and stops a child that outlives its flush: an interrupt
//! after the grace, or once the requests of the flush's own turn have grown the
//! prefix by the headroom beyond the prefix the flush was asked at (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`), then SIGTERM and SIGKILL if
//! the child ignores that. Signals go to the child's process group.
//!
//! One run is one live session (ruling `eureka-body:ruling:live-mind-sessions`).
//! A result ends a turn, not the session. What her Self is asked is derived from
//! her mind (the `inbox` module): an open follow-up with owner `self-run` admitted
//! by her Persona turn is a brief. `drive` is the one writer into her
//! session, and keeps one write outstanding (ruling
//! `eureka-body:ruling:self-session-one-outstanding-write`): the start message,
//! then one line at a time holding the briefs that wait and the flush ask, each
//! sent only after the previous line's echo, mid-turn or idle. The CLI echoes each
//! line it takes into a request (`--replay-user-messages`) and joins lines queued
//! behind one request into a single echo, so the echo is the consumption point
//! only while no second line is in flight. Her Self resolves each brief she
//! answers; the actuator closes none, and a brief she left open is told to her
//! next session. Compaction is off in
//! the CLI (`DISABLE_COMPACT=1`): the organ's own ceiling, flush, end and rehydrate
//! is the only path. The session ends where its cache would be evicted, near the
//! unit's cap, at the context ceiling, or after the flush each of those asks.
//!
//! Every timing decision is one pure function, `decide` (the `decide` module,
//! ruling `eureka-body:ruling:self-run-decider`). `carry` is the loop that turns
//! the operating system's facts into events and executes the actions.
//!
//! The child's credential is a lease, not a variable (rulings
//! `eureka-body:ruling:self-runs-through-connector` and
//! `eureka-body:ruling:self-run-credential-lease`): before it starts `claude`,
//! `carry` asks the connector for a lease (`lease_client`) until the unit runtime
//! cap, given to it in seconds (the connector binds the lease to this unit
//! invocation), and the granted descriptor is the only place the token reaches
//! the child. A refusal closes the run Recorded and starts nothing. Nothing is
//! reserved, billed or settled: the lease ends when `carry` drops it, and nothing
//! bounds a run in dollars or in usage; the bounds are `--max-turns`, the unit
//! `RuntimeMaxSec` and the context-ceiling flush. The grant carries the plan usage
//! windows, which go to the child in its start message.
//!
//! The actuator admits run closures and brief closures, nothing else. Every exit of
//! `carry` that leaves her run open resolves it Recorded exactly once (the
//! child's result in the rationale, or that it gave none in the reason). It reads neither the
//! brake nor the environment and stops no unit: the gate and the brake hold own those.
//! It never echoes an argument, an endpoint, a
//! connector's refusal text or a stream line. Stream-json is Claude Code's wire, a
//! foreign owner's JSON: it is parsed into `serde_json::Value` here and nothing
//! parsed is stored.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineRef, ResolutionOutcome, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelinePageItems};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::launch::{Clock, closure, holders};
use crate::connector::render_plan;
use crate::lease_client::{LeaseRefusalKind, LeaseRequest, Lender, Lent, TokenFd};
use model_connector::PlanQuota;
use crate::queue::MindPort;

pub(crate) use decide::Why;
use inbox::Brief;
use session::{Polled, SessionInbox};
pub(crate) use session::drive;

/// The measured Self ceiling: at this prefix the flush is asked for.
pub const CEILING_TOKENS: u64 = 300_000;
/// The context window the CLI reports for the model a Self runs (`modelUsage.contextWindow` in every result of the
/// `claude-2.1.312-live-session-*` fixtures: the default model, `claude-opus-5-5`, 1M). The seed sets no model, so the CLI's default is the model.
/// The window under the real subscription is read from a result's `contextWindow` and not otherwise observed here.
const SELF_CONTEXT_WINDOW_TOKENS: u64 = 1_000_000;
/// What the CLI keeps free below its window before it refuses a prompt (`blocking_limit`): the blocking-limit fixture sends a 990k request and refuses the next.
const CLI_BLOCKING_BUFFER_TOKENS: u64 = 10_000;
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
/// How long before the unit's cap the session is asked for its flush, idle or working.
pub const ROTATE_MARGIN_S: i64 = 25 * 60;
/// The signal ladder's length: three escalations, the stream drain and a minute of slack. The grace interrupt is never later than the cap less this.
pub const LADDER_S: i64 = 3 * ESCALATE_AFTER_S + EXIT_DRAIN_S + 60;
const _: () = assert!(FLUSH_GRACE_S + LADDER_S < ROTATE_MARGIN_S, "the flush and the whole ladder must finish before the unit's cap");
/// The ceiling, and the headroom its flush's own turn may grow by, sit below the CLI's blocking limit with 600k to spare at a 1M window; a window under
/// 400k has no room for the flush's turn, and the assertion refuses to compile until the ceiling is cut to fit the window.
const _: () = assert!(CEILING_TOKENS + FLUSH_HEADROOM_TOKENS + 40_000 < SELF_CONTEXT_WINDOW_TOKENS - CLI_BLOCKING_BUFFER_TOKENS, "the ceiling and its flush headroom must fit under the CLI's blocking limit");
/// How often the mind is asked for new briefs.
pub const INBOX_POLL_S: i64 = 30;
/// The prompt cache's lifetimes, and how long before one runs out the session ends.
pub const TTL_1H_S: i64 = 3600;
pub const TTL_5M_S: i64 = 300;
pub const TTL_MARGIN_S: i64 = 60;
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
/// Turns off the CLI's compaction, auto, idle and manual: the mind is the one summary.
const DISABLE_COMPACT_VARIABLE: &str = "DISABLE_COMPACT";
/// Tells `claude` to scrub credentials from the environment of what it starts.
const ENV_SCRUB_VARIABLE: &str = "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB";
/// Optional, set by the unit: `off` turns that scrub off for a run whose user holds root, where it
/// protects nothing and its sandbox's no_new_privs would only stop `sudo`. Anything else keeps it on.
const SCRUB_VARIABLE: &str = "MIND_SELF_SCRUB";
/// The name the start message tells her to admit as.
pub const AGENT: &str = "mind-self-run";
/// The namespace of the session id: the run id is the name in it.
pub const SESSION_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;

/// What the child is told at the start. The contract with the `eureka`
/// skill: it names the skill, the instance and the run id.
pub fn start_message(instance: &Slug, run: &PipelineRef, plan_quota: &[PlanQuota], now_ms: u64) -> String {
    format!(
        "Use the eureka skill. You are Self for the instance `{}`; your run is `{}`. Admit as faculty SelfFaculty, agent {AGENT}.\n\nThis session stays open after you answer: briefs from your Persona arrive while you work and between tasks. When you answer a brief, resolve follow-up <its id> Recorded with your answer as rationale; if you decline it, resolve it Withdrawn with why. Work you take on beyond an answer is a follow-up of your own.\n\n{}",
        instance.0,
        run.id.0,
        render_plan(plan_quota, now_ms)
    )
}

/// The task section of the start message: one section per claim, in claim
/// order. A claimed follow-up gives its id, campaign and `item` verbatim,
/// because the brief lives in the claim and a run that cannot reach its mind
/// would never see its job; a claimed cut spec is named as the task, to be read
/// with `huginn view`; a claim the mind does not hold, or of another kind, is
/// named by id. The claims are bounded by their types, so nothing is cut. A mind
/// that cannot answer is an error: the run is not started on a guessed task.
fn task_section(mind: &dyn MindPort, claims: &[PipelineRef]) -> Result<String> {
    let mut task = String::new();
    for claim in claims {
        let one = Selection { keys: Some(vec![claim.id.0.clone()]), projection: "document".into(), ..Selection::default() };
        let page = mind.query(&one).with_context(|| format!("the claim `{}` could not be read from the mind", claim.id.0))?;
        let PipelinePageItems::Documents(views) = page.items else { bail!("the mind answered headers to a document selection") };
        let id = &claim.id.0;
        match views.into_iter().next().map(|view| view.document) {
            Some(PipelineDocument::FollowUp(follow_up)) => {
                task.push_str(&format!("

Task, claimed follow-up `{id}` (campaign `{}`):
{}", follow_up.campaign.0, follow_up.item.0));
            }
            Some(PipelineDocument::CutSpec(spec)) => {
                task.push_str(&format!("

Task, claimed cut spec `{id}` (campaign `{}`): the spec is the task; read it with `huginn view`.", spec.campaign.0));
            }
            Some(_) => task.push_str(&format!("

The run claims `{id}` ({:?}), which is no brief: its task is unknown.", claim.kind)),
            None => task.push_str(&format!("

The run claims `{id}`, which the mind does not hold: its task is unknown.")),
        }
    }
    Ok(task)
}

/// What the child is told once, when its session must end: the ceiling, the cap or the cache.
pub const FLUSH_MESSAGE: &str = "The context ceiling is reached. Admit to the mind what it lacks, close your run, and end.";

/// The flush as written: the one message, and what she does with the briefs she holds.
fn flush_text(why: Why) -> String {
    let cause = match why {
        Why::Ceiling => "",
        Why::Rotation => " (This session ends soon: the unit's time is nearly up.)",
        Why::Idle => " (This session ends because nothing is waiting and its cache would lapse.)",
    };
    format!("{FLUSH_MESSAGE}{cause} Resolve each brief you answered (Recorded, your answer as rationale); a brief you are still working stays open and the next session continues it.")
}

/// The session id of a run: UUIDv5 over the run's id. Derived, never stored.
pub fn session_id(run: &PipelineRef) -> Uuid {
    Uuid::new_v5(&SESSION_NAMESPACE, run.id.0.as_bytes())
}

/// The argument list of the child, and nothing else: no resume, continue or
/// fork, no model, no settings and no hooks belong to this actuator. It asks the
/// CLI to echo each stdin line it takes into a request (`--replay-user-messages`),
/// which is how delivery is counted. It fixes the permission mode: print mode starts in Manual when nothing on the command
/// line sets a mode (the managed `defaultMode` was not applied in print mode,
/// 2026-10-31), and `--permission-prompts none` then denies every tool that
/// needs approval, so a Mind's reach within her slice is granted here, but
/// with `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` set the CLI forces the mode to
/// default (probe, claude 2.1.312): the flag has no effect there and only the
/// seed's allow list decides tools. Only the
/// user's settings load (the fresh configuration directory
/// the seed fills); the working directory's project and local settings, which a
/// run could have written, do not.
pub fn claude_args(session: Uuid) -> Vec<String> {
    [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--session-id",
        &session.to_string(),
        "--max-turns",
        &MAX_TURNS.to_string(),
        "--permission-mode",
        "bypassPermissions",
        "--permission-prompts",
        "none",
        "--setting-sources",
        "user",
        "--replay-user-messages",
    ]
    .map(String::from)
    .to_vec()
}

/// The model name on the CLI's own assistant lines, which no request produced.
const SYNTHETIC_MODEL: &str = "<synthetic>";

/// One line of the child's stream, as far as the actuator reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Usage {
    /// A request of the main thread: the message's id and the context it
    /// read (input, cache read and cache write tokens).
    Request { id: String, prefix: u64 },
    /// A user line of the main thread that the CLI took into a request (a replay echo): ours, never a tool result.
    Echo,
    /// A turn ended. `ok`: not an error. `blocked`: the CLI refused the context (`blocking_limit`).
    Result { ok: bool, blocked: bool },
    /// Valid JSON that is none of the above, or a request of a subagent.
    Other,
    /// Not a JSON object.
    Unreadable,
}

/// Whether a main-thread request wrote its prompt cache for an hour (the split
/// `cache_creation.ephemeral_1h_input_tokens`), else for five minutes. None for
/// any other line.
pub fn writes_hour_cache(line: &str) -> Option<bool> {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return None };
    if event.get("type").and_then(Value::as_str) != Some("assistant") || !event.get("parent_tool_use_id").is_none_or(Value::is_null) {
        return None;
    }
    let hour = event.get("message")?.get("usage")?.get("cache_creation")?.get("ephemeral_1h_input_tokens").and_then(Value::as_u64);
    Some(hour.unwrap_or(0) > 0)
}

/// Reads one stream line.
pub fn read_usage(line: &str) -> Usage {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return Usage::Unreadable };
    match event.get("type").and_then(Value::as_str) {
        Some("result") => Usage::Result {
            ok: !event.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            blocked: event.get("terminal_reason").and_then(Value::as_str) == Some("blocking_limit"),
        },
        Some("user")
            if event.get("isReplay").and_then(Value::as_bool) == Some(true)
                && event.get("parent_tool_use_id").is_none_or(Value::is_null)
                && event.get("message").and_then(|message| message.get("content")).is_some_and(Value::is_string) =>
        {
            Usage::Echo
        }
        Some("assistant") if event.get("parent_tool_use_id").is_none_or(Value::is_null) => {
            let message = event.get("message");
            // The CLI's own lines after an API error or a refused slash command carry a uuid id and no usage: no request was made.
            if message.and_then(|message| message.get("model")).and_then(Value::as_str) == Some(SYNTHETIC_MODEL) {
                return Usage::Other;
            }
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

/// Her one open Self run and its claims, read through the launch primitive holder
/// reader. Anything but exactly one run is none: the mind admits at most one Self
/// run of hers in force, and the launch that opens the next closes a dead one.
fn her_run(mind: &dyn MindPort) -> Result<Option<(PipelineRef, Vec<PipelineRef>)>> {
    let mut held = holders(mind, mind.instance(), RunTurn::SelfRun)?;
    let (Some(run), None) = (held.pop(), held.pop()) else { return Ok(None) };
    let one = Selection { keys: Some(vec![run.id.id.0.clone()]), projection: "document".into(), ..Selection::default() };
    let PipelinePageItems::Documents(views) = mind.query(&one)?.items else { bail!("the mind answered headers to a document selection") };
    match views.into_iter().next().map(|view| view.document) {
        Some(PipelineDocument::Run(document)) => Ok(Some((run.id, document.claims))),
        _ => Err(anyhow!("the run's document could not be read")),
    }
}

/// What the stream said at the end of the run: its head becomes the closure's reason, its text (bounded) the rationale.
#[derive(Default)]
struct Heard {
    /// The last result heard, ok or not: its head line and its text cut to `RATIONALE_RESULT_LIMIT`.
    concluded: Option<(String, String)>,
}

impl Heard {
    fn hear(&mut self, line: &str) {
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return };
        if event.get("type").and_then(Value::as_str) == Some("result") {
            self.concluded = Some((result_head(&event), cut_text(result_text(&event), RATIONALE_RESULT_LIMIT)));
        }
    }
}

fn unix_ms(at: DateTime<Utc>) -> u64 {
    u64::try_from(at.timestamp_millis()).unwrap_or(0)
}

/// Closes a run whose lease was refused: Recorded with the refusal's kind, so a
/// run that cannot be paid for counts as an empty run and trips the breaker.
fn close_refused(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef, kind: LeaseRefusalKind) -> Result<SelfRunOutcome> {
    let reason = format!("the connector refused the credential lease: {kind:?}");
    admit_closure(mind, clock, run, ResolutionOutcome::Recorded { reason: reason.as_str().into() }, "The Self run's credential lease was refused, so no claude was started.")?;
    Ok(SelfRunOutcome::LeaseRefused { kind })
}

/// Admits the closing resolution of `run`: the one writer of a closure.
fn admit_closure(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef, outcome: ResolutionOutcome, rationale: &str) -> Result<()> {
    let document = closure(mind, run, clock.now(), outcome, rationale)?;
    let label = run.id.0.rsplit(':').next().unwrap_or(&run.id.0);
    match mind.admit(AGENT, label, vec![document])? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(()),
        _ => Err(anyhow!("the run could not be closed")),
    }
}

/// Resolves `run` Recorded if it is still in force: the child may have closed it
/// itself in its flush, and a refused lease was closed already. One try; a mind
/// that cannot be reached leaves it open: the launch that opens the next run closes it.
fn close_if_open(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef, ran: &Result<SelfRunOutcome>, heard: &Heard) -> Result<()> {
    if !holders(mind, mind.instance(), RunTurn::SelfRun)?.iter().any(|held| held.id == *run) {
        return Ok(());
    }
    let how = match ran {
        Ok(outcome) => outcome.to_string(),
        Err(_) => "the actuator failed after the run opened".to_string(),
    };
    let (reason, rationale) = match &heard.concluded {
        Some((head, text)) if !text.is_empty() => (format!("{how}; {head}"), text.as_str()),
        Some((head, _)) => (format!("{how}; {head}"), CARRIER_CLOSED),
        None => (format!("{how}; claude gave no result"), CARRIER_CLOSED),
    };
    admit_closure(mind, clock, run, ResolutionOutcome::Recorded { reason: reason.as_str().into() }, rationale)
}

/// The rationale of a closure that has no result text to give.
const CARRIER_CLOSED: &str = "The Self run's carrier closed the run its claude left open.";

/// Closes a run whose lease was refused for its length: Withdrawn, because no
/// claude ever ran and a misconfiguration is not an empty run; the breaker
/// counts nothing and a launch that finds the unit dead has no open run to close.
fn withdraw_misconfigured(mind: &dyn MindPort, clock: &dyn Clock, run: &PipelineRef) -> Result<()> {
    let outcome = ResolutionOutcome::Withdrawn { reason: "the connector refused the lease as too long".into() };
    admit_closure(mind, clock, run, outcome, "The unit's runtime cap exceeds the connector's lease length, so no claude was started.")
}

/// Carries her one open Self run: one `claude` child from the lease to its
/// end. The request lines go to `out`. A mind that cannot be reached, a
/// connector that cannot be reached or a child that cannot be started is an
/// error; every other end is an outcome. The lease comes first: with none
/// granted no child exists. The lease is dropped when the child is known to
/// have ended, which ends it.
/// `run_cap_s` is the unit's `RuntimeMaxSec`: a connector that finds the lease
/// too long is misconfigured, which is an error naming the mismatch and not a
/// refusal of the run: the run is withdrawn, so the breaker counts nothing.
///
/// Whatever happens after the run is found, it is resolved Recorded exactly once
/// before `carry` returns: the child's result (or that it gave none) in the
/// reason. A resolve that fails is logged and tried no more: the outcome (or the
/// original error) stands, a failed close of a run that ended normally is an
/// error, and the launch that opens the next run closes the run left open.
pub fn carry(mind: &dyn MindPort, clock: &dyn Clock, lender: &dyn Lender, spawner: &dyn Spawner, run_cap_s: u64, out: &mut dyn Write) -> Result<SelfRunOutcome> {
    let Some((run, claims)) = her_run(mind)? else { return Ok(SelfRunOutcome::NoRun) };
    let mut task = task_section(mind, &claims)?;
    let mut told: Vec<Brief> = inbox::derive(mind, &HashSet::new())?;
    let also: Vec<&Brief> = told.iter().filter(|brief| !claims.iter().any(|claim| claim.id.0 == brief.id)).collect();
    if !also.is_empty() {
        task.push_str("

Also open for you:");
        for brief in also {
            task.push_str(&format!("
- follow-up `{}` (campaign `{}`, repo `{}`): {} {}", brief.id, brief.campaign, brief.repo, brief.item, inbox::resolving_sentence(&brief.id)));
        }
    }
    let mut heard = Heard::default();
    let ran = lend_and_drive(mind, clock, lender, spawner, run_cap_s, out, (&run, &task), &mut told, &mut heard);
    let closed = close_if_open(mind, clock, &run, &ran, &heard);
    if closed.is_err() {
        let _ = writeln!(out, "the run could not be closed: the next launch closes it");
    }
    match (ran, closed) {
        (Err(error), _) => Err(error),
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Ok(_), Err(error)) => Err(error.context("the run's process ended but its run could not be closed")),
    }
}

#[allow(clippy::too_many_arguments, reason = "carry's ports, passed on whole")]
fn lend_and_drive(mind: &dyn MindPort, clock: &dyn Clock, lender: &dyn Lender, spawner: &dyn Spawner, run_cap_s: u64, out: &mut dyn Write, (run, task): (&PipelineRef, &str), told: &mut Vec<Brief>, heard: &mut Heard) -> Result<SelfRunOutcome> {
    let started_ms = unix_ms(clock.now());
    let request = LeaseRequest { expires_at_unix_ms: started_ms.saturating_add(run_cap_s.saturating_mul(1000)) };
    let lease = match lender.lend(&request)? {
        Lent::Granted(lease) => lease,
        Lent::Refused(refused) if refused.kind == LeaseRefusalKind::Expired => {
            withdraw_misconfigured(mind, clock, run)?;
            bail!("the connector refused a {run_cap_s} s lease as too long: raise its --lease-max-ms to at least the unit's RuntimeMaxSec ({RUN_CAP_VARIABLE}) plus slack");
        }
        Lent::Refused(refused) => return close_refused(mind, clock, run, refused.kind),
    };
    let child = spawner.spawn(&claude_args(session_id(run)), lease.token())?;
    let mut inbox = SelfInbox::new(mind, run, task, (lease.plan_quota(), started_ms), told, heard);
    drive(clock, child, &mut inbox, run_cap_s, out)
}

/// The Self's side of the session: her briefs, derived from the mind, are what she is told.
struct SelfInbox<'a> {
    mind: &'a dyn MindPort,
    start: String,
    plan: String,
    told: &'a mut Vec<Brief>,
    /// The briefs derived at the last poll that no line has carried yet.
    waiting: Vec<Brief>,
    heard: &'a mut Heard,
}

impl<'a> SelfInbox<'a> {
    fn new(mind: &'a dyn MindPort, run: &PipelineRef, task: &str, (plan_quota, now_ms): (&[PlanQuota], u64), told: &'a mut Vec<Brief>, heard: &'a mut Heard) -> Self {
        let start = format!("{}{task}", start_message(mind.instance(), run, plan_quota, now_ms));
        Self { mind, start, plan: render_plan(plan_quota, now_ms), told, waiting: Vec::new(), heard }
    }
}

impl SessionInbox for SelfInbox<'_> {
    fn start_line(&mut self) -> Result<String> {
        Ok(self.start.clone())
    }

    fn poll_every_s(&self) -> i64 {
        INBOX_POLL_S
    }

    fn poll(&mut self) -> Result<Polled> {
        let held: HashSet<String> = self.told.iter().map(|brief| brief.id.clone()).collect();
        self.waiting = inbox::derive(self.mind, &held)?;
        Ok(Polled { pending: !self.waiting.is_empty(), yield_now: false })
    }

    fn tell(&mut self) -> Result<String> {
        let text = inbox::render(&self.waiting, &self.plan);
        self.told.append(&mut self.waiting);
        Ok(text)
    }

    fn flush_text(&self, why: Why) -> String {
        flush_text(why)
    }

    fn line(&mut self, line: &str) {
        self.heard.hear(line);
    }
}

/// The most of a result's text that reaches the journal.
const RESULT_TEXT_LIMIT: usize = 4096;
/// The most of a result's text that reaches the closing resolution's rationale
/// (a `Para` holds 4000 bytes; the cut marker rides inside the rest).
const RATIONALE_RESULT_LIMIT: usize = 3800;

/// A result event's subtype and error flag when it has them, as one line.
fn result_head(event: &serde_json::Map<String, Value>) -> String {
    let mut head = String::from("result");
    if let Some(subtype) = event.get("subtype").and_then(Value::as_str) {
        head.push_str(&format!(" subtype={subtype}"));
    }
    if let Some(is_error) = event.get("is_error").and_then(Value::as_bool) {
        head.push_str(&format!(" is_error={is_error}"));
    }
    head
}

fn result_text(event: &serde_json::Map<String, Value>) -> &str {
    event.get("result").and_then(Value::as_str).unwrap_or("")
}

/// `text` cut at a character boundary to `limit` bytes, marked when cut.
fn cut_text(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let marker = if end < text.len() { "\n[truncated]" } else { "" };
    format!("{}{marker}", &text[..end])
}

/// A result event as one line: its head, then its text cut to `limit` bytes.
fn describe_result(event: &serde_json::Map<String, Value>, limit: usize) -> String {
    format!("{}: {}", result_head(event), cut_text(result_text(event), limit))
}

/// What `claude` concluded, written to the unit's stdout so it reaches the
/// journal before the transcript goes with the runtime directory.
fn write_result(line: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return Ok(()) };
    if event.get("type").and_then(Value::as_str) != Some("result") {
        return Ok(());
    }
    writeln!(out, "{}", describe_result(&event, RESULT_TEXT_LIMIT))
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
    /// Her sandbox: with it the child runs under bubblewrap (`sandbox_args`), the Persona's; the Self passes none.
    sandbox: Option<PathBuf>,
}

impl ClaudeSpawner {
    /// From the unit's environment: it must name [`SCRATCH_VARIABLE`], and may name [`SEED_VARIABLE`].
    pub fn new(unit_environment: impl IntoIterator<Item = (OsString, OsString)>) -> Result<Self> {
        Self::in_sandbox(unit_environment, SEED_VARIABLE, None)
    }

    /// The same, with the seed named by `seed_variable` and, if `sandbox` is given, the child run under bubblewrap with that directory as its writable home.
    pub fn in_sandbox(unit_environment: impl IntoIterator<Item = (OsString, OsString)>, seed_variable: &str, sandbox: Option<PathBuf>) -> Result<Self> {
        let unit_environment: Vec<(OsString, OsString)> = unit_environment.into_iter().collect();
        let named = |variable: &str| unit_environment.iter().find(|(name, _)| name == variable).map(|(_, value)| PathBuf::from(value));
        // systemd lists several runtime directories colon-separated; the unit names one.
        let scratch = named(SCRATCH_VARIABLE).filter(|path| !path.as_os_str().is_empty()).ok_or_else(|| anyhow!("{SCRATCH_VARIABLE} is not set: the unit needs RuntimeDirectory="))?;
        let seed = named(seed_variable).filter(|path| !path.as_os_str().is_empty());
        Ok(Self { unit_environment, scratch, seed, sandbox })
    }
}

impl Spawner for ClaudeSpawner {
    fn spawn(&self, args: &[String], token: &TokenFd) -> Result<Box<dyn Child>> {
        let config = FreshConfig::create(&self.scratch, self.seed.as_deref())?;
        let Some(sandbox) = &self.sandbox else { return spawn_program(Path::new("claude"), args, token, &self.unit_environment, config) };
        let mut wrapped = sandbox_args(sandbox, &self.scratch);
        wrapped.push("claude".into());
        wrapped.extend(args.iter().cloned());
        spawn_program(Path::new("bwrap"), &wrapped, token, &self.unit_environment, config)
    }
}

/// The bubblewrap arguments up to the program: the whole filesystem read-only, a private `/tmp`
/// and `/run`, the unit's scratch directory and her sandbox writable, her unit's credentials
/// (`/etc/gamecult`, the other `/var/lib` directories, the lease socket under `/run`) hidden, a
/// private process table, and the child dead when the unit's process is. The network is shared: the
/// web, `huginn` and the API are reachable. The boundary is the operating system's, not a pattern list
/// (ruling `eureka-body:ruling:persona-git-personal-sandbox`).
pub(crate) fn sandbox_args(sandbox: &Path, scratch: &Path) -> Vec<String> {
    let (sandbox, scratch) = (sandbox.to_string_lossy(), scratch.to_string_lossy());
    [
        "--ro-bind", "/", "/",
        "--dev", "/dev",
        "--proc", "/proc",
        "--tmpfs", "/tmp",
        "--tmpfs", "/run",
        "--ro-bind-try", "/run/systemd/resolve", "/run/systemd/resolve",
        "--bind", &scratch, &scratch,
        "--tmpfs", "/etc/gamecult",
        "--tmpfs", "/var/lib",
        "--bind", &sandbox, &sandbox,
        "--unshare-pid",
        "--die-with-parent",
        "--new-session",
        "--chdir", &sandbox,
        "--",
    ]
    .map(String::from)
    .to_vec()
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
/// `EUREKA_INSTANCE` and `HUGINN_ENDPOINT` are the `huginn` CLI's whole identity
/// (ruling `eureka-body:ruling:cli-over-mcp-agent-surface`): a name and an address, no secret.
fn child_may_see(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    matches!(name.as_ref(), "PATH" | "LANG" | "TERM" | "EUREKA_INSTANCE" | "HUGINN_ENDPOINT") || name.starts_with("LC_")
}

/// The unit's environment as the child sees it: the allow-list, and nothing else.
fn child_environment(unit: &[(OsString, OsString)]) -> Vec<(&OsString, &OsString)> {
    unit.iter().filter(|(name, _)| child_may_see(name)).map(|(name, value)| (name, value)).collect()
}

/// Whether the unit turned the child's subprocess scrub off: only `MIND_SELF_SCRUB=off` does.
fn scrub_is_off(unit: &[(OsString, OsString)]) -> bool {
    unit.iter().any(|(name, value)| name == SCRUB_VARIABLE && value == "off")
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
    command.env(DISABLE_COMPACT_VARIABLE, "1").env(TOKEN_FD_VARIABLE, inheritable.number().to_string()).env(ENV_SCRUB_VARIABLE, if scrub_is_off(unit) { "0" } else { "1" });
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
pub mod inbox;
pub(crate) mod session;

#[cfg(test)]
mod tests;
