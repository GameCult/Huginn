<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/huginn-organ-cut.md; text below this line is unchanged. -->
# Huginn wired in: cut map for Cuts 11-15

Date: 2026-09-29. Imagination output (Opus). Nothing here is committed, and no
repository or live service was written to. Self places this map. Its natural
home is a section of `F:\Projects\Epiphany\notes\eureka-pipeline-state-cut.md`
that replaces the old Cuts 11-15 (lines 5490-5853). Mark those lines as history
the same day, so that no reader sees two live designs.

**Q-H1 reversed, 2026-09-30 (operator).** Self laid out the five things 14-I still needs and recommended the
interim unit. The operator: "So, what's blocking Huginn? I want us to be gathering evidence on Eureka ASAP" and then
"Go for it." **Q-H1 is now A.** Cut 14 builds the interim unit as mapped, and its deletion line stays 14-I. Self
deploys it under the operator's standing deployment authority, which replaces "the operator deploys" in Cut 14.
Q-H2 has moved on: authority backups now run to Raven (weekly tier added 2026-09-30), so Cut 14's backup items apply
under Q-H2 A. Cut 15 follows once 14 is live. The Q-H1 B text below is history.

**Cut 15 merged, 2026-09-30 (Self).** GameCult/Eureka `main` is at `bdb9dfd` (`051e1fc`, `2a0e9f7`, `272f5ca`,
`f0ff484`, `bdb9dfd`).
- Soul took three passes. The first found the Soul brief had stopped reading operator rulings (High). That is fixed:
  Soul claims against every cited ruling and every direction in force, independent of Hands' promises.
- Live checks passed against the mind. `eureka-state` was redeployed on CultLib `8fc74c70`, and checks 3 and 6 now
  pass from Starfire.
- The substrate gaps are tabulated in `references/campaign-state.md`, and each becomes a `follow_up` in the first
  campaign.
- **Operator, on moving to typed state: "how are we going to move to Huginn mid-campaign? Methinks we don't."**
  Campaigns already in flight finish in prose. The first typed campaign starts fresh in a new session: Idunn as a
  watchdog that reports to the operator on Discord (store-recovery Q-R4).

**Cut 14 status, 2026-09-30 (Self).** Huginn is live on Yggdrasil: `huginn.service` on `10.77.0.1:17872/udp`,
instance `eureka`, started 13:24 CEST.
- The release is `0c4e410` (the `eureka/memory-organ` head), with binary sha256 `a02ae1f5...`.
- gamecult-ops `a0320e4`/`14df7ce`, Huginn README `dcb85de`. The ufw rule admits Starfire only.
- Checks 1, 2, 4, 5 (at the firewall counters) and 7 pass. Check 4 admitted the mind's identity document
  `eureka:instance:self` (receipt `mind-commit-379a2a34...`), and it survived a restart.
- Check 8 waits for the 04:00 CEST backup on 1 October. The map's freeze-loop script no longer exists. The
  non-freezing state-backup producer covers `/var/lib/gamecult`, so no freeze loop was added.
- **Check 3 fails, and Cut 15 is blocked on it.** `eureka-state` on Windows cannot reach the daemon. The
  cultnet-rs client binds `127.0.0.1` by default (`cultmesh.rs:256`), and on Windows a loopback-bound socket cannot
  send to `10.77.0.1` (os error 10051). This is CultLib's defect, and it is fixed there (`hands/rudp-client-bind`).
  After that come Huginn's pin bump, the Windows rebuild of `eureka-state`, and a rerun of checks 3 and 6.
- Self default: the derived Qdrant collection `huginn_mind_eureka` joins the backup's `QDRANT_SKIP`, because the
  collection is disposable and the mind file is the truth.

**Operator rulings, 2026-09-29, each asked on its own:**

- **Q-H1: B, wait for Idunn.** Cut 14's interim systemd unit is **not**
  built. The organ deploys through Idunn after route continuity (Idunn
  `docs/route-continuity-cut.md`) lands. Cut 14 is remapped as 14-I once that
  campaign settles. A Rust runtime-presence implementation shared with Idunn,
  rather than a third copy, is its prerequisite, and where it lives is a fork
  to bring then. Cuts 11a, 11b, 13a and 13b proceed. They run against a
  hand-started daemon in tests only.
- **Q-H2: C, no backup yet.** Nothing on Yggdrasil is backed up today (the
  authority backup unit was never installed). That is recorded as a
  standing risk, not solved here.
- **Q-H4: one mind, instance `eureka`.** The operator asked first: "Wouldn't
  a single instance be bad for concurrency? ... can multiple Eureka instances
  share a Mind on Huginn?" Self answered from the Body. The daemon is the
  mind's only writer, and any number of sessions are its clients. Racing
  admissions get typed refusals. Focus comes from scoped queries, not from
  mind size. **Self's addition for Cut 13, from that exchange:** receipts
  name the instance but not the session that admitted a batch. With several
  sessions sharing one mind, the admitting session is recorded on the receipt
  (declared attribution, like the instance).
- **Q-H5: park Cut 12**, tag and note, not deleted.
- **Q-H6: A, the hybrid, with a standing direction.** The operator: "every bit
  of prose we're passing around should be viewed as a potential liability,
  *especially* if it's too big to just shove the whole thing into context. We
  should be looking for ways to turn what remains into typed state as well."
  Every Imagination pass looks for the next prose surface that could become
  typed (body facts, the model page, the subtraction ledger), and proposes it
  rather than adding prose.

**Objective (operator, 2026-09-29):** "Huginn wired in". Eureka's pipeline
runs on Huginn's typed state. In scope: Cuts 11, 13, 14 and 15. Cut 12 is
recommended for parking (see Q-H5). Out of scope: Cut 16 (the proof
campaign), the Verse-presence cut (Q23 A), a public or stranger-ready skill,
and the landing site.

**What lands ahead of these cuts** (mapped elsewhere, not re-mapped here):

- BP-1: CultLib content plane. It is in Hands on `cultmesh/rust-content-plane`, with three commits pushed.
- BP-2: the pin move.
- RS-3: `Mind::query(selection, semantic)`. It adds `cultnet-rs` to `huginn-mind`, and deletes `PipelineQuery` and `QUERY_LIMIT_MAX`.
- BP-3: `HuginnMindResponse::Deferred`, `MAX_DEFERRED_BODY_BYTES`, and a deferral budget of 256 MiB.

Every Huginn anchor below is against `eureka/memory-organ` at `8c5f784`, which
is clean. RS-3 and BP-3 move `query.rs`, `wire.rs` and `serve.rs`, so **each
brief re-takes its anchors after they land.**

## Pins

