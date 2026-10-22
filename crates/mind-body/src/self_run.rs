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
//! after the grace, or once the prefix has grown by the headroom beyond the
//! prefix the flush was asked at (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`), then SIGTERM and SIGKILL if
//! the child ignores that.
//!
//! The actuator admits nothing, reads neither the brake nor the environment and
//! stops no unit: the gate and the brake hold own those. It never echoes an
//! argument, an endpoint or a stream line. Stream-json is Claude Code's wire, a
//! foreign owner's JSON: it is parsed into `serde_json::Value` here and nothing
//! parsed is stored.

use std::collections::HashSet;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child as OsChild, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration as StdDuration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineRef, RunTurn, Slug};
use huginn_mind::PipelinePageItems;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::launch::{Clock, holders};
use crate::queue::MindPort;

/// The measured Self ceiling: at this prefix the flush is asked for.
pub const CEILING_TOKENS: u64 = 300_000;
/// What the prefix may grow by beyond the prefix the flush was asked at before
/// the actuator interrupts. Relative to the flush, never an absolute limit.
pub const FLUSH_HEADROOM_TOKENS: u64 = 60_000;
/// How long the child may take over the flush before it is interrupted.
pub const FLUSH_GRACE_S: i64 = 15 * 60;
/// How long a child may ignore a signal before the next, stronger one.
pub const ESCALATE_AFTER_S: i64 = 120;
/// A backstop `claude` enforces; the ceiling is reached first.
pub const MAX_TURNS: u32 = 500;
/// The name the start message tells her to admit as.
pub const AGENT: &str = "mind-self-run";
/// The namespace of the session id: the run id is the name in it.
pub const SESSION_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;

/// The longest the loop waits on the child when no deadline is nearer.
const POLL: StdDuration = StdDuration::from_secs(60);

