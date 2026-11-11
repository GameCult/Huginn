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
//! is the only path. The session ends where its cache would be evicted (and is
//! then cleared), near the unit's cap, at the context ceiling, or after the flush
//! each of those asks. A clean end inside the cache's life is kept
//! (`decide::warm_until`), and the next process resumes it with
//! `--resume <kept path> --fork-session` while the cache is still warm and the
//! seed is unchanged (`decide::resumes`; rulings
//! `eureka-body:ruling:session-rotate-and-resume` and
//! `eureka-body:ruling:live-session-floor-cache-evicts`). The kept transcript is a
//! cache of the conversation, never memory: her mind and replies stay the truth.
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
//! The actuator admits run closures and the replies of her Self (each result's
//! words, `SelfInbox::result`), nothing else. Every exit of `carry` that leaves
//! her run open resolves it Recorded exactly once (how it ended and the
//! child's last result head in the reason; her words are her replies). It reads neither the
//! brake nor the environment and stops no unit: the gate and the brake hold own those.
//! It never echoes an argument, an endpoint, a
//! connector's refusal text or a stream line. Stream-json is Claude Code's wire, a
//! foreign owner's JSON: it is parsed into `serde_json::Value` here and nothing
//! parsed is stored but a result's text, as a reply.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, PipelineReply, ResolutionOutcome, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelinePageItems};
use serde_json::Value;
use uuid::Uuid;

use crate::launch::{Clock, closure, holders};
use crate::connector::render_plan;
use crate::persona::Conversation;
use crate::lease_client::{LeaseRefusalKind, LeaseRequest, Lender, Lent, TokenFd};
use model_connector::PlanQuota;
use crate::queue::MindPort;

pub(crate) use decide::Why;
#[cfg(test)]
use decide::{State, decide};
#[cfg(test)]
use serde_json::json;
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
/// The variable the child's `huginn` reads its `--session` from: the run's label, the id the start message names.
const SESSION_VARIABLE: &str = "HUGINN_SESSION";
/// The name the start message tells her to admit as.
pub const AGENT: &str = "mind-self-run";
/// The namespace of the session id: the run id is the name in it.
pub const SESSION_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;

/// What the child is told at the start. The contract with the `eureka`
/// skill: it names the skill, the instance and the run id.
pub fn start_message(instance: &Slug, run: &PipelineRef, plan_quota: &[PlanQuota], now_ms: u64) -> String {
    let closing_rule = inbox::CLOSING_RULE;
    format!(
        "Use the eureka skill. You are Self for the instance `{}`; your run is `{}`. Admit as faculty SelfFaculty, agent {AGENT}.\n\nThis session stays open after you answer: briefs from your Persona arrive while you work and between tasks. {closing_rule} Work you take on beyond an answer is a follow-up of your own. What you say when you end a turn reaches your Persona as a reply, and your Persona's answers come as briefs. Briefs reach you one conversation at a time. More briefs of the conversation you are serving may arrive while you work; another conversation's brief is told only once you are idle, with no turn running and no background task of yours still running or waiting for its turn. What you say ending any turn, the turns that report your background tasks included, reaches your Persona in the conversation you are serving, so a long background task keeps other members waiting.\n\n{}",
        instance.0,
        run.id.0,
        render_plan(plan_quota, now_ms)
    )
}

/// What a resumed process is told instead of the start message: the conversation
/// is hers already (the skill is loaded), so only the new run and its plan are new.
pub fn continue_message(run: &PipelineRef, plan_quota: &[PlanQuota], now_ms: u64) -> String {
    format!(
        "Your session continues in a new process under a new run: your run is now `{}`; the previous run is closed. Admit as faculty SelfFaculty, agent {AGENT}. What you say when you end a turn still reaches your Persona as a reply, and your Persona's answers come as briefs.\n\n{}",
        run.id.0,
        render_plan(plan_quota, now_ms)
    )
}

/// The task section of the start message: one section per claim, in claim
/// order. A claimed follow-up gives its id, campaign, repo and `item` verbatim,
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

Task, claimed follow-up `{id}` (campaign `{}`, repo `{}`):
{}", follow_up.campaign.0, follow_up.repo.0, follow_up.item.0));
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

/// What the child is told once, when its session must end, after the reason the flush names for itself.
pub const FLUSH_MESSAGE: &str = "Admit to the mind what it lacks, close your run, and end.";

/// The flush as written: the one message, and what she does with the briefs she holds.
fn flush_text(why: Why) -> String {
    let cause = match why {
        Why::Ceiling => "The context ceiling is reached.",
        Why::Rotation => "This process ends soon: the unit's time is nearly up. If your next brief comes within the hour, the next process continues this conversation.",
        Why::Idle => "This session ends because nothing is waiting and its cache would lapse.",
    };
    format!("{cause} {FLUSH_MESSAGE} Close each brief that is done by the closing rule; a brief whose change is not on its default branch stays open, or is superseded by your own follow-up when it waits on someone else, and the next session continues it.")
}

/// The label of a run, the last segment of its id (`<instance>:run:<label>`): what the opening receipt of every run already carries as its session
/// (`launch::run_label`), so a `huginn` admission and the run's own receipts name one session in one spelling.
fn run_label(run: &PipelineRef) -> &str {
    run.id.0.rsplit(':').next().unwrap_or(&run.id.0)
}

