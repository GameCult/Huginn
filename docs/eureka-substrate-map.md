# Eureka substrate: map

Status: map, Imagination (`imagination-substrate`), 2026-10-01. Campaign
`eureka-substrate` in Eureka's mind. Questions, cut specs and rulings are
typed documents there; this page keeps body facts, the model page and
rationale only. Target: `docs/eureka-substrate-target.md` (`133324a7`).

Pinned heads for every fact below:

| Repo | Ref | Commit |
| --- | --- | --- |
| Huginn | `origin/main` | `133324a7f78047164f0ca02331398066a971ac15` |
| Huginn | `origin/eureka/memory-organ` (deployed release) | `9dd6d8007ab7de84c926a34edd3947ba15d1f01c` |
| Epiphany | `origin/main` | `4d1113ef0a4dbcead2e2c3cdc43aa13ef773dbb8` |
| Epiphany | `origin/codex/eureka-pipeline-state` | `4006b14bf29b49410bd18b43acb4d42c3c4e2443` |
| Epiphany | leaf pinned by Huginn (`crates/huginn-mind/Cargo.toml:22`) | `73edee5cdaddece8bda9c6c3271e8ddd064337f9` |
| CultLib | `cultcache-rs` pinned by both | `8fc74c709444b884a2bd339e0c306bf9484d99d2` |
| gamecult-ops | `main` | `b4eea88d801ad45aad2a49574a9eb2f750c7dd97` |
| Eureka skill | `main` | `094083566e19cccca026de8cbfc24462f1debe93` |

Source reads were made in scratch clones at the pinned commits
(`scratchpad/imagination-substrate/{epiphany,huginn}`); line numbers are
against those commits. `lib.rs` below is `epiphany-pipeline/src/lib.rs`.

## Body facts

### B1. Key grammar (source read, leaf at `73edee5c`)

- `LOCAL_MAX = 64` (`lib.rs:877`) bounds every composed local, whole
  (`local`, `lib.rs:892-901`). It is the only bound on a resolution chain's
  depth (comment `lib.rs:917-922`, test `lib.rs:2105-2150`).
- A resolution's local is `<subject kind>.<subject local>.n<seq>`
  (`lib.rs:923-929`). Overhead over the subject's local: `finding` 11 bytes,
  `follow_up` 13, `question` 12, `ruling` 10, `cut_spec` 12, `stewardship` 15
  at one sequence digit. A withdrawal (resolution of a resolution) adds
  `resolution.` and `.n<seq>`, 14 more.
- So with subjects bounded at 64 and resolutions at 64, a finding local over
  53, a follow-up label over 51, a question label over 52, a ruling label over
  54 or a cut-spec local over 52 admits and can never be resolved. Self's map
  said follow-up labels over 52; it is 51 (`follow_up` is 9 bytes, plus two
  dots and `n1`).
- `pipeline_id` (`lib.rs:798-819`) parses every id's local with `dotted_text`
  (`lib.rs:148-158`), whose 64-byte bound is the same one `Slug` uses
  (`lib.rs:251`). Every `PipelineRef` validation goes through it
  (`validate`, `lib.rs:589`; `validate_ref`, `lib.rs:609`), and so do
  Huginn's view (`query.rs:433`), index (`index.rs:120`) and selection keys and
  cites (`rows.rs:306`, `rows.rs:331`). Raising `LOCAL_MAX` alone therefore
  makes a key that no id parser accepts. The parser bound has to move with it.
- The epoch is `epiphany.pipeline.epoch.v2` (`lib.rs:886`). `value_types!`
  (`lib.rs:400-419`) emits no serde default on any field, so adding a field
  would break the decode of stored documents. **No cut in this map adds a
  field**, so this constraint is recorded and not exercised.
- The published schemas (`schemas/cultnet/epiphany.pipeline.*.v2.schema.json`)
  carry no key bound. A resolution's `subject.id` is `Short`, 200 bytes. A key
  is not a field, so no published schema changes when a local bound moves.