/// What the child is told at the start. The contract with the `mind-self`
/// skill: it names the skill, the instance and the run id.
pub fn start_message(instance: &Slug, run: &PipelineRef) -> String {
    format!(
        "Use the mind-self skill. You are Self for the instance `{}`; your run is `{}`. Admit as faculty SelfFaculty, agent {AGENT}.",
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
/// fork, no model, no permission mode and no settings belong to this actuator.
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
        "--permission-prompts",
        "none",
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

/// What the child's stdout gave in one wait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    Line(String),
    Timeout,
    /// The stream ended: the child is gone or going.
    Closed,
}

/// The running `claude`. A test hands in a scripted one.
pub trait Child {
    /// Writes one line to its input. A dead child's stream closes, which is
    /// how `carry` learns of it, so a failed write is not an error here.
    fn write_line(&mut self, line: &str);
    /// Closes its input; later writes go nowhere.
    fn close_input(&mut self);
    fn next_line(&mut self, timeout: StdDuration) -> Read;
    /// Sends it the signal; whether the signal was delivered.
    fn stop(&mut self, how: Stop) -> bool;
    /// Its exit code, none when a signal ended it.
    fn wait(&mut self) -> Option<i32>;
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
    fn spawn(&self, args: &[String]) -> Result<Box<dyn Child>>;
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
    Ended { cause: Cause, code: Option<i32>, requests: u64, prefix: u64 },
}

impl SelfRunOutcome {
    /// 0 when the child exited 0, 3 when no child was started, else 1 (a
    /// child killed by a signal has no code).
    pub fn exit_status(&self) -> i32 {
        match self {
            Self::NoRun => 3,
            Self::Ended { code: Some(0), .. } => 0,
            Self::Ended { .. } => 1,
        }
    }
}

impl fmt::Display for SelfRunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRun => write!(f, "no run: she has no single open Self run"),
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

/// Carries her one open Self run: one `claude` child from the start message to
/// its end. The request lines go to `out`. A mind that cannot be reached or a
/// child that cannot be started is an error; every other end is an outcome.
pub fn carry(mind: &dyn MindPort, clock: &dyn Clock, spawner: &dyn Spawner, out: &mut dyn Write) -> Result<SelfRunOutcome> {
    let Some((run, budget)) = her_run(mind)? else { return Ok(SelfRunOutcome::NoRun) };
    let mut child = spawner.spawn(&claude_args(session_id(&run), &budget))?;
    let user_line = |text: &str| json!({"type": "user", "message": {"role": "user", "content": text}}).to_string();
    child.write_line(&user_line(&start_message(mind.instance(), &run)));

    let (mut seen, mut requests, mut prefix) = (HashSet::new(), 0u64, 0u64);
    let mut result_seen = false;
    // The prefix and the grace deadline of the flush, once asked for.
    let mut flush: Option<(u64, DateTime<Utc>)> = None;
    // The last signal attempted and when, and the strongest one delivered.
    let mut stopping: Option<(Stop, DateTime<Utc>)> = None;
    let mut delivered: Option<Stop> = None;
    loop {
        // The one deadline the wait must not pass: the next escalation once a signal went out, else the grace.
        let deadline = match (stopping, flush) {
            (Some((_, sent)), _) => Some(sent + Duration::seconds(ESCALATE_AFTER_S)),
            (None, Some((_, grace_ends))) => Some(grace_ends),
            (None, None) => None,
        };
        let wait = deadline.map_or(POLL, |ends| (ends - clock.now()).to_std().unwrap_or_default().min(POLL));
        match child.next_line(wait) {
            Read::Line(line) => match read_usage(&line) {
                // A message id seen before is the same request again (F24).
                Usage::Request { id, prefix: newest } if seen.insert(id.clone()) => {
                    requests += 1;
                    prefix = newest;
                    writeln!(out, "request {requests} prefix {prefix}")?;
                }
                Usage::Result if !result_seen => {
                    result_seen = true;
                    child.close_input();
                }
                _ => {}
            },
            Read::Timeout => {}
            Read::Closed => break,
        }
        if flush.is_none() && !result_seen && prefix >= CEILING_TOKENS {
            child.write_line(&user_line(FLUSH_MESSAGE));
            flush = Some((prefix, clock.now() + Duration::seconds(FLUSH_GRACE_S)));
        }
        // The backstop is relative to the flush point, so it cannot fire in the pass that wrote the flush.
        let due = match (stopping, flush) {
            (Some((_, sent)), _) => clock.now() >= sent + Duration::seconds(ESCALATE_AFTER_S),
            (None, Some((at, grace_ends))) => clock.now() >= grace_ends || at.checked_add(FLUSH_HEADROOM_TOKENS).is_some_and(|limit| prefix >= limit),
            (None, None) => false,
        };
        if due {
            let how = stopping.map_or(Stop::Interrupt, |(last, _)| last.next());
            if child.stop(how) {
                delivered = Some(how);
            }
            stopping = Some((how, clock.now()));
            if how == Stop::Kill {
                // A grandchild can hold the stream open past a dead child: do not wait for its end.
                break;
            }
        }
    }
    if matches!(stopping, Some((Stop::Kill, _))) && delivered != Some(Stop::Kill) {
        return Ok(SelfRunOutcome::Ended { cause: Cause::Unstoppable, code: None, requests, prefix });
    }
    let code = child.wait();
    let cause = match (delivered, flush) {
        (Some(how), _) => Cause::Signalled(how),
        (None, Some(_)) => Cause::FlushRequested,
        (None, None) => Cause::Own,
    };
    Ok(SelfRunOutcome::Ended { cause, code, requests, prefix })
}

/// The real spawner: the `claude` on the unit's `PATH`, with the unit's
/// environment (the library names no variable), its stderr in the journal.
pub struct ClaudeSpawner;

impl Spawner for ClaudeSpawner {
    fn spawn(&self, args: &[String]) -> Result<Box<dyn Child>> {
        spawn_program(Path::new("claude"), args)
    }
}

fn spawn_program(program: &Path, args: &[String]) -> Result<Box<dyn Child>> {
    let mut process = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().context("claude could not be started")?;
    let stdin = process.stdin.take();
    let stdout = process.stdout.take().ok_or_else(|| anyhow!("claude has no stdout"))?;
    let (sender, lines) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).split(b'\n') {
            let Ok(bytes) = line else { return };
            if sender.send(String::from_utf8_lossy(&bytes).into_owned()).is_err() {
                return;
            }
        }
    });
    Ok(Box::new(ClaudeProcess { process, stdin, lines }))
}

struct ClaudeProcess {
    process: OsChild,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
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

    fn next_line(&mut self, timeout: StdDuration) -> Read {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => Read::Line(line),
            Err(RecvTimeoutError::Timeout) => Read::Timeout,
            Err(RecvTimeoutError::Disconnected) => Read::Closed,
        }
    }

    fn stop(&mut self, how: Stop) -> bool {
        // The `kill` program, not a libc call: the library names no FFI.
        let signal = match how {
            Stop::Interrupt => "-INT",
            Stop::Terminate => "-TERM",
            Stop::Kill => return self.process.kill().is_ok(),
        };
        Command::new("kill").args([signal, &self.process.id().to_string()]).status().is_ok_and(|status| status.success())
    }

    fn wait(&mut self) -> Option<i32> {
        self.process.wait().ok().and_then(|status| status.code())
    }
}

#[cfg(test)]
mod tests;
