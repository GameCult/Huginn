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

The crates are stubs. Each cut of the Eureka pipeline-state campaign fills one
in; the campaign's cut map in `Epiphany/notes/eureka-pipeline-state-cut.md`
owns what each crate must do next.

```powershell
cargo check --workspace
```

## Persona

Huginn's legacy repo Persona lives under `.voidbot/`. Its `state/huginn.cc`
holds legacy `void.*` documents and its `voice/identity.json` names the
Persona. The two disagree about jurisdiction; that migration belongs to the
portable-Persona work, not to this workspace, and nothing here reads or writes
`.voidbot/`.
