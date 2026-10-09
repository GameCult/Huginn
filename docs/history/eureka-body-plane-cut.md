<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/eureka-body-plane-cut.md; text below this line is unchanged. -->
# The body plane for oversize answers: cut map

Date: 2026-09-22. Imagination output (Opus). Nothing here is committed and no
repository was edited. Self places and commits this file; the natural homes
are a section of `F:\Projects\Epiphany\notes\eureka-pipeline-state-cut.md`
(the campaign that owns the ruling) with the CultLib half copied or linked
into `F:\Projects\CultLib\docs\`, because the cut spans the organ and
CultMesh.

Status: cut map for three cuts, **BP-1** (CultLib: the content plane in the
Rust runtime), **BP-2** (the pin move that lets Huginn see it) and **BP-3**
(Huginn: an oversize answer is deferred to the body plane).

**2026-09-29 (Self):** both questions in section 8 are ruled (Q-BP1 A,
Q-BP2 A, 2026-09-22). Selection Cut 1 merged to CultLib `main` at `dbee044`,
so section 6's collision is gone. **BP-1 is in Hands** (Sonnet) on CultLib
`cultmesh/rust-content-plane`, based on `268e0ef`, with two overrides to
section 3: no committed mutation entries (the operator retired hand-written
harnesses on 2026-09-22; each rule's test is shown red once by a scratch
mutant instead), and verification on Yggdrasil through the stopgap rather
than on the workstation. The Pins table below is as of 2026-09-22.

**BP-1 landed** (Sonnet, 2026-09-29) on `cultmesh/rust-content-plane` at
`29c3e4c`, `11485fb` and `8a66b1d`. `cultnet-rs` went from 264 to 279 tests,
plus 3 C# vector tests, and all 22 listed mutants went red. Size: +486 source
lines against ~330 estimated, and +778 test lines against ~350.
**Soul (Opus) will merge it after a fix batch, now in Hands:**
- F1: a flood of zero-size chunks makes the client send unbounded requests;
- F2: malformed requests are refused at decode, so a Rust server answers
  nothing where the reference answers `found:false`. **This was the map's
  defect**: section 3 ordered decode-time request validation, which
  contradicts "a failure is an answer, never silence". Refusals belong to the
  answer function;
- F3: `BTreeMap` metadata breaks byte identity for keys that are not sorted;
- F4: hashes are normalised only after requests have gone out;
- F5: one error spelling differs from the reference;
- F6: 8 surviving mutants.
Held: byte identity in both directions beyond the fixture, verification,
serving, validation order, and one path. The empty-body deviation (one
zero-length chunk) is the reference's behaviour, so the spec was wrong there too.

**The ruling this map is written under** (operator, 2026-09-17, recorded in
the Epiphany map at about line 368): an answer too large for the control
plane travels over CultMesh's content/body transfer. The control plane
carries a summary and a reference to the body. Raising the window
(1,228,800 bytes) and bounding documents smaller were both rejected. **The
typed oversize refusal stays as the backstop.** The ruling says the cut "spans
the organ and CultMesh", and nothing about it was in Cut 10 except the refusal.

## Pins

| Repo | Branch | HEAD | State |
|---|---|---|---|
| CultLib | `main` (checked out; the operator's shell is on `main`, and the gitStatus snapshot's `cultnet/selection-cut1` is the locked worktree `.claude/worktrees/agent-af580008561975a4c`, also at `b3d9cf7`) | `b3d9cf7` | Dirty in `native/GameCult.Mesh.Quic.Native/*` and `scripts/mutate-cultmesh.mjs` from the QUIC campaign. **Not touched.** `git diff --stat a0813c6 b3d9cf7 -- packages/cultcache-rs packages/cultnet-rs packages/cultmesh-rs` is **empty**, so every Rust anchor below holds at Huginn's pin as well. |
| CultLib selection Cut 1 | `cultnet/selection-cut1` | `b3d9cf7` + uncommitted work in the locked worktree | In Hands (another agent). The collision is in section 6. |
| Huginn | `eureka/memory-organ` | `202e5e3` | Clean. Cut 10 and its two fix batches have landed. |
| Epiphany | `codex/eureka-pipeline-state` | `2876a827` | Map read only. The leaf `epiphany-pipeline` is pinned by Huginn at `5cda0886` and pins `cultcache-rs` at `a0813c6`. |

## 0b. Model page: identity, lifecycle and authority

This table was written before any cut was mapped. Every persistent or
long-lived kind the cut touches has a row, and no cell is empty.

| Kind | Identity (what names it) | Lifecycle (what happens to it over time) | Authority (who decides) |
|---|---|---|---|
| **Deferred body**: the encoded bytes of one `HuginnMindResponse` that did not fit one send | The SHA-256 of those bytes, lowercase hex: the manifest's `contentHash`. Two identical answers are one body. | Created when an answer's encoded envelope exceeds the window and the body is within the deferral bound. A second deferral of the same bytes refreshes it; it is never duplicated. Every chunk served also refreshes it (sliding expiry). It expires when it has been untouched for the TTL, and it is evicted least-recently-touched first when the retained total exceeds the budget. It is lost when the process exits, and the client asks again. It is never written to the mind, a file or any `.cc` store. | The daemon's serve module (`huginn-daemon`) decides to defer and holds the retention store. The mind decides what the answer *is* and knows nothing about deferral. |
| **Chunk** | The SHA-256 of the chunk's bytes. Its record key is `mesh:cdn:chunk:<hash>`, which is the C# reference's spelling (`CultMeshCdnArtifactChunk.CreateRecordKey`, `CultMeshCdn.cs:120`). | It lives exactly as long as some retained body references it. A chunk shared by two bodies is stored once and stays until both are gone. | Chunking (the boundaries and the hash) belongs to `cultnet-rs` `pack_content`. Retention belongs to the daemon's store. |
| **Reference (manifest)** | `gamecult.mesh.cdn_artifact_manifest.v1`, the reference's type, reused and not reinvented. `contentHash` identifies the body; `artifactId` and the other fields describe it and never identify it. | Transient: minted with the body, carried once inside the deferred response, and never stored by the daemon or the client. It stays valid while the body is retained. After that a chunk request answers `found: false` and the client re-asks the operation. | `cultnet-rs` owns the shape and the validation rules, with parity to `CultMeshCdn.ValidateManifestShape` (`CultMeshCdn.cs:499-526`). The daemon fills it in. |
| **Deferred response**: the control-plane answer that stands in for an oversize one | Correlated by the operation envelope's `messageId`, exactly like every other answer. | One per request, sent once, and never deferred itself. It always fits: at the deferral bound it is about 41 KB, far under the window. | Only the daemon's serve module produces it. `Mind` never does. |
| **Content chunk wire messages** `cultmesh.content_chunk_request.v1` / `.response.v1` | Their `schemaVersion` strings, both already defined by the reference (`CultNetSchemaMessages.cs:153,157`, classes `:1414-1446`). | Stateless: one request, one response. | The C# reference owns the spelling. `cultnet-rs` gains it with byte identity, pinned by vectors in both directions. |
| **Vector fixtures** `contracts/cultmesh/content-vectors.cs-written.json` and `.rs-written.json` | File path plus the `label` of each vector. | Committed. Each writer regenerates its own file under `CULTNET_WRITE_VECTORS=1`. The file is expected to grow and is not pinned as final. | Each runtime owns its written file. Both runtimes' tests read both files. |
| **Mind documents and receipts** | Unchanged: leaf keys and receipt ids. | Unchanged. **No body-plane state enters the mind.** | Unchanged: `huginn-mind` admission. |

Forks this page surfaced, which are sections 8's Q-BP1 and Q-BP2:

- **Authority over the physical path.** CultMesh's own doctrine forbids
  schema messages carrying bulk bodies and names the RUDP content path as
  legacy. That is Q-BP1.
- **What the summary is.** The ruling says "a summary". Whether that means the
  reference's own facts or a per-operation typed summary decides whether
  `huginn-mind` grows new types. That is Q-BP2.

**Identity cannot be the request.** Re-deriving a body per chunk request from
the mind would need no retention at all. But a chunk request names only a
hash, so the daemon could not know which answer to recompute without a
Huginn-specific request shape, and that would give up wire reuse with the
reference. Rejected for that reason, not asked.

## 1. Body facts

Each fact is backed by a source read at the pins above or by a probe that
ran code. The probes are in the scratchpad and nothing was committed:

- `bodyplane-cs/`: a net10.0 console against the built reference DLLs
  `bin/GameCult.{Networking,Mesh}/Debug/netstandard2.1/*.dll`, dated
  2026-09-16. The sources it depends on last moved at `f622ee1` (2026-08-18),
  so the DLLs match `b3d9cf7`. It emitted real reference bytes to
  `bodyplane-cs/out/`.
- `bodyplane-rs/`: a crate against `packages/cultnet-rs` by path, release
  build. `src/main.rs` is the transport probe and `src/bin/parity.rs` the
  byte-parity probe.
- `pinprobe/`: `cargo tree -d` over mixed CultLib git revisions.

### F1. Only the C# reference implements body or content transfer. The Rust client Huginn uses has no path today.

- **Source.** Across `packages/` there is no occurrence of
  `content_chunk_request`, `body_read_request`, `cdn_artifact_manifest`,
  `ContentChunk*` or `BodyRead*`, and no `cdn`, `chunkHash` or
  `artifact_manifest` in any `.rs`, `.ts`, `.py` or `.kt` source.
  `cultnet-rs` does not contain the word "body". What the other runtimes have
  is the *local* stream-body negotiation enum (`SharedMemory`,
  `CultCachePage`, `InlineBytes`, and so on): `cultmesh-rs/src/lib.rs:191-200`,
  `cultnet-py/.../cultmesh_contracts.py:434`, and in `cultmesh-ts` and
  `cultmesh-kotlin`. That is same-machine body mapping, not network transfer.
- **Probe A (decode).** `cultnet_rs::decode_cultnet_message_from_slice` over
  bytes the C# reference emitted:
  - `content_chunk_request` gives *unknown variant
    `cultmesh.content_chunk_request.v1`*, because the closed enum
    (`contracts.rs:252-392`) lists 20 `cultnet.*` variants.
  - `content_chunk_response`, `body_read_response` and a real 256 KiB chunk
    response give *invalid type: byte array, expected any valid JSON value*.
    The non-raw decode path converts through `serde_json::Value`
    (`contracts.rs:404-411`), which cannot hold MessagePack `bin`. So adding
    the variants alone is not enough: both must ride the raw path that
    `document_put_raw` uses (`contracts.rs:771-854`).
- **Probe E (on the wire).** A Rust hub sent the reference's own 262,357-byte
  chunk response raw on the `schema` channel. The transport delivered it, and
  the Rust client refused it in `receive_schema_message_once` with the same
  byte-array error.

**Conclusion:** no Rust runtime can request, answer, decode or verify a body
or a content chunk today. Rust gets its first content-plane code in BP-1.

### F2. The reference has two planes, and only one of them fits this class of content.

- **The content plane.** It carries large immutable artifacts
  (`src/GameCult.Mesh/docs/content-sessions.md:1-3`). The reference is a
  `CultMeshCdnArtifactManifest` (`CultMeshCdn.cs:127-190`) listing
  content-addressed chunks (default 4 MiB, `CultMeshCdnPackOptions`
  `:44`). The chunk protocol is one bounded response per chunk
  (`content-sessions.md:44-46`). The transfer owner
  (`CultMeshContentTransferService`, `CultMeshContentTransfer.cs:115`) is the
  only owner of chunk verification, whole-body SHA-256 verification, resume
  and promotion. The preferred connector is TCP: a typed header, then raw
  bytes outside the envelope (`transport-planes.md:22-26`). It is plaintext
  and **restricted to explicit loopback development**
  (`transport-planes.md:103-106`). The RUDP content server
  (`CultMeshLegacyRudpContentServer`, `CultMeshContentSessions.cs:14-87`) is
  "basement priority and exists for compatibility and parity measurement"
  (`transport-planes.md:93-96`), and its connector is never installed by
  default (`CultMeshContentSessions.cs:97-116`, priority 10,000).
- **The body plane proper.** It carries live, lease-bound generations of
  mutable bodies: `CultMeshNetworkBodyStore` (`CultMeshBodySessions.cs:19-219`,
  a 16 MiB default bound, 8 retained generations, HMAC capability tokens,
  expiry by lease). It is served **whole in one response**
  (`CultMeshBodyServer.HandleAsync`, `:241-272`, payload inline). There is no
  chunking.
- **Why that decides it.** A pipeline answer is an immutable artifact, which
  is the content plane's stated class. The body plane's single response meets
  exactly the wall the ruling is routing around: probe B's whole-body send
  failed with `RUDP reliable send queue is full`. The operator's phrase
  "content/body transfer" covers both planes, and the source says which one
  this is. **This map uses the content plane's chunk protocol and manifest.**
  It is recorded as a reading of the source, not asked, because the body
  plane cannot carry this answer on any transport Huginn has.

### F3. What the wire carries. Decoded from real bytes and pinned at byte level.

| Message | Reference encoding (probe, hex prefix) | Fields in order |
|---|---|---|
| `cultmesh.content_chunk_request.v1` | named map `0x85`, 169 bytes for the sample | `schemaVersion`, `messageId`, `chunkHash` (64 hex), `recordKey` (may be empty), `expectedSizeBytes` (int32) |
| `cultmesh.content_chunk_response.v1` | named map `0x87`, 179 bytes for a 4-byte payload | `schemaVersion`, `messageId`, `found`, `chunkHash`, `sizeBytes` (int32), `payload` (**MessagePack `bin`**), `error` (string, empty on success) |
| `gamecult.mesh.cdn_artifact_manifest.v1` | **array** `0x9A` (10 integer-keyed fields), 1,097 bytes for 6 chunks | `[artifactId, kind, version, contentHash, sizeBytes(int64), mimeType, createdAtUtc, chunks[[chunkHash, offset(int64), sizeBytes(int32), recordKey]], tags[], metadata{}]` |
| `cultmesh.body_read_response.v1` (not used) | named map `0x8B`, 11 fields | whole generation in `payload` |

Measured:

- A 256 KiB chunk response encodes to **262,357 bytes**, 213 bytes of
  overhead, so four fit in Huginn's 1,228,800-byte window.
- The manifest for the real 1,315,551-byte case at 256 KiB chunks is 6 refs
  and 1,097 bytes.
- **Probe parity:**
  - A Rust `rmpv` map written in the reference's key order is **byte-identical**
    to the reference's request (169 = 169) and response (179 = 179).
  - A Rust tuple-shaped manifest decodes the reference's bytes and re-encodes
    them **byte-identically**.
  - A Rust re-pack of the same body with SHA-256 at 256 KiB reproduces every
    chunk reference (hash, offset, size, record key) and the content hash.
  - `rmp_serde::to_vec_named`, the encoder Huginn uses for responses, still
    writes a tuple-shaped type as an array (`0x9A`). So the reference's array
    form survives inside Huginn's named-map payload **only if** the Rust type
    serialises as a tuple.

### F4. Chunked delivery works on Huginn's exact transport. Pipelining past the window does not.

Probes B, C and D used a hub configured exactly as Huginn's `serve::bind`
(`serve.rs:135-143`: fragment 1200, pending 1024, one settled session) and the
real 1,315,551-byte body, with chunks carried as base64 inside operation
responses, the only bytes-bearing message Rust has today.

- **One reply carrying the whole body** (a 1,754,254-byte envelope):
  `RUDP reliable send queue is full`, returned to the sender only.
- **Sequential request, then reply, one chunk at a time** at 256, 512 and 768
  KiB: delivered whole and SHA-equal, in 1.56 s, 1.54 s and 1.45 s on loopback
  (about 0.85 MB/s, paced by the 32-packet send window,
  `rudp.rs:34`). **Chunk size barely moves throughput.**
- **900 KiB** as base64 is a 1,228,983-byte envelope, which exceeds the window,
  so the send fails.
- **Six 256 KiB chunks sent without waiting:** four accepted, two refused
  immediately, and all four accepted ones were delivered. This is Cut 10's F2
  (pipelined reads) reproduced at chunk scale. The window is per session and
  counts unacknowledged packets.

**Consequence for the design:** the fetch is sequential, one chunk in
flight, at 256 KiB. With `bin` payloads, as opposed to this probe's base64,
one chunk reply is 262,357 bytes, which leaves room for a control-plane
answer on the same session.

### F5. Moving one CultLib pin duplicates `cultcache-rs` even when its content is identical.

`pinprobe` (`cultcache-rs` at `a0813c6` beside `cultnet-rs` at `b3d9cf7`,
whose `cultcache-rs` content is byte-identical) reports **two**
`cultcache-rs v0.2.0` and two `cultcache-rs-derive` in `cargo tree -d`,
because Cargo keys a git source by revision.

Huginn has three pins at `a0813c6`: `huginn-mind`'s `cultcache-rs`,
`huginn-daemon`'s `cultnet-rs`, and the Epiphany leaf `epiphany-pipeline`'s
`cultcache-rs` (at leaf `5cda0886`). All three must move together, or "one
crate pins one revision" breaks (see `huginn-daemon/Cargo.toml` and the Cut 8
build check `cargo tree -p huginn-mind -e normal -d` with zero duplicates).
That is BP-2.

### F6. Huginn today.

The anchors in `huginn-daemon/src/serve.rs` at `202e5e3`:

- The module doc's stated limit is at `:10-17`, and the pipelined-read
  paragraphs are at `:32-44`.
- `MAX_FRAGMENT_BYTES`, `MAX_PENDING_RELIABLE_PACKETS` and `MAX_RESPONSE_BYTES`
  are at `:124-131`, and `bind` is at `:135-143`.
- `answer` (`:165-194`) routes `OperationRequest` and `SchemaCatalogRequest`,
  and answers every other family with `Error` (`:188-192`).
- **`within_window` (`:217-251`) is the single owner of the size decision
  today.** It encodes the envelope, admits it at `<= MAX_RESPONSE_BYTES`, and
  otherwise answers `Refused(ResponseTooLarge { bytes, limit })`.
- `run` (`:257-305`) reads the clock once per frame, at `:294`.

Elsewhere in the organ:

- `MindRefusal::ResponseTooLarge` is at `huginn-mind/src/refusal.rs:57-65`, and
  its doc names the daemon as owner.
- `HuginnMindResponse` is at `huginn-mind/src/wire.rs:81-90`, and `status` is
  at `:97-102`.
- The published schema is `schemas/cultnet/huginn.mind_response.v1.schema.json`,
  pinned byte for byte by `published_wire_schemas_match_derivation`
  (`wire.rs:210-216`).
- The tests that pin the gate are
  `an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client`
  (`serve.rs:548-656`) and
  `the_gates_boundary_is_the_transports_boundary_byte_for_byte`
  (`serve.rs:749-777`).
- The mutation entries D20, D20L through D20L5, D21 and D21L are in
  `tools/eureka-cut10-mutations.psd1:250-330`.
- `huginn-mind` has **no** `cultnet-rs` dependency today. The selection
  consumer cut adds one, because `Query { instance, selection }` carries the
  substrate's type (`docs/cultnet-selection-cut.md` §12).
- `eureka-state/src/lib.rs` is empty. Cut 13 has not been built.

## 2. The cut order

```
[selection Cut 1, Rust commit 3 lands on CultLib main]   (in Hands now, not this map)
        │
        ▼
BP-1  CultLib   the content plane in cultnet-rs (wire, manifest, pack, answer, verified fetch) + vectors both ways
        │
        ▼
BP-2  Epiphany leaf + Huginn   one pin move to a CultLib rev carrying selection Cut 1 and BP-1
        │
        ├──► [Huginn selection consumer cut]   (Epiphany map, unnumbered, not this map)
        ▼
BP-3  Huginn    an oversize answer is deferred to the body plane; the refusal becomes the backstop
        │
        ▼
Cut 11 / Cut 13 (Cut 13's spec gains: resolve a Deferred answer)
```

BP-3 comes **after** the Huginn selection consumer cut by default, for two
reasons. That cut already adds `cultnet-rs` to `huginn-mind`, which BP-3's
response variant needs. It also deletes `open_items` and `history`, so BP-3
tests fewer read operations. If the operator orders BP-3 first, BP-3 adds
that dependency itself (section 7, BP-3 "Adds") and nothing else changes.

## 3. Cut BP-1. The content plane in `cultnet-rs`

- **Repo and branch:** CultLib, `cultmesh/rust-content-plane`, from `main`
  **after selection Cut 1's Rust commit (its commit 3) has landed on `main`**.
  It depends on nothing else. See section 6 for why it cannot branch now.
- **First:**
  - Record `packages/cultnet-rs` baseline tests (51 in `tests/cultnet.rs`
    and 7 in `tests/rudp_server_hub.rs` at `b3d9cf7`; recount at the actual
    base).
  - Record the target-dir path count and size.
  - Capture the reference's bytes for every vector with the C# writer
    before writing any Rust.
- **Deletes first:** none. This cut fills an absence. No Rust code carries a
  body today, so nothing is stood beside, and there is no compensator to cut.
  This is stated rather than padded.
- **Keeps (untouched):**
  - Every C# source file. The reference does not change: its chunk classes,
    serializer arms (`CultNetSchemaMessageSerialization.cs:79-80`), CDN pack
    and validation, and servers stay as they are.
  - `CultNetDatabaseSubscribeMessage` and its `bodyIds`/`supportedBodyTransports`.
  - `CultMeshBodyDemand.cs` and `CultMesh.cs`.
  - `contracts/cultnet/*.schema.json` and `CultNetSchemaRegistry.cs`.
  - `cultmesh-rs` (Huginn does not depend on it, and this cut does not make it).
  - `rudp.rs` (the transport carries these as ordinary schema frames).
- **Adds:**

| Add | Owner | Live consumer | Protected invariant | Why an existing owner cannot serve |
|---|---|---|---|---|
| `contracts.rs`: `CultNetMessage::ContentChunkRequest { message_id, chunk_hash, record_key, expected_size_bytes: i32 }` and `ContentChunkResponse { message_id, found, chunk_hash, size_bytes: i32, payload: Vec<u8>, error }`, renamed to the reference's `schemaVersion`s, **both on the raw path** | `cultnet-rs` | Huginn's daemon (BP-3), `fetch_content` | **byte identity with the reference in both directions** | The closed enum cannot carry them (F1). The JSON-value path cannot carry `bin` (F1). |
| `src/content.rs`: `CultMeshCdnArtifactManifest` and `CultMeshCdnChunkRef` (the reference's names; named Rust fields; serde `into`/`from` a private tuple mirror, so every encoder writes the reference's array, F3) | `cultnet-rs` | BP-3's deferred response, the fetch | one reference vocabulary across runtimes; the array form survives `to_vec_named` | A Huginn-local reference type would be a second vocabulary for one thing |
| `normalize_hash` (mirrors `CultMeshCdn.NormalizeHash`, `CultMeshCdn.cs:549-560`: an optional case-insensitive `sha256:` prefix is stripped, then trimmed, then lowercased; empty is refused) | `cultnet-rs` | answer, fetch | one hash spelling | — |
| `pack_content(artifact_id, kind, version, mime_type, created_at_utc, bytes, chunk_size) -> (CultMeshCdnArtifactManifest, Vec<CultMeshCdnChunk>)` (mirrors `CultMeshCdn.PackArtifact`, `:232-310`) | `cultnet-rs` | BP-3's store | **the same bytes and options produce the reference's manifest byte for byte** | — |
| `validate_manifest` (mirrors `ValidateManifestShape`, `:499-526`, **including its sort by offset before the contiguity check**) and a caller-supplied `max_bytes` refused **before any fetch** | `cultnet-rs` | `fetch_content` | a hostile or corrupt manifest never makes a client allocate or ask | — |
| `answer_content_chunk_request(request, lookup: impl Fn(&str) -> Option<&[u8]>) -> CultNetMessage` (mirrors `CultMeshLegacyRudpContentServer.HandleAsync`, `CultMeshContentSessions.cs:43-86`, **in its validation order**: message id, hash normalisation, record-key agreement with `mesh:cdn:chunk:<hash>`, lookup, size and hash of what is served; a failure is `found: false`, an empty payload, and `error` = `<ExceptionKind>: <message>` in the reference's spelling) | `cultnet-rs` | BP-3's `serve::answer` | a server never serves bytes whose hash is not the requested one, and never answers a failure as silence | Rust has no `ICultNetSchemaServer`, so a pure function is the whole server |
| `fetch_content(manifest, max_bytes, ask: impl FnMut(CultNetMessage) -> Result<CultNetMessage>) -> Result<Vec<u8>>`: sequential, one chunk in flight, in manifest-offset order; verifies each response's correlation id, `found`, size and SHA-256 against the reference, then the whole body's size and SHA-256 against `contentHash` | `cultnet-rs` | Huginn's tests now, `eureka-state` (Cut 13) later | **one owner of verification on the receiving side**, as `CultMeshContentTransferService` is in the reference; no caller re-derives a check | The reference's transfer service is durable and file-promoting, and a C# owner does not serve Rust. This is its in-memory subset, with no resume, no failover and no files |
| `lib.rs:21-39`: `mod content; pub use content::*;` | `cultnet-rs` | — | — | — |
| `tests/content.rs` (new) | `cultnet-rs` | CI | the rules below | — |
| `contracts/cultmesh/content-vectors.cs-written.json` and `content-vectors.rs-written.json`: request, response found, response not found, and manifest vectors. A **deterministic** body (`byte[i] = i % 251`, 1,315,551 bytes, the real oversize case) at 256 KiB. **The fixture must carry:** a hash given with an uppercase `SHA256:` prefix; a manifest whose chunk list is **out of offset order**, which the reference accepts; a final chunk shorter than the rest; a zero-length body (0 chunks); and a chunk response whose `chunkHash` differs from its payload's hash in the **last** hex digit only. Otherwise a loosening that compares prefixes cannot fail. | each runtime writes its own | `tests/content.rs`, the C# test below | **vectors the reference writes decode and re-encode identically in Rust, and the other way round** | — |
| `tests/GameCult.Mesh.Tests/CultMeshContentVectorTests.cs` (**a new file**, not `NetworkingTests.cs`, which the selection cut is rewriting): writes the cs vectors under `CULTNET_WRITE_VECTORS=1`; asserts the committed cs vectors equal a fresh encode; decodes the rs-written vectors, asserts field equality and byte-identical re-encode; asserts `CultMeshCdn.PackArtifact` of the fixture body equals the fixture manifest | C# tests | CI | parity from the reference's side | — |
| Docs: `src/GameCult.Mesh/docs/transport-planes.md` cut line (`:93-101`) and `content-sessions.md` gain one paragraph each naming the Rust runtime's content path and its transport (the wording depends on **Q-BP1**); `docs/runtime-parity-scope.md` gets one row | docs | — | describe the live system | — |

No new dependency: `sha2`, `serde_bytes` and `rmpv` are already in
`cultnet-rs/Cargo.toml:14-31`. No new package, binary, transport, store
format or JSON schema file (see section 5).

- **Per-file changes** in `packages/cultnet-rs/src/contracts.rs` at `b3d9cf7`.
  Re-anchor at the base, because selection Cut 1 moves this file.
  - **`:252-392`**, the enum: two variants after `OperationResponse`.
    `payload` is `#[serde(with = "serde_bytes")]`, although the raw path
    never serialises it through serde.
  - **`:398-418`**, `parse_cultnet_message`: both `schemaVersion`s join the
    raw arm at `:405`.
  - **`:420-436`**, `encode_cultnet_message_for_wire`: both join the raw arm
    at `:427`.
  - **`:458-643`**, `validate_message`: two arms.
    - Request: `messageId` non-empty; `chunkHash` non-empty after
      normalisation; `expectedSizeBytes >= 0`.
    - Response: `messageId` non-empty. `found` implies
      `payload.len() == sizeBytes` and an empty `error`. Not `found` implies
      an empty payload.
    - These are the reference's own invariants, as its constructors and
      `HandleAsync` produce them. They are not new policy.
  - **`:771-810`**, `parse_raw_cultnet_schema_message`: two arms, reading
    `bin` via `as_slice` and ints via `as_i64` range-checked to `i32`.
  - **`:812-854`**, `encode_raw_cultnet_schema_message`: two arms writing keys
    **in the reference's declaration order** (F3), with the payload as
    `rmpv::Value::Binary`.
  - **`:964-1138`**, the `gamecult.networking.v0` path: unchanged. Its `_`
    arms already refuse. A test pins that refusal.
- **Authority map:**
  - **Owner:** `cultnet-rs::content` owns chunking, hashing, manifest shape,
    manifest validation, answering a chunk request and verifying a fetched
    body, for every Rust runtime. `contracts.rs` owns the two messages'
    spelling.
  - **Inputs:** bytes plus pack options (packing); a chunk request plus a
    lookup (answering); a manifest, a size cap and a transport closure
    (fetching).
  - **Outputs:** a manifest and chunks; a chunk response message; verified
    bytes or a typed error.
  - **Derived state:** `recordKey` is derived from the hash and is never an
    independent identity. `sizeBytes` and `contentHash` are derived from the
    bytes.
  - **Forbidden writers:** no consumer (Huginn, `eureka-state`) hashes,
    chunks, validates a manifest or checks a chunk itself. No consumer
    re-orders or retries chunks behind `fetch_content`.
  - **Shared paths:** the daemon's answer and the client's fetch both call
    `normalize_hash`. `pack_content` and `validate_manifest` share the one
    contiguity rule.
  - **Deletion line:** none. This is the first owner.
- **Verification:**
  - **Builds** (PowerShell, `$env:CARGO_TARGET_DIR='C:\Users\Meta\.cargo-target-codex'`):
    `cargo test -p cultnet-rs --tests` from `packages/cultnet-rs`; then
    `dotnet test tests/GameCult.Mesh.Tests --filter CultMeshContentVectorTests`.
  - **Tests, each with the rule it pins and the mutants that must kill it**
    (committed in `scripts/mutate-cultmesh.mjs`, CultLib's own runner, which
    owes the Eureka harness contract by name, **once the QUIC campaign has
    released that file**; see section 6):
    - `reference_vectors_encode_byte_identically` pins byte identity of the
      two messages and the manifest.
      Mutants:
      - swap two keys in `encode_raw`'s request arm (revert-shaped);
      - write `payload` as a `str` or an array of ints (loosening);
      - encode the manifest through a named-field struct, i.e. drop the tuple
        mirror (loosening, killed because `to_vec_named` then writes a map).
    - `rust_vectors_decode_in_the_reference` (C#) pins the direction the
      reference reads.
      Mutant: Rust writes `sizeBytes` as a `u64`, which MessagePack-CSharp
      rejects into `int`.
    - `a_chunk_response_carries_bin_through_the_raw_path` pins F1's failure
      mode.
      Mutant: remove the response from the raw arm at `:405`. It must fail
      with the byte-array error.
    - `normalize_hash_matches_the_reference` pins the reference's
      normalisation.
      Mutants:
      - case-sensitive prefix strip;
      - no lowercasing;
      - no trim (each killed by the fixture's `SHA256:` uppercase vector).
    - `a_manifest_out_of_offset_order_is_accepted_as_the_reference_accepts_it`
      pins parity of the sort.
      Mutant: drop the sort. It must fail, because the fixture's chunk list
      is shuffled.
    - `a_manifest_over_the_cap_is_refused_before_any_request` pins that the
      cap precedes fetching.
      Mutants:
      - check the cap after the first chunk (the test counts closure calls,
        expecting 0);
      - compare `>` instead of `>=` at the boundary.
      The cap's boundary vector is exactly `max_bytes` accepted and one byte
      over refused. **At least one mutant must be a function of the input**:
      `max_bytes * 2`. The probe sizes sit where doubling would admit them.
    - `a_chunk_whose_hash_differs_in_the_last_digit_is_refused` pins whole-hash
      comparison.
      Mutants:
      - compare the first 63 characters;
      - compare lengths;
      - compare prefixes.
      All are killed only because the fixture pair differs in the last digit.
    - `a_body_whose_chunks_verify_but_whole_hash_does_not_is_refused` pins the
      whole-body check.
      Mutant: skip the final `contentHash` comparison. The fixture substitutes
      a manifest whose chunk refs are valid but whose `contentHash` is another
      body's.
    - `a_response_for_another_message_id_is_refused` pins correlation.
      Mutant: skip the id check.
    - `answer_refuses_a_record_key_that_disagrees_with_its_hash` pins parity
      with `HandleAsync`'s third check.
      Mutant: delete the record-key check.
    - `answer_serves_found_false_with_the_reference_error_spelling` pins that a
      failure is an answer.
      Mutant: return `Error` instead.
    - `gamecult_networking_contract_refuses_content_messages` pins that the
      legacy contract does not grow them.
      Mutant: add them to its encoder.
    - A **no-op control** rewrites each target through the runner's I/O path
      and must leave everything green.
  - **Negative checks:**
    - `git diff --stat main -- src/GameCult.Networking src/GameCult.Mesh/*.cs contracts/cultnet packages/cultmesh-rs packages/cultnet-rs/src/rudp.rs`
      is empty.
    - `rg -n "untagged" packages/cultnet-rs/src/content.rs` is empty.
  - **Operator:** nothing to click. This cut's evidence is the two vector files.
- **Subtraction estimate:** removes 0 lines. Adds about 330: `content.rs`
  about 190, `contracts.rs` about 90, `lib.rs` 2, and docs about 20. Tests add
  about 260 Rust and 90 C#, plus two vector files. There are no dependencies,
  targets or formats to remove or add. **The delta is net positive because it
  buys an explicitly ruled capability with the smallest surface that carries
  it: the reference's own wire and reference type, not a new one.**
- **Build budget:**
  - Host and target are both the Windows workstation, debug.
  - Rust: `cultnet-rs` library and tests only, in the warm codex target dir.
    The probe's release build of the same crate by path added 945 files and
    0.32 GiB (12.31 to 12.63 GiB) cold. The expected debug delta is up to
    +1,000 paths and about +0.4 GiB. Nothing is cleaned.
  - C#: `GameCult.Mesh.Tests`, which rebuilds Networking, Mesh and Caching as
    dependencies with no source change in them.
  - No native build, no TypeScript, no Python, no workspace-wide build.
  - C: had 258 GB free.
  - The Linux target (Yggdrasil) is not exercised here. Nothing in this cut
    is platform-specific, and Cut 14 builds on Yggdrasil.

## 4. Cut BP-2. One pin move

- **Repo and branch:** Epiphany `codex/eureka-pipeline-state` and Huginn
  `eureka/memory-organ`. It depends on BP-1 **and** selection Cut 1 being on
  CultLib `main`, pushed.
- **Why it is its own cut:** F5. Moving `cultnet-rs` alone duplicates
  `cultcache-rs`. The selection consumer cut needs the same move, so **one**
  move to one revision carrying both CultLib changes serves both Huginn cuts.
  That saves a second leaf release and a second Huginn bump. Self schedules
  it once, whichever Huginn cut runs first.
- **Changes:**
  - Epiphany `epiphany-pipeline/Cargo.toml` `cultcache-rs` rev: from
    `a0813c6…` to the new CultLib SHA. This is one commit. The leaf's own
    tests must stay green. `cultcache-rs` content is unchanged across
    `a0813c6..b3d9cf7`, so any change after that is itself a finding.
  - Huginn: `crates/huginn-mind/Cargo.toml` `cultcache-rs` rev and
    `epiphany-pipeline` rev to the new leaf SHA, and
    `crates/huginn-daemon/Cargo.toml` `cultnet-rs` rev. This is one commit.
- **Verification:**
  - `cargo tree -p huginn-daemon -e normal -d` shows zero `cultcache-rs` or
    `cultnet-rs` duplicates.
  - The full Huginn suite is green: 74 tests at `202e5e3`.
  - The cut8, cut9 and cut10 mutation suites rerun clean.
  - `rg "a0813c6" F:\Projects\Huginn\crates F:\Projects\Epiphany\epiphany-pipeline`
    is empty.
- **Subtraction:** 0 or 0. Only four revision strings change.
- **Build budget:** Huginn workspace check and tests, and `epiphany-pipeline`
  library tests. Each is a cold compile of three CultLib crates at a new
  revision, about +300 to +600 paths. The same host as before.

## 5. Cut BP-3. Huginn defers an oversize answer to the body plane

- **Repo and branch:** Huginn `eureka/memory-organ`. It depends on BP-2 and,
  by default, on the Huginn selection consumer cut (section 2). It needs
  **Q-BP2** answered, and **Q-BP1** decides one paragraph of its module
  documentation.
- **First:** rerun
  `an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client`
  and record the two sizes it prints. At `202e5e3` they are 1,315,551 bytes
  for the wide view and 1,189,495 for the fitting one. They are the
  before-and-after witness.
- **Deletes first:**
  - `serve.rs:217-251` `within_window` is **deleted whole**. It is the one
    owner of the size decision, and it is replaced by `deliver` (below), not
    wrapped. A gate that refuses and a second gate that defers would be two
    owners of one decision.
  - `serve.rs:10-17`, the stated limit ("This is the current bound and not a
    design target… expect the number to move"), is deleted. The number no
    longer moves, because the window stops being the organ's delivery limit.
  - In `tools/eureka-cut10-mutations.psd1:250-330`, entries D20, D20L, D20L2,
    D20L3, D20L4, D20L5, D21 and D21L are **deleted from the cut10 suite and
    re-authored in BP-3's suite** against the new spelling. Their `Old`
    anchors name `within_window`'s lines, which no longer exist, so leaving
    them would make the cut10 runner fail on anchor-not-found rather than on
    a rule.
- **Keeps:**
  - `MAX_FRAGMENT_BYTES`, `MAX_PENDING_RELIABLE_PACKETS` and
    `MAX_RESPONSE_BYTES` (`:124-131`): the window is still what one send
    carries.
  - `MindRefusal::ResponseTooLarge { bytes, limit }` keeps its name and
    shape; **its doc is re-scoped** (below).
  - The pipelined-read paragraphs `:32-44`, unchanged and still true:
    deferral does not fix F2.
  - `encode_or_fail` (`:197-215`).
  - The `Daemon` dispatch. `daemon.rs` is untouched: **the mind and the pure
    dispatch know nothing about deferral.**
- **Adds:**

| Add | Owner | Live consumer | Protected invariant | Why an existing owner cannot serve |
|---|---|---|---|---|
| `huginn-mind/src/wire.rs:81-90`: `HuginnMindResponse::Deferred(DeferredAnswer)` with `DeferredAnswer { manifest: CultMeshCdnArtifactManifest }` under Q-BP2 A, or plus the typed summary under B; `status()` (`:97-102`) answers `accepted` for it. The manifest's schema is embedded through a `#[schemars(schema_with)]` field attribute returning a `$ref` to its published schema, as the selection consumer cut now does for `Selection` (corrected 2026-09-22: a hand-written `impl JsonSchema` for a foreign type is `E0117`, the orphan rule). `schemas/cultnet/huginn.mind_response.v1.schema.json` is regenerated. **The schema id stays `v1`**, because no client of it exists (Cut 13 is unbuilt) | `huginn-mind` (types only) | the daemon, Cut 13 | the reference rides the response schema, so there is no second vocabulary for a client | `HuginnMindResponse` is the only response a client decodes |
| `huginn-daemon/src/bodies.rs` (new), `DeferredBodies`: a map from chunk hash to bytes with reference counts, and a map from content hash to `(chunk hashes, bytes, last_touched)`. `retain(manifest, chunks, now)`, which is idempotent on the content hash and refreshes it; `chunk(hash, now) -> Option<&[u8]>`, which refreshes the owning bodies; `expire(now)`; eviction least-recently-touched first when the total is over budget. Constants: `DEFERRED_CHUNK_BYTES = 256 * 1024`; `MAX_DEFERRED_BODY_BYTES = 64 MiB`; `DEFERRED_BUDGET_BYTES = 256 MiB`; `DEFERRED_TTL = 60 s`, which is twice `session_timeout`, and is a field of `ServeOptions` (`:70-79`) with that default | `huginn-daemon::serve` | `deliver`, `answer` | retention is bounded in bytes and time, and a body is never evicted while its chunks are being served within the TTL | The mind must not hold transport artifacts. `cultnet-rs` owns no retention policy, and should not, because the policy is this service's memory budget |
| `serve.rs`: `deliver(reply, message_id, operation, runtime_id, bodies, now)` replaces `within_window`. It encodes the envelope. If the envelope is `<= MAX_RESPONSE_BYTES`, the reply is sent unchanged. Otherwise it takes the **payload bytes**, the named MessagePack of the response. If they are `<= MAX_DEFERRED_BODY_BYTES`, it calls `pack_content("huginn.mind_response", "package", "", "application/vnd.gamecult.huginn.mind-response+msgpack", now, payload, DEFERRED_CHUNK_BYTES)`, then `bodies.retain`, and answers `Deferred`. Otherwise it answers `Refused(ResponseTooLarge { bytes: payload_len, limit: MAX_DEFERRED_BODY_BYTES })` | `huginn-daemon::serve` | `answer` | **the one owner of how an answer is delivered.** Three outcomes, in that order, and nothing else decides | — |
| `serve.rs:165-194`, `answer`: a new `CultNetMessage::ContentChunkRequest` arm calls `cultnet_rs::answer_content_chunk_request(request, \|h\| bodies.chunk(h, now))`. The error text at `:188-192` names the third family. The signature gains `bodies: &mut DeferredBodies`, and `run` (`:257-305`) owns one and passes it | `huginn-daemon::serve` | the client's fetch | a chunk is answered on the requester's own session, like every other frame | — |
| `refusal.rs:57-65`, doc only: `ResponseTooLarge` is now "the answer exceeds the largest body the organ will deliver by any plane", `limit` is `MAX_DEFERRED_BODY_BYTES`, and `bytes` is the encoded answer (the payload, not the envelope). It is still the daemon's, and still never raised by the mind | `huginn-mind` | clients | **the backstop stays, per the ruling** | — |

- **Authority map:**
  - **Owner:** `huginn-daemon::serve::deliver` decides, per answer, between
    delivering it whole, deferring it, or refusing it. `DeferredBodies` owns
    retention. `cultnet-rs::content` owns chunking, hashing, the manifest and
    chunk answering.
  - **Inputs:** the encoded reply, the window constants, the deferral bound,
    the store's state and the per-frame clock read (`serve.rs:294`, still the
    crate's only one).
  - **Outputs:**
    - The reply, unchanged.
    - `Deferred { manifest }` plus retained chunks.
    - `Refused(ResponseTooLarge)`.
    - `ContentChunkResponse` for each chunk request.
  - **Derived state:**
    - The manifest and the chunk hashes are derived from the payload bytes.
    - The deferred body is **cache-only** (ephemeral transport state, like
      the reference's `CultMeshNetworkBodyStore`, `CultMeshBodySessions.cs:15-18`,
      "not a CultCache document store and not world truth").
    - The mind is not a writer or a reader of any of it.
  - **Forbidden writers:**
    - `Daemon::handle` and `Mind` never produce `Deferred` and never read the
      window.
    - No second gate: `within_window` is gone, and nothing else compares
      against `MAX_RESPONSE_BYTES`.
    - The client never re-derives a check: it calls `fetch_content`.
    - No pagination or truncation behind the caller's back (the Cut 10 F1
      ruling still holds).
  - **Shared paths:** every operation's reply passes through `deliver`,
    including `Whoami` and `Admit`. There is no per-operation path to
    deferral. Direct replies and chunk replies leave through the same
    `hub.send_schema_message` at `:295`.
  - **Deletion line:** `within_window` and the stated-limit paragraph are
    deleted before `deliver` is added, and the cut10 entries that anchored
    on them are removed in the same commit.
- **Verification:**
  - **Builds** (PowerShell, the codex target dir):
    - `cargo test -p huginn-mind --lib`;
    - `cargo test -p huginn-daemon --lib`;
    - `cargo check --workspace`;
    - `cargo tree -p huginn-daemon -e normal -d`, which must show no
      `cultcache-rs` or `cultnet-rs` duplicate.
  - **Tests, each with its rule and mutants.** They are committed in a new
    `tools/eureka-bodyplane-mutations.psd1` and run by the Eureka harness
    `C:\Users\Meta\.claude\skills\eureka\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn`,
    with a no-op control.
    - `an_oversize_answer_arrives_deferred_and_fetches_to_the_same_answer`
      rewrites `serve.rs:548-656` over a real loopback hub and a real mind.
      The wide view, at 1,315,551 bytes, comes back `Deferred`. The client
      runs `cultnet_rs::fetch_content` over the **same session**, one chunk
      in flight, and decodes the bytes as `HuginnMindResponse`. The result
      **equals** `daemon.handle` of the same request, and is `View(Some)` and
      not `Deferred`. The fitting view and `Whoami` still arrive directly.
      This pins the ruling.
      Mutants:
      - `deliver` refuses instead of deferring (revert to Cut 10's
        behaviour);
      - `deliver` defers at `MAX_RESPONSE_BYTES * 2` (a function of the input:
        the wide answer sits between 1x and 2x, so doubling lets it through
        whole and the send fails silently, and the test times out with no
        reply).
    - `the_delivery_boundary_is_the_transports_boundary_byte_for_byte`
      rewrites `serve.rs:749-777`. An envelope of exactly `MAX_RESPONSE_BYTES`
      goes direct; one byte more is `Deferred`, **not** refused. This pins
      that the threshold is the window.
      Mutants:
      - `<` for `<=` (off by one);
      - measure the payload and not the envelope (the old D20L5, re-authored);
      - `MAX_RESPONSE_BYTES - MAX_FRAGMENT_BYTES` (a function of the constant
        that a "safety margin" edit would produce).
    - `an_answer_over_the_deferral_bound_is_refused_by_name` uses a synthetic
      response sized exactly `MAX_DEFERRED_BODY_BYTES` (deferred) and one
      byte more (refused, with `bytes` = payload length and `limit` =
      `MAX_DEFERRED_BODY_BYTES`). This pins that the backstop stays and is
      measured against the deferral bound.
      Mutants:
      - `if true` (never refuse);
      - `limit: MAX_RESPONSE_BYTES` (the old limit leaking through);
      - `bytes: envelope_len` (the wrong measure);
      - `MAX_DEFERRED_BODY_BYTES * 4` (a function of the input).
    - `a_deferred_answer_is_never_itself_deferred` pins a rule written
      *inside* `deliver`: the deferred reply is always under the window, and
      a body never decodes to `Deferred`. Encode the `Deferred` reply at the
      bound (about 256 manifest refs) and assert it is `<= MAX_RESPONSE_BYTES`.
      Mutant: recurse `deliver` on the deferred reply.
    - `retention_is_idempotent_and_sliding` exercises `DeferredBodies` alone
      with an injected `now`. The same body deferred twice is stored once, and
      its budget use is counted once. A chunk served at `t = TTL - 1`
      refreshes the body, so it survives to `2 * TTL - 2`. An untouched body
      is gone at `TTL`.
      Mutants:
      - expiry from creation rather than last touch;
      - `>` rather than `>=` at the TTL boundary;
      - a TTL that is a function of the input (`ttl / 2`);
      - count bytes per retain instead of per body.
    - `eviction_is_least_recently_touched_and_bounded_in_bytes` uses three
      bodies over the budget. The one touched longest ago goes first, and a
      shared chunk survives while any owner lives.
      Mutants:
      - evict the newest;
      - evict by insertion order (killed because the fixture touches the
        oldest-inserted body last);
      - free a shared chunk with its first owner.
    - `an_evicted_body_answers_found_false_and_the_client_can_ask_again`
      pins lifecycle end to end.
      Mutant: `chunk()` returns stale bytes after eviction. That is
      impossible by construction, so the mutant used is "do not refresh on
      serve" at a TTL shorter than a fetch.
    - `the_mind_never_sees_a_deferral` is a negative test:
      `rg -n "Deferred|DeferredBodies|MAX_RESPONSE_BYTES" F:\Projects\Huginn\crates\huginn-mind\src`
      matches only the `wire.rs` variant and its schema test.
    - `published_wire_schemas_match_derivation` (`wire.rs:210-216`, kept)
      pins that the regenerated schema equals the derivation.
    - **Scenario sited at the decision:** the loopback test holds its client
      on the `Deferred` reply **before** fetching and asserts the manifest's
      `sizeBytes` equals the direct `daemon.handle` payload's length. That
      observation is at the decision, not downstream of the fetch (the
      skill's "put the observation where the rule is decided").
  - **Negative checks:**
    - `rg -n "fn within_window" F:\Projects\Huginn\crates` is empty.
    - `rg -n "sha2|Sha256" F:\Projects\Huginn\crates\huginn-daemon\src` is
      empty, because hashing belongs to `cultnet-rs`.
    - Verified to not collide: `huginn-mind`'s `sha2` is for receipts, in
      another crate.
  - **Operator:** none until Cut 14. The real-network check (WireGuard to
    Yggdrasil, an oversize read end to end) is Cut 14's runbook step and is
    recorded there.
- **Subtraction estimate:**
  - Removes about 35 lines (`within_window` 35, the stated-limit doc 8) and
    8 mutation entries.
  - Adds about 190 lines: `bodies.rs` about 110, `deliver` about 35, the
    `answer` arm 8, the wire variant with its `schema_with` `$ref` about
    35.
  - Tests are about +220 net over the rewritten two.
  - Entries: 8 removed, about 20 added.
  - No dependency is added: `cultnet-rs` is already a daemon dependency, and
    `huginn-mind` already has it after the selection consumer cut. If BP-3
    runs first, it adds that dependency to `huginn-mind`, and that is the one
    listed cost.
- **Build budget:**
  - `huginn-mind` and `huginn-daemon` library and tests, warm after BP-2.
  - Expected +50 to +200 paths.
  - Host and target are both the workstation. The unix arm is not exercised,
    and Cut 14 builds on Yggdrasil.

## 6. Interaction with CultNet selection Cut 1, and sequencing

Selection Cut 1 is in Hands on `cultnet/selection-cut1` in the locked worktree
`.claude/worktrees/agent-af580008561975a4c`, based at `b3d9cf7`.

**What it touches, from its own map:**

- `CultNetDatabaseSubscribeMessage`: v1 classes beside v0. It **keeps** the
  body-plane demand fields `bodyIds` and `supportedBodyTransports`, which its
  §5 lists as not selection.
- `CultMeshBodyDemand.cs:177-221` and `CultMesh.cs` body and snapshot
  selector construction.
- `CultNetSchemaRegistry.cs:360-385`.
- `CultNetSchemaMessageSerialization.cs:9-85`.
- `NetworkingTests.cs`.
- In Rust: `packages/cultnet-rs/src/contracts.rs:314-331`, which gains
  `SnapshotRequestV1`, `DatabaseSubscribeV1` and `SnapshotResponseRawV1`. The
  last is a **raw** message, so it edits the same `parse_raw` and
  `encode_raw` dispatch this map edits.
- `lib.rs` re-exports, the new `selection.rs`, and the vector writer pattern
  under `CULTNET_WRITE_VECTORS=1`.

**Semantic interaction: none.** `bodyIds` and `supportedBodyTransports` on the
subscribe message are demand for *live* bodies, the reference's body plane
proper (F2). This cut uses the content plane and never touches that message,
its schema, the body-demand code or the database subscription server and
client. A Huginn read is an operation request and not a subscription, so the
two cuts share no rule.

**Textual collision: certain, in five places:**

- the `CultNetMessage` enum;
- `parse_cultnet_message`'s raw arm;
- `encode_cultnet_message_for_wire`'s raw arm;
- `parse_raw` and `encode_raw`;
- `validate_message`;
- and `lib.rs`.

Two Hands in one working tree is forbidden, and two branches editing one
dispatch list is a merge that falls to whoever lands second.

**Sequence:**

1. Selection Cut 1 lands its Rust commit (its commit 3) on `main`.
2. BP-1 branches from that `main`. The anchors in section 3 are re-taken at
   that base, and the discrepancy is reported.
3. BP-1 lands.
4. BP-2 moves the pins once to a revision carrying both.
5. Then the Huginn selection consumer cut and BP-3, in that order by
   default.

BP-1 does **not** add JSON schema files or registry entries, so it does not
touch `CultNetSchemaRegistry.cs` or `contracts/cultnet/`, which selection
Cut 1 is editing. The absence of published schemas for the content messages
predates both cuts and is a follow-up (section 7).

**The mutation runner:** `scripts/mutate-cultmesh.mjs` is modified in the
working tree by the live QUIC campaign. BP-1 adds its entries only after that
file is committed or released, and never onto someone else's uncommitted
edit. If the QUIC campaign still holds it when BP-1 is ready, BP-1 carries
its entries in a new `scripts/mutate-cultnet-content.mjs` under the same
contract, and Self collapses the two later. That is a default, not a
question.

## 7. Deliberately not in scope

- **Pipelined chunk fetch, and Cut 10's F2 (pipelined reads).** The fetch is
  sequential. F4 shows the throughput barely depends on chunk size, and
  pipelining is the known way to lose a reply silently. A window-aware sender
  is a transport change in `rudp.rs`, not this cut.
- **Throughput.** About 0.85 MB/s on loopback (F4), set by the 32-packet
  pacing window. A 64 MiB body takes about 75 s. Recorded, not addressed.
- **A TCP or QUIC content connector in Rust, and Rust session identity with
  Odin-signed route certificates.** These are the reference's
  `CultMeshSessionIdentityClient` and `transport-planes.md:73-86`. Huginn's
  control plane has none of that today, and the body rides the same session
  with the same (absent) trust. **Trust for both planes is Cut 14's** (D8, the
  trust boundary).
- **Durable, resumable or file-promoted fetch** (the reference transfer
  service's resume state and `<content-hash>.body` promotion). An answer is
  re-asked, not resumed.
- **Publishing JSON schemas for `cultmesh.content_chunk_*`.** The reference
  never published them: there is no file in `contracts/cultnet/` and no entry
  at `CultNetSchemaRegistry.cs:360-385`. Follow-up: a CultLib parity fill for
  every runtime, after selection Cut 1 has released the registry.
- **The TypeScript, Python and Kotlin content plane.** It follows when one of
  them has a caller, the same rule the selection cut ruled.
- **The live body plane** (`body_read_*`, `CultMeshNetworkBodyStore`, body
  demand) in Rust.
- **Persisting deferred bodies across a daemon restart.**
- **Per-operation summaries,** unless Q-BP2 B is ruled. Row-level summaries
  are the selection vocabulary's projections.
- **Cut 13's client handling of `Deferred`.** Consequence for Cut 13's spec,
  to be carried into its refresh: every read tool resolves `Deferred` through
  `cultnet_rs::fetch_content` with a cap no larger than
  `MAX_DEFERRED_BODY_BYTES`, and treats `ResponseTooLarge` as "narrow the
  selection".
- **Consequence for Cut 14's spec:** the daemon's memory budget on Yggdrasil
  grows by up to `DEFERRED_BUDGET_BYTES` (256 MiB).

## 8. Operator questions

Two real forks. Every other choice above (the content plane over the body
plane per F2, the reference's manifest, the ephemeral store, the constants,
the order) is a default with its reason stated. It is recorded, not asked.

**Q-BP1: RULED A by the operator, 2026-09-22 ("A is fine here").**
- Chunk requests and responses travel on the session the client already
  holds, byte-identical on the wire to the reference's
  `CultMeshLegacyRudpContentServer`.
- BP-1 amends `transport-planes.md` to name this as Rust's *explicit* content
  path, never an implicit default.
- It lasts until Rust has an authenticated content connector, TCP+TLS or
  QUIC. That later move sits behind the same `fetch_content` owner and needs
  no change to the reference type.

*History, the question as asked:* **Q-BP1. Which physical path carries the body?**

CultMesh's own transport doctrine says "Schema messages must not carry bulk
bodies" and that the RUDP content path is legacy, "compatibility and parity
measurement" only (`transport-planes.md:90-96`). But the only session
Huginn and its client share is CultNet RUDP, and the doctrine's preferred
content plane (TCP byte stream) is plaintext and loopback-only until TLS
(`:103-106`), while Huginn serves remote clients over WireGuard.

- **A. Chunk request and response on the session the client already holds.**
  This is wire-identical to the reference's `CultMeshLegacyRudpContentServer`.
  `transport-planes.md` is amended to name it as the Rust runtime's
  *explicit* content path, never an implicit default, until Rust has an
  authenticated content connector. Cost: it re-legitimises, for Rust, a path
  the C# doctrine retired, and it inherits RUDP's pacing (F4).
- **B. Port the TCP content plane to Rust** (`cultmesh-content+tcp://`, a
  typed header and raw bytes). Faster, and outside the envelope. It is
  blocked for Huginn by the doctrine's own loopback-only gate until TLS
  exists, so it needs a TLS content connector as well: a much larger cut, and
  Huginn waits on it.
- **C. Wait for authenticated QUIC in Rust**, which does not exist. Huginn's
  oversize answers stay refused until then.

**Recommended: A.** It is the only option that delivers what the ruling
asked for on the substrate Huginn has. It reuses the reference's wire byte
for byte, and the amendment keeps the doctrine honest rather than quietly
contradicted. B and C are where the body moves later, behind the same
`fetch_content` owner and without changing the wire's reference type.

**What depends on it:** BP-1's documentation paragraph, whether BP-1 grows a
TCP connector (B), and whether BP-3 can land before a TLS campaign (B and C
block it).

**Q-BP2: RULED A by the operator, 2026-09-22 ("I accept your
recommendations").** The deferred answer carries the reference's own facts:
the manifest (`sizeBytes`, `contentHash`, the chunk list) and the operation.
It carries no per-operation summary. Row-level facts come from asking again
with a header projection. `huginn-mind` gains no summary types. B can be added
later on the selection projection if Cut 13's tools show the need.

*History, the question as asked:* **Q-BP2. What is "the summary" the control plane carries?**

- **A. The reference's own facts.** The manifest carries `sizeBytes`,
  `contentHash` and the chunk list, and the envelope carries the operation.
  A caller decides whether to fetch on size alone. Anything row-level (ids,
  statuses, `matched`) comes from asking again with a selection projection,
  which the selection cut already provides.
- **B. A typed per-operation summary produced by the mind:** for a view, the
  id, kind, status and admission facts without the document; for a page,
  `matched` and row references. The caller can act without fetching at all.
  Cost: new summary types and derivations in `huginn-mind`, one per read
  operation, re-specified whenever the selection consumer cut reshapes the
  reads, plus schema growth.

**Recommended: A.** It keeps deferral a pure transport decision owned by one
function that knows nothing about operations. A per-operation summary is a
second projection vocabulary beside the one the operator just ruled into
CultNet. If Cut 13's tools show that agents need to decide without fetching,
B can be added then, on the selection projection rather than beside it.

**What depends on it:** `DeferredAnswer`'s shape, whether `huginn-mind`
gains types, BP-3's size (B adds about 120 lines and a test per operation),
and Cut 13's tool output.

## 9. What was probed, what was only read, and what is not settled

- **Probed by running code:** F1 (decode refusals, plus the on-wire refusal),
  F3 (real reference bytes, byte identity of both messages and the manifest
  in Rust, the Rust re-pack equal to the reference's), F4 (whole-body send
  failure; sequential delivery at three chunk sizes with SHA equality and
  timings; the 900 KiB failure; the pipelining loss), and F5 (duplicate
  `cultcache-rs` under mixed revisions).
- **Read only, not run:**
  - C# `ValidateManifestShape` sorting and contiguity, `NormalizeHash`, and
    the legacy server's validation order. These are the rules the vectors
    will pin in BP-1.
  - The reference's TCP connector behaviour and the loopback-only gate, from
    `transport-planes.md`, not from running `CultMeshTcpContentTransport`.
  - Selection Cut 1's exact edit sites in the worktree. I read its map, not
    its uncommitted diff, and did not open the locked worktree's changes, to
    avoid reading another agent's working tree mid-edit.
- **Not probed:**
  - Throughput over WireGuard or to Yggdrasil (loopback only).
  - The 60 s TTL against a real slow link.
  - C# decoding **Rust-written** bytes. Byte identity makes it follow, but
    BP-1's C# test is the first thing that will actually run it.
  - Whether a hostile chunk flood can starve the control plane on one
    session (the window is per session, so the likely answer is that it can
    starve only the flooding session). Recorded for Cut 14's trust pass.
- **Substrate missing, recorded per the skill:** typed pipeline state. This
  map, its model page and its questions live in markdown because Huginn,
  which would hold them, is the thing being built.