| Repo | Branch | HEAD | Used for |
|---|---|---|---|
| Huginn | `eureka/memory-organ` | `8c5f784` | Cuts 11 and 13 |
| Epiphany | `codex/eureka-pipeline-state` | `4f784d04` | the leaf's text fields (`epiphany-pipeline/src/lib.rs:510-578`), read only |
| Eureka skill | `main` | `19487f0` | Cut 15 (`~/.claude/skills/eureka`) |
| gamecult-ops | `main` | `8554e60` | Cut 14 |
| Idunn | (current) | `5bbb461` | read only. The route-continuity target and cut map are the dependency for 14-I |

## Probe facts (run 2026-09-29, read-only except as noted)

- **P1. Qdrant on Yggdrasil** is `qdrant 1.17.1`, on `127.0.0.1:6333`.
  - It holds exactly two collections, both voidbot's:
    - `voidbot_repository_source_chunks`: 116,877 points;
    - `voidbot_discord_history_chunks`: 40,788 points.
  - Both are 1,024-dimensional, `Cosine`, `on_disk: true`, with keyword payload indexes (`sourceId`, `corpusKind`, `repoName`, `pathPrefixes`, …).
  - **`config.metadata` is null on both.** They were restored, not created by voidbot's code. So voidbot's embedder-compatibility check (`VoidBot/packages/rag/src/qdrant-vector-store.ts:458-477`) has nothing to compare against in production.
- **P2. Raven's Ollama is live, and voidbot already uses it.**
  - `/srv/voidbot/retrieval/voidbot.env` reads `RAG_OLLAMA_BASE_URL=http://10.77.0.4:11434` and `RAG_OLLAMA_MODEL=qwen3-embedding:0.6b`. So the 2026-09-26 cutover has happened.
  - `gamecult-ops/inventory.md:669` and `runbooks/voidbot-retrieval-recovery-yggdrasil.md:40-62` still name Nightwing. They are stale.
  - Nightwing (`10.77.0.3:11434`) did not answer.
  - Epiphany's old Yggdrasil-local endpoint (`10.77.0.1:11435`) did not answer.
- **P3. What Raven serves.** Ollama `0.21.1`. Tags:
  - `qwen3-embedding:0.6b`: digest `ac6da0dfba84…`, Q8_0;
  - `bge-m3:latest`.
  - One embed call per model, from Yggdrasil, returned **1,024 dimensions for both**. Each call took **5-6 s cold**. `/api/ps` reports a context of 4,096.
  - Side effect: this probe loaded `bge-m3` into Raven's VRAM until its 5-minute keep-alive expired.
- **P4. How voidbot uses Qdrant**, which is the convention to reuse (`qdrant-vector-store.ts`):
  - The point id is the first 16 bytes of `sha256(chunk id)`, shaped into a v5-style UUID (`:264-280`). The real id travels in the payload.
  - Collections are created with `metadata {managedBy, corpusKind, embedderId, vectorLength, schemaVersion}` (`:203-216`).
  - **`embedderId` is `ollama:<baseUrl>:<model>`** (`ollama-embedder.ts:29-31`). Moving the host therefore reads as a model change, which is the hazard the 2026-09-26 memory warns about. Huginn keys compatibility on **model name plus digest**, not on the URL.
  - Query text is prefixed `Instruct: …\nQuery: …` when an instruction is configured (`:42-48`).
- **P5. WireGuard exists.**
  - Starfire has the adapter `wg-gamecult-starfire` at `10.77.0.2`, and it is Up.
  - Yggdrasil has `wg-gamecult` at `10.77.0.1/24`, reachable from Starfire.
  - `10.77.0.1:17872/udp` is unbound. Ports in use today: 17871 (Odin, via nginx), 17873, 17876 and 17890 (Idunn's host-actuator hub, which is not in R21's list).
  - **ufw admits UDP on wg only per port:** 3075, 17871, 17876 (from `10.77.0.2` only) and 17890. **17872 needs a rule.**
  - Starfire's 17877 RUDP requester is not listening today. So a Rust RUDP client on Windows over WireGuard has a precedent, but it is not proven live right now.
- **P6. `crates/eureka-state` is an empty stub:** `src/lib.rs` has 0 lines, and `Cargo.toml` has no dependencies.
- **P7. The daemon serves one mind per process.**
  - `serve.rs:91-111` takes exactly `--state-root`, `--instance` and `--bind`.
  - `Mind::open` takes a lifetime-exclusive redb lock (`mind.rs:127-139`).
  - `IndexSink::committed` runs **synchronously inside `Daemon::handle`, before the reply** (`daemon.rs:74-83`).
  - `run` is a single-threaded poll loop (`serve.rs:257-297`). Any blocking call made there stalls every session.
- **P8. The hand-off already works one side at a time.** Admission derives:
  - the source side's withdrawal when `from_instance == mind` (`admission.rs:331-346`);
  - the target side's stewardship when `to_instance == mind` (`:347-353`).
  So a hand-off is two ordinary admissions, one per mind.
- **P9. Idunn v2's contract for a routed stateful target**, read from source:
  - The state slot kinds are only `cultcache-file` and `external-file` (`Idunn/src/deployment.rs:182-187`). **There is no `cultcache-directory`.**
  - A dependency on non-Idunn infrastructure is `kind = "external-operator-binding"`. It is satisfied by the binding's `external_capabilities` (`deployment.rs:1359-1383`, `deployment_plan.rs:146-249`). `shared-infrastructure` needs a managed provider with Odin correlation.
  - The route challenge for `rudp` is a CultNet `SnapshotRequest` for `gamecult.runtime_presence_health.v2`, keyed by the target (`drivers.rs:5524-5545`). The service must answer it with signed presence, signed with the Idunn activation credential. It must also adopt the process write lease.
  - Huginn implements none of this.
  - **No shared Rust implementation exists.** `cultnet-rs` has the contracts and signers (`runtime_authority_contracts.rs`, 3,309 lines). Each Rust service writes its own participant: Ghostlight's `idunn_health.rs` is 2,001 lines, and Odin has its own.
  - Unit names are `idunn-<target>-<sha256>` per incarnation.
  - There are **391 failed `idunn-*` transient units** on Yggdrasil today.
- **P10. No backup runs on Yggdrasil.**
  - `gamecult-authority-backup.service` is not installed. The only backup timer is `dpkg-db-backup.timer`.
  - The script `scripts/backup-gamecult-authority-yggdrasil.sh:74-76` would refuse at its first step anyway: it freezes `epiphany.service` and `epiphany-swarm.service`, which are absent (`freeze_unit` returns 1 when a unit is inactive).
  - Its path list does not name any Idunn target's state root either, including `odin` and `streampixels-service`.
- **P11. How Claude Code reaches MCP servers today** (`~/.claude.json`):
  - `voidbot` at `:17875` is the **Yggdrasil-hosted** MCP, reached through the SSH tunnel (`start-yggdrasil-tunnel.ps1:24`).
  - `aquasynth_dings` at `:17878` runs locally.
  - The target's "Shape decided" says `eureka-state` is a local **stdio** server. That governs here. The HTTP convention recorded in memory is not a ruling.

**Not probed:**

