# Tempo fallback PID: plant, sweep, chosen gains (2026-11-01)

Rulings: `tempo-default-is-usage-fitter`, `tempo-fallback-pid-tuned-by-sim`,
`minds-control-burn-democratically`, `tempo-silence-is-abstention`. The
controller is `tempo::control` in `crates/mind-body/src/tempo.rs`; `fallback_beat`
is it with the committed `GAINS`. The plant is `crates/mind-body/src/tempo/sim.rs`
(test-only). Nothing is wired into the heartbeat yet.

## Control law

Actuator: the turn rate `u` in beats per hour (usage per hour is roughly linear
in `u`; the beat is not). `target(now) = target_permille x elapsed / 7 d`,
`e = target(now) - used` (permille, positive means behind).
`u = 3600/900 + kp e + ki I - kd d(used)/dt`, `beat = clamp(round(3600/u), 60, 3600)`
seconds, `u <= 0` gives 3600. Derivative on the measurement and only between
distinct observations; conditional integration (no integrating when the output is
at a limit and `e` pushes further into it); while a vote decides, `I` tracks the
voted rate so release is bumpless; a new `resets_at` clears `I` and the derivative
memory; no reading, an expired window or a reading older than its window holds the
last beat and integrates nothing (time spent holding is not integrated later).

## Plant

One seven-day window, 60 s steps, `target = 900` permille. Each beat that elapses
is one turn: cost lognormal with mean `c` and sigma(ln) 0.5; every 25th turn is a
Self run at 5 x. Per-turn cost range used: `c` in 0.6 (cheap) .. 1.3 (base) ..
3.0 (dear) permille; the 900 s bias beat reaches 90% at about `c` = 1.34. Usage
saturates at 1000. Sensor: `Stream` refreshes the reading when a turn ends,
`Poll` every 15 minutes; readings are rounded to 1 permille; a gap shows no
reading. Three seeds per scenario and sensor.

Scenarios: quiet (c 1.3); cheap-turns (0.6); dear-turns (3.0); burst (+200 permille
over hours 48-52); cost-x3 (turn cost x3 from hour 84); vote-300s-24h (a standing
vote of 300 s from hour 48 to 72, then release); rollover (two windows, used
resets to 0, the sensor shows the old window until its next refresh); gap-6h (no
reading from hour 72 to 78); start-ahead (500 permille used at the start);
unreachable (c 0.05: even at the 60 s minimum the target cannot be reached, the
controller must sit at 60 s over the last day).

A vote of 60 s for 24 h, as first sketched, costs about 1900 permille at c 1.3,
more than the week; it is unreachable by construction, so the vote scenario uses
300 s.

Metrics: used at each window's reset; worst |error| over the last day; "jumps":
minutes where the beat moved by more than 5%, per hour, outside votes; "band
crossings": the error going from beyond +15 to beyond -15 permille (or back) in
the last three days. No oscillation is sustained when crossings stay at or under
2 (one overshoot).

Recorded trace: not used. Each Mind's `persona-trace.cc` on Yggdrasil holds one
`plan seven_day` line (the newest), not a series, so there is nothing to replay.

## Sweep

`kp` in {0.01, 0.02, 0.03, 0.05, 0.1}, `ki` in {0.002, 0.005, 0.01, 0.02, 0.04},
`kd` in {0, 0.002, 0.005, 0.01, 0.02, 0.05}: 150 candidates x 10 scenarios x 2
sensors x 3 seeds. Rerun:

    cargo test -p mind-body --lib tempo::sim::sweep -- --ignored --nocapture

(on Yggdrasil through `tools/stopgap/ygg-verify.sh`). Candidates are ranked by
`final error + 0.5 x last-day error + 20 x jumps/h + 15 x crossings`. Best fifteen:

