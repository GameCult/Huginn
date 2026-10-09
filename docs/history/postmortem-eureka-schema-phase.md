<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/postmortem-eureka-schema-phase.md; text below this line is unchanged. -->
# Eureka pipeline state, schema-ownership phase: postmortem

Date: 2026-09-16. Written at the phase boundary, not at the end of the
campaign. Cuts 1-6d closed Epiphany's stake in the work; Cuts 7-16 build
Huginn and the `eureka-state` client and are still running. Evidence: the cut
map `notes/eureka-pipeline-state-cut.md`, the branch
`codex/eureka-pipeline-state`, and the Soul reports recorded in the map's
Landed section.

## Summary

Epiphany now owns the schemas for Eureka's pipeline state, in one leaf
library that a separate organ can depend on without dragging the runtime in
with it. `epiphany-pipeline` is 2,444 lines with its tests, 34 of them, and
publishes 13 derived schemas. It has no dependency on `epiphany-core`; the
dependency runs the other way. The public surface is five doors:
registration, prepare, decode, write-envelope validation, and reference
validation.

The phase landed across 113 commits in three days. At writing, the leaf is
pinned by Huginn at `5cda0886` and nothing in Epiphany blocks the organ.

## Scope and invariants

The target was schema ownership, and only that. The organ, the index, the
CultNet surface and the MCP client were all excluded from this repo by
design, because the previous shape put a store, a lease and git preconditions
inside Epiphany and the operator rejected it mid-campaign.

The invariants that had to survive: one owner per decision, a key grammar in
which no reader can excuse a document it does not like, schemas that derive
byte for byte from the types rather than being maintained beside them, and a
leaf that a second repo can consume without inheriting the runtime.

The earlier attempt failed by bundling. The repo-owned store design is kept
in the map under History, clearly marked dead, rather than deleted, because
the reason it died is the reason Cut 4 exists.

## Timeline

| Cut | What it owned | Outcome |
|---|---|---|
| 1-2 | Re-pin CultLib, collapse to one commit owner | Landed, Soul-closed |
| 3a | Ten document kinds and schemas, plus store, lease, git gates | Landed, then half of it deleted by Cut 4 |
| 4 | Delete the repo-owned store whole | 1,302 lines removed, 549 added |
| 5 | Collapse the store profile back out | Landed |
| 6 | Extract `epiphany-pipeline` as a leaf | Landed after two fix batches |
| 6b | The key grammar | Three fix batches; the grammar was redesigned rather than patched a fourth time |
| 6c | The Ghostlight shapes | Landed, one fix batch plus two harness repairs |
| 6d | Resolution history and stewardship by sequence | Landed both sides; the Huginn follow-up needed its own fix batch |
| follow-up | The public reference validator | Landed both sides, `5cda0886` and `5ba7b0c6` |

## Structural delta

Cut 4 is the honest headline: it deleted 1,302 lines and added 549, and
retired one store format, one lease and two git preconditions. Cut 6 moved
the types out of `epiphany-core` rather than copying them, so the extraction
was a move and not a fork.

The estimates were wrong in a consistent direction. Cut 4's test arithmetic
was off because the deleted module held 20 tests on Windows rather than 15,
and Cut 6's `epiphany-core` estimate of −623 should have read about −1,060
once the test module travelled with the code. Both corrections are recorded
in the map against the sections they falsify. Cut 6d's test delta was +181,
not the +173 claimed.

Nothing was parked in this phase.

## What Soul caught

Green tests passed all of the following.

**Split and mislocated authority.** The epoch guard had no test that could
even construct an envelope to reach it, so an `if false` mutation survived.
Self ruled deletion over testing what nothing can construct, and Cut 8 wrote
it in the organ where admission can reach it. Separately, the write path was
believed live and was in fact test-only; `decode` and the CultCache imports
looked live only because dead code mentioned them.

**A rule open one level up, three times.** The key grammar was patched twice
and the same invariant came back both times. The third occurrence was the
signal to redesign rather than patch. Soul then found no collision in 212,450
adversarial keys against the new grammar.

**Readers excusing what writers refuse.** The reader accepted a trailing dot
on the root and the local while the writer refused it, so a reference with
trailing junk passed the reader against its own documentation. A mutant
excusing the `Instance` root from the kind check survived: exactly the shape
Cut 6b existed to kill.

**Tests pinning spelling instead of rules.** A depth test restated the
literal 64 rather than referencing the bound, so moving both to 60 passed it.
Fixtures that used the sequence 1 at every nesting level left a nested
resolution's own sequence unpinned.

**A regression introduced by an earlier fix.** An exact replay of any batch
carrying a derived write was refused rather than recognised as already
admitted, because the derivation recomputed a sequence over an image that
already held the first admission's record. The replay test had used only
batches without derivations.

**Ordering by key rather than by meaning.** The hand-off replay picked its
subject by key order, so from the eleventh transfer on, where `n11` sorts
before `n2`, it derived a withdrawal of the wrong record. Ten round trips
alone did not expose it.

**Dead code with a test that could not reach it.** The resolution-base arm of
the reinstatement rule was unreachable because the depth cap fired first.
Deleting it was the point: a rule defended by a test that cannot run is the
failure mode of subtraction, not a defence against it.

