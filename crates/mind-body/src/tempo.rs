//! Who takes the beat, pure: no clock, no mind, no I/O (ruling
//! `eureka-body:ruling:swarm-shares-log-bound`).
//!
//! The heartbeat owns when a beat falls and what it wakes; this module owns
//! only the decision of who takes it: `pick`, by `score`.

use std::cmp::Ordering;

/// How many orders of magnitude a full urgency may multiply a Mind's wait by.
/// Interim, per `swarm-shares-log-bound`, until the swarm's governance sets it.
pub const LOG_BOUND_ORDERS: f64 = 1.0;

/// What the pick sees of one Mind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
    pub slug: String,
    /// Her brake is released and she is not in a turn.
    pub eligible: bool,
    /// Seconds since the heartbeat last woke her for a turn; none when it never has.
    pub waited_s: Option<u64>,
    /// When the heartbeat last woke her for a turn, in unix milliseconds; none when never.
    pub last_turn_ms: Option<i64>,
    /// The urgency her subscriptions give her pending deliveries, in thousandths.
    pub topic_permille: u16,
    /// A member (not the operator) has a pending message that names or tags her.
    pub addressed_by_member: bool,
}

/// The urgency in force for a Mind, in thousandths: the stronger of what her
/// topics say and a member addressing her (which is 1000).
pub fn urgency_permille(standing: &Standing) -> u16 {
    let addressed = if standing.addressed_by_member { 1000 } else { 0 };
    standing.topic_permille.max(addressed).min(1000)
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
