<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/eureka-read-side-cut.md; text below this line is unchanged. -->
# Huginn read side: the consumer cut map

Date: 2026-09-22. Imagination output (Opus). Nothing here is committed and no
repository was edited. Self places and commits this file. Its natural home is a
section of `F:\Projects\Epiphany\notes\eureka-pipeline-state-cut.md`, the
campaign that owns the read-side rulings, with a pointer from §12 of
`F:\Projects\CultLib\docs\cultnet-selection-cut.md`.

Status, 2026-09-29 (Self): **RS-L, RS-2 and RS-1 are landed and closed**
(2026-09-22; the record is in `eureka-pipeline-state-cut.md`, "RS-L closes",
"RS-2 and RS-1 are closed"). **P-1 and P-2** were folded into selection's R-G
and R-M, and selection Cut 1 merged to CultLib `main` at `dbee044`
(2026-09-23); the RS-3 brief confirms both are in that merge. **Both operator
questions in §9 are ruled** (2026-09-22: Q-RS1 B, one non-empty `Title`;
Q-RS2 A, `huginn-mind` links `cultnet-rs`). **Next:** BP-2 once BP-1 lands
(BP-1 in Hands 2026-09-29), then **RS-3**. The Pins table below is as of
2026-09-22 and stale; each brief re-takes its anchors.

**2026-09-29: BP-2 and RS-3 landed and closed.** BP-2 is at Huginn `e770fb9` and leaf `4891f2e5`,
with pins at CultLib `6251cbc`. RS-3 is at `511afa2`, `eb57bdb` and `9673ab8`: huginn-mind 91 tests,
daemon 19, and nothing reimplemented. Soul closed it; in-force, the snapshot restriction and the
single owner of repo identity all held under probe. Recorded:
- one orphan record takes every read offline, failing closed on a corruption the single writer
  cannot produce;
- a cursor that dies with a restart is refused with the substrate's message about a "selection
  digest". That is CultLib's wording to fix;
- the selection `in_force=true` + `cited answers exists=false` misses a question reopened by a
  withdrawal, because the withdrawn answer still cites it. Cut 15's recipes must say so;
- header pages are bounded per header, not per page, and `ResponseTooLarge` stays reachable.
**Next: BP-3**, in Hands.

*As written 2026-09-22:* cut map for five cuts and two CultLib prerequisites.
**P-1** and **P-2** go to the selection campaign's next fix batch, before
`cultnet/selection-cut1` merges. **RS-L** is the leaf (Epiphany). **RS-1**,
**RS-2** and **RS-3** are Huginn. **BP-2** belongs to the body-plane map and
is widened here by one mechanical clause.

**Rulings this map is written under.**

- **Operator, 2026-09-17** (Epiphany map, "Operator rulings on the read side"):
  - the generic vocabulary lives in CultNet, and the organ consumes it;
  - receipt ordinals: yes. The stored type changes, the store takes a new
    version, and an existing store refuses to open rather than being misread;
  - `open_items` and `history` are deleted, not kept as presets;
  - `question` and `ruling` gain a `Short` title in the leaf, and it rides the
    same store version as the ordinal.
- **R-3 (selection map §16):** `admitted_after` and `admitted_before` are deleted
  from the organ's wire. The latest N is `descending, limit: N`.
- **Q13 A (Epiphany map, 2026-09-16):** the wire types live in `huginn-mind`,
  and the client depends on that crate for types only.
- **What stays with the organ:** the row and reference implementations, the
  roles, in force, the summary, the snapshot restriction and the deletions.
- **S6 (Cut 10 residue, recorded 2026-09-22) is owned here:**
  - declared names used only as read filters are never checked against the
    grammar;
  - `OrgRepo` claims a format it does not enforce.

## Pins

| Repo | Branch | HEAD | State |
|---|---|---|---|
| Huginn | `eureka/memory-organ` | `4f7e3b5` | **Dirty: the Cut 10 residue batch is in Hands.** It modifies seven files: `huginn-mind/Cargo.toml` (+8, a `test-support` feature), `mind.rs` (+74), `admission.rs` (±6), `daemon.rs`, `serve.rs`, `huginn-daemon/Cargo.toml` and the cut10 entries file. Anchors below are against `4f7e3b5`. **Hands re-takes them after the residue lands:** `mind.rs` lines shift, and `admission.rs` lines may shift. This map was not written against the uncommitted diff. |
| Epiphany | `codex/eureka-pipeline-state` | `b3ae787b` | Clean. The leaf changed after `9d3a3efd` only in tests (+32 lines at `lib.rs:2484`), so leaf anchors hold at both. Huginn pins the leaf at `9d3a3efd`. |
| CultLib | `main` | `070fac0` | The selection branch is not merged. `origin/main` is 14 commits past the merge base `b3d9cf7`. |
| CultLib | `cultnet/selection-cut1` | `00f4c02` | Selection Cut 1 finished executing. Its final Soul pass is running. Rust anchors are against `git show 00f4c02:packages/cultnet-rs/src/selection.rs`. |

**Pins in use:**

- Huginn: `cultcache-rs` and `cultnet-rs` at `a0813c6`, and the leaf at `9d3a3efd`.
- The leaf: `cultcache-rs` at `a0813c6`.

`git diff --stat a0813c6 00f4c02 -- packages/cultcache-rs` is **empty**. In the
same range, `cultnet-rs` gains selection (+3,111/−21 over 6 files).

## 0b. Model page: identity, lifecycle and authority

Every persistent or long-lived kind this cut touches has a row. No cell is empty.

| Kind | Identity (what names it) | Lifecycle | Authority (who decides) |
|---|---|---|---|
| **Commit receipt** (`huginn.mind_commit_receipt.v2`) | `receipt_id` = `mind-commit-` + sha256 of `(instance, strong_reads, writes)`. **`ordinal` is not part of the identity.** It is excluded from the digest, as `committed_at` and `provenance` are (`receipt.rs:10-14`), so an exact replay still finds its stored receipt (A9). | Written once in its batch's own swap. It never changes and is never deleted. A replay answers `AlreadyAdmitted` with the stored receipt. | `receipt.rs`. `candidate` (`:160-178`) is the one construction site. `commit` (`:211-245`) is the one swap. |
| **Admission ordinal** (a field on the receipt) | A `u64` position in this mind's admission history. It starts at 1 and is dense: the receipts carry exactly `{1..=N}`. **Head** means `N`, the number of receipts. | Assigned when a batch is admitted, as `head + 1`, under the single writer: `&mut Mind` plus redb's lifetime lock. It is never reused. A lost swap lands nothing, so it leaves no gap. | `receipt.rs` owns both `head()` and assignment. Admission asks it for `head + 1`. The read side asks it for `head` as the current `asOf`. Both refuse a chain that is not dense, with `Unavailable`. No other code computes an ordinal. |
| **Store version** | The epoch record `huginn.mind_epoch.v1`, keyed by and naming the leaf's `PIPELINE_SCHEMA_EPOCH` (`mind.rs:25-42`), together with every stored type id. Each id carries its own version. | The epoch moves when the leaf breaks its schema. A type id moves when its owner breaks that type. An old store is refused by the opener and never migrated. That is the operator's ruling, and it costs nothing today (F5). | The leaf owns the epoch string and its thirteen type ids. Huginn owns the receipt's type id and the gates. **The epoch gate runs first** (RS-1), so a store written at an old epoch is refused as `ForeignEpoch`. A foreign type in a store at the current epoch stays `ForeignStore`. |
| **Question and ruling title** | The field `title` on `PipelineQuestion` and `PipelineRuling`. Its type depends on Q-RS1. | Written by the author, immutable like every leaf field, and required. | The leaf owns the field and its bound. Admission validates it through `PipelineDocument::validate`, as it does every other field. |
| **Selection cursor** | An opaque string minted by `cultnet_rs::Cursor::mint`. It carries `asOf`, the last `(ordinal, schemaId, recordKey)`, and a digest of the selection with `cursor` and `limit` cleared. | One per page that has a next page. It lives as long as the client keeps it. **It never goes stale on Huginn:** the mind is append-only with one writer, so the organ can always present rows as of any `asOf ≤ head`. | The substrate mints it, parses it and checks its digest. The organ decides which `asOf` to answer at: the cursor's own, and never `head`, because the substrate refuses `cursor_stale` whenever the two differ (F7). An `asOf > head` is refused `CursorInvalid`: this mind could not have minted it. |
| **Snapshot image** (the rows as of `asOf`) | `asOf`. | Built per read call and thrown away with it. Nothing is stored. | `query.rs` `Reader::at(mind, as_of)` builds it. It holds the documents whose writing receipt has `ordinal ≤ asOf`, and **every derivation runs over that restricted image**: in force, the closing resolution, citations and base. |
| **Declared vocabulary**: aliases, roles and their value domains | Alias and role names, which are the organ's closed lists (RS-3 §"Vocabulary"). | Changes only with code. There is no catalog. A new alias or role is a normal change. | `huginn-mind` `rows.rs`, through `RowSet`. The substrate door checks the **names**. The organ's door checks the **values**, which is S6. |
| **Citation edge** | `(from: PipelineRef, role: CitationRole, to: PipelineRef)`. | Derived from the leaf fields each time it is read. Never stored. | `docs.rs` `citations()` is the one edge list. Admission's A7 (`admission.rs:242-285` today) and the read side's `Row::references` both read it. |
| **Header (summary)** | `PipelineDocumentSummary`, keyed by the row's `id`. | Derived each time it is read, and bounded by `SUMMARY_MAX_BYTES`. | `query.rs`. It is the organ's meaning of `projection: header` (selection §2 "Projection"). |
| **Published schemas** | The leaf's thirteen `epiphany.pipeline.<kind>.v2.schema.json` files plus `index.json`. Huginn's `huginn.mind_request.v1` and `huginn.mind_response.v1`. | They are regenerated whenever the derivation changes, and pinned byte for byte by a test (leaf `lib.rs:2324`; Huginn `wire.rs:210-216`). The request schema `$ref`s CultLib's published selection schema by its `$id` (F9). | The leaf owns its thirteen. Huginn owns its two. CultLib owns `cultnet.selection.schema.json`, and nothing copies it. |
| **Mutation suites** | Files: `tools/eureka-cut9-mutations.psd1` (V entries), `tools/eureka-cut10-mutations.psd1` (D/S entries), and the new `tools/eureka-readside-mutations.psd1` (Huginn) and `tools/eureka-readside-leaf-mutations.psd1` (Epiphany). | The entries whose rule died with a deletion are **deleted** (RS-2). The entries whose rule survives under a new spelling are **re-authored** in the readside suite (RS-3). Every file keeps its M0 control. | Hands writes them. The harness is `~/.claude/skills/eureka/tools/eureka-mutations.ps1`. |

