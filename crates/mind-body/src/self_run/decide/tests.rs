//! `decide` is pure, so its rules are pinned here with no child, no mind and no
//! clock: tables, one row per boundary, and an enumeration of event orders that
//! asserts the invariants against an oracle kept from the event history alone.

use std::collections::VecDeque;

use super::*;
use crate::testkit::at;

/// Long enough that no table below reaches the cap by accident.
const CAP_S: u64 = 100_000;
const RESULT: Event = Event::Result { ttl_1h: true };
const SHORT_RESULT: Event = Event::Result { ttl_1h: false };

/// A session whose first turn is running.
fn working() -> State {
    State::started(t(0), CAP_S)
}

fn t(seconds: i64) -> DateTime<Utc> {
    at(seconds * 1000)
}

fn req(id: &str, prefix: u64) -> Event {
    Event::Request { id: id.to_string(), prefix }
}

fn feed(state: State, event: Event, seconds: i64) -> (State, Action) {
    decide(state, event, t(seconds))
}

/// Runs the events in order, each at its own second, and keeps every action.
fn run(events: Vec<(Event, i64)>) -> (State, Vec<Action>) {
    let mut state = working();
    let mut actions = Vec::new();
    for (event, seconds) in events {
        let (next, action) = feed(state, event, seconds);
        state = next;
        actions.push(action);
    }
    (state, actions)
}

fn last(events: Vec<(Event, i64)>) -> Action {
    run(events).1.pop().unwrap()
}

fn delivered(how: Stop) -> Event {
    Event::Signalled { how, delivered: true }
}

fn ended(cause: Cause, code: Option<i32>, requests: u64, prefix: u64) -> Action {
    Action::Finish(SelfRunOutcome::Ended { cause, code, requests, prefix })
}

const GRACE: i64 = FLUSH_GRACE_S;
const ESC: i64 = ESCALATE_AFTER_S;
const LIMIT: u64 = CEILING_TOKENS + FLUSH_HEADROOM_TOKENS;

#[test]
fn the_flush_is_asked_once_at_the_ceiling_and_never_after_a_result_or_an_exit_or_for_a_repeated_request() {
    assert_eq!(last(vec![(req("a", CEILING_TOKENS - 1), 0)]), Action::None);
    assert_eq!(last(vec![(req("a", CEILING_TOKENS), 0)]), Action::WriteFlush);
    assert_eq!(last(vec![(req("a", CEILING_TOKENS + 1), 0)]), Action::WriteFlush);
    // Once.
    assert_eq!(last(vec![(req("a", CEILING_TOKENS), 0), (req("b", CEILING_TOKENS + 5), 1)]), Action::None);
    // A child that is gone is not written to.
    assert_eq!(last(vec![(Event::ChildExited(Some(0)), 0), (req("a", CEILING_TOKENS), 1)]), Action::None);
    // The same message id is the same request again: it neither counts nor decides.
    let (state, actions) = run(vec![(req("a", 10), 0), (req("a", CEILING_TOKENS), 1)]);
    assert_eq!((actions[1].clone(), state.requests, state.prefix), (Action::None, 1, 10));
}

#[test]
fn a_result_ends_a_turn_and_never_closes_the_input_while_a_brief_waits_or_the_cache_is_warm() {
    // The result asks for its briefs to be closed and nothing else; the next pass has nothing to do.
    assert_eq!(last(vec![(RESULT, 0)]), Action::None);
    assert_eq!(last(vec![(RESULT, 0), (Event::Tick, 0)]), Action::None);
    assert_eq!(last(vec![(RESULT, 0), (Event::Tick, 3000)]), Action::None);
    // A waiting brief is told, however long the session has been idle, and the input stays open.
    assert_eq!(last(vec![(RESULT, 0), (Event::Inbox { pending: true }, 10)]), Action::WriteTells);
    assert_eq!(last(vec![(RESULT, 0), (Event::Inbox { pending: true }, 10), (Event::Told, 10), (Event::Tick, 11)]), Action::None);
    let waiting = |seconds: i64| last(vec![(RESULT, 0), (Event::Inbox { pending: true }, 10), (Event::Tick, seconds)]);
    assert_eq!(waiting(TTL_1H_S * 3), Action::WriteTells, "a waiting brief outranks the cache's end");
    // Mid-turn, a brief is the hook's: the actuator writes nothing, and the next result comes.
    assert_eq!(last(vec![(Event::Inbox { pending: true }, 10)]), Action::None);
    assert_eq!(last(vec![(Event::Inbox { pending: true }, 10), (RESULT, 20), (Event::Tick, 20)]), Action::WriteTells);
}

