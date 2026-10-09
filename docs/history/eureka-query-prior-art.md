<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/eureka-query-prior-art.md; text below this line is unchanged. -->
# Prior art for a typed, serializable query surface over a memory organ's read side

Eyes pass, 2026-09-16/17. Facts with evidence pointers. No ranking, no recommendation.

## The thing being measured against

The five capabilities, abbreviated throughout as **C1..C5**:

- **C1** typed predicates over each kind's own fields, expressed as data
- **C2** projection (summary or whole document)
- **C3** ordering plus an opaque cursor
- **C4** one hop of citation in both directions
- **C5** count / existence without bodies

The portability facts, abbreviated **P1..P5**:

- **P1** does the query itself serialize as typed data across runtimes (MessagePack wire, constructible from TS/C#), or is it Rust-only builder calls?
- **P2** does it derive a JSON Schema, or is the schema hand-written?
- **P3** transitive dependency cost; anything dragging in a runtime, network client, DB driver or parser generator
- **P4** does it assume a backing store engine?
- **P5** licence, last release, maintenance signal, single-maintainer?

## The crate this lands in

`F:\Projects\Huginn\crates\huginn-mind`, manifest at `F:\Projects\Huginn\crates\huginn-mind\Cargo.toml:9-19`. Dependencies exactly as stated in the brief: `anyhow`, `chrono 0.4.44`, `cultcache-rs` (CultLib, rev `a0813c6e`), `epiphany-pipeline` (Epiphany, rev `5cda0886`), `rmp-serde 1`, `schemars 1`, `serde 1`, `sha2 0.10`. Dev-only: `serde_json`, `tempfile`. The manifest comment states the constraint in its own words: "No network, no socket, no index: those are the daemon's (Cut 10, Cut 11)."

**The baseline P3 measurement, taken directly.** `F:\Projects\Huginn\Cargo.lock` currently resolves **139 packages** for the whole three-crate workspace. A grep of the lockfile for `tokio`, `hyper`, `reqwest`, `tonic`, `async-std`, `mio`, `prost`, `pest`, `lalrpop` and `nom` returns **NONE** — there is no async runtime, no HTTP client, no gRPC stack and no parser generator anywhere in the tree today. The sibling `huginn-daemon` (`crates/huginn-daemon/Cargo.toml`) gets its socket from `cultnet-rs` and its process lifetime from `signal-hook 0.3`, synchronously. So any candidate pulling a runtime in would be introducing the first one, and the 139 is the number a transitive-dependency count should be read against.

---

# Part I — GameCult-internal candidates

## I-1. The incumbent: `PipelineQuery` in `huginn-mind`

**What it is.** A flat option-struct filter, already built, already on the wire, already schema-published. This is the surface a general one would replace.

**Owner.** `huginn-mind`, this crate. Landed in commit `49cc6e3` "Read a mind: views, queries, open items and history".

**Evidence.**

- `F:\Projects\Huginn\crates\huginn-mind\src\query.rs:77-97` — `PipelineQuery`: `campaign: Option<Slug>`, `repo: Option<OrgRepo>`, `cut: Option<Label>`, `kinds: Vec<PipelineKind>`, `in_force: Option<bool>`, `faculty: Option<Faculty>`, `admitted_after: Option<Short>`, `admitted_before: Option<Short>`, `limit: Option<u32>`, `semantic: Option<SemanticQuery>`. Derives `Serialize, Deserialize, JsonSchema`.
- `query.rs:249-290` — `Reader::matches`, the whole evaluator: a hand-written `if let Some(..)` chain, one branch per field, AND-only.
- `query.rs:30-32` — `QUERY_LIMIT_MAX = 200`.
- `query.rs:307-311` — `ordered()`, a fixed sort by `(admitted_at, id)`; not selectable.
- `query.rs:331-350` — `Mind::query`; `matched` is computed before truncation so a short page is not mistaken for the whole answer.
- `query.rs:110-117` — `PipelineOpenItems`, five named buckets.
- `query.rs:121-125` — `HistoryScope::{Subject, Repo}`.
- `F:\Projects\Huginn\crates\huginn-mind\src\wire.rs:43-50` — `HuginnMindRequest::{Whoami, Admit, View, Query, OpenItems, History}`.
- `wire.rs:188, 203` — round-trips through `rmp_serde::to_vec_named` in test; MessagePack is the live encoding.
- `F:\Projects\Huginn\crates\huginn-daemon\src\envelope.rs:39, 84, 144-145` — on the live wire the payload is **`rmp_serde::to_vec_named` then base64** inside a CultNet operation envelope, dispatched by schema id (`MIND_RESPONSE_SCHEMA` vs `FAILURE_SCHEMA`). Base64 is a 4/3 size multiplier on whatever the query serializes to, which is a fact about any encoding that nests deeply.
- `wire.rs:31-34, 210-221` — the published JSON Schemas are `include_str!`'d from `schemas/cultnet/huginn.mind_request.v1.schema.json` and pinned byte-for-byte against `schemars::schema_for!` by the test `published_wire_schemas_match_derivation`, with `HUGINN_WRITE_SCHEMAS` as the regeneration path. The request schema currently carries 59 `$defs`.

**Portability facts.** P1: yes, it is already typed data on a MessagePack wire. P2: yes, `schemars`, derived and pinned. P3: zero added. P4: no; it runs over `Docs::from_image(mind.envelopes())`. P5: ours.

**Coverage.**

- C1: **partial and cross-cutting only.** Every filter is a field shared across kinds or derived from admission. There is no way to express "severity = High" or "outcome = Withdrawn" — the brief's own example. `severity`, `confidence`, `origin` (`PipelineFinding`), `sequence`/`outcome` (`PipelineResolution`) are unreachable.
- C2: **no.** `PipelineQueryPage.items` is always `Vec<PipelineDocumentView>` and a view carries the whole `PipelineDocument` (`query.rs:57-63`).
- C3: **no cursor, fixed order.** `limit` truncates; `matched` tells you it truncated. A page is a truncation, exactly what the brief says it must stop being.
- C4: **no, but two hops are hardcoded.** `open_items` (`query.rs:366-389`) walks `CutReport.cut_spec` and `Verdict.cut_report` by hand to derive `specs_without_report` and `reports_without_verdict`. `Reader::base` (`query.rs:237-246`) walks resolution→subject so a filter reads through a resolution.
  The derivation layer beneath it (`F:\Projects\Huginn\crates\huginn-mind\src\docs.rs`) is entirely kind-specific and entirely `pub(crate)`: `find` (`:71`), `of_kind` (`:75`), `resolutions_of` (`:97`), `assignments_of` (`:104`), `stewardships_of` (`:194`), `closing_resolution` (`:186`), `later_in_force` (`:212`), `stewardship_of` (`:225`). There is **no generic "documents citing X" and no generic "documents cited by X"** anywhere in the crate; each traversal that exists was written for one pair of kinds.
- C5: **no.** `matched: u32` is the only count, and it is a by-product of a query that already materialized every body.

**Three places the incumbent reaches into key strings because there is no structured alternative** — worth recording as pressure evidence:

- `query.rs:169-174` `root_and_local` splits `<root>:<kind>:<local>` by position.
- `query.rs:262` matches a cut by `format!("cut-{}.", cut.0)` prefix on the local segment.
- `query.rs:178-195` `repo_matches` is a hand-written match over 13 document variants to answer "does this document carry this repo".

**The semantic hole is already declared.** `query.rs:65-72` and `331-336`: `SemanticQuery { text, top_k }` is accepted by the type and refused typed — "Cut 11 also owns which fields of each kind are text, so no substring filter lives here to become a second answer to that question."

## I-2. `epiphany-pipeline` — the document leaf, and how its references are shaped

**What it is.** The crate that owns document shape, bounds, formats and keys. Not a query surface; it is the schema C1 and C4 would have to be expressed over.

**Owner.** Epiphany, `F:\Projects\Epiphany\epiphany-pipeline\src\lib.rs`, 2444 lines, single file.

**The load-bearing fact for C4: references are not uniformly typed.**

- `lib.rs:400-437` — `PipelineRef { kind: PipelineKind, id: Short }` with `validate_ref()`. This is the typed reference.
- But most citations are bare `Short`, kind implied by field name:
  - `lib.rs:349` `PipelineCutReport { cut_spec: Short, forks: Vec<Short>[8], .. }`
  - `lib.rs:356` `PipelineVerdict { cut_report: Short, .. }`
  - `lib.rs:357` `PipelineFinding { verdict: Short, .. }`
  - `lib.rs:342` `PipelineCutSpec { depends_on: Vec<Short>[8], rulings: Vec<Short>[32], questions: Vec<Short>[16] }`
  - `lib.rs:324` `VerdictClaim { findings: Vec<Short>[16], mutations: Vec<Label>[8], .. }`
- Typed `PipelineRef` appears in only three places: `PipelineQuestion.raised_in: Option<PipelineRef>` (`lib.rs:334`), `PipelineFollowUp.source: PipelineRef` (`lib.rs:362`), `PipelineResolution.subject: PipelineRef` (`lib.rs:374`).
- `ForeignRef` (`lib.rs:286`) is the cross-repo reference: `{ repo, commit, kind, id, payload_sha256 }` — typed, but names another repo.
- `DocRef` (`lib.rs:287`) points at a file range, not a document.

There is **no generic "references out of this document" enumerator** in the leaf. `grep` finds 137 occurrences of `PipelineRef` and no `fn refs`, `fn references` or `fn cites`. A generic C4 hop must therefore either carry a per-field kind table or the leaf must grow a reference enumerator.

**Per-kind predicate fields C1 would need to reach** (`lib.rs:357` and around): `PipelineFinding { confidence: FindingConfidence, severity: FindingSeverity, origin: FindingOrigin, invariants: Vec<Label>[8] }`; `PipelineResolution { sequence: u32, outcome: ResolutionOutcome }` (`lib.rs:374`); `PipelineStewardship { instance, repo, sequence, assigned_on }` (`lib.rs:388`). Every bound is declared in the macro (`[8]`, `[16]`, `[64]`), so field cardinality is already known statically.

**Portability facts.** P1/P2: the documents already serialize and derive `JsonSchema`, same `schemars`. P3: it is already a dependency. P4: no. P5: ours.

**Coverage.** Owns none of C1..C5; it is the type universe they range over.

## I-3. CultCache (`cultcache-rs`) — no query concept at all

**What it is.** The store. Envelope persistence, compare-exchange, snapshot pull.

**Owner.** CultLib, `F:\Projects\CultLib\packages\cultcache-rs\src\lib.rs`, single file.

**Evidence.** The entire public surface is CAS/batch/snapshot: `compare_and_swap_entry` (`:412`), `insert_entry_if_absent` (`:437`), `compare_and_swap_batch` (`:456`), `append_if_snapshot_unchanged` (`:510`), `delete_batch_if_unchanged` (`:543`), `replace_and_append_if_snapshot_unchanged` (`:571`), `pull_all_read_only_snapshot` (`:674`), `compare_exchange` (`:728`), `register_document_type` (`:1952`), `pull_all_backing_stores` (`:2022`). There is **no** `fn query`, `fn filter`, `fn find`, `fn select` or `fn where` anywhere in the file.

**One correction to "no selection at all": the C# reference has two single-value index lookups, and the Rust runtime has neither.**

- `F:\Projects\CultLib\src\GameCult.Caching\CultCache.cs:1495-1509` — `GetByName<T>(string name)` and `GetByIndex<T>(string alias, string value)`. Both funnel through `Single<T>(...)`, so each returns **at most one document**: these are unique-index lookups, not multi-result selection, and there is no range, no set membership, no composition.
- Consumers are all in the C# tree: `src/GameCult.Mesh/CultMesh.cs:2351, 2433, 3330`, `src/GameCult.Networking/CultNetDatabase.cs:794-797, 1105-1106`.
- `grep` for `get_by_index` / `by_index` / `index_alias` in `packages/cultcache-rs/src/lib.rs` returns **nothing**. The runtime `huginn-mind` actually depends on has no index concept at all.

A repo-wide search for declared filter/predicate/selector types across `CultLib`, `Epiphany`, `Eve`, `Odin`, `Norn` and `Mimir` sources (`grep -rliE "(struct|class|interface|enum) +[A-Za-z]*(Filter|Predicate|Selector|Criterion|Criteria)"`) returns **zero hits in CultLib, Odin and Norn**; the Eve hits are all `node_modules`; Mimir's two are a signal-processing matched filter and a transport selector, unrelated.

**Coverage.** None of C1..C5. Read is "give me every envelope"; selection is the caller's loop. P4 is the notable one: CultCache is exactly the in-memory-image shape the brief describes, so a candidate that needs a store engine is a different kind of thing.

## I-4. CultNet — one selection shape on the wire, and it is two id allowlists

**What it is.** `CultNetMessage::SnapshotRequest`, the only CultNet message that narrows what comes back.

**Owner.** CultLib, `F:\Projects\CultLib\packages\cultnet-rs\src\snapshot_query.rs`.

**Evidence.**

- `snapshot_query.rs:160-163` — `CultNetRawSnapshotQuery { message_id: String, expectations: BTreeMap<(String, String), CultNetSnapshotSourceExpectation> }`.
- `snapshot_query.rs:197-217` — `request()` lowers those expectations to `CultNetMessage::SnapshotRequest { message_id, schema_ids: Option<Vec<String>>, record_keys: Option<Vec<String>> }`. Note it lowers to the *cross product* of schema ids and record keys, then re-validates the response against the exact expectations in `accept_response` (`:220-289`).
- `snapshot_query.rs:30-56` — `CultNetReadOnlySnapshotPolicy` is an explicit `(schema_id, record_key)` allowlist, not a predicate.

**Portability facts.** P1: `SnapshotRequest` crosses the wire, so yes, but it carries no predicates to serialize. P2: not `schemars`-derived here. P3: in-substrate. P4: no. P5: ours.

**Coverage.** C1 no (identity only, no fields). C2 no. C3 no. C4 no. C5 no. This is a fetch-by-id list, not a query.

## I-5. CultMesh — a query *surface*, with the query shape left to the caller

**What it is.** The generic plumbing for "a parameterised, watchable read". It deliberately does not define what a query is.

**Owner.** CultLib. C# reference `F:\Projects\CultLib\src\GameCult.Mesh\CultMeshPrimitives.cs`; TS runtime `F:\Projects\CultLib\packages\cultmesh-ts\src\index.ts`.

**Evidence.**

- `CultMeshPrimitives.cs:628` — `CultMeshQuerySurface<TParameters, TResult>`; `:706` `CultMeshBoundQuerySurface<TParameters, TResult>`. `TParameters` is entirely caller-defined. The TS mirror is `packages/cultmesh-ts/src/index.ts:~643-730`.
- `CultMeshPrimitives.cs:876-881` — `CultMeshDocumentQueryParameters` is an **empty struct** with a single `Empty` singleton. The document-handle case has no parameters at all.
- `CultMeshPrimitives.cs:487` `CultMeshQueryContext` and `:593` its builder — this is *routing and locality* context (verse, route hint), not selection.
- `F:\Projects\CultLib\src\GameCult.Mesh\CultMeshDiscoveryService.cs:34-57` — `CultMeshDiscoveryQuery { EndpointId, VerseIds, AuthorityRuntimeId }`. A three-field discovery filter over Verses, not over documents.

**Coverage.** CultMesh supplies the *slot* a query document would sit in (`TParameters`) and nothing that fills it. None of C1..C5 are answered; the type parameter is the evidence that the question was left open.

## I-6. Eve DSL — binding, and an explicit refusal to own selection

**What it is.** A composition language for interface projection. It binds components to documents and fields.

**Owner.** Eve, `F:\Projects\Eve`.

**Evidence.**

- `F:\Projects\Eve\docs\surface-contract-v1.md:66-71` — the binding primitives: `var` (one live value), `collection` (event-shaped ordered values), `derive` (computed fields such as counts and latest events), `bind` (component props subscribe to a var/collection/derived field).
- `surface-contract-v1.md:156-158` — a component names `documentId`, `schemaId`, `slotId`, `presentationKind`, `routeHint`. Then, verbatim: "Provider adapters or advertised plugins own schema-specific query selection and decoding. Renderer code must not switch on provider schema ids or call provider-specific query methods to resolve a slot."
- Selection that does exist in Eve is presentation-only and enumerated, not general. `F:\Projects\Epiphany\epiphany-core\src\atlas\eve_surface.rs:65-93` — `AtlasEveAttentionFilter` is a five-variant enum (`NeedsAttention`, `Degraded`, `Disputed`, `Stale`, `All`) lowered to a `control.select` with a fixed option list (`eve_surface.rs:~548-574`) and an advertised command marked `presentation_only: true, domain_state_effects: "none"` (`eve_surface.rs:~1146-1180`).

**Coverage.** Eve binds to a subset someone else selected. `derive` covers counts (C5-adjacent) at the presentation layer only. C1..C4 are explicitly out of scope by the surface contract's own sentence.

## I-7. Odin — two structs named Query, neither a predicate language

**Owner.** Odin, `F:\Projects\Odin`.

**Evidence.**

- `F:\Projects\Odin\crates\odin-daemon\src\lib.rs:324-328` — `OdinStoreQuery { incarnation: IncarnationRef, provider_signer_identity_id: String, dependencies: Vec<IncarnationRef> }`. A by-identity read; `OdinTopologyStore::read` (`:339`) returns one `OdinStoreSnapshot`. No derives — not serializable as written.
- `F:\Projects\Odin\crates\odin-core\src\discovery.rs:6-11` — `OdinEndpointQuery<'a> { schema: Option<&'a str>, transport_contains: Option<&'a str>, host_hint: Option<&'a str>, device_filter: Option<&'a str> }`. Four optional string filters, one of them a substring match. **Borrowed lifetimes and no serde derives**: this cannot cross a wire as written. `discover_provider_endpoints` (`:22`) loops `cache().get_all::<EveProviderAdvertisementRecord>()` and filters in Rust.

**Coverage.** `OdinEndpointQuery` is the same flat-option-struct shape as `PipelineQuery` (I-1), arrived at independently, over a different type. Neither covers C1..C5.

## I-8. VoidBot — the live Qdrant filter translation, and a second flat option struct

**What it is.** The only place in the substrate that builds a Qdrant filter document today. Directly relevant because Cut 11 depends on Qdrant.

**Owner.** VoidBot, `F:\Projects\VoidBot`.

**Evidence.**

- `F:\Projects\VoidBot\packages\shared\src\index.ts:114-124` — `RetrievalFilters { corpusKind?: "discord_history" | "repository_source" | "persona_memory"; guildId?; channelId?; authorId?; identityId?; repoName?; pathPrefix?; language?; sourceId? }`. Nine optional strings, AND-only, no ordering, no cursor.
- `F:\Projects\VoidBot\packages\rag\src\qdrant-vector-store.ts:21` — `type QdrantFilter = Schemas["Filter"]`, i.e. the types come from the Qdrant client's own generated OpenAPI schema types, not hand-written.
- `qdrant-vector-store.ts:543-563` — `toQdrantFilter`: builds `const must: Schemas["Condition"][]`, appends one exact match per set field, returns `must.length > 0 ? { must } : undefined`. **`should` and `must_not` are never used.** The full Qdrant boolean model is available and only the AND arm is exercised.
- `qdrant-vector-store.ts:566-584` — `buildMatchAnyFilter(field, values)` produces a `match: { any: [...] }` condition, used only for delete-by-sourceId batches (`:119-125`).
- `qdrant-vector-store.ts:129-136` `deleteByFilters`, `:142-162` search with `filter: toQdrantFilter(filters)`.
- Consumers: `packages/rag/src/{file-vector-store,in-memory-vector-store,message-archive,retrieval-service,source-vector-store}.ts` all import `RetrievalFilters` — so the same nine-field struct is the filter shape for the in-memory store, the file store and Qdrant alike.
- The MCP tool surface exposes a hand-written subset of the same fields: `packages/providers/src/local-llm-tools.ts:79-115` (`search_sources` with `repoName`, `pathPrefix`, `language`, `limit`).

**This is the strongest in-substrate evidence of the pressure the brief names.** Two independent flat option-structs (`PipelineQuery`, `RetrievalFilters`) plus a third (`OdinEndpointQuery`) exist because no general one does, and `RetrievalFilters` additionally flattens Qdrant's boolean model down to `must`-only on the way through.

## I-9. Huginn's own README and AGENTS — authority boundary

Recorded because it bounds where a query surface may live. `F:\Projects\Huginn\README.md:14-24` and `AGENTS.md:26-40`: Huginn owns `.cc` inspection projection and Eve DSL emission for `cultcache.huginn.inspector`; it does **not** own CultCache persistence. Note the AGENTS file still names `E:\Projects\Huginn` as project root while the live tree is `F:\Projects\Huginn` — stale, flagged, not part of this question.

---

# Part II — Prior attempts and abandonments in our own history

## II-1. `ThreadEpiphanyGraphQuery` — a serializable citation-hop query, built and deleted

**What it was.** The closest thing the substrate has ever had to C4 as data.

**Evidence.**

- Added: Epiphany commit `5c62a650` "Add Epiphany graph query surface", 2026-04-29.
- Shape, from the generated TS at `vendor/codex/codex-rs/app-server-protocol/schema/typescript/v2/ThreadEpiphanyGraphQuery.ts` in that commit:
  `{ kind: ThreadEpiphanyGraphQueryKind, nodeIds?: string[], edgeIds?: string[], paths?: string[], symbols?: string[], edgeKinds?: string[], direction?: ThreadEpiphanyGraphQueryDirection | null, depth?: number | null, includeLinks?: boolean | null }`
- `ThreadEpiphanyGraphQueryKind = "node" | "path" | "frontierNeighborhood" | "neighbors"`.
- `ThreadEpiphanyGraphQueryDirection = "incoming" | "outgoing" | "both"` — **both directions of the hop, as data**.
- Doc comments in the generated file: direction "Defaults to `both`"; depth "Defaults to 1 and is capped by the server"; includeLinks "Defaults to true so neighborhood queries preserve dataflow/architecture joins".
- Schema was **derived, not hand-written**: `// This file was generated by [ts-rs]`. The same commit added `codex_app_server_protocol.schemas.json` and `.v2.schemas.json` (138 lines each) and `ClientRequest.json` (85 lines).
- Deleted: Epiphany commit `5f6f2441` "Remove Epiphany app-server compatibility surface", 2026-07-12. Nothing named `*GraphQuery*` survives in the tree today (`find` returns nothing outside `node_modules`).

**What happened to it.** It was not removed for being a bad query shape. It went out with the whole app-server compatibility surface it lived in — it was a Codex protocol type in `vendor/`, carried on ts-rs and JSON Schema generation, and died with its host. Related surviving history: `2ea57376` "Add local Verse context query", `e2a8b479` "Read graph query from memory graph store", `fa222584` "Move retrieval query law to core", `0cf4b081` "Move graph query smoke to Rust", `bf433d63` "Make semantic readiness proof query-owned".

**Coverage it had.** C1 partial (id/kind sets, not per-kind field predicates), C2 via `includeLinks` only, C3 no cursor but a server-capped depth, **C4 yes — direction and depth as data**, C5 no.

## II-1b. Selective hydration, deleted from CultCache three days ago

**What it was.** A filter applied at the store layer so a pull could hydrate a subset rather than the whole image.

**Evidence.** CultLib commit `900e148d8ad6dceed4b9c14dac23d12a869cb539`, "Read only v4 directory stores; delete probes and selective hydration", 2026-09-13. Its own message: "DirectoryMessagePackBackingStore loses **HydrationFilter**, FlushStageProbe, ReadStageProbe and their stage constants, ReadPersistedGeneration, **PullSelected**, and every legacy path ... **CacheBackingStore.PullSelected** and CultPersistedRecordMetadata go with them."

**What happened to it.** It went out with the v1/v2/v3 store formats in a format-narrowing cut, not as a judgement on filtering. But the effect stands: as of three days before this pass, CultCache's backing-store surface has **no way to pull a subset**. `pull_all_read_only_snapshot` and `pull_all_backing_stores` are what remain. Any query surface above it reads the whole image, which is what `huginn-mind` does (`query.rs:207-208`, `Docs::from_image(mind.envelopes())` per call).

**Also worth recording:** every other CultLib commit matching query/filter/predicate/selector is about **schema-alias or shard-membership resolution**, not field predicates — e.g. `54543b5` "Resolve Python shard discovery filters by schema alias", `1b9140d` "Resolve C# subscription filters by schema alias", `2738f4c` "Resolve TS snapshot filters by schema alias", `7ad74cf` "Filter Python snapshots by shard membership". "Filter" in CultLib means "which records by schema and shard", never "which documents by field value".

## II-2. The RethinkDB lineage — what the archive actually says

The operator's account is **confirmed as to adoption and abandonment**, and **not confirmed as to ReQL or changefeeds specifically**.

**Adoption, with the stated reasons.** Discord `#development`, 2020-03-15:

- `688549686373515270` (00:50): "I've decided [Tympanic] is right, and abusing git for document storage is a bad strategy. I spent a couple days researching a bunch of different db solutions, I feel like I've looked at every one of them."
- `688550094030503985` (00:52), the anchor: settled on RethinkDB for four named properties — it stores JSON, supports queries, has a realtime push feature like Firebase realtime db, and has a C# client library, "which even Firebase bafflingly lacks".
- `688550271554682890`: "db is now live at asgard.gamecult.games:28015" (28015 is RethinkDB's client driver port).

**The alternative that was considered and passed over.** 2020-03-11, `#development`: Tympanic `687397982638833793` "What would be a reason to not just go straight to something like SQL? It all depends on how much data we expect to have." Metacrat `687400730948272258` "Maybe you're right about SQL though. I had to use it for my database class and didn't particularly enjoy it, but it's battle tested"; `687411066522697873` "I kinda feel like saying let's cross that bridge when we get to it, migrate to SQL if document store + git ends up being a bottleneck".

**Abandonment.** 2020-2021 RethinkDB was the real production database — editor tooling wrote to it (`691363946623402014`, 2020-03-22), the game would not run without it (`716084893888806983`, 2020-05-30). Then 2021-06-23, `857099401398845442`: "Pretty big change in how we're handling data here. From now on, the Rethink database is obsolete. We can still go back to it, but for now I've configured it to use JSON files when present and fall back to entries defined in a msgpack file", followed immediately by `857099432168128553` "this will make modding trivial".

**What that message is.** It is the CultCache origin: files plus MessagePack, chosen for moddability, replacing a server. Also `1023147757860565012` (2022-09-24) "No really take a look at rethinkdb" — still recommending it a year after dropping it.

**What the archive does not contain.** Searches of the indexed Discord history for ReQL, for changefeeds, and for query/filter-language design return **no hit naming ReQL or changefeeds at all**. The realtime feature is referenced only obliquely and by analogy — "a realtime push feature like Firebase realtime db". So:

- The inheritance the archive *documents* is: document store, JSON/schemaless records, a real client library, and realtime push. Push/watch is the dimension that visibly survives — CultMesh's `watch`/`observe` on a document handle (`CultMeshPrimitives.cs:~2010`, `packages/cultmesh-ts/src/index.ts:~886`) and `CultMeshPollingWatchOptions` (`CultMeshSnapshots.cs:~558`).
- The inheritance the archive **does not** document is the query side. No term tree, no serialized predicate, no filter document, no selector exists anywhere in CultCache or CultNet (see I-3, I-4). CultNet's only wire selection is two id allowlists.
- Stated plainly: on the evidence available, **the borrowing was in change notification and document-shaped storage, not in querying.** Whether that was deliberate or simply never reached is not answerable from the archive; see the open questions.

One adjacent scar worth recording, since it is about query complexity in our own code: `1510998723730346114` (2026-06-01), on the Aetheria codebase — a TODO cursing a LINQ query nested ten levels deep.

---

# Part III — External prior art

## III-1. RethinkDB ReQL wire encoding (named prior art)

**What it is.** A query represented not as text but as a tree of typed terms, serialized as nested arrays. Each language driver is a builder over that one wire format.

**Evidence.** `rethinkdb/rethinkdb`, `src/rdb_protocol/ql2.proto`, branch `next` at commit `a2cf1308b57dca2e05d744f225a082881bab3fca` (2026-03-28), 862 lines — https://github.com/rethinkdb/rethinkdb/blob/next/src/rdb_protocol/ql2.proto . Driver-authoring spec: https://rethinkdb.com/docs/writing-drivers/ . Python driver `ast.py`: https://github.com/rethinkdb/rethinkdb-python/blob/master/rethinkdb/ast.py

**The encoding, confirmed with one correction.**

- A term serializes as `[termType, [args...], {optargs}]`, and **the optargs object is omitted entirely when empty** — `ast.py:164-168`, `res = [self.term_type, self._args]; if len(self.optargs) > 0: res.append(self.optargs)`. So a term is a 2- or 3-element array, not always 3.
- Leaf data is not wrapped; a datum is bare JSON. Documented example: `r.db("blog").table("users")` → `[15, [[14, ["blog"]], "users"]]`, where 15 = TABLE and 14 = DB.
- Outer frame: `[QueryType, term, globalOptargs]`. `r.expr("foo")` → `[1,"foo",{}]`. `Query.QueryType`: START=1, CONTINUE=2, STOP=3, NOREPLY_WAIT=4, SERVER_INFO=5.
- Protobuf-era messages: `Query { type=1, query(Term)=2, token=3, OBSOLETE_noreply=4, accepts_r_json=5, global_optargs(repeated AssocPair)=6 }`; `Term { type=1, datum=2, args(repeated Term)=3, optargs(repeated AssocPair)=4 }`; `Datum.DatumType` = R_NULL, R_BOOL, R_NUM, R_STR, R_ARRAY, R_OBJECT, R_JSON.
- Framing: 8-byte LE token, 4-byte LE length, UTF-8 JSON. Response is token, length, then `{t, r, b, p, n}` = ResponseType, result, backtrace, profile, notes.

**Term types: closed enum, stable ids, never recycled.**

- `Term.TermType` holds **186 named values**; highest id is `BIT_SAR = 196`.
- Retired ops are left as comments rather than reused: `// OBSOLETE_GROUPED_MAPREDUCE = 46;`, `// OBSOLETE_GROUPBY = 47;` (ql2.proto:452-453). Superseded ops keep their slot: `BETWEEN_DEPRECATED = 36;`. Ids are non-contiguous by design (`HTTP = 153`, `UUID = 169` sit among low-numbered ops), which is only workable because the enum is a fixed registry.

**Protocol versions.** `VersionDummy.Version`: V0_1 `0x3f61ba36`, V0_2 `0x723081e1`, V0_3 `0x5f75e83e`, V0_4 `0x400c2d20`, V1_0 `0x34c2bdc3`. `VersionDummy.Protocol`: PROTOBUF `0x271ffc41`, JSON `0x7e6970c7`. Originally protobuf-framed (ql2.proto:5-31: send version magic as LE32, auth-key length and key, then a length-prefixed serialized `Query` blob). The protocol-type selector arrives at **V0_3**, which is where JSON-over-socket becomes selectable. **V0_4** keeps the same handshake shape and adds parallel query execution. **V1_0** replaces the handshake with SCRAM-SHA-256 (RFC 7677) exchanged as NUL-terminated JSON objects, and protobuf is no longer a choice.

**P2 — was a schema published for the query document? No.** No JSON Schema or other machine-readable schema for the query document was ever published. The driver-authoring spec describes the array encoding in prose plus worked examples and points implementers at `ql2.proto` for the integer constants; third-party drivers (Go `rethinkdb-go/ql2/ql2.pb.go`, Deno, Clojure `rethinkdb-protobuf`) each vendor a copy of the `.proto` and code-generate from it. The `.proto` describes the *protobuf* framing; for the JSON protocol the term-array shape is documented prose only.

**What drivers owned vs. the wire.** Drivers were builders, not query engines. Driver: fluent API → term tree; 8-byte token allocation; length framing; handshake/SCRAM; JSON serialization; response demux by token; cursor and `CONTINUE` batching; pseudotype decoding on the way back (times, binary, geometry); variable-id allocation for FUNC. Server: all query semantics, type checking, arity checking, optarg validation, and backtraces. Errors return `RUNTIME_ERROR`/`COMPILE_ERROR` with a `b` backtrace of frames indexing into the term tree the driver sent; the driver renders that backtrace against its own tree (`compose()` in `ast.py`) but does **not** pre-validate. Structurally confirmed: `ast.py` is ~500 near-empty subclasses carrying only `term_type = P_TERM.X`.

**Function / lambda terms — the part that makes the shape unbounded.**

Term ids: `FUNC = 69`, `VAR = 10`, `IMPLICIT_VAR = 13`, `MAKE_ARRAY = 2`, `FUNCALL = 64`, `JAVASCRIPT = 11`.

- FUNC signature comment: `FUNC = 69; // ARRAY, Top -> ARRAY -> Top`. Proto text (ql2.proto:605-644), verbatim: "An anonymous function. Takes an array of numbers representing variables (see [VAR] above), and a [Term] to execute with those in scope."
- VAR (ql2.proto:295-303), verbatim: "It's the responsibility of the client to translate from their local representation of a variable to a unique _non-negative_ integer for that variable. (We do it this way instead of letting clients provide variable names as strings to discourage variable-capturing client libraries, and because it's more efficient on the wire.)"
- Encoding shape: `[69, [[2, [varId...]], body]]` — arg 0 is a MAKE_ARRAY of integer variable ids, arg 1 is the body term. A reference to parameter *n* is `[10, [varId]]`.
- How a client closure is captured (`ast.py:1958-1980`): the driver reads the lambda's arity by reflection, mints a fresh globally-monotonic integer per parameter, builds `Var` placeholders, **calls the client lambda once at build time with those placeholders**, and records the term tree that the lambda's operator overloading produces. The lambda body is never inspected, compiled or transmitted; only the residue of ReQL operations survives. Host-language control flow inside the lambda runs on the client at build time and leaves no trace.
- `r.row` is a bare `[13,[]]`. `func_wrap` (`ast.py:1951-1955`) scans a built term for an IMPLICIT_VAR and retro-wraps it as `Func(lambda x: val)`. That is why `r.row` does not work in subqueries (https://rethinkdb.com/api/python/row/) — with nesting there is no way to say which implicit scope is meant. The Java driver dropped `r.row` entirely.
- **Implication for describing a query's shape in advance:** the term tree is a general expression tree, not a bounded predicate grammar. FUNC introduces lambda abstraction with integer binders, FUNCALL(64) applies them, BRANCH(65) is a conditional, FOR_EACH(68) iterates, and functions are first-class (HTTP's `page` optarg takes `FUNC | STRING`). Terms recurse with no depth or arity bound, and the "type system" is the doc comments (`Sequence, Function(1) -> OBJECT`), not anything enforceable. A query can be validated in advance only as far as "well-formed nested array whose head integer is a member of a 186-entry enum"; arity and type conformance are decided by the server at compile time. **The enum is closed and bounded; what can be built out of it is not.**
- **Arbitrary client code: no.** The wire carries no client bytecode, source or serialized closure. The single escape hatch is `JAVASCRIPT = 11` — a JS string written by the developer, evaluated server-side in RethinkDB's embedded V8, default timeout 5s (https://rethinkdb.com/api/python/js/). Its second signature returns `Function(*)`, so a JS string can stand where a FUNC is expected: the one place the term grammar hands off to an opaque body.

**P1/P3/P4/P5.** P1: the query is pure data by construction — the drivers exist precisely because the query is the wire format, which is the property being looked for. P3: nothing to depend on; there is no reusable Rust library of the *encoding* separate from a client. P4: yes, unambiguously a front end for a database engine — the server owns every semantic. P5: Apache 2.0 (`LICENSE` at `rethinkdb@next`; GitHub reports `spdx_id: NOASSERTION` due to third-party notices). RethinkDB Inc. shut down 2016; CNCF bought the copyright and assets for $25,000, relicensed from AGPL to Apache 2.0 and contributed it to the Linux Foundation, announced 2017-02-06 (https://www.cncf.io/blog/2017/02/06/cncf-purchases-rethinkdb-source-code-contributes-linux-foundation-apache-license/); community-maintained under the `rethinkdb` org since, not a CNCF hosted project. Last server release **v2.4.4, 2023-12-11** (prior v2.4.3 2023-02-05, v2.4.2 2022-05-07). Repo not archived, 26,998 stars, 1,352 open issues, last default-branch commit `a2cf1308` 2026-03-28. Release cadence roughly one per one-to-two years.

**Rust drivers.** `reql` — crates.io max 0.11.2, published 2023-07-24, 96,744 all-time downloads, 886 in 90 days, MIT/Apache-2.0, repo `rethinkdb/rethinkdb-rs`, last push 2023-07-24, 213 stars, dormant ~3 years. `unreql` — a `reql` fork, max 0.2.1 published 2026-02-10, 13,437 all-time, 1,589 in 90 days, MIT, `https://github.com/vettich/un-rethinkdb-rs`; currently the only Rust driver with recent releases and its 90-day downloads now exceed `reql`'s. Also `rethinkdb-rs/thinker` (superseded, "Use the `reql` crate instead") and `mobc-reql` 0.6.4 (pool wrapper).

**Coverage of C1..C5.** C1 yes and then some — per-field predicates are just terms, but unbounded. C2 yes (`pluck`, `without`, `MAKE_OBJ`). C3 yes — ordering as terms, and cursors as protocol-level `CONTINUE` on a token rather than as a value in the query. C4 yes as general traversal (`eq_join`, `get_all` with an index), nothing citation-specific. C5 yes (`count`, `contains`).

## III-2. Qdrant's filter model

Recorded in detail because Cut 11 depends on Qdrant, and because VoidBot already builds these documents (I-8).

**REST JSON shape.** Top level `Filter`: `must`, `should`, `must_not` (each a `Condition` or an array of them), plus `min_should: {conditions: [...], min_count: uint>=1}`. Recursive — a `Condition` may itself be a `Filter`. `Condition` variants: `FieldCondition`, `IsEmptyCondition`, `IsNullCondition`, `HasIdCondition`, `HasVectorCondition`, `SliceCondition`, `NestedCondition`, `Filter`. `FieldCondition = {key, match?, range?, geo_bounding_box?, geo_radius?, geo_polygon?, values_count?, datetime_range?}`. `Match` variants: `{value}`, `{any:[...]}`, `{except:[...]}`, `{text}`, `{text_any}`, `{phrase}`, `{prefix}`. `Range = {lt,gt,gte,lte}` (floats; RFC-3339 strings for datetime). Nested keys use dot paths with array projection: `"country.cities[].population"`.

```json
{
  "must": [
    { "key": "city", "match": { "value": "London" } },
    { "key": "price", "range": { "gte": 100.0, "lte": 450.0 } },
    { "nested": { "key": "diet", "filter": {
        "must": [ { "key": "food", "match": { "value": "meat" } },
                  { "key": "likes", "match": { "value": true } } ] } } }
  ],
  "must_not": [ { "key": "color", "match": { "any": ["red","black"] } } ],
  "should":   [ { "key": "location", "geo_radius": {
                    "center": { "lon": 13.403683, "lat": 52.520711 }, "radius": 1000.0 } } ],
  "min_should": { "conditions": [ { "is_empty": { "key": "reports" } } ], "min_count": 1 }
}
```

**gRPC/protobuf shape**, `qdrant/qdrant` `lib/api/src/grpc/proto/qdrant_common.proto`, Apache-2.0: `Filter { repeated Condition should=1, must=2, must_not=3; optional MinShould min_should=4 }`; `Condition` is a `oneof` of 8 arms; `FieldCondition { string key=1; Match match=2; Range range=3; ... optional bool is_empty=9, is_null=10 }`; `Match` is a `oneof` of 11 scalar-typed arms (`keyword`, `integer`, `boolean`, `text`, `keywords`, `integers`, `except_integers`, `except_keywords`, `phrase`, `text_any`, `prefix`).

**The two shapes are not isomorphic.** gRPC flattens `Match` into scalar-typed oneof arms; REST uses one polymorphic `value`. A single internal shape cannot be both without choosing.

**P2 — yes, a schema is published.** `https://github.com/qdrant/qdrant/blob/master/docs/redoc/master/openapi.json`, OpenAPI 3.0.1, 404 component schemas, every filter type present and verified (`Filter`, `Condition`, `FieldCondition`, `Match`, `Range`, `DatetimeRange`, `GeoBoundingBox`, `GeoRadius`, `GeoPolygon`, `ValuesCount`, `IsEmptyCondition`, `IsNullCondition`, `HasIdCondition`, `HasVectorCondition`, `NestedCondition`, `MinShould`, `SliceCondition`). It is **generated from `schemars` derives** on `lib/segment/src/types.rs`, where `Filter` carries `Deserialize, Serialize, JsonSchema, Validate`. Same generator we use.

**Structural hazard, verified.** `Condition` and `Match` are `#[serde(untagged)]` / OpenAPI `anyOf` **with no discriminator** — variants are told apart by structure alone; `Filter`'s `must`/`should`/`must_not` additionally use a one-or-array codec. Untagged serde enums force `deserialize_any`, and `rmp-serde` has an open reported failure there: msgpack-rust issue #148, untagged enums containing enums fail under rmp-serde while working under serde_json. This is a fact about copying that shape onto a MessagePack wire.

**`qdrant-client` (P1/P3/P4/P5).** P1: **protobuf, not serde.** `Filter` implements `prost::Message`, `Clone`, `PartialEq`, `Debug`, `Default` — **no `Serialize`/`Deserialize`**. The crate's `serde` feature is misleading: `src/serde_impl.rs` and `src/serde_deser.rs` cover payload `Value`/`Struct`/`ListValue` only, never `Filter`/`Condition`. P3: **209 transitive crates with default features, 129 with `default-features = false`** — against our current 139 for the whole workspace. Non-optional `tokio 1.52.3` and `tonic 0.14.6`; plus `reqwest 0.13.4` (via the default `download_snapshots` feature), hyper, h2, axum, tower/tower-http, rustls + ring + webpki, mio, socket2, prost. P4: yes, assumes a running Qdrant server over gRPC; not an in-memory evaluator. P5: Apache-2.0, v1.19.0 released 2026-08-04 (server v1.19.1, 2026-09-04), repo `qdrant/rust-client`, 29 contributors, commits through 2026-09-01 — org-maintained, tracks server releases, not single-maintainer.

**There is no types-only Qdrant crate.** `qdrant` on crates.io is a **0.0.0 placeholder** (last touched 2024-04-18). The server's `segment` crate v0.6.0 holds the real serde+schemars types but is **workspace-internal and unpublished** (`segment` on crates.io is an unrelated Meilisearch analytics fork). `qdrant_rest_client` 0.2.1 is a third-party WASM SDK from second-state.

**Coverage.** C1 yes over payload fields, with real boolean algebra and ranges. C2 no (projection is a separate `with_payload` parameter, not part of the filter). C3 no in-filter ordering; paging is `offset`/`limit` or `scroll` with a point-id continuation, not part of the filter document. C4 no. C5 yes (`count` endpoint takes a filter and returns a count without bodies).

## III-3. Baseline for reading dependency counts

`serde` + `schemars` + `rmp-serde` alone resolve to **22 transitive crates** (verified with `cargo generate-lockfile`, cargo 1.95.0, counting `[[package]]`). Current versions: serde 1.0.229 (MIT/Apache-2.0), schemars 1.2.2 (MIT, 2026-07-27), rmp-serde 1.3.1 (MIT, 2025-12-23). Read against the workspace's 139.

## III-4. Expression evaluators and scripting languages

**A cross-cutting fact first: none of these derive schemars anywhere in their repositories** (GitHub code search, `total_count` 0 for each). And none resolve tokio, reqwest, hyper, sqlx or lalrpop.

| crate | version / date | P1 | P3 transitive | P5 |
|---|---|---|---|---|
| `evalexpr` | 13.1.0, 2025-11-26 | **String-parsed.** `Node` AST is public, but `feature_serde` implements **`Deserialize` only, from a string** (`deserialize_str` → `build_operator_tree`). **No `Serialize` on `Node`** — one-way. | 0 default; 24 with serde+regex+rand | **AGPL-3.0-only.** 413★, 20 contributors, last commit 2025-11-26, effectively single-maintainer (ISibboI) |
| `rhai` | 1.26.1, 2026-09-10 | **String-parsed.** `AST` has private fields, internals behind an `internals` feature. No serde on the AST. Its `serde` feature serializes `Dynamic` *values*; `metadata` emits function signatures. Neither is an AST. | 45 (48 w/ serde). futures-core/task/util, slab, pin-project-lite (async plumbing, no runtime); wasm-bindgen, js-sys | MIT/Apache-2.0. 5,682★, commit 2026-09-16, led by Stephen Chung |
| `fasteval` | 0.2.4, **2020-01-25** | **String-parsed.** Slab-arena AST, **no serde dependency at all.** Algebraic expressions, not a general predicate language | 0 | MIT. **Dormant** — last commit 2020-09-21, 2 contributors. Forks `fasteval2` (2023-01) and `fasteval3` (2023-11) also stale |
| `cel` (was `cel-interpreter`/`cel-parser`) | cel 0.14.5, 2026-09-07 | **String-parsed.** AST (`Ast`, `Expr`, `CallExpr`, `ComprehensionExpr`) is public but derives only `Clone, Debug, Default, PartialEq` — **no serde, not even behind a cfg_attr.** Upstream Google CEL defines protobuf `ParsedExpr`/`CheckedExpr`; **cel-rust does not implement it** | 67 — the largest here. **LOUD: `antlr4rust` 0.5.2 (ANTLR runtime) AND `nom` 7.1.3 — two parsing frameworks.** Plus futures/parking_lot, chrono, regex, `cc` + find-msvc-tools (a C compiler in the build graph), 6 windows-* crates | MIT. 677★, pushed 2026-09-16, multi-maintainer |
| `jsonlogic-rs` | 0.5.0, 2024-12-13 | **Structured data on the wire** (JsonLogic rules are JSON trees) — **but the Rust type is `serde_json::Value`.** No typed enum with derived Serialize/Deserialize; shape checked at runtime | 34. phf + generator/macros, rand stack, deprecated proc-macro-hack, clap 2.33.1 | MIT. Last commit 2024-12-13, 56★, single-maintainer |
| `jsonlogic` | 0.5.1, **2020-03-05** | Same: `Value` in, `Value` out | 12 | MIT. **Abandoned**, 5★, **1 contributor** |
| `datalogic-rs` | 5.5.0, 2026-09-12 | **Structured JSON rule, not a string.** Compiles to an arena AST behind `Logic`; **`Logic` is not Serialize/Deserialize** — the serializable artifact is the input JSON | **15 — the cleanest non-trivial tree in the survey.** bumpalo, datavalue-rs, fast-float2, itoa, ryu, self_cell, smallvec, serde. No parser generator, no async, no network, no DB | Apache-2.0. 89★, 6 contributors, 0 open issues, commit 2026-09-12, effectively single-maintainer (GoPlasmatic). Ships Node, WASM, Python, Go, Java, **.NET**, PHP and C bindings plus a 1,804-case conformance suite |
| `jmespath` | 0.5.0, 2026-01-19 | **String-parsed.** `ast::Ast` public, 18 variants, public fields, derives `Clone, Debug, Display, PartialEq` — **no serde.** Serialize in the repo is on `Variable`, the data type. **`Ast` is not `Send`/`Sync`** (holds `Rcvar`) | 22. serde, serde_json, zmij, itoa, memchr, once_cell, wasm-bindgen; build-dep `slug` → `deunicode` | MIT. 161★, 16 contributors, org-maintained, bursty |
| `jsonpath-rust` | 1.0.11, 2026-09-09 | **String-parsed.** `JpQuery`, `Segment`, `Selector`, `Filter`, `FilterAtom` derive `Debug, Clone, PartialEq` only — **no serde, no schemars** | 24. **LOUD: `pest` 2.7 + pest_derive/generator/meta + ucd-trie — a full parser generator.** Plus the regex stack | MIT. 159★, 23 contributors, commit 2026-09-09, ~90M downloads |

**P4 for the whole family:** none assumes a backing store. But each is bound to a document model — jsonlogic/jsonpath-rust/jmespath to `serde_json::Value`, datalogic-rs to `DataValue`/bumpalo, evalexpr/rhai/fasteval to their own context types. None reads typed Rust structs directly.

**Why the family as a whole does not meet P1, stated plainly:** six of eight are string-parsed languages whose wire form is source text. Four expose a public AST; **none of the four derive serde on it.** `evalexpr` is the only one with any AST serde impl and it is Deserialize-from-string with no Serialize. Two put structured data on the wire (the JsonLogic family), but as untyped `serde_json::Value` rather than a typed tagged union. **Zero generate a JSON Schema.**

## III-5. Filter and predicate crates

**`predicates` 3.1.4** — MIT/Apache-2.0, updated 2026-02-11, ~225M downloads, assert-rs org (Nick Stevens, Ed Page), 21 contributors. P3: 13 transitive, regex optional, nothing flagged. P4: pure in-memory `fn(&T) -> bool`.
**Why it does not fit:** P1 fails absolutely. `Predicate<T>` is a trait; combinators produce nested Rust generic types (`AndPredicate<…>`, `FnPredicate<F,T>`) or `BoxPredicate<T>`, and `FnPredicate` wraps a Rust closure. **There is no data-shaped AST and nothing crosses a wire.** A TS or C# caller cannot construct one. Nothing to derive a schema from either.

**`filtrex` — does not exist as a Rust crate.** `crates.io/api/v1/crates/filtrex` returns **404**. Filtrex is JavaScript (joewalnes/filtrex, fork cshaa/filtrex) plus an Elixir port (rcdilorenzo/filtrex). Recorded because it is named in the brief's family list and is otherwise easy to assume into existence.

**nom-based filter languages** — the published examples are small and each carries a flag. `dsq-parser` 0.2.0 (MIT/Apache-2.0, 2026-02-18, **571 downloads**, docs.rs build **failed**): parses a DSQ string into an `Expr` AST, serde non-optional so the AST is serde-shaped, but the authoring surface is a string; **flag: `nom ^8.0`**. `tellaro-query-language` 2.0.0 (2026-09-05, 3,554 downloads): **flag: a 213-line Pest grammar plus an optional `opensearch` feature that is a network client**; crates.io reports its licence as **"non-standard"**. `filter-expr` 0.2.0 (MIT, 2026-08-13, 1,324 downloads), hand-rolled parser; **flag: `async-trait` — its evaluation path is async.** Meilisearch's `filter-parser`, the most-cited real nom filter language, is **not published** (`crates.io/api/v1/crates/filter-parser` → 404); it is workspace-internal.

**`sea-query` 1.0.2** — MIT/Apache-2.0, 2026-08-12, ~1.8M downloads/month, SeaQL org, multi-maintainer. P3: 16 transitive, 15 direct and nearly all optional value-type integrations; **no tokio or DB driver in the core** (those live in `sea-query-binder` and the `sea-query-*-driver` crates); `#![forbid(unsafe_code)]`.
**Why it does not fit — two independent disqualifiers.** P1: in 1.0 `SimpleExpr` is a type alias for the `Expr` enum, and the docs.rs trait-impl list for `sea_query::expr::Expr` shows `Clone`, `Debug`, many `From<T>`, `PartialEq`, `PgExpr`, `SqliteExpr` and auto traits — **no `Serialize`, no `Deserialize`**; same for `Condition`. Its `serde` feature (one of 44) covers **value** types only (bigdecimal, chrono, uuid, rust_decimal, time, ipnetwork, pgvector). P4: output is a SQL string plus bound parameters for MySQL/Postgres/SQLite; it cannot evaluate over an in-memory collection. P2: no schemars.

**`diesel` 2.3.13** — MIT/Apache-2.0, 2026-09-04, ~36M downloads, lead maintainer Georg Semmler (effectively one primary maintainer, very actively maintained).
**Why it does not fit — categorically.** The DSL is compile-time typed: each fragment is a distinct generic Rust type (`Eq<column, Bound<…>>`) implementing `QueryFragment<Backend>`. **The query exists only as a monomorphized Rust type, so there is nothing to serialize**, and a TS/C# caller can never produce one. Diesel's serde involvement is row values. P3 flag: backend features pull libpq / libmysqlclient / libsqlite3, and async needs `diesel-async` → tokio. P4: hard-assumes a database engine.

**`sqlx` 0.9.0** — MIT/Apache-2.0, 2026-05-21, 147M downloads, LaunchBadge org.
**Why it does not fit:** it has no query IR at all. "Compile-time checked queries without a DSL" means SQL strings validated against a live database at compile time; there is no query object, serializable or otherwise. P3 is flagged on every axis the brief names: **mandatory async runtime (tokio/async-std/smol), full PostgreSQL/MySQL/SQLite wire-protocol drivers, TLS stacks.** P4: assumes a live database at compile *and* run time.

## III-6. Query builders and analytic IRs with a serializable IR

**`polars-plan` `Expr` (polars 0.55.2)** — MIT.
- P1: **serializable.** `polars_plan::dsl::Expr` implements `Serialize` + `Deserialize` (confirmed on the docs.rs trait-impl list for 0.51.0 — the **0.55.2 docs.rs build failed**, last successful render is 0.51.0). Python exposes it as `Expr.meta.serialize()` / `Expr.deserialize()`, `format="binary"` (default) or `"json"`.
- Exact feature flags: on the umbrella `polars` crate, `serde` (→ polars-buffer/core/utils) and **`serde-lazy`** (→ polars-core/io/lazy/ops/time/utils — *this* is the one that reaches the expression layer). On `polars-plan` itself the feature is plain `serde`, plus `ir_serde` for the resolved IR.
- **The wire format is explicitly unstable.** Polars documentation states serialization is not stable across versions; a LazyFrame serialized in one version may not deserialize in another. Recurrent serde-feature build breakage: issues #3431, #3875, #18697.
- P2: **yes, real schemars.** `polars-plan` has a **`dsl-schema`** feature pulling **schemars ^1.2.1** (was ^0.8.22 at 0.51.0), plus serde_json and sha2; it auto-enables `serde` and propagates across the sub-crates. **`dsl-schema` is not exposed on the umbrella `polars` crate's 161-feature list** — it is reachable only via `polars-plan` directly.
- P3: very large — ~22-57 MB, **~1M SLoC**. **Flag: `polars-plan` 0.55.2 deps include `tokio` and `futures`.** Plus rayon and the arrow/compute stack; `polars-io` brings object-storage/cloud network clients when enabled. No parser generator, no DB driver.
- P4: **runs over in-memory collections natively**, no database engine required.
- P5: 0.55.2, 2026-08-06. 120 releases, **55 of them breaking.** ~792K downloads/month, 771 dependents. Ritchie Vink plus 100+ contributors, commercially backed (Polars Inc). **`polars-plan`'s own docs.rs page states it is an internal sub-crate "not intended for external usage"** — no public API stability guarantee.

**`datafusion-proto` 55.1.0 (with `datafusion`'s `LogicalPlan`)** — Apache-2.0, 2026-09-11, 67 versions, quarterly-ish majors with frequent breaking changes, 5.2M downloads, ASF project with many maintainers.
- P1: **serializable typed data via protobuf, not serde.** `LogicalPlan`/`Expr` are Rust enums without serde derives; `datafusion-proto` converts bidirectionally to generated prost messages. TS/C# callers *can* construct these — the `.proto` is the contract. Caveat: round-tripping UDFs and extension nodes requires registering codecs on the Rust side.
- P2: the protobuf schema is the machine-readable schema (`datafusion/proto/proto/datafusion.proto` + `datafusion_common.proto`, split into `datafusion-proto-common`/`-models` at 55.x). No schemars. `serde_json` is optional and debug-path only.
- P3: **heavy.** 22 direct deps: arrow ^59.2.0, **`object_store ^0.13.2` (a network/cloud storage client)**, prost ^0.14.1 and 16 `datafusion-*` crates. **`datafusion-execution` 55.1.0 pulls `tokio` (twice, target-gated), `tokio-util`, `async-trait`, `futures`, `dashmap`, `parquet` and `object_store` unconditionally.** The transitive graph runs to many hundreds: you cannot take `datafusion-proto` without most of a query engine.
- P4: ships a full execution engine. It does not require an external DB (an in-memory `MemTable` works), but you embed a catalog and a scheduler and tokio.
- `datafusion-substrait` 55.1.0 (Apache-2.0, 2026-09-11, ~2.4M downloads, ~9,085 SLoC) converts `LogicalPlan` ⇄ Substrait protobuf and **inherits the entire DataFusion tree** — tokio, object_store, arrow, parquet all present.

**`substrait` (substrait-rs) 0.65.0** — Apache-2.0, 2026-08-27, edition 2024, MSRV 1.88.
- P1: **serializable typed data is the crate's entire purpose** — "Cross-Language Serialization for Relational Algebra". Ships prost-generated `substrait::proto` (`Plan`, `Rel`, `Expression`, `FunctionArgument`, …) plus `substrait::text` for YAML extensions. TS and C# construct these from the published `.proto` with standard codegen; that is designed usage, not a workaround.
- P2: the protobuf schema **is** the machine-readable schema, published at `substrait-io/substrait` `proto/substrait/*.proto`, packaged at `substrait-io/substrait-packaging`, bindings at `substrait-io/substrait-protobuf`. The simple-extensions YAML additionally has a real **JSON Schema 2020-12** at `text/simple_extensions_schema.yaml`. Spec nuance: Substrait is defined **only** in Protobuf; the JSON you see is Protobuf-JSON output, not an official text format. The crate's `serde` feature "generates serde implementations that match the Protobuf JSON Mapping **via pbjson**" — so serde output is proto-canonical JSON, not an independent serde shape. **No schemars.**
- P3: direct deps indexmap, prost, serde, serde_json, substrait-extensions, substrait-prost; feature-gated hex, serde_yaml, thiserror; build-deps semver, toml, prost-types. ~6-9.5 MB / **~196K SLoC**, the bulk being generated prost code. **No tokio, no network client, no DB driver, no parser generator.** The only flag is the optional `protoc`/`protox` features, which invoke a protobuf compiler at build time (`protox` is pure Rust, no external binary). Seven features, none default: embed-descriptor, extensions, parse, protoc, protox, semver, serde.
- P4: **no backing store and no engine.** Pure IR types plus serialization; **it does not evaluate anything** — you supply the evaluator.
- P5: 162 versions, fast cadence. **17.5M lifetime / ~892K recent downloads.** Owner Jacques Nadeau with co-owners Matthijs Brobbel, Victor Barua, YongChul Kwon, Weston Pace, 13+ contributors — a cross-vendor spec body, not single-maintainer. The repo itself is modest (92★). Still 0.x: no 1.0 stability promise on the Rust binding, and the spec versions independently.

## III-7. Document-store and protocol filter documents, reusable as a shape

**Only four of the surveyed protocols put a structured document on the wire:** MongoDB filters, Elasticsearch/OpenSearch query DSL, Firestore StructuredQuery, and Kubernetes `LabelSelector` (plus JSON Schema, structured but a different kind of predicate). OData, AIP-160, CESQL, GraphQL documents and the Kubernetes `labelSelector`/`fieldSelector` query parameters are all **strings**.

**Only three publish a machine-readable schema of the query shape itself:** OpenSearch, Elastic, Firestore. Kubernetes publishes one for `LabelSelector` specifically. **MongoDB — the most imitated shape of all — publishes none.**

### MongoDB query documents and BSON
Structured. Keys are dot-notation field paths; values are literals (implicit `$eq`) or operator documents. `$and`/`$or`/`$nor` take arrays of filter documents; **`$not` wraps an operator document, not a full filter** — a known asymmetry. Operators: `$eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex $mod $all $elemMatch $size $bitsAllSet $expr $jsonSchema $text $where`. Example: `{ "$and": [ {"status":"active"}, {"score":{"$gte":10,"$lt":100}}, {"tags":{"$in":["a","b"]}}, {"meta.region":{"$ne":"eu"}} ] }`.

**P2: no.** A BSON 1.1 wire spec exists (bsonspec.org/spec.html) and driver behaviour specs at github.com/mongodb/specifications, but **no published JSON Schema or IDL for the legal shape of a filter document.** Operator semantics are prose. `$jsonSchema` is a validator operator, not a schema of the filter language.

`bson` v3.1.0, 2025-11-20, MIT — **fully standalone, does not require the driver.** Default features are only `compat-3-0-0` (a no-op shim); **`serde` is optional**, and with it `bson::Document` implements Serialize/Deserialize. **No tokio anywhere.** Optional chrono-0_4, jiff-0_2, time-0_3, uuid-1, serde_json-1, serde_with-3, large_dates; also zero-copy `RawDocument`/`RawDocumentBuf`. MessagePack caveat: BSON has types with no MessagePack analogue (ObjectId, Decimal128, Binary subtypes, Timestamp, MinKey/MaxKey), which round-trip through serde as extended-JSON-ish maps rather than natively.

`mongodb` v3.9.1, 2026-09-10, Apache-2.0 — **loud: `tokio ^1.17` is a non-optional dependency** (io-util, sync, macros, net, process, rt, time, fs), plus tokio-util, futures-*, a crypto stack (hmac, md-5, sha1, sha2, pbkdf2, stringprep), base64, rand; optional rustls/openssl, reqwest, hickory-dns, mongocrypt, aws-sigv4. Notably `bson` itself is *optional* there.

### Elasticsearch / OpenSearch Query DSL
Structured. `bool` with `must`/`should`/`must_not`/`filter` plus `minimum_should_match` and `boost`; leaves `term`, `terms`, `range` (gt/gte/lt/lte/format/time_zone), `match`, `match_phrase`, `exists`, `prefix`, `wildcard`, `regexp`, `ids`, `nested`, `script`. Polymorphism is "a single-key object selects the variant" — a tagged union expressed as an object with exactly one key, which is precisely why Elastic had to write a custom spec language.

**P2: yes, both vendors.** Elastic: `elastic/elasticsearch-specification` (Apache-2.0); source of truth is TypeScript definitions and the compiler emits `output/schema/schema.json`, **a custom JSON type-model — not JSON Schema and not OpenAPI** — covering ~500 endpoints and ~3000 types including `QueryContainer`, `BoolQuery`, `TermQuery`, `RangeQuery`. OpenSearch: `opensearch-project/opensearch-api-specification` (Apache-2.0, **OpenAPI 3.1.0**), query DSL in `spec/schemas/_common.query_dsl.yaml` (~1400 lines), `QueryContainer` with 60+ variants, plain OpenAPI/JSON-Schema-flavoured.

Rust: the official `elasticsearch` 9.1.0-alpha.1 (2025-08-08, Apache-2.0) is **a full HTTP client, reqwest + tokio**, and its bodies are `serde_json::Value` anyway — no typed DSL. Types-only alternatives exist: `elasticsearch-dsl` 0.4.13 (~2023, MIT/Apache-2.0, explicitly independent of the official client, no HTTP, serde types, **no runtime deps**) and `opensearch-dsl` (a fork). Both stale-ish and community-maintained.

### Firestore StructuredQuery (protobuf)
Structured, protobuf, `google/firestore/v1/query.proto` in googleapis/googleapis, Apache-2.0. `StructuredQuery { select, from[], where: Filter, order_by[], start_at, end_at, offset, limit, find_nearest }`. `Filter` is a **oneof of exactly three**: `composite_filter`, `field_filter`, `unary_filter`. `CompositeFilter { op: AND|OR, filters: repeated Filter }` — recursion lives here and only here. `FieldFilter { field: FieldReference, op, value: Value }` with ops `LESS_THAN, LESS_THAN_OR_EQUAL, GREATER_THAN, GREATER_THAN_OR_EQUAL, EQUAL, NOT_EQUAL, ARRAY_CONTAINS, IN, ARRAY_CONTAINS_ANY, NOT_IN`. `UnaryFilter { field, op }` with `IS_NAN, IS_NULL, IS_NOT_NAN, IS_NOT_NULL`. `FieldReference { field_path }`, dot-delimited. **There is no NOT combinator** — negation exists only as `NOT_EQUAL`/`NOT_IN`/`IS_NOT_*`.

JSON mapping is standard protobuf JSON (lowerCamelCase, enums as string names):
`{"where":{"compositeFilter":{"op":"AND","filters":[{"fieldFilter":{"field":{"fieldPath":"status"},"op":"EQUAL","value":{"stringValue":"active"}}},{"fieldFilter":{"field":{"fieldPath":"score"},"op":"GREATER_THAN_OR_EQUAL","value":{"integerValue":"10"}}},{"unaryFilter":{"field":{"fieldPath":"deletedAt"},"op":"IS_NULL"}}]}}}`

Rust: **no first-party Google crate.** Generated bindings (`googleapis-tonic-google-firestore-v1`, `gcloud-sdk`, `firestore`) — **loud: tonic ⇒ hyper + tokio + prost; `gcloud-sdk` adds an HTTP and auth stack.** The runtime-free path is taking the Apache-2.0 `.proto` and running prost yourself, which is build-time codegen and gives protobuf encoding rather than MessagePack.

### Kubernetes label selectors and fieldSelector — two distinct surfaces
`labelSelector` **as a query parameter on LIST/WATCH is a string**: `environment=production,tier!=frontend`, `environment in (production,qa)`, `partition`, `!partition`. Comma means AND. **No OR at all.** The `LabelSelector` **API object** embedded in Deployment/Service specs is **structured**: `{matchLabels:{k:v}, matchExpressions:[{key, operator: In|NotIn|Exists|DoesNotExist, values:[...]}]}`, all terms AND, **no OR, no nesting, no recursion.** `fieldSelector` is **string only** (`metadata.name=foo,status.phase!=Running`) with no structured equivalent and per-resource, mostly undocumented selectable fields.

**P2: yes for `LabelSelector`** — `io.k8s.apimachinery.pkg.apis.meta.v1.LabelSelector` and `.LabelSelectorRequirement` in the Kubernetes OpenAPI (kubernetes/kubernetes `api/openapi-spec`, served live at `/openapi/v3`). `k8s-openapi` v0.28.0, 2026-06-15, Apache-2.0, generated from that spec, serde-derived, **no async runtime, types only** (the runtime hazard lives in `kube`, which pulls tokio/hyper/tower). Requires a `v1_xx` feature flag and is a very large crate if you want two structs.

**Partial fit, stated plainly:** the smallest well-schematized structured predicate in the survey and proof the pattern works — but flat and conjunction-only.

### JSON:API filter
**Unspecified.** Spec v1.1 reserves the `filter` query-parameter family and states it is agnostic about the strategies servers support. No operators, no grammar, no nesting rules, no document shape. jsonapi.org publishes a JSON Schema for the *response* document, not for filters. No Rust crate.
**Why it does not fit:** it is a naming convention for a URL query parameter, not a predicate language. The only transferable idea is `filter[field]=value`, a string/URL encoding that cannot carry typed values or nesting.

### OData `$filter`
**String expression:** `$filter=Price gt 20 and (contains(Name,'milk') or Category/Name eq 'Dairy')`. Operators `eq ne gt ge lt le and or not in has`, arithmetic, functions, lambda `any`/`all`, `/` traversal. **A machine-readable grammar is published — as ABNF, not as a data schema:** OASIS OData 4.01 ABNF Construction Rules (docs.oasis-open.org/odata/odata/v4.01/cs01/abnf/odata-abnf-construction-rules.txt, source at oasis-tcs/odata-abnf). CSDL JSON schematizes the *entity model*, not the filter (OASIS Standard 4.01, 2020-05-11; validating schemas at oasis-tcs/odata-csdl-schemas). So the data model has a JSON schema and the predicate language does not. No mainstream typed-AST Rust crate exists; **any usable path means an ABNF/PEG parser generator and you own the AST and the MessagePack mapping.**
**Why it does not fit:** string-first by design. Serializing it as typed data means inventing the AST OASIS deliberately never published.

### GraphQL
**The query is a string** — a GraphQL document parsed server-side. Filters inside it are arguments typed by **input object types** (`input UserFilter { and: [UserFilter!], or: [UserFilter!], age: IntFilter }`), which is a genuinely reusable predicate shape — but values reach the server either as literals in the query string or as JSON `variables`, so only the variables half is structured data. Spec: GraphQL October 2021 (spec.graphql.org/October2021/). **P2: yes for the type system** — SDL plus standardized introspection (`__schema`, `__type`, `__InputValue`), so clients can discover filter input types at runtime. **No standard schema for filter semantics**: there is no canonical `IntFilter`/`and`/`or` vocabulary, and Hasura, Postgraphile, Prisma and Dgraph each invent their own operator names. Rust: async-graphql, juniper, graphql-parser — **flag: a parser, and for servers an async runtime.**
**Partial fit:** the input-object pattern is directly stealable; the transport is not.

### JSON Schema as a predicate language
Draft **2020-12** is current (json-schema.org/draft/2020-12); IETF standardization is ongoing (`draft-ietf-jsonschema-json-schema-03`, Aug 2026). The ecosystem is fragmented across draft-07 / 2019-09 / 2020-12. A schema is itself JSON, and it is **self-describing** — meta-schema at json-schema.org/draft/2020-12/schema.
**Can express:** full boolean algebra including **`not`** — real negation, which both MongoDB and Firestore lack — plus type checks, numeric ranges, string constraints (`pattern`, `minLength`, `format`), `enum`, `const`, array constraints (`contains`, `minContains`, `prefixItems`, `uniqueItems`), object constraints, `if`/`then`/`else`, and `$ref`/`$dynamicRef` recursion.
**Cannot express:** comparison between two fields of the same document (no `a.x > a.y`); any reference to an external value or parameter — every bound is a literal baked into the document; sorting, projection, aggregation or joins; arithmetic; dot-path addressing in a predicate sense — a deep-field range check requires nested `properties` mirroring the document rather than `"a.b.c": {gte: 5}`. Parameterizing a query means rewriting the schema document.
Rust: `jsonschema` (MIT, active) — **flag: a regex engine, fancy-regex, URL resolution, and optionally `reqwest` for remote `$ref` fetching.** `schemars` generates schemas from Rust types but is not an evaluator.

### CloudEvents SQL (CESQL)
**String expression**, SQL-like: `subject LIKE '%mySubject%' AND myext = 'value' AND (type = 'a' OR type = 'b')`, evaluating to boolean/integer/string. **v1.0 approved 2024-06-13**, github.com/cloudevents/spec/blob/main/cesql/spec.md, Apache-2.0 (CNCF), SDKs for Go and Java. **No document schema** — the spec defines an ANTLR-based grammar, not a serializable AST — and it operates over the flat CloudEvents attribute set, so there are no nested paths. **No Rust crate**; `cloudevents-sdk` has no CESQL module. **Flag: porting means a parser generator.**

### Cross-cutting structural facts for this family
- **Recursion and negation matrix:** MongoDB has `$and`/`$or`/`$nor` plus operator-scoped `$not`; Elasticsearch's `bool` has `must_not` and nests freely; **Firestore has AND/OR recursion but no NOT combinator**; Kubernetes `LabelSelector` has neither recursion nor OR; JSON Schema has full boolean algebra including `not`.
- **Tagged-union encoding differs sharply.** MongoDB and Elasticsearch use "object with a single magic key" (`{"$gt":5}`, `{"term":{…}}`) — cheap in JavaScript, painful in C# and for MessagePack codegen, and the same untagged-discrimination problem Qdrant has (III-2). Firestore's protobuf `oneof` with named enum operators and Substrait's proto messages are the explicitly-tagged counterexamples in the set, and both map cleanly to TypeScript discriminated unions and C# sealed hierarchies.
- **Runtime-free Rust type sources that were found:** `bson` 3.1.0 + serde (MIT, no tokio, standalone); `k8s-openapi` 0.28.0 (Apache-2.0, types-only, large); prost-generating from Firestore's `query.proto` yourself (Apache-2.0 proto, build-time codegen); `elasticsearch-dsl` 0.4.13 (MIT/Apache, no HTTP, stale). **Carrying runtimes:** `mongodb` (mandatory tokio), `elasticsearch` (reqwest + tokio), `kube` (tokio/hyper/tower), and every OData/AIP-160/CESQL path (parser generators).

## III-8. Cursor and pagination prior art as data

**Google AIP-158** (https://google.aip.dev/158). Request: `int32 page_size` (optional; the API may choose a default and return fewer) and `string page_token`. Response: repeated results as the **first** field, `string next_page_token` (empty means end), optional `int32 total_size`. Opacity, verbatim: page tokens **must** be opaque (but URL-safe) strings and **must not** be user-parseable. **No encoding is mandated.** The AIP *suggests* an internal protobuf message, serialized and base64-encoded, for non-sensitive tokens — and warns that base64-encoding an otherwise-transparent token is **not** sufficient obfuscation. A server-side handle is not required: tokens may carry state, but scope is strict — they must be limited to indicating where to continue and **must not provide any form of authorization**, which is re-checked every call. Expiry is permitted, ~3 days named as reasonable if the server stores data behind the token. No cryptographic guidance.

**AIP-159** covers the `-` wildcard for cross-collection reads and **contains nothing about page tokens.** Only tangentially relevant: cross-parent requests should avoid `order_by` because ordering is ambiguous.

**Relay GraphQL Cursor Connections** (https://relay.dev/graphql/connections.htm). `Connection { edges: [Edge], pageInfo: PageInfo! }`; `Edge { node, cursor }` where `node` may be Scalar/Enum/Object/Interface/Union or Non-Null thereof and **must not be a list**, and `cursor` is a String or a custom scalar that serializes as a String; `PageInfo { hasPreviousPage: Boolean!, hasNextPage: Boolean!, startCursor: String, endCursor: String }`. Arguments `first`/`after` and `last`/`before`. Verbatim: the cursor field's result "should be considered opaque by the client, but will be passed back to the server." `startCursor`/`endCursor` must correspond to the first and last nodes and **can be null when there are no results**. **The spec mandates no encoding — base64 appears nowhere in it.** Base64 of `type:id` or of an offset is ecosystem convention (graphql-js, graphql-ruby, Relay tooling), not spec text. Neither the HTML nor the markdown source carries a printed version or date; the spec's own example uses the placeholder `after: "opaqueCursor"`.

**Rust crates offering an opaque-cursor type.** The exact names `cursor-pagination`, `page-token`, `seek-pagination` and `opaque-cursor` return **zero crates.io results**.

- **`use-cursor` 0.0.1** (crates.io/crates/use-cursor, repo RustUse/use-api). Types `OpaqueCursor`, `AfterCursor`, `BeforeCursor`, `NextCursor`, `PreviousCursor`, `CursorDirection`. P1: **a string newtype, not typed data** — `OpaqueCursor::new("…")` / `as_str()`, **no serde.** P2: no. P3: **zero dependencies.** P4: store-agnostic, and it does no encoding or decoding at all — a validated string wrapper plus direction slots. P5: MIT/Apache-2.0, a single release 2026-05-25, **132 downloads**, single maintainer, Rust 1.95 / edition 2024, ~170 LoC. One release, no track record.
- **`api-paginate` 0.1.1** (crates.io/crates/api-paginate). `PaginationParams`, `PaginatedResponse`, `CursorPagination { cursor: Option<String>, limit: u32 }`. P1: **the envelope is serde-typed; the cursor itself is `Option<String>`**, with no encode/decode helpers. P2: no schemars; OpenAPI via an optional `utoipa ^4`. P3: serde plus optional utoipa; dev-only criterion/proptest; **no runtime, network, DB or parser deps.** P4: store-agnostic. P5: MIT/Apache-2.0, 0.1.0 2026-09-01 and 0.1.1 2026-09-12, **27 total downloads**, single maintainer, ~196 LoC. Two weeks old.
- **`paginator-utils` / `paginator-rs` 0.4.0** (crates.io/crates/paginator-rs). Keyset cursors, "URL-safe Base64 and validated on decode", `next_cursor`/`prev_cursor` derived from boundary rows. P1: string cursors over base64; the surrounding page structs are serde. P2: no schemars (a `paginator-zod` sibling generates Zod/TS schemas, 32 downloads). P3: utils deps are base64 ^0.22, serde, serde_json — clean, **no async runtime in core** — but the family fans out into `paginator-axum`/`-actix`/`-rocket`/`-sqlx`/`-sea-orm`/`-surrealdb`, which pull web servers and DB drivers. P4: core is store-agnostic. P5: MIT, 0.4.0 2026-09-07, 10 releases since 2025-05-07, ~16.8K downloads, **single maintainer**.
- **`bomboni_request` 0.5.0** — the only AIP-158-shaped page-token implementation found. `PlainPageTokenBuilder` plus base64, AES-GCM-encrypted and RSA-encrypted variants; tokens are typed and serde-serializable. P2: no schemars. P3: **loud.** Non-optional aes-gcm 0.11, rsa 0.9, blake2, base64ct, **prost 0.14**, **pest + pest_derive 2.8 (a parser generator)**, regex, itertools, serde, thiserror, time, and four internal `bomboni_*` crates; optional **tonic 0.14**, mysql_common, postgres-types, wasm-bindgen. This is a filter/query-language framework, not a cursor primitive. P5: crates.io reports the licence as **"Non-standard"**; 0.5.0 2026-07-20, 65 versions since 2023-11, 106K downloads, single maintainer.
- Inspected and **not** opaque-cursor primitives: `page-turner` 1.0.0 (an abstraction over *calling* paginated APIs as a stream; futures 0.3, tokio dev-only; defines no cursor type); `mongodb-cursor-pagination` 0.3.2 (**pulls the MongoDB driver**, stale since 2023); `paginate` 1.1.11 (offset math, 2022); `page-hunter` 0.7.0 (offset model); `rdb-pagination`, `pg_filters`, `diesel_filter`, `diesel_pagination`, `sqlx-data-params`, `clickhouse-filters` (all SQL/ORM-bound); `api-bones` 6.12.0 (a whole REST-envelope framework).

**Stated plainly: no general-purpose, standalone, typed opaque-cursor crate exists in Rust.** The four obvious names are unpublished; the only crate whose stated purpose is modelling an opaque cursor (`use-cursor`) is a zero-dep **string newtype** at 0.0.1 with no serde that explicitly declines to assume a serialization; `api-paginate` types only the envelope; and everything with real download numbers is framework- or database-bound or is offset arithmetic. **Nothing found offers schemars or JSON Schema for a cursor type** — the closest schema stories are `utoipa` (api-paginate) and `paginator-zod`.

**Framework-bound cursors worth recording.**
- **async-graphql 7.2.1** (2026-09-12, MIT/Apache-2.0): `trait CursorType { type Error: Display; fn decode_cursor(s:&str)->Result<Self,Self::Error>; fn encode_cursor(&self)->String; }` (not object-safe), implemented for primitive ints/floats, bool, char, String, Uuid, `DateTime<Utc>`, Timestamp and ID — plus **`OpaqueCursor<T>` where `T: Serialize + DeserializeOwned`**, "encode/decode the value to base64". **This is the one place in the entire survey where a cursor is a typed serde value with a defined wire encoding, and it is framework-bound.**
- **sea-orm** has two mechanisms: `Paginator` (`.paginate(db, page_size)`) is offset/limit page numbers with no token; `Cursor` (`.cursor_by(col)`, PR SeaQL/sea-orm#822) is true keyset with `.after()/.before()/.first()/.last()`, where **the cursor value is the raw typed column value** — no serialization, no base64, nothing wire-shaped.
- **diesel** has **no built-in pagination.** The canonical approach is the "Extending Diesel" guide: hand-roll `Paginated<T>` implementing `QueryFragment`, emitting `SELECT *, COUNT(*) OVER () FROM (subquery) LIMIT … OFFSET …`. Offset-based, Postgres-specific for the window count, no cursor type. Third-party helpers (`diesel_pagination` 2.0.0, `diesel_filter` 2.0.0, `length_aware_paginator`) reproduce the same offset pattern.

## III-9. Two cross-cutting facts that touch the MessagePack constraint

1. **Nothing surveyed targets MessagePack natively.** Serde-based candidates (polars `Expr`) would go through `rmp-serde`. Protobuf-based candidates (substrait, datafusion-proto, Firestore, qdrant-client) have protobuf-binary and protobuf-JSON as their defined encodings; MessagePack means encoding the proto-derived structure yourself, or going proto → pbjson → serde → rmp, which drops the protobuf wire contract.
2. **Untagged-union discrimination is the recurring hazard.** Qdrant (`Condition`, `Match`), MongoDB (`{"$gt":5}`) and Elasticsearch (`{"term":{…}}`) all discriminate variants by structure or by a single magic key rather than an explicit tag. `#[serde(untagged)]` forces `deserialize_any`, and `rmp-serde` has an open reported failure there — msgpack-rust issue #148, untagged enums whose variants contain enums fail under rmp-serde while succeeding under serde_json. Firestore's protobuf `oneof`, Substrait's proto messages and ReQL's leading integer term id are the explicitly-tagged counterexamples in the set.

**Sources for Part III-2 through III-9:** [qdrant filtering](https://qdrant.tech/documentation/concepts/filtering/) · [qdrant openapi.json](https://github.com/qdrant/qdrant/blob/master/docs/redoc/master/openapi.json) · [qdrant_common.proto](https://raw.githubusercontent.com/qdrant/qdrant/master/lib/api/src/grpc/proto/qdrant_common.proto) · [qdrant-client Filter](https://docs.rs/qdrant-client/latest/qdrant_client/qdrant/struct.Filter.html) · [lib.rs qdrant-client](https://lib.rs/crates/qdrant-client) · [msgpack-rust #148](https://github.com/3Hren/msgpack-rust/issues/148) · [serde enum representations](https://serde.rs/enum-representations.html) · [evalexpr](https://github.com/ISibboI/evalexpr) · [rhai](https://docs.rs/rhai/1.26.1/rhai/) · [fasteval](https://github.com/likebike/fasteval) · [cel](https://docs.rs/cel/0.14.5/cel/) · [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) · [jmespath.rs](https://github.com/jmespath/jmespath.rs) · [jsonpath-rust](https://github.com/besok/jsonpath-rust) · [predicates](https://lib.rs/crates/predicates) · [sea-query Expr](https://docs.rs/sea-query/1.0.2/sea_query/expr/enum.Expr.html) · [polars-plan features](https://docs.rs/crate/polars-plan/latest/features) · [substrait-rs](https://docs.rs/substrait/latest/substrait/) · [simple_extensions_schema.yaml](https://github.com/substrait-io/substrait/blob/main/text/simple_extensions_schema.yaml) · [datafusion-proto deps](https://deps.rs/crate/datafusion-proto/55.1.0) · [bson](https://crates.io/crates/bson) · [elasticsearch-specification](https://github.com/elastic/elasticsearch-specification) · [opensearch query_dsl.yaml](https://github.com/opensearch-project/opensearch-api-specification/blob/main/spec/schemas/_common.query_dsl.yaml) · [Firestore query.proto](https://github.com/googleapis/googleapis/blob/master/google/firestore/v1/query.proto) · [AIP-160](https://google.aip.dev/160) · [AIP-158](https://google.aip.dev/158) · [Relay Connections](https://relay.dev/graphql/connections.htm) · [k8s-openapi](https://crates.io/crates/k8s-openapi) · [CESQL](https://github.com/cloudevents/spec/blob/main/cesql/spec.md) · [async-graphql CursorType](https://docs.rs/async-graphql/latest/async_graphql/types/connection/trait.CursorType.html) · [use-cursor](https://crates.io/crates/use-cursor) · [sea_orm::Cursor](https://docs.rs/sea-orm/latest/sea_orm/struct.Cursor.html)

---

# Questions this pass could not answer

1. **Was the query side of RethinkDB deliberately not carried into CultCache, or simply never reached?** The archive names four adoption reasons and none of them is ReQL; ReQL and changefeeds are never mentioned by name in the indexed Discord history. Nothing found says "we chose not to have queries". **To answer:** ask the operator directly. The archive cannot settle intent, only what was said.

2. **Does the 2020-2021 Aetheria-Economy tree contain the C# ReQL driver usage, and what query shapes were actually written against it?** That would show which ReQL constructs were ever exercised here, which is different from what ReQL offered. **To answer:** check out `GameCult/Aetheria-Economy` around commits from 2020-03 to 2021-06 and grep for the C# driver's term builders. Not indexed by voidbot under a name I could confirm.

3. **Which of the thirteen document kinds actually need per-kind predicates, and on which fields?** I recorded what the types expose (`severity`, `confidence`, `origin`, `sequence`, `outcome`, `repo`). I did not find a record of what callers have asked for and been unable to express. **To answer:** search the Eureka campaign's own findings and follow-ups for read-side requests, or ask the operator for the driving query list.

4. **What exactly does Cut 11 intend to own?** `query.rs:65-72` says Cut 11 owns "which fields of each kind are text" and the index. Whether Cut 11 also intends to own structured filter pushdown into Qdrant — which would make the structured-filter shape and Qdrant's filter shape the same question — is not written anywhere I found. The Huginn repo contains no `docs/` or `notes/` directory and no file mentioning "Cut 11" outside `query.rs`'s comments. **To answer:** locate the campaign's target document and cut map (the `PipelineCampaign.target_doc` DocRef would name it) or ask the operator.

5. **Is there a stored mind image to measure against?** Every portability claim about paging and counting assumes a document population size. I found no fixture corpus outside `fixtures.rs`. **To answer:** run the daemon against a real mind store and count documents per kind, or read `crates/huginn-mind/src/fixtures.rs` as the only current sample and treat it as synthetic.

6. **Does `epiphany-pipeline` want to grow a reference enumerator, and is that Epiphany's call or Huginn's?** C4 needs one; the leaf has none and references are split between typed `PipelineRef` and bare `Short`. Changing the leaf is a cross-repo change at a pinned rev (`5cda0886`). **To answer:** an operator ruling on which repo owns the hop table.

7. **Whether `unreql` or any other crate exposes the ReQL term-tree encoding *without* a client.** The survey found drivers, not an encoding library. **To answer:** read `unreql`'s crate structure for a types-only module, or accept that no such split exists.

8. **Does `rmp-serde` actually fail on the specific untagged shape we would write, or only on the nested-enum case in msgpack-rust #148?** The hazard is recorded (III-2, III-9) but not reproduced against our own types. **To answer:** a ten-line test — define a two-variant `#[serde(untagged)]` enum whose variants contain enums, round-trip it through `rmp_serde::to_vec_named`, and see. This is cheap and would settle whether untagged encoding is available to us at all.

9. **What does `qdrant-client` actually resolve to with `default-features = false` *inside our lockfile*?** 209/129 are the survey's numbers taken in isolation; the overlap with our existing 139 is unmeasured, and the marginal cost is what matters. **To answer:** add it to `huginn-daemon` on a scratch branch, `cargo generate-lockfile`, diff the `[[package]]` count, discard.

10. **Can the Qdrant filter shape be obtained as types without the client?** The survey establishes there is no published types-only crate and that `segment` (which has the serde+schemars derives) is workspace-internal. Whether generating Rust types from Qdrant's published OpenAPI, or from `qdrant_common.proto` via prost, produces something usable without tonic was not tested. **To answer:** run a schema-to-Rust generator over `openapi.json` and count what comes out.

11. **Is the polars `Expr` serialization instability a blocker in practice, and does `dsl-schema` actually emit a usable schema?** Both facts are from documentation and feature lists; neither was exercised. The 0.55.2 docs.rs build failure means the current API surface was not directly inspected. **To answer:** build `polars-plan` with `dsl-schema`, emit the schema, and check whether a serialized `Expr` survives a patch-version bump.

12. **Does Substrait's expression subset cover per-kind field predicates over our document types without a relational schema underneath it?** Substrait is relational algebra; our image is not tables. Whether `Expression` is usable detached from `Rel`/`ReadRel` was not established. **To answer:** attempt to construct a bare `Expression` tree with `substrait` 0.65.0 and see what it requires.

13. **How large does a serialized query actually get under each shape, through base64?** The daemon base64s its MessagePack payload (I-1), so encoding verbosity costs 4/3 on top. No candidate was measured. **To answer:** encode the same worked predicate — "open findings of High severity on this campaign" — in each candidate shape and compare byte counts.
