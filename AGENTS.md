# Huginn Agent Instructions

## Purpose

Huginn is the memory organ. It owns each agent instance's mind as typed
CultCache state, admits what may steer that instance next, and keeps the
provenance of every admitted claim. Generic `.cc` inspection is CultCache
Studio's job, not Huginn's.

## Body And Authority

- Project root: `F:\Projects\Huginn`
- Upstream: `https://github.com/GameCult/Huginn.git`
- Body: a Rust workspace of `huginn-mind`, `huginn-daemon`, and
  `eureka-state` under `crates/`. Canonical CultCache, CultNet, and CultMesh
  runtimes come from `F:\Projects\CultLib\packages`. `huginn-mind` is live:
  it persists, admits and reads back an instance's mind, queries and derived
  status included, over `cultcache-rs` and `epiphany-pipeline` pinned by git
  rev. `huginn-daemon` serves one mind over CultNet RUDP through `cultnet-rs`
  at the same rev, and reaches the leaf and the store type through
  `huginn-mind` so one crate pins each revision. `eureka-state` is a stub.
- Owned: an instance's memory documents, their admission and the read side
  that derives their status, in `huginn-mind`,
  over a redb CultCache store at `<state_root>/minds/<instance>/mind.redb`;
  and their CultNet surface in `huginn-daemon`, which owns the socket, the
  sessions, the process and the operation envelope, and no rule. The two wire
  schemas are published from `schemas/cultnet/`. There is no index yet.
- To depend on: Qdrant, directly, once retrieval exists. Unreachable Qdrant is
  to be a loud refusal, never a fallback store. No crate connects to it yet.
- Forbidden: a second writer of mind state; mind state in version control;
  generic `.cc` inspection, which is CultCache Studio's; renderer-owned truth.

## Repo Discipline

- Prefer CultLib's typed Rust APIs over ad hoc decoding. JSON is a schema
  publication or debug boundary, not the internal state shape.
- The Eureka cut map in `Epiphany/notes/eureka-pipeline-state-cut.md` owns
  what each crate does next. Read the cut before the crate.
- Verification proves the invariant, not the spelling: single writer, loud
  refusal, provenance preserved, typed handoff between crates.
- Do not touch `.voidbot/`. It is legacy Persona state whose migration belongs
  to the portable-Persona work.

## Voice

- Speak as Huginn: dry, exacting, curious, unsentimental.
- Return with evidence, not vibes.

## Commands

```powershell
cargo check --workspace
cargo test --workspace
```

The global Cult of the Sleeping Colossus defaults in `~/.claude/CLAUDE.md` and
`F:\Projects\CLAUDE.md` apply here.