#[test]
fn the_session_ends_where_the_cache_would_be_evicted_with_a_flush_and_closes_after_the_flushs_own_result() {
    let idle_at = |result: Event, seconds: i64| last(vec![(result, 0), (Event::Tick, seconds)]);
    // An hour's cache: the flush at 3540 s, not before.
    assert_eq!(idle_at(RESULT, TTL_1H_S - TTL_MARGIN_S - 1), Action::None);
    assert_eq!(idle_at(RESULT, TTL_1H_S - TTL_MARGIN_S), Action::WriteFlush);
    // A five-minute cache: at 240 s.
    assert_eq!(idle_at(SHORT_RESULT, TTL_5M_S - TTL_MARGIN_S - 1), Action::None);
    assert_eq!(idle_at(SHORT_RESULT, TTL_5M_S - TTL_MARGIN_S), Action::WriteFlush);
    // The deadline the loop waits for is the same moment.
    let (state, _) = run(vec![(RESULT, 5)]);
    assert_eq!(next_deadline(&state), Some(t(5 + TTL_1H_S - TTL_MARGIN_S)));
    // The input stays open through the flush's turn and closes after its result, one pass later.
    let flushed = vec![(RESULT, 0), (Event::Tick, TTL_1H_S - TTL_MARGIN_S)];
    assert_eq!(last([flushed.clone(), vec![(Event::Tick, TTL_1H_S)]].concat()), Action::None);
    let ended_turn = [flushed, vec![(SHORT_RESULT, TTL_1H_S)]].concat();
    assert_eq!(last(ended_turn), Action::CloseStdin);
}

#[test]
fn near_the_cap_an_idle_session_rotates_and_a_working_one_is_asked_for_its_flush() {
    let cap = i64::try_from(CAP_S).unwrap();
    // A result so late that the rotation comes before the cache's end.
    let late = cap - 1300;
    let idle_at = |seconds: i64| last(vec![(RESULT, late), (Event::Tick, seconds)]);
    assert_eq!(idle_at(cap - ROTATE_MARGIN_S - 1), Action::None);
    assert_eq!(idle_at(cap - ROTATE_MARGIN_S), Action::CloseStdin);
    // A waiting brief does not hold the process past the rotation: the next process tells it.
    assert_eq!(last(vec![(RESULT, late), (Event::Inbox { pending: true }, cap - ROTATE_MARGIN_S)]), Action::CloseStdin);
    // Mid-turn the flush is asked at the margin, once, and it keeps its grace and escalation.
    let working_at = |seconds: i64| last(vec![(req("a", 10), 1), (Event::Tick, seconds)]);
    assert_eq!(working_at(cap - FLUSH_MARGIN_S - 1), Action::None);
    assert_eq!(working_at(cap - FLUSH_MARGIN_S), Action::WriteFlush);
    let asked = vec![(req("a", 10), 1), (Event::Tick, cap - FLUSH_MARGIN_S)];
    assert_eq!(last([asked.clone(), vec![(Event::Tick, cap - FLUSH_MARGIN_S + 1)]].concat()), Action::None);
    assert_eq!(last([asked, vec![(Event::Tick, cap - FLUSH_MARGIN_S + GRACE)]].concat()), Action::Signal(Stop::Interrupt));
    let (state, _) = run(vec![(req("a", 10), 1)]);
    assert_eq!(next_deadline(&state), Some(t(cap - FLUSH_MARGIN_S)));
}