| kp ki kd | worst final error | worst error, last day | worst jumps/h | worst band crossings | holds MIN when unreachable |
|---|---|---|---|---|---|
| 0.1 0.04 0 | 30.9 | 54.9 | 1.43 | 0 | true |
| 0.1 0.04 0.002 | 30.9 | 54.4 | 1.49 | 0 | true |
| 0.1 0.02 0 | 30.9 | 56.8 | 1.44 | 0 | true |
| 0.1 0.02 0.002 | 30.9 | 56.8 | 1.54 | 0 | true |
| 0.1 0.01 0 | 30.9 | 65.7 | 1.45 | 0 | true |
| 0.1 0.04 0.005 | 30.9 | 54.9 | 1.82 | 0 | true |
| 0.1 0.01 0.002 | 30.9 | 65.7 | 1.55 | 0 | true |
| 0.1 0.02 0.005 | 30.9 | 56.8 | 1.89 | 0 | true |
| 0.02 0.01 0 | 30.0 | 65.7 | 0.37 | 2 | true |
| 0.1 0.01 0.005 | 30.9 | 65.7 | 1.89 | 0 | true |
| 0.1 0.005 0 | 30.9 | 85.9 | 1.48 | 0 | true |
| 0.01 0.02 0 | 30.9 | 54.4 | 0.07 | 3 | true |
| 0.01 0.04 0 | 29.6 | 56.7 | 0.10 | 3 | true |
| 0.1 0.005 0.002 | 30.9 | 85.9 | 1.55 | 0 | true |
| 0.02 0.01 0.002 | 30.0 | 65.7 | 0.67 | 2 | true |

Chosen `GAINS = { kp: 0.02, ki: 0.01, kd: 0.0 }`, per scenario (worst over sensors
and seeds):

```
    quiet          final_err    5.4  worst_err_last_day   14.3  jumps/h  0.03  last_beat 1028
    cheap-turns    final_err    9.3  worst_err_last_day   19.5  jumps/h  0.00  last_beat 392
    dear-turns     final_err   29.6  worst_err_last_day   33.8  jumps/h  0.37  last_beat 2041
    burst          final_err   14.1  worst_err_last_day   17.2  jumps/h  0.07  last_beat 876
    cost-x3        final_err    7.8  worst_err_last_day   32.5  jumps/h  0.21  last_beat 3465
    vote-300s-24h  final_err   30.0  worst_err_last_day   65.7  jumps/h  0.02  last_beat 1442
    rollover       final_err    7.3  worst_err_last_day   10.1  jumps/h  0.04  last_beat 875
    gap-6h         final_err    8.7  worst_err_last_day   13.1  jumps/h  0.04  last_beat 1014
    start-ahead    final_err   27.6  worst_err_last_day   29.8  jumps/h  0.02  last_beat 1479
    unreachable    final_err    0.0  worst_err_last_day    0.0  jumps/h  0.00  last_beat 60
```

The chosen row is ninth by that rank; the rank weights accuracy over chatter, and the choice trades two crossings for a quarter of the jumps. `kp` 0.1 removes the overshoot after the vote (0 crossings) but moves the
beat on nearly every turn (1.4 jumps/h) because each turn's usage lump is read as
error; 0.02 with `ki` 0.01 is nearly as accurate with a quarter of the chatter and
one overshoot. `kd` helps nowhere: with 1-permille readings and about four turns an
hour, the measured slope is mostly quantisation, and every `kd` > 0 only adds jumps
(0.37 to 0.67 at `kd` 0.002). The committed controller is therefore a PI in
practice; the D term stays in the law (tested with explicit gains) so a plant with
finer readings can use it.

Bounds the acceptance test holds `GAINS` to: reachable scenarios end within 35
permille of the target at every reset (30 was the first sketch; the dear-turns and
vote scenarios sit at 29.6-30.0 and a Self run in the dear scenario is itself 15
permille); worst last-day error at most 75; at most 2 band crossings; at most 0.5
jumps/h; the unreachable case sits at 60 s.