/// The session id of a run: UUIDv5 over the run's id. Derived, never stored.
pub fn session_id(run: &PipelineRef) -> Uuid {
    Uuid::new_v5(&SESSION_NAMESPACE, run.id.0.as_bytes())
}

/// The argument list of the child, and nothing else: no continue flag, no model,
/// no settings and no hooks belong to this actuator. With a kept conversation it
/// appends `--resume <transcript path> --fork-session`: the fork is why the new
/// run's own `--session-id` is accepted beside a resume (the CLI refuses the two
/// without it, probe P2b; ruling `eureka-body:ruling:claude-code-handle-reseeds`),
/// and the path, not the CLI's cwd-slug lookup, is what finds the transcript
/// (probe P2d). It asks the
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
pub fn claude_args(session: Uuid, resume: Option<&Path>) -> Vec<String> {
    // `--disallowedTools` (probe, claude 2.1.312): the init line's tools lose these two. A turn the CLI begins on its own must be a
    // notification turn the actuator tracks (a task's), so no wake of the Self's own is scheduled in the CLI: her heartbeat owns her wakes.
    const DENIED_TOOLS: &str = "ScheduleWakeup,CronCreate";
    let mut args: Vec<String> = [
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
        "--disallowedTools",
        DENIED_TOOLS,
    ]
    .map(String::from)
    .to_vec();
    if let Some(transcript) = resume {
        args.extend(["--resume".to_string(), transcript.to_string_lossy().into_owned(), "--fork-session".to_string()]);
    }
    args
}

/// What a user line starts with when the CLI replays a task notification to the model.
const NOTIFICATION_TAG: &str = "<task-notification>";

/// The names a task-stopping call carries on the stream: the tool's own and its two aliases, each of which the CLI runs as the same stop and the
/// stream shows as the model named it (real claude 2.1.312, fake API).
const STOP_TOOLS: [&str; 3] = ["TaskStop", "KillShell", "KillBash"];

/// The model name on the CLI's own assistant lines, which no request produced.
const SYNTHETIC_MODEL: &str = "<synthetic>";

/// The kinds (`task_type`) whose stop by the session queues nothing for the model, so their stopped notification owes no turn: `local_bash` (a Bash task or a Monitor) is captured, the rest are read from the bundle's kill code
/// (Soul verdict `eureka-body:verdict:cut-self-reply-conversation.s6`). `local_agent`, `monitor_mcp`, `monitor_ws` and any kind not listed or unread queue a message when stopped, so they owe their turn.
const SILENT_STOP_KINDS: [&str; 8] = ["local_bash", "local_workflow", "remote_agent", "mcp_task", "in_process_teammate", "dream", "auto_mode_scan", "local_memory_import"];

