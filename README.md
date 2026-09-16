# Huginn

Huginn is the GameCult memory organ. It owns each agent instance's mind as
typed state: what an instance remembers, which of that memory is admitted to
steer its next action, and the provenance of every admitted claim.

Upstream: `https://github.com/GameCult/Huginn.git`

## Authority

- An instance owns its mind; Huginn owns the state. Huginn is the single
  writer of an instance's memory documents. No other service, script, or agent
  writes them.
- Memory documents are CultCache `.cc` state. Huginn persists them through
  CultLib's Rust runtime and publishes them as typed documents over CultNet.
- Huginn depends on Qdrant directly for retrieval. When Qdrant is unreachable
  Huginn refuses loudly; it does not fall back to a second store or a second
  writer.
- Mind state is not version-controlled. It lives in Huginn's store, not in
  any repository.

Generic `.cc` inspection is not Huginn's job. CultCache Studio inspects and
edits `.cc` state. Huginn reads and writes minds.

## Layout

A Cargo workspace of three crates:

- `crates/huginn-mind`: storage, identity, and admission of memory documents.
- `crates/huginn-daemon`: the CultNet surface and serve loop.
- `crates/eureka-state`: typed state for the Eureka pipeline.

`huginn-mind` is live: it opens one instance's store (an owned redb CultCache
at `<state_root>/minds/<instance>/mind.redb`, locked for the mind's lifetime),
and refuses a store whose `instance` document names another instance, whose
epoch record is foreign, or whose types are not a mind's. It admits batches
of pipeline documents through one commit path: the leaf's bounds, formats and
keys, then the organ's cross-document rules (references, in-force status,
the resolution matrix, derived resolutions and stewardships), then one
compare-and-swap that lands the batch whole with a receipt naming the exact
bytes it read and wrote. An exact replay answers with the stored receipt.
Document shape and keys come from `epiphany-pipeline`; the store is CultLib's
Rust CultCache.
`huginn-daemon` and `eureka-state` are stubs; the
campaign's cut map in `Epiphany/notes/eureka-pipeline-state-cut.md` owns what
each crate must do next.

```powershell
cargo check --workspace
```

## Persona

Huginn's legacy repo Persona lives under `.voidbot/`. Its `state/huginn.cc`
holds legacy `void.*` documents and its `voice/identity.json` names the
Persona. The two disagree about jurisdiction; that migration belongs to the
portable-Persona work, not to this workspace, and nothing here reads or writes
`.voidbot/`.
