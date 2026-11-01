//! `decide` is pure, so its rules are pinned here with no child, no mind and no
//! clock: tables, one row per boundary, a sweep that the signal ladder always
//! ends before the cap, and an enumeration of event orders that asserts the
//! write accounting from the actions alone.

use super::*;
use crate::testkit::at;

/// Long enough that no table below reaches the cap by accident.
const CAP_S: u64 = 100_000;
const OK: Event = Event::Result { ok: true, blocked: false, ttl_1h: true };
const OK_SHORT: Event = Event::Result { ok: true, blocked: false, ttl_1h: false };
const FAILED: Event = Event::Result { ok: false, blocked: false, ttl_1h: true };
const BLOCKED: Event = Event::Result { ok: false, blocked: true, ttl_1h: true };

/// A session whose start message was written and not yet echoed.
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

/// Runs the events in order, each at its own second, from a state, and keeps every action.
fn run_from(mut state: State, events: Vec<(Event, i64)>) -> (State, Vec<Action>) {
    let mut actions = Vec::new();
    for (event, seconds) in events {
        let (next, action) = feed(state, event, seconds);
        state = next;
        actions.push(action);
    }
    (state, actions)
}

fn run(events: Vec<(Event, i64)>) -> (State, Vec<Action>) {
    run_from(working(), events)
}

fn last(events: Vec<(Event, i64)>) -> Action {
    run(events).1.pop().unwrap()
}

/// The one write with only the waiting briefs, and with only the flush.
fn tells() -> Action {
    Action::Write { tells: true, flush: None }
}

fn flush(why: Why) -> Action {
    Action::Write { tells: false, flush: Some(why) }
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

/// The start message echoed and answered: idle at `at`, the cache written for an hour.
fn idle_at(at: i64) -> (State, Vec<Action>) {
    run(vec![(Event::Echo, 0), (req("a", 10), 1), (OK, at)])
}

#[test]
fn the_flush_is_asked_once_at_the_ceiling_and_never_after_a_result_or_an_exit_or_for_a_repeated_request() {
    assert_eq!(last(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS - 1), 0)]), Action::None);
    let (state, actions) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 0), (req("a", CEILING_TOKENS), 1), (req("b", CEILING_TOKENS + 1), 2)]);
    assert_eq!(actions, vec![Action::None, flush(Why::Ceiling), Action::None, Action::None]);
    assert_eq!((state.written, state.requests), (2, 2));
    assert_eq!(last(vec![(Event::ChildExited(Some(0)), 0), (req("a", CEILING_TOKENS), 1)]), Action::None);
}

#[test]
fn what_waits_is_one_write_sent_only_after_the_outstanding_writes_echo_the_briefs_and_the_flush_ask_together() {
    // The start message is not echoed: the ceiling asks the flush and a brief waits, and nothing is written.
    let (state, actions) = run(vec![(req("a", CEILING_TOKENS), 1), (Event::Inbox { pending: true }, 2), (Event::Tick, 3)]);
    assert_eq!(actions, vec![Action::None, Action::None, Action::None]);
    assert_eq!(state.written, 1);
    // The echo frees the writer: one write carries both, and the flush is asked only once.
    let (state, action) = feed(state, Event::Echo, 4);
    assert_eq!(action, Action::Write { tells: true, flush: Some(Why::Ceiling) });
    assert_eq!((state.written, state.consumed), (2, 1));
    assert_eq!(next_deadline(&state), Some(t(1 + GRACE)), "the grace runs from the ask");
    // Briefs alone: the first poll writes, the second finds that write outstanding and waits for its echo.
    let (state, actions) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true }, 1), (Event::Inbox { pending: true }, 2)]);
    assert_eq!(actions, vec![Action::None, tells(), Action::None]);
    assert_eq!(state.written, 2);
    let (state, action) = feed(state, Event::Echo, 3);
    assert_eq!(action, tells(), "the second poll's brief is written at the echo");
    assert_eq!(state.written, 3);
}