/// One line of the child's stream, as far as the actuator reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Usage {
    /// A request of the main thread: the message's id, the context it
    /// read (input, cache read and cache write tokens) and the tasks a stop call (`TaskStop` or its aliases) on its line names (`task_id`, else `shell_id`).
    Request { id: String, prefix: u64, stops: Vec<String> },
    /// A user line of the main thread that the CLI took into a request (a replay echo): ours, never a tool result and never the CLI's
    /// own replay of a task notification.
    Echo,
    /// The CLI began a turn: a `system` init line.
    TurnBegan,
    /// The CLI's level of background tasks (`background_tasks_changed`): `live` the `task_id` of every entry, `main` those of entries with no
    /// parent task that nobody awaits.
    BackgroundTasks { live: Vec<String>, main: Vec<String>, shell: Vec<String> },
    /// A task of the main session went to the background, by `task_id`: a `task_started` with `is_backgrounded` true and no parent task or
    /// awaiter, or a `task_updated` whose patch backgrounds it.
    TaskBackgrounded { id: String, shell: bool },
    /// A task notification, by `task_id`; `stopped`: its status is `stopped`.
    TaskNotified { id: String, stopped: bool },
    /// The CLI replayed a task notification as a user line, taking it into a turn; the `<task-id>` is the task's.
    NotificationTaken { id: String },
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
        // Task lines are read for liveness only: they name a task by its `task_id` and never a conversation.
        Some("system") => {
            let task_id = |object: &serde_json::Map<String, Value>| object.get("task_id").and_then(Value::as_str).map(String::from);
            // A task of the main session that nobody awaits: no parent task, `awaited` not true.
            let main = |object: &serde_json::Map<String, Value>| object.get("parent_task_id").is_none_or(Value::is_null) && object.get("awaited").and_then(Value::as_bool) != Some(true);
            // The kind the CLI gives a task, `task_type`: only a kind in `SILENT_STOP_KINDS` is known to get no turn when we stop it.
            let shell = |object: &serde_json::Map<String, Value>| object.get("task_type").and_then(Value::as_str).is_some_and(|kind| SILENT_STOP_KINDS.contains(&kind));
            match event.get("subtype").and_then(Value::as_str) {
                Some("init") => Usage::TurnBegan,
                Some("background_tasks_changed") => {
                    let entries: Vec<&serde_json::Map<String, Value>> = event.get("tasks").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).collect();
                    Usage::BackgroundTasks {
                        live: entries.iter().copied().filter_map(task_id).collect(),
                        main: entries.iter().copied().filter(|entry| main(entry)).filter_map(task_id).collect(),
                        shell: entries.iter().copied().filter(|entry| shell(entry)).filter_map(task_id).collect(),
                    }
                }
                Some("task_started") if event.get("is_backgrounded").and_then(Value::as_bool) == Some(true) && main(&event) => task_id(&event).map_or(Usage::Other, |id| Usage::TaskBackgrounded { id, shell: shell(&event) }),
                Some("task_updated") if event.get("patch").and_then(|patch| patch.get("is_backgrounded")).and_then(Value::as_bool) == Some(true) => task_id(&event).map_or(Usage::Other, |id| Usage::TaskBackgrounded { id, shell: false }),
                Some("task_notification") => task_id(&event).map_or(Usage::Other, |id| Usage::TaskNotified { id, stopped: event.get("status").and_then(Value::as_str) == Some("stopped") }),
                _ => Usage::Other,
            }
        }
        Some("user") if event.get("isReplay").and_then(Value::as_bool) == Some(true) && event.get("parent_tool_use_id").is_none_or(Value::is_null) => {
            match event.get("message").and_then(|message| message.get("content")).and_then(Value::as_str) {
                // The CLI replays a notification it takes into a turn: it is the CLI's line, never our echo.
                Some(content) if content.starts_with(NOTIFICATION_TAG) => content
                    .split_once("<task-id>")
                    .and_then(|(_, rest)| rest.split_once("</task-id>"))
                    .map_or(Usage::Other, |(id, _)| Usage::NotificationTaken { id: id.to_string() }),
                Some(_) => Usage::Echo,
                // A line with images replays as blocks: a text block first is still our line.
                None if event.get("message").and_then(|message| message.get("content")).and_then(Value::as_array).and_then(|blocks| blocks.first()).and_then(|block| block.get("type")).and_then(Value::as_str) == Some("text") => Usage::Echo,
                None => Usage::Other,
            }
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
            // The tasks a `TaskStop` call on this line stops: the CLI's kill tells the model nothing, so the decision must know it was ours.
            let stops = message
                .and_then(|message| message.get("content"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use") && block.get("name").and_then(Value::as_str).is_some_and(|name| STOP_TOOLS.contains(&name)))
                .filter_map(|block| block.get("input")?.as_object()?.get("task_id").or_else(|| block.get("input")?.get("shell_id"))?.as_str().map(String::from))
                .collect();
            Usage::Request { id: id.to_string(), prefix, stops }
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

/// The organ's one kept conversation: the transcript of the last clean process,
/// the instant its cache runs out, and whether the seed is still the one it was
/// made under. Whether it is resumed is `decide::resumes`, nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The session id the kept transcript was written under.
    pub session: Uuid,
    pub transcript: PathBuf,
    pub warm_until: DateTime<Utc>,
    /// When the record was written; `warm_until` may not lie more than the longest cache lifetime past it.
    pub written_at: DateTime<Utc>,
    pub same_seed: bool,
}

pub trait Spawner {
    /// Starts the child with `token` as its credential descriptor and, when `session` is given, that label
    /// as its `HUGINN_SESSION`: the session its `huginn` admissions are attributed to. A child with no run
    /// label (the Persona's) is given none.
    fn spawn(&self, args: &[String], token: &TokenFd, session: Option<&str>) -> Result<Box<dyn Child>>;

    /// The organ's one kept conversation, if any. A spawner that keeps nothing says none.
    fn kept(&self) -> Option<Kept> {
        None
    }

    /// Keeps the conversation this process ended under `session` until the instant, replacing any other; none clears it.
    fn keep(&self, _keep: Option<(Uuid, DateTime<Utc>)>) -> Result<()> {
        Ok(())
    }
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
    /// `warm_until`: until when the conversation is worth resuming (`decide::warm_until`), none when it is not.
    Ended { cause: Cause, code: Option<i32>, requests: u64, prefix: u64, warm_until: Option<DateTime<Utc>> },
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
            Self::Ended { cause, code, requests, prefix, warm_until } => {
                let cause = match cause {
                    Cause::Own => "own",
                    Cause::FlushRequested => "flush-requested",
                    Cause::Signalled(Stop::Interrupt) => "interrupted",
                    Cause::Signalled(Stop::Terminate) => "terminated",
                    Cause::Signalled(Stop::Kill) => "killed",
                    Cause::Unstoppable => "unstoppable",
                };
                match code {
                    Some(code) => write!(f, "ended: {cause} code {code} requests {requests} prefix {prefix}")?,
                    None => write!(f, "ended: {cause} signal requests {requests} prefix {prefix}")?,
                }
                match warm_until {
                    Some(until) => write!(f, " warm until {}", until.to_rfc3339_opts(SecondsFormat::Secs, true)),
                    None => Ok(()),
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

/// What the stream said at the end of the run: the head of its last result becomes the closure's reason. Her words are her replies, not the closure's.
#[derive(Default)]
struct Heard {
    /// The last result heard, ok or not: its head line.
    concluded: Option<String>,
    /// The session this process resumed; none when it started fresh. Set at the start, read by the closure's reason.
    resumed: Option<Uuid>,
}

impl Heard {
    fn hear(&mut self, line: &str) {
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return };
        if event.get("type").and_then(Value::as_str) == Some("result") {
            self.concluded = Some(result_head(&event));
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
    let reason = match &heard.concluded {
        Some(head) => format!("{how}; {head}"),
        None => format!("{how}; claude gave no result"),
    };
    let reason = match heard.resumed {
        Some(previous) => format!("{reason}; resumed {previous}"),
        None => format!("{reason}; fresh"),
    };
    admit_closure(mind, clock, run, ResolutionOutcome::Recorded { reason: reason.as_str().into() }, CARRIER_CLOSED)
}

/// The rationale of every closure the carrier writes: what she said is in her replies.
const CARRIER_CLOSED: &str = "The Self run's carrier closed the run its claude left open; what she said is in her replies.";

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
    let open = inbox::derive(mind, &HashSet::new())?;
    // One Self turn answers one conversation: the start set is the claimed briefs and the open briefs of the first claimed brief's
    // conversation (with no claimed brief, of the oldest open brief's). A claim no longer open (resolved) has its conversation read by
    // key from the mind, as a reply's briefs are: the gate opens only for it. The rest are derived by poll, one conversation at a time.
    let mut claimed: Vec<(PipelineRef, Option<Conversation>)> = Vec::new();
    for claim in claims {
        let of = if claim.kind != PipelineKind::FollowUp {
            None
        } else if let Some(brief) = open.iter().find(|brief| brief.id == claim.id.0) {
            Some(brief.conversation.clone())
        } else {
            inbox::conversation_by_key(mind, &claim.id.0).with_context(|| format!("the claim `{}` could not be read from the mind", claim.id.0))?
        };
        claimed.push((claim, of));
    }
    let first = claimed.iter().find_map(|(_, of)| of.clone()).or_else(|| open.first().map(|brief| brief.conversation.clone()));
    // A claim of another conversation is not rendered or carried: the start line holds one conversation only.
    let claims: Vec<PipelineRef> = claimed.into_iter().filter(|(claim, of)| claim.kind != PipelineKind::FollowUp || *of == first).map(|(claim, _)| claim).collect();
    // The conversation the session serves from its start: the claim's, else the oldest open brief's, else the public one.
    let conversation = first.clone().unwrap_or(Conversation::Public);
    let mut task = task_section(mind, &claims)?;
    let mut told: Vec<Brief> = open.into_iter().filter(|brief| claims.iter().any(|claim| claim.id.0 == brief.id) || Some(&brief.conversation) == first.as_ref()).collect();
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
    let ran = lend_and_drive(mind, clock, lender, spawner, run_cap_s, out, (&run, &claims, &task, conversation), &mut told, &mut heard);
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
fn lend_and_drive(mind: &dyn MindPort, clock: &dyn Clock, lender: &dyn Lender, spawner: &dyn Spawner, run_cap_s: u64, out: &mut dyn Write, (run, claims, task, conversation): (&PipelineRef, &[PipelineRef], &str, Conversation), told: &mut Vec<Brief>, heard: &mut Heard) -> Result<SelfRunOutcome> {
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
    // The one start decision: a kept conversation is resumed iff `decide::resumes`; any other is cleared before the child starts.
    let kept = spawner.kept();
    let mut resume = kept.clone().filter(|kept| decide::resumes(kept, clock.now()));
    if let Some(warm) = &resume {
        // A resumed start consumes the record before the child exists: the same conversation, kept as already cold (its transcript stays for `--resume`),
        // so a process that never reaches the end keep (a signal, a panic) cannot let the next resume from before the run it was killed in.
        if spawner.keep(Some((warm.session, clock.now()))).is_err() {
            resume = None;
        }
    }
    if resume.is_none() {
        let why = match &kept {
            None => "none kept",
            Some(kept) if !kept.same_seed => "seed changed",
            Some(_) => "cold",
        };
        if spawner.keep(None).is_err() {
            writeln!(out, "session cache could not be cleared")?;
        }
        writeln!(out, "session fresh: {why}")?;
    }
    if let Some(kept) = &resume {
        heard.resumed = Some(kept.session);
        writeln!(out, "session resumed {} warm until {}", kept.session, kept.warm_until.to_rfc3339_opts(SecondsFormat::Secs, true))?;
    }
    let child = spawner.spawn(&claude_args(session_id(run), resume.as_ref().map(|kept| kept.transcript.as_path())), lease.token(), Some(run_label(run)))?;
    let mut inbox = SelfInbox::new(mind, (run, claims), task, (lease.plan_quota(), started_ms), (told, conversation), heard, resume.is_some());
    let ran = drive(clock, child, &mut inbox, run_cap_s, out);
    inbox.admit_unadmitted();
    // The one end decision: a clean end inside the cache's life is kept under this run's session, any other end (an Err included) clears.
    let keep = match &ran {
        Ok(SelfRunOutcome::Ended { warm_until: Some(until), .. }) => Some((session_id(run), *until)),
        _ => None,
    };
    let _ = match (spawner.keep(keep), keep) {
        (Ok(()), Some((_, until))) => writeln!(out, "session kept until {}", until.to_rfc3339_opts(SecondsFormat::Secs, true)),
        (Ok(()), None) => writeln!(out, "session cleared"),
        (Err(_), _) => writeln!(out, "session cache could not be updated"),
    };
    ran
}

/// The Self's side of the session: her briefs, derived from the mind, are what she is told.
///
/// One conversation at a time (ruling `eureka-body:ruling:self-serves-one-conversation-at-a-time`): the session serves one conversation, and
/// every result belongs to it. The actuator changes it only in a write `decide` marks `switch`, which `decide` issues only when the session is
/// idle; a result is never attributed from the order of the stream.
struct SelfInbox<'a> {
    mind: &'a dyn MindPort,
    start: String,
    plan: String,
    told: &'a mut Vec<Brief>,
    /// The briefs derived at the last poll that no line has carried yet.
    waiting: Vec<Brief>,
    heard: &'a mut Heard,
    run: PipelineRef,
    /// The briefs of the lines written that the CLI has not yet taken into a request: the start line's, then each `tell`'s.
    carried: Vec<String>,
    /// The briefs of the lines the CLI took, which the next reply answers.
    echoed: Vec<String>,
    /// The conversation being served: the start line's, then the one each switch writes. Its new briefs may be written at any time; another
    /// conversation's only at the switch.
    serving: Conversation,
    /// The briefs the last reply answered, or the briefs of the line that began this span. A result that took in no brief (a notification
    /// turn, an error, an empty answer) answers them: it belongs to the span being served.
    last_answers: Vec<String>,
    /// The flush was written and the CLI has not yet taken it.
    flush_written: bool,
    /// The CLI took the flush and its result has not come: that result is the session's note to its successor, never a reply to a brief.
    flushing: bool,
    /// Replies made so far; the next is `<run label>-r<n+1>`.
    replies: u32,
    /// Replies the mind did not take: process memory, retried at the next result and once at the end. The journal keeps their text either way.
    unadmitted: Vec<PipelineReply>,
}

impl<'a> SelfInbox<'a> {
    /// The start line carries the claimed follow-ups and every brief already open (`told`), all of `serving`'s conversation. A resumed conversation holds the skill already.
    fn new(mind: &'a dyn MindPort, (run, claims): (&PipelineRef, &[PipelineRef]), task: &str, (plan_quota, now_ms): (&[PlanQuota], u64), (told, serving): (&'a mut Vec<Brief>, Conversation), heard: &'a mut Heard, resumed: bool) -> Self {
        // A resumed conversation holds the skill already: it is told only that it continues under this run, then its task.
        let opening = if resumed { continue_message(run, plan_quota, now_ms) } else { start_message(mind.instance(), run, plan_quota, now_ms) };
        let start = format!("{opening}{task}");
        let mut carried: Vec<String> = claims.iter().filter(|claim| claim.kind == PipelineKind::FollowUp).map(|claim| claim.id.0.clone()).collect();
        for brief in told.iter() {
            if !carried.contains(&brief.id) {
                carried.push(brief.id.clone());
            }
        }
        let last_answers = carried.clone();
        Self { mind, start, plan: render_plan(plan_quota, now_ms), told, waiting: Vec::new(), heard, run: run.clone(), carried, echoed: Vec::new(), serving, last_answers, flush_written: false, flushing: false, replies: 0, unadmitted: Vec::new() }
    }

    /// Admits each reply the mind has not taken, in order; one try each. A refusal or an unreachable mind keeps that reply and every one behind it
    /// (the admission ordinal is the order the Persona hears them in, so none may overtake a refused one) and never ends the session.
    fn admit_unadmitted(&mut self) {
        let label = self.run.id.0.rsplit(':').next().unwrap_or(&self.run.id.0).to_string();
        let mut queue = std::mem::take(&mut self.unadmitted).into_iter();
        for reply in queue.by_ref() {
            let taken = matches!(self.mind.admit(AGENT, &label, vec![PipelineDocument::Reply(reply.clone())]), Ok(PipelineAdmissionOutcome::Committed { .. } | PipelineAdmissionOutcome::AlreadyAdmitted { .. }));
            if !taken {
                self.unadmitted.push(reply);
                break;
            }
        }
        self.unadmitted.extend(queue);
    }

    /// One result of the main thread: with words, one reply answering the briefs the CLI has taken since the last reply (the first 64, in tell
    /// order), else the briefs of the span being served. An error or an empty result makes none and leaves those briefs for the next reply.
    fn reply_to(&mut self, line: &str, ok: bool) {
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return };
        let text = result_text(&event);
        let text = text.as_str();
        let flush = std::mem::take(&mut self.flushing);
        // The flush result is the session's note to its successor, in the journal: it is no reply.
        if ok && !flush && !text.trim().is_empty() {
            let run_label = self.run.id.0.rsplit(':').next().unwrap_or(&self.run.id.0).to_string();
            let taken = std::mem::take(&mut self.echoed);
            let answered: Vec<String> = if taken.is_empty() { self.last_answers.clone() } else { taken.into_iter().take(64).collect() };
            let answers: Vec<PipelineRef> = answered.iter().map(|id| PipelineRef { kind: PipelineKind::FollowUp, id: id.as_str().into() }).collect();
            if !answered.is_empty() {
                self.last_answers = answered;
            }
            // A reply holds a bounded text, so a long result is delivered whole as consecutive replies, in order,
            // each carrying the answers. A piece of only whitespace (the tail after a cut) is no reply.
            for chunk in chunks(text, REPLY_TEXT_LIMIT).filter(|chunk| !chunk.trim().is_empty()) {
                self.replies += 1;
                self.unadmitted.push(PipelineReply {
                    instance: self.mind.instance().clone(),
                    label: format!("{run_label}-r{}", self.replies).as_str().into(),
                    run: self.run.clone(),
                    answers: answers.clone(),
                    text: chunk.into(),
                });
            }
        }
        self.admit_unadmitted();
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
        let pending = self.waiting.iter().any(|brief| brief.conversation == self.serving);
        let other = self.waiting.iter().any(|brief| brief.conversation != self.serving);
        let other_first = self.waiting.first().is_some_and(|brief| brief.conversation != self.serving);
        Ok(Polled { pending, other, other_first, yield_now: false })
    }

    /// The served conversation's waiting briefs; at a switch, those of the oldest waiting brief's conversation, which then is served and starts
    /// its own span: no turn of it inherits the old one's briefs.
    fn tell(&mut self, switch: bool) -> Result<String> {
        let conversation = if switch { self.waiting.first().map_or_else(|| self.serving.clone(), |brief| brief.conversation.clone()) } else { self.serving.clone() };
        let (mut mine, rest): (Vec<Brief>, Vec<Brief>) = std::mem::take(&mut self.waiting).into_iter().partition(|brief| brief.conversation == conversation);
        self.waiting = rest;
        let text = inbox::render(&mine, &self.plan);
        let ids: Vec<String> = mine.iter().map(|brief| brief.id.clone()).collect();
        if conversation != self.serving {
            self.serving = conversation;
            self.echoed.clear();
            self.last_answers = ids.clone();
        }
        self.carried.extend(ids);
        self.told.append(&mut mine);
        Ok(text)
    }

    fn flush_text(&mut self, why: Why) -> String {
        self.flush_written = true;
        flush_text(why)
    }

    fn line(&mut self, line: &str) {
        self.heard.hear(line);
    }

    fn echoed(&mut self) {
        self.echoed.append(&mut self.carried);
        self.flushing |= std::mem::take(&mut self.flush_written);
    }

    fn result(&mut self, line: &str, ok: bool, blocked: bool) -> Result<(bool, bool)> {
        self.reply_to(line, ok);
        Ok((ok, blocked))
    }
}

/// The most of a result's text that one reply holds (a `Para` holds 4000 bytes).
const REPLY_TEXT_LIMIT: usize = 3800;

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

/// A result's words as they leave the actuator, for the journal and for her mind: the one place URL credentials are redacted.
fn result_text(event: &serde_json::Map<String, Value>) -> String {
    redact_url_userinfo(event.get("result").and_then(Value::as_str).unwrap_or(""))
}

/// `text` with the userinfo of every `scheme://userinfo@host` replaced: `scheme://<redacted>@host`. A forge token rides in a remote URL
/// (`https://user:token@host/...`, `https://token@host/...`) and Self prints remotes. Userinfo with a colon is user and password, always
/// redacted. Without a colon it is a bare token on http, https, ftp and ftps, and a plain username on every other scheme (`ssh://git@host`),
/// which stays. The userinfo ends at the last `@` before the authority ends (`/`, `?`, `#`, whitespace, a quote or a bracket), so a password
/// holding `@` leaks no tail. A tiny syntactic guardrail on URL shape, not a reading of meaning; the CLI's own transcript files are not covered.
fn redact_url_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at);
        let scheme_start = before.rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))).map_or(0, |i| i + 1);
        let scheme = before[scheme_start..].to_ascii_lowercase();
        let body = &after[3..];
        let end = body.find(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | '\\')).unwrap_or(body.len());
        let authority = &body[..end];
        out.push_str(before);
        out.push_str("://");
        match authority.rfind('@') {
            Some(i) if !scheme.is_empty() && (authority[..i].contains(':') || matches!(scheme.as_str(), "http" | "https" | "ftp" | "ftps")) => {
                out.push_str("<redacted>@");
                out.push_str(&authority[i + 1..]);
            }
            _ => out.push_str(authority),
        }
        rest = &body[end..];
    }
    out.push_str(rest);
    out
}