Forks this page surfaced: **Q-RS1** (what a title is) and **Q-RS2** (whether
`huginn-mind` may link `cultnet-rs`). Both are in §9.

## 1. Body facts

Each fact comes from a source read at a named HEAD, and wherever a mechanism
claim needed it, from a run. The probes are in the session scratchpad:

- `readside-probe/` was built against `cultnet-rs@00f4c02` and `huginn-mind` by
  path. That path was the `4f7e3b5` tree plus the residue batch's uncommitted
  edits, read only; see "Not probed".
- `storev2/` holds copies of the leaf and of `huginn-mind`, emulating the
  version bump.
- `schemaprobe/`.

The target directory is `C:\Users\Meta\.cargo-target-codex\imag-readside`: 941
files, 0.37 GiB, cold build 3m07s. Nothing was cleaned.

### F1. The four reads today (`query.rs` at `4f7e3b5`)

**What every read shares.** Every read builds `Reader::new`
(`query.rs:206-209`). That decodes **the whole image**
(`docs.rs:42-52`) and **every receipt** (`mind.rs:186-196`, then
`AdmissionIndex::build`, `query.rs:134-150`), on every call. A document that no
receipt wrote refuses as `Unavailable`. A document that two receipts wrote
refuses the same way.

**`view`** (`:323-327`). It asks `validate_ref` first (V24), then finds the
`(kind, key)` by a linear search, then joins the facts and the status.

**`query`** (`:331-350`):
- It refuses `semantic` before the reader exists (V8/V8L).
- It scans every held document.
- `Reader::matches` (`:249-290`) applies these filters:
  - `campaign` = the key's root segment (V18/V18L: the root, not a field);
  - `repo` and `cut`, read through `base()` (`:237-246`), which walks
    resolutions down to their subject;
  - `kinds`;
  - `in_force`;
  - `faculty`;
  - `admitted_after` and `admitted_before`, compared as strings.
- It orders by `(admitted_at, id)` (`:307-311`).
- It clamps `limit` into `1..=200` (`QUERY_LIMIT_MAX`, `:32`), and reports
  `matched` before the cap.

**`open_items`** (`:355-391`). It filters all views to the campaign root, then
builds five lists:
- in-force questions, findings and follow-ups;
- in-force cut specs that no `cut_report.cut_spec` names;
- cut reports that no `verdict.cut_report` names.

It is uncapped.

**`history`** (`:401-435`):
- `Subject(ref)` asks `validate_ref`, reads `resolutions_of`, and orders by
  sequence.
- `Repo(org_repo)` reads `assignments_of(mind, repo)` and orders by sequence.
  **Nothing checks the `OrgRepo`.** The doc comment at `:399-400` claims "the
  type already holds [it] to its own format". It does not: `Bounded` is
  crate-private to the leaf, and a deserialised `OrgRepo` is never validated.
  This is S6.

**Wire.** `HuginnMindRequest` (`wire.rs:42-50`) and `HuginnMindResponse`
(`:81-90`) carry one variant each per read. **Daemon.** The arms are at
`daemon.rs:84-99`. The instance gate at `:66-71` runs for every read.

**Probe, run.** A real redb mind built through the organ's public API
reproduces the `open_items` scenario of `query.rs:943-1003` and three answer
cycles for `history`. Every list the organ answered was then re-answered
through `cultnet_rs::select` over rows built from the organ's own views. **All
seven relations match, in members and in order:** the five open lists,
`history(Subject)` and `history(Repo)`.

`specs_without_report` came back `[cut-9.r2, cut-12.r2]`. That is admission
order, because the probe's clock advances per batch. The Cut 9 test pins
`[cut-12.r2, cut-9.r2]`, the id order that results when every batch shares one
`now()`. **Tests that pin id order under a single clock change their
expectation once the order becomes the ordinal.** Hands expects this; it is
not a regression.

### F2. Where receipt ordinals come from

There is no ordinal today.
- `candidate` (`receipt.rs:160-178`) is the one place a receipt is constructed.
  `commit` (`:211-245`) makes one compare-and-swap. Its expectation is the
  strong reads, and its insert-only replacements are the writes plus the
  receipt.
- **The swap does not guard an ordinal.** Receipts are insert-only under their
  digest key, so two receipts claiming one ordinal would both insert (the store
  semantics are at `store.rs:135-159`). Uniqueness therefore rests on the single
  writer: `admit` takes `&mut self`, and the owned store holds a lifetime lock
  (ruling 15). F7 of the Cut 10 pass already recorded that the store is not
  sealed.
- The map's answer is to derive the ordinal as `head + 1` and to **validate
  density wherever it is read**. It does not add an insert-only ordinal row,
  which would be a new persistent kind that guards only against this organ's own
  code. Sealing the store is the recorded follow-up that closes the gap
  structurally.

**Probe, run: the order cannot be recovered from the clock.** A real store,
written by Cut 10's Soul probe (copied from
`…F--Projects-Aetheria…/cut10-probe-run/state-1789593994318/minds/yggdrasil/mind.redb`),
decodes to 31 receipts with **5 distinct `committed_at` values**. A store
therefore cannot be migrated by deriving ordinals from time. That confirms the
ruling to refuse rather than migrate.

### F3. The payload shapes, decoded from the real store

**The receipt** is a **positional 7-element MessagePack array** (`0x97`). That
holds even though it is written through `prepare_entry_named`
(`cultcache-rs@a0813c6 lib.rs:2181-2197`). The positions are the
`#[cultcache(key = N)]` slots.

A v1 receipt decoded under a v2 shape that adds `ordinal` at key 7 gives:
- **strict:** `Err("invalid length 7, expected struct … with 8 elements")`,
  which surfaces as `Unavailable` at read time, deep inside every read;
- **`#[serde(default)]`:** `Ok(0)`. That is a **silent misread**: every old
  receipt gets ordinal 0.

**So the receipt's type id must move.** Adding the field alone is either a late
refusal or a silent lie.

**Leaf documents** are `[value]` (`0x91`), where `value` is a named map
(`schemas/cultnet/README.md:36-38`). A required `title` is therefore a
breaking change to the leaf. Without a default, an old payload fails with
"missing field", which follows from serde and was not probed. With a default,
the change is additive and keeps the epoch, per the leaf's own rule (`README.md:40-46`).

### F4. What the store version bump touches

**Leaf (`epiphany-pipeline` at `b3ae787b`):**
- `PIPELINE_SCHEMA_EPOCH` `lib.rs:720` changes `v1 → v2`. Its doc at `:713-719`
  stays true.
- **All thirteen** type ids in the `pipeline_kinds!` invocation (`:596-630`)
  move, because `every_kind_is_at_the_epochs_version` (`:2381-2400`) pins every
  kind at the epoch's version. Two kinds change shape, and thirteen ids move.
- Thirteen schema files in `schemas/cultnet/` are renamed `v1 → v2`, and
  `question` and `ruling` gain `title`.
- `schemas/cultnet/index.json` (`schemaId`, `schemaVersion`, `documentType`,
  `path` ×13) is updated.
- `schemas/cultnet/README.md:41` (the epoch named in prose) is updated.

**Huginn:**
- `receipt.rs:27` (`RECEIPT_SCHEMA_VERSION`), `:95` (the type id) and `:348`
  (a test literal) change `v1 → v2`. `HuginnCommitReceipt` gains
  `#[cultcache(key = 7)] pub ordinal: u64`.
- The daemon test literals `"epiphany.pipeline.epoch.v1"` at `daemon.rs:380`
  and `envelope.rs:183` change, and so does the `"epiphany.pipeline.instance.v1/…"`
  detail at `daemon.rs:645`.
- Both published wire schemas are regenerated, because they embed
  `PipelineDocument` (the new titles) and `AdmissionFacts` (the new ordinal).
- `HuginnMindEpoch` (`huginn.mind_epoch.v1`) is **unchanged**. Its shape does
  not move, and it is keyed by whatever the epoch string is.

**Untouched, checked:**
- `epiphany-core/src/runtime_spine.rs:8761` uses
  `"epiphany.pipeline.campaign.v1"` as a stand-in for a *foreign* type id. Its
  rule is "an id the spine never registered", which any string satisfies. It
  does not depend on the leaf; that is stated at `:8754-8757`.

### F5. How an existing store refuses to open (probe, run)

The `storev2/` probe copies the leaf and applies the bump:
- the epoch becomes v2 and all thirteen type ids become `.v2`;
- `question` and `ruling` gain `title`.

It also copies `huginn-mind`, with the receipt at v2 carrying `ordinal`, and
opens a copy of the real v1 store:

```
gate order today (types, then epoch): Some(ForeignStore { type: "epiphany.pipeline.campaign.v1" })
gate order epoch first:              Some(ForeignEpoch { found: "epiphany.pipeline.epoch.v1", expected: "epiphany.pipeline.epoch.v2" })
v2 opener refuses: ForeignStore { type: "epiphany.pipeline.campaign.v1" }
```

**Today's opener refuses the old store, but by the wrong gate.** A real epoch
bump always moves the type ids, by the leaf's rule. The type gate
(`mind.rs:137`, `:220-225`) therefore always fires before the epoch gate
(`:138`, `:235-257`). **The epoch gate is unreachable for the one case it
exists for.** Its doc at `mind.rs:21-24` promises "a breaking schema bump
refuses the old store".