#[test]
fn the_start_message_is_one_line_written_and_the_session_is_idle_only_when_every_line_was_echoed_and_no_turn_runs() {
    let (state, _) = run(vec![]);
    assert_eq!((state.written, state.consumed), (1, 0));
    assert_eq!(state.idle_s(t(5)), None, "the start message is not yet echoed");
    let (state, _) = run(vec![(Event::Echo, 0)]);
    assert_eq!(state.idle_s(t(5)), None, "its turn is open");
    let (state, _) = run(vec![(Event::Echo, 0), (OK, 7)]);
    assert_eq!(state.idle_s(t(10)), Some(3));
    // A tell written after the result is outstanding until its echo: not idle.
    let (state, actions) = run(vec![(Event::Echo, 0), (OK, 7), (Event::Inbox { pending: true }, 8)]);
    assert_eq!(actions[2], tells());
    assert_eq!(state.idle_s(t(9)), None);
    // The result of a turn with a queued line outstanding is not idleness.
    let (state, _) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true }, 1), (OK, 2)]);
    assert_eq!(state.idle_s(t(3)), None, "the tell written at 1 has not been echoed");
}

#[test]
fn a_flush_folded_into_the_running_turn_is_answered_by_the_result_after_its_echo() {
    let (state, actions) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1), (Event::Echo, 2), (OK, 3)]);
    assert_eq!(actions, vec![Action::None, flush(Why::Ceiling), Action::None, Action::CloseStdin]);
    assert_eq!(next_deadline(&state), Some(t(1 + GRACE)), "closed: only the flush's grace backstop remains, no lifecycle deadline");
}

#[test]
fn a_flush_queued_behind_the_running_turn_is_answered_by_the_second_result_not_the_first() {
    let (_, actions) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1), (OK, 2), (Event::Echo, 3), (req("b", 11), 4), (OK, 5)]);
    assert_eq!(actions[2], Action::None, "the first result answers the turn already running");
    assert_eq!(actions[5], Action::CloseStdin);
    assert!(actions.iter().all(|action| !matches!(action, Action::Signal(_))), "never a grace interrupt");
    // The queued flush is not idleness: no second flush, no tell, while it waits.
    let (state, _) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1), (OK, 2)]);
    assert_eq!(state.idle_s(t(3)), None);
}

#[test]
fn an_error_result_before_any_answer_or_a_blocked_one_closes_stdin_at_once_with_no_flush() {
    for (result, why) in [(FAILED, "an error before any answer"), (BLOCKED, "blocked")] {
        let (state, actions) = run(vec![(Event::Echo, 0), (result, 1)]);
        assert_eq!(actions[1], Action::CloseStdin, "{why}");
        assert_eq!(state.written, 1, "{why}: no flush was written");
    }
    // Blocked closes even after an answer.
    let (_, actions) = run(vec![(Event::Echo, 0), (OK, 1), (Event::Echo, 2), (BLOCKED, 3)]);
    assert_eq!(actions[3], Action::CloseStdin);
}

#[test]
fn an_error_after_an_ok_answer_ends_the_turn_and_not_the_session() {
    let (state, actions) = run(vec![(Event::Echo, 0), (OK, 1), (Event::Inbox { pending: true }, 2), (Event::Echo, 3), (FAILED, 4)]);
    assert_eq!(actions[4], Action::None);
    assert!(next_deadline(&state).is_some(), "the session waits on its cache still");
    assert_eq!(state.idle_s(t(5)), Some(1));
}

#[test]
fn a_brief_waiting_is_told_mid_turn_or_idle_once_and_never_after_the_flush() {
    // Mid-turn: written at once.
    let (state, actions) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true }, 1)]);
    assert_eq!(actions[1], tells());
    assert_eq!(state.written, 2);
    // The same poll answered again with nothing new is no second tell.
    assert_eq!(feed(state.clone(), Event::Inbox { pending: false }, 2).1, Action::None);
    assert_eq!(feed(state, Event::Tick, 3).1, Action::None);
    // Idle: the same.
    let (_, actions) = idle_at(5);
    assert!(actions.iter().all(|action| *action == Action::None));
    assert_eq!(feed(idle_at(5).0, Event::Inbox { pending: true }, 6).1, tells());
    // After the flush no line is written, a brief waiting or not.
    let (state, _) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 0)]);
    assert_eq!(feed(state.clone(), Event::Inbox { pending: true }, 1).1, Action::None);
    assert_eq!(feed(state, Event::Tick, 2).1, Action::None);
}