- whether `cultnet-rs`'s RUDP client works on Windows;
- whether Claude Code accepts a tool schema that `$ref`s a remote `$id`;
- whether Qdrant accepts `.` in a collection name;
- whether `ureq` without TLS covers both adapters;
- which connection id Idunn's RUDP challenge uses.

Each is assigned to a cut's first step below.

## Standing rulings, checked against the maps

| Ruling | Where it is recorded | Holds? |
|---|---|---|
| An instance owns its mind; a service owns the state | target 14 and 15; map rulings 14 and 15 | yes |
| The organ depends on Qdrant directly, not on voidbot | target 16; map 16 | yes |
| The organ shares voidbot's Qdrant, with the coupling declared | map 21 (Q9 A) | yes. "Declared" means an Idunn dependency, which the interim deploy (Q-H1 A) cannot express. The runbook states the coupling until 14-I. |
| Identity is declared, not authenticated | map 18 (Q6 A) | yes |
| The workstation reaches the organ over WireGuard | map 19 (Q7 A) | yes. P5 confirms the mesh; the ufw rule is Cut 14's. |
| Eureka has no Persona | target 6; map 9 | yes |
| Embeddings: Raven's Ollama (operator, 2026-09-26, for voidbot) | memory `nightwing-embedding-migration-2026-09-26.md` | **the target disagrees.** Target ruling 16 still says "Epiphany's own service embeds against a Yggdrasil-local endpoint … only voidbot uses Nightwing". Both endpoints are dead (P2). Self sweeps that sentence. Huginn uses the endpoint voidbot uses, with the same model. |
| `open_items` and `history` are deleted, not kept as presets | read-side map (2026-09-17) | yes. Cut 13's old seven tools shrink to four. |
| Q13 A: the client depends on `huginn-mind` for types only | map ~3931 | yes |
| No committed mutation harness | operator, 2026-09-22 | yes. Old Cut 11 and 12's "Mutations:" lines are void. Soul runs `cargo-mutants` on each diff. |

## 0b. Model page

One row per persistent or long-lived kind these cuts touch. **A bold cell is empty or disputed.**