/// `text` in consecutive pieces of at most `limit` bytes, each cut at a character boundary; their concatenation is `text`.
fn chunks(text: &str, limit: usize) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut end = rest.len().min(limit);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (piece, tail) = rest.split_at(end);
        rest = tail;
        Some(piece)
    })
}

/// A result event as one line: its head, then its whole text.
fn describe_result(event: &serde_json::Map<String, Value>) -> String {
    format!("{}: {}", result_head(event), result_text(event))
}

/// What `claude` concluded, written to the unit's stdout so it reaches the
/// journal: the transcript is a cache, and the result's words are kept here.
fn write_result(line: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let Ok(Value::Object(event)) = serde_json::from_str::<Value>(line) else { return Ok(()) };
    if event.get("type").and_then(Value::as_str) != Some("result") {
        return Ok(());
    }
    writeln!(out, "{}", describe_result(&event))
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
    /// Where her one kept conversation lives (the second of the unit's `STATE_DIRECTORY` entries, outside her working directory, the first); none keeps nothing, and every start is fresh.
    keep: Option<PathBuf>,
    /// The digest of the seed tree this process copies from (empty without a seed); none when the seed cannot be read, which no kept conversation matches.
    seed_digest: Option<String>,
}

impl ClaudeSpawner {
    /// From the unit's environment: it must name [`SCRATCH_VARIABLE`], and may name [`SEED_VARIABLE`] and
    /// [`STATE_DIRECTORY_VARIABLE`]. With the last, the spawner keeps her last clean conversation for the next process;
    /// without it every start is fresh, and the journal says so.
    pub fn new(unit_environment: impl IntoIterator<Item = (OsString, OsString)>) -> Result<Self> {
        let unit_environment: Vec<(OsString, OsString)> = unit_environment.into_iter().collect();
        // systemd lists several state directories colon-separated: the first is her working directory, the second the kept conversation's own, outside it.
        let keep = unit_environment
            .iter()
            .find(|(name, _)| name == STATE_DIRECTORY_VARIABLE)
            .and_then(|(_, value)| value.to_string_lossy().split(':').nth(1).filter(|second| !second.is_empty()).map(PathBuf::from));
        let mut spawner = Self::in_sandbox(unit_environment, SEED_VARIABLE, None)?;
        spawner.seed_digest = match &spawner.seed {
            Some(seed) => seed_digest(seed).ok(),
            None => Some(String::new()),
        };
        spawner.keep = keep;
        Ok(spawner)
    }

