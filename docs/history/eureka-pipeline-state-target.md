<!-- Provenance: copied verbatim from GameCult/Epiphany ef956865 notes/eureka-pipeline-state-target.md; text below this line is unchanged. -->
# Eureka pipeline state: target

Status: target (the ends). The means live in `notes/eureka-pipeline-state-cut.md`,
written by Imagination. Operator rulings are dated 2026-09-15 and 2026-09-16.

**This target was rewritten on 2026-09-16** after the operator rejected the
ownership model it had been built on. Rulings 5, 6, 10 and 13's store clauses,
which put the store in the task's repo under a per-clone lease, are superseded
by rulings 14-17 below. The cut map's Cut 3 sections describe the old model and
are history, not live design.

## Why

Eureka is the Claude Code counterpart of this organism. It is a skill,
published as `GameCult/Eureka` and installed at `~/.claude/skills/eureka`, that
runs the same faculties (Self, Imagination, Hands, Soul, Mind Steward) over
foundation changes. In its first campaign, the Aetheria CultCache migration, the
pipeline worked, but its findings lived in prose: the cut map, subagent reports
and memory files. Self re-read and re-summarised that prose at every step. Stale
claims survived until a steward caught them. The postmortem needed a transcript
crawl, and one operator quote survived only in a compaction summary.

Epiphany already has the other half: typed documents, admission and receipts.
Her own `notes/faculty-workflow-lessons-2026-09-04.md:146-176` proposes typed
forms of Eureka's habits (`Map`, `Spec`, `CutReport`, `Verdict`, rulings, a
landed-names digest), but none of them are built.

This campaign builds them once and gives Eureka agents access to them.

## What the first model got wrong

The store was owned by a repo. Nothing owned the state, so the machinery grew to
compensate: a per-clone writer lease, git-directory resolution, committed
`.gitattributes` checks, and a merge tool for divergent copies. Three Soul passes
went into propping that up, and it still resolved a store into a directory the
operator never named.

Mind state also does not want version control. Git offers branches, merges and
history rewriting; a mind wants one owner and an append-only record.

The operator's correction: **someone owns the state**. An instance owns its mind,
and stewardship over repos is an assignment, not an identity. That is Epiphany's
own design, and Eureka inherits it.

## Operator rulings

1. **Epiphany owns the schemas** for pipeline state.
2. **Searchable state serves agents first.** Self and new agents rehydrate from
   it ("what rulings govern stores?", "what is still open?"). Soul and
   Imagination check precedent in it ("have nil map keys bitten us?"). Operator
   browsing across projects comes later.
3. **Two campaigns.** This one covers the schemas, the memory organ, admission,
   the Eureka MCP client and a proof on one real Eureka task. The second,
   Epiphany adopting Eureka's habits, comes after the schema stops moving.
4. **Re-pin first.** Landed: Epiphany moved from CultLib `e171eca3` to
   `a0813c6`.
5. **Q1-Q5 (2026-09-15), of which only Q2 and Q5 survive the rewrite:** additive
   schema changes keep the epoch and a breaking bump refuses the old store
   (Q2); `schemars` is an unconditional `epiphany-core` dependency and the
   published JSON schemas are derived from the Rust types (Q5).
6. **The operator channel is the Claude Code session.** Eureka has no Persona.
   Self is the operator surface, because spec iteration works best where the
   question and the tree share a context. A blocking question may be pushed
   through whatever notification MCP the user configures; Eureka owns no
   transport and names no provider. Answers come back in the session. Answering
   over a chat channel is a later campaign and needs identity binding first.
7. **Eureka is published** as `GameCult/Eureka`, MIT.

### Ownership (2026-09-16)

14. **An instance owns its mind.** A store is canonical to exactly one instance.
    Identity lives in the state, not in a path. Admission refuses a write
    carrying another instance's identity, whatever the transport. Stewardship
    over repos is an assignment recorded in that mind, and one instance may
    steward several repos. Reassignment is an explicit typed hand-off recorded
    in both minds, so history stays attributable.
15. **A service owns the state, on Yggdrasil.** The memory organ is a daemon: it
    outlives every session, owns a real resource dependency, and isolates a dead
    session from a corrupted mind. It earns its process under the GameCult
    daemon test. Minds are no longer committed to git, so there is no per-clone
    lease, no git-attribute precondition, and no divergence between clones.
