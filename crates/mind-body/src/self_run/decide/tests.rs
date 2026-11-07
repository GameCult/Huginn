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
    Event::Request { id: id.to_string(), prefix, stops: vec![] }
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
    Action::Write { tells: true, switch: false, flush: None }
}

fn flush(why: Why) -> Action {
    Action::Write { tells: false, switch: false, flush: Some(why) }
}

fn delivered(how: Stop) -> Event {
    Event::Signalled { how, delivered: true }
}

fn ended(cause: Cause, code: Option<i32>, requests: u64, prefix: u64) -> Action {
    Action::Finish(SelfRunOutcome::Ended { cause, code, requests, prefix, warm_until: None })
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
    let (state, actions) = run(vec![(req("a", CEILING_TOKENS), 1), (Event::Inbox { pending: true, other: false, other_first: false }, 2), (Event::Tick, 3)]);
    assert_eq!(actions, vec![Action::None, Action::None, Action::None]);
    assert_eq!(state.written, 1);
    // The echo frees the writer: one write carries both, and the flush is asked only once.
    let (state, action) = feed(state, Event::Echo, 4);
    assert_eq!(action, Action::Write { tells: true, switch: false, flush: Some(Why::Ceiling) });
    assert_eq!((state.written, state.consumed), (2, 1));
    assert_eq!(next_deadline(&state), Some(t(1 + GRACE)), "the grace runs from the ask");
    // Briefs alone: the first poll writes, the second finds that write outstanding and waits for its echo.
    let (state, actions) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true, other: false, other_first: false }, 1), (Event::Inbox { pending: true, other: false, other_first: false }, 2)]);
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
    let (state, actions) = run(vec![(Event::Echo, 0), (OK, 7), (Event::Inbox { pending: true, other: false, other_first: false }, 8)]);
    assert_eq!(actions[2], tells());
    assert_eq!(state.idle_s(t(9)), None);
    // The result of a turn with a queued line outstanding is not idleness.
    let (state, _) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true, other: false, other_first: false }, 1), (OK, 2)]);
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
    let (state, actions) = run(vec![(Event::Echo, 0), (OK, 1), (Event::Inbox { pending: true, other: false, other_first: false }, 2), (Event::Echo, 3), (FAILED, 4)]);
    assert_eq!(actions[4], Action::None);
    assert!(next_deadline(&state).is_some(), "the session waits on its cache still");
    assert_eq!(state.idle_s(t(5)), Some(1));
}

#[test]
fn a_brief_waiting_is_told_mid_turn_or_idle_once_and_never_after_the_flush() {
    // Mid-turn: written at once.
    let (state, actions) = run(vec![(Event::Echo, 0), (Event::Inbox { pending: true, other: false, other_first: false }, 1)]);
    assert_eq!(actions[1], tells());
    assert_eq!(state.written, 2);
    // The same poll answered again with nothing new is no second tell.
    assert_eq!(feed(state.clone(), Event::Inbox { pending: false, other: false, other_first: false }, 2).1, Action::None);
    assert_eq!(feed(state, Event::Tick, 3).1, Action::None);
    // Idle: the same.
    let (_, actions) = idle_at(5);
    assert!(actions.iter().all(|action| *action == Action::None));
    assert_eq!(feed(idle_at(5).0, Event::Inbox { pending: true, other: false, other_first: false }, 6).1, tells());
    // After the flush no line is written, a brief waiting or not.
    let (state, _) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 0)]);
    assert_eq!(feed(state.clone(), Event::Inbox { pending: true, other: false, other_first: false }, 1).1, Action::None);
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
    let (waiting, _) = run_from(idle_at(10).0, vec![(Event::Inbox { pending: true, other: false, other_first: false }, 11)]);
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
    Other,
    Late,
}

const SYMBOLS: [Sym; 9] = [Sym::Request, Sym::Ceiling, Sym::Echo, Sym::Ok, Sym::Failed, Sym::Waiting, Sym::Nothing, Sym::Other, Sym::Late];

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
            Sym::Waiting => Event::Inbox { pending: true, other: false, other_first: false },
            Sym::Nothing => Event::Inbox { pending: false, other: false, other_first: false },
            Sym::Other => other_waits(),
            Sym::Late => Event::Tick,
        };
        let (next, action) = feed(state, event, now);
        state = next;
        match action {
            Action::Write { tells: told, switch, flush } => {
                assert!(!flushed && !closed, "a write after the flush or the close: {symbols:?}");
                assert!(!(switch && flush.is_some()), "a switch carried the flush: {symbols:?}");
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
    assert_eq!(feed(state, Event::Inbox { pending: true, other: false, other_first: false }, 6).1, Action::None, "closed: text that waits is told to the next session");
}

#[test]
fn a_flush_already_written_ends_the_session_by_itself_and_a_yield_does_not_cut_in() {
    let (state, actions) = run(vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 1)]);
    assert_eq!(actions[1], flush(Why::Ceiling), "the ceiling's flush is written at once");
    let (_, action) = feed(state, Event::Yield, 3);
    assert_eq!(action, Action::None, "the flush's own answer closes stdin");
}