#[test]
fn an_idle_session_asks_the_flush_where_the_cache_would_lapse_and_a_waiting_brief_keeps_it_open() {
    let (idle, _) = idle_at(10);
    let lapse = 10 + TTL_1H_S - TTL_MARGIN_S;
    assert_eq!(next_deadline(&idle), Some(t(lapse)));
    assert_eq!(feed(idle.clone(), Event::Tick, lapse - 1).1, Action::None);
    assert_eq!(feed(idle, Event::Tick, lapse).1, flush(Why::Idle));
    // A five-minute cache lapses sooner.
    let (short, _) = run(vec![(Event::Echo, 0), (req("a", 10), 1), (OK_SHORT, 10)]);
    assert_eq!(next_deadline(&short), Some(t(10 + TTL_5M_S - TTL_MARGIN_S)));
    // A brief waiting is told, not flushed, and the lapse is not the end while a line is outstanding.
    let (waiting, _) = run_from(idle_at(10).0, vec![(Event::Inbox { pending: true }, 11)]);
    assert_eq!(feed(waiting, Event::Tick, lapse + 100).1, Action::None);
    // The idle flush closes after its own answer.
    let (_, actions) = run_from(idle_at(10).0, vec![(Event::Tick, lapse), (Event::Echo, lapse + 1), (req("b", 11), lapse + 1), (OK, lapse + 2)]);
    assert_eq!(actions, vec![flush(Why::Idle), Action::None, Action::None, Action::CloseStdin]);
}

#[test]
fn near_the_cap_the_session_is_asked_for_its_flush_idle_or_working_and_closes_after_its_answer() {
    let cap = 10_000_u64;
    let rotate = cap as i64 - ROTATE_MARGIN_S;
    let started = || State::started(t(0), cap);
    // Working: the start message is still being answered.
    let (state, _) = run_from(started(), vec![(Event::Echo, 0)]);
    assert_eq!(next_deadline(&state), Some(t(rotate)));
    assert_eq!(feed(state.clone(), Event::Tick, rotate - 1).1, Action::None);
    assert_eq!(feed(state, Event::Tick, rotate).1, flush(Why::Rotation));
    // Idle, its cache still warm.
    let (idle, _) = run_from(started(), vec![(Event::Echo, 0), (OK, rotate - 100)]);
    assert_eq!(feed(idle.clone(), Event::Tick, rotate).1, flush(Why::Rotation));
    // And the close follows the answer, never a bare close.
    let (_, actions) = run_from(idle, vec![(Event::Tick, rotate), (Event::Echo, rotate + 1), (OK, rotate + 2)]);
    assert_eq!(actions, vec![flush(Why::Rotation), Action::None, Action::CloseStdin]);
}

#[test]
fn a_stdin_is_never_closed_without_a_flush_except_by_a_blocked_result_or_an_error_before_any_answer() {
    let (_, actions) = run(vec![(Event::Echo, 0), (OK, 1), (Event::Echo, 2), (OK, 3), (Event::Tick, CAP_S as i64 / 2)]);
    assert!(actions.iter().all(|action| *action != Action::CloseStdin));
}

#[test]
fn a_flush_is_never_asked_of_a_session_whose_input_is_closed() {
    let (state, _) = run(vec![(Event::Echo, 0), (FAILED, 1)]);
    assert_eq!(feed(state.clone(), req("a", CEILING_TOKENS), 2).1, Action::None);
    assert_eq!(feed(state, Event::Tick, CAP_S as i64).1, Action::None);
}

#[test]
fn the_grace_runs_from_the_flush_ask_to_the_second_and_no_later_request_moves_it() {
    let (state, _) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 100), (req("b", CEILING_TOKENS + 1), 500)]);
    assert_eq!(next_deadline(&state), Some(t(100 + GRACE)));
    assert_eq!(feed(state.clone(), Event::Tick, 100 + GRACE - 1).1, Action::None);
    assert_eq!(feed(state, Event::Tick, 100 + GRACE).1, Action::Signal(Stop::Interrupt));
}