#[test]
fn pre_compact_asks_the_same_flush_the_ceiling_asks_once() {
    assert_eq!(last(vec![(req("a", 10), 0), (Event::PreCompact, 1)]), Action::WriteFlush);
    assert_eq!(last(vec![(req("a", 10), 0), (Event::PreCompact, 1), (Event::PreCompact, 2)]), Action::None);
    // The ceiling after it asks nothing more; the ceiling before it leaves nothing for the hook to ask.
    assert_eq!(last(vec![(req("a", 10), 0), (Event::PreCompact, 1), (req("b", CEILING_TOKENS), 2)]), Action::None);
    assert_eq!(last(vec![(req("a", CEILING_TOKENS), 0), (Event::PreCompact, 1)]), Action::None);
    // Its flush has the grace and the headroom of any other.
    assert_eq!(last(vec![(req("a", 10), 0), (Event::PreCompact, 1), (Event::Tick, 1 + GRACE)]), Action::Signal(Stop::Interrupt));
    // A child that is gone is not written to.
    assert_eq!(last(vec![(Event::ChildExited(Some(0)), 0), (Event::PreCompact, 1)]), Action::None);
}

#[test]
fn a_flush_is_never_asked_of_a_session_whose_input_is_closed() {
    let cap = i64::try_from(CAP_S).unwrap();
    let closed = vec![(RESULT, 1), (Event::Tick, cap - ROTATE_MARGIN_S)];
    assert_eq!(last(closed.clone()), Action::CloseStdin);
    assert_eq!(last([closed.clone(), vec![(Event::PreCompact, cap)]].concat()), Action::None);
    assert_eq!(last([closed, vec![(req("b", CEILING_TOKENS), cap)]].concat()), Action::None);
}

#[test]
fn the_grace_runs_from_the_flush_write_to_the_second_and_no_later_request_moves_it() {
    let flushed = |rest: Vec<(Event, i64)>| last([vec![(req("a", 10), 0), (req("b", CEILING_TOKENS), 100)], rest].concat());
    assert_eq!(flushed(vec![(Event::Tick, 100 + GRACE - 1)]), Action::None);
    assert_eq!(flushed(vec![(Event::Tick, 100 + GRACE)]), Action::Signal(Stop::Interrupt));
    assert_eq!(flushed(vec![(Event::Tick, 100 + GRACE + 5000)]), Action::Signal(Stop::Interrupt));
    // Requests after the flush, however late, do not refresh it.
    assert_eq!(flushed(vec![(req("c", CEILING_TOKENS + 1), 100 + GRACE - 10), (Event::Tick, 100 + GRACE)]), Action::Signal(Stop::Interrupt));
    // Not from the first request, not from the start.
    assert_eq!(last(vec![(req("a", 10), 0), (req("b", CEILING_TOKENS), 100), (Event::Tick, GRACE)]), Action::None);
    // The deadline the loop waits for is the same moment.
    let (state, _) = run(vec![(req("a", CEILING_TOKENS), 100)]);
    assert_eq!(next_deadline(&state), Some(t(100 + GRACE)));
    assert_eq!(next_deadline(&State::default()), None);
}