/// The CLI's level after a background task of the main session came in: it is in the level and the session backgrounded it.
fn level(ids: &[&str]) -> Event {
    let live: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    Event::BackgroundTasks { main: live.clone(), shell: live.clone(), live }
}

/// A shell task (the CLI's `task_type` `local_bash`) of the main session went to the background.
fn backgrounded(id: &str) -> Event {
    Event::TaskBackgrounded { id: id.to_string(), shell: true }
}

/// A task of any other kind (an agent, a workflow, a remote one, or one whose kind the stream did not give).
fn backgrounded_other(id: &str) -> Event {
    Event::TaskBackgrounded { id: id.to_string(), shell: false }
}

fn notified(id: &str) -> Event {
    Event::TaskNotified { id: id.to_string(), stopped: false }
}

fn stopped(id: &str) -> Event {
    Event::TaskNotified { id: id.to_string(), stopped: true }
}

/// A request whose line names a `TaskStop` call on these tasks.
fn stops_of(id: &str, tasks: &[&str]) -> Event {
    Event::Request { id: id.to_string(), prefix: 10, stops: tasks.iter().map(|task| task.to_string()).collect() }
}

fn taken(id: &str) -> Event {
    Event::NotificationTaken { id: id.to_string() }
}

fn switches() -> Action {
    Action::Write { tells: true, switch: true, flush: None }
}

fn other_waits() -> Event {
    Event::Inbox { pending: false, other: true, other_first: false }
}

/// The start message echoed and answered: a Self turn of ours is over, at second 2.
fn answered_start() -> Vec<(Event, i64)> {
    vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1), (OK, 2)]
}

#[test]
fn a_turn_opens_at_the_first_turn_line_echo_or_request_and_the_first_echo_or_request_says_who_began_it() {
    let by = |events: Vec<(Event, i64)>| {
        let (state, _) = run(events);
        (state.turns, state.turn_by)
    };
    assert_eq!(by(vec![(Event::TurnBegan, 0)]), (1, TurnBy::Unknown), "an init line opens a turn nobody has claimed");
    assert_eq!(by(vec![(Event::TurnBegan, 0), (Event::Echo, 0)]), (1, TurnBy::Us), "an echo first: our line began it");
    assert_eq!(by(vec![(Event::TurnBegan, 0), (req("a", 10), 0)]), (1, TurnBy::Cli), "a request first: the CLI began it");
    assert_eq!(by(vec![(Event::TurnBegan, 0), (req("a", 10), 0), (Event::Echo, 1)]), (1, TurnBy::Cli), "a folded echo does not change who began it");
    assert_eq!(by(vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1), (Event::Echo, 2)]), (1, TurnBy::Us));
    assert_eq!(by(vec![(Event::Echo, 0), (req("a", 10), 1)]), (1, TurnBy::Us), "no init line: the echo opens it");
    assert_eq!(by([answered_start(), vec![(Event::TurnBegan, 3), (req("b", 10), 3)]].concat()), (2, TurnBy::Cli), "the CLI's own turn after a result");
    assert_eq!(by([answered_start(), vec![(Event::TurnBegan, 3), (Event::Echo, 3)]].concat()), (2, TurnBy::Us));
}

/// A task of the main session backgrounded in a turn: the CLI's level holds it from the second line on.
fn task_in_level(id: &str, second: i64) -> Vec<(Event, i64)> {
    vec![(backgrounded(id), second), (level(&[id]), second)]
}

/// Its notification and the empty level the CLI says after it.
fn task_ends(id: &str, second: i64) -> Vec<(Event, i64)> {
    vec![(notified(id), second), (level(&[]), second)]
}