    /// The same, with the seed named by `seed_variable` and, if `sandbox` is given, the child run under bubblewrap with that directory as its writable home.
    pub fn in_sandbox(unit_environment: impl IntoIterator<Item = (OsString, OsString)>, seed_variable: &str, sandbox: Option<PathBuf>) -> Result<Self> {
        let unit_environment: Vec<(OsString, OsString)> = unit_environment.into_iter().collect();
        let named = |variable: &str| unit_environment.iter().find(|(name, _)| name == variable).map(|(_, value)| PathBuf::from(value));
        // systemd lists several runtime directories colon-separated; the unit names one.
        let scratch = named(SCRATCH_VARIABLE).filter(|path| !path.as_os_str().is_empty()).ok_or_else(|| anyhow!("{SCRATCH_VARIABLE} is not set: the unit needs RuntimeDirectory="))?;
        let seed = named(seed_variable).filter(|path| !path.as_os_str().is_empty());
        Ok(Self { unit_environment, scratch, seed, sandbox, keep: None, seed_digest: None })
    }
}

/// The unit's `StateDirectory=`, which systemd names in the environment, colon-separated: her working directory first, then the kept conversation's own directory (ruling `eureka-body:ruling:kept-session-ordinary-separation`).
pub const STATE_DIRECTORY_VARIABLE: &str = "STATE_DIRECTORY";
/// The record of the kept conversation beside its transcript: `{"session", "warm_until", "seed"}`.
const KEPT_RECORD: &str = "kept.json";

