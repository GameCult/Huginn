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

// The fallback controller. `control` is the one controller; these tests give it
// explicit gains so they pin each term whatever `GAINS` the simulation chose.

const RESETS: i64 = 1_000_000_000_000;
const MID: i64 = RESETS - WINDOW_MS / 2;
const HOUR: i64 = 3_600_000;
const T: Gains = Gains {
    kp: 0.02,
    ki: 0.002,
    kd: 0.02,
};

fn read(used_permille: u32, observed_at_ms: i64) -> Reading {
    Reading {
        used_permille,
        observed_at_ms,
        resets_at_ms: RESETS,
    }
}

fn near(got: u32, want: u32) {
    assert!(
        got.abs_diff(want) <= 1,
        "{got} is not within one second of {want}"
    );
}

#[test]
fn behind_target_shortens_the_beat() {
    let at = |used| {
        control(
            &T,
            &mut Fallback::default(),
            Some(&read(used, MID)),
            MID,
            900,
            false,
        )
    };
    // Halfway through the window the trajectory is 450: error +50, 0, -50 is a
    // turn rate of 5, 4 and 3 an hour.
    near(at(400), 720);
    near(at(450), DEFAULT_BEAT_S);
    near(at(500), 1200);
}

#[test]
fn derivative_is_on_measurement() {
    let g = Gains { ki: 0.0, ..T };
    let mut s = Fallback::default();
    near(
        control(&g, &mut s, Some(&read(410, MID)), MID, 900, false),
        750,
    );
    // A new observation 10 permille higher a quarter hour on: the slope is 40 an hour.
    let next = read(420, MID + 15 * 60_000);
    near(
        control(&g, &mut s, Some(&next), MID + 15 * 60_000, 900, false),
        941,
    );
    // The same reading again carries no derivative term.
    near(
        control(&g, &mut s, Some(&next), MID + 16 * 60_000, 900, false),
        778,
    );
    // A step in the target with used unchanged moves the beat by kp x error alone.
    near(
        control(&g, &mut s, Some(&next), MID + 18 * 60_000, 1000, false),
        639,
    );
}

#[test]
fn saturated_output_does_not_wind_up() {
    let g = Gains {
        kp: 0.1,
        ki: 0.01,
        kd: 0.0,
    };
    let day5 = RESETS - 2 * 24 * HOUR;
    let mut s = Fallback::default();
    let behind = read(0, day5);
    for i in 0..100 {
        assert_eq!(
            control(&g, &mut s, Some(&behind), day5 + i * 60_000, 900, false),
            FALLBACK_MIN_S
        );
    }
    assert_eq!(
        s.integral, 0.0,
        "error is positive and the output is at its limit"
    );
    let near_on = read(600, day5 + 100 * 60_000);
    for i in 100..110 {
        control(&g, &mut s, Some(&near_on), day5 + i * 60_000, 900, false);
    }
    assert!(s.integral > 0.0, "unsaturated again, it integrates");

    // Saturated at the slow limit with the error pulling the other way integrates.
    let mut s = Fallback {
        integral: -1000.0,
        ..Fallback::default()
    };
    let reading = read(400, MID);
    control(&g, &mut s, Some(&reading), MID, 900, false);
    control(&g, &mut s, Some(&reading), MID + HOUR, 900, false);
    assert!(s.integral > -1000.0);
}

#[test]
fn vote_freezes_and_release_is_bumpless() {
    let reading = read(300, MID);
    let mut s = Fallback::default();
    for i in 0..5 {
        control(&T, &mut s, Some(&reading), MID + i * 60_000, 900, false);
    }
    s.rate_per_h = 3600.0 / 300.0;
    let (mut short, mut long) = (s.clone(), s.clone());
    let (t_short, t_long) = (MID + 6 * 60_000, MID + 6 * 60_000 + HOUR);
    assert_eq!(
        control(&T, &mut short, Some(&reading), t_short, 900, true),
        300
    );
    assert_eq!(
        control(&T, &mut long, Some(&reading), t_long, 900, true),
        300
    );
    // However long the vote held, the first beat after it is the voted beat.
    near(
        control(&T, &mut short, Some(&reading), t_short + 60_000, 900, false),
        300,
    );
    near(
        control(&T, &mut long, Some(&reading), t_long + 60_000, 900, false),
        300,
    );
}

