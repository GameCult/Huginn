//! The Self-run actuator's lifecycle decision, and nothing else.
//!
//! `decide(state, event, now) -> (state, action)` is the one owner of when the
//! flush is asked for, when the stdin closes, when a signal goes out and when
//! the run is over (ruling `eureka-body:ruling:self-run-decider`). It performs
//! no I/O and reads no clock: the loop in `carry` turns operating-system facts
//! into events, reads the clock once per event, and executes the action. A
//! timing rule is pinned here, by tables and by an enumeration of event orders.
//!
//! Delivery is counted, not guessed, and at most one write is outstanding (ruling
//! `eureka-body:ruling:self-session-one-outstanding-write`): the CLI joins the
//! lines queued behind a request into one echo, so a second line is never
//! written before the first one's echo. Every stdin line the actuator writes (the
//! start message, a write of tells and the flush) increments `written`; every
//! replay echo increments `consumed`. The echo is the point the CLI takes a line
//! into a request, folded into the running turn or queued for the next. What
//! waits (the briefs, the flush ask) is coalesced into the next single write.
//! Idle is derived: no turn open and no write outstanding.
//!
//! The rules, in this order, while stdin is open and no signal is out:
//! (1) a result that is blocked, or an error before any answer, closes stdin: no
//! cache is worth keeping and a flush would fail the same way; (2) the first
//! result after the flush's echo closes stdin, folded or queued alike; (3) the
//! cap less `ROTATE_MARGIN_S`: ask the flush (rotation), idle or mid-turn; (4)
//! idle with no brief waiting, at the last result's cache lifetime less a margin:
//! ask the flush (idle); (5) with nothing outstanding, write what waits: the
//! briefs and the flush ask if there is one, as one line. The ceiling asks the
//! same flush from a request. Compaction is off in the CLI (`DISABLE_COMPACT=1`),
//! so the flush is the only way a session ends short of its cache.
//!
//! The headroom counts the requests after the flush's echo, from the prefix the
//! flush was asked at. The grace interrupt is at the earlier of the grace
//! after the flush write and the cap less `LADDER_S`, so the signal ladder
//! finishes before the unit's cap (ruling
//! `eureka-body:ruling:flush-always-gets-a-turn`). At most one action comes out
//! of an event; a deadline that is already due yields its action at the next
//! event, and `next_deadline` says it is now.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use super::{
    CEILING_TOKENS, Cause, ESCALATE_AFTER_S, EXIT_DRAIN_S, FLUSH_GRACE_S, FLUSH_HEADROOM_TOKENS, LADDER_S, ROTATE_MARGIN_S, SelfRunOutcome, Stop, TTL_1H_S, TTL_5M_S, TTL_MARGIN_S,
};

/// Why the flush was asked: the session's own end, three ways.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Why {
    /// The context reached the ceiling.
    Ceiling,
    /// The unit's cap is near.
    Rotation,
    /// The cache would be evicted and nothing waits.
    Idle,
}

