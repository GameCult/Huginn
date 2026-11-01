//! The tempo rule of the shared heartbeat, pure: no clock, no mind, no I/O
//! (rulings `eureka-body:ruling:shared-heartbeat-joint-lever`,
//! `self-declared-urgency-etiquette`, `swarm-shares-log-bound`).
//!
//! The Minds jointly hold the lever to one heartbeat. `tally` turns their votes
//! into the beat; `pick` chooses who takes the turn when a beat falls. The
//! heartbeat owns when a beat falls and what it launches; this module owns only
//! the decisions: the swarm's tempo, who takes the turn, and the third, the
//! beat when no vote is cast (`fallback_beat`, rulings
//! `tempo-default-is-usage-fitter`, `tempo-fallback-pid-tuned-by-sim`).

use std::cmp::Ordering;

/// The beat, in seconds, when no vote is cast.
pub const DEFAULT_BEAT_S: u32 = 900;

/// The shortest and longest beat the fallback controller may answer, in seconds.
pub const FALLBACK_MIN_S: u32 = 60;
pub const FALLBACK_MAX_S: u32 = 3600;

const WINDOW_MS: i64 = 7 * 24 * 3_600_000;
const HOUR_MS: f64 = 3_600_000.0;
/// The turn rate, in beats per hour, that `DEFAULT_BEAT_S` stands for: the
/// controller's bias.
const BIAS_PER_H: f64 = 3600.0 / DEFAULT_BEAT_S as f64;

/// The seven-day plan window as the connector last read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub used_permille: u32,
    pub observed_at_ms: i64,
    pub resets_at_ms: i64,
}

/// PID gains. The actuator is the turn rate in beats per hour and the error is
/// in permille of the plan, so `kp` is beats/h per permille, `ki` beats/h per
/// permille-hour and `kd` beats/h per (permille/h).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gains {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
}

/// Chosen by the committed simulation (`docs/research/2026-11-01-tempo-pid-sim.md`),
/// never by a run-time search.
pub const GAINS: Gains = Gains { kp: 0.02, ki: 0.01, kd: 0.0 };

/// The fallback controller's memory. The heartbeat owns persisting it; it is
/// never published as truth. `rate_per_h` is the last output as a turn rate
/// (zero: none yet); while a vote decides the caller sets it to the voted beat's
/// rate, so the release is bumpless.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fallback {
    pub integral: f64,
    pub last_used_permille: u32,
    pub last_observed_at_ms: i64,
    pub last_now_ms: i64,
    pub window_resets_at_ms: i64,
    pub rate_per_h: f64,
}

/// The beat when no vote is cast: a PID on the gap between the seven-day usage
/// trajectory to `target_permille` and what was used, acting on the turn rate
/// (behind the trajectory raises it). Derivative on the measurement, only
/// between distinct observations; conditional integration against the beat
/// bounds; while `voted` nothing integrates and the integral tracks the voted
/// rate, so release is bumpless. A new window clears the memory; with no
/// usable reading the last beat holds (`DEFAULT_BEAT_S` before any).
pub fn fallback_beat(
    state: &mut Fallback,
    reading: Option<&Reading>,
    now_ms: i64,
    target_permille: u32,
    voted: bool,
) -> u32 {
    control(&GAINS, state, reading, now_ms, target_permille, voted)
}

fn beat_of(rate_per_h: f64) -> u32 {
    if rate_per_h > 0.0 {
        ((3600.0 / rate_per_h).round().clamp(f64::from(FALLBACK_MIN_S), f64::from(FALLBACK_MAX_S))) as u32
    } else {
        FALLBACK_MAX_S
    }
}

fn held(state: &Fallback) -> u32 {
    if state.rate_per_h == 0.0 {
        DEFAULT_BEAT_S
    } else {
        beat_of(state.rate_per_h)
    }
}