#[test]
fn rollover_resets_memory() {
    let mut s = Fallback::default();
    let before = RESETS - 2 * HOUR;
    for i in 0..12 {
        let now = before + i * 10 * 60_000;
        control(&T, &mut s, Some(&read(100, before)), now, 900, false);
    }
    assert!(s.integral > 100.0);
    let new = Reading {
        used_permille: 5,
        observed_at_ms: RESETS + HOUR,
        resets_at_ms: RESETS + WINDOW_MS,
    };
    // Used fell from 100 to 5 across the reset: no derivative kick, no old integral.
    near(
        control(&T, &mut s, Some(&new), RESETS + HOUR, 900, false),
        898,
    );
    assert!(s.integral < 2.0);
    assert_eq!(s.window_resets_at_ms, RESETS + WINDOW_MS);
}

#[test]
fn no_reading_holds() {
    let mut s = Fallback::default();
    assert_eq!(
        control(&T, &mut s, None, MID, 900, false),
        DEFAULT_BEAT_S,
        "never set"
    );
    let live = read(400, MID);
    control(&T, &mut s, Some(&live), MID, 900, false);
    let held = control(&T, &mut s, Some(&live), MID + 60_000, 900, false);
    assert_ne!(held, DEFAULT_BEAT_S);
    let integral = s.integral;
    assert_eq!(
        control(&T, &mut s, None, MID + 2 * 60_000, 900, false),
        held
    );
    let stale = Reading {
        observed_at_ms: RESETS - WINDOW_MS - 1,
        ..live
    };
    assert_eq!(
        control(&T, &mut s, Some(&stale), MID + 3 * 60_000, 900, false),
        held
    );
    let expired = Reading {
        resets_at_ms: MID + 4 * HOUR,
        observed_at_ms: MID + 4 * HOUR - WINDOW_MS,
        ..live
    };
    assert_eq!(
        control(&T, &mut s, Some(&expired), MID + 6 * HOUR, 900, false),
        held
    );
    assert_eq!(s.integral, integral, "nothing integrated while held");
    // The six hours without a reading are not integrated when it comes back.
    control(
        &T,
        &mut s,
        Some(&read(400, MID + 6 * HOUR + 60_000)),
        MID + 6 * HOUR + 60_000,
        900,
        false,
    );
    assert!(
        s.integral - integral < 2.0,
        "{} then {}",
        integral,
        s.integral
    );
}

#[test]
fn beat_is_bounded() {
    let wild = Gains {
        kp: 1.0,
        ki: 0.0,
        kd: 0.0,
    };
    let late = RESETS - 12 * HOUR;
    assert_eq!(
        control(
            &wild,
            &mut Fallback::default(),
            Some(&read(0, late)),
            late,
            900,
            false
        ),
        FALLBACK_MIN_S
    );
    assert_eq!(
        control(
            &wild,
            &mut Fallback::default(),
            Some(&read(1000, MID)),
            MID,
            900,
            false
        ),
        FALLBACK_MAX_S
    );
    for used in [0, 1, 450, 900, 1000, 5000] {
        for target in [0, 900, 1000] {
            for gains in [wild, T, GAINS] {
                let beat = control(
                    &gains,
                    &mut Fallback::default(),
                    Some(&read(used, MID)),
                    MID,
                    target,
                    false,
                );
                assert!((FALLBACK_MIN_S..=FALLBACK_MAX_S).contains(&beat), "{beat}");
            }
        }
    }
}

#[test]
fn fallback_beat_is_the_controller_with_the_committed_gains() {
    let live = read(400, MID);
    let (mut a, mut b) = (Fallback::default(), Fallback::default());
    let beat = fallback_beat(&mut a, Some(&live), MID, 900, false);
    assert_eq!(beat, control(&GAINS, &mut b, Some(&live), MID, 900, false));
    assert_ne!(beat, DEFAULT_BEAT_S, "behind the trajectory");
    assert_eq!(a, b);
    // On the trajectory with no memory the answer is the bias, whatever the gains.
    let on = read(450, MID);
    let beat = fallback_beat(&mut Fallback::default(), Some(&on), MID, 900, false);
    assert_eq!(beat, DEFAULT_BEAT_S);
    assert_eq!(
        fallback_beat(&mut Fallback::default(), None, MID, 900, false),
        DEFAULT_BEAT_S
    );
}

#[test]
fn a_window_that_ends_this_instant_is_expired() {
    let mut s = Fallback::default();
    let live = read(400, MID);
    let held = control(&T, &mut s, Some(&live), MID, 900, false);
    let ending = Reading {
        resets_at_ms: MID + 60_000,
        observed_at_ms: MID + 60_000 - WINDOW_MS,
        used_permille: 0,
    };
    // At its reset instant the window holds the last beat instead of steering.
    assert_eq!(
        control(&T, &mut s, Some(&ending), MID + 60_000, 900, false),
        held
    );
}
