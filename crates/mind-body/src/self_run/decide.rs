//! The Self-run actuator's lifecycle decision, and nothing else.
//!
//! `decide(state, event, now) -> (state, action)` is the one owner of when the
//! flush is asked for, when the stdin closes, when a signal goes out and when
//! the run is over (ruling `eureka-body:ruling:self-run-decider`). It performs
//! no I/O and reads no clock: the loop in `carry` turns operating-system facts
//! into events, reads the clock once per event, and executes the action. A
//! timing rule is pinned here, by tables and by an enumeration of event orders.
//!
//! The flush's turn begins at the first result after the flush is written (the
//! turn already running ends there; the queued flush is the next one). The
//! headroom counts only requests after that boundary, measured from the prefix
//! the flush was written at, and the grace from the flush write (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`, refined by the decider ruling).
//! At most one action comes out of an event; a deadline that is already due
//! yields its action at the next event, and `next_deadline` says it is now.
//!
//! A result ends a turn, never the session: it marks the session idle and asks
//! for the briefs it answered to be closed. A brief waiting while idle is told.
//! The session ends where its cache would be evicted (ruling
//! `eureka-body:ruling:live-session-floor-cache-evicts`): at the last request's
//! cache TTL less a margin, while no brief waits, the flush is asked, and stdin
//! closes after the flush's own result. Under the run cap it closes at an idle
//! rotation deadline; the PreCompact hook and the cap's margin ask the same
//! flush the ceiling asks.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use super::{
    CEILING_TOKENS, Cause, ESCALATE_AFTER_S, EXIT_DRAIN_S, FLUSH_GRACE_S, FLUSH_HEADROOM_TOKENS, FLUSH_MARGIN_S, ROTATE_MARGIN_S, SelfRunOutcome, Stop, TTL_1H_S, TTL_5M_S,
    TTL_MARGIN_S,
};

/// What the loop saw, already turned into a fact the decision can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    /// A request of the main thread: its message id and the context it read.
    Request { id: String, prefix: u64 },
    /// A turn ended. `ttl_1h`: the last request wrote its cache for an hour, else for five minutes.
    Result { ttl_1h: bool },
    /// A poll of the inbox: whether a brief waits in the spool.
    Inbox { pending: bool },
    /// A user line carrying briefs was written: a turn has begun.
    Told,
    /// The PreCompact hook blocked an auto-compaction.
    PreCompact,
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
    /// Claim the spool and write its briefs as one user line (the session is idle).
    WriteTells,
    /// Resolve the briefs told up to this result.
    CloseBriefs,
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
    /// A turn is running or queued: the start message, a tell and the flush begin one; a result ends it.
    busy: bool,
    last_result_at: Option<DateTime<Utc>>,
    /// The cache lifetime the last result left, in seconds.
    pub(super) ttl_s: i64,
    pub(super) inbox: bool,
    /// When the process started and the unit's cap in seconds; none where no cap is known.
    cap: Option<(DateTime<Utc>, i64)>,
    stdin_closed: bool,
    flush: Option<Flush>,
    /// The last signal attempted and when.
    stopping: Option<(Stop, DateTime<Utc>)>,
    /// The strongest signal delivered.
    delivered: Option<Stop>,
    exited: Option<(Option<i32>, DateTime<Utc>)>,
    eof: bool,
    finished: bool,
}

impl State {
    /// A session whose start message was just written: its first turn is running.
    pub(super) fn started(at: DateTime<Utc>, run_cap_s: u64) -> Self {
        Self { busy: true, cap: Some((at, i64::try_from(run_cap_s).unwrap_or(i64::MAX))), ..Self::default() }
    }