#[test]
fn a_session_is_idle_only_with_no_turn_no_write_an_empty_level_and_no_notification_owed_its_turn() {
    let idle = |events: Vec<(Event, i64)>| run(events).0.idle_s(t(1_000)).is_some();
    let turn = |tail: Vec<(Event, i64)>| [vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1)], tail].concat();
    assert!(!idle(vec![(Event::TurnBegan, 0)]), "a turn is open from its init line");
    assert!(idle(answered_start()));
    // The level holds the session after the result of the turn that started the task.
    assert!(!idle([answered_start(), vec![(level(&["t"]), 2)]].concat()), "a task in the level, whatever started it");
    let with_task = |rest: Vec<(Event, i64)>| turn([task_in_level("t", 1), vec![(OK, 2)], rest].concat());
    assert!(!idle(with_task(vec![])), "the task still runs");
    // Its notification is owed a turn even when the level has already emptied: not idle until the CLI's own turn ends.
    assert!(!idle(with_task(task_ends("t", 3))), "owed its turn, the level empty");
    assert!(!idle(with_task([task_ends("t", 3), vec![(Event::TurnBegan, 3), (req("b", 10), 3)]].concat())), "the notification turn runs");
    assert!(idle(with_task([task_ends("t", 3), vec![(Event::TurnBegan, 3), (req("b", 10), 3), (OK, 4)]].concat())), "the CLI's turn ended");
    // A notification of a task never backgrounded (a foreground or unknown one) owes nothing.
    assert!(idle([answered_start(), vec![(notified("unseen"), 3)]].concat()));
    // A notification that arrives inside the turn that started the task is not settled by that turn's result.
    let inside = turn([task_in_level("t", 1), task_ends("t", 1), vec![(OK, 2)]].concat());
    assert!(!idle(inside.clone()), "the result of the starting turn leaves the notification owed");
    // A notification that arrives inside a turn the CLI began is not settled by that turn's own result: its own turn is still to come.
    let nested = turn([task_in_level("t", 1), task_in_level("u", 1), task_ends("t", 1), vec![(OK, 2), (Event::TurnBegan, 3), (req("b", 10), 3), (notified("u"), 3), (level(&[]), 3), (OK, 4)]].concat());
    assert!(!idle(nested.clone()), "the first notification is settled by the turn the CLI began, the second arrived inside it");
    assert!(idle([nested, vec![(Event::TurnBegan, 5), (req("c", 10), 5), (OK, 6)]].concat()), "the CLI's next turn settles it");
    // A turn our line began settles none it did not replay: the hold lasts to the rotation flush.
    assert!(!idle([inside.clone(), vec![(Event::TurnBegan, 3), (Event::Echo, 3), (req("b", 10), 3), (OK, 4)]].concat()));
    // The replay of the notification settles it with the result of the turn that took it, ours or the CLI's.
    assert!(idle([inside.clone(), vec![(Event::TurnBegan, 3), (Event::Echo, 3), (taken("t"), 3), (req("b", 10), 3), (OK, 4)]].concat()));
    assert!(idle([inside, vec![(Event::TurnBegan, 3), (req("b", 10), 3), (OK, 4)]].concat()), "a turn the CLI began after it came");
}

#[test]
fn the_level_is_replaced_whole_and_an_edge_line_never_edits_it() {
    let running = |events: Vec<(Event, i64)>| run(events).0.running.into_iter().collect::<Vec<_>>();
    assert_eq!(running(vec![(level(&["t", "u"]), 1)]), ["t", "u"]);
    assert_eq!(running(vec![(level(&["t", "u"]), 1), (level(&["u"]), 2)]), ["u"], "a second level drops the ids absent from it");
    assert!(running(vec![(level(&["t"]), 1), (level(&[]), 2)]).is_empty(), "an empty level empties it");
    // The edges: a start, an update and a notification say nothing of what runs.
    assert!(running(vec![(backgrounded("t"), 1), (notified("t"), 2), (stopped("t"), 3)]).is_empty(), "nothing enters the level by an edge");
    assert_eq!(running(vec![(level(&["t"]), 1), (backgrounded("u"), 2), (notified("t"), 3), (stopped("u"), 3)]), ["t"], "nothing leaves it by an edge");
    // A subagent's or an awaited task is in the level (it holds idleness) but is not the main session's: it is never backgrounded.
    let (state, _) = run(vec![(Event::BackgroundTasks { live: vec!["t".into(), "s".into()], main: vec!["t".into()], shell: vec![] }, 1)]);
    assert_eq!(state.running.len(), 2);
    assert_eq!(state.backgrounded.into_iter().collect::<Vec<_>>(), ["t"]);
}