#[test]
fn the_headroom_counts_the_requests_after_the_flushs_echo_from_the_prefix_of_the_flush() {
    let asked = vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1)];
    // Queued flush: requests before its echo never count, however large.
    let before_echo = [asked.clone(), vec![(req("b", u64::MAX), 2)]].concat();
    assert!(run(before_echo).1.iter().all(|action| !matches!(action, Action::Signal(_))));
    let echoed = [asked, vec![(Event::Echo, 3)]].concat();
    let after = |prefix: u64| last([echoed.clone(), vec![(req("c", prefix), 4)]].concat());
    assert_eq!(after(LIMIT - 1), Action::None);
    assert_eq!(after(LIMIT), Action::Signal(Stop::Interrupt));
}

/// Sweep: whatever second the flush is written at, and whatever asked for it, a
/// child that ignores every signal is gone before the unit's cap.
#[test]
fn the_ladder_ends_before_the_cap_whenever_the_flush_was_written() {
    let cap = 5_400_u64;
    let ceiling = |second: i64| run_from(State::started(t(0), cap), vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), second)]);
    let rotation = |second: i64| run_from(State::started(t(0), cap), vec![(Event::Echo, 0), (Event::Tick, second)]);
    // A flush written inside the last LADDER_S cannot finish its ladder: the unit's own cap is the backstop there.
    for second in (0..cap as i64 - LADDER_S).step_by(7) {
        for (state, actions) in [ceiling(second), rotation(second)] {
            if !matches!(actions.last(), Some(Action::Write { flush: Some(_), .. })) {
                continue;
            }
            let mut state = state;
            let mut now = second;
            let finished = loop {
                let Some(at_deadline) = next_deadline(&state) else { break None };
                now = now.max((at_deadline - t(0)).num_seconds());
                let (next, action) = feed(state, Event::Tick, now);
                state = next;
                match action {
                    Action::Signal(how) => state = feed(state, Event::Signalled { how, delivered: true }, now).0,
                    Action::Finish(_) => break Some(now),
                    _ => {}
                }
            };
            assert!(finished.is_some_and(|end| end < cap as i64), "flush at {second}: ended at {finished:?}, cap {cap}");
        }
    }
}

#[test]
fn a_flush_written_late_has_its_grace_clamped_so_the_ladder_still_ends_under_the_cap() {
    let cap = 5_400_i64;
    let late = cap - 100;
    let (state, _) = run_from(State::started(t(0), cap as u64), vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), late)]);
    assert_eq!(next_deadline(&state), Some(t(cap - LADDER_S)), "the interrupt is not the full grace after the write");
}

#[test]
fn the_flush_pass_never_signals_whatever_the_clock_does() {
    // Two events apart by more than the grace: the write is its own pass, the signal comes in the next.
    let (state, _) = feed(working(), Event::Echo, 0);
    let (state, action) = feed(state, req("a", CEILING_TOKENS), 0);
    assert_eq!(action, flush(Why::Ceiling));
    assert_eq!(feed(state, Event::Tick, 10 * GRACE).1, Action::Signal(Stop::Interrupt));
    // A request that both crosses the ceiling and is far past the headroom writes the flush and nothing else.
    assert_eq!(last(vec![(Event::Echo, 0), (req("a", u64::MAX), 0)]), flush(Why::Ceiling));
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
    let quiet = vec![(Event::Echo, 0), (req("a", 10), 0), (OK, 1), (Event::Tick, 100_000)];
    assert!(run(quiet).1.iter().all(|action| !matches!(action, Action::Signal(_))));
    let (state, actions) = run(vec![(Event::ChildExited(Some(3)), 0), (Event::StdoutEof, 0)]);
    assert_eq!(actions[1], ended(Cause::Own, Some(3), 0, 0));
    assert_eq!(next_deadline(&state), None);
    assert_eq!(feed(state, Event::Tick, 10_000).1, Action::None);
}


// ---- every order of events ------------------------------------------------------

/// One thing that can happen next, for the enumeration.
#[derive(Clone, Copy, Debug)]
enum Sym {
    Request,
    Ceiling,
    Echo,
    Ok,
    Failed,
    Waiting,
    Nothing,
    Late,
}

const SYMBOLS: [Sym; 8] = [Sym::Request, Sym::Ceiling, Sym::Echo, Sym::Ok, Sym::Failed, Sym::Waiting, Sym::Nothing, Sym::Late];