/// The digest of a seed tree: sha256 over every file's and link's relative path in sorted order,
/// each with the file's bytes or the link's target, as lowercase hex. A conversation made under
/// one seed is never resumed under another.
fn seed_digest(seed: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    fn walk(root: &Path, dir: &Path, hash: &mut Sha256) -> std::io::Result<()> {
        let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let kind = entry.file_type()?;
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
            if kind.is_dir() {
                walk(root, &path, hash)?;
                continue;
            }
            let (tag, body) = if kind.is_symlink() { (b'l', std::fs::read_link(&path)?.to_string_lossy().into_owned().into_bytes()) } else { (b'f', std::fs::read(&path)?) };
            hash.update([tag]);
            hash.update((relative.len() as u64).to_le_bytes());
            hash.update(relative.as_bytes());
            hash.update((body.len() as u64).to_le_bytes());
            hash.update(&body);
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    walk(seed, seed, &mut hash)?;
    Ok(hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Writes `bytes` to `dir/name` through a `.tmp` and a rename.
fn write_through_tmp(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, dir.join(name))
}

/// Makes the kept directory owner-only if it is not there.
fn make_kept_dir(dir: &Path) -> std::io::Result<()> {
    let mut made = std::fs::DirBuilder::new();
    made.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut made, 0o700);
    made.create(dir)
}

/// Removes every file of the kept directory but `except`.
fn clear_kept_dir(dir: &Path, except: &[String]) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if !except.contains(&entry.file_name().to_string_lossy().into_owned()) && entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

impl Spawner for ClaudeSpawner {
    fn kept(&self) -> Option<Kept> {
        let dir = self.keep.as_ref()?;
        let record: Value = serde_json::from_slice(&std::fs::read(dir.join(KEPT_RECORD)).ok()?).ok()?;
        let session = Uuid::parse_str(record.get("session")?.as_str()?).ok()?;
        let warm_until = DateTime::parse_from_rfc3339(record.get("warm_until")?.as_str()?).ok()?.with_timezone(&Utc);
        let transcript = dir.join(format!("{session}.jsonl"));
        transcript.is_file().then_some(())?;
        let same_seed = self.seed_digest.as_deref() == Some(record.get("seed")?.as_str()?);
        let written_at = DateTime::parse_from_rfc3339(record.get("written_at")?.as_str()?).ok()?.with_timezone(&Utc);
        Some(Kept { session, transcript, warm_until, written_at, same_seed })
    }

    fn keep(&self, keep: Option<(Uuid, DateTime<Utc>)>) -> Result<()> {
        let Some(dir) = &self.keep else { return Ok(()) };
        let kept = keep.and_then(|(session, until)| Some((session, until, self.seed_digest.as_ref()?)));
        let Some((session, until, seed)) = kept.filter(|(session, ..)| dir.join(format!("{session}.jsonl")).is_file()) else {
            // The clear: the record goes first, so no record ever names a transcript that is gone.
            let _ = std::fs::remove_file(dir.join(KEPT_RECORD));
            return clear_kept_dir(dir, &[]).context("the kept conversation could not be cleared");
        };
        let record = serde_json::json!({"session": session.to_string(), "warm_until": until.to_rfc3339_opts(SecondsFormat::Secs, true), "written_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true), "seed": seed});
        write_through_tmp(dir, KEPT_RECORD, record.to_string().as_bytes()).context("the kept conversation's record could not be written")?;
        clear_kept_dir(dir, &[KEPT_RECORD.to_string(), format!("{session}.jsonl")]).context("the conversation before it could not be removed")
    }

    fn spawn(&self, args: &[String], token: &TokenFd, session: Option<&str>) -> Result<Box<dyn Child>> {
        let config = FreshConfig::create(&self.scratch, self.seed.as_deref(), self.keep.clone())?;
        let Some(sandbox) = &self.sandbox else { return spawn_program(Path::new("claude"), args, token, (&self.unit_environment, session), config) };
        let mut wrapped = sandbox_args(sandbox, &self.scratch);
        wrapped.push("claude".into());
        wrapped.extend(args.iter().cloned());
        spawn_program(Path::new("bwrap"), &wrapped, token, (&self.unit_environment, session), config)
    }
}

/// The bubblewrap arguments up to the program: the whole filesystem read-only, a private `/tmp`
/// and `/run`, the unit's scratch directory read-only with only its `claude-config` writable (she
/// cannot create, replace or link a file where the organ writes, such as the instructions file),
/// her sandbox writable, her unit's credentials (`/etc/gamecult`, the other `/var/lib`
/// directories, the lease socket under `/run`) hidden, a private process table, and the child dead
/// when the unit's process is. The network is shared: the web, `huginn` and the API are reachable,
/// and what she reaches on the host is the uid fence's (gamecult-ops `mind-fence.service`), which
/// binds the organ and this sandbox alike. The boundary is the operating system's, not a pattern
/// list (ruling `eureka-body:ruling:persona-git-personal-sandbox`).
pub(crate) fn sandbox_args(sandbox: &Path, scratch: &Path) -> Vec<String> {
    let (sandbox, scratch) = (sandbox.to_string_lossy(), scratch.to_string_lossy());
    let config = scratch.to_string() + "/" + CONFIG_DIR_NAME;
    [
        "--ro-bind", "/", "/",
        "--dev", "/dev",
        "--proc", "/proc",
        "--tmpfs", "/tmp",
        "--tmpfs", "/run",
        "--ro-bind-try", "/run/systemd/resolve", "/run/systemd/resolve",
        "--ro-bind", &scratch, &scratch,
        "--bind", &config, &config,
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
struct FreshConfig {
    dir: PathBuf,
    /// Where the main transcripts go when the config does: the kept directory, none where nothing is kept.
    keep: Option<PathBuf>,
}

impl FreshConfig {
    fn create(scratch: &Path, seed: Option<&Path>, keep: Option<PathBuf>) -> Result<Self> {
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
        let fresh = Self { dir, keep };
        if let Some(seed) = seed {
            copy_tree(seed, &fresh.dir).context("the configuration seed could not be copied")?;
        }
        Ok(fresh)
    }
}

impl Drop for FreshConfig {
    fn drop(&mut self) {
        if let Some(keep) = &self.keep {
            // A conversation that cannot be copied out is a fresh start; nothing here decides whether it is kept.
            let _ = move_out_transcripts(&self.dir, keep);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Copies the main transcripts out of a config directory, `projects/*/*.jsonl`
/// and nothing deeper (never a project's `memory/` or a session's own directory),
/// into `keep` as `<name>`, each through a `.tmp` and a rename.
fn move_out_transcripts(config: &Path, keep: &Path) -> std::io::Result<()> {
    make_kept_dir(keep)?;
    for project in std::fs::read_dir(config.join("projects"))? {
        let project = project?;
        if !project.file_type()?.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(project.path())? {
            let file = file?;
            let path = file.path();
            if file.file_type()?.is_file() && path.extension().is_some_and(|extension| extension == "jsonl") {
                let name = file.file_name().to_string_lossy().into_owned();
                let tmp = keep.join(format!("{name}.tmp"));
                std::fs::copy(&path, &tmp)?;
                std::fs::rename(tmp, keep.join(name))?;
            }
        }
    }
    Ok(())
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
/// Its session, `HUGINN_SESSION`, is set by the spawn from the run, never passed through.
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
fn spawn_program(program: &Path, args: &[String], token: &TokenFd, (unit, session): (&[(OsString, OsString)], Option<&str>), config: FreshConfig) -> Result<Box<dyn Child>> {
    let inheritable = token.inheritable().context("the credential descriptor could not be placed")?;
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    command.env_clear().envs(child_environment(unit));
    command.env("HOME", &config.dir).env(CONFIG_DIR_VARIABLE, &config.dir);
    command.env(DISABLE_COMPACT_VARIABLE, "1").env(TOKEN_FD_VARIABLE, inheritable.number().to_string()).env(ENV_SCRUB_VARIABLE, if scrub_is_off(unit) { "0" } else { "1" });
    // The unit's own `HUGINN_SESSION` is not on the allow-list: the only value the child can hold is the one its run was given.
    if let Some(session) = session {
        command.env(SESSION_VARIABLE, session);
    }
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
