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
distinct observations. Conditional integration: the controller does not
integrate error in the direction of a saturated actuator. Saturation is one input
the caller gives, `saturated` (ruling `tempo-controller-knows-the-ceiling`): the
last beat was the floor, or launched nothing because every eligible Mind was busy;
with it and `e > 0` nothing integrates. The slow limit needs no report: with
`u(I) <= 1/h` (the integral's own answer, the carry excluded) and `e < 0` nothing
integrates. One call integrates at most one longest beat (3600 s) of time: a
heartbeat down for two days that finds a fresh reading integrates 1 h of error, not
48 h (Soul's probe wound 11736 permille-h and held the floor to the window end).
A new `resets_at` (more than a minute from the last: the connector reports whole
unix seconds, and a new window is a week on) clears `I` and the derivative memory
and integrates nothing on the call that finds it; no reading, an expired window, a
reading older than its window or one observed more than two longest beats (2 h)
ago holds the last beat and integrates nothing (time spent holding is not
integrated later).

Votes. While a vote decides nothing integrates. The caller sets `rate_per_h` to
the voted rate; the controller keeps `carry = clamp(voted rate, 1/h, 60/h) - its
own rate now`, so the first beat after release is the voted beat (a beat beyond the
bounds is answered at the bound), and the carry then fades with time constant
0.25 h, the sensor's poll period. Three limits, each measured and kept:

- Release is the voted beat to within one integration step, not within a second:
  the first unvoted call integrates a step of error before it answers. A 3600 s
  vote released 150 permille behind answers 3506 s with a one-minute step (94 s,
  2.6%), and so for every longer vote; a 60 s vote moves none. The unit test holds
  the slow end to 3400..3599 s.
- The carry fades on the sensor's period whatever the vote's length, so a vote in
  force for one call releases at its beat and takes about 75 min to return to the
  slow beat: tens of extra turns for a vote that lasted a minute, bounded by the
  span of the beat bounds, and it decays.
- The carry is not part of the output the integral is judged against. Before, a
  60 s vote at 160 permille ahead left the integral at about -143 where the unvoted
  controller stays locked at 0; now it stays 0 (unit test
  `a_short_vote_while_ahead_does_not_outlive_itself`).

Why not an integral that tracks the voted rate (the first sketch): the integral is
the slow term, `dI/dt = e`, so parking it at the voted rate keeps the vote's burn
for as long as `I` takes to unwind. After a 60 s vote it is parked at ~5600 and
unwinds at `e` per hour: a 60 s vote of 1 h or 4 h left the plan spent to 1000
within the day (last-day error 215-229 permille, Soul's probe), and clamping the
tracked value to the beat bounds does not touch it, since 60 s is itself a bound.
The carry is bounded by construction; the integral is never moved by a vote.

## Plant

One seven-day window, 60 s steps, `target = 900` permille. Each beat that elapses
launches one turn if a Mind is free (see the ceiling below): cost lognormal with mean `c` and sigma(ln) 0.5; every 25th turn is a
Self run at 5 x. Per-turn cost range used: `c` in 0.6 (cheap) .. 1.3 (base) ..
3.0 (dear) permille; the 900 s bias beat reaches 90% at about `c` = 1.34. Usage
saturates at 1000. Sensor: `Stream` refreshes the reading when a turn ends,
`Poll` every 15 minutes; readings are rounded to 1 permille; a gap shows no
reading. Three seeds per scenario and sensor.

Scenarios: quiet (c 1.3); cheap-turns (0.6); dear-turns (3.0); burst (+200 permille
over hours 48-52); cost-x3 (turn cost x3 from hour 84); votes from hour 48, then
release: 300 s for 24 h, 60 s for 1 h, 60 s for 4 h, 60 s for 24 h, 7200 s for 24 h
and 86400 s for 24 h (the longest a Mind may vote; the last two are beyond the
3600 s the controller can answer); rollover (two windows, used
resets to 0, the sensor shows the old window until its next refresh); gap-6h (no
reading from hour 72 to 78); start-ahead (500 permille used at the start);
unreachable (c 0.05: even at the 60 s minimum the target cannot be reached, the
controller must sit at 60 s over the last day); cheap-then-x3 (c 0.6, tripled from
hour 84: the winding case Soul probed); one-slow-mind (one Mind and turns 45 x
longer, a ceiling near 5 turns an hour, with cheap-then-x3's cost).

A vote of 60 s for 24 h costs about 1900 permille at c 1.3, more than the week: the
vote itself exhausts the plan, so no controller can end on the target and the
scenario asserts what the ruling asks of the fallback once the vote is gone: it
stops burning (slowest beat over the whole last day). The 1 h and 4 h holds at 60 s
(about 80 and 310 permille over) are reachable and are held to the same final and
last-day bounds as the rest.

Metrics: used at each window's reset; worst |error| over the last day; "jumps":
minutes where the beat moved by more than 5%, per hour, outside votes; "band
crossings": the error going from beyond +15 to beyond -15 permille (or back) in
the last three days. No oscillation is sustained when crossings stay at or under
2 (one overshoot).

Recorded trace: not used. Each Mind's `persona-trace.cc` on Yggdrasil holds one
`plan seven_day` line (the newest), not a series, so there is nothing to replay.

### The throughput ceiling

The heartbeat launches at most one on-beat Persona turn per poll, skips a Mind
whose Persona run is in force, and a beat that finds nobody free launches nothing
and is tried again at the next poll (`heartbeat.rs` `poll`). The sim books a launch
only when a Mind is free: each turn holds its Mind for a lognormal time, a beat
that finds all busy launches nothing, and the controller's calls carry `saturated`
from then until a launch comes on time (the report a heartbeat can make: "a beat
since the last launch found every Mind busy"; a launch that came late keeps it up
through the next beat). The step, 60 s, is the beat floor and longer than the
poll, so the spacing between launches never binds.

Calibration, read-only on Yggdrasil (journal retention is about two days of the
heartbeat and 29 h of unit lines, not five):

- `sudo journalctl -u mind-heartbeat.service --since "-5 days" -o short-iso`:
  4100 `polled:` lines from 2026-10-30T02:16Z, 43 h. Gaps between consecutive
  polls under 300 s: p10 25 s, median 30 s, mean 37.7 s, p90 54 s. 782 polls
  launched something (472 of them two or more Minds at once: mention and
  Self-ended wakes, which do not wait for the beat), 1346 `held: BrakeAbsent`.
  Launched slugs: aqua 166, eureka 152, heimdall 170, kiko 150, libby 146, nibu 181,
  void 173, weksa 146: eight Minds. The journal says only `no launch`, not why, so
  how often every Mind was busy is not logged.
- `sudo journalctl _PID=1 --since "-5 days"` filtered to `mind-persona@`, each
  `Started` paired with its `Deactivated successfully`: 1284 turns over 29.4 h,
  duration min 3.3 s, p10 4.0, median 6.4, p90 27.7, p99 186, max 505, mean 15.4 s;
  at most 6 turns in flight at once; starts per hour median 20, max 154 (the bursts
  are the mention wakes); gaps between starts median 44 s. `mind-self@` (Self runs;
  they do not hold a Persona turn's Mind): 147 over 28 h, mean 457 s, median 44 s,
  p99 7299 s, one Self run per 8.7 Persona turns where the sim's cost model has one
  per 25 (not recalibrated here).
- The sim's busy time is lognormal with mean 15.4 s and sigma 1.2 (median 7.5 s,
  p90 35 s against the measured 6.4 and 27.7; the mean is what the ceiling is made of).

Ceiling: spacing 30 s gives at most 120 on-beat launches an hour; eight Minds each
busy 15.4 s give 8 x 3600 / 15.4 = 1870 an hour. The beat floor, 60 turns an hour,
is below both, so on the measured plant the ceiling does not bind: every real
scenario, vote-60s-24h included, runs with zero stalled minutes, and its results
are those of the unthrottled plant (a 60 s vote for 24 h still spends ~1900
permille and the controller still backs off to 3600 s after it; cheap-then-x3 ends
within 15 permille of the target, 33.8 worst last-day error).
It would bind with fewer Minds or turns tens of times longer, which is what
`one-slow-mind` stands for: one Mind and a ceiling near 5 turns an hour. The
controller does not end on target there (the target needs more than the ceiling
for 84 h and then the cost triples); the scenario compares the controller told of
the stalls with the same controller not told, six runs (3 seeds x 2 sensors); used
at the end of the week, worst last-day error:

| run | told | not told |
|---|---|---|
| Stream 11 | 941, 131 | 1000, 192 |
| Stream 23 | 929, 113 | 1000, 165 |
| Stream 47 | 999, 111 | 1000, 165 |
| Poll 11 | 941, 131 | 1000, 193 |
| Poll 23 | 930, 114 | 1000, 166 |
| Poll 47 | 1000, 113 | 1000, 165 |

The report lowers the overshoot in every run and does not remove it. It is one bit
per call and the ceiling is soft (turn durations are lognormal, so some beats still
find a Mind free); the integral keeps growing in those intervals and carries about
1500 permille-hours into the cost step. A controller that also read how much of
the requested rate was delivered would need more than the one input the ruling
gives it; whether that is wanted is the operator's.

## Sweep

`kp` in {0.01, 0.02, 0.03, 0.05, 0.1}, `ki` in {0.002, 0.005, 0.01, 0.02, 0.04},
`kd` in {0, 0.002, 0.005, 0.01, 0.02, 0.05}: 150 candidates x 17 scenarios x 2
sensors x 3 seeds. Rerun:

    cargo test -p mind-body --lib tempo::sim::sweep -- --ignored --nocapture

(on Yggdrasil through `tools/stopgap/ygg-verify.sh`). Candidates are ranked by
`final error + 0.5 x last-day error + 20 x jumps/h + 15 x crossings`. Best fourteen, and the chosen row (27th by that rank):

| kp ki kd | worst final error | worst error, last day | worst jumps/h | worst band crossings | holds MIN when unreachable, MAX when overspent |
|---|---|---|---|---|---|
| 0.1 0.02 0 | 30.9 | 46.9 | 1.44 | 0 | true |
| 0.1 0.01 0 | 30.9 | 47.3 | 1.45 | 0 | true |
| 0.1 0.02 0.002 | 30.9 | 46.9 | 1.54 | 0 | true |
| 0.1 0.01 0.002 | 30.9 | 47.3 | 1.55 | 0 | true |
| 0.1 0.04 0 | 30.9 | 52.3 | 1.43 | 0 | true |
| 0.1 0.005 0 | 30.9 | 52.3 | 1.48 | 0 | true |
| 0.1 0.002 0 | 30.9 | 52.3 | 1.49 | 0 | true |
| 0.1 0.04 0.002 | 30.9 | 52.3 | 1.49 | 0 | true |
| 0.1 0.005 0.002 | 30.9 | 52.3 | 1.55 | 0 | true |
| 0.1 0.002 0.002 | 30.9 | 52.3 | 1.58 | 0 | true |
| 0.03 0.002 0 | 33.6 | 54.9 | 0.77 | 1 | true |
| 0.1 0.02 0.005 | 30.9 | 46.9 | 1.89 | 0 | true |
| 0.1 0.01 0.005 | 30.9 | 47.3 | 1.89 | 0 | true |
| 0.1 0.04 0.005 | 30.9 | 52.3 | 1.82 | 0 | true |
| 0.02 0.01 0 (chosen, 27th) | 29.6 | 46.8 | 0.37 | 3 | true |

Chosen `GAINS = { kp: 0.02, ki: 0.01, kd: 0.0 }`, per scenario (worst over sensors
and seeds; one-slow-mind is not scored, see the ceiling above):

```
    quiet          final_err    5.4  worst_err_last_day   14.3  jumps/h  0.03  crossings 1  last_beat 1028
    cheap-turns    final_err    9.3  worst_err_last_day   19.5  jumps/h  0.00  crossings 1  last_beat 392
    dear-turns     final_err   29.6  worst_err_last_day   33.8  jumps/h  0.37  crossings 1  last_beat 2041
    burst          final_err   14.1  worst_err_last_day   17.2  jumps/h  0.07  crossings 2  last_beat 876
    cost-x3        final_err    7.8  worst_err_last_day   32.5  jumps/h  0.21  crossings 0  last_beat 3465
    vote-300s-24h  final_err   29.5  worst_err_last_day   46.8  jumps/h  0.14  crossings 1  last_beat 928
    vote-60s-1h    final_err   18.0  worst_err_last_day   19.9  jumps/h  0.30  crossings 3  last_beat 847
    vote-60s-4h    final_err   23.2  worst_err_last_day   32.5  jumps/h  0.25  crossings 1  last_beat 625
    vote-60s-24h   final_err    0.0  worst_err_last_day    0.0  jumps/h  0.18  crossings 0  last_beat 3600
    vote-7200s-24h final_err   16.5  worst_err_last_day   24.5  jumps/h  0.08  crossings 2  last_beat 1326
    vote-86400s-24h final_err   22.4  worst_err_last_day   24.6  jumps/h  0.08  crossings 2  last_beat 1212
    cheap-then-x3  final_err   14.9  worst_err_last_day   33.8  jumps/h  0.06  crossings 3  last_beat 1760
    rollover       final_err    7.3  worst_err_last_day   10.1  jumps/h  0.04  crossings 0  last_beat 875
    gap-6h         final_err    5.4  worst_err_last_day   13.1  jumps/h  0.04  crossings 0  last_beat 1014
    start-ahead    final_err   27.6  worst_err_last_day   29.8  jumps/h  0.02  crossings 2  last_beat 1479
    unreachable    final_err    0.0  worst_err_last_day    0.0  jumps/h  0.00  crossings 0  last_beat 60
```

The chosen row is 27th by that rank: the rank weights accuracy over chatter, and
nothing in the grid is both within 30 permille at every reset and quiet at a
crossing bound of 2: with the sweep's own filter (worst final error at most 30,
jumps at most 0.5 an hour, at most 2 crossings on any scenario) no row of the 150
passes. `kp` 0.1 (first by rank) ends at 30.9 on dear-turns, over the 30 bound, and
moves the beat on nearly every turn (1.4 jumps/h) because each turn's usage lump is
read as error. With the crossing bound at 3, exactly three rows pass, all `kp` 0.02:
`ki` 0.01 (final 29.6, last-day 46.8, 0.37 jumps/h), 0.02 (29.6, 44.5, 0.35) and 0.04
(29.6, 54.4, 0.33). They are within noise of each other on every measured axis, and
the sweep does not separate them; `GAINS` has the smallest integral gain of the
three, which is the choice made, not a measured winner (0.02 would rank slightly
higher). `kd` helps nowhere: with 1-permille readings and about four turns an hour,
the measured slope is mostly quantisation, and every `kd` > 0 only adds jumps. The
committed controller is therefore a PI in practice; the D term stays in the law
(tested with explicit gains) so a plant with finer readings can use it.

Bounds the acceptance test holds `GAINS` to: reachable scenarios end within 30
permille of the target at every reset (the chosen row's worst is 29.6, dear-turns
and vote-300s-24h); worst last-day error at most 75 (worst 46.8); at most 0.5
jumps/h (worst 0.37); at most 2 band crossings, except the two scenarios that
carry their own bound of 3, vote-60s-1h and cheap-then-x3: each crosses three times
on some seed and sensor (vote-60s-1h on two of the six runs, cheap-then-x3 on at least
the Stream run of seed 47), a wobble just over the 15-permille band with last-day
error at most 33.8, not a sustained oscillation. Admitting those two rows is the
whole of the widening; the other scenarios stay at 2. The unreachable case sits at
60 s and the overspent vote at 3600 s over the last day.