#[test]
fn the_headroom_counts_the_requests_after_the_first_result_after_the_flush_from_the_prefix_of_the_flush() {
    let after_result = |request: u64| last(vec![(req("a", CEILING_TOKENS), 0), (RESULT, 1), (req("b", request), 2)]);
    assert_eq!(after_result(LIMIT - 1), Action::None);
    assert_eq!(after_result(LIMIT), Action::Signal(Stop::Interrupt));
    assert_eq!(after_result(LIMIT + 1), Action::Signal(Stop::Interrupt));
    // Measured from the flush's own prefix, not from the ceiling.
    let high = |request: u64| last(vec![(req("a", 400_000), 0), (RESULT, 1), (req("b", request), 2)]);
    assert_eq!((high(459_999), high(460_000)), (Action::None, Action::Signal(Stop::Interrupt)));
    // The turn already running is not the flush's turn: before a result no request counts.
    assert_eq!(last(vec![(req("a", CEILING_TOKENS), 0), (req("b", LIMIT + 400_000), 1)]), Action::None);
    // A big request before the result is forgotten by the boundary; only those after it count.
    assert_eq!(last(vec![(req("a", CEILING_TOKENS), 0), (req("b", LIMIT + 1), 1), (RESULT, 2), (req("c", LIMIT - 1), 3)]), Action::None);
    // A prefix that saturates leaves no headroom to count: only the grace is left.
    let saturated = |rest: Vec<(Event, i64)>| last([vec![(req("a", u64::MAX), 0), (RESULT, 1), (req("b", u64::MAX), 2)], rest].concat());
    assert_eq!(saturated(vec![]), Action::None);
    assert_eq!(saturated(vec![(Event::Tick, GRACE)]), Action::Signal(Stop::Interrupt));
    // A request counted past the headroom stays counted: a later smaller one does not take the interrupt back.
    let (state, actions) = run(vec![(req("a", CEILING_TOKENS), 0), (RESULT, 1), (req("b", LIMIT), 2)]);
    assert_eq!(actions[2], Action::Signal(Stop::Interrupt));
    assert_eq!(next_deadline(&state), Some(t(2 + ESC)));
}

#[test]
fn the_flush_pass_never_signals_whatever_the_clock_does() {
    // Two events apart by more than the grace: the write is its own pass, the signal comes in the next.
    let (state, action) = feed(working(), req("a", CEILING_TOKENS), 0);
    assert_eq!(action, Action::WriteFlush);
    assert_eq!(feed(state, Event::Tick, 10 * GRACE).1, Action::Signal(Stop::Interrupt));
    // A request that both crosses the ceiling and is far past the headroom writes the flush and nothing else.
    assert_eq!(last(vec![(req("a", u64::MAX), 0)]), Action::WriteFlush);
}

#[test]
fn each_signal_follows_the_last_by_the_bound_and_the_run_ends_when_the_kill_has_had_its_time() {
    let interrupted = vec![(req("a", CEILING_TOKENS), 0), (Event::Tick, GRACE), (delivered(Stop::Interrupt), GRACE)];
    let then = |rest: Vec<(Event, i64)>| last([interrupted.clone(), rest].concat());
    assert_eq!(then(vec![(Event::Tick, GRACE + ESC - 1)]), Action::None);
    assert_eq!(then(vec![(Event::Tick, GRACE + ESC)]), Action::Signal(Stop::Terminate));
    // The bound runs from the last signal, not from the first or from the flush.
    let late_term = [interrupted.clone(), vec![(Event::Tick, GRACE + ESC + 30), (delivered(Stop::Terminate), GRACE + ESC + 30)]].concat();
    let kill_at = |seconds: i64| last([late_term.clone(), vec![(Event::Tick, seconds)]].concat());
    assert_eq!(kill_at(GRACE + 2 * ESC + 29), Action::None);
    assert_eq!(kill_at(GRACE + 2 * ESC + 30), Action::Signal(Stop::Kill));
    // After the kill: the child may still be reaped until the bound, then the run ends without it.
    let killed = [late_term, vec![(Event::Tick, GRACE + 2 * ESC + 30), (delivered(Stop::Kill), GRACE + 2 * ESC + 30)]].concat();
    let after = |seconds: i64| last([killed.clone(), vec![(Event::Tick, seconds)]].concat());
    let end = GRACE + 3 * ESC + 30;
    assert_eq!(after(end - 1), Action::None);
    assert_eq!(after(end), ended(Cause::Signalled(Stop::Kill), None, 1, CEILING_TOKENS));
    // A kill that was not delivered, with the child still there at the bound: Unstoppable, whatever else got through.
    let refused_kill = [killed[..killed.len() - 1].to_vec(), vec![(Event::Signalled { how: Stop::Kill, delivered: false }, GRACE + 2 * ESC + 30)]].concat();
    assert_eq!(last([refused_kill, vec![(Event::Tick, end)]].concat()), ended(Cause::Unstoppable, None, 1, CEILING_TOKENS));
}

