//! The plant the fallback controller is tuned against: one seven-day plan
//! window at one-minute steps, each beat a Persona turn costing a lognormal
//! number of permille, plus the operator's own usage on the same subscription.
//! The swarm has a throughput ceiling: a turn needs a Mind that is not in one,
//! each turn holds its Mind for a lognormal time, and when every Mind is busy
//! the beat launches nothing and the controller is told so.
//! Test-only. The controller here is `super::control`, the same one the
//! heartbeat will call through `fallback_beat`; the sweep passes it candidate
//! gains, `GAINS` is committed. Rerun the sweep with
//! `cargo test -p mind-body tempo::sim::sweep -- --ignored --nocapture`.

use super::*;

const STEP_MS: i64 = 60_000;
const POLL_MS: i64 = 15 * 60_000;
const W0: i64 = 1_000 * 3_600_000;
const TARGET: u32 = 900;
/// Sigma of ln(turn cost).
const COST_SIGMA: f64 = 0.5;
/// Every this-many turns is a Self run, costing `SELF_FACTOR` times a Persona turn.
const SELF_EVERY: u32 = 25;
const SELF_FACTOR: f64 = 5.0;
/// Half-width, in permille, of the band the error must leave to count as a side.
const BAND: f64 = 15.0;
/// The roster the heartbeat routes (aqua, eureka, heimdall, kiko, libby, nibu,
/// void, weksa: the slugs it launched over 2026-10-30..11-01).
const MINDS: usize = 8;
/// How long a Persona turn holds its Mind, lognormal. Measured on Yggdrasil from
/// the `mind-persona@*` start and stop lines over 29 h (1284 turns): mean 15.4 s,
/// median 6.4 s, p90 27.7 s, p99 186 s; the fit below has median 7.5 s, p90 35 s.
const BUSY_MEAN_S: f64 = 15.4;
const BUSY_SIGMA: f64 = 1.2;
/// The shortest spacing between two on-beat launches is one heartbeat poll
/// (median 30 s over 4100 polls); the sim's step, 60 s, is the beat floor and is
/// longer, so the spacing never binds.
const POLL_S: i64 = 30;
const _: () = assert!(POLL_S * 1000 <= STEP_MS);

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn lognormal(&mut self, mean: f64, sigma: f64) -> f64 {
        let (a, b) = (self.next().max(1e-12), self.next());
        let normal = (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos();
        (mean.ln() - sigma * sigma / 2.0 + sigma * normal).exp()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sensor {
    /// The reading refreshes when a turn ends.
    Stream,
    /// The reading refreshes every fifteen minutes.
    Poll,
}

#[derive(Clone, Debug)]
struct Scenario {
    name: &'static str,
    windows: u32,
    start_used: f64,
    /// Permille per Persona turn.
    mean_cost: f64,
    /// (from hour, to hour, permille spread evenly) of the operator's own usage.
    burst: Option<(f64, f64, f64)>,
    /// (at hour, factor) on the per-turn cost.
    cost_step: Option<(f64, f64)>,
    /// (from hour, to hour, beat) of a standing vote.
    vote: Option<(f64, f64, u32)>,
    /// (from hour, to hour) with no reading.
    gap: Option<(f64, f64)>,
    /// The cost is too low to reach the target even at the minimum beat.
    unreachable: bool,
    /// The vote itself spends the week: no controller can end on target, it
    /// can only stop burning once the vote is gone.
    overspent: bool,
    /// Minds the heartbeat can launch, and the factor on how long a turn holds one.
    minds: usize,
    busy_scale: f64,
    /// The most band crossings the controller may show here.
    max_crossings: u32,
    /// The ceiling is below what the target needs for most of the week: no
    /// controller ends on target, and the scenario only compares the controller
    /// told of the stalls with the one that is not.
    stress: bool,
}

const BASE: Scenario = Scenario {
    name: "quiet",
    windows: 1,
    start_used: 0.0,
    mean_cost: 1.3,
    burst: None,
    cost_step: None,
    vote: None,
    gap: None,
    unreachable: false,
    overspent: false,
    minds: MINDS,
    busy_scale: 1.0,
    max_crossings: 2,
    stress: false,
};

fn scenarios() -> Vec<Scenario> {
    vec![
        BASE,
        Scenario {
            name: "cheap-turns",
            mean_cost: 0.6,
            ..BASE
        },
        Scenario {
            name: "dear-turns",
            mean_cost: 3.0,
            ..BASE
        },
        Scenario {
            name: "burst",
            burst: Some((48.0, 52.0, 200.0)),
            ..BASE
        },
        Scenario {
            name: "cost-x3",
            cost_step: Some((84.0, 3.0)),
            ..BASE
        },
        Scenario {
            name: "vote-300s-24h",
            vote: Some((48.0, 72.0, 300)),
            ..BASE
        },
        Scenario {
            name: "vote-60s-1h",
            vote: Some((48.0, 49.0, 60)),
            max_crossings: 3,
            ..BASE
        },
        Scenario {
            name: "vote-60s-4h",
            vote: Some((48.0, 52.0, 60)),
            ..BASE
        },
        Scenario {
            name: "vote-60s-24h",
            vote: Some((48.0, 72.0, 60)),
            overspent: true,
            ..BASE
        },
        Scenario {
            name: "vote-7200s-24h",
            vote: Some((48.0, 72.0, 7200)),
            ..BASE
        },
        Scenario {
            name: "vote-86400s-24h",
            vote: Some((48.0, 72.0, 86_400)),
            ..BASE
        },
        Scenario {
            name: "cheap-then-x3",
            mean_cost: 0.6,
            cost_step: Some((84.0, 3.0)),
            max_crossings: 3,
            ..BASE
        },
        Scenario {
            name: "one-slow-mind",
            mean_cost: 0.6,
            cost_step: Some((84.0, 3.0)),
            minds: 1,
            busy_scale: 45.0,
            stress: true,
            ..BASE
        },
        Scenario {
            name: "rollover",
            windows: 2,
            ..BASE
        },
        Scenario {
            name: "gap-6h",
            gap: Some((72.0, 78.0)),
            ..BASE
        },
        Scenario {
            name: "start-ahead",
            start_used: 500.0,
            ..BASE
        },
        Scenario {
            name: "unreachable",
            mean_cost: 0.05,
            unreachable: true,
            ..BASE
        },
    ]
}

#[derive(Clone, Debug, Default)]
struct Outcome {
    /// Used at each window's reset.
    final_used: Vec<f64>,
    /// Worst |trajectory - used| over the last day of the last window.
    worst_error_last_day: f64,
    /// Beat moves of more than 5% between minutes, per hour, outside votes.
    jumps_per_h: f64,
    /// Times the error crossed from beyond +BAND to beyond -BAND (or back)
    /// during the last three days.
    band_crossings: u32,
    /// Share of the last day of the last window spent at the minimum beat.
    frac_at_min: f64,
    /// The same at the maximum beat.
    frac_at_max: f64,
    /// The beat on the last minute.
    last_beat: u32,
    /// Minutes the beat was due and every Mind was busy.
    stalls: u32,
}

fn run(scenario: &Scenario, sensor: Sensor, gains: &Gains, seed: u64) -> Outcome {
    drive(scenario, sensor, gains, seed, true)
}

/// `report_stalls` false withholds the busy half of the saturation report (the
/// floor half is always given): the controller as it was before it was told.
fn drive(
    scenario: &Scenario,
    sensor: Sensor,
    gains: &Gains,
    seed: u64,
    report_stalls: bool,
) -> Outcome {
    let mut rng = Rng(seed);
    let mut busy_rng = Rng(seed ^ 0xB05E_B05E_B05E_B05E);
    let mut busy_until = vec![W0; scenario.minds];
    // The heartbeat's report: a beat since the last launch found every Mind busy.
    let (mut stalled, mut found_busy) = (false, false);
    let mut state = Fallback::default();
    let mut used = scenario.start_used;
    let mut resets = W0 + WINDOW_MS;
    let mut window = 0u32;
    let sense = |used: f64, now: i64, resets: i64| Reading {
        used_permille: used.round().clamp(0.0, 1000.0) as u32,
        observed_at_ms: now,
        resets_at_ms: resets,
    };
    let mut reading = sense(used, W0, resets);
    let (mut last_turn, mut turns, mut prev_beat) = (W0, 0u32, DEFAULT_BEAT_S);
    let steps = i64::from(scenario.windows) * WINDOW_MS / STEP_MS;
    let mut out = Outcome::default();
    let (mut jumps, mut at_min, mut at_max, mut last_day) = (0u32, 0u32, 0u32, 0u32);
    let mut side = 0i8;
    let mut worst_day = 0f64;
    for i in 1..steps {
        let now = W0 + i * STEP_MS;
        if now >= resets {
            out.final_used.push(used);
            used = 0.0;
            resets += WINDOW_MS;
            window += 1;
        }
        let in_window = (now - (resets - WINDOW_MS)) as f64 / HOUR_MS;
        if let Some((from, to, amount)) = scenario.burst
            && in_window >= from
            && in_window < to
        {
            used += amount * STEP_MS as f64 / HOUR_MS / (to - from);
        }
        let gap = scenario
            .gap
            .is_some_and(|(from, to)| in_window >= from && in_window < to);
        if sensor == Sensor::Poll && !gap && (now - W0) % POLL_MS == 0 {
            reading = sense(used, now, resets);
        }
        let vote = scenario
            .vote
            .filter(|&(from, to, _)| in_window >= from && in_window < to)
            .map(|v| v.2);
        let shown = if gap { None } else { Some(reading) };
        let beat = match vote {
            Some(beat) => {
                state.rate_per_h = 3600.0 / f64::from(beat);
                control(gains, &mut state, shown.as_ref(), now, TARGET, true, false);
                beat
            }
            None => {
                let saturated = prev_beat == FALLBACK_MIN_S || (report_stalls && stalled);
                control(
                    gains,
                    &mut state,
                    shown.as_ref(),
                    now,
                    TARGET,
                    false,
                    saturated,
                )
            }
        };
        if vote.is_none() {
            if window + 1 == scenario.windows && in_window >= 144.0 {
                last_day += 1;
                at_min += u32::from(beat == FALLBACK_MIN_S);
                at_max += u32::from(beat == FALLBACK_MAX_S);
            }
            jumps += u32::from(f64::from(beat.abs_diff(prev_beat)) > 0.05 * f64::from(prev_beat));
        }
        prev_beat = beat;
        out.last_beat = beat;
        if now - last_turn >= i64::from(beat) * 1000 {
            if let Some(mind) = busy_until.iter().position(|&until| until <= now) {
                stalled = found_busy;
                found_busy = false;
                let step =
                    scenario.cost_step.map_or(
                        1.0,
                        |(at, factor)| if in_window >= at { factor } else { 1.0 },
                    );
                turns += 1;
                let self_run = if turns % SELF_EVERY == 0 {
                    SELF_FACTOR
                } else {
                    1.0
                };
                used += rng.lognormal(scenario.mean_cost * step, COST_SIGMA) * self_run;
                busy_until[mind] = now
                    + (busy_rng.lognormal(BUSY_MEAN_S * scenario.busy_scale, BUSY_SIGMA) * 1000.0)
                        as i64;
                last_turn = now;
                if sensor == Sensor::Stream && !gap {
                    reading = sense(used.min(1000.0), now, resets);
                }
            } else {
                stalled = true;
                found_busy = true;
                out.stalls += 1;
            }
        }
        used = used.min(1000.0);
        let error =
            f64::from(TARGET) * (now - (resets - WINDOW_MS)) as f64 / WINDOW_MS as f64 - used;
        if window + 1 == scenario.windows {
            if in_window >= 144.0 {
                worst_day = worst_day.max(error.abs());
            }
            if in_window >= 96.0 {
                let now_side = if error > BAND {
                    1
                } else if error < -BAND {
                    -1
                } else {
                    0
                };
                if now_side != 0 {
                    out.band_crossings += u32::from(side != 0 && side != now_side);
                    side = now_side;
                }
            }
        }
    }
    out.final_used.push(used);
    out.worst_error_last_day = worst_day;
    out.jumps_per_h = f64::from(jumps) / (f64::from(scenario.windows) * 168.0);
    out.frac_at_min = f64::from(at_min) / f64::from(last_day.max(1));
    out.frac_at_max = f64::from(at_max) / f64::from(last_day.max(1));
    out
}

const SENSORS: [Sensor; 2] = [Sensor::Stream, Sensor::Poll];
const SEEDS: [u64; 3] = [11, 23, 47];

/// The most chatter, in jumps per hour, `GAINS` may show on any scenario.
const JUMPS_PER_H_BOUND: f64 = 0.5;
/// How far from the target, in permille, a reachable scenario may end.
const FINAL_TOLERANCE: f64 = 30.0;
/// The worst |error| on the last day: the vote scenarios' recovery sets it.
const LAST_DAY_ERROR_BOUND: f64 = 75.0;

#[test]
fn sim_scenarios_meet_target() {
    for scenario in scenarios() {
        for sensor in SENSORS {
            for seed in SEEDS {
                let o = run(&scenario, sensor, &GAINS, seed);
                let tag = format!("{} {sensor:?} seed {seed}: {o:?}", scenario.name);
                assert!(o.jumps_per_h <= JUMPS_PER_H_BOUND, "chatter {tag}");
                if scenario.stress {
                    continue;
                }
                if scenario.unreachable {
                    assert_eq!(o.last_beat, FALLBACK_MIN_S, "saturated {tag}");
                    assert!(o.frac_at_min > 0.9, "saturated {tag}");
                    assert!(
                        o.final_used.iter().all(|u| *u < f64::from(TARGET) - 100.0),
                        "{tag}"
                    );
                    continue;
                }
                if scenario.overspent {
                    assert_eq!(o.last_beat, FALLBACK_MAX_S, "backs off {tag}");
                    assert!(o.frac_at_max > 0.9, "backs off {tag}");
                    continue;
                }
                for used in &o.final_used {
                    assert!(
                        (used - f64::from(TARGET)).abs() <= FINAL_TOLERANCE,
                        "final {tag}"
                    );
                }
                assert!(
                    o.worst_error_last_day <= LAST_DAY_ERROR_BOUND,
                    "error {tag}"
                );
                assert!(
                    o.band_crossings <= scenario.max_crossings,
                    "oscillation {tag}"
                );
            }
        }
    }
}

/// `cargo test -p mind-body tempo::sim::sweep -- --ignored --nocapture`
#[test]
#[ignore]
fn sweep() {
    let scen = scenarios();
    let mut rows = Vec::new();
    for kp in [0.01, 0.02, 0.03, 0.05, 0.1] {
        for ki in [0.002, 0.005, 0.01, 0.02, 0.04] {
            for kd in [0.0, 0.002, 0.005, 0.01, 0.02, 0.05] {
                let g = Gains { kp, ki, kd };
                let (mut final_err, mut err_day, mut jumps, mut cross) = (0f64, 0f64, 0f64, 0u32);
                let mut sat_ok = true;
                let mut detail = String::new();
                for s in &scen {
                    let (mut f, mut e, mut j, mut x) = (0f64, 0f64, 0f64, 0u32);
                    let (mut lastb, mut stalls) = (0, 0);
                    for sensor in SENSORS {
                        for seed in SEEDS {
                            let o = run(s, sensor, &g, seed);
                            j = j.max(o.jumps_per_h);
                            if s.stress {
                            } else if s.unreachable {
                                sat_ok &= o.last_beat == FALLBACK_MIN_S && o.frac_at_min > 0.9;
                            } else if s.overspent {
                                sat_ok &= o.last_beat == FALLBACK_MAX_S && o.frac_at_max > 0.9;
                            } else {
                                f = o
                                    .final_used
                                    .iter()
                                    .fold(f, |m, u| m.max((u - f64::from(TARGET)).abs()));
                                e = e.max(o.worst_error_last_day);
                                cross = cross.max(o.band_crossings);
                                x = x.max(o.band_crossings);
                            }
                            lastb = o.last_beat;
                            stalls = stalls.max(o.stalls);
                        }
                    }
                    final_err = final_err.max(f);
                    err_day = err_day.max(e);
                    jumps = jumps.max(j);
                    detail.push_str(&format!(
                        "    {:<14} final_err {f:6.1}  worst_err_last_day {e:6.1}  jumps/h {j:5.2}  crossings {x}  last_beat {lastb}  stalled_min {stalls}\n",
                        s.name
                    ));
                }
                let rank = final_err + 0.5 * err_day + 20.0 * jumps + 15.0 * f64::from(cross);
                rows.push((rank, g, final_err, err_day, jumps, cross, sat_ok, detail));
            }
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!(
        "kp ki kd | worst final err | worst err last day | worst jumps/h | worst crossings | saturates when unreachable or overspent"
    );
    for r in &rows {
        println!(
            "{} {} {} | {:.1} | {:.1} | {:.2} | {} | {}",
            r.1.kp, r.1.ki, r.1.kd, r.2, r.3, r.4, r.5, r.6
        );
    }
    for r in rows.iter().take(4) {
        println!("\nBEST {:?}\n{}", r.1, r.7);
    }
    if let Some(r) = rows.iter().find(|r| r.1 == GAINS) {
        println!("\nCHOSEN {:?}\n{}", r.1, r.7);
    }
}

/// Against a ceiling the target cannot be reached under, the controller told
/// that a beat found every Mind busy winds its integral less, so it overspends
/// less at the end than the one that is not told.
#[test]
fn the_busy_report_lowers_the_overshoot_against_a_ceiling() {
    let scenario = scenarios().into_iter().find(|s| s.stress).unwrap();
    for sensor in SENSORS {
        for seed in SEEDS {
            let told = drive(&scenario, sensor, &GAINS, seed, true);
            let blind = drive(&scenario, sensor, &GAINS, seed, false);
            let tag = format!("{sensor:?} seed {seed}: told {told:?} blind {blind:?}");
            assert!(told.stalls > 0, "the ceiling binds {tag}");
            assert!(told.final_used[0] <= blind.final_used[0], "final {tag}");
            assert!(
                told.worst_error_last_day < blind.worst_error_last_day,
                "error {tag}"
            );
        }
    }
}
