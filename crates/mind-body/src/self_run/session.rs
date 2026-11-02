//! The one session driver, for both organs: it writes a live `claude` child's stdin,
//! reads its stream and executes what `decide` says. The Self's briefs and the
//! Persona's chat are the two inboxes it serves (the [`SessionInbox`] each
//! organ implements); the loop, the one outstanding write and every timing
//! decision are the same code (rulings `eureka-body:ruling:self-session-one-outstanding-write`
//! and `eureka-body:ruling:persona-session-full-sandbox`).

use std::collections::VecDeque;
use std::io::Write;
use std::time::Duration as StdDuration;

use anyhow::Result;
use chrono::Duration as ChronoDuration;
use serde_json::json;

use super::decide::{Action, Event, State, Why, decide, next_deadline};
use super::{Child, Read, SelfRunOutcome, Usage, read_usage, write_result, writes_hour_cache};
use crate::launch::Clock;

/// What one poll of an inbox found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Polled {
    /// Something waits to be told.
    pub(crate) pending: bool,
    /// The session should end at its next idle: what is oldest and unanswered is not its business.
    pub(crate) yield_now: bool,
}

/// What the session is told, and what its results mean: the organ's side of the loop.
pub(crate) trait SessionInbox {
    /// The first line written, whole.
    fn start_line(&mut self) -> Result<String>;
    /// Seconds between polls.
    fn poll_every_s(&self) -> i64;
    fn poll(&mut self) -> Result<Polled>;
    /// The text of what waits, which is thereby told.
    fn tell(&mut self) -> Result<String>;
    fn flush_text(&mut self, why: Why) -> String;
    /// Every line of the child's stream, before it is read.
    fn line(&mut self, _line: &str) {}
    /// The CLI took one of our lines into a request.
    fn echoed(&mut self) {}
    /// A turn ended with `line`; the `(ok, blocked)` the decision sees. An error is the organ's world failing, not the session's.
    fn result(&mut self, _line: &str, ok: bool, blocked: bool) -> Result<(bool, bool)> {
        Ok((ok, blocked))
    }
}

/// The loop: facts in, one clock read per fact, actions out. The one writer into the session.
pub(crate) fn drive(clock: &dyn Clock, mut child: Box<dyn Child>, inbox: &mut dyn SessionInbox, run_cap_s: u64, out: &mut dyn Write) -> Result<SelfRunOutcome> {
    let user_line = |text: &str| json!({"type": "user", "message": {"role": "user", "content": text}}).to_string();
    child.write_line(&user_line(&inbox.start_line()?));

    let poll_every = ChronoDuration::seconds(inbox.poll_every_s());
    let longest = StdDuration::from_secs(u64::try_from(inbox.poll_every_s()).unwrap_or(1));
    let mut now = clock.now();
    let mut state = State::started(now, run_cap_s);
    let mut polled = now;
    let mut ttl_1h = false;
    let mut closed = false;
    loop {
        let poll_due = polled + poll_every;
        let wake = next_deadline(&state).into_iter().chain((!closed).then_some(poll_due)).min();
        let wait = wake.map_or(longest, |at| (at - now).to_std().unwrap_or_default().min(longest));
        let fact = match child.next(wait) {
            Read::Line(line) => {
                inbox.line(&line);
                write_result(&line, out)?;
                if let Some(hour) = writes_hour_cache(&line) {
                    ttl_1h = hour;
                }
                match read_usage(&line) {
                    Usage::Request { id, prefix } => Event::Request { id, prefix },
                    Usage::Echo => {
                        inbox.echoed();
                        Event::Echo
                    }
                    Usage::Result { ok, blocked } => {
                        let (ok, blocked) = inbox.result(&line, ok, blocked)?;
                        Event::Result { ok, blocked, ttl_1h }
                    }
                    Usage::Other | Usage::Unreadable => Event::Tick,
                }
            }
            Read::Timeout => Event::Tick,
            Read::Eof => Event::StdoutEof,
            Read::Exited(code) => Event::ChildExited(code),
        };
        now = clock.now();
        let mut events = VecDeque::from([fact]);
        if !closed && now >= poll_due {
            polled = now;
            match inbox.poll() {
                Ok(found) => {
                    events.push_back(Event::Inbox { pending: found.pending });
                    if found.yield_now {
                        events.push_back(Event::Yield);
                    }
                }
                Err(_) => writeln!(out, "inbox poll failed")?,
            }
        }
        while let Some(heard_event) = events.pop_front() {
            let counted = state.requests;
            // The decision's inputs, as they stood when the event came.
            let (ttl_s, inbox_pending, idle_s) = (state.ttl_s, state.inbox || matches!(&heard_event, Event::Inbox { pending: true }), state.idle_s(now));
            let (next, action) = decide(state, heard_event, now);
            state = next;
            if state.requests != counted {
                writeln!(out, "request {} prefix {}", state.requests, state.prefix)?;
            }
            match &action {
                Action::None => {}
                Action::Write { tells, flush } => {
                    let mut parts = Vec::new();
                    if *tells {
                        parts.push(inbox.tell()?);
                    }
                    if let Some(why) = *flush {
                        parts.push(inbox.flush_text(why));
                    }
                    child.write_line(&user_line(&parts.join("\n\n")));
                }
                Action::CloseStdin => {
                    child.close_input();
                    closed = true;
                }
                Action::Signal(how) => events.push_front(Event::Signalled { how: *how, delivered: child.stop(*how) }),
                Action::Finish(_) => {}
            }
            if !matches!(action, Action::None | Action::Finish(_)) {
                let idle = idle_s.map_or_else(|| "working".to_string(), |seconds| seconds.to_string());
                writeln!(out, "decision {action:?} prefix {} ttl_s {ttl_s} idle_s {idle} inbox {inbox_pending} written {} consumed {}", state.prefix, state.written, state.consumed)?;
            }
            if let Action::Finish(outcome) = action {
                return Ok(outcome);
            }
        }
    }
}