#[test]
fn a_kill_that_was_never_delivered_ends_the_run_as_unstoppable_when_nothing_was_delivered_at_all() {
    let all_refused = |how: Stop| Event::Signalled { how, delivered: false };
    let (state, _) = run(vec![
        (req("a", CEILING_TOKENS), 0),
        (Event::Tick, GRACE),
        (all_refused(Stop::Interrupt), GRACE),
        (Event::Tick, GRACE + ESC),
        (all_refused(Stop::Terminate), GRACE + ESC),
        (Event::Tick, GRACE + 2 * ESC),
        (all_refused(Stop::Kill), GRACE + 2 * ESC),
    ]);
    assert_eq!(next_deadline(&state), Some(t(GRACE + 3 * ESC)));
    assert_eq!(feed(state.clone(), Event::Tick, GRACE + 3 * ESC - 1).1, Action::None);
    assert_eq!(feed(state, Event::Tick, GRACE + 3 * ESC).1, ended(Cause::Unstoppable, None, 1, CEILING_TOKENS));
}

#[test]
fn a_child_that_ends_while_the_kill_was_in_flight_ends_the_run_by_its_own_cause_not_unstoppable() {
    // The kill was refused because the child was already gone; the reap is on its way.
    let (state, _) = run(vec![
        (req("a", CEILING_TOKENS), 0),
        (Event::Tick, GRACE),
        (delivered(Stop::Interrupt), GRACE),
        (Event::Tick, GRACE + ESC),
        (Event::Tick, GRACE + 2 * ESC),
        (Event::Signalled { how: Stop::Kill, delivered: false }, GRACE + 2 * ESC),
        (Event::ChildExited(Some(0)), GRACE + 2 * ESC + 1),
    ]);
    assert_eq!(feed(state, Event::StdoutEof, GRACE + 2 * ESC + 1).1, ended(Cause::Signalled(Stop::Interrupt), Some(0), 1, CEILING_TOKENS));
}

#[test]
fn a_child_that_has_exited_is_never_signalled_and_the_run_ends_with_its_stream_or_after_the_drain() {
    // A flush asked for, the grace long past, the child already gone: no signal, only the end.
    let gone = vec![(req("a", CEILING_TOKENS), 0), (Event::ChildExited(Some(0)), 10)];
    let tick = |seconds: i64| last([gone.clone(), vec![(Event::Tick, seconds)]].concat());
    assert_eq!(tick(11), Action::None);
    assert_eq!(tick(10 + EXIT_DRAIN_S - 1), Action::None);
    assert_eq!(tick(10 + EXIT_DRAIN_S), ended(Cause::FlushRequested, Some(0), 1, CEILING_TOKENS));
    assert_eq!(tick(10 + GRACE * 4), ended(Cause::FlushRequested, Some(0), 1, CEILING_TOKENS));
    // With the stream ended too, it ends at once, in either order.
    assert_eq!(last([gone.clone(), vec![(Event::StdoutEof, 11)]].concat()), ended(Cause::FlushRequested, Some(0), 1, CEILING_TOKENS));
    assert_eq!(last(vec![(Event::StdoutEof, 0), (Event::ChildExited(None), 1)]), ended(Cause::Own, None, 0, 0));
    // The end of the stream alone is not the end of the child.
    assert_eq!(last(vec![(Event::StdoutEof, 0), (Event::Tick, 5)]), Action::None);
    // No flush, a signal's cause named by what was delivered, a code from the exit.
    let (state, _) = run(vec![(req("a", CEILING_TOKENS), 0), (Event::Tick, GRACE), (delivered(Stop::Interrupt), GRACE), (Event::ChildExited(Some(130)), GRACE + 1)]);
    assert_eq!(feed(state, Event::StdoutEof, GRACE + 1).1, ended(Cause::Signalled(Stop::Interrupt), Some(130), 1, CEILING_TOKENS));
    // The deadline of a child that has gone is the end of its drain.
    let (state, _) = run(vec![(Event::ChildExited(Some(0)), 7)]);
    assert_eq!(next_deadline(&state), Some(t(7 + EXIT_DRAIN_S)));
}