- Storage: `cultcache-rs` stores rows in redb as
  `TableDefinition<&[u8], &[u8]>` (`CultLib packages/cultcache-rs/src/lib.rs:1023`
  at `8fc74c70`), with no key bound of its own. A Qdrant point id is the
  SHA-256 of the document id (`huginn-daemon/src/index/mod.rs:225-231`), so
  key length does not reach the index.
- Id length ceiling: `<root ≤64>:<kind ≤11>:<local>` must fit `Short` (200)
  wherever an id is cited. That allows a local of up to 123 bytes.

### B2. Census of the live mind (probe)

- Command: `query` for every question, ruling, cut_spec, target, stewardship
  and follow_up (119 ids), then every finding and resolution (138 ids), header
  projection. The ids were extracted with Python, and the lengths of each
  depth-one resolution local (`n1`) and depth-two withdrawal local were
  computed.
- Where and when: the eureka-state tools from Starfire, 2026-10-01 about
  08:40 UTC, `as_of` 82, mind status 293 documents, 82 receipts.
- Result (256 resolvable-kind documents): the longest stored local is 64 and
  the longest id is 90.
  - **Six subjects can have no resolution at all** (n1 local over 64). All
    are findings in force under `idunn-watchdog`:
    `cut-bifrost-retire-alarm.s2.verb-default-accepts-malformed` (69),
    `cut-ops-notice-deploy.s2.pinned-bifrost-predates-retry` (65),
    `cut-bifrost-notice-retry.s1.unknown-test-ignores-backoff` (67),
    `cut-bifrost-notice-retry.s2.closed-unknown-retry-unpinned` (68),
    `cut-bifrost-notice-retry.s2.flapping-clock-retries-every-tick` (72), and
    `cut-idunn-topology-lock.s2.boot-reconcile-skipped-on-contention` (74).
    That matches Self's count. The seventh finding in force,
    `cut-idunn-incident.s3.mutants-share-target-dir`, can be resolved (57),
    but its resolution could never be withdrawn (71).
  - **39 stored resolutions can never be withdrawn** (their withdrawal local
    is 65-78). So their subjects can never be reopened, which is a lifecycle
    the matrix allows (`admission.rs:555-560`, Q17 B). Two follow-ups
    (`gap-substrate-followups-campaign-rooted` under both roots, and
    `projection-mode-branch-vs-private-lock`) and about 40 resolved findings
    have the same latent defect one level up.
- Conclusion: an admission guard alone cannot satisfy
  `every-subject-resolvable` for stored documents, because they cannot be
  re-keyed (`stored-documents-valid`). The resolution's local bound has to
  widen.

### B3. Refusal classification (source read, Huginn at `9dd6d80`)

- `Mind::admit` (`huginn-mind/src/admission.rs:73-85`) calls
  `document.validate()` (which returns a typed refusal), then
  `document.prepare(cache)`. `prepare` (`lib.rs:714-721`) runs
  `pipeline_key(self)?` into `anyhow`, and admission maps any `prepare` error
  through `unavailable` (`mind.rs:46-48`) to `MindRefusal::Unavailable`. The
  derived writes take the same path (`admission.rs:136`). A ruling whose
  derived `Answered` resolution cannot be keyed is refused `Unavailable` there.
- The daemon passes the outcome through unchanged
  (`huginn-daemon/src/daemon.rs:143`). So `Refused(Unavailable{..})` reaches
  the client as an admit answer, not as `isError`.
- The later A3 step (`stage` through `validate_pipeline_write_envelope`,
  `lib.rs:1003-1015`) returns typed refusals. Only the prepare path is
  misclassified.
- `Unavailable` is also raised legitimately, for store and decode faults
  (`admission.rs:949`, `admission.rs:2091`, `docs.rs:48`, `mind.rs:140`,
  `receipt.rs:209`). Those stay.

### B4. Smaller rules (source read, Huginn at `9dd6d80`)

- **Sha identity:** the one place shas are compared is
  `admission.rs:433`, `commit.sha == report.range.head`, a literal string
  equality. `Sha` is 7-40 lowercase hex (`lib.rs:253`). `FullSha` (40) exists
  for `ForeignRef` (`lib.rs:255`, `lib.rs:467`).
