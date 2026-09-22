# Huginn

Huginn is the GameCult memory organ. It owns each agent instance's mind as
typed state: what an instance remembers, which of that memory is admitted to
steer its next action, and the provenance of every admitted claim.

Upstream: `https://github.com/GameCult/Huginn.git`

## Authority

- An instance owns its mind; Huginn owns the state. Huginn is the single
  writer of an instance's memory documents. No other service, script, or agent
  writes them.
- Memory documents are CultCache state. `huginn-mind` persists them through
  CultLib's Rust CultCache into a redb store at
  `<state_root>/minds/<instance>/mind.redb`. `huginn-daemon` serves one such
  mind over CultNet RUDP: admission and the read side ride
  `cultnet.operation_request.v0` and `cultnet.operation_response.v0`, and the
  two wire schemas answer a schema catalog request.
- Retrieval is to depend on Qdrant directly, refusing loudly when Qdrant is
  unreachable rather than falling back to a second store or a second writer.
  No crate opens that connection yet.
- Mind state is not version-controlled. It lives in Huginn's store, not in
  any repository.

Generic `.cc` inspection is not Huginn's job. CultCache Studio inspects and
edits `.cc` state. Huginn reads and writes minds.

## Layout

A Cargo workspace of three crates:

- `crates/huginn-mind`: storage, identity, admission, and queries and derived
  status over memory documents.
- `crates/huginn-daemon`: the CultNet surface. The socket, the sessions, the
  process and the operation envelope, and no rule.
- `crates/eureka-state`: a stub. It will carry typed state for the Eureka
  pipeline; today it holds none.

`huginn-mind` is live: it opens one instance's store (an owned redb CultCache
at `<state_root>/minds/<instance>/mind.redb`, locked for the mind's lifetime),
and refuses a store whose `instance` document names another instance, whose
epoch record is foreign, or whose types are not a mind's. It admits batches
of pipeline documents through one commit path: the leaf's bounds, formats and
keys, then the organ's cross-document rules (references, in-force status,
the resolution matrix, derived resolutions and stewardships), then one
compare-and-swap that lands the batch whole with a receipt naming the exact
bytes it read and wrote. An exact replay answers with the stored receipt.
It reads them back the same way: a document with the facts of its admission
joined from that receipt and its status derived at read time, and typed
queries over one mind. Status is never stored, and the views derive it
through the same rules admission does. Document shape and keys come from
`epiphany-pipeline`; the store is CultLib's Rust CultCache.

`huginn-daemon` opens one mind, binds one UDP socket in that order, and
answers every frame on the session it arrived on: one operation per `Mind`
method plus `whoami`, whose payload is the mind's own types as named
MessagePack. A refusal is an answer with a `rejected` status, never a
transport error; an envelope that does not decode is answered with a typed
failure and reaches no mind. The two schemas it publishes live in
`schemas/cultnet/` and are pinned to their derivation by a test. There is no
index yet.

`eureka-state` is a stub; the campaign's cut map in
`Epiphany/notes/eureka-pipeline-state-cut.md` owns what each crate must do
next.

```powershell
cargo check --workspace
cargo run -p huginn-daemon -- --state-root <abs> --instance <slug> --bind 127.0.0.1:17872
```

## Persona

Huginn's legacy repo Persona lives under `.voidbot/`. Its `state/huginn.cc`
holds legacy `void.*` documents and its `voice/identity.json` names the
Persona. The two disagree about jurisdiction; that migration belongs to the
portable-Persona work, not to this workspace, and nothing here reads or writes
`.voidbot/`.