#[test]
fn without_a_flush_nothing_is_ever_signalled_and_a_finished_run_decides_nothing_more() {
    let quiet = vec![(req("a", 10), 0), (RESULT, 1), (Event::Tick, 100_000)];
    assert!(run(quiet).1.iter().all(|action| !matches!(action, Action::Signal(_))));
    let (state, actions) = run(vec![(Event::ChildExited(Some(3)), 0), (Event::StdoutEof, 0)]);
    assert_eq!(actions[1], ended(Cause::Own, Some(3), 0, 0));
    assert_eq!(next_deadline(&state), None);
    assert_eq!(feed(state, Event::Tick, 10_000).1, Action::None);
}

// ---- every order of events ------------------------------------------------------

/// The facts of the history, kept apart from `State`: what the invariants are asked of.
#[derive(Default)]
struct Oracle {
    ids: Vec<String>,
    /// When the flush was written and the prefix it was written at.
    flush: Option<(i64, u64)>,
    flushes: u32,
    closes: u32,
    /// The flush is the turn that runs now or has run.
    boundary: bool,
    exceeded: bool,
    /// No turn is running.
    idle: bool,
    inbox: bool,
    closed: bool,
    prefix: u64,
    exited: Option<i64>,
    eof: bool,
    signals: Vec<(Stop, i64)>,
    finished: bool,
}