- **Deferred:** the matrix (`admission.rs:562-585`) allows
  `(Finding, Deferred { to })` only when `to.kind == FollowUp`
  (`admission.rs:574`). The leaf's `Deferred { to: PipelineRef }` already takes
  any kind, so this is an admission change only.
- **Faculty:** `Faculty` lives in Huginn, not in the leaf
  (`huginn-mind/src/receipt.rs:32-40`, variant `MindSteward`). The selection
  alias domain is a second copy (`rows.rs:42`). The eureka-state client takes
  the type from huginn-mind (`eureka-state/src/main.rs:27`, `main.rs:66`), so
  the client must be rebuilt for a new variant.
- Probe: `query` with `faculty any_of [MindSteward]` matched 0 documents
  (2026-10-01 08:40 UTC). No stored receipt carries the old name.
- **Stewardship:** admission reads stewardship in one live rule. A campaign
  needs a stewardship in force for each repo (`admission.rs:363-366`,
  `RepoNotStewarded`). The hand-off rule (`admission.rs:501`, `NotStewarded`)
  is parked with Cut 12. Six stewardship documents are stored.
- **depends_on:** this is `Vec<Short>[8]` (`lib.rs:525`). Huginn's edge list
  says outright that `depends_on` is a label, not an edge
  (`huginn-mind/src/docs.rs:398-404`). Nothing checks it. The only reader is
  the header projection (`query.rs:121`, `query.rs:173`). The
  `idunn-watchdog` specs stored revision ids in it. That came from the skill's
  habit, not from the code's intent.
- `open_checked` (`mind.rs:180-188`) re-validates epoch, types and identity,
  not keys or bounds, so an opened mind never re-judges a stored key. No
  migration path exists, and none is needed under this map.

### B5. Branches and in-flight work (git, 2026-10-01 ~08:35 UTC)

- Huginn: `memory-organ` is 107 commits ahead of the merge base `91b7fcf`;
  `main` is 1 ahead (`133324a`, docs only). `git merge-tree --write-tree
  origin/main origin/eureka/memory-organ` is clean, tree `993ab1fd`. Its only
  difference from `9dd6d80` is `docs/eureka-substrate-target.md`.
- Epiphany: the branch is 174 ahead of the merge base `b5d9d4ff`; `main` is 1
  ahead (`4d1113ef`, the Ghostlight pin). Merge-tree is clean, tree
  `a75aa388`, and **identical** to the branch tip's tree: the branch already
  carries the same Ghostlight pin and lock. The leaf tree `cf84c032` is
  identical at `73edee5c`, at the branch tip and in the merge.
