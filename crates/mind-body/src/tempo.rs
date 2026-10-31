//! The tempo rule of the shared heartbeat, pure: no clock, no mind, no I/O
//! (rulings `eureka-body:ruling:shared-heartbeat-joint-lever`,
//! `self-declared-urgency-etiquette`, `swarm-shares-log-bound`).
//!
//! The Minds jointly hold the lever to one heartbeat. `tally` turns their votes
//! into the beat; `pick` chooses who takes the turn when a beat falls. The
//! heartbeat owns when a beat falls and what it launches; this module owns only
//! the two decisions.

use std::cmp::Ordering;

/// The beat, in seconds, when no Mind has voted.
pub const DEFAULT_BEAT_S: u32 = 900;

/// How many orders of magnitude a full urgency may multiply a Mind's wait by.
/// Interim, per `swarm-shares-log-bound`, until the swarm's governance sets it.
pub const LOG_BOUND_ORDERS: f64 = 1.0;

/// The swarm's tempo (ruling `eureka-body:ruling:tempo-silent-means-default`):
/// the lower median over every Mind in the roster, each her newest vote in seconds
/// per beat. A Mind with no standing vote, or whose mind cannot be read, is
/// `None` and counts as `DEFAULT_BEAT_S`, so a lone voter cannot move the beat.
pub fn tally(votes: &[Option<u32>]) -> u32 {
    let mut sorted: Vec<u32> = votes.iter().map(|vote| vote.unwrap_or(DEFAULT_BEAT_S)).collect();
    sorted.sort_unstable();
    match sorted.len() {
        0 => DEFAULT_BEAT_S,
        n => sorted[(n - 1) / 2],
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
mod tests;