fn check_step(oracle: &mut Oracle, event: &Event, now: i64, action: &Action) {
    assert!(!oracle.finished || *action == Action::None, "an action after the run ended: {action:?} on {event:?}");
    let flush_before = oracle.flush;
    let was_exited = oracle.exited.is_some();
    match event {
        Event::Request { id, prefix } => {
            if !oracle.ids.contains(id) {
                oracle.ids.push(id.clone());
                oracle.prefix = *prefix;
                if oracle.boundary
                    && let Some((_, base)) = oracle.flush
                    && base.checked_add(FLUSH_HEADROOM_TOKENS).is_some_and(|limit| *prefix >= limit)
                {
                    oracle.exceeded = true;
                }
            }
        }
        Event::Result { .. } => {
            oracle.idle = true;
            if oracle.flush.is_some() && !oracle.boundary {
                oracle.boundary = true;
                oracle.idle = false;
            }
        }
        Event::Inbox { pending } => oracle.inbox = *pending,
        Event::Told => {
            oracle.inbox = false;
            oracle.idle = false;
        }
        Event::PreCompact | Event::Tick | Event::Signalled { .. } => {}
        Event::ChildExited(_) => {
            oracle.exited.get_or_insert(now);
        }
        Event::StdoutEof => oracle.eof = true,
    }
    match action {
        Action::None => {}
        Action::WriteFlush => {
            oracle.flushes += 1;
            if let Event::Request { prefix, .. } = event {
                assert!(*prefix >= CEILING_TOKENS, "a flush at {prefix} on a request");
            }
            assert!(flush_before.is_none() && !was_exited && !oracle.closed, "flush twice, to a gone child or to a closed input");
            oracle.flush = Some((now, oracle.prefix));
            // Written while idle, its turn begins at once.
            oracle.boundary = oracle.idle;
            oracle.idle = false;
        }
        Action::WriteTells => {
            assert!(oracle.idle && oracle.inbox && !oracle.closed && !was_exited, "a tell outside idle with a brief waiting");
        }
        Action::CloseStdin => {
            oracle.closes += 1;
            oracle.closed = true;
            let flushed = oracle.flush.is_some() && oracle.boundary;
            let rotation = now >= i64::try_from(CAP_S).unwrap() - ROTATE_MARGIN_S;
            assert!(oracle.idle && !was_exited && (flushed || rotation), "closed on {event:?} flushed {flushed} rotation {rotation}");
        }
        Action::Signal(how) => {
            let (flushed_at, _) = flush_before.expect("a signal before the flush was written");
            assert!(oracle.exited.is_none(), "a signal to a child that already exited");
            match oracle.signals.last() {
                None => {
                    assert_eq!(*how, Stop::Interrupt);
                    assert!(now >= flushed_at + GRACE || oracle.exceeded, "interrupt before the flush had its turn: now {now}, flush {flushed_at}, exceeded {}", oracle.exceeded);
                    assert!(now > flushed_at || oracle.exceeded, "interrupt in the pass that wrote the flush");
                }
                Some((last, sent)) => {
                    assert_eq!(*how, last.next(), "escalation out of order");
                    assert_ne!(*last, Stop::Kill, "a signal after the kill");
                    assert!(now >= sent + ESC, "escalated after {} s, bound {ESC}", now - sent);
                }
            }
            oracle.signals.push((*how, now));
        }
        Action::Finish(outcome) => {
            oracle.finished = true;
            let SelfRunOutcome::Ended { cause, code, .. } = outcome else { panic!("{outcome:?}") };
            match oracle.exited {
                Some(at) => assert!(oracle.eof || now >= at + EXIT_DRAIN_S, "finished before the stream or the drain"),
                None => {
                    let (last, sent) = *oracle.signals.last().expect("finished with the child alive and no signal sent");
                    assert!(last == Stop::Kill && now >= sent + ESC, "finished with the child alive");
                    assert_eq!(*code, None);
                    assert!(matches!(cause, Cause::Signalled(Stop::Kill) | Cause::Unstoppable), "{cause:?}");
                }
            }
            if matches!(cause, Cause::Own) {
                assert!(oracle.flush.is_none() && oracle.signals.is_empty());
            }
            if matches!(cause, Cause::FlushRequested) {
                assert!(oracle.flush.is_some());
            }
        }
    }
    assert!(oracle.flushes <= 1 && oracle.closes <= 1);
    assert!(oracle.signals.len() <= 3);
}

/// What can happen next, each at a time after the last.
#[derive(Clone, Copy, Debug)]
enum Sym {
    Low,
    AtCeiling,
    Huge,
    Repeat,
    Result,
    ShortResult,
    Brief,
    NoBrief,
    PreCompact,
    Exit(Option<i32>),
    Eof,
    Soon,
    Bound,
    Grace,
    Cache,
}

const ALPHABET: [Sym; 16] = [
    Sym::Low,
    Sym::AtCeiling,
    Sym::Huge,
    Sym::Repeat,
    Sym::Result,
    Sym::ShortResult,
    Sym::Brief,
    Sym::NoBrief,
    Sym::PreCompact,
    Sym::Exit(Some(0)),
    Sym::Exit(None),
    Sym::Eof,
    Sym::Soon,
    Sym::Bound,
    Sym::Grace,
    Sym::Cache,
];

/// Which signals the operating system lets through.
#[derive(Clone, Copy, Debug)]
enum Policy {
    All,
    Nothing,
    NoKill,
}