/// What the loop saw, already turned into a fact the decision can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    /// A request of the main thread: its message id and the context it read.
    Request { id: String, prefix: u64 },
    /// The CLI took one of our stdin lines into a request (its replay echo).
    Echo,
    /// A turn ended. `ok`: not an error. `blocked`: the CLI refused the context. `ttl_1h`: the last request wrote its cache for an hour, else five minutes.
    Result { ok: bool, blocked: bool, ttl_1h: bool },
    /// A poll of the mind: whether a brief waits to be told.
    Inbox { pending: bool },
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
    /// The one write: the waiting briefs and the flush ask, coalesced into one user line.
    Write { tells: bool, flush: Option<Why> },
    CloseStdin,
    /// To the child's whole process group.
    Signal(Stop),
    Finish(SelfRunOutcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Flush {
    /// The flush's line number once written (its echo is the `seq`-th); none while it waits for the outstanding write's echo.
    seq: Option<u32>,
    /// The prefix at which the flush was asked: the headroom's base.
    prefix: u64,
    asked: DateTime<Utc>,
    /// Its echo has come: the CLI took it into a request.
    consumed: bool,
    /// A request after its echo reached the headroom.
    exhausted: bool,
    why: Why,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct State {
    seen: HashSet<String>,
    pub(super) requests: u64,
    pub(super) prefix: u64,
    /// Lines written to stdin, and lines the CLI echoed back as taken.
    pub(super) written: u32,
    pub(super) consumed: u32,
    turn_open: bool,
    /// An ok result has come: the session holds an answer worth its cache.
    answered_once: bool,
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
    /// A session whose start message was just written.
    pub(super) fn started(at: DateTime<Utc>, run_cap_s: u64) -> Self {
        Self { written: 1, cap: Some((at, i64::try_from(run_cap_s).unwrap_or(i64::MAX))), ..Self::default() }
    }

    /// A line written whose echo has not come.
    fn outstanding(&self) -> bool {
        self.written > self.consumed
    }

    fn idle(&self) -> bool {
        !self.turn_open && !self.outstanding()
    }

    /// Seconds idle since the last result, none while a line or a turn is outstanding.
    pub(super) fn idle_s(&self, now: DateTime<Utc>) -> Option<i64> {
        self.idle().then(|| self.last_result_at.map_or(0, |at| (now - at).num_seconds()))
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

/// When the flush's grace runs out: the grace after the write, never later than
/// the point the signal ladder needs to finish before the unit's cap.
fn grace_deadline(state: &State, flush: &Flush) -> DateTime<Utc> {
    let grace = flush.asked + Duration::seconds(FLUSH_GRACE_S);
    match state.cap {
        Some((started, cap)) => grace.min(started + Duration::seconds(cap.saturating_sub(LADDER_S))),
        None => grace,
    }
}

/// When the flush is next asked on its own, and why: the cap's margin, or the
/// cache's end while idle with no brief waiting.
fn due(state: &State) -> Option<(DateTime<Utc>, Why)> {
    if state.stdin_closed || state.exited.is_some() || state.flush.is_some() {
        return None;
    }
    let rotation = state.cap.map(|(started, cap)| (started + Duration::seconds(cap.saturating_sub(ROTATE_MARGIN_S)), Why::Rotation));
    let idle = (state.idle() && !state.inbox).then_some(state.last_result_at).flatten().map(|last| (last + Duration::seconds(state.ttl_s - TTL_MARGIN_S), Why::Idle));
    [rotation, idle].into_iter().flatten().min_by_key(|(at, _)| *at)
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
        (None, Some(flush)) => Some(grace_deadline(state, flush)),
        (None, None) => None,
    };
    signals.into_iter().chain(due(state).map(|(at, _)| at)).min()
}

/// The flush is asked once, by the ceiling, the cap's margin or the cache's end;
/// never to a child whose input is closed or that has exited. Asking writes
/// nothing: `send` writes it when no write is outstanding.
fn ask_flush(state: &mut State, now: DateTime<Utc>, why: Why) -> bool {
    if state.flush.is_some() || state.stdin_closed || state.exited.is_some() {
        return false;
    }
    state.flush = Some(Flush { seq: None, prefix: state.prefix, asked: now, consumed: false, exhausted: false, why });
    true
}

/// The lifecycle rules 3 to 5: ask the flush where it is due, then write what waits.
fn lifecycle(state: &mut State, now: DateTime<Utc>) -> Action {
    if let Some((at, why)) = due(state)
        && at <= now
    {
        ask_flush(state, now, why);
    }
    send(state)
}

/// With nothing outstanding, the one write: the waiting briefs (until the flush
/// is written) and the flush, if it has not been written, as one line.
fn send(state: &mut State) -> Action {
    if state.stdin_closed || state.exited.is_some() || state.outstanding() || state.flush.as_ref().is_some_and(|flush| flush.seq.is_some()) {
        return Action::None;
    }
    let flush = state.flush.as_ref().map(|flush| flush.why);
    if !state.inbox && flush.is_none() {
        return Action::None;
    }
    state.written += 1;
    let seq = state.written;
    if let Some(flush) = state.flush.as_mut() {
        flush.seq = Some(seq);
    }
    Action::Write { tells: std::mem::take(&mut state.inbox), flush }
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
                    && flush.consumed
                    && flush.prefix.checked_add(FLUSH_HEADROOM_TOKENS).is_some_and(|limit| prefix >= limit)
                {
                    flush.exhausted = true;
                }
                if prefix >= CEILING_TOKENS {
                    ask_flush(&mut state, now, Why::Ceiling);
                }
            }
        }
        Event::Echo => {
            state.consumed += 1;
            state.turn_open = true;
            let consumed = state.consumed;
            if let Some(flush) = state.flush.as_mut()
                && flush.seq.is_some_and(|seq| consumed >= seq)
            {
                flush.consumed = true;
            }
        }
        Event::Result { ok, blocked, ttl_1h } => {
            state.turn_open = false;
            state.last_result_at = Some(now);
            state.ttl_s = if ttl_1h { TTL_1H_S } else { TTL_5M_S };
            let no_answer = !ok && !state.answered_once;
            state.answered_once |= ok;
            let flush_answered = state.flush.as_ref().is_some_and(|flush| flush.consumed);
            if (blocked || no_answer || flush_answered) && !state.stdin_closed && state.exited.is_none() {
                state.stdin_closed = true;
                action = Action::CloseStdin;
            }
        }
        Event::Inbox { pending } => state.inbox = pending,
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
        (None, Some(flush)) => (flush.exhausted || now >= grace_deadline(&state, flush)).then_some(Stop::Interrupt),
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