16. **The organ depends on Qdrant directly, not on voidbot.** It owns its own
    collections and indexes at admission time. voidbot keeps its read-only
    public-repo retrieval; it is not in this path. Embeddings come from the
    Ollama endpoint the map names. Epiphany's own service embeds against a
    Yggdrasil-local endpoint with the same model, and only voidbot uses
    Nightwing; follow the local precedent and name the endpoint, not the host.
17. **Huginn is the memory organ.** The dormant `.cc`-to-Eve CLI is retired and
    generic `.cc` inspection belongs to CultCache Studio in CultLib. Huginn
    becomes the Rust service that owns instance minds, admission and the index.
    That gives Huginn a real runtime for the first time, but it closes the
    authority vacancy only for pipeline state: doctrine still names Huginn the
    Persona-state steward, and this campaign builds none of that. Recorded as a
    follow-up, not solved here.

### Shape decided with those rulings

- **Transport:** the organ speaks CultNet. `eureka-state` stays a local stdio MCP
  server that is a thin CultNet client, so the MCP surface stays swappable and
  the service protocol stays typed.
- **Unreachable organ:** refuse loudly and let the campaign stall on that step.
  No local spool, because that reintroduces two writers.
- **Cut 3a code:** keep the document kinds, keys, validation and the refusals
  that survive the layer. Six of the thirteen typed refusals exist only for the
  repo store and die with it. Delete the store, lease and git layers, and
  re-point admission at the organ.

## End state

- **Documents.** Epiphany defines the typed pipeline documents: campaign, target,
  question, ruling, cut spec, cut report, verdict, finding, follow-up and
  resolution, plus the three ruling 14 requires: instance, stewardship and
  hand-off. Each has a stable schema id and version, a derived key, and a live
  Eureka consumer. Schemas are derived from the Rust types and published in
  `schemas/cultnet/index.json`.
- **Instance identity.** A mind carries the instance that owns it and the repos
  it stewards. A campaign names its repo; the mind holds campaigns across every
  repo that instance stewards.
- **The organ (Huginn).** One service owns every mind it hosts:
  - admission, with per-document rules and typed refusals;
  - exact and filtered queries (by campaign, repo, cut, kind, status,
    supersession, time), "open items", and "rulings in force";
  - semantic search over its own Qdrant collections, indexed at admission;
  - a typed hand-off for reassigning stewardship, and an import path for
    another instance's mind. Import is not a merge: with one owner per mind,
    two copies cannot diverge, so nothing needs reconciling;
  - supervised by Idunn, with a named backup owner for its volume.
- **Eureka.** The skill's briefs tell Self, Imagination, Hands and Soul to admit
  and query typed state instead of relaying prose. `eureka-state` is the thin
  MCP client, registered at user scope.
- **Proof.** One real Eureka task runs end to end on typed state: rulings
  admitted, a cut spec, Hands' cut report, Soul findings, follow-ups queried
  back, and a fresh agent rehydrating from queries alone.

## Invariants

- **One owner.** Exactly one instance owns a mind, and the organ is its only
  writer. Eureka agents and the MCP client are clients; the index is a
  projection.
- **Identity travels with the write.** An admission carries the instance, and a
  foreign instance is refused.
- **Typed, not prose.** No JSON store and no blob fields where a typed field
  exists. JSON appears only in published schemas.
- **Mind state is not in version control.** Campaign prose (target, cut map) stays
  in the repo; the mind does not.
- **Supersession, not deletion.** An old ruling or finding stays queryable as
  superseded.
- **The skill defers to the schema.** Eureka describes how to use the state; the
  schema set and admission rules are owned here.
- **Availability is honest.** When the organ is unreachable, Eureka refuses and
  says so. It never writes a second copy.

## Not in scope

- Epiphany's own organs consuming pipeline documents: the second campaign.
- Operator browsing surfaces (Eve/CultUI) over pipeline state.
- Answering operator questions over a chat channel.
- Migrating the Aetheria CultCache campaign's prose into typed documents.
- Cleanup of `.epiphany-run/` (105 GiB) and the repo-local `target/` (39 GiB).
- Huginn's legacy `.voidbot` Persona state, which needs its own admission
  decision.

## Evidence

- Substrate maps (scratch, 2026-09-15): Epiphany typed state and admission; the
  MCP and client landscape.
- Dated comparison: `~/.claude/skills/eureka/references/epiphany-comparison-2026-09-15.md`.
- Campaign precedent: `F:\Projects\Aetheria\docs\cultcache-migration-postmortem.md`.
- Huginn's dormancy and authority vacancy:
  `F:\Projects\gamecult-ops\docs\repo-census-2026-09\repos\Huginn.md`.
