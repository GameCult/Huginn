//! The Self-run actuator's lifecycle decision, and nothing else.
//!
//! `decide(state, event, now) -> (state, action)` is the one owner of when the
//! flush is asked for, when the stdin closes, when a signal goes out and when
//! the run is over (ruling `eureka-body:ruling:self-run-decider`). It performs
//! no I/O and reads no clock: the loop in `carry` turns operating-system facts
//! into events, reads the clock once per event, and executes the action. A
//! timing rule is pinned here, by tables and by an enumeration of event orders.
//!
//! The flush's turn begins at the first result after the flush was written (the
//! turn already running ends there; the queued flush is the next one). The
//! headroom counts only requests after that boundary, measured from the prefix
//! the flush was written at, and the grace from the flush write (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`, refined by the decider ruling).
//! At most one action comes out of an event; a deadline that is already due
//! yields its signal at the next event, and `next_deadline` says it is now.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use super::{CEILING_TOKENS, Cause, ESCALATE_AFTER_S, EXIT_DRAIN_S, FLUSH_GRACE_S, FLUSH_HEADROOM_TOKENS, SelfRunOutcome, Stop};

/// What the loop saw, already turned into a fact the decision can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    /// A request of the main thread: its message id and the context it read.
    Request { id: String, prefix: u64 },
    Result,
    /// `waitpid` returned: the child's exit code, none when a signal ended it.
    ChildExited(Option<i32>),
    StdoutEof,
    /// Time passed, or a line that no rule reads.
    Tick,
    /// What the operating system said of the signal the last action sent.
    Signalled { how: Stop, delivered: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    None,
    WriteFlush,
    CloseStdin,
    /// To the child's whole process group.
    Signal(Stop),
    Finish(SelfRunOutcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Flush {
    /// The prefix at which the flush was written: the headroom's base.
    prefix: u64,
    written: DateTime<Utc>,
    /// A result has come since the write: the flush is the next turn.
    turn_begun: bool,
    /// A request of the flush's turn reached the headroom.
    exhausted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct State {
    seen: HashSet<String>,
    pub(super) requests: u64,
    pub(super) prefix: u64,
    result_seen: bool,
    flush: Option<Flush>,
    /// The last signal attempted and when.
    stopping: Option<(Stop, DateTime<Utc>)>,
    /// The strongest signal delivered.
    delivered: Option<Stop>,
    exited: Option<(Option<i32>, DateTime<Utc>)>,
    eof: bool,
    finished: bool,
}

fn outcome(state: &State, cause: Cause, code: Option<i32>) -> SelfRunOutcome {
    SelfRunOutcome::Ended { cause, code, requests: state.requests, prefix: state.prefix }
}

/// How the run ended, by what was done to the child: the strongest signal delivered, else the flush, else nothing.
fn cause(state: &State) -> Cause {
    match (state.delivered, &state.flush) {
        (Some(how), _) => Cause::Signalled(how),
        (None, Some(_)) => Cause::FlushRequested,
        (None, None) => Cause::Own,
    }
}

/// The moment the loop must next be heard at, if any. A moment already past
/// means an action is due now.
pub(super) fn next_deadline(state: &State) -> Option<DateTime<Utc>> {
    if state.finished {
        return None;
    }
    if let Some((_, exited)) = state.exited {
        return (!state.eof).then(|| exited + Duration::seconds(EXIT_DRAIN_S));
    }
    match (&state.stopping, &state.flush) {
        (Some((_, sent)), _) => Some(*sent + Duration::seconds(ESCALATE_AFTER_S)),
        (None, Some(flush)) => Some(flush.written + Duration::seconds(FLUSH_GRACE_S)),
        (None, None) => None,
    }
}

pub(super) fn decide(mut state: State, event: Event, now: DateTime<Utc>) -> (State, Action) {
    if state.finished {
        return (state, Action::None);
    }
    let mut action = Action::None;
    match event {
        Event::Request { id, prefix } => {
            if state.seen.insert(id) {
                state.requests += 1;
                state.prefix = prefix;
                if let Some(flush) = state.flush.as_mut()
                    && flush.turn_begun
                    && flush.prefix.checked_add(FLUSH_HEADROOM_TOKENS).is_some_and(|limit| prefix >= limit)
                {
                    flush.exhausted = true;
                }
                if state.flush.is_none() && !state.result_seen && state.exited.is_none() && prefix >= CEILING_TOKENS {
                    state.flush = Some(Flush { prefix, written: now, turn_begun: false, exhausted: false });
                    action = Action::WriteFlush;
                }
            }
        }
        Event::Result => {
            if !state.result_seen {
                state.result_seen = true;
                if let Some(flush) = state.flush.as_mut() {
                    flush.turn_begun = true;
                }
                action = Action::CloseStdin;
            }
        }
        Event::ChildExited(code) => {
            state.exited.get_or_insert((code, now));
        }
        Event::StdoutEof => state.eof = true,
        Event::Tick => {}
        Event::Signalled { how, delivered } => {
            if delivered {
                state.delivered = Some(how);
            }
        }
    }

    // A child that has exited is never written to or signalled: the run is over
    // once its stream has ended, or once the stream has had its drain.
    if let Some((code, exited)) = state.exited {
        if state.eof || now >= exited + Duration::seconds(EXIT_DRAIN_S) {
            state.finished = true;
            let outcome = outcome(&state, cause(&state), code);
            return (state, Action::Finish(outcome));
        }
        return (state, Action::None);
    }
    if action != Action::None {
        return (state, action);
    }
    if let Some((Stop::Kill, sent)) = state.stopping {
        // SIGKILL went out and the child still has not been reaped: stop waiting for it.
        if now >= sent + Duration::seconds(ESCALATE_AFTER_S) {
            state.finished = true;
            let cause = if state.delivered == Some(Stop::Kill) { Cause::Signalled(Stop::Kill) } else { Cause::Unstoppable };
            let outcome = outcome(&state, cause, None);
            return (state, Action::Finish(outcome));
        }
        return (state, Action::None);
    }
    let how = match (&state.stopping, &state.flush) {
        (Some((last, sent)), _) => (now >= *sent + Duration::seconds(ESCALATE_AFTER_S)).then(|| last.next()),
        (None, Some(flush)) => (flush.exhausted || now >= flush.written + Duration::seconds(FLUSH_GRACE_S)).then_some(Stop::Interrupt),
        (None, None) => None,
    };
    match how {
        Some(how) => {
            state.stopping = Some((how, now));
            (state, Action::Signal(how))
        }
        None => (state, Action::None),
    }
}
#[cfg(test)]
mod tests;