#[test]
fn a_notification_owes_a_turn_when_the_task_was_backgrounded_unless_the_session_stopped_it() {
    let owes = |events: Vec<(Event, i64)>| !run([answered_start(), events].concat()).0.owed.is_empty();
    assert!(owes(vec![(backgrounded("t"), 3), (notified("t"), 4)]), "a backgrounded task's notification owes its turn");
    assert!(owes(vec![(level(&["t"]), 3), (notified("t"), 4)]), "the level's main entry is backgrounded");
    assert!(!owes(vec![(notified("t"), 4)]), "a task never backgrounded (foreground, or unknown) owes nothing");
    assert!(!owes(vec![(Event::BackgroundTasks { live: vec!["s".into()], main: vec![], shell: vec![] }, 3), (notified("s"), 4)]), "a subagent's or an awaited task owes nothing");
    // A stop of ours: the model was told nothing but the call's result, so no turn follows.
    assert!(!owes(vec![(backgrounded("t"), 3), (stops_of("b", &["t"]), 3), (stopped("t"), 4)]), "a stop the session asked for owes nothing");
    // A stop the CLI made on its own, a stop of another task, and a task that finished before our stop took hold, owe their turns.
    assert!(owes(vec![(backgrounded("t"), 3), (stopped("t"), 4)]), "a stop the session did not ask for owes its turn");
    assert!(owes(vec![(backgrounded("t"), 3), (stops_of("b", &["other"]), 3), (stopped("t"), 4)]));
    assert!(owes(vec![(backgrounded_other("t"), 3), (stops_of("b", &["t"]), 3), (stopped("t"), 4)]), "a stop of ours owes nothing only for a shell task: any other kind gets its turn");
    assert!(!owes(vec![(Event::BackgroundTasks { live: vec!["t".into()], main: vec!["t".into()], shell: vec!["t".into()] }, 3), (stops_of("b", &["t"]), 3), (stopped("t"), 4)]), "a kind read only from the level's entry is as silent as one read from the start line");
    assert!(owes(vec![(Event::BackgroundTasks { live: vec!["t".into()], main: vec!["t".into()], shell: vec![] }, 3), (stops_of("b", &["t"]), 3), (stopped("t"), 4)]), "a level entry of another or unread kind gets its turn");
    assert!(owes(vec![(backgrounded("t"), 3), (stops_of("b", &["t"]), 3), (notified("t"), 4)]), "a task that completed is told in its own turn: only a status of stopped is ours");
    // The stop is read from any line of the turn, including a request line already seen.
    let (state, _) = run(vec![(req("a", 10), 1), (stops_of("a", &["t"]), 1)]);
    assert!(state.stopped_by_us.contains("t"));
}

#[test]
fn another_conversations_brief_is_written_only_at_idle_with_the_switch_and_never_with_the_flush() {
    let after = |events: Vec<(Event, i64)>| last([events, vec![(other_waits(), 5)]].concat());
    assert_eq!(after(answered_start()), switches());
    assert_eq!(after(vec![(Event::TurnBegan, 0), (Event::Echo, 0)]), Action::None, "a turn runs");
    assert_eq!(after(vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1), (level(&["t"]), 1), (OK, 2)]), Action::None, "a task is in the level");
    assert_eq!(after(vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1), (backgrounded("t"), 1), (level(&["t"]), 1), (notified("t"), 1), (level(&[]), 1), (OK, 2)]), Action::None, "a notification is owed");
    assert_eq!(after(vec![(Event::TurnBegan, 0)]), Action::None);
    assert_eq!(last(vec![(Event::TurnBegan, 0), (other_waits(), 1)]), Action::None, "the start message is not echoed");
    // The served conversation's brief goes at any time with nothing outstanding while a turn runs: no switch, even when the other is older.
    let both = |other_first: bool| Event::Inbox { pending: true, other: true, other_first };
    let (state, _) = run(vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1)]);
    assert_eq!(feed(state, both(true), 2).1, tells(), "mid-turn: the served conversation's brief, not a switch");
    assert_eq!(last([answered_start(), vec![(both(false), 5)]].concat()), tells(), "at idle with the served brief the oldest: no switch");
    // At idle the oldest waiting brief goes first: another conversation's older brief is the switch, and the served one's waits for the next poll.
    let (state, actions) = run_from(run(answered_start()).0, vec![(both(true), 5)]);
    assert_eq!(actions, vec![switches()]);
    assert!(!state.other && !state.inbox && !state.other_first && state.written == 2, "a switch clears what the poll said: the next poll restates it");
    // At idle with the flush asked, the write is the flush alone.
    let rotation = i64::try_from(CAP_S).unwrap() - ROTATE_MARGIN_S;
    assert_eq!(last([answered_start(), vec![(other_waits(), rotation)]].concat()), flush(Why::Rotation), "the rotation flush never switches");
    assert_eq!(last([answered_start(), vec![(both(true), rotation)]].concat()), Action::Write { tells: true, switch: false, flush: Some(Why::Rotation) }, "the flush carries the served conversation's brief, never the other's: no switch");
}