- Hands worktrees `hands/rs3` (`9673ab8`, idle since 2026-09-29 06:32),
  `hands/cut13a` (`3dc50ee`, 2026-09-30 00:16) and `hands/cut13b` (`cd3dfc0`,
  2026-09-30 04:55) are each 0 commits ahead of `memory-organ`, and clean. RS-3
  and BP-3 have landed (BP-3's deferred bodies are in `huginn-daemon/src/bodies.rs`).
  **Nothing is in flight.** The only other unmerged Huginn branch is
  `codex/windows-atomic-replace-retry` (2026-05-24), which is unrelated.
- No Epiphany branch has unmerged commits under `epiphany-pipeline/`.
- `F:\Projects\Huginn`'s main worktree is checked out on `eureka/memory-organ`,
  so the merge has to run in its own worktree.

### B6. Deployment (read-only ssh, Yggdrasil, 2026-10-01 08:41 UTC)

- `readlink /opt/gamecult/huginn/current` is
  `releases/9dd6d8007ab7de84c926a34edd3947ba15d1f01c`. The previous release
  `0c4e4107…` is kept. `huginn` is active since 2026-09-30 13:21:08 UTC.
  `ExecStart` is as in the runbook (`--state-root /var/lib/gamecult/huginn
  --instance eureka …`).
- `minds/` holds one mind, `eureka`: `mind.redb` 4,198,400 bytes, mtime
  2026-10-01 08:33, owned `huginn:huginn` 0640. This is the only mind on the
  host.
- The nightly snapshot `/var/backups/gamecult-state/2026-10-01/` holds
  `./var/lib/gamecult/huginn/minds/eureka/mind.redb` (01:59 UTC), recorded
  `stable-double-read` in `consistency.tsv`. It predates receipts 81-82 (this
  campaign's frame), so the deploy cut takes its own cold copy.
- Runbook: `gamecult-ops runbooks/huginn-yggdrasil.md:42-72`. Its release
  section builds at a GitHub commit and keeps the previous release. It has no
  pre-upgrade copy step and no read-back proof.

## Model page

One row per persistent kind or rule that this campaign touches. **Identity**:
what names it. **Lifecycle**: what happens to it over time. **Authority**:
who decides.

| Kind or rule | Identity | Lifecycle | Authority |
| --- | --- | --- | --- |
| Subject keys (`finding`, `follow_up`, `question`, `ruling`, `cut_spec`, `target`, `stewardship`) | `<root>:<kind>:<local>`, with the local bounded at 64 bytes; injective (`composed_keys_cannot_collide`, `lib.rs:1392`). **Unchanged by this campaign**: every stored key stays exactly as it is. | Admitted once; never re-keyed. A revision is a new key. | The leaf derives it (`pipeline_key`); admission refuses a mismatch (A3). Forbidden: any writer composing a key by hand. |
| Resolution keys | `<subject root>:resolution:<subject kind>.<subject local>.n<seq>`, and one nesting for a withdrawal. **Target:** bounded by its own constant, derived from the deepest legal chain over the longest subject local at `u32::MAX` sequences: 10 + 1 + (11 + 1 + 64 + 2 + 10) + 2 + 10 = **111 bytes**. Ids up to 64 + 12 + 111 = 187 bytes, within `Short`. | n1, n2 … per subject; withdrawn by a depth-two resolution; never deeper (Q19 A, admission). Stored resolutions keep their keys and become withdrawable. | The leaf owns the bound and the id parser; admission owns sequence, matrix and depth. The parser bound (`pipeline_id`) is derived from the kind and is no longer `dotted_text`'s 64. |
| Every-subject-resolvable guard | **Empty by design** under Q `key-bound` option A: with subjects at 64 and resolutions at 111, every admissible subject is resolvable to depth two at any sequence, so no guard exists to own. Under option B a guard would be admission's. | | |
| Refusal classes | `MindRefusal::Document(PipelineRefusal)` for shape, bound and key; `Unavailable` only for store, cache and decode faults. | Fixed at admission time per batch. | Huginn admission. The leaf's `prepare` keeps the cache-encode error as `anyhow`. Admission keys the document first, so a key error is typed. Forbidden: `unavailable()` wrapping a leaf refusal. |
| `depends_on` | Free `Short` text today. **Target:** cut labels in the spec's own campaign (Q `depends-on`). | Stored revision-id values stay as they are (they decode, and nothing re-judges them). New batches follow the target rule. | Under A, admission refuses a new spec whose entries are not labels of cuts with a spec in the campaign. Under B, only the skill convention. |
| Sha identity | `Sha` 7-40 hex. **Target:** two shas name the same commit when one is a prefix of the other. | Only judged in a new report's batch. | A leaf method on `Sha` (the type owns its equality semantics); admission calls it at `admission.rs:433`. |
| `Deferred` referent | `PipelineRef`, any kind in the leaf. **Target:** follow_up or cut_spec, for a finding. | When the cited spec is later superseded, the reader follows `superseded_by`; nothing is rewritten. | Huginn's matrix (`admission.rs:574`). |
| Faculty | `receipt.rs` enum plus the `rows.rs:42` alias domain. **Target:** `Life` replaces `MindSteward`, with `#[serde(alias = "MindSteward")]` so any old receipt decodes. 0 stored receipts carry it (B4). | Attribution on the receipt, never re-written. | huginn-mind. The client picks it up only when rebuilt. |
| Stewardship admission rule | Rule `RepoNotStewarded` on campaign repos. | Q `stewardship-rule`. Stored stewardship documents stay; under A they decide nothing until Cut 12 (hand-off and leasing) defines a rule again. | Huginn admission; the operator owns whether the rule exists. |
| The gap home | Root `eureka-substrate`, labels `gap-*` (ruling `gap-home`). | Already moved (19 follow-ups in force here; the 20 `idunn-watchdog` copies resolved). Closed by `Fixed` when a cut lands, by Self after Soul holds. | Self admits; the skill recipe names the root (`campaign-state.md:278-299`, already done at Eureka `0940835`). No code. |
| The six stuck findings | Unchanged keys under `idunn-watchdog`. | They become resolvable when the leaf cut deploys. Self triages them then, like any finding (Deferred or Recorded). This is Self triage, not a fork. | Self. |
| Deployed release | `/opt/gamecult/huginn/releases/<full sha>`, `current` symlink. | Built at a main commit, flipped in, the previous release kept, flipped back on failure. | Operator's deployment authority, exercised by Self or Hands (memory `deploy-instruction-is-authorization`). |

Cells that could not be filled are stated in the rows above: the guard row
is empty by design if `key-bound` A is ruled, and the `depends_on` and
stewardship authorities wait on their questions.

## Rationale

**Why a per-kind bound and not a bigger `LOCAL_MAX`.** If one number bounds
every local, then widening it widens the subjects too, and the gap comes back
one level up: a 100-byte finding label has a 111-byte resolution. Keeping
subject locals at 64 and deriving the resolution bound from the deepest legal
chain makes resolvability a property of the grammar, with no admission guard
to drift. It also re-keys nothing: every stored key was at most 64 and stays
valid. A guard alone (keeping 64 everywhere) would refuse future offenders.
It would leave the six stuck findings and the 39 unwithdrawable resolutions
permanently broken, which violates the target for stored documents.

**Why the parser bound moves with it.** Every reader validates ids through
`pipeline_id`. A resolution key the leaf can derive but `view`, `keys` or
`cites` refuses would turn an unresolvable subject into an unreadable
resolution.

**Cut order.** The merges come first (ruling `merge-to-main-first`). Then the
leaf, because Huginn pins it. Then Huginn admission, which bumps the pin. Then
the client rebuild, because the new `Life` variant crosses the wire in
receipts. Then the deploy, which proves the read-back. The skill cut goes
last, because it must not tell agents to admit as `Life` before the client
and daemon understand it. In cut labels: `merge-epiphany-main`, then
`merge-huginn-main`, then `leaf-key-bound`, then `huginn-pin-and-refusals`,
then `huginn-admission-rules`, then `ops-mind-upgrade`, then `skill-recipes`.
The client rebuild is a step inside `ops-mind-upgrade`. It produces no repo
commit, and a `cut_report` needs one (`gap-fork-before-commit`). The new
daemon's added refusal variants and `Life` must not reach an old client for
long, so the client swap sits next to the daemon flip.

**Why Cut 0 does not bump the leaf pin.** After the Epiphany merge, `73edee5c`
is an ancestor of Epiphany `main`, and the leaf tree is byte-identical. So the
daemon built from Huginn `main` is built from main code
(`code-from-main`), with an unchanged `Cargo.lock`, and the deployed release
stays as it is. The pin moves in the Huginn admission cut, to the leaf cut's
merge on Epiphany `main`.

**Proof of `stored-documents-valid`.** Each code cut runs one read-back test
over a real copy of the mind. Every envelope decodes; `pipeline_key` of the
decoded document equals the stored key; `validate()` passes; and for every
resolvable subject the n1 resolution key and its withdrawal key derive and
parse back. The test reads the path from `HUGINN_MIND_SNAPSHOT` and is
`#[ignore]` by default, so CI never needs the file. Before code cuts, the
copy is the 2026-10-01 nightly snapshot. The deploy takes a cold copy with the
unit stopped.
