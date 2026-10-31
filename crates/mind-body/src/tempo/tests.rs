//! The tempo rule as tables: what `tally` and `pick` answer for the cases that
//! carry the rulings. Pure, so nothing here needs a mind, a clock or a daemon.

use super::*;

fn mind(slug: &str, waited_s: Option<u64>) -> Standing {
    Standing {
        slug: slug.into(),
        eligible: true,
        waited_s,
        last_turn_ms: waited_s.map(|waited| 1_000_000 - i64::try_from(waited).unwrap() * 1000),
        self_permille: 0,
        topic_permille: 0,
        addressed_by_member: false,
    }
}

#[test]
fn the_score_is_the_wait_in_beats_times_ten_to_the_urgency_in_orders() {
    let mut standing = mind("a", Some(1800));
    assert_eq!(score(&standing, 900), 2.0, "two beats waited, no urgency");
    assert_eq!(score(&standing, 600), 3.0);
    standing.self_permille = 1000;
    assert_eq!(score(&standing, 900), 20.0, "full urgency is worth one order of magnitude");
    standing.self_permille = 500;
    assert!((score(&standing, 900) - 2.0 * 10f64.sqrt()).abs() < 1e-9);
    assert_eq!(score(&mind("never", None), 900), f64::INFINITY);
}

#[test]
fn no_votes_is_the_default_beat() {
    assert_eq!(tally(&[]), DEFAULT_BEAT_S);
    assert_eq!(tally(&[None, None]), DEFAULT_BEAT_S);
    assert_eq!(DEFAULT_BEAT_S, 900);
}

#[test]
fn the_tally_is_the_lower_median_and_one_outlier_cannot_move_it() {
    let table: [(&[u32], u32); 7] = [
        (&[600], 600),
        (&[300, 900], 300),
        (&[900, 300, 600], 600),
        (&[100, 300, 600, 900], 300),
        (&[600, 600, 86_400], 600),
        (&[60, 600, 600], 600),
        (&[60, 60, 86_400, 86_400, 600], 600),
    ];
    for (votes, want) in table {
        let votes: Vec<Option<u32>> = votes.iter().copied().map(Some).collect();
        assert_eq!(tally(&votes), want, "{votes:?}");
    }
}

#[test]
fn silence_is_abstention_and_only_cast_votes_are_counted() {
    let mut eight = vec![None; 8];
    eight[0] = Some(60);
    assert_eq!(tally(&eight), 60, "one voter among eight silent Minds decides");
    assert_eq!(tally(&[None; 8]), DEFAULT_BEAT_S, "none cast is the default");
    assert_eq!(tally(&[Some(3600), None, Some(60)]), 60, "two cast: the lower median");
    assert_eq!(tally(&[Some(3600), Some(1800), None, None, Some(60)]), 1800, "abstainers do not dilute the median");
}

#[test]
fn the_tally_does_not_depend_on_the_order_the_votes_came_in() {
    assert_eq!(tally(&[Some(900), Some(60), None, Some(600)]), tally(&[None, Some(60), Some(600), Some(900)]));
}

#[test]
fn waiting_alone_round_robins() {
    let rows = [
        mind("ann", Some(100)),
        mind("bob", Some(400)),
        mind("cy", Some(250)),
    ];
    assert_eq!(
        pick(&rows, 100),
        Some(1),
        "the longest waiter takes the beat"
    );
    // After bob's turn he has waited least; the next longest waiter goes.
    let after = [
        mind("ann", Some(200)),
        mind("bob", Some(0)),
        mind("cy", Some(350)),
    ];
    assert_eq!(pick(&after, 100), Some(2));
}

#[test]
fn a_full_urgency_equals_ten_times_the_wait_at_one_order() {
    assert_eq!(LOG_BOUND_ORDERS, 1.0);
    let steady = mind("ann", Some(100));
    let mut urgent = mind("bob", Some(11));
    urgent.self_permille = 1000;
    assert_eq!(
        pick(&[steady.clone(), urgent.clone()], 100),
        Some(1),
        "eleven seconds at full urgency beat a hundred waiting"
    );
    urgent.waited_s = Some(9);
    urgent.last_turn_ms = Some(1_000_000 - 9000);
    assert_eq!(pick(&[steady, urgent], 100), Some(0), "nine do not");
}

#[test]
fn a_half_urgency_is_the_square_root_of_ten() {
    let mut half = mind("ann", Some(100));
    half.self_permille = 500;
    let plain = mind("bob", Some(100));
    let ratio = score(&half, 100) / score(&plain, 100);
    assert!((ratio - 10f64.sqrt()).abs() < 1e-9, "{ratio}");
}

#[test]
fn urgency_is_the_strongest_of_self_topic_and_a_member_addressing_her() {
    let mut row = mind("ann", Some(100));
    assert_eq!(urgency_permille(&row), 0);
    row.self_permille = 300;
    row.topic_permille = 700;
    assert_eq!(urgency_permille(&row), 700);
    row.addressed_by_member = true;
    assert_eq!(urgency_permille(&row), 1000);
    row.addressed_by_member = false;
    row.topic_permille = 0;
    row.self_permille = 5000;
    assert_eq!(
        urgency_permille(&row),
        1000,
        "an urgency past the scale is the scale"
    );
}

#[test]
fn a_tie_goes_to_the_oldest_last_turn_then_to_the_slug() {
    let mut old = mind("zed", Some(100));
    old.last_turn_ms = Some(10);
    let mut new = mind("ann", Some(100));
    new.last_turn_ms = Some(20);
    assert_eq!(
        pick(&[new.clone(), old.clone()], 100),
        Some(1),
        "the older last turn wins the tie, whatever the slug"
    );
    new.last_turn_ms = Some(10);
    assert_eq!(
        pick(&[old, new], 100),
        Some(1),
        "equal turns: the lower slug"
    );
}

#[test]
fn a_mind_that_never_acted_goes_first_and_the_never_acted_tie_by_slug() {
    let rows = [
        mind("ann", Some(86_400)),
        mind("cy", None),
        mind("bob", None),
    ];
    assert_eq!(pick(&rows, 900), Some(2));
}

#[test]
fn a_braked_or_busy_mind_is_never_picked_however_high_she_scores() {
    let mut rows = [mind("ann", Some(10)), mind("bob", Some(100_000))];
    rows[1].eligible = false;
    rows[1].self_permille = 1000;
    assert_eq!(pick(&rows, 100), Some(0));
    rows[0].eligible = false;
    assert_eq!(pick(&rows, 100), None, "nobody eligible, nobody picked");
    assert_eq!(pick(&[], 100), None);
}

#[test]
fn a_longer_beat_does_not_change_who_wins_among_equal_urgency() {
    let rows = [mind("ann", Some(500)), mind("bob", Some(300))];
    assert_eq!(pick(&rows, 60), pick(&rows, 86_400));
}