#[test]
fn the_cache_end_flush_waits_for_a_task_in_the_level_and_its_timer_runs_from_the_notification_turns_result() {
    let ttl = TTL_1H_S - TTL_MARGIN_S;
    let running = [vec![(Event::TurnBegan, 0), (Event::Echo, 0), (req("a", 10), 1)], task_in_level("t", 1), vec![(OK, 2)]].concat();
    assert_eq!(last([running.clone(), vec![(Event::Tick, 2 + ttl + 100)]].concat()), Action::None, "a task runs: the cache end does not close the session");
    let notified_turn = [task_ends("t", 3), vec![(Event::TurnBegan, 3), (req("b", 10), 3), (OK, 4000)]].concat();
    let (state, _) = run([running.clone(), notified_turn.clone()].concat());
    assert_eq!(next_deadline(&state), Some(t(4000 + ttl).min(t(i64::try_from(CAP_S).unwrap() - ROTATE_MARGIN_S))));
    assert_eq!(last([running.clone(), notified_turn.clone(), vec![(Event::Tick, 2 + ttl + 100)]].concat()), Action::None, "the timer runs from the notification turn's result, not the first");
    assert_eq!(last([running, notified_turn, vec![(Event::Tick, 4000 + ttl)]].concat()), flush(Why::Idle));
    // Another conversation's waiting brief is no reason to end the session at its cache's end either: it is switched to at idle.
    assert_eq!(last([answered_start(), vec![(other_waits(), 2 + ttl)]].concat()), switches());
}

// ---- the conversation worth keeping ---------------------------------------------

/// Runs `events` from a state, then the child exits with `code` and its stream ends: the `warm_until` of the outcome the run finishes with.
fn warm_after(state: State, mut events: Vec<(Event, i64)>, code: Option<i32>) -> Option<DateTime<Utc>> {
    events.extend([(Event::ChildExited(code), 9_000), (Event::StdoutEof, 9_000)]);
    match run_from(state, events).1.pop() {
        Some(Action::Finish(SelfRunOutcome::Ended { warm_until, .. })) => warm_until,
        other => panic!("the run did not finish: {other:?}"),
    }
}

/// A cap that puts the rotation flush at second 100, before any idle lapse.
fn near_cap() -> State {
    State::started(t(0), u64::try_from(ROTATE_MARGIN_S + 100).unwrap())
}

#[test]
fn a_clean_end_is_kept_until_its_last_request_plus_its_cache_lifetime_less_the_margin() {
    // Two requests and a result at three different times: the base is the last request's.
    let events = |result: Event| vec![(Event::Echo, 0), (req("a", 10), 10), (req("b", 20), 50), (result, 90)];
    assert_eq!(warm_after(working(), events(OK), Some(0)), Some(t(10 + 3600 - 60)));
    assert_eq!(warm_after(working(), events(OK_SHORT), Some(0)), Some(t(10 + 300 - 60)));
    // A repeated request id is not a new request and moves nothing.
    let repeated = vec![(Event::Echo, 0), (req("a", 10), 10), (req("a", 10), 50), (OK, 90)];
    assert_eq!(warm_after(working(), repeated, Some(0)), Some(t(3600 - 60)));
}

#[test]
fn a_requests_start_is_bounded_by_the_line_before_it_never_by_its_own_first_line() {
    // The first assistant line arrives 500 s after the echo: the request began somewhere in between, and the earliest bound is the echo.
    assert_eq!(warm_after(working(), vec![(Event::Echo, 0), (req("a", 10), 500), (OK, 600)], Some(0)), Some(t(3600 - 60)));
    // The next request of the turn begins after the previous request's last line, not after its result.
    assert_eq!(warm_after(working(), vec![(Event::Echo, 0), (req("a", 10), 100), (req("a", 10), 200), (req("b", 10), 900), (OK, 950)], Some(0)), Some(t(200 + 3600 - 60)));
    // A request with no echo before it (nothing was written) is bounded by the result before it.
    assert_eq!(warm_after(working(), vec![(Event::Echo, 0), (req("a", 10), 10), (OK, 20), (req("b", 10), 900), (OK, 905)], Some(0)), Some(t(20 + 3600 - 60)));
    // After a result, the next turn's echo is the bound.
    assert_eq!(warm_after(working(), vec![(Event::Echo, 0), (req("a", 10), 10), (OK, 20), (Event::Echo, 300), (req("b", 10), 900), (OK, 950)], Some(0)), Some(t(300 + 3600 - 60)));
}