| Kind | What names it | What happens to it over time | Who decides |
|---|---|---|---|
| **Index point**: one per indexable document | A v5-shaped UUID from `sha256(document id)` (P4's function, reused). The payload holds `doc_id`, `kind`, `root`, `ordinal` and `text_sha256`. | Written after the document's batch commits, off the serve loop. Never updated, because documents are immutable. Never deleted, except when its collection is rebuilt. A missing point is recreated by reconciliation. | Only the daemon's index worker writes one. `huginn-mind::index_text` decides what the text is. Qdrant decides nothing. |
| **Collection**, one per mind | `huginn_mind_<instance>`. Its metadata is `{managed_by: "huginn", instance, model, model_digest, dimensions, index_text_version}`. | Created on the first reconciliation. Rebuilt whole when its metadata disagrees with the running configuration. **A collection whose mind is retired has no owner.** With one instance this is a recorded follow-up, not a cut. | The daemon serving that instance. It never touches a collection whose `managed_by` or `instance` is not its own. |
| **Index text version** | `huginn_mind::index::INDEX_TEXT_VERSION` | Bumped whenever `index_text` changes. A bump makes every collection disagree, so each is rebuilt. | `huginn-mind` |
| **Embedding model binding** | Model name plus the digest from `/api/tags`. Never the base URL (P4). | Changes when the operator changes the configuration, or when Raven re-pulls the model and the digest changes. Either one rebuilds the collection. | The operator, through the unit's arguments. The daemon verifies the binding at each reconciliation. |
| Index health (behind or unreachable) | none: in-memory in the worker | Lost on restart, and recomputed by reconciliation at startup. Never persisted, never written to the mind. | The worker |
| Pending semantic reply | `(session, message_id)` in the serve loop | Lives until the worker answers or the session times out. Lost on restart, and the client asks again. | `serve` |
| **Eureka instance** | A slug. **The operator names it (Q-H4).** | Created by the first admission of its `instance` document, which Self writes at the first campaign. Never renamed: a rename is a new mind. | The operator names it. Admission enforces it (A1, `require_instance`). |
| MCP registration | `eureka-state` at user scope in `~/.claude.json`, with the environment variables `EUREKA_INSTANCE` and `HUGINN_ENDPOINT` | One stdio child per Claude Code session. It dies with the session and holds nothing. | The operator, because it is configuration |
| Deferred body (BP-3), client side | The manifest's `contentHash` | Fetched and verified per call, then dropped. Never written by `eureka-state`. | `cultnet_rs::fetch_content` owns verification. |
| **Deployed daemon identity** | Interim (Q-H1 A): the systemd unit `huginn.service` on Yggdrasil, with runtime id `huginn-<slug>` (`daemon.rs:54-56`). Later: `idunn-huginn-<sha>`. | Interim: systemd restarts the unit, which is continuity. **The operator installs a new binary, which is deployment, and no brake gates it (Q-H1).** | Interim: the operator. After 14-I: Idunn. |
| Mind store | `/var/lib/gamecult/huginn/minds/<slug>/mind.redb` | Preserved forever. It is the only truth. **No backup owner runs (P10), so this is Q-H2.** | The daemon is the single writer, through the redb lock. |
| Firewall admission | A ufw rule: `10.77.0.1 17872/udp on wg-gamecult from 10.77.0.2` | Installed once. It dies in 14-I if Idunn's route owns it. | Interim: the operator |
| Hand-off / imported mind | The leaf `hand_off` key | Admitted separately into each mind (P8). Import has no consumer (Q-H5). | `huginn-mind` admission |

## Operator questions (one batch)

**Q-H1. How does Huginn get onto Yggdrasil this week?**

- **A. An interim operator-installed systemd unit, retired into Idunn by a named later cut (14-I).**
  - The operator builds the binary in the pinned Rust image, at an exact revision, on Yggdrasil.
  - It binds `10.77.0.1:17872` directly, with no nginx in the path.
  - The state root is chosen now so that 14-I is a unit swap and no data moves.
  - Costs:
    - it contradicts the old authority map's "no operator script deploys Huginn directly";
    - it has no deployment brake;
    - the Qdrant coupling is written in a runbook rather than declared to Idunn.
- **B. Wait for Idunn.** Huginn is deployed only through Idunn, after two things land: route continuity's route-proof class (its Q1 b) and a shared Rust runtime participant in `cultnet-rs`. "Wired in" does not close this week.
- **C. Idunn now, against today's contract.**
  - Huginn writes a third copy of the service-side presence, activation and lease code (Ghostlight's copy is 2,001 lines).
  - It also has to answer the route challenge and pass Odin-correlated readiness.
  - Its RUDP route goes through nginx UDP, whose reload loop is exactly what route continuity exists to fix. Every reload breaks RUDP sessions (route-continuity target).
  - All of this is reworked when that campaign lands.
- **Recommended: A,** with 14-I named, dated and blocked on route continuity plus the shared participant.
  - Neither B nor C gives a working organ on a path that will survive.
  - C would build the thing route continuity is deleting.
  - A's authority gap is real. The operator, not a map, should accept it.
- **Depends on it:** Cut 14's whole shape, whether this week closes, and whether 14-I exists.

**Q-H2. Who backs up the mind?** The target requires a named backup owner. None runs (P10).

- **A. Revive the authority backup.**
  - Edit the freeze list: drop the two absent Epiphany units and add `huginn.service`.
  - Add `var/lib/gamecult/huginn` to the path list.
  - Install the timer.
  - Side effect: daily backups resume for voidbot, Bifrost and Idunn configuration, including a daily pause of five containers.
- **B. A Huginn-only timer:** freeze `huginn.service`, tar `minds/`, thaw. This makes a second backup mechanism.
- **C. Ship with no backup,** and record it as a blocking follow-up.
- **Recommended: A.** One owner of backup, and the script is already that owner. It is simply not running. **The operator must accept the side effects, and should know that Idunn targets' state (`odin`, `streampixels-service`) is in no backup either.**
- **Depends on it:** Cut 14's gamecult-ops diff, and one verification step.

**Q-H4. What is the Eureka instance called, and how many are there?**

- **A. One instance for all Claude Code Eureka work,** for example `eureka`, with host `starfire`.
- **B. One per workstation or per operator,** for example `metacrat.starfire`.
- **Recommended: A.** Ruling 14 lets one instance steward several repos. Stewardship, not the instance, is what separates repos.
- **Depends on it:** the unit's `--instance`, the state path, the collection name, and the MCP environment. It is cheap now and a new mind later.

**Q-H5. Park Cut 12?** (the consumer audit)

- Nothing that "wired in" needs calls a cross-mind hand-off or an import:
  - there is one instance (Q-H4);
  - no second mind exists;
  - Epiphany's adoption is the second campaign.
- Old Cut 12's `hand_off(&mut Mind from, &mut Mind to)` **cannot be built as specified**:
  - each mind is locked by its own daemon process (P7);
  - "both minds or neither" would need a cross-process protocol.
- The hand-off already works as two ordinary admissions, one per mind, each derived and idempotent (P8).
- **Recommended: park it.** The map keeps a note: the hand-off is two admissions, sender first. Import waits for a consumer. No tag is needed, because no code exists.
- **Depends on it:** whether Cut 12 is built at all.

**Q-H6. What does Eureka keep in typed state, and what stays in prose?**

- **A. A hybrid.**
  - Typed documents hold the questions, rulings, `cut_spec` (Hands' contract), `cut_report`, verdicts, findings, follow-ups and resolutions.
  - The status header becomes a query.
  - The committed map keeps only what has no kind: body facts, the model page, rationale, and the subtraction ledger narrative.
  - Per-cut specs live only as `cut_spec`, so no spec exists twice.
- **B. Typed state for records only.** Specs stay in the prose map, and `cut_spec` goes unused.
- **C. Retire the markdown map entirely** (old Cut 15).
- **Recommended: A.**
  - B leaves the kind the leaf spent most of its bounds on without a consumer.
  - C discards body facts and the model page, which no kind carries. Step 0b is what stopped re-cutting.
- **Depends on it:** Cut 15's text, and the output format of every future Imagination pass, this one included.

## Cut order

```
[BP-1 → BP-2 → RS-3 → BP-3]                     (in flight, other maps)
        │
BP-2 ──► 11a  Huginn: the index write side (text, adapters, worker, reconcile)
        │      (after BP-2; may land before or after RS-3, serially on the branch)
        ▼
BP-3 ──► 11b  Huginn: semantic read (rank the hits through the one evaluator; non-blocking)
        ▼
       13a  Huginn: envelope moves to huginn-mind; eureka-state client core
        ▼
       13b  Huginn: the MCP surface, Windows build and install
        │
11a ──► 14   gamecult-ops: the interim unit, ufw, runbook, backup (Q-H1 A, Q-H2); the operator deploys
        ▼
       15   Eureka skill: typed-state wiring (Q-H6); after 13b is registered and 14 is live
       ---
       12   parked (Q-H5)        14-I  Idunn path, blocked (see the end)
```

## Cut 11a. The index write side

- **Repo/branch:** Huginn, `eureka/memory-organ`, after BP-2. The worktree is named by Self.
- **First:** Hands checks, in `ureq` 3's own Cargo manifest, that `default-features = false` gives plain HTTP with no TLS. If it does not, Hands uses `reqwest` with `blocking` and no TLS, and reports which.
- **Deletes first:**
  - `NoIndex` (`daemon.rs:20-27`) as the binary's index. It survives only in tests, or goes if the new worker's mock serves them.
  - The "No crate opens that connection yet" line in `README.md`.

**Adds, `huginn-mind/src/index.rs`.** This is pure: no network and no JSON.

- `pub const INDEX_TEXT_VERSION: u32`.
- `pub struct IndexEntry { id: PipelineRef, text: String, text_sha256: String, ordinal: u64 }`.
- `pub fn index_text(&PipelineDocument) -> Option<String>`. It reads the leaf's fields (`lib.rs:510-578`), joined by newlines and bounded at `INDEX_TEXT_MAX_BYTES` (16 KiB, cut at a char boundary):

  | Kind | Fields |
  |---|---|
  | campaign | `title` |
  | target | `invariants[].statement`, `not_in_scope`, `canonical_implementations` |
  | question | `title`, `question`, `options[].text` |
  | ruling | `title`, `ruling`, `operator_quote` |
  | cut_spec | `title`, `first`, `adds`, `keeps_moves` |
  | cut_report | `commits[].subject`, `deviations` (`what`, `why`), `undone`, `promises[].text` |
  | verdict | `claims[].claim` |
  | finding | `claim`, `failure_scenario`, `evidence[].result` |
  | follow_up | `item`, `why_it_can_wait` |
  | resolution | `rationale`, **withdrawn ones included** (the Q17 B obligation) |
  | hand_off | `reason` |
  | instance, stewardship | none |

- `impl Mind { pub fn index_entries(&self, only: Option<&[PipelineRef]>) -> Vec<IndexEntry> }`. The ordinal comes from the writing receipt (RS-1).
- Owner: `huginn-mind`, because "which fields are text" is a rule about the leaf's documents, and RS-3 names Cut 11 its owner.
  - Live consumer: the daemon's worker.
  - Protected invariant: the text is derived, never stored, and never includes receipts, provenance or status.
  - Why no existing owner serves: the leaf owns shape, not retrieval text, and the daemon owns no rule.

**Adds, `huginn-daemon/src/index/`.** Three files: `mod.rs`, `ollama.rs`, `qdrant.rs`. The daemon depends on `ureq` (see First) and `serde_json`, and `serde_json` is used only in the two adapter files: that is the xenos boundary.

- **Ports**, which are narrow traits so tests mock them:
  - `Embedder { model_identity() -> ModelIdentity{name, digest, dimensions}; embed(&[String]) -> Vec<Vec<f32>> }`;
  - `VectorIndex { describe(name) -> Option<CollectionMeta>; recreate(name, meta); ids(name) -> BTreeSet<String>; upsert(name, points) }`.

  `search` is 11b's.
- **The worker** is one thread that owns both ports and receives jobs over a channel.
  - **`IndexSink::committed` builds the entries** (local and cheap) **and only enqueues them.** It never blocks and never waits on the network. This is the fix for P7.
  - **At startup, one reconciliation:**
    1. read the model identity;
    2. `describe` the collection;
    3. if it is absent, or its metadata disagrees, `recreate` it;
    4. if the metadata names another `managed_by` or `instance`, refuse and log, and never recreate;
    5. take `ids`, subtract, then embed and upsert what is missing, in batches of 32, with `wait=true`.
  - **On failure:** log, keep the failed entries in memory, and retry with a backoff that starts at 30 s and is capped at 10 min.
- **Options:** `--qdrant-url`, `--ollama-url` and `--embedding-model` join `parse_options` (`serve.rs:96`). **All three are required**, so there is no index-less mode. An unreachable index is a degraded projection, not a configuration.
- **The query instruction constant** for qwen3 lives beside the Ollama adapter. 11b uses it.

**Authority map.**

- Owner: the typed mind owns truth, and the worker owns the projection.
- Inputs: `IndexEntry`s built by `huginn-mind`, and the configuration.
- Outputs: Qdrant points and collection metadata.
- Derived state: the whole collection and the worker's in-memory health.
- Forbidden writers: nothing else writes `huginn_mind_*`; voidbot never does; admission never waits on the index; the mind never stores index state; there is no `pending_index` document (old Cut 11 is superseded).
- Shared paths: `IndexSink::committed` (live writes) and startup reconciliation share one embed-and-upsert routine.
- Deletion line: `NoIndex` leaves the binary.

**Verification** (Yggdrasil, stopgap, Rust image): `ygg-verify.sh F:/Projects/Huginn <sha> rust 'cargo test --locked --workspace'`, then `cargo mutants --in-diff` on the cut's range.

| Test (behaviour) | Rule |
|---|---|
| `an_admission_replies_while_the_embedder_is_blocked` (the mock embedder waits on a barrier; `handle` returns `Committed` before the barrier is released) | The index never delays or refuses admission |
| `reconcile_indexes_exactly_the_indexable_documents_the_collection_lacks` | The projection converges by set difference |
| `a_model_digest_change_rebuilds_the_collection` (same name, different digest) | Compatibility is keyed on model and digest, **never on the URL**. At least one fixture pair must differ only in URL and expect **no** rebuild. |
| `a_collection_owned_by_another_instance_is_never_recreated` | No cross-mind writes |
| `index_text_ignores_admission_facts_and_indexes_withdrawn_resolutions` | Text is derived from the document alone, and Q17's obligation holds |
| `point_ids_are_a_pure_function_of_the_document_id` (a fixed vector, computed by P4's algorithm) | Stable ids across restarts, with voidbot's convention |

- **Negative greps:**
  - `rg -n "ureq|reqwest|serde_json|qdrant|ollama" crates/huginn-mind/src` is empty;
  - `rg -n "thread::spawn" crates/huginn-daemon/src` hits only `index/`.
- **Live adapter check (operator-approved, because it writes to the shared Qdrant):** a scratch instance `verify.<rand>` on Yggdrasil admits two documents; its collection appears with P1-style parameters and the metadata above; then the collection is deleted by hand.

**Estimate:** about 650 lines added, most of them tests. +`ureq`, +`serde_json` (daemon only). −`NoIndex` from the binary.

**Build budget:** the Huginn workspace, on Yggdrasil only, with one release build for the live check.

**Hands context:** about 150k tokens: `daemon.rs`, `serve.rs` and `mind.rs` headers plus the new code.

## Cut 11b. Semantic read

**Added 2026-09-29 (Self), from Soul's pass on 11a:**
- **The mind judges every hit.** The index never deletes points. Withdrawn and
  superseded documents are indexed on purpose, and point payloads carry no status. So
  Qdrant supplies candidate ids and scores, and every hit is joined back through the
  mind: ids the mind doesn't hold are dropped (a restored mind, or a second store), and
  the rest are filtered by RS-3's in-force derivation, through selection. A hit is
  never shown on the index's word alone.
- **The effective embedding bound is the model's context, not 16 KiB.**
  `qwen3-embedding:0.6b` takes 4096 tokens and Ollama truncates beyond that silently.
  Long documents rank on their first ~4k tokens.
- **Model identity is re-checked on each flush.** A model re-pulled with the same
  dimensions and a new digest must not mix vectors until a restart. An idle mind whose
  collection is lost also stays `Current` until the next upsert fails. Found by Soul on
  11a's closing batch; 11b owns both.
- **11a merged** into `eureka/memory-organ`. It was: Soul closed it on conditions. A fix batch is in Hands: pin the
  backoff, reconcile again after any failure, label the collection with the mind's
  identity, and merge BP-3.

- **Repo/branch:** Huginn, after RS-3 and BP-3. Anchors are re-taken on their tip.
- **Deletes first:**
  - the refusal "semantic query: the index is not wired (Cut 11)", at both sites (`query.rs:296-299`, and the daemon test at `daemon.rs:534-541`, re-taken);
  - the test `semantic_query_refuses_typed_until_wired`. Its rule dies; the replacements are below.

**Adds.**

- **`huginn-mind`:** `Mind::rank(&self, selection: &Selection, hits: &[(PipelineRef, f32)]) -> Result<PipelineSelectionPage, MindRefusal>`. Its rules:
  1. A selection with a `cursor`, together with `semantic`, is refused `SelectionInvalid` (`field: "cursor"`), because semantic answers are not pageable.
  2. The rows are restricted to hit ids present in the image as of `head`. A hit with no document is dropped and logged by the caller, never fabricated.
  3. RS-3's own path runs: `cultnet_rs::validate`, the value door, then `select` over the restricted rows. **The one evaluator decides membership. Qdrant never filters.**
  4. Order is score descending, with ties broken by ordinal.
  5. Truncate to `limit`.
  6. `next: None`.
- **`huginn-daemon`:**
  - A `Query` with `semantic` is not answered inline. `serve` stores `(session, message_id, request)` and sends a `Search { text, top_k }` job to the worker.
  - The worker embeds with the query instruction and searches the collection, with no filter and an oversample of `min(top_k × 4, 200)`.
  - The loop drains worker replies on each poll, calls `rank`, and replies on the stored session.
  - When the worker is unhealthy, or the call fails: `Refused(Unavailable { detail })`, and the detail names the endpoint.
- **`VectorIndex` gains `search`.**

**Authority map:** the owner of membership is `rank`, over RS-3's evaluator. Qdrant supplies only a candidate set and scores. Forbidden: lowering a `FieldPredicate` into a Qdrant filter, because that would be a second evaluator; RS-3 §8 had left this to Cut 11, and this cut refuses it.

**Verification:**

| Test | Rule |
|---|---|
| `semantic_hits_pass_through_the_selections_own_predicates` (a hit of an excluded kind is absent) | One evaluator |
| `semantic_results_are_ordered_by_score_not_ordinal` | Ranking |
| `a_hit_without_a_document_is_dropped` | Qdrant is not truth |
| `an_unreachable_embedder_is_refused_unavailable_never_an_empty_page` | Honest availability |
| `another_session_is_answered_while_a_semantic_search_is_outstanding` (blocked mock embedder) | The loop never waits on the network |
| `cursor_with_semantic_is_refused` | Not pageable |

- **Live (operator-approved):** on the scratch instance, a paraphrase query returns a document that shares no exact words with it, and the hit resolves through the same page type.

**Estimate:** about 350 lines added. No new dependencies.

**Hands context:** about 150k tokens.

## Cut 13a. The envelope moves to its owner; the client core

- **Repo/branch:** Huginn, after 11b.
- **Moves (ownership only; one commit that builds on its own):**
  - `huginn-daemon/src/envelope.rs` (223 lines) moves to `huginn-mind/src/envelope.rs`: `encode_request`, `decode_request`, the response pair, `OperationFailure` and `FAILURE_SCHEMA`.
  - `huginn-mind` gains `base64`. It already has `cultnet-rs` through RS-3.
  - The daemon imports the module from `huginn-mind`.
  - Rule: **one spelling of the operation envelope.** The client and the daemon cannot drift apart.
  - Verified by the daemon's existing envelope tests, unchanged, run from their new home.
- **Adds, `crates/eureka-state` (library part).**
  - `Cargo.toml`: `huginn-mind` (types, envelope), `cultnet-rs`, `anyhow`, `serde`, `serde_json`.
  - `HuginnClient { endpoint: SocketAddr, instance: Slug, timeout }`:
    - its one method is `call(HuginnMindRequest) -> Result<HuginnMindResponse, ClientError>`;
    - the transport is `CultNetRudpSocketTransportConnection` on `CULTNET_OPERATION_CONNECTION_ID`, channel `schema`, as the daemon expects (`serve.rs:283`);
    - it resolves `Deferred` through `cultnet_rs::fetch_content`, capped at BP-3's `MAX_DEFERRED_BODY_BYTES` (imported, not copied);
    - `ClientError::Unavailable { endpoint, detail }` is the only transport error.
  - Owner: none over state; the client is a transport shim.
  - Forbidden: opening a store, calling `Mind::open`, `admit` or `admit_prepared`, deriving status, or writing any file.

**Verification** (Yggdrasil):

| Test | Rule |
|---|---|
| `every_operation_round_trips_against_a_live_daemon` (an in-process daemon on `127.0.0.1:0` with a temp mind) | Pipeline smoke |
| `a_closed_port_is_unavailable_naming_the_endpoint_within_the_timeout` | Loud unavailability. The timeout is a function of the configured value, so probe two values. |
| `a_deferred_answer_resolves_to_the_same_page_as_a_direct_one` | BP-3's client side |
| `a_foreign_instance_is_an_answer_not_an_error` | Refusals are data |

- **Negative grep:** `rg -n "Mind::open|\.admit\(|admit_prepared|OwnedRedb|std::fs::(write|create)" crates/eureka-state/src` is empty.

**Estimate:** about 300 lines added. The move is net 0.

**Hands context:** about 120k tokens.

## Cut 13b. The MCP surface, the build and the install

- **Landed before it (2026-09-30, Self):** Cut 11b and its follow-ups merged to
  `eureka/memory-organ` at `e3e0c38` and `ff39d80`. Cut 13a merged at `33641f4`
  (Soul: no blocker; fixes at `3dc50ee`). 13a's client now has **two** errors:
  `ClientError::Unavailable` (transport, timeout, out-of-protocol answer) and
  `ClientError::Rejected { endpoint, code, detail }` (an envelope-level
  `OperationFailure`). Its `timeout` bounds the whole call; answers are checked
  against the request's id and operation; every call disconnects its session.
  13b maps `Rejected` like `Unavailable`: a result with `isError: true` and the
  typed body, so an MCP caller can tell a permanent mismatch from a down daemon.
- **Built (2026-09-30):** `hands/cut13b` at `120e0a6`, green on Yggdrasil; Soul pass running. Windows
  binary `C:\Users\Meta\.eureka\bin\eureka-state.exe` (from `63ac503`). **Registered by the operator**
  (user scope), 2026-09-30. In Windows PowerShell the documented form
  `claude mcp add ... -e K=V -- <exe>` fails with "missing required argument 'commandOrUrl'": the
  variadic `-e` swallows the name and path. The form that worked puts positionals first:
  `claude mcp add eureka-state <exe> --scope user -e EUREKA_INSTANCE=eureka -e HUGINN_ENDPOINT=rudp://10.77.0.1:17872`.
  `whoami` answers `reachable: false` until Cut 14.
- **Repo/branch:** Huginn, after 13a.
- **First:**
  1. Hands builds a 20-line scratch rmcp server whose tool schema carries a `$defs`-internal `$ref`, and confirms `claude mcp` lists and calls it. It is not committed.
  2. Hands confirms the `claude mcp add` syntax from `claude mcp add --help`.
- **Adds.**
  - The binary `eureka-state` (`src/main.rs`) uses `rmcp = "2.2.0"`, the version Epiphany's lockfile already resolves, with `server`, `macros` and `transport-io`, plus `tokio` (`rt`, `macros`, `io-std`) and `schemars`.
  - **Configuration comes from the environment:** `EUREKA_INSTANCE` and `HUGINN_ENDPOINT` (`rudp://10.77.0.1:17872`). Missing or invalid configuration does not stop startup. `whoami` reports it, and every other tool refuses it, typed. An MCP server that dies at startup vanishes silently from Claude Code.
  - **Four tools, 1:1 with the wire.** The old `open_items`, `rulings_in_force`, `stewardship` and `history` tools are deleted designs. Their recipes move to the skill (Cut 15).

    | Tool | Input | Output |
    |---|---|---|
    | `whoami` | `{}` | `{ instance, endpoint, reachable, status: Option<MindStatus> }` |
    | `admit` | `{ faculty, agent, session, documents }` | `PipelineAdmissionOutcome`. `instance` and `provenance.tool = "eureka-state"` are filled in by the server, **never taken from input**. |
    | `view` | `{ id }` | `Option<PipelineDocumentView>` |
    | `query` | `{ selection, semantic? }` | `PipelineSelectionPage` |

  - Refusals are successful tool results carrying the typed refusal. `Unavailable` is a result with `isError: true` and the typed body. Malformed input is `invalid_params`.
  - **Input schemas are self-contained:** schemars output with the CultLib selection schema bundled into `$defs`, and no remote `$id` reference.

**Verification.**

- On Yggdrasil, a JSON-RPC exchange piped into the Linux build: `initialize`, `notifications/initialized`, then `tools/list`, which lists 4 tools and contains no `$ref` outside `#/$defs`. Then `whoami` against a closed port, which gives `reachable: false` and no JSON-RPC error.
- Test `admit_ignores_any_instance_in_input`. The rule: declared identity is configuration, not a per-call argument.
- **Windows (Starfire; Windows-only work, one job, nothing else running):** `cargo install --locked --path crates/eureka-state --root C:\Users\Meta\.eureka`. Hands reports the binary path and the build log's target directory.
- **Registration is the operator's:** `claude mcp add --scope user eureka-state -e EUREKA_INSTANCE=<Q-H4> -e HUGINN_ENDPOINT=rudp://10.77.0.1:17872 -- C:\Users\Meta\.eureka\bin\eureka-state.exe`, in the syntax confirmed by First.
- **Operator only, after Cut 14:** `whoami` from a Claude Code session answers `reachable: true`. This is the only proof of the Windows RUDP path over WireGuard.

**Estimate:** about 400 lines added. +1 binary, +`rmcp`, +`tokio`.

**Hands context:** about 180k tokens. rmcp's API is the unknown.

## Cut 14. Interim deployment (under Q-H1 A and Q-H2 A)

- **Repos:** gamecult-ops `main`, plus one line in Huginn's `README.md` for the run command. Depends on 11a, because the options exist from then.
- **Deletes first:** from `scripts/backup-gamecult-authority-yggdrasil.sh:74-76`, the absent `epiphany-swarm.service` and `epiphany.service` in the freeze loop (under Q-H2 A).

**Adds, gamecult-ops.**

- **`systemd/huginn.service`:**
  - `User=huginn`, with a system user created by the runbook;
  - `ExecStart=/opt/gamecult/huginn/current/huginn-daemon --state-root /var/lib/gamecult/huginn --instance <Q-H4> --bind 10.77.0.1:17872 --qdrant-url http://127.0.0.1:6333 --ollama-url http://10.77.0.4:11434 --embedding-model qwen3-embedding:0.6b`;
  - `Restart=on-failure`;
  - `After=`/`Requires=wg-quick@wg-gamecult.service`, because the bind needs the interface;
  - **no ordering on Qdrant or Raven**, because the index is a projection;
  - `StateDirectory`/`ReadWritePaths` limited to `/var/lib/gamecult/huginn`;
  - `ProtectSystem=strict`;
  - `MemoryMax=768M`, which covers BP-3's 256 MiB deferral budget plus the mind.

  Hands confirms the WireGuard unit's real name with `systemctl list-units` before writing `Requires=`.
- **`runbooks/huginn-yggdrasil.md`:**
  - Authority: the mind file is truth; the collection is disposable; Qdrant is shared with voidbot, and **this is the undeclared coupling ruling 21 wanted declared**; the embedder is Raven.
  - Release body: build in `rust@sha256:4c2fd73e…` (the Odin binding's pinned image) at an exact revision, with `cargo build --locked --release -p huginn-daemon`. Install to `/opt/gamecult/huginn/releases/<sha>/`, then flip `current`.
  - Install: user, directories, unit, and the ufw rule `ufw allow in on wg-gamecult from 10.77.0.2 to 10.77.0.1 port 17872 proto udp comment 'Huginn mind RUDP from Starfire'`.
  - Verification.
  - Recovery: stop the unit, delete `huginn_mind_<slug>` to force a rebuild, restore `minds/` from backup.
  - **Retirement:** this unit, the ufw rule and the release section die in 14-I.
- **`inventory.md`:** allocate 17872 to Huginn, and add a Huginn line to the Yggdrasil services list. The stale Nightwing embedding text (P2) belongs to the embedding migration's owner. Hands does not rewrite it. Hands reports it.
- **Backup (under Q-H2 A):**
  - add `var/lib/gamecult/huginn` to the tar list;
  - add `huginn.service` to the freeze loop;
  - install and enable `gamecult-authority-backup.timer`, which is operator-run.

**Authority map (interim).**

- Owner: the operator owns deployment and continuity until 14-I. Huginn owns its state.
- Forbidden writers: nothing but the daemon writes `/var/lib/gamecult/huginn`. Nothing but the runbook's release step writes `/opt/gamecult/huginn`.
- Deletion line: 14-I.

**Verification (operator only; Hands does not deploy):**

1. The unit reaches `active`.
2. The daemon's stderr says it is serving `10.77.0.1:17872`.
3. From Starfire, `eureka-state` `whoami` answers `reachable: true`.
4. **Restart preserves state:** admit, run `systemctl restart huginn`, then `view` the document.
5. **Negative:** a `whoami` from Raven (`10.77.0.4`) times out, because ufw admits Starfire only.
6. **Negative:** stop the unit. The next `admit` is `Unavailable` naming the endpoint, and no file appears on Starfire.
7. The collection `huginn_mind_<slug>` exists, with its metadata.
8. **Backup:** run the backup service once. `tar --list` shows `var/lib/gamecult/huginn/minds/<slug>/mind.redb`.

**Estimate:** about 150 lines of ops configuration and prose. +1 unit, +1 ufw rule, +1 port, +1 backup path, and backups resume.

## Cut 15. Skill wiring (under Q-H6 A)

- **Repo:** `GameCult/Eureka`, `main` at `19487f0` (`~/.claude/skills/eureka`). Depends on 13b being registered and 14 being live.
- **The old Cut 15's anchors are stale:** `SKILL.md` is now 533 lines, and "Mind Steward" is now Life. Hands re-takes them. The sections to change:

| Where | Change |
|---|---|
| `SKILL.md:8-19` (the substrate paragraph) | Eureka keeps questions, rulings, cut specs, reports, verdicts, findings, follow-ups and resolutions as typed documents in the instance's mind (Huginn), through the `eureka-state` tools. The committed target and map keep what has no kind. Delete "committed docs … memory files" as the store. |
| §0 (`:58-72`) | Self calls `whoami` first. If the organ is unreachable, stop and say so. Then Self admits `stewardship` (if absent), `campaign` and `target`. |
| §0b and §1 (`:74-165`) | Imagination admits `question`s and `cut_spec`s. The map document keeps body facts, the model page and rationale, and **has no per-cut spec sections**. "Commit the map" applies to that prose only. |
| §2 (`:167-189`) | A ruling is admitted with `answers`, `choice` and `operator_quote`. Supersession is a `resolution`, never an edit. |
| §3 (`:190-300`) | The Hands brief names the `cut_spec` id and the rulings in force. Hands admits a `cut_report`. `cargo-mutants` survivor triage goes into `cut_report.deviations`, or into a follow-up. |
| §4 (`:304-335`) | Soul admits one `verdict` plus its `finding`s. Admitting is a Mind write, not a Body edit, so Soul stays read-only on repos. |
| §5 and §6 (`:337-399`) | Triage outcomes are `resolution`s and `follow_up`s. The status header becomes a set of selection recipes. Self reconciles the ledger from `cut_report.structural_delta`. |
| Self's discipline (`:401-451`) | Never restate typed state in prose. When the organ is unreachable, stop; never keep a second copy (the target's invariant). **Keep "Record the substrate you needed and did not have".** |
| `references/briefs.md` | Each faculty's "read first" becomes selection recipes. Hands gets `view <cut_spec id>`. A new **Rehydrate** section: `whoami`, then the recipes. |
| `references/cut-map.md` | Rewrite it as "Campaign state": the document set, the keys, the resolution matrix, and **selection recipes** in RS-3's vocabulary (below). The prose-map template shrinks to body facts, model page and rationale. |
| `references/changelog.md` | A dated entry with this map's evidence. |

**Selection recipes** to write. These replace the deleted presets, spelled in RS-3's aliases:

- **rulings in force:** `schemas: [epiphany.pipeline.ruling.v2]`, `in_force = true`, `root = <campaign>`;
- **open items:** questions, findings and follow-ups with `in_force = true`;
- **specs with no report:** `cut_spec`s, with the hop `cut_spec` inbound and absent (Hands must confirm that the substrate can express absence; if it cannot, this recipe is two queries and says so);
- **a subject's history:** `resolution`s with `subject` = X, `descending`.

**Verification (Soul, by reading plus one live exercise).**

- `rg -n "status header|WriterLeaseHeld|lease|open_items|rulings_in_force|Mind Steward" ~/.claude/skills/eureka` hits only changelog and history lines.
- Every leaf kind is named by at least one brief.
- No brief asks for prose relay of a typed artifact.
- **Live:** a fresh agent given only the skill and the instance slug runs `whoami` and the rulings-in-force recipe against the live organ, and both return typed answers.

**Estimate:** prose shrinks. `cut-map.md` is replaced, not extended.

## Cut 12. Parked (Q-H5)

Recorded, not built:

- A hand-off is two admissions, one per mind, sender first. Each side's derivation already exists (P8).
- Import waits for a consumer, most likely Epiphany's adoption campaign.
- Old Cut 12's in-process `hand_off(&mut from, &mut to)` is infeasible under one daemon per mind (P7).

## 14-I. Idunn path (blocked; not this week)

Named so that the interim unit has a deletion line. Its dependencies:

1. **Route continuity** (`Idunn/docs/route-continuity-target.md`, invariants 1, 2 and 5, and its Q1 b):
   - Huginn declares no `odin.verse-rendezvous` dependency, so it is a route-proof target.
   - No nginx reload on observation, because every reload breaks RUDP sessions.
   - Stateful deadlines resolve (its Q3).
2. **A shared Rust runtime participant** in `cultnet-rs`: presence signing, activation credential, lease adoption and the challenge answer. Otherwise Huginn writes a third copy (P9). The owner is CultLib.
3. **Idunn has no path from a selected dependency's endpoint to a service argument.** Qdrant and Ollama will be `external-operator-binding` dependencies whose endpoints are duplicated in `argument_bindings`, as Ghostlight duplicates Odin's. Route this to Idunn.
4. **The slot for `minds/<slug>/mind.redb` is `external-file`**, because redb is not Idunn's single-file CultCache format. It is one slot per instance, so the recipe names the instance.
5. Whether the huginn-daemon hub (`serve.rs:138`, which drops other connection ids) receives Idunn's RUDP challenge. Not probed.

The later Verse-presence cut (Q23 A) comes after 14-I.

## Subtraction ledger (estimates; Self reconciles at each landing)

| Cut | Removed | Added | Dependencies, targets, formats |
|---|---|---|---|
| 11a | `NoIndex` from the binary | ~650 | +`ureq`, +`serde_json` (daemon); +1 collection per mind |
| 11b | the unwired refusal and its test | ~350 | none |
| 13a | 223 (moved) | ~300 | +`base64` in `huginn-mind` (moved) |
| 13b | the old design's 3 preset tools | ~400 | +1 binary; +`rmcp`, +`tokio` |
| 12 | the whole cut (parked) | 0 | none |
| 14 | 2 dead freeze entries | ~150 ops | +1 unit, +1 ufw rule, +1 port; backups resume |
| 15 | per-cut prose sections, status headers | prose shrinks | `cut-map.md` replaced |

## What in the old specs (cut map lines 5490-5866) is now wrong

1. **Cut 11's `pending_index` document.** A mind admits exactly fifteen types (`mind.rs:84-88`), and this would be derived state in the truth store. It is replaced by reconciliation by set difference.
2. **Cut 11's "indexing at admission, in the same call".** It blocks the single-threaded serve loop (P7). A cold embed takes 5-6 s (P3).
3. **Cut 11's flags "following `epiphany.service`'s spelling (R16)" and its `10.77.0.1:11435` precedent.** Both endpoints are dead (P2).
4. **The compatibility record ported from voidbot.** Its `embedderId` includes the host URL (P4). Compatibility keys on model and digest instead.
5. **One shared collection with an `instance` payload.** It becomes one collection per mind.
6. **Cut 12's two-mind in-process hand-off.** It is infeasible (P7), and the hand-off already works per mind (P8).
7. **Cut 13's seven tools.** They become four, after the operator deleted `open_items` and `history` as presets.
8. **Cut 13's "tool schemas equal the published schemas".** The published request schema will `$ref` CultLib's `$id` (RS-3 F9). MCP tool schemas must be self-contained.
9. **Cut 13 is missing Deferred handling** (BP-3 §7).
10. **Cut 14's `kind = cultcache-directory`.** No such kind exists (P9).
11. **Cut 14's "`shared-infrastructure` dependency on Qdrant, Ghostlight's shape (`:168-187`)".** Ghostlight declares only Odin, and non-Idunn infrastructure is `external-operator-binding` (P9).
12. **Cut 14's `idunn-huginn` unit name** in the backup freeze loop. Units are named per incarnation, `idunn-<target>-<sha>` (P9).
13. **Cut 14's "Named owner: `gamecult-authority-backup.service`, daily at 03:20 UTC".** It is not installed, and it would refuse to run (P10).
14. **Cut 14's route through nginx `rudp://10.77.0.1:17872` with presence health.** Huginn has no runtime participant, and route continuity is rewriting that path (P9).
15. **Cut 15's anchors, and its "Mind Steward" name.** Its `open_items` plus `query` status header and its rehydration sequence both use deleted tools.
16. **Q14 C** (move the mutation harness into the skill repo). It is moot: the harnesses were retired.