    /// Seconds idle since the last result, none while a turn runs.
    pub(super) fn idle_s(&self, now: DateTime<Utc>) -> Option<i64> {
        (!self.busy).then(|| self.last_result_at.map_or(0, |at| (now - at).num_seconds()))
    }
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
    let signals = match (&state.stopping, &state.flush) {
        (Some((_, sent)), _) => return Some(*sent + Duration::seconds(ESCALATE_AFTER_S)),
        (None, Some(flush)) => Some(flush.written + Duration::seconds(FLUSH_GRACE_S)),
        (None, None) => None,
    };
    signals.into_iter().chain(lifecycle_deadline(state)).min()
}

/// The deadline of the next lifecycle action: the flush margin while a turn
/// runs; while idle, now for a pending tell or a finished flush, else the
/// earlier of the rotation and the cache.
fn lifecycle_deadline(state: &State) -> Option<DateTime<Utc>> {
    if state.stdin_closed {
        return None;
    }
    let rotation = |margin: i64| state.cap.map(|(started, cap)| started + Duration::seconds(cap - margin));
    if state.busy {
        return if state.flush.is_none() { rotation(FLUSH_MARGIN_S) } else { None };
    }
    let last = state.last_result_at?;
    if state.flush.as_ref().is_some_and(|flush| flush.turn_begun) || state.inbox {
        return Some(last);
    }
    [rotation(ROTATE_MARGIN_S), Some(last + Duration::seconds(state.ttl_s - TTL_MARGIN_S))].into_iter().flatten().min()
}

/// The flush is asked once, by the ceiling, the PreCompact hook, the cap's
/// margin or the cache's end; never to a child whose input is closed or that has exited.
fn ask_flush(state: &mut State, now: DateTime<Utc>) -> bool {
    if state.flush.is_some() || state.stdin_closed || state.exited.is_some() {
        return false;
    }
    // Written while idle, its turn begins at once; written mid-turn, it is the next one.
    state.flush = Some(Flush { prefix: state.prefix, written: now, turn_begun: !state.busy, exhausted: false });
    state.busy = true;
    true
}

/// What the session does next on its own. Mid-turn: the flush at the cap's
/// margin. Idle: close after the flush's own result or at the rotation; tell a
/// waiting brief; ask the flush where the cache would be evicted.
fn lifecycle(state: &mut State, now: DateTime<Utc>) -> Action {
    if state.stdin_closed || state.exited.is_some() {
        return Action::None;
    }
    if state.busy {
        let due = state.cap.is_some_and(|(started, cap)| now >= started + Duration::seconds(cap - FLUSH_MARGIN_S));
        return if due && ask_flush(state, now) { Action::WriteFlush } else { Action::None };
    }
    let last = state.last_result_at.unwrap_or(now);
    let rotate = state.cap.is_some_and(|(started, cap)| now >= started + Duration::seconds(cap - ROTATE_MARGIN_S));
    if rotate || state.flush.as_ref().is_some_and(|flush| flush.turn_begun) {
        state.stdin_closed = true;
        return Action::CloseStdin;
    }
    if state.inbox {
        return Action::WriteTells;
    }
    if now >= last + Duration::seconds(state.ttl_s - TTL_MARGIN_S) && ask_flush(state, now) {
        return Action::WriteFlush;
    }
    Action::None
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
                if prefix >= CEILING_TOKENS && ask_flush(&mut state, now) {
                    action = Action::WriteFlush;
                }
            }
        }
        Event::Result { ttl_1h } => {
            state.busy = false;
            state.last_result_at = Some(now);
            state.ttl_s = if ttl_1h { TTL_1H_S } else { TTL_5M_S };
            // The turn already running ends here; a queued flush is the next one.
            if state.flush.as_mut().is_some_and(|flush| !std::mem::replace(&mut flush.turn_begun, true)) {
                state.busy = true;
            }
            action = Action::CloseBriefs;
        }
        Event::Inbox { pending } => state.inbox = pending,
        Event::Told => {
            state.inbox = false;
            state.busy = true;
        }
        Event::PreCompact => {
            if ask_flush(&mut state, now) {
                action = Action::WriteFlush;
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
    if state.stopping.is_none() {
        let next = lifecycle(&mut state, now);
        if next != Action::None {
            return (state, next);
        }
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