#[test]
fn the_rotation_flush_keeps_its_conversation_and_every_other_end_clears_it() {
    let clean = vec![(Event::Echo, 0), (req("a", 10), 10), (OK, 20)];
    let with = |more: Vec<(Event, i64)>| [clean.clone(), more].concat();
    assert_eq!(warm_after(working(), clean.clone(), Some(0)), Some(t(3600 - 60)));
    // The rotation: the flush is asked near the cap, answered, and the end is kept from that answer's request.
    let rotation = vec![(Event::Tick, 100), (Event::Echo, 101), (req("b", 20), 101), (OK, 102)];
    assert_eq!(warm_after(near_cap(), with(rotation.clone()), Some(0)), Some(t(101 + 3600 - 60)));
    // Each condition alone clears.
    let idle = vec![(Event::Tick, 3560), (Event::Echo, 3561), (req("b", 20), 3561), (OK, 3562)];
    assert_eq!(warm_after(working(), with(idle), Some(0)), None, "an idle end is the clear");
    let ceiling = vec![(Event::Echo, 0), (req("a", CEILING_TOKENS), 10), (Event::Echo, 11), (OK, 20)];
    assert_eq!(warm_after(working(), ceiling, Some(0)), None, "a ceiling end would only flush again");
    let over_after_rotation = with(vec![(Event::Tick, 100), (Event::Echo, 101), (req("b", CEILING_TOKENS), 101), (OK, 102)]);
    assert_eq!(warm_after(near_cap(), over_after_rotation, Some(0)), None, "the prefix alone, under a rotation flush");
    assert_eq!(warm_after(working(), with(vec![(delivered(Stop::Interrupt), 30)]), Some(0)), None, "a delivered signal");
    assert_eq!(warm_after(working(), with(vec![(Event::Signalled { how: Stop::Interrupt, delivered: false }, 30)]), Some(0)), Some(t(3600 - 60)), "a signal that never went out ended nothing");
    let blocked_but_ok = vec![(Event::Echo, 0), (req("a", 10), 10), (Event::Result { ok: true, blocked: true, ttl_1h: true }, 20), (OK, 30)];
    assert_eq!(warm_after(working(), blocked_but_ok, Some(0)), None, "blocked is sticky");
    assert_eq!(warm_after(working(), with(vec![(Event::Echo, 30), (FAILED, 40)]), Some(0)), None, "the last result failed");
    assert_eq!(warm_after(working(), with(vec![(Event::Echo, 30)]), Some(0)), None, "a turn was open");
    assert_eq!(warm_after(working(), clean.clone(), Some(1)), None, "a non-zero exit");
    assert_eq!(warm_after(working(), clean, None), None, "an exit by signal");
    assert_eq!(warm_after(working(), vec![(Event::Echo, 0), (OK, 20)], Some(0)), None, "no request, no cache");
}

#[test]
fn a_kept_conversation_resumes_until_its_warm_until_and_not_after_and_never_across_a_seed_change() {
    let kept = |same_seed| Kept { session: uuid::Uuid::nil(), transcript: "/state/session/x.jsonl".into(), warm_until: t(100), written_at: t(0), same_seed };
    assert!(resumes(&kept(true), t(99)));
    assert!(!resumes(&kept(true), t(100)));
    assert!(!resumes(&kept(true), t(101)));
    assert!(!resumes(&kept(false), t(0)));
}

#[test]
fn a_record_claiming_more_warmth_than_a_cache_has_is_cold() {
    let written = |warm_until, written_at| Kept { session: uuid::Uuid::nil(), transcript: "/state/session/x.jsonl".into(), warm_until, written_at, same_seed: true };
    // One hour from the write is the longest a cache lives; a record may claim up to it and no more.
    assert!(resumes(&written(t(1000 + 3600), t(1000)), t(1100)));
    assert!(!resumes(&written(t(1000 + 3600 + 1), t(1000)), t(1100)), "a forged far-future warm_until");
    assert!(!resumes(&written(t(1_000_000), t(1000)), t(1100)));
}
