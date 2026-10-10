# Prose knowledge: target

Serves `eureka-body:follow_up:prose-knowledge-inventory` under operator ruling
`eureka-body:ruling:prose-knowledge-into-mind` (2026-10-10). The operator's
words are in the ruling; read them there. Written 2026-10-10 by Imagination
(`imagination-prose-knowledge`, session `self-2026-10-10-eureka`).

Inputs, read this pass:

- Census: `gamecult-ops/docs/research/prose-knowledge-census-2026-10-10.md`
  (82 of 2,872 in-scope files opened; cited below as "census §n").
- Prior art: `gamecult-ops/docs/research/agent-knowledge-store-prior-art.md`
  (cited as "PA §n" or "PA failure n"; its [V] and [M] tags are carried, and
  every [M] claim this target relies on is flagged where it is used).
- The mind's kinds: `~/.claude/skills/eureka/references/campaign-state.md`.

This is a target and a backlog. It is not a migration plan and changes no code.

## 1. Pre-implementation statement

- **Objective.** Rulings, questions, owed work and ideas that live in prose
  become findable by any agent of the collective mind, and nothing that decides
  a campaign lives in two places.
- **Current mechanism.** The mind holds 13 kinds of typed pipeline documents
  (3,344 documents at `whoami` this pass). Everything else is `.md` files in
  three places with different reach: committed repo docs, which voidbot indexes
  (§4); loose files at `F:\Projects\` root and in worktrees, which nothing
  indexes (census §3.1); and Claude Code memory directories, which load only
  into sessions whose working directory matches (census §1).
- **Invariants.**
  - `one-home`: a ruling, question, owed item or fact lives in exactly one
    place. Every other place carries a pointer (a mind id or a path and line),
    never a restatement.
  - `operator-words-typed`: an operator ruling that steers work is a `ruling`
    document carrying the operator's words. Self admits it; no other faculty
    admits rulings.
  - `prose-is-retrievable`: prose that stays prose is reachable by one search
    path that returns the repo, path and line range.
  - `no-bulk-extraction`: no unattended extraction pass writes into the mind.
    Each item is read, checked against the mind and admitted by a faculty.
- **Intended change.** Ownership becomes simpler: typed kinds own decisions and
  owed work; prose owns explanation. Root files and memory files stop being
  owners.
- **Cut line.** Root `F:\Projects\*.md` files as a place where knowledge lives;
  handoff next-action lists as a store of owed work; memory files as a store of
  operator rulings.
- **Subtraction budget.** No new index, store, service or daemon. One new kind
  only, the one already owed (`observation`, §2.6). Retrieval reuses voidbot's
  existing index; coverage is bought by moving files into indexed repos, not by
  indexing more places.

## 2. The model page

One row per class the census found. "Kind" names an existing pipeline kind, a
kind earned under the doctrine's bar (owner, live consumer, invariant, liability
replaced), or "prose, retrievable".

| Class | Identity | Lifecycle | Authority | Home |
|---|---|---|---|---|
| Operator rulings and decisions | `<campaign>:ruling:<label>` | admitted once; superseded, never edited | Self admits; the operator's words in `operator_quote` | `ruling` (exists) |
| Open questions | `<campaign>:question:<label>` | open until a ruling answers it or it is withdrawn | Imagination raises; operator rules | `question` (exists) |
| Follow-ups, TODOs, owed work | `<campaign>:follow_up:<label>` | open until Fixed, Superseded or Withdrawn | Imagination or Self admits; the closer resolves | `follow_up` (exists) |
| Plans and roadmaps | campaign slug | a campaign's target and its cut specs | Self opens; Imagination maps | `campaign`, `target`, `cut_spec` (exist); repo roadmaps without a campaign stay prose |
| Ideas and design sketches | path | edited freely | the repo | prose, retrievable; a `follow_up` points at it once someone directs the work |
| Body facts and probe results | `<campaign>:observation:<label>` (proposed) | admitted with an as-of; superseded by a newer observation | Eyes admits | `observation`, earned (§2.6) |
| Rationale and history | path | postmortems and maps are history once written | the repo | prose, retrievable |
| Reference and how-to | path | maintained with the code it describes | the repo | prose, retrievable |
| Handoffs | path at `F:\Projects\` | one per Self rotation, superseded by the next | Self | a pointer to queries (§2.9); not a kind |
| Memory files | path under `~/.claude/projects/*/memory/` | edited by Life | Life | split four ways (§2.10) |

### 2.1 Operator rulings and decisions

`ruling` fits, including operator directions that answer no question
(`answers: null`, `authority: Operator`). Prose holds rulings in three shapes:

1. **Prose campaigns that finished in prose.** On 2026-09-30 the operator ruled
   that campaigns then in flight finish in their prose maps ("how are we going
   to move to Huginn mid-campaign? Methinks we don't.",
   `~/.claude/projects/F--Projects/memory/eureka-typed-vs-prose-campaigns.md:11-12`).
   Their rulings stay in the cut maps (census §4.1 lists the ten largest:
   Ghostlight play-agent 65 mentions, CultNet selection 42, fire control 39,
   and so on). They are history, and they are retrievable: every one of those
   repos is in voidbot's index (census §2). They are not migrated. That
   operator ruling is itself only in memory; it is in the list for Self (§6).
2. **Rulings restated in prose beside a typed campaign.** Examples:
   `Aetheria/docs/aetheria-release-map.md` names rulings by label in "Demo
   scope", and `eureka-site/BRIEF.md:12` lists operator rulings. Where the prose
   cites a ruling id it is a pointer and conforms to `one-home`. Where it
   restates the ruling's substance without an id, it is a second copy.
3. **Rulings only in memory files or loose doc heads** (census §4.1). These go
   to Self to admit with the operator's words (§6). Once a ruling is typed, the
   memory file keeps its "how to apply" text and cites the id instead of
   restating the ruling. `aetheria-ship-the-game.md:9` already does this.

### 2.2 Open questions

`question` fits when the question is a real fork in a campaign. Prose "Open
questions" sections come in two kinds that look alike:

- **Forks** (an operator choice with options): Asura Q10
  (`Asura/docs/cut-map.md:1671`), Fensalir Q-F1
  (`Fensalir/docs/planet-work-assimilation.md:392`), Hermodr
  (`Odin/docs/hermodr-extraction-plan.md:296`). In a campaign's repo they are
  admitted as questions. In a repo with no campaign they wait in prose until a
  campaign exists (§5, question `prose-campaign-coverage`).
- **Inquiry** (research questions with no decision attached):
  `Ghostlight/docs/research/things-to-steal.md:323`, the `Mimir/research/*`
  "Open Questions" sections, the prior-art "Gaps" sections. These stay prose.
  A question with no options and no decider is not the `question` kind.

### 2.3 Follow-ups, TODOs, owed work

`follow_up` fits. The prose shapes are postmortem "Open follow-ups" tables,
handoff next-action lists, runbook "follow-up" lines and Soul or Eyes passes
that say "Self admits these" (census §4.3). This is the TODO-list anti-pattern:
owed work in a list no tracker reads and no one closes. The documented
practice is to harvest it into the tracker and replace the text with a
reference (PA §5, [M]: todo-to-issue harvesters; Beads replacing markdown plan
files, [M]). Here the tracker is the mind. §7 is the first harvest.

A postmortem written after this target lists its open follow-ups by id and
does not describe them again. Eureka's `references/postmortem-template.md`
should say so. That is an edit to the Eureka skill, and it rides with
`cut-skill-huginn-cli` or the next skill cut.

### 2.4 Plans and roadmaps

No `plan` kind is earned. A plan with an owner is a campaign: its target holds
the invariants, its cut specs hold the steps, and its prose map holds rationale.
That is already the rule (`campaign-state.md`, "The prose map"). Roadmaps in
repos with no campaign (`Mimir/docs/implementation-plan.md:440`,
`Fensalir/docs/perfect-fensalir-machine-roadmap.md:512`,
`StreamPixels/docs/implementation-plan.md:110` and others, census §4.4) stay
prose and retrievable until a campaign opens. Their next steps then become cut
specs or follow-ups.

`state/map.yaml` and `state/evidence.jsonl` (in 15 and 11 repos; census §4.9)
are Epiphany-era repo state. They are not migrated. They are history, and
`ghostlight-play-agent-postmortem.md:226` records that one of them is not even
parseable YAML.

### 2.5 Ideas and design sketches

These stay prose, retrievable, with no `idea` kind. The bar fails on owner and
consumer. An idea nobody has directed has no owner who would close it and no
consumer who reads it on a schedule. A typed list of ideas would be the
free-text memory block with no status, owner or supersession (PA failure 17,
[M]) with a schema on top. Explanation is the Diataxis kind that holds
rationale and ideas (PA §5, [M]).

An idea crosses into the mind when someone directs work on it. That happens in
one of three ways: the operator says so (`Aetheria/docs/faction-territory-target.md:20-30`,
"slot it in right after fire control"), a campaign's map adopts it, or a ruling
points at it. Then a `follow_up` cites the doc by path and line, and the doc
stays the idea's home.

### 2.6 Body facts and probe results: `observation`, earned

The open follow-up `eureka-body:follow_up:eyes-observation-kind` (operator
2026-10-01: "Shouldn't eyes have write access to typed memory for its
findings?") already proposes this kind. This target confirms it meets the bar
and narrows it:

- **Owner.** `huginn-mind` admission. The leaf now lives in Huginn
  (`leaf-into-huginn` landed; verdict `cut-leaf-into-huginn.s2` exists).
- **Admitted by.** Eyes. Today no kind can hold an Eyes report: `finding`
  needs a Soul verdict and `follow_up` needs owed work.
- **Shape.** claim (`Line`), scope (repo, path or host), as-of (commit or UTC
  time), evidence (`Evidence[]`, the type findings already use), supersedes
  (an observation ref), and an optional `DocRef` to the committed report for
  anything longer than a `Para`.
- **Live consumers.** Imagination and Soul already cite Eyes facts. Cut specs
  cite them by `DocRef` today, which requires the report to be committed in a
  repo (`campaign-state.md`, prose map: "Body facts").
- **Invariant.** A fact carries the time it was true and is superseded rather
  than edited. This is the bi-temporal invalidation in Zep/Graphiti, where an
  older fact is marked invalid and kept, not deleted (PA §2, [V]). It is also
  the supersession rule the mind already applies to rulings.
- **Liability replaced.** The stand-in Eyes files at `F:\Projects\` root:
  `EYES-*.md`, 17 `aetheria-*` files plus 10 `aetheria-map-addenda-*`, 9
  `eureka-*` and others (census §4.6). None of them is version-controlled or
  indexed (census §3.1).

The follow-up's owner field still names "cut leaf-into-huginn", which has
landed without the kind. Self should re-own it: an eureka-body Imagination cut
in Huginn, after `huginn-cli` lands so the kind is added once, behind the new
door.

What can happen now, before the kind exists: Eyes writes its reports into the
owning repo's `docs/research/` (or `gamecult-ops/docs/research/` when no repo
owns the topic) and commits them there, never at `F:\Projects\` root. That
alone makes them retrievable (§4). Follow-up `prose-root-files-into-repos`
moves the existing root files.

### 2.7 Rationale and history

These stay prose, retrievable: campaign map Rationale sections, postmortems,
incident logs, superseded cut maps. Retirement uses the supersession rule of
ADRs, PEPs and RFCs: the old document keeps its text and gains a pointer to its
successor (PA §6, [M]). Where a superseded doc would mislead (for example
`Epiphany/notes/fresh-workspace-handoff.md`, "paused", census §5), the fix is a
one-line status header naming the successor, not a rewrite.

Duplicates are the retrieval hazard here. A search over several copies returns
old and new versions together unless the index can filter them (PA failure 18,
[M]). The census found six large cut documents duplicated between
`Epiphany/notes/` and `Huginn/docs/history/`, and `.voidbot/birth/` copies in
CultLib and Fensalir (census §4.6). Follow-up `indexed-duplicate-copies` owns
this.

### 2.8 Reference and how-to

These stay prose: runbooks, READMEs, AGENTS.md, contracts, prompts. Reference
is the one prose kind that tracks code mechanically (Diataxis, PA §5, [M]). It
stays correct only where a generator or test checks it (PA §6, [M]: generated
docs plus a CI diff check). Nothing in this target adds such checks; they
belong to each repo's own campaign.

### 2.9 Handoffs

No kind. The `hand_off` kind exists, but it records a stewardship transfer
between minds (parked, Cut 12), and a Self rotation is not that. A rotation
handoff should hold only what the mind cannot answer:

- the area and the campaigns this Self owns;
- the session label convention;
- pointers to the recipes to run at re-entry.

Owed work goes into the mind before rotation, as follow-ups. Memory
`handoff-is-a-pointer.md` already says to query the mind rather than trust a
handoff's lists. The 2026-10-10 handoff still carries a "Host loose ends"
section and cast-art plans that were in no document (§7 admits them).

When `run-kind` lands (`epiphany.pipeline.run.v2`, eureka-substrate), "who is
working on what" becomes derivable from runs, and the handoff shrinks again.

### 2.10 Memory files

Memory files (100 of them, census §1) hold four different things, and each has
a different home:

| Content | Example | Home |
|---|---|---|
| Operator rulings | `foss-games-monetization.md`, `cultcache-stores-outside-assets.md` | `ruling`, admitted by Self (§6); the memory keeps a pointer and its how-to-apply text |
| Operator context, relationship, lessons and scars that steer judgment | `operator-rapport.md`, `probe-process-spawning.md` | Eureka's Persona state (`gamecult.persona_state.v0` in Huginn) once `cut-huginn-persona-state.r3` lands. The standard puts "hard-won operating lessons, rakes stepped on, relationship context" there (`gamecult-ops/docs/persona-state-standard.md:22-25`) |
| Reference facts about the body | `cultlib-ci-harness.md`, `mcp-server-registration.md` | the owning repo's docs or runbooks (prose, retrievable); the memory keeps a pointer |
| Cross-project pointers and the index | `MEMORY.md` | stays, as Claude Code's harness loads it |

The second row hides a fork. Claude Code auto-loads `MEMORY.md`. Huginn's
Persona state is loaded by nothing in a Claude Code session. If both hold
lessons, they are two copies. The question `memory-persona-projection` (§8)
puts the options. The recommendation is that Persona state is canonical and the
memory index becomes a generated projection, marked `provenance.authority:
projection` as the cut spec already requires of file copies. That follows the
generated-file pattern (PA §6, [M]) and must wait for the persona-state cut.

## 3. Keeping prose and typed state from becoming second copies

**The rule.** When a class is typed, the prose item becomes a pointer: the
mind id, plus at most a one-line gloss. The prose is never deleted when it
holds rationale, and never left restating the decision. This is the
single-source rule as ADR, PEP and RFC indexes practise it: one record, other
places link to it (PA §6, [M]).

**Retirement order.** First admit, then repoint the prose in the same session,
citing the admission receipt. A prose item that is done (§7 found several:
items 4, 14, 33, 39) gets nothing new. It is left as history, with a status
line if it would mislead a reader.

**Who enforces it, and how.**

- **Life** owns memory files and the doctrine surfaces. At phase boundaries it
  checks that memory rulings it finds are typed, and repoints them.
- **Soul**, when a cut touches a campaign's prose map, checks it against
  `campaign-state.md`'s existing rule ("no progress section, no rulings and no
  ledger").
- **No regex tribunal.** Whether a paragraph restates a ruling is a question of
  meaning. Published drift detectors reach 72% accuracy or 0.72 precision with
  0.48 recall, and their benchmarks contain mislabelled data (PA §6 and
  failure 21, [V]). Detection is therefore advisory: an agent reads, proposes,
  and a faculty confirms. This is the propose-then-confirm pattern of published
  tooling (PA §4, [M]).

**Why no bulk extraction.** Single-prompt extraction over a long document
under-extracts, recovering one or two decisions from an hour of discussion
(PA §4, [V]). Extraction has two silent failures, omission and invention
(PA failure 13, [V]). Constrained decoding guarantees shape, not truth (PA
failure 14, [M]). LLM-driven memory updates overwrite without review (PA
failure 16). So each item is triaged by an agent that reads the cited section,
checks the mind by semantic query (dedupe), checks the file still says it
(staleness), and admits with `locations` citing file and line. §7 is that
procedure run once over the census list.

## 4. Retrieval for prose that stays prose

**One path: voidbot's existing source index. No second index.**

What voidbot is, verified this pass:

- The service is `voidbot-retrieval.service` on Yggdrasil. Its MCP container
  serves streamable HTTP on `127.0.0.1:17875/mcp`, with the tool allowlist
  `list_indexed_repos, search_history, get_message_context, search_sources,
  get_source_context, get_exact_source_document`
  (`gamecult-ops/compose/voidbot-retrieval.yggdrasil.yaml:11-27`).
- `search_sources` takes `query`, `limit`, `repoName`, `pathPrefix` and
  `language` (`VoidBot/apps/worker/src/mcp-server-shared.ts:93-99`). It
  returns `repoName`, `path`, `chunkIndex`, `lineStart`, `lineEnd` and
  `lastModifiedAt` for each hit (`mcp-server-shared.ts:309-321`), so an answer
  can cite a path and line range. It returns no commit.
- `voidbot-source-refresh.timer` runs hourly. It mirrors the **public GameCult
  GitHub catalog** into `/srv/voidbot/source-repos` and re-embeds changed
  chunks (`gamecult-ops/runbooks/voidbot-retrieval-recovery-yggdrasil.md:109-121`;
  the catalog URL is `scripts/refresh-voidbot-sources.sh:82`).
- Collection `voidbot_repository_source_chunks` lives in Qdrant on
  `127.0.0.1:6333`. Huginn's own index uses the same Qdrant and the same
  embedder and model, `qwen3-embedding:0.6b` on Raven
  (`gamecult-ops/runbooks/huginn-yggdrasil.md:18-27`). The two indexes already
  share their substrate. They differ in what they hold: Huginn indexes only
  typed documents' named fields (`Huginn/crates/huginn-mind/src/index.rs:42`).

Who can reach it:

- On Starfire, sessions reach it through the Yggdrasil tunnel
  (`gamecult-ops/scripts/start-yggdrasil-tunnel.ps1:23`,
  `runbooks/yggdrasil-ssh-tunnel.md:88`). It is registered in
  `~/.claude.json` as the `voidbot` HTTP MCP server.
- On Yggdrasil, a Self-run unit or the Huginn daemon reaches it on loopback.

What it does not reach, and the fix for each:

| Not indexed | Why | Fix |
|---|---|---|
| `F:\Projects\` root files (56) and non-git dirs | not in any repo | move them into the owning repo's docs and commit (`prose-root-files-into-repos`) |
| memory dirs | outside repos | the §2.10 split: rulings to the mind, reference to repo docs |
| worktrees | duplicates of indexed repos | nothing; excluding them is correct |
| private or forge-only repos, and repos missing from the catalog (`Aetheria-legacy`, `Blueberrichu`) | the catalog is public GitHub only | out of scope until a repo with live work is affected |

Rejected: a second prose index in Huginn. It would duplicate an hourly-refreshed
index over the same Qdrant and embedder, and give the corpus two owners.
Graph and tree indexes (GraphRAG, RAPTOR) were also rejected. They cost an LLM
pass over the corpus at index time and an update strategy on every change (PA
failure 11, [M]), and their reported gains fall under independent audit (PA
failure 10, [V]). Plain retrieval with citations is the baseline this corpus
needs. Letta's report that a filesystem plus grep competes with specialised
memory (PA §2, [M]) points the same way.

Two gaps in the path itself:

- **No commit in the hit.** A `DocRef` needs a commit. The fix is VoidBot's
  own: add the indexed commit to the source metadata (follow-up
  `voidbot-hit-commit`).
- **Which door.** The operator's direction to replace MCP with plain CLIs is
  recorded against eureka-state (`eureka-substrate:follow_up:huginn-cli-replaces-mcp`).
  Whether voidbot's door follows is a separate fork, put in question
  `prose-retrieval-door` (§8). The recommendation is to keep voidbot's door
  now; if it changes, the CLI is VoidBot's own and Huginn never proxies it.

## 5. Sequencing

| Now, with existing kinds | Waits on |
|---|---|
| §7 triage: follow-ups and a question admitted under owning campaigns | none |
| §6: Self admits the memory-only rulings with the operator's words | none |
| Eyes reports committed in repos, not at `F:\Projects\` root; root files moved (`prose-root-files-into-repos`) | none |
| Handoffs as pointers; loose ends admitted before rotation | none |
| Postmortem template lists follow-ups by id | rides the next Eureka skill cut (`cut-skill-huginn-cli`) |
| `observation` kind; then the root Eyes facts are admitted | `cut-huginn-cli` (one door to add the kind behind), then an eureka-body Imagination cut re-owning `eyes-observation-kind` |
| Memory lessons and rapport into Eureka's Persona state | `cut-huginn-persona-state.r3`, then question `memory-persona-projection` |
| Work in repos with no campaign (Kalsa, Fensalir, Mimir, AquaSynth, Odin, Muninn, StreamPixels and others) | question `prose-campaign-coverage`; a campaign spanning existing campaigns' repos waits on `eureka-body:follow_up:body-campaign-repos` (campaigns cannot gain repos) |
| Handoff lists derived from runs | `run-kind` (eureka-substrate) |
| `DocRef` from a voidbot hit | `voidbot-hit-commit` |

## 6. Rulings found only in prose or memory: for Self to admit

Self admits these with the operator's words. Imagination does not type rulings.
Each was checked against the mind by semantic query this pass; "near" names
the nearest typed ruling, which does not carry the same decision.

| Ruling (paraphrase) | Operator's words at | In the mind? |
|---|---|---|
| FOSS games are monetized as games-as-a-service, microtransactions included; the fork is the check | `~/.claude/projects/F--Projects/memory/foss-games-monetization.md:11-14` (the decision is stated in the memory's voice; the correction is narrated at `:21-22`; no verbatim quote) | no; near `eureka-body:ruling:always-open-source` |
| CultCache stores never live under Unity `Assets/`; no Unity serialization for game data | `F--Projects/memory/cultcache-stores-outside-assets.md:10` (verbatim, 2026-09-14), plus the Studio goal at `:19` | no; near `aetheria-release:ruling:ship-data-all-in-cc` |
| Spotiverse and Stonks are wanted small daemons, not corpses | `F--Projects/memory/wanted-small-daemons.md:11-18` (2026-09-06; partly quoted: "a real simple CultMesh bridge") | no |
| Having no consumer does not make code dead: park it with a tag and a restore note | `F--Projects/memory/parked-not-dead.md:11-14`, intent quote `:23` (2026-09-13) | only per instance: `ghostlight-verse:ruling:park-pre-rebuild-tree` |
| The operator's original CultCache.cs is the style bar | `F--Projects/memory/original-cultcache-style-bar.md:10` (verbatim, 2026-09-13) | not checked by query; no near ruling surfaced in the CultCache queries |
| Not going back to Codex/OpenAI as inference supplier, for alignment reasons | `F--Projects/memory/codex-retirement.md:20-23` (verbatim, 2026-09-05) | no; near `eureka-body:ruling:connector-owner` |
| AetheriaEve is preserved for taxidermy | `F--Projects/memory/aetheriaeve-taxidermy.md:10` (verbatim, 2026-09-13) | covered in substance by `variants:ruling:aetheriaeve-not-consumer` (2026-09-30); Self decides whether the quote is worth a ruling |
| Aetheria mods are compiled plugins; no scripting runtime or DSL for modders | `F--Projects-Aetheria/memory/aetheria-mods-are-compiled.md:16-18` (verbatim, 2026-10-03) | no; near `aetheria-release:ruling:modders-share-our-editor` |
| Aetheria's release bar: more ships and gear, UX polish and audio feedback before announcing | `F--Projects-Aetheria/memory/aetheria-release-bar.md:9-11` (verbatim, 2026-10-02) | no |
| The built-in pipeline is chosen; owning shaders is the point | `F--Projects-Aetheria/memory/built-in-pipeline-owned-shaders.md:10` (verbatim, 2026-10-10) | in substance `aetheria-release:ruling:shaders-own-every-pixel` (2026-10-03); the 2026-10-10 words add the reason |
| Campaigns in flight on 2026-09-30 finish in their prose maps | `F--Projects/memory/eureka-typed-vs-prose-campaigns.md:11-12` (verbatim) | no |
| Embedding moves off Nightwing because its availability "is more sporadic nowadays" | `F--Projects/memory/nightwing-embedding-migration-2026-09-26.md:12-16` | not checked |
| Faction territory: "slot it in right after fire control" | `Aetheria/docs/faction-territory-target.md:20-30` (2026-09-19) | no; the follow-up in §7 points at it |
| Negent weapons concept direction | `Aetheria/docs/negent-weapons-concept.md` head (2026-09-18) | no; the follow-up in §7 points at it |
| Asura: rulings on the cut map's questions | `Asura/docs/target.md:150` (2026-09-25) | the prose-campaign rule applies (§2.1); not migrated |

## 7. Triage of the census list (census §6)

Each item was checked in two ways. The file was read at the cited line on the
working tree (Ghostlight on `origin/main`, because local checkouts lag). The
mind was searched by semantic query. Admitted ids are under the campaign named.
"Stays" means no document was admitted, and why.

| # | Item | Result |
|---|---|---|
| 1 | Heimdall secrets exposed; rotate and move to `secret_files` | admitted `eureka-body:follow_up:heimdall-binding-secrets` |
| 2 | Backups unencrypted; key owner | admitted `eureka-body:follow_up:state-backup-encryption` (the weekly tier landed, `3d091c5`; encryption is still open, runbook `gamecult-state-backup.md:103`) |
| 3 | Raven reboot test, ESP, `reagentc`, inventory entry | admitted `eureka-body:follow_up:raven-reboot-esp-inventory` |
| 4 | Stale Raven LAN IP in inventory | done: `inventory.md:56-59` carries the 2026-09-09 IP and marks the old one superseded |
| 5 | Brush-tree campaign | no campaign repo (Brokkr, GameCult.Geometry.Csg); a proposal card exists; in question `prose-campaign-coverage` |
| 6 | Cast art: Flux portraits, shortlist | admitted `site-masthead:follow_up:cast-art-flux-portraits` |
| 7 | Faction territory generative names | admitted `aetheria-release:follow_up:faction-territory-target` |
| 8 | Negent weapons concept | admitted `aetheria-release:follow_up:negent-weapons-concept` |
| 9 | Content batch one §9 forks | admitted `aetheria-release:follow_up:content-batch-one-reconcile` (F1 resolved in the doc; the doc predates the demo rulings) |
| 10 | Asura Q10 switch probabilities | admitted question `asura:question:q10-eroded-aeolian-rate` |
| 11 | Asura dual contouring | stays: a deferred design in the target, rationale |
| 12 | Asura 4b-ii remaining terms | admitted `asura:follow_up:terrain-4b-ii-remaining-terms` |
| 13 | CultMath tape open items | admitted `cultmath-tapes:follow_up:tape-unpark-open-items` |
| 14 | Variants open forks | done: ruled in prose, `CultLib/docs/document-variants-cut.md:77-100` (2026-09-17 and 09-29) |
| 15 | TS schema id literal vs C# fingerprint | superseded by `variants:ruling:wire-id-is-version` and `variants:follow_up:wire-id-cuts` |
| 16 | Mutation testing scope | admitted `cultlib-gaps:follow_up:mutation-testing-scope` |
| 17 | CultNet distributed database next slice | admitted `cultlib-gaps:follow_up:cultnet-distdb-next-slice` (related: `variants:follow_up:golden-path-write-api`) |
| 18 | Muninn has two bodies | no campaign repo; in question `prose-campaign-coverage` |
| 19 | Muninn records out of `odin-core` | no campaign repo; same question |
| 20 | Hermodr extraction and its operator questions | no campaign repo; same question |
| 21 | Mimir next | no campaign repo; same question |
| 22 | Eve remaining work | admitted `thing:follow_up:eve-roadmap-remaining-work` |
| 23 | Heimdall gaps | no campaign repo; same question |
| 24 | Gjallar next questions | no campaign repo; same question |
| 25 | Fensalir Q-F1 | no campaign repo; same question |
| 26 | AquaSynth next pull | no campaign repo; same question |
| 27 | StreamPixels M1 | no campaign repo; same question |
| 28 | Ghostlight play-agent postmortem open follow-ups | admitted `ghostlight-verse:follow_up:play-agent-postmortem-open-items` |
| 29 | Ghostlight stock lenses L1.f1, f13, f14 | admitted `ghostlight-verse:follow_up:stock-lenses-open-items` |
| 30 | Ghostlight multiplayer social policy | stays: `ghostlight-dungeon-multiplayer-intention.md` is gone from `origin/main` (parked by `ghostlight-verse:ruling:park-pre-rebuild-tree`) |
| 31 | Ghostlight research questions | stays: inquiry, not a fork (§2.2) |
| 32 | Soul pass owed, 13 survivors | typed: `ghostlight-verse:follow_up:owed-soul-unpinned-survivors` |
| 33 | Aetheria design review | done: swept by `aetheria-release:follow_up:release-target-is-a-demo` (Fixed, `228f241e`) and `vault-docs-sweep` |
| 34 | Merge checklist open questions | Locomotion Q4 is answered by `aetheria-release:ruling:facing-separate-from-aim`; the rest is admitted as `aetheria-release:follow_up:merge-checklist-residue` |
| 35 | Vault Cleanup Q-08..Q-15 | admitted `aetheria-release:follow_up:vault-cleanup-accepted-queue` |
| 36 | Kalsa deepening queue | no campaign repo; in question `prose-campaign-coverage` |
| 37 | Zyphos next action | no campaign repo; same question |
| 38 | weksa roadmap | no campaign repo; same question (see also `eureka-body:follow_up:weksa-mind`) |
| 39 | Eureka brief lessons | done: all six landed in `references/briefs.md` (`:113`, `:369`, `:401`, `:409`, `:81`) and `SKILL.md:658` |
| 40 | Doctrine de-shout and publish scrub | de-shout landed (Eureka `352165d`); the scrub's application was not checked; both rulings are typed (`doctrine-explains-not-shouts`, `doctrine-publish-scrub`) |
| 41 | Idunn watchdog open findings | stays: the postmortem restates mind documents |
| 42 | Substrate leftovers on Yggdrasil | admitted `eureka-substrate:follow_up:huginn-107552dc-leftovers` |
| 43 | Bifrost first vertical slice | admitted `eureka-body:follow_up:bifrost-context-next-target` |
| 44 | Pomba Branca off Wix | no campaign repo; in question `prose-campaign-coverage` |
| 45 | Notaris brief questions | stays: questions for an outside notary, not forks in the mind |
| 46 | Prior-art gaps sections | stays: Eyes gaps; `observation` will hold them (§2.6) |
| 47 | Repixelizer hygiene | no campaign repo; in question `prose-campaign-coverage` |
| 48 | Spotiverse and Stonks | the ruling is in §6; the work is in question `prose-campaign-coverage` |
| 49 | Memory-only rulings | §6, for Self |

Added from this target's own mechanism, with the source set to the follow-up:
`eureka-body:follow_up:prose-root-files-into-repos`,
`eureka-body:follow_up:indexed-duplicate-copies` and
`eureka-body:follow_up:voidbot-hit-commit`.

## 8. Questions raised

- `eureka-body:question:prose-campaign-coverage`: which repos with no campaign
  get one.
- `eureka-body:question:memory-persona-projection`: Persona state or
  `MEMORY.md` as the home of lessons.
- `eureka-body:question:prose-retrieval-door`: whether voidbot's MCP door
  changes.

Each carries options and a recommendation in the mind.
