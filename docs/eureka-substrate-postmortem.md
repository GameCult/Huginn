# eureka-substrate postmortem (cuts 2026-10-01)

The campaign fixed the mind's correctness core that the first typed Eureka
campaign (idunn-watchdog) tripped over: keys too long to resolve, key errors
reported as outages, report heads compared by spelling, findings that could
not defer to a cut, an unchecked `depends_on`, a stewardship ritual nobody
needed, and a faculty still named `MindSteward`. Target:
`docs/eureka-substrate-target.md`; map: `docs/eureka-substrate-map.md`. The
campaign stays open as the standing home of substrate gaps (ruling `gap-home`).

## What landed

| Cut | Repo | Merge / commit | Soul passes |
|---|---|---|---|
| merge-epiphany-main (0a) | Epiphany | `2bf39eef` | 1, holds |
| merge-huginn-main (0b) | Huginn | `922e6194` | 1, holds |
| leaf-key-bound | Epiphany | `ef956685` | 1, 2 Lows deferred |
| huginn-pin-and-refusals | Huginn | `b85cd5d7` | 2 |
| huginn-admission-rules | Huginn | `107552dc` | 2 |
| ops-mind-upgrade | gamecult-ops | `1f4cb12e`, `0dd1a1a3` | 1 |
| skill-recipes | Eureka | `e7567333`, `243182cc` | 2 |

Live: Huginn `107552dc` serves `eureka` on Yggdrasil since 12:00:59 UTC
(14:00:59 CEST). Before and after the flip, 346 documents and 108 receipts.
The read-back test on a scratch copy of the cold copy validated and re-keyed
all 346 documents and withdrew 45 long-keyed resolutions. The first admission
with a local key over 64 bytes went in at 12:15:10 UTC, so rollback is now
restoring `/var/backups/huginn-upgrade/107552dc…/mind.redb`, not a flip.

Closed with it: six substrate gaps (`gap-unresolvable-finding-key`,
`gap-sha-identity-literal`, `gap-defer-finding-to-cut`,
`gap-depends-on-pins-revision`, `gap-stewardship-ceremony`,
`gap-faculty-enum-name`), and the seven idunn-watchdog findings that the key
bound had left stuck in force (three Fixed, four Deferred to the follow-ups
already carrying them).

## Soul's ledger

10 verdicts, 14 findings: 8 Fixed, 3 Deferred (to `leaf-bound-hygiene` and
`leaf-typed-prepare`), 3 Recorded. No Blocker or High. Every finding was in
the tests or the prose, none in shipped behaviour, which says the cuts were
small enough for Hands to get right and the suites were not yet strict enough
to prove it:

- The read-back test first proved one withdrawal and skipped refusals
  (`readback-tolerates-refusal`). Rebuilt, it censused the whole mind and
  became the deploy's gate. That gate is the most valuable artefact of the
  campaign.
- Exact-match rules were pinned with fixtures that cannot tell exact from
  prefix (`dep-near-miss`) or case (`dep-case`): labels `1, 2, 3, 9`. A
  fixture for an equality rule needs near misses; digits alone are degenerate.
- `cargo mutants` scored 8/8 on admission-rules but mutates `check()` and
  `matrix()` whole, so the score defended no individual rule. Soul's hand
  mutants that were functions of the input (prefix in one direction, length
  compare, case fold) found what the tool could not.
- The docs pass left a recipe steering Self to the stewardship step the
  operator had dropped (`stale-steward-recipe`). Deleting a rule from code does
  not delete the prose that teaches it; the docs cut needs its own grep for
  every consumer of the dropped step, not only its name.

## What the campaign itself proved about the substrate

- **The bug bit its own fix.** Self's first attempt to close two findings of
  the pin cut was refused `InvalidFormat` wrapped as `Unavailable`, the very
  defect the cut was fixing. Admitting a resolution had to wait for the deploy.
  Any campaign that fixes the mind's admission runs on the unfixed mind until
  the upgrade; plan resolutions after it.
- **A finding cannot be `Answered`.** Closing the self-dependency fork by its
  ruling was refused `IncompatibleResolution`; it was closed `Fixed` by the
  commit that implemented the ruling.
- **Old clients survived the rename.** The pre-upgrade `eureka-state` decoded
  every response used afterwards, including receipts carrying `Life`, because
  the receipt digest excludes provenance. Not every kind was exercised through
  the old client.
- **The cold-copy-then-read-back pattern worked as designed**: build, record
  counts, stop, copy, prove on a scratch copy, flip, recount, view every
  document. Total downtime was about three minutes.

## What went wrong in the running of it

- A merge was run with `git merge -F -` (stdin), which git rejected; the
  following push was a no-op and the worktree was removed anyway. Nothing was
  lost because the branch was pushed, but the chain should have stopped on the
  failed merge. Write merge messages to a file.
- The admission-rules spec's line anchors were taken at `133324a7`; Hands
  branched from `b85cd5d7` and re-anchored by content. `base` cannot say
  "main after X merges" (`gap-depends-on-pins-revision`, second half, still
  open as prose in `first`).
- Self decided the self-dependency fork as a Defaulted ruling on proportion
  rather than asking. The operator may overturn `eureka-substrate:ruling:self-dep`.

## Open

- Fresh-session check: the Rehydrate block and recipes in a new session, which
  also loads the swapped client and its `Life` faculty.
- 15 substrate gaps in force under `eureka-substrate` (verify stopgap,
  landing records, faculty attribution, mutation triage field, and others).
- Leftovers to clean: `~/huginn-readback-107552dc…` and
  `~/huginn-build/107552dc…` on Yggdrasil; the client clone in the session
  scratchpad. Keep the cold copy until the operator retires it.