The fix swaps the two calls at `:137-138`. Checked against the opener test
(`mind.rs:345-413`): **none of its eight cases changes its expected refusal**
under the swap.
- The "runtime store passed by mistake" case carries a current epoch record, so
  it still reaches the type gate.
- A foreign store with **no** epoch record becomes `MissingIdentity` instead of
  `ForeignStore`. No test pins that case. It is honest: such a store has no
  mind identity at all.

### F6. `select`'s snapshot contract, run

On a probe row set:
- A cursor minted at `asOf 21` answers page 2 at `asOf 21`.
- The same cursor answered with `asOf 22` gives
  `CursorStale { as_of: 21, current: 22 }` (`selection.rs:1131-1136`).
- **So "Huginn never refuses `cursor_stale`" (selection map §2 "Snapshot") is
  the organ's job:** it must answer as of the cursor's `asOf`, and pass that
  `asOf`. It learns `asOf` through `Cursor::parse` (public, `:1326-1346`)
  **before** calling `select`, so that it can restrict the rows. This is a
  dependency on near-final code (§10).

### F7. The substrate door checks names, never values (S6 lives in the organ), run

`cultnet_rs::validate` (`selection.rs:756-862`) refuses these:
- empty lists and blank entries;
- a bad `op`;
- `values` and `number` present in the wrong combination;
- an undeclared `index` on the reachable schemas;
- a comparison on a non-numeric alias;
- undeclared roles.

**It never looks at a value.** Probe:
- a `root` value `"eureka state/../x"` validates, and answers `[]`;
- a `cites.target` of `{schemaId: ruling, recordKey: <a question's key>}`
  validates, and answers `[]`.

An unknown `schemas` id is not refused either. `reachable_schemas` (`:864-873`)
filters it away silently. This is the S6 class exactly: a malformed filter reads
as an empty answer. Selection map D1 already assigns value refusal to "the row
owner … at the door". **The organ's value door is RS-3's.**

### F8. `Evaluation` carries no `matched` (source read)

`Evaluation { rows, edges, next_cursor }` (`selection.rs:1080-1085`) has no
count. `select` computes the full matched list (`:1120-1152`) and discards its
length. The page's `matched` is on the wire (`contracts.rs@00f4c02:365-368`),
but a Rust consumer cannot fill it without re-running `select`, and even then
only up to the 200 cap.

This is a gap in the owner: **P-1**. No Rust server answers v1 today
(`snapshot_query.rs` serves v0), so Huginn is the first caller to feel it.

### F9. Publishing Huginn's request schema with a substrate type in it (probe, run)

`impl JsonSchema for cultnet_rs::Selection` inside a consumer crate fails with
**`E0117`**, the orphan rule. So §12's "hand-maintained `JsonSchema` impl for
the `cultnet-rs` types inside Huginn" **cannot be written**. The same applies
to BP-3's plan for the manifest.

What works, and was run, is
`#[schemars(schema_with = "selection_ref")]` on Huginn's own field, returning
`{"$ref": "https://github.com/GameCult/CultLib/contracts/cultnet/cultnet.selection.schema.json"}`,
which is the `$id` published at `00f4c02`. This gives one owner for the
selection's shape and no mirror type in Huginn.

`Edge` has no `$defs` entry in CultLib's response schema; it is inline under
`properties.edges`. Huginn therefore types its edges in its own vocabulary
instead (RS-3).

### F10. `Selection` does not derive `Eq` (probe, run)

`#[derive(PartialEq, Eq)]` on a struct carrying `cultnet_rs::Selection` fails
with `E0277`. `Selection` and `FieldPredicate` derive `PartialEq` only
(`selection.rs:400`, `:435`). `HuginnMindRequest` derives `Eq` (`wire.rs:42`).
This is a gap in the owner: **P-2**. Every field is a string, bool, integer or
vector, so `Eq` is derivable.

### F11. `huginn-mind` has no CultNet dependency, and a check says so

`huginn-mind/Cargo.toml` depends on `cultcache-rs` and the leaf, and says "No
network, no socket, no index". The Cut 8 verification
(`eureka-pipeline-state-cut.md:3701`) requires this grep to come back empty:

`rg -n "reqwest|UdpSocket|qdrant|ollama|cultnet|cultmesh|IndexPort|EmbeddingPort|pending_index" crates/huginn-mind`

**Carrying `Selection` in `wire.rs`, which is where Q13 A put the wire types,
breaks that check.**

`cultnet-rs` brings `socket2`, `aes-gcm`, `ed25519-dalek`, `rand`, `uuid` and
`windows-sys`. It adds **zero** packages to the workspace build, because
`huginn-daemon` already pulls all of them. `selection.rs` itself uses only
`serde`, `serde_bytes`, `base64` and `sha2`. This is Q-RS2.

### F12. Record keys are unique across kinds

- The leaf derives every key as `<root>:<kind>:<local>`
  (`query.rs:165-174` reads it that way).
- The probe image had 36 distinct keys over 36 views.

This matters because `select` indexes rows by record key alone
(`selection.rs:1095`). Huginn is safe under that indexing whichever way the
selection Soul pass settles it.

## 2. CultLib prerequisites (routed to the selection campaign, not this map's Hands)

These fill gaps in their owner. Self adds them to the selection branch's next
fix batch, so that they land **before the merge** that BP-2 pins to.

- **P-1. `Evaluation.matched: u32`**, the length of the ordered list before
  cursor and limit.
  - Rule to pin: `matched` is the count before paging, unchanged across a
    walk's pages.
  - Revert: drop it. Loosening: `matched = page.len()` must die on a fixture
    with 3 matches and `limit: 1`.
  - The C# evaluator's page already carries it. The parity vectors should
    compare `matched`; check whether they already do.
- **P-2. `Eq` on `Selection`, `FieldPredicate`, `Citation`, `Incoming` and
  `Edge`.**
  - `RecordRef` already has it.
  - No test is needed beyond a compile use. Fallback, if the selection campaign
    refuses: Huginn drops `Eq` from `HuginnMindRequest`. That costs nothing
    today, because its tests use `assert_eq!`, which needs only `PartialEq`.

## 3. Cut order

```
[Cut 10 residue batch lands on Huginn]                       (in Hands now)
      │
      ├──► RS-2  Huginn: delete open_items, history, the admission window       (no pin move)
      │      │
      │      ▼
      │    RS-1  Huginn: the receipt ordinal, receipt v2, epoch-first opener     (no pin move)
      │
      ├──► RS-L  Epiphany leaf: titles, epoch v2, OrgRepo and Label doors         (parallel: another repo)
      │
[P-1, P-2 → selection Soul → selection merges to CultLib main]
      ▼
BP-1  CultLib content plane                                  (body-plane map; Q-BP1)
      ▼
BP-2  one pin move: leaf cultcache → X; Huginn cultcache, cultnet → X; Huginn leaf → RS-L tip
      │  (widened here: the mechanical adoption of RS-L's titles and epoch in Huginn's fixtures and literals)
      ▼
RS-3  Huginn: the consumer (Row/RowSet, value door, snapshot, page, summary, edges)
      ▼
BP-3  Huginn: deferral to the body plane                     (body-plane map; Q-BP2)
      ▼
Cut 11, Cut 13
```

**Why RS-2 and RS-1 run early.** Neither needs a new pin. Both are plain Huginn
work that can land while CultLib finishes selection. RS-2 comes first because
it is pure subtraction, and it leaves RS-1 and RS-3 fewer read paths to carry.

**What the gap costs.** Between RS-2 and RS-3 the organ answers `view`,
`query`, `whoami` and `admit`. The two relations that RS-2 unpins (R-B, D6) are
re-pinned as selections in RS-3. No client exists, because Cut 13 is unbuilt.

**RS-L can land at any time**, but **before BP-2**, so that Huginn moves its
leaf pin once (§4).

## 4. The pin sequence

- **CultLib revision X** is the first `main` commit that carries both of these:
  - the selection merge, including P-1 and P-2;
  - BP-1.
- **The leaf** pins `cultcache-rs` at X. BP-2 makes this the leaf's commit,
  **after** RS-L, so the leaf's tip carries both changes.