/// The one controller; `fallback_beat` is it with `GAINS`, the simulation is it
/// with candidates.
fn control(
    gains: &Gains,
    state: &mut Fallback,
    reading: Option<&Reading>,
    now_ms: i64,
    target_permille: u32,
    voted: bool,
) -> u32 {
    let dt_h = if state.last_now_ms > 0 { (now_ms - state.last_now_ms).max(0) as f64 / HOUR_MS } else { 0.0 };
    state.last_now_ms = now_ms;
    let Some(r) = reading.filter(|r| now_ms < r.resets_at_ms && r.observed_at_ms >= r.resets_at_ms - WINDOW_MS) else {
        return held(state);
    };
    if r.resets_at_ms != state.window_resets_at_ms {
        state.window_resets_at_ms = r.resets_at_ms;
        state.integral = 0.0;
        state.last_used_permille = r.used_permille;
        state.last_observed_at_ms = r.observed_at_ms;
    }
    let elapsed = (now_ms - (r.resets_at_ms - WINDOW_MS)).clamp(0, WINDOW_MS) as f64;
    let error = f64::from(target_permille) * elapsed / WINDOW_MS as f64 - f64::from(r.used_permille);
    let fresh = r.observed_at_ms > state.last_observed_at_ms;
    let slope = if fresh {
        (f64::from(r.used_permille) - f64::from(state.last_used_permille))
            / ((r.observed_at_ms - state.last_observed_at_ms) as f64 / HOUR_MS)
    } else {
        0.0
    };
    if fresh {
        state.last_used_permille = r.used_permille;
        state.last_observed_at_ms = r.observed_at_ms;
    }
    if voted {
        if gains.ki != 0.0 && state.rate_per_h > 0.0 {
            state.integral = (state.rate_per_h - BIAS_PER_H - gains.kp * error) / gains.ki;
        }
        return held(state);
    }
    let rate = |integral: f64| BIAS_PER_H + gains.kp * error + gains.ki * integral - gains.kd * slope;
    let (low, high) = (3600.0 / f64::from(FALLBACK_MAX_S), 3600.0 / f64::from(FALLBACK_MIN_S));
    let before = rate(state.integral);
    let winding = (before >= high && error > 0.0) || (before <= low && error < 0.0);
    if !winding {
        state.integral += error * dt_h;
    }
    let beat = beat_of(rate(state.integral));
    state.rate_per_h = 3600.0 / f64::from(beat);
    beat
}

/// How many orders of magnitude a full urgency may multiply a Mind's wait by.
/// Interim, per `swarm-shares-log-bound`, until the swarm's governance sets it.
pub const LOG_BOUND_ORDERS: f64 = 1.0;

/// The swarm's tempo (ruling `eureka-body:ruling:tempo-silence-is-abstention`):
/// the lower median of the votes actually cast, each a Mind's newest standing
/// vote in seconds per beat. Silence is abstention: a Mind with no standing vote
/// (never voted, or withdrew) or whose mind cannot be read is `None` and is not
/// counted, so the beat follows those who care to vote. With no vote cast it is
/// `DEFAULT_BEAT_S`. On one voter that voter decides; on two the lower of them.
pub fn tally(votes: &[Option<u32>]) -> u32 {
    let mut cast: Vec<u32> = votes.iter().flatten().copied().collect();
    cast.sort_unstable();
    match cast.len() {
        0 => DEFAULT_BEAT_S,
        n => cast[(n - 1) / 2],
    }
}

/// What the pick sees of one Mind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
    pub slug: String,
    /// Her brake is released and she is not in a turn.
    pub eligible: bool,
    /// Seconds since her last Persona turn; none when she has never had one.
    pub waited_s: Option<u64>,
    /// When her last Persona turn began, in unix milliseconds; none when never.
    pub last_turn_ms: Option<i64>,
    /// The urgency she declared for herself and no turn has consumed, in thousandths.
    pub self_permille: u16,
    /// The urgency her subscriptions give her pending deliveries, in thousandths.
    pub topic_permille: u16,
    /// A member (not the operator) has a pending message that names or tags her.
    pub addressed_by_member: bool,
}

/// The urgency in force for a Mind, in thousandths: the strongest of what she
/// declared, what her topics say and a member addressing her (which is 1000).
pub fn urgency_permille(standing: &Standing) -> u16 {
    let addressed = if standing.addressed_by_member {
        1000
    } else {
        0
    };
    standing
        .self_permille
        .max(standing.topic_permille)
        .max(addressed)
        .min(1000)
}

/// `(waited / beat) x 10^(LOG_BOUND_ORDERS x urgency / 1000)`. A Mind that has
/// never had a turn has waited forever: infinite, so she is first, and the tie
/// rule orders the never-acted among themselves.
pub fn score(standing: &Standing, beat_s: u32) -> f64 {
    let waited = standing
        .waited_s
        .map_or(f64::INFINITY, |waited| waited as f64);
    let exponent = LOG_BOUND_ORDERS * f64::from(urgency_permille(standing)) / 1000.0;
    waited / f64::from(beat_s.max(1)) * 10f64.powf(exponent)
}

/// The winner of a beat, as an index into `rows`: the highest score among the
/// eligible; a tie goes to the oldest last turn (never is oldest), then to the
/// lowest slug. None when nobody is eligible. A braked or busy Mind is never picked.
pub fn pick(rows: &[Standing], beat_s: u32) -> Option<usize> {
    let order = |a: &(usize, &Standing), b: &(usize, &Standing)| -> Ordering {
        score(b.1, beat_s)
            .total_cmp(&score(a.1, beat_s))
            .then_with(|| a.1.last_turn_ms.cmp(&b.1.last_turn_ms))
            .then_with(|| a.1.slug.cmp(&b.1.slug))
    };
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.eligible)
        .min_by(order)
        .map(|(index, _)| index)
}

#[cfg(test)]
mod sim;
#[cfg(test)]
mod tests;
