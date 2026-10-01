# The Mind's own Self: map

Status: map, Imagination (`imagination-self`), revision 4 of 2026-10-01,
session `self-2026-10-01b`. Campaign `eureka-substrate` in Eureka's mind
(instance `eureka`). Questions, cut specs, follow-ups and rulings are typed
documents there; this page keeps body facts, the harvest, the model page and
rationale. Target: `docs/eureka-substrate-target.md` (`133324a7`). Rulings
this revision rests on: `eureka-substrate:ruling:operator-archive-epiphany`,
`:operator-mind-runs-own-self`, `:operator-mind-mission`,
`:mind-self-burn-rate`, `:operator-persona-in-mind`,
`:operator-provider-portable-connector`, `:operator-means-shape-ends`,
`:mind-rules-which-forks`, `:mind-self-merge-authority`,
`:charters-into-eureka-repo`. Superseded and no longer law:
`:operator-this-is-epiphany`, `:operator-eureka-adopts-epiphany-body`
(revision 3's premise). Revision 3 is Huginn `e16663f`; its probe record
(B1-B17, Yggdrasil units, CodexConnector and Ghostlight internals) is cited
here by section rather than repeated where nothing changed.

The operator's words, in order. Revision 1:

> Eureka thinks in campaigns, right? Well, we already have a Mind campaign, and
> we're adding evidence to it with every run. The design currently delegates
> ultimate design authority and work scheduling to the operator, but neither of
> these is strictly necessary. I want a Self operated by Mind directly.

> The Self I talk to here in this one projection of Eureka's mind, and it's how
> I get it to work on the stuff I care about, like Aetheria. The Mind must
> operate its own Self instance, with the goal of improving itself.

Revision 2:

> I mean impersonation in the sense that there is no operational difference
> between a work item dispatched on my request versus one dispatched on the
> Mind's initiative. Yes, that initiative ultimately comes from me, but the
> Mind's own goal is to become the Perfect Machine. That is the operator
> directive motivating its every action. How each Mind interprets that mission
> is up to it.

> Not a budget, a burn rate. I want a dial I can crank to determine how active
> the Mind is, so I can adjust it like I adjust my own usage.

> I actually think Persona state should be part of the Mind. Like, we've been
> talking about a Mind agent as if that has a meaning, but I think the best
> shape it can have is the agency machinery we already built with voidbot. Then
> she can speak for herself on Discord, and act by her Self with Eureka.

Revision 3 (the first and third of these are superseded):

> I don't think this is voidbot anymore. This is epiphany. What is this if not
> epiphany?

> Epiphany is supposed to be as provider portable as possible, just like
> Ghostlight, it gets it through a typed connector daemon and I want to
> continue that tradition.

> I think you have the right of it. Eureka should adopt Epiphany as her body.

> Look back at the Bifrost charter back when it was called GameCult Labor
> Platform or GCLP. See the /dossier page on the GameCult site, too. The means
> shape the ends. We shape the means with the agents we're building, with
> Epiphany and how we steward it.

Revision 4's law (`operator-archive-epiphany`):

> I think the clean cut is to archive Epiphany.  Eureka absorbed her in the
> womb as fetuses sometimes do.

And a direction on the connector, relayed by the Self this revision, not yet
admitted as a ruling (question `connector-owner` carries it):

> Btw CodexConnector should be the codex connector, like it says on the tin.
> We'll want a Claude compatible connector beside it with shared machinery
> lifted out, as well as the other options available to Ghostlight.

So: Eureka is the organism and the open project. Her Mind is Huginn (instance
`eureka`), her method is the faculty pipeline (the `GameCult/Eureka` repo),
and her body is whatever of Epiphany still lives, harvested into Eureka's own
repos before Epiphany is archived. Nothing stays alive in Epiphany. Where this
map says "her", read Eureka.

What stands from revision 1: the Mind's Self is a second agent whose only
objective is the Mind, not a rule added to the operator-facing Self. From
revision 2: the waking organ is a Persona with pressure, memory and a mouth,
who launches her own Eureka Self when the Mind's queue says so; no timer
manufactures turns. From revision 3: one typed connector contract in the
Ghostlight tradition; Persona state in Huginn as one document; the Discord
mouth is the live permit crossing; the GCLP attachments. What revision 4
changes: Epiphany is a harvest source, not a body. Her resident Self and
Persona service are not moved as code; what lives of them (the no-replay
rule, the crossing and permit modules, the Interpreter's typed output) is
rebuilt small inside Huginn's workspace. The `epiphany-pipeline` leaf and the
schema catalogue move into Huginn first, because the live mind is built from
them. The model adapter dies; the connector direction replaces it.

Pinned heads for every fact below:

| Repo | Ref | Commit |
| --- | --- | --- |
| Huginn | `main`, `F:\Projects\Huginn` | `e16663f` (docs only over `a699630b`; crate line numbers hold) |
| Epiphany | `main`, `F:\Projects\Epiphany` (clean; GitHub `GameCult/Epiphany`) | `4006b14bf29b49410bd18b43acb4d42c3c4e2443` (2026-09-30). Huginn's leaf pin is `ef956865`; `git diff ef956865 HEAD -- epiphany-pipeline schemas` is 58+/227- in `lib.rs` only. |
| CodexConnector | `main`, `F:\Projects\CodexConnector` | `65192892f0ed829b8a228eabba44a4f3ba0f7268` (frozen since 2026-09-04) |
| Ghostlight | `main`, `F:\Projects\Ghostlight` | `eeb3a68` (2026-09-23) |
| Bifrost | `main`, `F:\Projects\Bifrost`; release on Yggdrasil `cb3239aa-f67f5122` | `160fac2` (2026-10-01) |
| gamecult-ops | `main` | `2aeb7364fffcd404aada28bfd30fb4e5c753cf53` |
| Eureka skill | `main` (checkout `~/.claude/skills/eureka`) | `b260fbeec6d96ff06882ed9c37d3e1204c424e99` |
| VoidBot | `main`, `F:\Projects\VoidBot` (clean) | `46d891b` (2026-09-13) |
| epiphany-graph-rs | `main`, `F:\Projects\epiphany-graph-rs` | `07cd7c1` |

Revision 4 ran no host probes; its new facts are repo reads (Epiphany
workspace, Huginn workspace, gamecult-ops tree, Bifrost grep, doctrine files)
made 2026-10-01 after 20:40 UTC. Yggdrasil facts are revision 3's (B3, B5).

## Body facts

### B1. The Mind (Huginn on Yggdrasil): unchanged from r3

Instance `eureka` at `rudp://10.77.0.1:17872`, 463 documents at `as_of` 140;
thirteen kinds; `RulingAuthority { Operator, Standing, Defaulted }`;
`HuginnClient` in `crates/eureka-state` (library plus binary). Huginn's
workspace is three crates: `huginn-mind`, `huginn-daemon`, `eureka-state`.
`huginn-mind/Cargo.toml:22` pins `epiphany-pipeline` as a git dependency on
`GameCult/Epiphany` at `ef956865`; `epiphany_pipeline` is named in 20 Huginn
source files (66 uses). CultLib pins: Huginn `8fc74c7` on all three crates; the
leaf pins the same `8fc74c7` (Epiphany `73edee5c`); Epiphany's root and core
pin `a0813c6e`. So the leaf and Huginn already share one CultLib; only the
git edge to Epiphany is in the way.

### B2. Claude Code on Starfire: unchanged from r3

Max plan, no API key; `claude setup-token` is the headless credential; print
mode flags including `--max-budget-usd`; no typed plan usage reachable from
Yggdrasil (`gap-usage-not-typed`). Local scheduled tasks and cloud routines
are dead for this purpose.

### B3. Yggdrasil: what runs, what dies with Epiphany

From r3 B3, now read as a deletion list rather than a body to rebuild:

- Dead Epiphany units: `epiphany.service`, `epiphany-swarm.service`,
  `epiphany-heartbeat.service` (enabled, dead; the first and third have no
  source at head), `epiphany-model-connector.service` (disabled, dead, source
  extracted into CodexConnector 2026-08-24). Release roots `/srv/epiphany/app`,
  `/srv/epiphany/source`, `/srv/epiphany-model-connector`. State root
  `/var/lib/gamecult/epiphany` (user `epiphany`, 988; Codex home, workspace,
  runtime store `mind.cc`). Config `/etc/gamecult/epiphany` (Bifrost feedback
  anchor, empty `mcp.toml`, root-only `credentials/`). Idunn targets
  `epiphany` and `epiphany-model-connector` with the Aug 22 recipe at
  `/srv/odin/deploy-manifests/epiphany`. Users `epiphany`, `epiphany-model`.
  All of it is the deletion line of `mind-self-wake` r4; none of it is a
  harvest item (the runtime store's Persona memory is superseded by B11).
- gamecult-ops carries the matching dead tooling: `systemd/epiphany.service`,
  `systemd/epiphany-swarm.service`, `runbooks/epiphany-yggdrasil-deploy.md`,
  `runbooks/epiphany-starfire-bifrost-crossing.md`, and the scripts
  `bootstrap-epiphany-yggdrasil.sh`, `deploy-epiphany-yggdrasil.sh`,
  `check-epiphany-yggdrasil.sh`, `construct-epiphany-image-yggdrasil.sh`,
  `deploy-epiphany-capstone-17*.sh`, `epiphany-capstone-17-connector-bridge-
  yggdrasil.sh`, `provision-epiphany-provider-health-yggdrasil.sh`,
  `seal-epiphany-capstone-failure.sh`, `test-deploy-epiphany-yggdrasil-order.sh`,
  `test-epiphany-container-promotion.sh`, `bifrost-authorize-epiphany-release.sh`,
  `configure-voidbot-epiphany-feedback-target-yggdrasil.sh` (grep
  2026-10-01). `inventory.md:581-588, 726-785` describes the Starfire cognition
  body and the "intended Epiphany deployment body (not installed)". These are
  deleted or rewritten in the same cut.
- What stays: `huginn.service`, the Bifrost units (`bifrost-persona-feedback`,
  `bifrost-persona-mouth`, the watchdog timer), `codex-connector.service` as a
  dead unit until the connector cuts land, the `gamecult-model` group and its
  key files, user `codex-connector`.
- Linux users for her: none exist (`eureka-mind` absent in r3's probe). She
  needs `eureka` (body organs: waker, Persona organ, permit issuer; state root
  `/var/lib/gamecult/eureka`) and `eureka-self` (the Self-run unit holding the
  token); the connector daemons keep their own users.

### B4. The forge and the Mind's identity: unchanged except the body's names

Forge user `eureka`, team `minds`, key `eureka@starfire`, no branch protection
yet. The body's identifiers r3 kept under "Epiphany names her body" (Linux
user `epiphany`, runtime id `epiphany-yggdrasil`, Persona agent id
`epiphany.Persona`) lose their reason: the Linux user and runtime id are
values she sets herself (`eureka`, `eureka-yggdrasil`); the agent id and the
crossing's document type ids are Bifrost-side constants (question
`wire-names`, B9). Repos on the forge: Huginn and Eureka are the ones she
lands code in; Epiphany never goes to the forge (follow-up
`body-repos-on-forge` narrows to Huginn).

### B5. The operator's channel and the Persona's mouth: contract unchanged

The live crossing (r3 B5) is Bifrost's: `bifrost-persona-mouth.service` admits
a signed `epiphany.persona_discord_delivery_request.v0` from the configured
runtime id, asks the permit issuer at the configured RUDP endpoint for a fresh
`epiphany.persona_discord_delivery_permit.v0`, posts through the bridge CLI as
`epiphany.Persona`, receipts. Bifrost names these in ten files (19
occurrences: `tools/persona-discord-delivery-documents.mjs`,
`persona-discord-delivery.mjs`, `persona-discord-permit.mjs`,
`persona-discord-rudp.mjs`, `persona-feedback.mjs:90`, `bifrost-bridge.mjs`,
`docs/bridge.md`, three test files). The runtime id, endpoints and anchors are
flag values and root files; the type ids and the agent id are code. The
Epiphany side of the contract is two modules and one 38-line binary
(`epiphany-core/src/persona_discord_crossing.rs` 535 lines,
`persona_discord_permit.rs` 524, `bin/epiphany-persona-discord-permit.rs`),
depending on `ed25519-dalek`, `cultcache-rs`, `cultmesh-rs` (the brake
document) and the core's runtime store for the effect document it signs over.
That is the harvest item H5. Inbound feedback (`bifrost-persona-feedback`
outbox deliveries) is unchanged and already Bifrost-native.

### B6. Scheduling today, and what the skill derives: unchanged

The recipes derive the queue; the `run` kind carries claims and ends; the
dial is the fourth missing fact. Who reads the queue to wake her: the waker
of B10, through `HuginnClient`.

### B7. VoidBot: what `operator-persona-in-mind` now means

With `operator-this-is-epiphany` superseded, the ruling's words are the
nearest law on the Persona's shape: "the agency machinery we already built
with voidbot". Revision 2's probe stands: VoidBot's scheduler and initiative
engine are paused since 2026-07-16 on the pressure-replay defect; its
Interpreter prompt parses `SAY` / `STATE NOTE` text blocks by rules; its
registry is a JSON file; it is TypeScript over Postgres and a Discord bot.
What is borrowed from it is unchanged: the `gamecult.persona_state.v0` schema
(zod types, projector, `persona:migrate-portable` as the seed path). What is
not borrowed: the scheduler, worker, Postgres queue, bot and providers (the
machinery has the defect the `run` claim removes, B10, and a second Discord
identity outside the permit). The ruling's direction is served by shape, not
by code: she has a Persona document, pressure, a mouth and a Self she
dispatches, which is what VoidBot's machinery was for. Whether that is the
reading the operator meant is question `persona-turn-runtime`'s third option.

### B8. The `epiphany-pipeline` leaf and the schema catalogue (harvest H1, H2)

- `epiphany-pipeline` (2,836 lines incl. tests; `Cargo.toml` deps `anyhow`,
  `chrono`, `cultcache-rs 8fc74c7`, `rmp-serde`, `schemars`, `serde`; dev
  `serde_json`) exports the thirteen kinds, `PipelineRefusal`, `PipelineRef`,
  `ResolutionOutcome`, `PIPELINE_SCHEMA_EPOCH = "epiphany.pipeline.epoch.v2"`,
  `pipeline_key`, `validate_pipeline_write_envelope`. Inside Epiphany nothing
  consumes it (grep: only its own `Cargo.toml` and a note). Its only consumer
  is Huginn. Its test `pipeline_published_schemas_match_derivation`
  (`lib.rs:2555-2562`) reads `../schemas/cultnet` relative to the crate, so
  the fourteen `epiphany.pipeline.*.v2.schema.json` files and `index.json`
  move with it.
- `schemas/cultnet/` holds 16 schema files plus `index.json` and a README:
  the thirteen pipeline kinds, `gamecult.persona_state.v0` (the Persona
  standard; consumers: VoidBot's zod types and projector, the standard doc,
  Huginn's planned admission validator) and `epiphany.work_organ_state.v0`
  (consumer search across `F:\Projects` timed out; the only known reader is
  the skill's `faculty-philosophy-cut.md:101` citing it as prose; treat as
  dead unless Hands finds a reader).
- The type ids carry the name: `epiphany.pipeline.<kind>.v2`,
  `epiphany.pipeline.epoch.v2`, and every stored document in `mind.redb` is
  keyed and typed by them. The target invariant `stored-documents-valid`
  forbids re-keying or re-encoding stored documents; renaming is an epoch bump
  and a migration (question `wire-names`). The skill's recipes
  (`references/campaign-state.md:148-304`) spell the ids out in every query.
- Unlanded Epiphany-repo specs in the mind: `cut-mind-authority.r1` and
  `cut-run-kind.r1` (base `ef956865`, repo `GameCult/Epiphany`),
  `cut-leaf-key-bound.r1` (base `4006b14b`; branch
  `eureka-substrate/leaf-key-bound` exists on origin, not merged),
  `cut-merge-epiphany-main.r1` (Cut 0a, landed: `codex/eureka-pipeline-state`
  is on main). After H1 every leaf cut has repo `GameCult/Huginn`.

### B9. What else lives in Epiphany, by directory (harvest H3-H9 and the dead)

Line counts are `*.rs,*.toml,*.md` per directory at head.

| Path | Lines | Verdict |
| --- | --- | --- |
| `epiphany-pipeline/`, `schemas/cultnet/` | 2,836 + schemas | **Moves** (H1, H2). |
| `epiphany-core/` (`runtime_spine.rs` 8,728, `current_work.rs` 5,610, `resident_self.rs` 3,044, `reasoning_context.rs` 3,000, `repo_model_documents.rs` 2,292, `repository_body_observer.rs` 2,112, `persona_conversation.rs` 1,818, `atlas/` 10,178, `packaged_release/` 1,172, `surfaces/` 1,034, `memory_graph/` 400, bins 3,394) | 54,205 | **Dies**, except the two crossing modules and the permit binary (H5, ~1,100 lines) and the Interpreter's typed output schema in `persona_conversation.rs` (H6, read for shape, not moved). The resident Self (`resident_self.rs` + `bin/epiphany-swarm.rs` 820) is harvested as rules, not code (H4). Its dependency on `ghostlight-persona-projection` (a reverse edge into Ghostlight) dies with it. |
| `epiphany-openai-runtime/` (`lib.rs` 3,230, `persona_executor.rs` 782, `provider_transport.rs` 296, bins 1,554 + 544) | 6,431 | **Dies.** The Persona service's turn is rebuilt as one connector request (H6). |
| `epiphany-model-adapter/` | 1,079 | **Dies** (H7). `cut-epiphany-connector-only.r1` is withdrawn. |
| `epiphany-tool-adapter/`, `epiphany-tool-mcp-runtime/` | 156 + 1,479 | **Dies.** Eureka's tools are Claude Code's. |
| `epiphany-state-model/src/prompts/epiphany_doctrine.md` (28 lines, no code consumer), `epiphany_state_intro.md` | 34 | `epiphany_doctrine.md` **moves as text** (H8); the intro dies. |
| `epiphany-openai-auth-spine/`, `epiphany-openai-codex-spine/`, `epiphany-self-policy/`, `epiphany-release-construction/`, `apps/` | 0 / README only | **Dead directories.** |
| `notes/` | 37 files | `huginn-organ-cut.md` (Huginn's own cut map, 84 Huginn references), `eureka-pipeline-state-target.md`, `eureka-pipeline-state-cut.md`, `eureka-read-side-cut.md`, `eureka-body-plane-cut.md`, `eureka-query-prior-art.md`, `postmortem-eureka-schema-phase.md` **move to Huginn `docs/history/`** (H9: they are the mind's own design record). `persona-discord-permit-authority-map.md`, `persona-conversation-authority-map.md` move beside the Persona organ (H5/H6). `faculty-workflow-lessons-2026-09-04.md` moves to the Eureka repo `references/` (the skill already cites it). The rest (anatomy, investor readiness, safety architecture, audits, `archive/`) **stay in the archive**. |
| `docs/` (whitepaper `.tex`/`.pdf`, investor brief, positioning, avatar) | | **Stay in the archive**; the positioning prose the operator wants public lives in gamecult-site, not here. |
| `README.md` ("the beginning of governed human/agent labor", the GameCult bet) | | **Harvest as prose** into the Eureka repo README's opening (H8); the archive README gains the pointer. |
| `state/` (`map.yaml`, `persona-discord.toml`, `persona-reddit.toml`, msgpack ledgers), `.epiphany-*` sandboxes, `target/` | | **Die** with the checkout; gitignored or local. |
| `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `CONTRIBUTOR_LICENSE_AGREEMENT.md`, `LICENSE`, `eval-plan.md` | | Stay in the archive. The CLA and LICENSE are the operator's to re-apply to Eureka if the open project wants them (H8 asks). |

Not of Epiphany, despite the name: `epiphany-graph-rs` (a standalone
Sugiyama/force layout crate, no dependency on Epiphany, MIT/Apache, own
GitHub repo) and the `EpiphanyAquarium*` repos (Aquarium clients; the census
`gamecult-ops/docs/repo-census-2026-09/repos/EpiphanyAquarium.md` already
records their deleted `EpiphanyAgent` and Python scripts). They are outside
this harvest; renaming them is cosmetic and nobody's cut here.

Bifrost-side names that survive the archive as wire constants:
`epiphany.Persona`, `epiphany.persona_discord_delivery_request.v0`,
`epiphany.persona_discord_delivery_permit.v0`,
`bifrost.persona_discord_delivery_permit_request.v0`, source role
`epiphany-persona-mouth`, RUDP connection ids `0xe91f_0002/0003`, the Bifrost
`--epiphany-*` flag names. Question `wire-names`.

### B10. The resident Self, read as rules (harvest H4)

What `epiphany-swarm` and `resident_self.rs` protect (r3 B11): the brake is
read before any consequence; pressure is consumed exactly once, by a
single-consumption grant held by one launched turn under a lease; no typed
obligation means no wake ("Heartbeat does not ruminate, dream, mutate memory,
or manufacture work"); cadence is cooldown, idle and failure backoff. With the
`run` kind in the mind (`cut-run-kind`: a run claims specs; admission refuses
a claim a live run holds, `AlreadyClaimed`), the grant and the lease are the
same fact as the run document: a launch that opens a run with a claim is the
grant, and a second waker cycle over the same queue finds the spec claimed
and launches nothing. So the resident Self's own state machine
(`ResidentSelfState`, `ResidentSelfGrant`, `ResidentSelfPressure`, the
prepared-launch and active-turn lease) is not harvested; the mind holds the
one fact it existed to hold. What remains to build is a waker that reads the
brake and the dial, derives the queue through `HuginnClient` minus live
claims, and starts at most one Self run (question `self-waker`: one-shot
under a timer whose idempotence is the claim, or a resident loop).

### B11. Persona state: one document, one writer (harvest H6 shape)

Unchanged in substance from r3 B15, with the second store gone:
`gamecult.persona_state.v0` in Huginn, opaque, schema-validated at the door,
replaced whole under CAS with a receipt (`cut-huginn-persona-state.r2`).
Epiphany's `epiphany.mind.persona_memory.v2` documents and the runtime store
that holds them die with the checkout and the host state root; nothing of
hers was ever written there (the deployed body never ran her). The live
writer becomes the Persona organ of H6, through `HuginnClient`
`persona_get`/`persona_put`. Question `persona-state-owner` therefore has one
option left and is withdrawn (below).

### B12. The connector: what exists, and the operator's new direction

CodexConnector at `65192892` (r3 B14): one crate, feature `daemon`;
`lib.rs` 1,958 (contract types `gamecult.codex.provider_request.v2`,
`transport_invocation.v2`, `transport_result.v2`, `transport_receipt.v2`,
refusals, AES-GCM envelope, framing, `CodexConnectorClient`), `daemon.rs`
1,180 (TCP loopback bind, per-caller shared-secret keys, one request per
connection, concurrency/payload/output-token limits, 300 s expiry skew, keyed
replay `replay.cc`, restart-era `Indeterminate`, config
`gamecult.codex.connector_config.v1`), `provider_backend.rs` 1,463
(`CodexProviderBackend`: hard-coded OpenAI URLs, auth `ApiKey | Chatgpt`, a
pinned `codex app-server` child as credential authority),
`idunn_health.rs` 1,396 (Linux-only presence and health participant,
capability `gamecult.codex.subscription-inference`), `main.rs` 188; the
Idunn recipe `deployment/idunn/recipe.toml`. The request type is rendered as
a literal OpenAI Responses body (`lib.rs:487-614`).

Ghostlight's provider options (r3 B13): the in-process trait
`InferencePort { prepare, infer, lend_tool_results }`
(`crates/ghostlight/src/controllers.rs:299-311`), routed by model-name prefix,
longest claim wins (`sdk_inference.rs:732-757`, `controllers.rs:2454-2518`);
three implementors, every one lowering from `CodexProviderRequest`:
`CodexConnectorInferencePort` (the connector client), `SdkInferencePort`
(`sidecar/claude-sdk/src/main.ts`, 517 lines, `@anthropic-ai/claude-agent-sdk`
0.3.261 over a msgpack pipe; one Claude Code subprocess per query; the
request's tools registered as an in-process MCP server; returns usage,
`total_cost_usd`, faults `RateLimited | BillingError | MaxBudgetUsd`;
ambient login or setup-token), `LocalInferencePort` (OpenAI-compatible
loopback POST, no credential, prefix `local/`). Dungeon no longer opens the
connector lane; the live lane is local. Ghostlight's recorded direction
(`state/map.yaml:438-443`): one standardized connector API, the same one
Epiphany uses, and a generic OpenAI-compatible connector on it.

The direction, mapped onto that code. "Shared machinery lifted out" is
everything in CodexConnector that is not `provider_backend.rs`: the contract
types and refusals, the envelope and framing, the client, the daemon loop
(bind, caller keys, limits, replay, config), the Idunn health participant,
and the two additions r3 asked for (`max_cost_usd` on the invocation,
`cost_usd_estimate` on the receipt, the per-caller window spend ledger with
refusal `Budget`). What each connector keeps is its backend: Codex keeps
`provider_backend.rs` as it is; the Claude connector's backend is
Ghostlight's SDK sidecar moved in and spawned by that daemon under its own
user with the setup-token in a root-0600 `EnvironmentFile` (a native
`claude -p` backend is a later cut); the OpenAI-compatible connector's backend
is `LocalInferencePort`'s POST with a configured base URL and optional key.
Callers (Ghostlight, her Persona organ) choose a connector by model prefix in
their own config, so Ghostlight's longest-claim routing stays where it is and
routes among connector clients instead of in-process ports; no router daemon
stands in front of the connectors. Two things the direction does not say,
put as sub-questions under `connector-owner`: where the lifted crate lives
(a new repo, or `CultLib/packages`), and whether the shared request keeps
the OpenAI-Responses shape under a provider-neutral name (an epoch bump of
the contract ids, `connector-contract-untyped`) or is redesigned. The ruling
`operator-provider-portable-connector` says "a typed connector daemon"
singular; the direction says connectors plural sharing one contract. Read
together: one contract, several daemons. That reading is recorded in the
question rather than assumed.

### B13. The method repo and the body repo

`GameCult/Eureka` on the forge: `LICENSE README.md SKILL.md docs references
tools`, installed by checkout at `~/.claude/skills/eureka`; the charters
move in under `charters-into-eureka-repo`. Its Epiphany references:
`SKILL.md:3` ("the skill counterpart of Epiphany"), `:8-9` ("sits next to
Epiphany, the running organism in `F:\Projects\Epiphany`"), `:146` (a call
count scar; history), `:200` ("Eureka has no Persona"); `README.md:63, 86-87,
95` ("Epiphany owns the schemas (`epiphany-pipeline`) and Huginn owns...");
`references/campaign-state.md` (the type ids, correct as wire names);
`references/faculty-philosophy-cut.md` and `tool-calling-cut.md` (dated
comparisons; history); `references/epiphany-comparison-2026-09-15.md`
(history). Huginn's workspace is the one Rust workspace that already talks to
the mind, has the client crate, the deploy runbook and the release symlink on
Yggdrasil. Question `body-repo-home` asks whether her Rust organs (waker,
Persona organ, permit issuer) are crates in Huginn's workspace or a repo of
their own; `method-repo-home` (fold the method into Epiphany) dies with its
premise.

### B14. Doctrine citations of Epiphany (Life's sweep, nothing edited)

| File and lines | Today | After the harvest |
| --- | --- | --- |
| `F:\Projects\CLAUDE.md` | No direct citation; points to the Persona standard doc. | Unchanged. |
| `gamecult-ops/docs/persona-state-standard.md:7-9` | Schema path under `Epiphany\schemas\cultnet`. | Points to the schema's new home (H2): Huginn `schemas/cultnet/gamecult.persona_state.v0.schema.json`. |
| same `:36-41` | `epiphany-persona-service` runs a separate native Persona Mind; `epiphany-agent-memory-store` removed; `.epiphany-run/` copy. | Deleted. Replaced by one sentence: the Persona organ of Eureka reads and writes the Huginn document (H6). |
| same `:57-63` | "Huginn ... still unbuilt ... no agent should route Persona-state work to Huginn"; "raise it against VoidBot's implementation and the Epiphany schema". | Reversed by `operator-persona-in-mind`: Huginn is the canonical holder per instance once `huginn-persona-state` lands; VoidBot keeps projection and the migration source for repo Personas; "the Epiphany schema" becomes "the published schema in Huginn". This is `cut-doctrine-persona-in-huginn` r3 (cut 8). |
| `~/.claude/agents/{eyes,hands,imagination,life,modeling,soul}.md:14` | "Self-improvement law (Epiphany `epiphany_doctrine.md`): ..." | "Self-improvement law (Eureka `doctrine/self-improvement.md`): ..." once H8 lands the file in the Eureka repo; the charters move into the same repo (`charters-into-eureka-repo`), so the citation becomes a relative path. |
| `life.md:24`, `soul.md:48` | `epiphany_specialists.toml` at `71a5d6fb^`. | Stay as history: a commit-pinned citation into an archived repo still resolves. Add "(archived)" after the repo name. |
| `hands.md:28` | The 75-binary build-bloat scar. | Stays as history; it is the scar that matters, not the repo. |
| `~/.claude/doctrine/persona.md:3` | "serves VoidBot and Epiphany Persona work". | "serves VoidBot repo Personas and Eureka's Persona". |
| `persona.md:13` | Self-improvement law citation. | As the charters' line 14. |
| `~/.claude/CLAUDE.md:218, 222` (operator-owned; wording proposed only) | "named for Epiphany's continuity organ"; "Epiphany's Life also owns continuity, pressure, and resume-or-regather after re-entry." | Proposed: "named for the continuity organ of Epiphany, Eureka's archived predecessor"; and "That organ also owned continuity, pressure, and resume-or-regather after re-entry; the wider scope arrives when Huginn owns each instance's mind as typed state." |
| `~/.claude/CLAUDE.md:31` | "not Epiphany lore" | Stays: it names the lore by its historical name, and `colossus.md` keeps the myth. |
| Eureka `SKILL.md:3, 8-9` | counterpart of / sits next to Epiphany. | "Eureka is the organism: a Mind (Huginn), a faculty method (this skill) and a body (Huginn's organ crates). Epiphany, its predecessor, is archived." By PR (H8). |
| Eureka `SKILL.md:200` | "Eureka has no Persona: Self is the operator-facing surface". | "The operator channel is this session. Eureka's Persona (the Mind's own mouth on Discord) is not the operator channel; a blocking question still goes to the operator here or by the notifier." By PR, with `skill-mind-self` (cut 7). |
| Eureka `README.md:63, 86-87, 95` | Links Epiphany as the organism; "Epiphany owns the schemas". | "Huginn owns the schemas (`eureka-pipeline`)"; the comparison reference stays as a dated document. |
| `~/.claude/doctrine/colossus.md:47, 51` | The Epiphany event as myth. | Stays. |
| `gamecult-ops/inventory.md`, runbooks, scripts, `systemd/` (B3) | The host body of Epiphany. | Deleted or rewritten in `mind-self-wake` r4 (cut 9). |

### B15. The governance the Mind's stewardship is an embryo of: unchanged from r3 B17

GCLP decision paths, membership separate from login, work and care work,
ledgers and decay, nothing pulling money unsupervised. The four attachments
in the rationale stand with "Epiphany" read as Eureka.

## The harvest

Order is the order of cuts; a later item may not land before an earlier one
it names. "Consumer" is who reads it today; "home" is where it lives after.

| # | What | Consumer today | Home after | Decides the home | Order |
| --- | --- | --- | --- | --- | --- |
| H1 | `epiphany-pipeline` leaf crate (B8) | Huginn (`huginn-mind`, `huginn-daemon`, `eureka-state`) | Huginn `crates/eureka-pipeline` (package and lib renamed; type ids unchanged), copied from Epiphany head `4006b14b` with the `leaf-key-bound` branch's change folded in or re-cut; `huginn-mind` takes a path dependency; the git edge dies | No fork (the only consumer is the home). `wire-names` decides the ids later, not this cut | 1 |
| H2 | `schemas/cultnet/` (thirteen pipeline schemas, `index.json`, README) and `gamecult.persona_state.v0.schema.json`; `epiphany.work_organ_state.v0` dies unless Hands finds a reader | The leaf's derivation test; VoidBot zod types; the Persona standard doc; Huginn's planned validator | Huginn `schemas/cultnet/` (same cut as H1; the test path becomes `../../schemas/cultnet`); the standard doc re-points (B14) | No fork | 1 |
| H3 | The unlanded leaf specs `cut-mind-authority.r1`, `cut-run-kind.r1`, `cut-leaf-key-bound.r1` | The mind | Re-admitted as r2 with repo `GameCult/Huginn`, base = the H1 landing commit; same content | No fork | 2 |
| H4 | The resident Self (`epiphany-swarm`, `resident_self.rs`) as rules (B10) | Nothing live | `crates/eureka-wake` in the body repo: brake and dial read, queue minus live claims through `HuginnClient`, at most one Self-run start; no grant/lease state of its own | `self-waker`, `dial-home`, `body-repo-home` | 5 |
| H5 | The Persona crossing and permit (`persona_discord_crossing.rs`, `persona_discord_permit.rs`, the permit binary; the two authority-map notes) | Bifrost's mouth (contract) | The Persona organ crate (H6) and its `eureka-persona-permit` binary; the notes under its `docs/` | `body-repo-home`, `wire-names` (what the signed request is called) | 6 |
| H6 | The Persona turn: the Interpreter's typed output (speech + state notes, `persona_conversation.rs`), the feedback-delivery import, the delivery-request signing | Nothing live | `crates/eureka-persona`: one connector request per turn with an output schema; reads the Persona document and ended runs from Huginn, feedback deliveries from Bifrost's outbox; puts the Persona document under CAS; signs one delivery request | `persona-turn-runtime`, `connector-owner`, `body-repo-home` | 6 |
| H7 | The model adapter (`epiphany-model-adapter`, `provider_transport.rs`) | Epiphany only | **Dies.** The connector contract is the lifted crate of B12; `cut-epiphany-connector-only.r1` is withdrawn | `connector-owner` (the lifted crate replaces it) | 4 |
| H8 | `epiphany_doctrine.md` (text), the README's open-project prose, `faculty-workflow-lessons-2026-09-04.md`, the CLA/LICENSE question | The charters' line 14 cites the doctrine; the skill cites the lessons | Eureka repo: `doctrine/self-improvement.md` (the per-lane law paragraph and the heartbeat and swarm-boundary lines), README opening, `references/`; the charters land in the same repo | `charters-into-eureka-repo` (ruled); whether the CLA applies to Eureka is the operator's, asked in the cut report, not a fork here | 8 |
| H9 | The mind's design record in `notes/` (`huginn-organ-cut.md` and the six `eureka-*`/postmortem notes) | Readers of Huginn's history | Huginn `docs/history/` verbatim, with a one-line provenance header | No fork | 1 (same cut as H1; prose only) |
| H10 | Host state and tooling of the Epiphany body (B3) | Nothing | **Deleted**: units, release roots, Idunn targets and recipe, users `epiphany` and `epiphany-model`, state and config roots; gamecult-ops scripts, runbooks, units; inventory rewritten | `host-and-units` | 9 |
| H11 | The GitHub repo | | **Archived** last: README pointer to Eureka and Huginn naming what moved and from which commit; final tag `archive-2026-10`; GitHub archive flag (operator action, read-only) | No fork; the operator flips the flag | 11 |

What dies without moving, said plainly: `epiphany-core` except H5 (atlas,
runtime spine, current-work ingestion, repository body observer, reasoning
contexts, repo model documents, coordinator and its results, agent launch,
substrate gate, packaged release, memory graph, surfaces, the state and
release binaries), `epiphany-openai-runtime` (the native cognition stack and
the Persona service binary), the model adapter, both tool crates, the four
empty crates, `apps/`, the Codex-era prompts, the GUI and dogfood sandboxes,
the whitepaper and investor documents (archived, not deleted), the host body.

## The model page

One row per persistent kind this design adds or changes. Rows for the
thirteen existing kinds are in `eureka-substrate-map.md`.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| `ruling` with `authority: Mind` (`cut-mind-authority` r2, Huginn) | `<campaign>:ruling:<label>`; the variant on the document separates Mind rulings from the operator's. | Created by the Mind's Self when it rules a substrate-internal fork; superseded like any ruling; the operator overturns one with an Operator ruling. Stored rulings decode unchanged. | The Mind's Self admits them; the `run` record is the attribution until `operator-authority-needs-operator-credential` lands. |
| `run` (instance-rooted; `cut-run-kind` r2, Huginn) | `<instance>:run:<label>`; label `mind-<date>-<n>` for her Self (allocated by the waker), the Self's chosen label for the operator-facing Self. Fields `instance, operated_by: Operator \| Mind, host, started_on, budget_usd, claims, campaigns`. | Opened at launch before any other admission; ended `Recorded { reason }` or `Withdrawn`; a live run older than 24 h is withdrawn by the waker's next cycle. A spec claimed by a live run is not in another run's queue. | The Self that runs it admits it; admission refuses a claim a live run holds (`AlreadyClaimed`). **The run is the grant**: the waker launches nothing when the queue minus live claims is empty, and the Persona turn reports ended runs. |
| `gamecult.persona_state.v0` for an instance (Huginn; `cut-huginn-persona-state` r3) | One global document per mind, `personaId` = instance slug; schema vendored by hash from Huginn's own `schemas/cultnet` after H2. | Seeded once from VoidBot's `persona:migrate-portable` output by the operator-facing Self; replaced whole by `PersonaPut { state, expected_updated_at }` under CAS; read by `PersonaGet`. | Huginn admission owns entry. The Persona organ (H6) is the only live writer. Forbidden: a local `.cc` as canonical; any second copy without `provenance.authority: projection`. |
| The brake and the burn-rate dial (question `dial-home`) | A: one root-owned control store on Yggdrasil, `/etc/gamecult/eureka/control.cc`, documents `eureka.control.brake.v0 { released, set_at, set_by }` and `eureka.control.burn_rate.v0 { heat: 0.05..2, set_at, set_by }`, written by `eureka-control` (a small CLI run as root over ssh), read by the waker and the permit issuer before every consequence. B: instance-rooted `brake` and `burn_rate` documents in Huginn, one in force, admitted under the operator credential. | A: absent brake reads as held; absent heat reads as 1.0. B: superseded by resolution; waits on the credential follow-up. | The operator only; no organ of hers can write the store (filesystem under A, credential under B). Derived: waker cadence (`cooldown / heat`, bounded), run size (`max_cost_usd = base_usd × heat` in every connector invocation and every Self-run `--max-budget-usd`). |
| The connector's caller spend ledger (lifted crate; `gamecult.model.caller_spend.v1`) | Keyed by caller runtime id and window start; sum of `cost_usd_estimate`, request count, window cap from the caller config. | Rolled per 24 h window; refusal `Budget` at the cap; projected into each daemon's presence health. | Each connector daemon owns its own; the caller config (root) sets the cap. Never the plan's own usage (`gap-usage-not-typed`). |
| Her identity on each surface | The Mind: instance `eureka`, forge user `eureka`, Discord display name Eureka (question `her-name` dies: there is no other name once the body is not Epiphany). The body: Linux user `eureka`, runtime id `eureka-yggdrasil` (a flag value on Bifrost's unit), Persona agent id and crossing type ids per `wire-names`. | Instance never renamed; runtime id set at rebinding; the display name and avatar are the Bifrost binding's fields. | The operator names her; `wire-names` is a Bifrost cut if it renames. |
| The Mind's credential (`CLAUDE_CODE_OAUTH_TOKEN`) | Two root-0600 `EnvironmentFile`s on Yggdrasil: the Claude connector daemon's and `eureka-self@.service`'s. Never user `eureka`. | Created with `claude setup-token`; rotated or revoked by the operator. | The operator. Question `host-and-units`. |
| The Mind's forge key per host | `eureka@<host>` keys on forge user `eureka`. | `provision-forge-minds-eureka`. | gamecult-ops runbook, run as root. |
| Her units and anchors (`cut-mind-self-wake` r4) | `eureka-wake.timer` + `eureka-wake.service` (or `eureka-wake.service` resident, per `self-waker`), `eureka-persona.service`, `eureka-persona-permit.service`, `eureka-self@.service`, `claude-connector.service`, `codex-connector.service` (existing unit, re-pointed at the split crate); mouth and permit anchors for runtime `eureka-yggdrasil`; the Bifrost mouth unit's three flags. | Hand-installed by gamecult-ops per `reader-deploy-owner` and the Huginn precedent; Idunn targets re-declared later as the deletion line. The Epiphany units, roots, users and targets are deleted first. | gamecult-ops owns the install; the operator creates the token and the anchors (root). |

Cells not fillable from the Body: the token's lifetime; the 24 h run bound
(a proposal); `base_usd`; whether the SDK's `total_cost_usd` tracks the
plan's accounting closely enough to be the dial's measure.

## The authority map

- **Owner:** Eureka, in three organs with one Mind. Huginn owns what is true
  (every document, every receipt, every claim). The waker owns when she
  works: it turns the brake, the dial and the queue minus live claims into at
  most one Self-run start. The Persona organ owns what she says and her one
  Persona document (one CAS put per turn, one signed delivery request per
  turn). Her Self run (a Eureka session under `eureka-self@`) owns, within the
  Mind's jurisdiction: which substrate work happens next (the fixed queue
  order), the dispatch of Imagination, Hands, Soul and Life, Mind-authority
  rulings on substrate-internal forks, merging her own PRs after a holding
  Soul verdict, opening campaigns whose repos are Mind repos.
- **Inputs:** her Persona document and the queue and ended runs (Huginn,
  through `HuginnClient`); the brake and the dial (the control store);
  Bifrost Persona-feedback deliveries (the outbox); her checkouts of Huginn,
  Eureka, gamecult-ops and the connector repos; the verify stopgap. Nothing
  from Starfire, nothing from the operator's projects, no prose handoff.
- **Outputs:** signed Discord delivery requests through the typed crossing
  (posted by Bifrost under a permit her issuer grants while the brake is
  released); one Persona-state put per turn; at most one Self launch per
  unclaimed queue, which opens a `run`, admits documents, pushes branches and
  PRs to the forge, merges; connector invocations carrying `max_cost_usd`.
- **Derived state:** HANDOFF prose files are display-only and die once `run`
  exists; "what is Self working on" is a query over live runs; "how active is
  the Mind" is the dial; "what did my Self do" is a query over ended runs,
  projected by her Persona turn in her voice; each connector's spend ledger is
  a projection of receipts; the resident Self's grant and lease are no longer
  owners, they are derived from `run` claims; Epiphany's runtime Mind is dead.
- **Forbidden writers:** she never sets the brake or the dial; never rules a
  fork outside substrate-internal ones; never edits `~/.claude/`; never pushes
  to GitHub; never runs on Starfire; never writes her Persona state anywhere
  but Huginn; never holds the Claude token (the Claude connector and the
  Self-run unit do); never posts to Discord outside the permit crossing. No
  Epiphany binary runs again. VoidBot's scheduler, worker, registry and heat
  never decide anything for her. No connector executes a tool call or hosts
  an agent session. The operator-facing Self never claims a spec a live Mind
  run holds and never writes a prose handoff in place of a `run`.
- **Shared paths:** both Selves admit through the same `eureka-state` tools
  and admission rules; both open a `run` first; both read the queue with the
  same recipes minus live claims; both land code through branch-protected PRs
  on the forge and the same merge rule; both verify through the stopgap;
  every model turn of hers and of Ghostlight's goes through the same connector
  contract (different daemons, one contract); every Discord post of hers goes
  through the same permit crossing.
- **Deletion line:** before her units run for the first time: Huginn's git
  edge to Epiphany is gone (H1); the three dead Epiphany units, the
  model-connector unit, the Idunn `epiphany` and `epiphany-model-connector`
  targets, `/srv/epiphany*`, `/var/lib/gamecult/epiphany`,
  `/etc/gamecult/epiphany`, users `epiphany` and `epiphany-model` are removed
  from the host; gamecult-ops' Epiphany scripts, runbooks and units are
  deleted; `cut-epiphany-connector-only.r1`, `cut-epiphany-persona-huginn.r1`,
  `cut-epiphany-self-eureka-run.r1`, `cut-mind-self-wake.r3` are withdrawn
  (the first dies, the others are re-cut against the body repo); the prose
  handoff files are retired into `run`s; the local `.cc` as canonical Persona
  store for instance `eureka` never exists; the two doctrine passages of B14
  are rewritten in the cut that lands the reader; Epiphany is archived last.

## The nine questions, re-checked

`stand`: unchanged, a ruling can pick an option as admitted. `changes`: the
options are re-admitted as r2 with the text here. `dies`: withdrawn, with
the reason; a successor question is named where one exists. Every question
that survives lists its options so a ruling can pick one
(`gap-answer-outside-options`).

1. **`self-run-actuator` changes.** Option `native-coordinator` dies with
   `epiphany-mvp-coordinator`. Remaining: **A `actuator-unit`**: the waker
   starts `eureka-self@<label>.service` under user `eureka-self` (root-0600
   `EnvironmentFile` with the setup-token, the `eureka` forge key, the Mind's
   checkouts, the `eureka-state` MCP config) through one sudoers line,
   passing `--max-budget-usd` from the dial; the run opens and ends a `run`
   document; the connector is untouched. **B `connector-session`**: the Claude
   connector gains an agent-session operation with Claude Code print mode
   behind it, so no unit of hers holds the token; the connector becomes an
   agent host with workspace, git and MCP reach, against its own boundary.
   Recommendation A: the connector's trustworthiness is that it transports a
   request and cannot execute a tool call; Claude Code is the Eureka
   pipeline's one runtime, a fact about the method, not a provider choice.
2. **`connector-owner` changes** (the operator's direction, B12). Option
   `generalize-codex-connector` dies: CodexConnector stays the Codex
   connector. Options for the ruling: **A `kit-and-connectors-beside`**: the
   shared machinery (contract types and refusals, envelope and framing,
   client, daemon loop with caller keys, limits, replay and config, Idunn
   health participant, `max_cost_usd`, `cost_usd_estimate`, the spend ledger
   and refusal `Budget`) is lifted into one crate; `codex-connector` keeps
   `provider_backend.rs` and becomes a thin binary over the kit; a
   `claude-connector` binary beside it whose backend is Ghostlight's SDK
   sidecar spawned under the daemon's user; an `openai-compatible-connector`
   binary whose backend is `LocalInferencePort`'s POST; Ghostlight's three
   in-process ports become three connector clients in its own campaign; her
   Persona organ configures the Claude connector. **B `kit-inside-codex-repo`**:
   the same split, but the kit is a workspace member of the CodexConnector
   repo and the other connectors depend on it by git; one repo, the name on
   the tin wrong for two of its three binaries. **C `claude-variant-in-her
   organ`**: her Persona organ spawns `claude -p` itself; no daemon change;
   violates `operator-provider-portable-connector` (a provider door in her
   process, the token in her user). Recommendation A. Two sub-forks the
   direction leaves open, which the ruling should also pick: (i) the kit's
   home, **new repo** (name the operator's; the default proposal is
   `GameCult/ModelConnector`) or **`CultLib/packages`** (recommended against:
   CultLib holds the CultCache/CultNet/CultMesh runtimes, and a connector kit
   is a consumer of them, not a runtime); (ii) the contract's shape, **keep
   the OpenAI-Responses request shape** that every Ghostlight port already
   lowers from, renaming the ids `gamecult.codex.*` to `gamecult.model.*` in
   one epoch bump when `connector-contract-untyped` converts it to CultCache
   typed documents (recommended), or **redesign** a provider-neutral request
   now (a second contract while the first still has three callers). An
   inconsistency to rule on, not resolve here: `operator-provider-portable-
   connector` says "a typed connector daemon", singular; the direction says
   connectors plural. The map reads it as one contract, several daemons.
3. **`persona-state-owner` dies.** Option `epiphany-runtime-store` died with
   the store; a question with one option is not a fork. Withdrawn; the one
   remaining shape is `cut-huginn-persona-state` r3's (consumer: the Persona
   organ) and the doctrine cut.
4. **`dial-home` changes.** Option `beside-the-brake` loses its store (the
   brake lived in Epiphany's control store). Remaining: **A `root-control-
   store`**: brake and dial as two documents in one root-owned control store
   on Yggdrasil (`/etc/gamecult/eureka/control.cc`), written by an
   `eureka-control` CLI as root, read by the waker and the permit issuer
   before every consequence; she cannot write it by filesystem permission;
   no new credential mechanism. **B `huginn-document`**: instance-rooted
   `brake` and `burn_rate` documents in Huginn admitted under the operator
   credential; waits on `operator-authority-needs-operator-credential`, until
   then anyone with the MCP could admit one. Recommendation A first, B the
   day the credential lands, with A's documents becoming derived then.
5. **`mouth-binding` stands** with the option text re-homed: **A
   `crossing-on-yggdrasil`** rebinds the live crossing to runtime
   `eureka-yggdrasil` (anchors, the permit issuer from H5 bound on
   `10.77.0.1`, the mouth unit's three flags), Aquarium channel kept or one
   the operator names; **B `webhook-cli-direct`** (a second mouth outside the
   brake); **C `silent-for-now`**. Recommendation A.
6. **`host-and-units` changes.** Option `interim-units-from-head` no longer
   means Epiphany head: **A `interim-units`**: gamecult-ops hand-installs her
   units from the body repo (`eureka-wake`, `eureka-persona`,
   `eureka-persona-permit`, `eureka-self@`, `claude-connector`; `codex-
   connector` re-pointed at the split crate), creates users `eureka` and
   `eureka-self`, deletes the Epiphany host body (H10), re-declares Idunn
   targets later as the deletion line. **B `idunn-targets-first`**: declare
   Idunn targets for every unit from the start; nothing hand-installed;
   correct by doctrine, and the Idunn recipe work lands first. **C
   `starfire`**: rejected by the load budget. Recommendation A, the Huginn
   precedent.
7. **`her-name` dies.** Its fork was Eureka versus the body's name; the body
   has no other name now. She posts as Eureka; the display name and avatar
   are set in the mouth rebinding. The surviving fork is the wire names
   (`wire-names`, new).
8. **`method-repo-home` dies.** Its options were relative to Epiphany's
   repo. The successor is `body-repo-home` (new): where her Rust organs
   live.
9. **`authority-thresholds` stands** unchanged: `binary-plus-visible-class`,
   `stakes-scaled-now`, `binary-unchanged`. Recommendation
   `binary-plus-visible-class`.

New questions this revision raises (admitted by the Self, each with options):

- **`self-waker`** (blocks cut 5). **A `one-shot-under-timer`**: `eureka-wake`
  is a one-shot binary under a systemd timer at a short fixed interval; each
  run reads the brake, the dial and the queue minus live claims and starts at
  most one Self run; idempotence is the run claim (`AlreadyClaimed`), cadence
  is `next_eligible` = last run end + `cooldown / heat`, computed from ended
  runs, no state file of its own. **B `resident-loop`**: a resident daemon
  porting `epiphany-swarm`'s cycle (grant, lease, backoff) over `HuginnClient`,
  holding `ResidentSelfState` in its own store. **C `persona-decides`**: the
  Persona turn itself decides when to launch her Self (VoidBot's initiative
  shape); the waker only runs Persona turns. Recommendation A: revision 2
  killed the timer because a timer plus pressure manufactured turns; with the
  claim in the mind that defect is structurally gone, and B keeps 3,800 lines
  of state machine for a fact the mind already holds. C puts a model call in
  front of every wake and makes cadence depend on a Persona's judgment, which
  the dial should own.
- **`persona-turn-runtime`** (blocks cut 6). **A `connector-request`**: the
  Persona turn is one typed request through the Claude connector with an
  output schema (speech, state notes), built by `eureka-persona` from the
  Persona document, feedback deliveries and ended runs; the connector
  enforces `max_cost_usd`. **B `claude-code-actuation`**: the Persona turn is
  a second print-mode session under the Self-run unit shape with a Persona
  brief; the connector has no consumer in Eureka. **C `voidbot-machinery`**:
  revive VoidBot's scheduler, Interpreter and bot as her Persona, reading
  Huginn through a TypeScript client (withdrawn follow-up `huginn-ts-client`
  returns). Recommendation A: it is what `operator-provider-portable-
  connector` asks for, it is the smallest organ (one request, one put, one
  signed document), and it keeps her speech behind the brake and the dial. B
  would make the whole organism Claude Code with the connector built for
  Ghostlight alone; C is the paused machinery with the defect plus a second
  Discord identity.
- **`body-repo-home`** (blocks cuts 5, 6, 9). **A `huginn-workspace`**: her
  organ crates (`eureka-pipeline`, `eureka-wake`, `eureka-persona` with the
  permit binary) are members of Huginn's workspace; one Rust workspace, one
  CultLib pin, one deploy runbook, one release symlink, one campaign repo;
  Huginn's README says it is Eureka's Mind and body. **B `new-body-repo`**: a
  new forge repo for the organs, Huginn stays the memory organ alone; two
  workspaces pinning the leaf by git again. **C `eureka-repo-workspace`**:
  the skill repo gains `crates/`; every `~/.claude/skills/eureka` checkout
  pulls the Rust body (the objection that killed `method-repo-home` B).
  Recommendation A.
- **`wire-names`** (blocks nothing until a rename is wanted; informs cuts 1,
  6, 9). The stored type ids `epiphany.pipeline.*.v2` and the epoch, and
  Bifrost's crossing constants (`epiphany.Persona`,
  `epiphany.persona_discord_delivery_*`, role `epiphany-persona-mouth`,
  `--epiphany-*` flags). **A `freeze`**: the names are wire history, kept as
  they are; the archive README explains them; no epoch bump, no Bifrost
  change. **B `rename-at-next-epoch`**: the pipeline ids become
  `eureka.pipeline.*.v3` in the next breaking leaf change (a migration of
  `mind.redb` under `mind-survives-upgrade`), and the crossing ids become
  `eureka.*` in one Bifrost cut with the mouth rebinding. **C `rename-now`**:
  both renames as cuts of this campaign. Recommendation A now, B recorded as
  the rule for when an epoch bump is paid for by something else; the
  `stored-documents-valid` invariant makes C a migration bought for a name.

## Rationale

### Why a second Self and not a rule (unchanged)

The operator has tried doctrine, a dedicated organ (Life), and considered a
scored exam. Each puts a barrier in the path of an agent whose reward is the
task in front of it. The only shape that uses the instinct instead of
fighting it is an agent whose task *is* the Mind.

### Why the harvest is small and the archive is large

Epiphany at head is 66k lines, of which the live mind uses 2,836 (the leaf)
and the design uses about 1,100 more (the crossing and permit) plus the shape
of one typed Interpreter output. The resident Self's 3,800 lines exist to
hold one fact, "which pressure is consumed by which launched turn", which
`run` claims hold in the mind with admission enforcing them. The Persona
service's 7,000 lines build a prompt, call a provider, parse a schema and
write memory documents into a private store; with the store gone and the
connector owning the call, that is one request builder. The model adapter
exists to choose between two providers inside Epiphany; the connector
direction gives that choice to a caller config and a daemon per provider.
Everything else (atlas, runtime spine, repository observer, coordinator) was
the Codex-era cognition body that the Eureka pipeline replaced in September.
The code is a liability; the harvest takes the invariants and the two
modules whose contract a running Bifrost unit already checks.

### Why the leaf moves first, and keeps its ids

Huginn's daemon, mind and client are built from the leaf; `code-from-main`
says the live mind is built from main of both repos. Archiving Epiphany with
the git edge in place would build the mind from an archived repo. The move
is a path dependency and a renamed package; the type ids stay because every
document in `mind.redb` is typed by them and `stored-documents-valid` is an
invariant of the campaign. A name in a wire id is a scar, not a dependency
(`wire-names`).

### Why the waker has no state (question `self-waker`)

Revision 2 killed the timer because VoidBot's timer plus unconsumed pressure
manufactured turns, and revision 3 took Epiphany's grant and lease as the
cure. The cure is a single fact with one owner: "this pressure is consumed by
this turn". The `run` kind puts that fact in the mind, where admission
refuses a second claim. Once the mind holds it, a waker that keeps its own
copy is a second owner, and a timer that fires into a mind with no unclaimed
work launches nothing. So the waker is stateless and the mind is the grant.

### Why the Persona turn is one connector request (question `persona-turn-runtime`)

A Persona turn is a prompt built from typed inputs and a typed output
(speech plus state notes): exactly the request-to-receipt shape the connector
transports, with `output_schema_json` already in the contract. Putting it
behind the connector keeps her cognition provider-portable (the ruling's
tradition), keeps the token out of her user, and keeps her speech behind the
brake and the dial through `max_cost_usd`. Her Self run is different in kind:
it is an agentic session with a workspace, which no connector may host.

### Why the connectors are several daemons over one kit (question `connector-owner`)

The operator's direction names the shape: CodexConnector is Codex; a Claude
connector sits beside it; shared machinery is lifted; Ghostlight's other
options become connectors too. The shared machinery is everything that makes
a connector trustworthy and the same for every provider (admission, keys,
replay, limits, receipts, spend, health); the backend is the only thing that
differs. One kit crate, three thin binaries, one contract; each daemon under
its own user with its own credential; callers route by model prefix in their
own config. No router daemon in front: that would be a fourth authority
whose only job is to know what the caller config already says.

### Why the brake and the dial share one root-owned store (question `dial-home`)

Revision 3 put the dial beside the brake because Epiphany already had one
operator control its organs obeyed. The brake needs a home now as much as the
dial does; the same reasoning gives them one root-owned store on the host,
read by the two organs that have consequences (the waker and the permit
issuer), written only as root. Huginn is the cleaner home for both the day it
can tell the operator's admission from hers.

### Why Persona state stays in Huginn opaque, validated, receipted (unchanged)

Hand-porting the v0 schema into Rust value types would drift from the
published JSON Schema; Huginn validates at the door, checks `personaId` and
the CAS, stores one envelope with a receipt. The schema is now Huginn's own
published file (H2), vendored by hash in the validator.

### Why her mouth is the typed crossing and not the webhook CLI (unchanged)

The crossing is live, signed on both sides, permit-gated by the brake,
receipted. Its Epiphany side is the one piece of Epiphany code worth moving
as code (H5), because Bifrost checks its contract today.

### Why `Mind` is an authority value and why `run` carries claims (unchanged)

A ruling's authority says whose word it is. Two Selves over one Mind need one
new fact, who is working what; the run carries its claims, its end and her
Self's results.

### The Self's fixed queue order (unchanged)

Derived at every launch, from the mind; the first non-empty step is the
run's work, and a run does one step:

1. A blocking question the operator has answered since the last run: skip.
2. Findings in force, `Blocker` or `High`, on Mind campaigns: Hands, then Soul.
3. Reports with no verdict on Mind campaigns: Soul.
4. Specs with no report, not blocked, not claimed by a live run: Hands.
5. `gap-*` follow-ups in force with no spec: Imagination maps one cut.
6. Nothing: the run ends `Recorded` "idle" with no model call beyond the
   queries. The waker should not have launched; if it did, that is a finding
   against its queue derivation.

### Means shape ends: the four attachments (ruling `operator-means-shape-ends`)

Unchanged from r3 with "Epiphany" read as Eureka: stakes-scaled authority
(question `authority-thresholds`), contributor credit at the landing record
(`credit-attaches-at-landing`), care work counted through `run` documents and
Life's named mutations (`care-work-counted`), no permanent aristocrat
(`mind-standing-decay`). The portable Persona document, per-Mind forge
accounts and one redb file per instance keep exit alive; the operator's
sentence "we shape the means with the agents we're building, with Epiphany
and how we steward it" now reads with Eureka as the agent being stewarded,
and the archive itself is the first act of that stewardship: a body that
stopped making sense is cut, not kept.

### Campaign home (proposal; the Self admits campaigns and targets)

Keep `eureka-substrate` for the mind's own cuts: `leaf-into-huginn` (H1, H2,
H9), `mind-authority` r2, `run-kind` r2, `huginn-persona-state` r3,
`huginn-mind-self-kinds`, `doctrine-persona-in-huginn` r3. Open a sibling
campaign **`eureka-body`** (title "Eureka's body: the harvest of Epiphany
and the Mind's own Self"), repos `GameCult/Huginn`, `GameCult/Eureka`,
`GameCult/gamecult-ops`, `GameCult/CodexConnector`, `GameCult/Epiphany` (as
harvest source and archive target), `GameCult/Bifrost` (no code change
expected; present so a `wire-names` rename could be a cut), plus the kit
repo once it exists. Its target's invariants: `one-contract-many-doors`
("every model turn of hers goes through the connector contract; no organ of
hers holds a provider credential or URL"), `persona-state-one-writer`,
`one-mouth-under-the-brake`, `token-out-of-her-reach`, `run-is-the-grant`
("no organ of hers holds pressure-consumption state outside the mind's run
claims"), `nothing-runs-from-epiphany` ("no unit, build or dependency of hers
resolves into the archived repo"). The three Epiphany-repo specs admitted
under `eureka-substrate` in r3 (`epiphany-connector-only`,
`epiphany-persona-huginn`, `epiphany-self-eureka-run`) and `mind-self-wake`
r3 are withdrawn when `eureka-body` opens; their successors below are
admitted there. `epiphany-cuts-need-a-campaign` closes with the opening.

### Proposed target r2 for `eureka-substrate` (the Self admits)

`canonical_implementations[0]` becomes "Key grammar and document kinds:
Huginn `crates/eureka-pipeline` (Rust), type ids `epiphany.pipeline.*.v2`
kept as wire names." `code-from-main` becomes "The Huginn daemon, the
`eureka-pipeline` leaf and the `eureka-state` client the live mind runs are
built from commits on Huginn's main." `not_in_scope` drops "Persona-state
reading in Huginn." and gains "Epiphany's runtime organs: archived; the body
campaign `eureka-body` holds the harvest." `canonical_implementations` gains
"Persona state for an instance: Huginn huginn-mind (opaque,
schema-validated), schema Huginn `schemas/cultnet/gamecult.persona_state.v0`;
live writer the Persona organ (`eureka-body`)." Invariants gain
`persona-state-one-writer` as r3 proposed.

### Cut order

Each line is a spec Hands can take with the target shape in this map; the
section in brackets is where the shape is.

1. **`leaf-into-huginn`** (Huginn; `eureka-substrate`; no fork). Copy
   `epiphany-pipeline` at Epiphany `4006b14b` to `crates/eureka-pipeline`
   (package `eureka-pipeline`, lib `eureka_pipeline`; a `git log` provenance
   line in its `Cargo.toml` comment), fold in the `eureka-substrate/
   leaf-key-bound` branch or leave `leaf-key-bound` r2 as cut 2's sibling;
   move `schemas/cultnet/` (pipeline schemas, `gamecult.persona_state.v0`,
   `index.json`, README rewritten for Huginn; `work_organ_state` dropped
   unless a reader is found) to Huginn `schemas/cultnet/`; the derivation
   test path becomes `../../schemas/cultnet`; `huginn-mind` takes
   `eureka-pipeline = { path = "../eureka-pipeline" }`; every `use
   epiphany_pipeline` in the 20 files becomes `eureka_pipeline`; `Cargo.lock`
   loses the git source; H9 notes land under `docs/history/` with a
   provenance header. Proof: `cargo test -p eureka-pipeline -p huginn-mind
   -p huginn-daemon -p eureka-state` on the stopgap; the live mind reads back
   after a daemon rebuild (`mind-survives-upgrade`). [B8, H1, H2, H9]
2. **`mind-authority` r2, `run-kind` r2, `leaf-key-bound` r2** (Huginn;
   `eureka-substrate`): the r1 content, repo and base re-pointed. [r3 model
   page rows; `huginn-organ-cut.md` once moved]
3. **`huginn-persona-state` r3** (Huginn): as r2 with the schema vendored
   from Huginn's own `schemas/cultnet` and the consumer named as the Persona
   organ. No fork.
4. **`connector-kit`** (CodexConnector and the kit repo; `eureka-body`;
   blocked on `connector-owner`): lift `lib.rs`'s contract, envelope, framing
   and client, `daemon.rs`, `idunn_health.rs` and `main.rs`'s daemon skeleton
   into the kit crate with a `ProviderBackend` trait (`execute(invocation) ->
   result`, `capability()`, `health()`); add `max_cost_usd`,
   `cost_usd_estimate`, the spend ledger and `Budget`; `codex-connector`
   becomes `provider_backend.rs` plus a `main.rs` over the kit, its recipe
   unchanged; `claude-connector` is a second binary whose backend spawns
   Ghostlight's sidecar (copied, not depended on) and maps `MaxBudgetUsd` to
   `Budget`; `openai-compatible-connector` is a third whose backend is one
   POST with a base URL. Proof: the Codex daemon's existing tests pass
   unchanged against the kit; a smoke of each binary refusing an expired
   invocation and an over-budget one. [B12]
5. **`eureka-wake`** (body repo; blocked on `self-waker`, `dial-home`,
   `body-repo-home`): the one-shot (or loop) of B10; reads the control
   store; derives the queue through `HuginnClient` with the recipes minus
   live claims; allocates `mind-<date>-<n>`; starts `eureka-self@<label>`
   through the sudoers line with `--max-budget-usd`; withdraws runs older
   than 24 h; test: two consecutive wakes over one unclaimed spec launch
   once (the second sees the claim). Depends on 2.
6. **`eureka-persona`** (body repo; blocked on `persona-turn-runtime`,
   `connector-owner`, `body-repo-home`, `mouth-binding`): H5 modules moved
   with their tests and the permit binary renamed `eureka-persona-permit`;
   the turn builder of H6 (inputs: `PersonaGet`, Bifrost outbox deliveries
   not yet acknowledged, runs ended since the document's `updatedAt`; one
   connector request with the Interpreter's output schema; `PersonaPut`
   under CAS; one signed delivery request into the crossing store); mention
   obligations per `epiphany-inbound-mentions` (renamed
   `persona-inbound-mentions` again). Depends on 3, 4. Test: a turn with the
   brake held signs nothing and puts nothing; a CAS conflict re-reads once.
7. **`skill-mind-self`** (Eureka; blocked on `self-run-actuator`): the Self
   brief her unit runs (provenance line per `wake-prompt-provenance`, the
   queue recipe, the coexistence rule), and the `SKILL.md:200` rewrite. By PR.
8. **`doctrine-repoint`** (Eureka, gamecult-ops, the charters once in the
   Eureka repo; absorbs `doctrine-persona-in-huginn` r3 and the charters cut
   of `charters-into-eureka-repo`): every row of B14 except the operator's
   `~/.claude/CLAUDE.md` (wording proposed in the cut report) and the rows
   marked history; H8's `doctrine/self-improvement.md` and README opening.
   Depends on 1 (the schema path) and 3 (the reader). By PR on the forge.
9. **`mind-self-wake` r4** (gamecult-ops; blocked on `host-and-units`,
   `mouth-binding`, `dial-home`): H10 deletion first; users `eureka`,
   `eureka-self`; the two token files; the control store and `eureka-control`;
   the units of the model page; anchors for `eureka-yggdrasil`; the Bifrost
   mouth unit's three flags; the runbook; the inventory rewritten. Depends on
   4, 5, 6 by prose.
10. **`huginn-mind-self-kinds`** (Huginn; `eureka-substrate`): pin bump,
    aliases, `AlreadyClaimed`, the live-claim subtraction in the recipes.
    Depends on 2.
11. **`epiphany-archive`** (Epiphany; `eureka-body`; last): README replaced
    by the pointer (what moved, to where, from `4006b14b`; what was deleted
    and why, one line each); tag `archive-2026-10`; the operator sets the
    GitHub archive flag. Depends on every cut above having landed, proven by
    `nothing-runs-from-epiphany`: no Cargo source, unit, script or runbook on
    the host or in the Mind's repos resolves into the repo.

### Rejected

- Keeping Epiphany alive as a dependency source after the archive
  (`code-from-main` would build the live mind from an archived repo).
- Moving the resident Self's state machine as code: a second owner of the
  consumption fact the `run` claim holds (B10).
- Moving the Persona service as code: 7,000 lines coupled to a dead runtime
  store for one request builder's worth of behaviour (B9, H6).
- A `ClaudeCode` provider inside her Persona organ, or `claude -p` spawned by
  it: a provider door in her process and the token in her user.
- Generalising CodexConnector into the one connector (r3): the operator's
  direction keeps it Codex.
- A router or mux daemon in front of the connectors: a fourth authority for
  what caller config already says.
- Reviving VoidBot's scheduler, worker, Postgres or bot for her: the paused
  replay defect and a second Discord identity (B7).
- A timer with its own pressure state (r2's defect) or a Persona that decides
  cadence (the dial's job).
- Renaming the stored type ids now: a migration bought for a name
  (`wire-names`).
- Folding the method repo into Huginn, or Rust crates into the skill repo:
  every skill install would pull a Rust body, every Rust build a skill.
- The webhook CLI as her mouth; a DM reader as the review channel; Rust
  value types for the v0 schema; a `claim` kind separate from `run`; running
  her on Starfire. All as before.