- **Huginn**, in one commit (BP-2's Huginn half):
  - `huginn-mind` pins `cultcache-rs` at X;
  - `huginn-daemon` pins `cultnet-rs` at X;
  - `huginn-mind` pins the leaf at the leaf's tip.

  `huginn-mind` gains `cultnet-rs` at **X** in RS-3. That adds an edge at the
  same revision, not a pin move.
- **Result:** every CultLib pin moves once, and the leaf pin moves once.
  `cargo tree -d` shows one `cultcache-rs` and one `cultnet-rs`. The probe build
  shows two `cultcache-rs` when the revisions differ, confirming the body-plane
  map's F5.

**BP-2 is widened by one clause.** Adopting RS-L changes the leaf's API: two
required fields and a new epoch. BP-2's Huginn commit is therefore **not**
revision strings only. It also:
- adds `title` in `fixtures.rs:162-187` and in its own probe;
- updates the three version literals of F4;
- regenerates the two published wire schemas.

It changes no behaviour. It is verified by:
- the full suite green;
- the cut8, cut9 and cut10 mutation suites rerun clean;
- `cargo tree -p huginn-daemon -e normal -d` clean.

The body-plane map's "74 tests at `202e5e3`" is stale. Re-count after the
residue lands.

**Conditional default: this depends on Q-BP1, and nothing here re-asks it.**
- Under **Q-BP1 A**, BP-1 is small and starts now, so the pins wait for it and
  move once.
- Under **B or C**, BP-1 becomes a TLS or QUIC campaign. Waiting would stall
  RS-3 on an unrelated transport. In that case BP-2 splits:
  - it moves to the selection merge now, and RS-3 proceeds;
  - a second CultLib-only pin move follows BP-1. The leaf pin does not move
    again unless the leaf's `cultcache-rs` changes.

  Self decides at that point. It is not an operator question: a pin move costs
  four strings and one `cargo tree` check.

## 5. The cuts

### Cut RS-L. The leaf: titles, the epoch, and two grammar doors

- **Repo/branch:** Epiphany `codex/eureka-pipeline-state` from `b3ae787b`.
  Depends on Q-RS1 (the type of `title`). No pin moves.
- **Deletes first:** none. The v1 schema files are **renamed**, not kept
  beside v2, so there is no second copy.
- **Changes, by anchor (`lib.rs` at `b3ae787b`):**
  - `:349-352` `PipelineQuestion` and `:353-356` `PipelineRuling` each gain
    `title: <T>`, where `<T>` depends on Q-RS1. Under Q-RS1 B, `:344` campaign
    and `:358` cut spec move from `Short` to the same `<T>`.
  - `:720` `PIPELINE_SCHEMA_EPOCH` becomes `"epiphany.pipeline.epoch.v2"`.
  - The thirteen type ids in `:596-630` go to `.v2`. The test at `:2381`
    forces it.
  - Two public doors are added beside `Slug::validate_slug` (`:214-227`), on its
    pattern:
    - `OrgRepo::validate_org_repo(&self)` delegates to `org_repo_text`
      (`:157-166`), through `Bounded::validate(self, "org_repo")`;
    - `Label::validate_label(&self)` delegates to `label_text` (`:134-142`).

    Each delegates, so the grammar keeps one owner.
  - `schemas/cultnet/`: rename the thirteen files, regenerate them, update
    `index.json`, and update `README.md:41`.
- **Authority:** the leaf owns every grammar. The organ calls doors and
  re-derives nothing. That is the precedent set by `validate_ref` and
  `validate_slug`.
- **Verification:**
  - builds: `cargo test -p epiphany-pipeline --lib`, with the new target
    subdirectory given in §7.
  - tests:
    - `pipeline_published_schemas_match_derivation` pins the renamed and
      regenerated files byte for byte.
    - `every_kind_is_at_the_epochs_version` pins v2 throughout.
    - A new test, `the_org_repo_and_label_doors_are_the_grammar`, runs these
      inputs:

      | Door | Refuses | Accepts |
      |---|---|---|
      | `OrgRepo` | `"GameCult"`, `"/Repo"`, `"GameCult/"`, `"a/b/c"`, 201 bytes | `"GameCult/Epiphany"` |
      | `Label` | `""`, `"a.b"`, 65 bytes, `"é"` | `"cut-10"` |
    - A new test, `question_and_ruling_carry_a_title`, round-trips each kind
      through `prepare`/`decode`. Under Q-RS1 A or B it also asserts that an
      empty title refuses.
  - mutations (`tools/eureka-readside-leaf-mutations.psd1`, M0 control):

    | Entry | Kind | Mutation |
    |---|---|---|
    | `L1` | revert | `validate_org_repo` returns `Ok(())` |
    | `L1L` | loosening, a function of the input | `org_repo_text` accepts when `contains('/')` |
    | `L2` | revert | `validate_label` returns `Ok(())` |
    | `L2L` | loosening, the wrong grammar | `validate_label` delegates to `dotted_text`; the fixture `"a.b"` is a valid slug and an invalid label |
    | `L3` | revert, Q-RS1 A/B only | the title's non-empty check is removed |
  - negative: `git grep -n 'epiphany\.pipeline\.[a-z_]*\.v1' -- epiphany-pipeline schemas`
    is empty. Checked for collisions: `notes/` and the `runtime_spine`
    stand-in are outside that scope, deliberately.
- **Subtraction estimate:** about +45 source lines (4 fields, 2 doors with docs)
  and about +60 test lines. The schema files churn by rename, plus about 2 fields
  each on two files. No dependency changes.

### Cut RS-2. Huginn: delete `open_items`, `history`, and the admission window

- **Repo/branch:** Huginn `eureka/memory-organ`, after the residue batch lands.
  No pin move. Subtraction only, so that Soul can falsify it on its own.
- **First:** re-take every anchor below against the post-residue HEAD.
- **Deletes (anchors at `4f7e3b5`):**
  - **`query.rs`:**

    | Lines | What is deleted | Size |
    |---|---|---|
    | `:107-117` | `PipelineOpenItems` | 11 |
    | `:119-125` | `HistoryScope` | 7 |
    | `:352-391` | `open_items` | 40 |
    | `:393-435` | `history` | 43 |
    | `:89-92` | the fields `admitted_after`/`admitted_before` | |
    | `:279-288` | their filters | |
    | `:570-663` | tests: both history tests | about 93 |
    | `:942-1003` | tests: the open-items test | about 61 |
    | in `:765-886` | tests: the window lines of `query_filters_each_select_by_one_field` | |
  - **`wire.rs`:**
    - `:48-49` (the variants), `:60-61` and `:72-73` (the arms) and `:87-88`
      (the response variants);
    - the test fixtures at `:155-159` and `:172-173`;
    - the operation names asserted at `:184`.
  - **`lib.rs:41`:** the re-exports of `HistoryScope` and `PipelineOpenItems`.
  - **`daemon.rs`:**
    - `:92-99`, the two arms;
    - the tests that exercise them only:
      - `a_refusal_open_items_raised_is_the_answer_the_dispatch_returns_whole` (`:624`);
      - `open_items_refuses_on_a_fault_inside_the_requested_campaign` (`:660`);
      - the `OpenItems`/`History` lines in the instance-gate tests at
        `:401-416`, `:455-459`, `:518-522` and `:581`.

      Each instance-gate test keeps its `query` and `view` cases.
  - **`serve.rs`:**
    - `:591` (`"m-o"`);
    - `:855` (`"open_items"` as the long operation name).

    N4 needs two operation names of different lengths. **Replace `open_items`
    with `whoami`, six bytes against `view`'s four.** Do not delete the case:
    the rule that the gate is exercised across operations survives.
  - **`README.md:51`:** the clause "a campaign's open work, and a subject's or a
    repo's history".
  - **Entries:**
    - cut9: V9, V10, V11, V12, V13, V13L, V14, V14L, V15, V22 and V22L (11);
    - cut10: D19, D19L, D23, D23L, S1 and S1d (6).

    D13 and D13L anchor on the deleted `operation()` arms: re-anchor them on the
    surviving arms (`"view"`/`"query"`), with the same rule.

    Each deleted entry's rule is either **ruled away** (the window, R-3) or
    **re-pinned in RS-3** under the selection spelling. That second group is
    R-B, D6, V9-V15, D19 and D23: RS-3 re-authors them by name.
- **Keeps:**
  - `docs.rs` `resolutions_of` (`:97-99`) and `assignments_of` (`:104-106`).
    Admission's sequence rules still read both, through `latest_*`,
    `derived_*` and `stewardships_of`.
  - `semantic` and its refusal, as-is: Cut 11 owns "the `semantic` branch of
    `query`" (Epiphany map `:5321`).
- **Authority:** no ownership moves. Two public operations stop existing.
- **Verification:**
  - builds: `cargo test -p huginn-mind -p huginn-daemon`.
  - negative, all over `crates` and `README.md`: each of the following
    patterns is empty, and each has been checked for collisions:

    | Pattern | Checked against |
    |---|---|
    | `open_items`, `OpenItems`, `PipelineOpenItems` | — |
    | `HistoryScope` | — |
    | `fn history\b` | prose uses "history" in comments |
    | `admitted_after`, `admitted_before` | — |
  - mutations: cut9 (27 remaining) and cut10 (65 remaining) rerun, with every
    M0 green.
- **Subtraction estimate:** about −110 source lines and about −330 test lines.
  17 entries are removed and 2 re-anchored. No dependency or format changes.

### Cut RS-1. Huginn: the receipt ordinal, receipt v2, and the epoch gate first

- **Repo/branch:** Huginn, after RS-2. No pin move. The leaf is still at
  epoch v1 here. Its epoch arrives with BP-2, before any mind is deployed:
  Cut 14 is unbuilt. The one store version any deployed mind ever carries is
  therefore (leaf epoch v2, receipt v2), which is the ruling's single migration.
- **Deletes first:** none.
- **Changes (anchors at `4f7e3b5`):**
  - **`receipt.rs`:**
    - `:27` and `:95` change `v1 → v2`. `HuginnCommitReceipt` (`:94-111`)
      gains `#[cultcache(key = 7)] pub ordinal: u64`.
    - `candidate` (`:160-178`) takes `ordinal: u64`.
    - `digest` (`:116-120`) is **unchanged**: the ordinal is not digested. The
      header doc (`:10-14`) names the ordinal beside `committed_at` and
      `provenance`.
    - `validate` (`:122-142`) refuses `ordinal == 0`.
    - New: `pub(crate) fn head(mind) -> Result<u64, MindRefusal>` decodes the
      receipts and requires their ordinals to be exactly `{1..=N}`. Anything
      else is `Unavailable { detail: "receipt ordinals are not 1..=N: …" }`.
      Admission calls it for `head + 1`. Nothing else computes an ordinal.
  - **`admission.rs`:** the one `candidate(…)` call passes `head(self)? + 1`.
  - **`query.rs`:** `AdmissionFacts` (`:36-41`) gains `ordinal: u64`, and
    `AdmissionIndex::build` (`:134-150`) fills it. **The order is not switched
    here.** `ordered` (`:307-311`) is deleted in RS-3, not rewritten twice.
  - **`mind.rs:137-138`:** swap to `refuse_foreign_epoch`, then
    `refuse_foreign_types`. The doc at `:129-133` is updated to match.
  - **`wire.rs`:** regenerate the response schema, because `AdmissionFacts`
    changes.
- **Authority map:**
  - **Owner:** `receipt.rs` owns the ordinal, meaning both its assignment and
    its validity.
  - **Inputs:** the image's receipts.
  - **Outputs:** `head`; and `ordinal` on each receipt and on `AdmissionFacts`.
  - **Derived state:** `AdmissionFacts.ordinal`, which is display-only until
    RS-3 makes it the order.
  - **Forbidden writers:** no code outside `receipt.rs` counts receipts to
    produce an ordinal. `MindStatus.receipts` (`wire.rs:106-114`) counts them
    for display and is not an ordinal.
  - **Shared paths:** `admit` and `admit_prepared`, and so the daemon's sink
    and Cut 12's import, all end in the one `candidate` call.
  - **Deletion line:** none. The ordinal is new.
- **Verification (tests; each rule has a revert and a loosening that fail):**

  | Test | Rule it pins | Mutants that must die |
  |---|---|---|
  | `ordinals_are_admission_order_not_clock_or_id_order` | ordinal is `head + 1` at admission. The fixture has **three batches admitted at the same `now`**, whose receipt ids sort opposite to admission order; the probe's real store shows 31 receipts in 5 seconds, so this is the live case | revert `ordinal: 1` for every receipt (dies on the density check); **function-of-input loosening:** the ordinal is the rank of `(committed_at, receipt_id)` among receipts, which the same-second fixture kills; off-by-one `head` |
  | `an_exact_replay_keeps_its_ordinal` | the ordinal is not digested (A9) | the ordinal inside the digest makes the replay commit again or refuse, and must die |
  | `a_chain_that_is_not_dense_refuses_admission_and_reads_alike` | density is validated wherever the ordinal is read. The fixture plants a duplicate and a gap through `MemoryStore` | `head` returns `len()` without checking; the check runs only in admission |
  | `a_v1_receipt_store_is_refused_at_open` | receipt v2 is a new type. The fixture plants a v1-shaped 7-slot receipt under `huginn.mind_commit_receipt.v1` | adding the field with `#[serde(default)]` and keeping the v1 id; F3 shows that this decodes to 0 silently |
  | `a_store_written_at_the_previous_epoch_is_refused_by_the_epoch_gate` | epoch first. The fixture has an epoch record at `epoch.v0` and type ids at `.v0` | swapping the gates back makes it `ForeignStore`, which must die. The existing eight cases in `mind.rs:345-413` stay green unchanged, and that is part of the check |

  - Entries: `tools/eureka-readside-mutations.psd1`, R1-R5 with L variants
    and an M0.
  - Cut8's A9 replay entries rerun, because `candidate`'s signature changed.
- **Subtraction estimate:** about +45 source lines and +160 test lines. No
  dependency changes. **Formats:** one moves, receipt v1 → v2.

### Cut RS-3. Huginn: the consumer

- **Repo/branch:** Huginn, after BP-2 (pins at X, leaf at RS-L's tip) and after
  RS-1. Depends on Q-RS2, and on P-1 and P-2 being in X.
- **Commits, in order.** Each builds and passes on its own.
  1. **The edge list moves to its owner (ownership only).** It moves from
     `admission.rs:175-178` (`kind_of_id`) and `:223-285`
     (`outcome_references`, `references`) into
     `docs.rs::citations(document) -> Vec<(CitationRole, PipelineRef)>` and
     `CitationRole`: the fifteen roles of the earlier read-side spec's D3 (list
     below).
     - Admission's `references` becomes a map over `citations`, carrying the
       `Missing` classification (`:203-207`) and the hand-off source-side
       filter. It has the same refusals in the same order.
     - Cut8's `H17` entry (`refs.extend(outcome_references(…))`) is
       re-anchored into `docs.rs`, with the rule unchanged.
     - Verified by: the cut8 suite 67 of 67 (plus H17 re-anchored) and the
       full admission test set, unchanged.
  2. **Deletes first:**
     - `PipelineQuery` (`query.rs:77-97`) and `PipelineQueryPage` (`:101-105`);
     - `QUERY_LIMIT_MAX` (`:32`), which is replaced by
       `cultnet_rs::LIMIT_MAX`, the substrate's clamp;
     - `Reader::matches` (`:249-290`) and `ordered` (`:307-311`);
     - `root_and_local`'s role as a filter. It survives as the row's value
       source for `root` and `cut`;
     - `repo_matches` (`:178-195`), which moves to the row's `repo` values;
     - `lib.rs:40-43`, the re-exports of all of the above.
  3. **The consumer:** `rows.rs` (new), `query.rs` rewritten around it,
     `wire.rs`, `refusal.rs`, and `daemon.rs:88-91`. Details below.
  4. **Tests and the readside entries.**
- **New dependency:** `huginn-mind` gains
  `cultnet-rs = { git = …, rev = X, package = "cultnet-rs" }` (Q-RS2 A).
- **Vocabulary: the organ's closed lists.**
  - **Schemas.** These are the thirteen leaf type ids. `schemas` entries are
    matched exactly: Rust carries no alias matching (`selection.rs:438-440`).
  - **Aliases.** Each row answers `values(alias)` with strings. Every value is
    checked at the organ's door against the domain in the last column.

    | Alias | Declared on | Value | Door (domain) |
    |---|---|---|---|
    | `root` | all | the key's root segment. This is V18/V18L's rule, renamed from `campaign` because a stewardship's root is an instance and a resolution's is its subject's | `Slug::validate_slug` |
    | `in_force` | all | `"true"`/`"false"` | exactly those two |
    | `faculty` | all | `Faculty` variant name | the closed set |
    | `repo` | campaign (each listed repo), cut_spec, cut_report, follow_up, stewardship, hand_off, resolution (read through `base`: V17/V17L) | `OrgRepo` text | `OrgRepo::validate_org_repo` (RS-L) |
    | `cut` | cut_spec, cut_report, verdict, finding, resolution (through `base`) | the label in `cut-<label>.` (V16/V16L: the prefix ends at the dot) | `Label::validate_label` (RS-L) |
    | `severity`, `confidence`, `origin` | finding | variant name | closed set |
    | `authority` | ruling | variant name | closed set |
    | `claim_outcome` | verdict (one value per claim) | variant name | closed set |
    | `outcome` | resolution | `Superseded`, `Answered`, `Fixed`, `Deferred`, `Recorded` or `Withdrawn` | closed set |

    No alias is numeric. `is_numeric` is `false` everywhere, so the substrate
    refuses any comparison.
  - **Roles** (`CitationRole`, one per referring leaf field, with the wire
    spelling equal to the field name), and `target_leaves`:

    | Role | Source field | Target leaves |
    |---|---|---|
    | `raised_in` | `question.raised_in` | all 13 |
    | `answers` | `ruling.answers` | question |
    | `rulings` | `cut_spec.rulings` | ruling |
    | `questions` | `cut_spec.questions` | question |
    | `cut_spec` | `cut_report.cut_spec` | cut_spec |
    | `forks` | `cut_report.forks` | question |
    | `cut_report` | `verdict.cut_report` | cut_report |
    | `findings` | `verdict.claims[].findings` | finding |
    | `verdict` | `finding.verdict` | verdict |
    | `source` | `follow_up.source` | all 13 |
    | `subject` | `resolution.subject` | all 13 |
    | `superseded_by` | `Superseded.by` | all 13 |
    | `resolved_by` | `Answered.by` and `Fixed.by` | all 13 |
    | `deferred_to` | `Deferred.to` | all 13 |
    | `documents` | `hand_off.documents`, on both sides; an entry whose kind segment does not read cites nothing | all 13 |

    **"Any" is spelled as the thirteen ids, never as an empty list.** The
    substrate reads empty as "unrestricted" (`selection.rs:1204`), and Huginn
    should not depend on that reading (§10). Role names are unique, so
    `target_leaves(role)` needs no schema.
  - **Not edges:**
    - `cut_spec.depends_on`, `finding.invariants` and `claim.promise`, which
      are labels;
    - `precedents`, which is `ForeignRef`, another store's;
    - `Fixed.commit`.
- **`rows.rs` (new): types, rules and boundaries only.**
  - `pub(crate) struct SelectionRow` holds:
    - `kind`, `key`, `ordinal: u64`;
    - `values: BTreeMap<&'static str, Vec<String>>`;
    - `refs: Vec<(CitationRole, PipelineRef)>`;
    - the `PipelineDocumentView` it projects.
  - `impl cultnet_rs::Row for SelectionRow`:
    - `ordinal as i64`;
    - `references()` maps each ref to
      `(role.name(), RecordRef{kind.type_id(), id}, None)`. There are no
      payloads.
  - `pub(crate) struct Vocabulary` implements `cultnet_rs::RowSet` from the two
    tables above, and nothing else.
  - `fn refuse_values(selection) -> Result<(), MindRefusal>`, **the organ's
    door (S6)**, runs after `cultnet_rs::validate`:
    - every `schemas` entry is one of the thirteen ids;
    - every `keys` entry parses as a `PipelineRef` whose kind is its own kind
      segment, and passes `validate_ref`;
    - every `any_of` value is in its alias's domain;
    - `cites.target.schemaId` is one of the thirteen ids, and
      `PipelineRef{that kind, recordKey}.validate_ref()` passes. That is
      V24's rule, now at this door.

    Refusals name what the client sent: `SelectionInvalid { field:
    "fields[1].values" | "keys" | "schemas" | "cites.target", value, message }`.
    This carries Cut 10 N2's lesson.
- **`query.rs` rewritten: types and rules.**
  - `Reader::at(mind, as_of)`:
    - `AdmissionIndex` is built over **all** receipts, so an orphan or a double
      write still refuses as before (V4/V4L).
    - The image is filtered to documents whose writing receipt has
      `ordinal ≤ as_of`.
    - `Docs` is built over that filtered image, so in force, the closing
      resolution, `base` and citations are all as of `as_of`.
    - `Reader::new(mind)` becomes `Reader::at(mind, head)`. `view` keeps using
      it.
  - `Mind::query(&self, selection: &Selection, semantic: Option<&SemanticQuery>) -> Result<PipelineSelectionPage, MindRefusal>`
    does, in order:
    1. refuses `semantic`, before any read (V8/V8L unchanged);
    2. runs `cultnet_rs::validate(selection, &Vocabulary)`;
    3. runs `refuse_values`;
    4. takes `as_of` from `Cursor::parse(cursor)?.as_of` if a cursor is
       present, and otherwise `head`; an `as_of > head` is `CursorInvalid`;
    5. builds `Reader::at` and the rows;
    6. calls `cultnet_rs::select(&Vocabulary, &rows, selection, as_of)`;
    7. projects the page.
  - `PipelineSelectionPage` carries:
    - `matched: u32`, from P-1;
    - `as_of: u64`;
    - `next: Option<String>`;
    - `items: PipelinePageItems`;
    - `edges: Option<Vec<PipelineEdge>>`, present exactly when the selection
      has a hop.
  - `PipelinePageItems` is `enum { Headers(Vec<PipelineDocumentSummary>), Documents(Vec<PipelineDocumentView>) }`,
    chosen by `selection.projection` (`PROJECTION_HEADER`/`PROJECTION_DOCUMENT`).
  - `PipelineEdge` is `{ from: PipelineRef, role: CitationRole, to: PipelineRef }`,
    which is the organ's typing of `EdgeMatch` (F9).
  - `PipelineDocumentSummary` follows the earlier read-side spec's D1, with
    these changes:
    - it carries `id`, `admission` (now with `ordinal`), a status summary
      (`InForce | Resolved { resolution, outcome }`) and `facts: PipelineFacts`
      per kind;
    - **`Question` and `Ruling` facts gain `title`**;
    - it is bounded by `SUMMARY_MAX_BYTES = 8_192`.

    The earlier spec lives at
    `…F--Projects-Aetheria…/scratchpad/cut10b-read-side-spec.md:120-205`, a
    scratchpad and **not durable**. Self copies D1's type block into this
    section when placing the map, or the spec dies with the session.
- **`wire.rs`:**
  - `Query { instance: Slug, #[schemars(schema_with = "selection_ref")] selection: Selection, semantic: Option<SemanticQuery> }`.
    `selection_ref` returns the `$ref` of F9.
  - `HuginnMindResponse::Query(PipelineSelectionPage)`.
  - Regenerate both schema files.
  - Keep `Eq` (P-2) or drop it (the fallback).
- **`refusal.rs`:**
  - adds `SelectionInvalid { field: String, value: Option<String>, message: String }`
    and `CursorInvalid { message: String }`;
  - adds one total mapping from `SelectionRefusal`, **with no wildcard arm**:
    - `Invalid` becomes `SelectionInvalid`;
    - `CursorInvalid` becomes `CursorInvalid`;
    - `CursorStale` becomes `Unavailable` ("the organ answers at the cursor's
      asOf; a stale refusal is an organ defect"). A test proves it
      unreachable;
    - `ReferenceOutsideTarget` becomes `Unavailable`, as integrity: A7 admits
      only referents of the declared kind.
- **`daemon.rs:88-91`:** the arm passes `selection` and `semantic` through, and
  the refusal crosses unchanged. The existing D entries for this arm are
  re-anchored.
- **Authority map:**
  - **Owners:**
    - The substrate owns the grammar of a selection's names, the order, the
      cursor, the hop and paging.
    - The organ owns the rows, their ordinals and values, the vocabulary, the
      value domains, the snapshot, the header, and the mapping of refusals.
    - The leaf owns every value grammar.
  - **Inputs:** the image, the receipts, and `head` (from `receipt.rs`).
  - **Outputs:** `PipelineSelectionPage` and typed refusals.
  - **Derived state:**
    - `in_force`, `base` and the edges are derived per read, over the snapshot;
    - the summary is derived per read;
    - `matched` is the substrate's.
  - **Forbidden writers:**
    - no filter, order or paging logic in `huginn-mind`, beyond the as-of
      restriction;
    - no second copy of the selection's schema;
    - no refusal of a selection name that the substrate already refuses. If
      the substrate ever starts refusing unknown `schemas`, the organ's copy
      of that rule is deleted (§10);
    - `cultnet_rs::select`'s result is never re-filtered.
  - **Shared paths:**
    - `view` and `query` share `Reader::at`;
    - admission and reads share `citations` and `head`.
  - **Deletion line:** commit 2 above.
- **The relations, re-pinned by name** (R-B and D6 of Cut 9, and V9-V15):

  | Test | Relation |
  |---|---|
  | `a_subjects_history_is_one_cites_hop` | `schemas:[resolution], cites{target: S, role: subject}`, in ordinal order, with withdrawn records included with their status. The ten-cycle fixture of the deleted test is used, so n10 lands after n9 |
  | `a_repos_assignments_are_one_selection` | `schemas:[stewardship], root:[instance], repo:[R]` |
  | `open_work_is_five_selections` | the probe's five selections (F1), with the `query.rs:943-1003` fixture: the exact-id citation (V13L, V14L), and the superseded revision not open (V13) |

  Each is a positive answer **and** a fixture that fails when the relation is
  weakened. For example, `role` omitted from the subject history must return
  the `resolved_by` edges too, and the test must see that.
- **Verification (tests and mutations).** The file is
  `tools/eureka-readside-mutations.psd1`, with target
  `crates/huginn-mind/src/{rows.rs,query.rs,docs.rs,refusal.rs}`. The rows run
  in this order: snapshot, order, value door, edges and refusals, summary and
  wire.

  | Rule | Test | Revert | Loosening (at least one a function of the input where the rule is "derived from") |
  |---|---|---|---|
  | A page is exact as of `asOf`: rows **and** every derivation | `a_walk_reads_the_snapshot_its_cursor_names`. Mint at N; admit a batch that closes a page-2 item and adds a match; page 2 still shows the item in force, the new match is absent, and `matched` is unchanged | use `head` instead of the cursor's `asOf` (the substrate refuses stale, and the test must see `CursorStale` → `Unavailable`) | restrict rows but build `Docs` over the full image, so in force leaks; `ordinal < as_of` (the boundary batch is dropped: the fixture's last page-1 row lands at exactly `asOf`) |
  | `asOf > head` is refused | `a_cursor_from_the_future_is_invalid` | skip the check | `>=` in place of `>` (a cursor at exactly `head` must answer) |
  | The order is the ordinal | `the_page_order_is_admission_order` (same-second batches, ids opposite to admission) | `Row::ordinal` returns 0 | the ordinal from the rank of `admitted_at` |
  | `root` is the key root, in the Slug grammar (S6) | `a_root_outside_the_grammar_is_refused_not_empty` | skip | check only the first value (the fixture is `["eureka-state", "a/../b"]`) |
  | `repo` is an OrgRepo (S6) | `a_repo_outside_org_repo_is_refused` (`"GameCult/"`, `"a/b/c"`) | skip | `contains('/')` |
  | `cut` is a Label | `a_cut_outside_label_is_refused` (`"1.2"`) | skip | the Slug door in place of the Label door |
  | Enum domains are exact | `enum_values_are_the_variant_names` (`"high"` refused, `"High"` accepted) | skip | a case-folded comparison |
  | `schemas` and `keys` name things this organ holds | `an_unknown_schema_or_malformed_key_is_refused` | skip | accept any id that starts with `epiphany.pipeline.` |
  | `cites.target` passes the leaf's ref grammar (V24) | `a_cites_target_whose_kind_disagrees_is_refused` | skip | check the schema id only, not `validate_ref` |
  | V17/V17L: through base, to the first non-resolution | `repo_and_cut_reach_a_resolution_through_its_base` | values from the row itself | one step of `base` (the fixture is a withdrawal of a resolution) |
  | One edge list | `every_role_yields_its_edge` (a fixture per role, 15) | drop one role's arm | `resolved_by` from `Answered` only (a `Fixed { by: Some }` fixture) |
  | Refusals cross typed (D19/D23's successor) | `a_selection_refusal_crosses_the_dispatch_as_itself` | rewrap as `Unavailable` | swallow into an empty page |
  | The header is bounded | `a_summary_is_bounded_whatever_the_lists` (maximal cut spec) | drop the bound test's constant | measure the facts without `admission` |
  | Wire schemas equal derivation, and `selection` is CultLib's `$ref` | the existing `published_wire_schemas_match_derivation` | — | — |

  - builds:
    - `cargo test -p huginn-mind -p huginn-daemon`;
    - `cargo tree -p huginn-daemon -e normal -d`, which must show no
      `cultcache-rs` or `cultnet-rs` duplicate.
  - negative:
    - `rg -n "fn matches|fn ordered|QUERY_LIMIT_MAX|PipelineQuery\b|repo_matches" crates/huginn-mind/src`
      is empty;
    - `rg -n "impl .*JsonSchema for .*cultnet" crates` is empty;
    - under Q-RS2 A, Cut 8's grep is **replaced** with
      `rg -n "UdpSocket|CultNetRudp|bind\(|reqwest|qdrant|ollama|IndexPort|EmbeddingPort|pending_index" crates/huginn-mind/src`.
      Checked for collisions: `bind(` does not occur in `huginn-mind` today.
      Self amends the Cut 8 text the same day.
  - suites rerun: cut8, cut9 (whatever V entries remain: V1-V8, V16-V21, V23 and
    V24 are re-authored here and **deleted from cut9 in the same commit**, so no
    rule has two entries), and cut10.
  - operator: none. Cut 13's tool list decides whether any selection deserves a
    named convenience (the ruling puts that there).
- **Subtraction estimate:**
  - Source removes about 180 lines: `matches` 42, `ordered` 5, `PipelineQuery`
    with its page 28, `repo_matches` 18, and the admission reference code, which
    moves (about 65, net about +15).
  - Source adds about 520 lines:
    - `rows.rs`, about 230 (the vocabulary tables 60, the row build 70, the
      value door 80, the trait impls 20);
    - `query.rs`, about 170 (`Reader::at` 25, `query` 40, the page and items
      30, the summary with facts 75);
    - `docs.rs` `citations` and `CitationRole`, about 80;
    - `refusal.rs`, about 25;
    - `wire.rs`, about 15.
  - Net source is about +340.
  - Tests add about 650 lines. About 250 of the deleted test lines are re-pinned
    under the new spelling.
  - About 32 entries are added and about 15 cut9 entries deleted.
  - One dependency edge is added: `cultnet-rs` → `huginn-mind`. It adds no
    workspace package.

## 6. Subtraction ledger (estimate)

| Cut | Source removed | Source added | Tests (net) | Entries | Deps / formats |
|---|---:|---:|---:|---|---|
| P-1, P-2 (CultLib) | 0 | about 10 | about +30 | +2 | none |
| RS-L (leaf) | 0 | about 45 | about +60 | +5 | epoch v1→v2; 13 type ids; 13 schema files renamed |
| RS-2 | about 110 | 0 | about −330 | −17, 2 re-anchored | two operations removed from the wire |
| RS-1 | 0 | about 45 | about +160 | +10 | receipt v1→v2 |
| BP-2 widening | 0 | 0 | about ±10 (fixtures) | 0 | pins |
| RS-3 | about 180 | about 520 | about +400 | +32, −15 | +`cultnet-rs` edge on `huginn-mind` |
| **Total** | **about 290** | **about 620** | **about +320** | **about +17** | one store version |

**Honest net.** The organ grows by about +330 source lines, and the net test
growth is modest. What that buys:
- typed selection with a hop, a cursor and an exact snapshot, where there was a
  scan with a clock order;
- a value door where malformed filters read as empty;
- one edge list where there were two;
- a read-back of the admission order that the clock could not give.

What it retires:
- two hard-coded relation endpoints;
- an order that was a timestamp;
- a filter language private to the organ.

**This is budget pressure, not a metric.** Hands may escalate a miss with an
argument.

## 7. Build budget

- **Hosts.** The host and the target are both the workstation (Windows). Cut 14
  builds on Yggdrasil, and nothing here proves anything about Linux.
- **Commands.** Cargo runs through PowerShell only.
- **Huginn.** Use the target that the Huginn checkout already uses
  (`C:\Users\Meta\.cargo-target-codex`). **A second checkout of Huginn (Soul's)
  uses its own subdirectory**, following the Eureka scar about shared build
  output.
  - Builds: `huginn-mind` and `huginn-daemon`, lib and tests, debug.
  - RS-2 and RS-1 are warm: about +20 to +80 paths.
  - RS-3 after BP-2 is warm at X: `cultnet-rs` is already compiled for the
    daemon, and `huginn-mind` recompiles.
- **Leaf.** `epiphany-pipeline` lib tests, warm: about +10 paths.
- **CultLib P-1/P-2.** `cultnet-rs` lib and tests, run on the selection branch's
  own target, as its campaign has it.
- **Measured.** The probe's cold build of `cultnet-rs@00f4c02`, `huginn-mind`
  and the leaf in a fresh target took 3m07s at 0.37 GiB. That is an upper bound
  for any cold rebuild this campaign causes. C: had 229 GiB free.

## 8. Deliberately not in scope

- **A selection over another mind or several minds.** A mind is the instance.
  Selection §13 is "not across shards or minds".
- **Numeric aliases.** `revision`, `attempt`, `pass` and `sequence` could be
  declared numeric later, as a normal change. No caller asks for them now.
- **Text search.** `semantic` stays beside `selection` and stays refused until
  Cut 11, which owns it. Cut 11's lowering of `FieldPredicate` to Qdrant filters
  is Cut 11's.
- **Watching a selection.** Selection Cut 2 is not Huginn's, since Huginn does
  not subscribe (selection map D7).
- **Oversize pages.** A `document`-projection page can exceed the window. The
  typed `ResponseTooLarge` refusal stands until BP-3. Cut 13 should default to
  `header` with a small `limit`.
- **Sealing the store** so that admission is its only writer (Cut 10 F7). That
  would make ordinal uniqueness structural instead of resting on the single
  writer. It is recorded as the follow-up that owns that gap.
- **Migrating old stores.** The ruling is to refuse. No import path exists
  (F2 shows why).
- **Named conveniences** for the old endpoints. Cut 13 decides those.
- **Rust alias matching for `schemas`.** This is the substrate's (D7).
- **The leaf's `Short` admitting an empty campaign or cut-spec title.** It is in
  scope only if Q-RS1 B is ruled.
- **S7 (Windows device names in the slug grammar).** It is unrelated and remains
  recorded.

## 9. Operator questions

**Two new forks. Q-BP1 and Q-BP2 stay the operator's and are not re-asked
here.**

- **This cut depends on Q-BP1 for pin timing only** (§4, conditional
  default).
- **It depends on Q-BP2 not at all.** The header summary exists either way, as
  `projection: header`. Under Q-BP2 B, BP-3 would reuse this cut's
  `PipelineDocumentSummary`. It would not grow a second one.

**Q-RS1: RULED B by the operator, 2026-09-22 ("good rec").** A new leaf text
type, `Title`, holds 1 to 200 bytes. All four titles use it (campaign, cut
spec, question and ruling), in epoch v2. RS-L implements it.

*History, the question as asked:* **Q-RS1. What is a title? The leaf's epoch moves in RS-L, so this is the free
moment to decide.** Titles are required by the ruling. The leaf's `Short`
accepts the empty string, so a required `Short` can still be a bare leaf.
Campaign and cut-spec titles are already `Short` and can be empty today.

- **A.** A non-empty title for `question` and `ruling` only: a new leaf text
  type `Title`, 1 to 200 bytes. The other two titles stay as they are.
- **B.** `Title` for all four titles (campaign, cut spec, question, ruling),
  in the same epoch v2.
- **C.** `Short`, like the existing titles. Empty is allowed, and a non-empty
  rule would be a later change.

**Recommended: B.** The epoch moves anyway in this cut, and all thirteen type
ids move with it by the leaf's rule. Tightening the two existing titles now
costs about 4 lines and one more loosening mutant. Doing it later costs a
second epoch and a second store refusal, which is exactly what the "same store
version" ruling set out to avoid. C honours the letter of "gain a title" and
still admits a bare leaf.

**Depends:** the RS-L field types, the `L3` entry, the leaf's schema text, and
every fixture that builds these kinds.

**Q-RS2: RULED A by the operator, 2026-09-22 ("A, for sure").** `huginn-mind`
depends on the whole of `cultnet-rs`. Cut 8's standing check narrows from "no
`cultnet`" to "no transport use in `huginn-mind`". RS-3 rewrites that check.
**Follow-up recorded, FU-SelSplit (CultLib).** Make selection separable from
the transport, either as a feature or as a small package. Trigger: a second
consumer wants selection without CultNet's transport.

*History, the question as asked:* **Q-RS2. May `huginn-mind`, the rule engine, link `cultnet-rs`?** Q13 A put the
wire types in `huginn-mind`, and the substrate's `Selection` rides the wire. So
`huginn-mind` must depend on the crate that defines `Selection`. That crate
also carries CultNet's transport, crypto and sockets. Cut 8's standing check
says `huginn-mind` has no `cultnet` at all (F11).

- **A.** `huginn-mind` depends on `cultnet-rs` whole.
  - Cut 8's negative grep narrows from "no cultnet" to "no transport use"
    (sockets, the RUDP hub, `bind(`, the index ports).
  - No package is added to the workspace, because the daemon already links
    all of it.
  - Cost: the rule engine links code it must never call, held by a grep
    rather than by the graph.
- **B.** CultLib first makes selection separable. That means either a
  `transport` default feature on `cultnet-rs`, so that `huginn-mind` uses
  `default-features = false`, or a small `cultnet-selection-rs` package that
  `cultnet-rs` re-exports. Then `huginn-mind` links only `serde`, `base64` and
  `sha2`.
  - Cost: one CultLib cut before RS-3, plus either a feature flag or one more
    package in CultLib's Rust set. Doctrine treats both as liabilities that
    need a named consumer. Huginn would be that consumer.
- **C.** Move the wire types out of `huginn-mind`, reversing Q13.

**Recommended: A.** It follows from the two rulings as they stand. The crate
boundary Cut 8 drew was about *behaviour* (no network in the rule engine), and
a narrowed grep pins that behaviour. B is the right answer only if a consumer
appears that must not link transport, and a client of Huginn is not one:
`eureka-state` needs `cultnet-rs` for its transport anyway. Record B as a
CultLib follow-up with that trigger.

**Depends:** RS-3's `Cargo.toml` and negative checks, the Cut 8 text Self
amends, and whether RS-3 waits on a CultLib packaging cut (B).

## 10. Selection's near-final code this cut depends on (what a Soul finding could change)

All anchors are `selection.rs@00f4c02`.

| Dependency | Where | If it changes |
|---|---|---|
| `Evaluation` has no `matched` | `:1080-1085` | **Must change (P-1).** RS-3 fills `matched` from it. |
| Selection types lack `Eq` | `:400`, `:435` | P-2, or Huginn drops `Eq`. |
| `Cursor::parse` is public and exposes `as_of` | `:1301-1346` | **Load-bearing.** Huginn reads `as_of` before `select` to restrict rows. If Soul makes the cursor private, the substrate must expose `cursor_as_of(&str) -> Result<u64, SelectionRefusal>` instead. It must not become an organ-side parse of an opaque format. |
| `select` refuses `CursorStale` exactly when `cursor.as_of != as_of` | `:1131-1136` | Huginn passes the cursor's own `asOf`, so equality holds under any comparison rule. |
| `select` does not call `validate` | `:1089-1114` | Huginn calls it first. If `select` starts validating, Huginn's call becomes redundant: delete it, and keep one owner. |
| `validate` does not refuse unknown `schemas` | `:864-873` | Huginn's door refuses them. If the substrate starts refusing, **delete Huginn's copy.** |
| `target_leaves(role)` has no schema parameter, and empty means unrestricted | `:747`, `:1204` | Huginn's roles are unique and "any" is spelled as the 13 ids, so it is unaffected either way. |
| Rows are indexed by record key alone | `:1095` | Huginn's keys are unique across kinds (F12), so it is unaffected. |
| `Row::ordinal -> i64`, `as_of: u64` | `:699`, `:1093` | A cast in `rows.rs`. |
| Glob re-export: `select`, `validate`, `matches`, `Cursor` at the crate root | `lib.rs@00f4c02` (`pub use selection::*`) | If they move under `cultnet_rs::selection::`, only spelling changes. Huginn always calls them qualified. |
| `projection` is a `String` with two constants | `:374-376`, `:462` | Huginn matches the constants. An enum would be a spelling change. |
| The published `$id` of `cultnet.selection.schema.json` | `contracts/cultnet/…@00f4c02` | RS-3's `$ref` names it. A move of the `$id` fails Huginn's byte-for-byte schema test, which is the intended tripwire. |

## 11. What was probed, what was only read, and what is not settled

- **Probed by running code:**
  - F1: all seven old relations equal a `select` over rows. Organ answers and
    substrate answers match in members and in order.
  - F2: the real store has 31 receipts in 5 seconds.
  - F3: the receipt is a positional 7-slot array; a v2 strict decode fails;
    a v2 decode with a default gives a silent 0.
  - F5: the real v1 store is refused by today's gate order as `ForeignStore`
    and by epoch-first order as `ForeignEpoch`.
  - F6: `CursorStale` on an `asOf` mismatch.
  - F7: the substrate door accepts malformed values.
  - F9: `E0117`, and the `$ref` via `schema_with` works.
  - F10: `E0277` on `Eq`.
  - F12: key uniqueness.
  - The duplicate `cultcache-rs` when revisions differ (the body-plane map's F5)
    was seen in the probe build.
- **Read only:**
  - `Evaluation` lacking `matched` (F8). It is certain from the struct but not
    exercised.
  - That a required leaf field fails an old payload with "missing field". This
    follows from serde. It is not probed, because the opener refuses such a
    store first in any case.
  - The `$defs` layout of CultLib's response schema.
  - The Cut 8 grep text.
- **Not probed:**
  - **The probe's `huginn-mind` was the working tree at `4f7e3b5` plus the
    residue batch's uncommitted edits.** It was read through a path dependency
    and was never modified. The relations it exercised (`open_items`,
    `history`, `query`, `receipts`, `open`) are outside the residue's scope
    (the gate fixtures, `path_for`, the `open_with` door), so the results
    should hold at the post-residue HEAD. Hands re-runs the equivalence as
    RS-3's re-pinned tests in any case.
  - C# decoding anything Huginn emits. Huginn's wire is its own; only
    `Selection` is shared, and the selection campaign's parity vectors own it.
  - Performance. Every read still decodes the whole image and every receipt
    per call (F1), and RS-3 adds a row build. That is fine at campaign scale
    and is not measured at a thousand-document mind.
- **Not settled here:** Q-RS1 and Q-RS2 are the operator's; Q-BP1 and Q-BP2 are
  already open. P-1 and P-2 are the selection campaign's to accept.
- **Substrate missing, recorded per the skill:** typed pipeline state. This map,
  its model page and its questions live in markdown, and the durable summary
  spec (earlier read-side D1) lives in another session's scratchpad. Huginn is
  the organ that would hold both, and it is the thing being built.

## Appendix A. The summary type (carried here from cut10b-read-side-spec.md D1, 2026-09-17)

Copied verbatim by Self on 2026-09-22 from a session scratchpad that will not survive. It predates the move of selection into CultNet: `PipelineQuery`/`Projection` here are the organ's own vocabulary and are superseded by CultNet's `Selection` and `projection`. What remains authoritative is `PipelineDocumentSummary`, `PipelineStatusSummary`, `PipelineFacts`, the field table and the size bound. Q-RS1 decides whether question and ruling facts gain a `title`.

### D1. Projection: a summary is the default, the document is asked for

`PipelineQuery` gains `projection: Projection` with
`enum Projection { Summary, Document }`, `Default = Summary`. The page's
items become one of two vectors, not a per-item tag:

```
pub enum PipelineQueryItems { Summaries(Vec<PipelineDocumentSummary>), Documents(Vec<PipelineDocumentView>) }
pub struct PipelineQueryPage { pub items: PipelineQueryItems, pub matched: u32, pub as_of: u32, pub next: Option<String> }
```

`PipelineDocumentView` is unchanged and remains `view`'s answer and Cut 11's
index source. The summary:

```
pub struct PipelineDocumentSummary {
    pub id: PipelineRef,
    pub admission: AdmissionFacts,          // unchanged type: receipt_id, admitted_at, provenance
    pub status: PipelineStatusSummary,
    pub facts: PipelineFacts,
}
pub enum PipelineStatusSummary {
    InForce,
    Resolved { resolution: PipelineRef, outcome: ResolutionOutcome },   // the record's outcome, not its rationale
}
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]   // the leaf's own envelope spelling
pub enum PipelineFacts {
    Campaign    { title: Short, repos: Vec<OrgRepo> },
    Target      { revision: u32, invariants: Vec<Label> },
    Question    { label: Label, options: Vec<Label>, recommended: Label, raised_in: Option<PipelineRef>, asked_on: Date },
    Ruling      { label: Label, answers: Option<Short>, choice: Option<Label>, authority: RulingAuthority, ruled_on: Date },
    CutSpec     { cut: Label, revision: u32, title: Short, repo: OrgRepo, branch: Short, base: Sha, depends_on: Vec<Short> },
    CutReport   { cut_spec: Short, attempt: u32, repo: OrgRepo, branch: Short, range: CommitRange },
    Verdict     { cut_report: Short, pass: u32, range: CommitRange, outcomes: Vec<ClaimOutcome> },
    Finding     { verdict: Short, label: Label, confidence: FindingConfidence, severity: FindingSeverity, origin: FindingOrigin, claim: Line, invariants: Vec<Label>, range: CommitRange },
    FollowUp    { label: Label, source: PipelineRef, repo: OrgRepo, owner: Short, item: Line },
    Resolution  { subject: PipelineRef, sequence: u32, outcome: ResolutionOutcome, resolved_on: Date },
    Instance    { instance: Slug, display_name: Short, host: Short, created_at: Date },
    Stewardship { instance: Slug, repo: OrgRepo, sequence: u32, assigned_on: Date, note: Line },
    HandOff     { from_instance: Slug, to_instance: Slug, repo: OrgRepo, handed_on: Date },
}
```

Each field, by the question it answers for a caller deciding what to work on:

| Field | Question |
|---|---|
| `id` | what to `view`, cite, or resolve; the key already carries campaign, cut and label, so those are not repeated. |
| `admission` | who landed it, when, under which faculty (ruling 18: attribution). The type is Cut 9's, unchanged, so there is one admission shape. |
| `status` | is it open, and if not, what closed it and how (`Answered by`, `Superseded by`, `Withdrawn { reason }`). The closing record's `rationale` (Para) and `resolved_on` are the one thing dropped: the caller who wants why opens the resolution. |
| Campaign `title, repos` | which repos are in jurisdiction. |
| Target `revision, invariants` | which invariant labels findings and verdicts may cite; whether this is the standing revision (with `status`). |
| Question `label, options, recommended, raised_in, asked_on` | is it open, what are the choices, what raised it, how old it is. The question text is `Para` and not carried. |
| Ruling `label, answers, choice, authority, ruled_on` | did it answer a question and with which option; was it the operator's, standing, or defaulted. The ruling text is `Para` and not carried. |
| CutSpec `cut, revision, title, repo, branch, base, depends_on` | which cut, which revision, where it lands, what it waits on. |
| CutReport `cut_spec, attempt, repo, branch, range` | which spec it reports, which attempt, what range to check out. |
| Verdict `cut_report, pass, range, outcomes` | which report, which pass, and whether any claim was falsified or unproven, one enum per claim in claim order. |
| Finding `verdict, label, confidence, severity, origin, claim, invariants, range` | the triage row: how sure, how bad, whether it was introduced, which invariant it breaks, and its one-line claim. `claim` is the leaf's own `Line` and is carried whole, not projected; `failure_scenario` (Para), `locations`, `evidence` are not. |
| FollowUp `label, source, repo, owner, item` | who owns it, where, and its one-line item (the leaf's `Line`). |
| Resolution `subject, sequence, outcome, resolved_on` | what it closed, its place in the subject's history, how. |
| Instance | the whole document: four short fields. |
| Stewardship `instance, repo, sequence, assigned_on, note` | which assignment, and the note, which on a derived assignment carries the hand-off key (`docs.rs:136-140`). |
| HandOff `from, to, repo, handed_on` | who moved what where; `documents` (256 Shorts, 51 KB) and `reason` (Para) are not carried. |

**Rule the summary obeys, and the test pins:** a summary's encoded size is
bounded by a named constant independent of the document's lists.
`SUMMARY_MAX_BYTES = 8_192`. Arithmetic from the leaf's bounds: `id` 230,
`admission` 720, `status` worst 2,100 (`Superseded` by eight refs), facts worst
2,300 (CutSpec) or 2,100 (Resolution), names ~300: about 5.7 KB. A page of
200 is therefore ≤ 1.6 MiB in the pathological case and ~80 KB in the real
one (~400 B per summary). The maximal `Document` page stays what it is; the
daemon's size refusal (Self's F1 ruling) is the backstop for that projection,
and Cut 13's default projection is `Summary`.

**On prose in the summary (Q-A below).** `finding.claim` and `follow_up.item`
are carried because the leaf typed them as one-line statements. Question and
ruling have only `Para` text, so their rows show a label and a date. Whether
the leaf should give those kinds a `Short` title is a leaf question, raised,
not answered here.

**Rejected:** a per-item tag (`Vec<enum { Summary, Document }>`) — every item
would carry a tag and the caller would match per item for a decision made
once per query. A `fields: Vec<FieldName>` selector — a field-path language,
which is the thing bounded out. Carrying `PipelineStatus` whole in the
summary — 4 KB of rationale per resolved row against a bound whose point is
to be small.