## Operator corrections

**The repo-owned store.** The operator rejected it mid-campaign, and Cut 4
deleted it whole rather than demoting it. This is the single largest
correction of the phase and the reason the leaf is clean.

**A hand-off is a transfer, not a lease.** Self had modelled every stewardship
hand-off as a loan that returns. The operator's example was Odin holding a
swarm of infrastructure tools and spinning off a new steward when the
workload justifies it, with no return. The missing context was that Self had
only ever seen the cross-cutting case, where borrowing is the natural reading.

**Withdrawn resolutions are kept.** Self's default threw the record away.
The operator's model was a subject as a question with one canonical answer
and an affordance for withdrawal, where the withdrawn answer is context to
learn from. The operator also named the assumption the ruling rests on: that
the context actually reaches the agents. Self had landed the opposite default
and reverted it.

**Single-writer semantics are CultCache's.** Self attributed them to redb.
redb is the storage engine underneath; the owned store lock and the
compare-and-swap batch are CultCache's, and were built in this same session.

**Stop spending the scarce model on Hands.** The default model inherits the
root's, so every dispatch had been running on Fable without anyone choosing
it. The rule now is to name the model on every dispatch.

## Incidents

**Cargo through the Bash tool.** `CARGO_TARGET_DIR=C:\...` has its
backslashes collapsed, and cargo starts a from-scratch build into a path that
does not exist. Nothing landed in the shared directory. Cargo on this host
runs through PowerShell only.

**A scoped `cargo clean` against the brief.** Hands freed and partly retook
about 400 MB of the shared target directory. Soul could not establish that
another crate's artifacts were evicted. The next brief forbids it by name.

**Agents ending their turn mid-job, three times.** The briefs now name the
mechanism rather than the intent: a wait is a foreground poll loop inside one
tool call. One agent left detached timers that re-fired notifications for an
hour, so the briefs also forbid background monitors that outlive the run.

**The harness ate fixes.** An early mutation runner restored from a sidecar
that predated the fix it was testing, reverting real work. A later version
committed a NUL byte and a literal control character into a target. The
current harness reads the original as bytes, writes the sidecar before any
write, restores hash-first, attempts every target, and prints the original
digest before rethrowing. A tool-enforced timeout was the exact path that
left a tree mutated, so the harness now owns its own timeout and kills the
process tree inside its own cleanup.

**Map splices landing at the wrong heading depth.** A specification written
in a scratchpad began one level deeper than the map's section, and the fold
put every heading under it a level too deep.

**My own errors in the map**, each corrected in place: a follow-up number
used twice, an ungreppable regex, seventeen entries described as nineteen, a
zip digest labelled as the DLL's, a claim that CI was green throughout, an
overstated scar, and one commit attributed to its neighbour.

## What worked

**Mutation suites as the unit of proof, not test counts.** Every operator
ruling and every new rule gets a test that fails under its own mutation, and
the suite is committed with a no-op control that proves the harness restores
bytes exactly. The controls caught the harness eating fixes; test counts
never would have.

**A revert mutation and a loosening mutation per rule.** A plain revert is
too easy a target. The loosening is what found the reader excusing a root
kind and the unpinned nested sequence. When a loosening turns out to be
unfalsifiable by construction, it is demoted to a recorded note with the
reason rather than left in the suite implying coverage it does not have.

**Redesigning on the third occurrence.** The key grammar came back twice
under patches. The rule that emerged is to treat the third appearance of an
invariant as evidence about the design, not about the instance.

**Keeping dead designs marked rather than deleted.** The repo-owned store
sections carry banners naming their false sentences. A reader who finds the
old model finds it labelled.

**Not capping the Soul loop on a foundation cut.** Cuts 6 and 6b each took
four passes and the later passes still found real defects.

## What to change in the pipeline

All four of these were approved by the operator on 2026-09-16 after a cost
review that put roughly half the overhead on process waste and roughly half
on the inescapable cost of a second agent.

1. **A loosening mutation per rule, alongside the revert.** Landed in the
   Hands brief.
2. **One harness in the Eureka repository.** Owed. The PowerShell harness
   still lives in `F:\Projects\Epiphany\tools\` and Huginn reaches across for
   it; CultLib's JavaScript runner needs the same contract by name.
3. **Reports are findings, promises and numbers only, and a second Soul pass
   is scoped to the fix batch diff.** Landed in both briefs.
4. **A map commit per cut, rulings excepted.** In effect.

## Open follow-ups

- **Cut 9's spec text calls the `V10L` loosening failable.** It was demoted
  when the reference validator made it unfalsifiable. Stale text in the map's
  Cut 9 section, harmless to the code.
- **FU-9: CultLib's lock failure is text-matched.** `cultcache-rs` needs a
  typed error. Owner is CultLib, and it can wait because the only consumer
  matching the text is a test.
- **FU-1 through FU-8** stand as recorded in the map, each with its file and
  its reason.
- **Cut 11 must index withdrawn resolutions with their reasons.** This is the
  second obligation the operator's Q17 ruling created; the first, a history
  view, landed in Cut 9.