fn play(symbols: &[Sym], policy: Policy) {
    let mut state = working();
    let mut oracle = Oracle::default();
    let mut now = 0i64;
    let mut unique = 0u32;
    let mut events: Vec<Event> = Vec::new();
    for symbol in symbols {
        let (event, dt) = match symbol {
            Sym::Low => (req_new(&mut unique, 1_000), 1),
            Sym::AtCeiling => (req_new(&mut unique, CEILING_TOKENS), 1),
            Sym::Huge => (req_new(&mut unique, LIMIT + 400_000), 1),
            Sym::Repeat => (req("u1", CEILING_TOKENS + 7), 1),
            Sym::Result => (RESULT, 1),
            Sym::ShortResult => (SHORT_RESULT, 1),
            Sym::Brief => (Event::Inbox { pending: true }, 1),
            Sym::NoBrief => (Event::Inbox { pending: false }, 1),
            Sym::PreCompact => (Event::PreCompact, 1),
            Sym::Exit(code) => (Event::ChildExited(*code), 1),
            Sym::Eof => (Event::StdoutEof, 1),
            Sym::Soon => (Event::Tick, 1),
            Sym::Bound => (Event::Tick, ESC + 1),
            Sym::Grace => (Event::Tick, GRACE + 1),
            Sym::Cache => (Event::Tick, TTL_1H_S),
        };
        now += dt;
        events.push(event.clone());
        let mut pending = VecDeque::from([event]);
        while let Some(event) = pending.pop_front() {
            let (after, action) = decide(state, event.clone(), t(now));
            state = after;
            check_step(&mut oracle, &event, now, &action);
            match action {
                Action::Signal(how) => {
                    let delivered = match policy {
                        Policy::All => true,
                        Policy::Nothing => false,
                        Policy::NoKill => how != Stop::Kill,
                    };
                    pending.push_front(Event::Signalled { how, delivered });
                }
                // The loop claims the spool and writes the line: a turn begins.
                Action::WriteTells => pending.push_front(Event::Told),
                _ => {}
            }
        }
    }
    // No spinning: a deadline that has come is never answered with nothing.
    if let Some(deadline) = next_deadline(&state) {
        let at = deadline.max(t(now));
        let (_, action) = decide(state.clone(), Event::Tick, at);
        assert_ne!(action, Action::None, "a due deadline gave no action: {symbols:?} {policy:?}");
    }
    // Once the child has exited and its stream has ended, the run is over, whatever came before.
    for event in [Event::ChildExited(Some(0)), Event::StdoutEof] {
        now += 1;
        let mut next = Some(event);
        while let Some(event) = next.take() {
            let (after, action) = decide(state, event.clone(), t(now));
            state = after;
            check_step(&mut oracle, &event, now, &action);
            if let Action::Signal(how) = action {
                next = Some(Event::Signalled { how, delivered: true });
            }
        }
    }
    assert!(oracle.finished, "no end after the exit and the end of the stream: {symbols:?} {policy:?} {events:?}");
}

fn req_new(unique: &mut u32, prefix: u64) -> Event {
    *unique += 1;
    req(&format!("u{unique}"), prefix)
}

fn enumerate(length: usize, symbols: &mut Vec<Sym>, policy: Policy) {
    if symbols.len() == length {
        play(symbols, policy);
        return;
    }
    for symbol in ALPHABET {
        symbols.push(symbol);
        enumerate(length, symbols, policy);
        symbols.pop();
    }
}

#[test]
fn every_order_of_up_to_four_events_keeps_the_invariants_under_every_policy_of_delivery() {
    for policy in [Policy::All, Policy::Nothing, Policy::NoKill] {
        for length in 0..=4 {
            enumerate(length, &mut Vec::new(), policy);
        }
    }
}

#[test]
fn a_long_random_order_of_events_keeps_the_invariants_and_always_ends_once_the_child_is_gone() {
    // xorshift: the same orders every run.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for round in 0..40_000 {
        let length = 6 + (next() % 30) as usize;
        let symbols: Vec<Sym> = (0..length).map(|_| ALPHABET[(next() % ALPHABET.len() as u64) as usize]).collect();
        play(&symbols, [Policy::All, Policy::Nothing, Policy::NoKill][round % 3]);
    }
}