/// Plays the symbols and asserts, from the actions alone, the write accounting
/// and the single-writer invariants.
fn play(symbols: &[Sym]) {
    let mut state = working();
    let (mut writes, mut tells, mut flushes, mut closes, mut echoes) = (0_u32, 0_u32, 0_u32, 0_u32, 0_u32);
    let (mut now, mut unique) = (0_i64, 0_u32);
    let mut flushed = false;
    let mut closed = false;
    for sym in symbols {
        now += match sym {
            Sym::Late => 3_000,
            _ => 10,
        };
        unique += 1;
        let event = match sym {
            Sym::Request => req(&format!("r{unique}"), 10),
            Sym::Ceiling => req(&format!("r{unique}"), CEILING_TOKENS),
            Sym::Echo => {
                echoes += 1;
                Event::Echo
            }
            Sym::Ok => OK,
            Sym::Failed => FAILED,
            Sym::Waiting => Event::Inbox { pending: true },
            Sym::Nothing => Event::Inbox { pending: false },
            Sym::Late => Event::Tick,
        };
        let (next, action) = feed(state, event, now);
        state = next;
        match action {
            Action::Write { tells: told, flush } => {
                assert!(!flushed && !closed, "a write after the flush or the close: {symbols:?}");
                writes += 1;
                tells += u32::from(told);
                if flush.is_some() {
                    flushed = true;
                    flushes += 1;
                }
            }
            Action::CloseStdin => {
                assert!(!closed, "stdin closed twice: {symbols:?}");
                closed = true;
                closes += 1;
            }
            Action::Signal(_) => assert!(state.flush.is_some(), "a signal with no flush asked: {symbols:?}"),
            Action::None | Action::Finish(_) => {}
        }
        assert_eq!(state.written, 1 + writes, "every write is counted once: {symbols:?}");
        assert!(state.written <= state.consumed + 1, "two writes outstanding: {symbols:?}");
        assert!(tells <= writes && flushes <= writes);
        assert_eq!(state.consumed, echoes, "every echo is counted once: {symbols:?}");
        assert!(closes <= 1 && flushes <= 1);
    }
}

fn enumerate(length: usize, symbols: &mut Vec<Sym>) {
    play(symbols);
    if symbols.len() == length {
        return;
    }
    for sym in SYMBOLS {
        symbols.push(sym);
        enumerate(length, symbols);
        symbols.pop();
    }
}

#[test]
fn every_order_of_up_to_four_events_keeps_the_write_accounting_and_the_single_flush() {
    enumerate(4, &mut Vec::new());
}

#[test]
fn the_ladder_allowance_is_three_escalations_the_drain_and_a_minute() {
    // The sweep above is only as long as this constant: pin the number the spec derives (3 * 120 + 2 + 60).
    assert_eq!(LADDER_S, 422);
}

#[test]
fn a_yield_closes_stdin_at_idle_only_and_a_session_with_a_line_outstanding_or_a_turn_open_waits() {
    // Outstanding write (the start line has no echo yet): held.
    assert_eq!(feed(working(), Event::Yield, 1).1, Action::None);
    // Mid-turn: held, and it acts at the idle the result brings.
    let (state, actions) = run(vec![(Event::Echo, 0), (Event::Yield, 1)]);
    assert_eq!(actions[1], Action::None);
    assert_eq!(feed(state, OK, 5).1, Action::CloseStdin);
    // Idle: at once, and only once.
    let (state, _) = idle_at(5);
    let (state, action) = feed(state, Event::Yield, 6);
    assert_eq!(action, Action::CloseStdin);
    assert_eq!(feed(state, Event::Tick, 7).1, Action::None, "stdin is closed once");
}

#[test]
fn nothing_is_written_to_a_session_after_it_was_told_to_yield() {
    let (state, _) = idle_at(5);
    let (state, closed) = feed(state, Event::Yield, 5);
    assert_eq!(closed, Action::CloseStdin);
    assert_eq!(feed(state, Event::Inbox { pending: true }, 6).1, Action::None, "closed: text that waits is told to the next session");
}

#[test]
fn a_flush_already_written_ends_the_session_by_itself_and_a_yield_does_not_cut_in() {
    let (state, actions) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1)]);
    assert_eq!(actions[1], flush(Why::Ceiling), "the ceiling's flush is written at once");
    let (_, action) = feed(state, Event::Yield, 3);
    assert_eq!(action, Action::None, "the flush's own answer closes stdin");
}
