# eureka-body: map (Imagination, 2026-10-08)

Status: map, Imagination (`imagination-eureka-body`), session `self-2026-10-08`.
Campaign `eureka-body` in Eureka's mind (instance `eureka`). Target in force
`eureka-body:target:r1`, whose doc is Huginn `docs/eureka-self-cut.md` at
`570f369` (map r4 of 2026-10-01). This page keeps body facts, the model page
(step 0b) and rationale. Questions, cut specs, follow-ups and rulings are typed
documents in the mind; read them by id. Self commits this page where the
target doc lives (Huginn `docs/`), probably as `docs/eureka-body-map.md`.

## Pinned heads (fetched 2026-10-08 ~12:10 UTC)

| Repo | Ref | Commit | Note |
| --- | --- | --- | --- |
| Huginn | `origin/main` = local `main`, `F:\Projects\Huginn`, clean | `6b496eb` | last commit 2026-10-02; deployed daemon is `107552d` (docs-only diff to main) |
| Epiphany | `origin/main` | `ef956865` | = Huginn's leaf pin; local checkout is on `codex/eureka-pipeline-state` `4006b14b`. GitHub `isArchived: false` |
| CodexConnector | `origin/main` | `3ddc12e` | local checkout is on `codex/ghostlight-release-binding` `6519289`; Ghostlight pins `68fe94b` |
| gamecult-ops | `origin/main` | `7e23387` | clean |
| Bifrost | `origin/main` | `160fac2` | clean |
| Eureka (forge) | `main` | `0bb414d` | scratch clone, removed after |
| Idunn | `origin/main` | `6667f0f` | read only; not a campaign repo |

## Body facts

Each line: what was run, where, result.

- **F1 Huginn never left Epiphany.** `grep epiphany crates/*/Cargo.toml` (Huginn 6b496eb):
  `crates/huginn-mind/Cargo.toml:22` pins `epiphany-pipeline` by git at Epiphany
  `ef956865`. `Cargo.lock:376-378` carries the git source. The map-r4 cut
  `leaf-into-huginn` was never specified; no `cut_spec` for it exists in any
  campaign (query of cut_spec headers under `eureka-substrate` and `eureka-body`).
- **F2 Leaf users in Huginn.** `pub use epiphany_pipeline;` at
  `crates/huginn-mind/src/lib.rs:62`; direct `use epiphany_pipeline` in huginn-mind
  `admission.rs:33` (26 mentions), `docs.rs:9`, `fixtures.rs:7`, `index.rs:10,157`,
  `mind.rs:16` (9), `query.rs:28,624`, `receipt.rs:19,50-52,314`, `refusal.rs:6`,
  `rows.rs:15`, `wire.rs:9`; through the re-export
  `huginn_mind::epiphany_pipeline::` in eureka-state `src/lib.rs:29`, `src/main.rs:25`,
  `tests/client.rs:17`, `tests/common/mod.rs:16`, `tests/mcp.rs:14,539`; huginn-daemon
  `src/daemon.rs:7,220,688,718,747`, `src/index/mod.rs:102`, `src/semantic_tests.rs:13`,
  `src/serve.rs:71,551`. Prose: `README.md:64`, `AGENTS.md:18`.
- **F3 The leaf at the pin.** Epiphany `ef956865`: `epiphany-pipeline/Cargo.toml`
  (lib `epiphany_pipeline`, deps anyhow, chrono, cultcache-rs at CultLib `8fc74c7`
  = Huginn's own CultLib pin, rmp-serde, schemars, serde; dev serde_json),
  `epiphany-pipeline/src/lib.rs` (schema-publication test reads
  `CARGO_MANIFEST_DIR/../schemas/cultnet` at lib.rs:2725). `schemas/cultnet/` holds
  13 `epiphany.pipeline.*.v2` schemas, `epiphany.work_organ_state.v0`,
  `gamecult.persona_state.v0`, `index.json`, README; `schemas/README.md`.
- **F4 Mind deployment.** `ssh yggdrasil`: `huginn.service` active, ExecStart
  `/opt/gamecult/huginn/current/huginn-daemon --state-root /var/lib/gamecult/huginn
  --instance eureka --bind 10.77.0.1:17872 ...`; `current ->
  releases/107552dc...` (Huginn merge of huginn-admission-rules; main adds docs only).
  UFW admits 17872/udp on wg-gamecult from 10.77.0.2 only (runbook
  `huginn-yggdrasil.md:38,124`). The daemon has no peer authentication
  (`serve.rs`: one session per peer, `max_peers = 256`); any reachable peer admits
  as any faculty.
- **F5 Epiphany residue on Yggdrasil (12:13 UTC).** Units `epiphany.service`,
  `epiphany-swarm.service`, `epiphany-heartbeat.service` inactive/dead;
  `epiphany-model-connector.service` loaded, disabled. Users `epiphany` (988; groups
  epiphany-state, bifrost-feedback-readers) and `epiphany-model` (984; group
  gamecult-model); no process runs as either. Roots and sizes: `/srv/epiphany` 6.8G,
  `/srv/epiphany-model-connector` 16G, `/var/lib/gamecult/epiphany` 1.9G (body.cc,
  heartbeat.cc, codex-home, artifacts, capstones), `/var/lib/gamecult/epiphany-model-connector`
  28K, `/etc/gamecult/epiphany` (bifrost-persona-feedback-anchor.msgpack, credentials/,
  idunn-provider-projection-anchor.cc, mcp.toml), `/etc/gamecult/epiphany-model-connector`
  (connection.key). Manifests `/srv/odin/deploy-manifests/epiphany`, `epiphany-capstone-17`,
  `epiphany.capstone-17.yggdrasil.yaml`, `epiphany-ox17`, `epiphany.ox17.yggdrasil.yaml`,
  `epiphany-swarm.service`, `epiphany.yggdrasil.Dockerfile`.
  `/etc/gamecult/idunn/daemon-health-trust.cc` contains the string `epiphany`.
  About 24.7 GB in all.
- **F6 What the live crossing still names.** `/etc/systemd/system/bifrost-persona-mouth.service`
  carries flags `--epiphany-persona-mouth-trust-anchor`, `--epiphany-persona-permit-rudp`,
  `--epiphany-persona-permit-trust-anchor`, `--epiphany-runtime-id epiphany-starfire`.
  gamecult-ops `runbooks/bifrost-persona-feedback-yggdrasil.md:17` names
  `/etc/gamecult/epiphany/bifrost-persona-feedback-anchor.msgpack`, `:45` the installed
  `/usr/local/sbin/bifrost-authorize-epiphany-release` (sealed historical name), `:67`
  the feedback target `--persona-id epiphany --repo GameCult/Epiphany --runtime-id
  epiphany-yggdrasil`. These belong to the mouth rebinding, not to the teardown.
- **F7 Nothing Epiphany runs on Starfire.** `Get-Process`, `Get-ScheduledTask`,
  `Get-Service` filtered on epiphany|eureka|persona|bifrost: empty. The permit
  issuer the crossing expects on Starfire is not running, so the mouth cannot
  obtain a permit today.
- **F8 Idunn still lists Epiphany.** Idunn `deploy/legacy/README.md:51-52` lists
  targets `epiphany`, `epiphany-capstone-17`; `deploy/legacy/idunn-yggdrasil:126-135`
  dispatch, `:312,:321` restart/deploy cases; `deploy/legacy/idunn-yggdrasil.sudoers:10-11,19`.
  The precedent for retirement is the same file's "Retired targets"
  (bifrost-persona-feedback, 2026-09-30). Idunn is not one of `eureka-body`'s repos.
- **F9 Codex is dead in practice.** Idunn `docs/migration.md:49-55`: "The operator is
  not returning to Codex"; the deployed daemon failed with `401 token_expired`. On
  Yggdrasil `codex-connector.service` is disabled, inactive since 2026-09-05 21:54 UTC,
  and nothing listens on 4103. Ghostlight's codex-connector library dependency is pinned
  at `68fe94b`, so a kit lift does not move Ghostlight.
- **F10 CodexConnector at origin/main 3ddc12e** (2026-09-13): `src/lib.rs` 1,958,
  `daemon.rs` 1,180, `provider_backend.rs` 1,463, `idunn_health.rs` 1,396, `main.rs` 188,
  `deployment/idunn/recipe.toml`. Contract types: `CodexProviderRequest` lib.rs:76-94,
  `CodexInputItem` :212-229 (UserText, AssistantText, ToolCall, ToolResult; no
  reasoning item), `CodexTransportInvocation` :238-245 (no `max_cost_usd`),
  `CodexTransportOutcome::Completed` :449-462 (tokens, no cost), `CodexRefusal`
  :464-474 (no `Budget`). Schema ids `gamecult.codex.*.v2` lib.rs:51-55.
- **F11 gamecult-ops Epiphany tooling** (`git ls-files | grep -i epiphany`, 7e23387):
  `compose/epiphany.capstone-17.yggdrasil.yaml` 38, `compose/epiphany.yggdrasil.Dockerfile` 55,
  `runbooks/epiphany-yggdrasil-deploy.md` 196, `runbooks/epiphany-starfire-bifrost-crossing.md` 101
  (live crossing binding, kept), scripts `bootstrap-epiphany-yggdrasil.sh` 134,
  `check-epiphany-yggdrasil.sh` 136, `construct-epiphany-image-yggdrasil.sh` 91,
  `deploy-epiphany-yggdrasil.sh` 499, `deploy-epiphany-capstone-17-transaction-yggdrasil.sh` 5,
  `deploy-epiphany-capstone-17-yggdrasil.sh` 79, `epiphany-capstone-17-connector-bridge-yggdrasil.sh` 50,
  `provision-epiphany-provider-health-yggdrasil.sh` 116, `seal-epiphany-capstone-failure.sh` 67,
  `test-deploy-epiphany-yggdrasil-order.sh` 144, `test-epiphany-container-promotion.sh` 47,
  `systemd/epiphany.service` 31, `systemd/epiphany-swarm.service` 34; kept for the mouth
  cut: `allow-starfire-epiphany-persona-permit.ps1`, `bifrost-authorize-epiphany-release.sh`,
  `configure-voidbot-epiphany-feedback-target-yggdrasil.sh`. Inventory prose:
  `inventory.md:581-588` (Starfire as Epiphany body), `:726-730` (archived Verse residue),
  `:773-785` (intended Epiphany deployment body). `docs/repo-census-2026-09/` is history.
- **F12 Eureka repo (forge main 0bb414d)** carries the charters and doctrine (PR #3
  landed). Epiphany citations remain: `README.md:63,86-87,95`, `SKILL.md:3,8-9,146`,
  every charter's line 14 ("Self-improvement law (Epiphany `epiphany_doctrine.md`)"),
  `agents/hands.md:28`, `agents/soul.md:48`, `agents/life.md:24`, `doctrine/persona.md:3,13`,
  `doctrine/CLAUDE.md:31,229,233`, `doctrine/colossus.md:47,51` (lore, kept).
- **F13 eureka-substrate specs written against Epiphany.** `cut-mind-authority.r1` and
  `cut-run-kind.r1` have repo `GameCult/Epiphany`, base `ef956865`;
  `cut-huginn-persona-state.r2` base `bf73c42`; `cut-doctrine-persona-in-huginn.r2`
  gamecult-ops base `2aeb7364`. None has a report. After `leaf-into-huginn` they point at
  a repo nothing builds from.
- **F14 Discord developer policy** on training with message content: the policy page
  (support-dev.discord.com article 8563934450327) answered 403 to a fetch on 2026-10-08.
  Self's recollection, unverified: the policy forbids using message content obtained
  through the API to train AI models without express permission. An Eyes pass must
  verify before question `persona-trace-retention` is ruled.

## The model page (step 0b)

One row per persistent kind this body introduces or changes. Rows for the thirteen
pipeline kinds are in Huginn `docs/eureka-substrate-map.md`.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| Pipeline wire ids | `epiphany.pipeline.<kind>.v2`, epoch `epiphany.pipeline.epoch.v2`, defined by the leaf. After `leaf-into-huginn` the crate is Huginn `crates/eureka-pipeline` (lib `eureka_pipeline`), ids unchanged. | Renamed once to `eureka.pipeline.<kind>.v3` by one `mind.redb` migration, at the next breaking leaf change or before the first hosted mind (ruling `wire-names-rename-before-hosting`). Adding the `run` kind or the `Mind` authority variant is additive (old stored documents decode; no migration), so it does not trigger the rename; a field added to an existing struct would. | Huginn owns the leaf. The migration is a live-mind upgrade, which is the operator's (`mind-rules-which-forks`). |
| Leaf crate and schema catalogue | Huginn `crates/eureka-pipeline`, `schemas/cultnet/` (13 pipeline schemas, `gamecult.persona_state.v0`, index, README). `epiphany.work_organ_state.v0` dies: no reader outside Epiphany (grep, F1/F3). | Copied once from Epiphany `ef956865` byte-for-byte, then renamed in a separate commit; Epiphany's copy is archived with the repo. | Huginn. Forbidden: any Cargo source resolving into GameCult/Epiphany (`nothing-runs-from-epiphany`). |
| `run` | `<instance>:run:<label>` (cut-run-kind r1). Label `mind-<YYYY-MM-DD>-<n>` allocated by the waker as one plus the count of that day's runs; two wakers racing produce the same key with different content, which admission answers `Conflict`, so the name stays injective. | The waker admits it open, with its claims, before starting the unit (the run is the grant). The Self in the unit admits under that label and ends it `Recorded { reason }`; a run with no end after 24 h is `Withdrawn` by the next wake. Initiative runs (empty queue, `no-idle-time`) carry no claims; the dial bounds them. | Opening: the waker. Ending: the Self that worked it; the waker only withdraws dead ones. Forbidden: Hands, Soul, Imagination, Life. |
| Repetition breaker | No state of its own (rulings `self-waker`, `merge-gate-own-soul`). Trip condition derived from ended runs over one claimed item: last 3 ended runs admitted nothing but themselves, or 10 of the last 50. | When tripped, the waker admits one `question` labelled `breaker-<item local>` under the item's campaign, `raised_in` the item; the existing blocked-spec recipe then removes the item from the queue. Answered by a ruling or withdrawn; either reopens the item. | The waker admits the question; any Self may answer it (substrate-internal, `mind-rules-which-forks`). |
| Brake | One document per instance in a root-owned CultCache store `/etc/gamecult/minds/<instance>/control.cc`, type `eureka.control.brake.v1 { released: bool, set_at, set_by }`. | Replaced whole by the operator's CLI; never appended. Absent or undecodable reads as held. Holding it also stops running units (`systemctl stop`, ruling `self-run-actuator`). | Operator only (ruling `brake-dial-root-store-now`); her units read it, and the file is not writable by any user she runs as. |
| Burn-rate dial | Same store, `eureka.control.burn_rate.v1 { heat, base_cooldown_s, base_run_usd, set_at, set_by }`; `heat` in 0.05..=2.0. | Replaced whole by the CLI. Absent or out of bounds reads as heat 0, which launches nothing (fail closed, LiteLLM scar rider of ruling `connector-owner`). Schedules and her own rest arrive later as follow-up `burn-schedule-and-rest`. | Operator. Derived (never stored): wake cadence `base_cooldown_s / heat`, per-run cap `--max-budget-usd = base_run_usd × heat`, per-request `max_cost_usd`. |
| Connector request and transcript items | Request: `(caller_runtime_id, request_id)`, the replay key the daemon already keeps (`replay.cc`). Items: UserText, AssistantText, ToolCall, ToolResult, plus `ReasoningState { provider, model, opaque }` (ruling `connector-opaque-state-item`). Contract ids become provider-neutral `gamecult.model.*.v3` (follow-up `connector-contract-untyped`); the connector serves Ghostlight too, so not `eureka.*`. | Invocation expires at `expires_at_unix_ms`; replay record per key; restart-era `Indeterminate`. The caller stores the transcript and replays it verbatim; foreign-tagged reasoning state inside an in-flight tool exchange is refused typed; completed-turn foreign state is settled per provider in the connector cut (follow-up `connector-reasoning-state-cheap-providers`). | The connector produces, reads or refuses reasoning bytes; callers never branch on provider. |
| Connector spend ledger | `gamecult.model.caller_spend.v1` keyed by caller runtime id and window start, in each daemon's state root. | Rolled per window; refusal `Budget` at the cap; durable, and a missing ledger refuses the request. | Each daemon writes its own; the root caller config sets the cap. |
| Persona turn traces | **Empty: question `persona-trace-retention`.** Candidate: per turn, the request digest, the transcript items and the typed output, keyed by the turn's run. | Empty: how long they are kept, whether Discord-sourced content may enter a training export (F14). | Empty: who may export them. |
| Persona state | `gamecult.persona_state.v0`, one per mind, `personaId` = instance (cut-huginn-persona-state r2, eureka-substrate). | Replaced whole under CAS with a receipt; she may rewrite any field (`identity-write-all-visible`). | The Persona organ is the only writer. |
| Identity diffs | The pair of receipts of two consecutive Persona puts whose identity fields (publicName, publicDescription, presentation, values) differ. Derived, not stored. | Composed by the Persona organ at put time and sent as one delivery request through the crossing; a failed post is retried by Bifrost's delivery journal, not by her. | Persona organ composes; Bifrost posts under the permit. No rate limit. |
| Discord body surfaces | Outbound: crossing ids `epiphany.persona_discord_delivery_request.v0` and `..._permit.v0`, agent `epiphany.Persona`, runtime `epiphany-starfire` (F6), renamed to `eureka.*` and `eureka-yggdrasil` in the mouth rebinding (`mouth-crossing-on-yggdrasil`). Inbound: Bifrost persona-feedback deliveries for target `epiphany` (F6). Persona-session tools reach both (`body-includes-discord`). | Request, permit, post, receipt (Bifrost journal). Inbound deliveries: **unprobed** who records that a delivery was answered; `run-is-the-grant` forbids her organ from holding that fact. Blocks the Persona cuts only, not the cuts admitted today. | Bifrost owns posting and delivery records; her permit issuer grants while the brake is released. |
| Units and instance templates | `mind-wake@<instance>.timer/.service`, `mind-self@<instance>:<label>.service`, `mind-persona@<instance>`, `mind-permit@<instance>`; users `mind-<instance>` (organs) and `mind-<instance>-self` (Self runs); state `/var/lib/gamecult/minds/<instance>/`; control `/etc/gamecult/minds/<instance>/`. Instance `eureka`. | Hand-installed by gamecult-ops now; Idunn targets declared before the first hosted member, which deletes the hand install (`mind-units-templated-interim`). | gamecult-ops installs; Idunn later. |
| Self-run credential | `CLAUDE_CODE_OAUTH_TOKEN` in a root-0600 EnvironmentFile per instance. | Created by `claude setup-token`; rotated by the operator. | **Open: question `self-run-token-reach`.** The Self run's `claude` process can read its own environment, so `token-out-of-her-reach` holds only if the operator names this unit (F4 of the prior-art file, follow-up `token-reach-two-units`). |
| Wake frame | Fixed prompt frame (provenance, jurisdiction, brake obedience) in `/etc/gamecult/minds/<instance>/wake-frame.md`, root-owned. | Changed only by an install. | Host config (ruling `self-waker` rider 2); never a file her runs merge into. |
| Epiphany residue | F5, F8, F11. | Deleted by `epiphany-body-teardown`; the repo archived last. | gamecult-ops for host and repo tooling; Idunn for its legacy target list (follow-up). |

## Rationale

### Why the order is leaf, teardown, control, wake

The four specs admitted today depend on nothing the operator has to rule except the
wake target. `leaf-into-huginn` removes the only build edge into Epiphany and is the
base every Huginn-side body cut needs (the `run` kind re-pointed, Persona state, the
v3 rename). The teardown removes 24.7 GB and the tooling that would otherwise be
re-read by every later pass. The control store comes before the waker because the
waker's first rule is "read the brake before any consequence". The waker raises
`wake-target`, because the operator's 2026-10-03 ruling (`body-includes-discord`, "the
Persona session, which is also what drives the Self") and the Defaulted `self-waker`
("starts at most one Self run") read differently.

### One crate for her organs

Her organs live in Huginn's workspace (`organ-homes-huginn`). They share the control
reader, the client and the instance templating, and they deploy together. So they are
one crate, `mind-body`, with one binary per earned process (`mind-control` for the
root CLI, `mind-wake`; the Persona organ and permit issuer later), not a crate per
organ. The crate name is instance-generic because the units are templates.

### Why the kit keeps its Codex backend

Idunn's migration doc says Codex is dead (F9). The kit is lifted out of CodexConnector,
and the Codex backend's existing tests are the only proof that the lift is behaviour
neutral. Keeping the backend compiling costs nothing; deploying it is not proposed.

### Undone (not mapped this pass)

In dependency order, with what blocks each:

1. eureka-substrate re-points: `mind-authority` r2, `run-kind` r2, `huginn-persona-state` r3
   onto Huginn after `leaf-into-huginn` (follow-up `substrate-specs-repoint`). The waker
   needs `run` and `AlreadyClaimed` (`huginn-mind-self-kinds`, never specified).
2. `connector-kit-lift`, `connector-spend`, `connector-reasoning-item`,
   `connector-contract-v3`, `claude-connector`, `openai-compatible-connector`: blocked
   on the kit repo joining the campaign (follow-up `body-campaign-repos`).
3. `self-run-unit` and `skill-mind-self` (Eureka): blocked on `self-run-token-reach`
   and `wake-target`.
4. `mind-units-install` (gamecult-ops): after 3; needs `body-repos-on-forge`.
5. `mind-persona` and `mouth-rebinding` (Huginn, Bifrost): blocked on `wake-target`,
   `persona-trace-retention`, the inbound-delivery cell, and the connector cuts.
6. `doctrine-repoint` (Eureka, F12), `epiphany-archive` (last).

## Admitted this pass (receipts in the mind)

Specs, in dependency order: `eureka-body:cut_spec:cut-leaf-into-huginn.r1`,
`cut-epiphany-body-teardown.r1` (independent), `cut-mind-control-store.r1`,
`cut-mind-wake.r1` (blocked: `wake-target`, and the run kind in Huginn).
Questions: `eureka-body:question:wake-target`, `self-run-token-reach`,
`persona-trace-retention`. Follow-ups: `eureka-body:follow_up:substrate-specs-repoint`,
`body-campaign-repos`. H9 sources exist at Epiphany ef956865: notes/huginn-organ-cut.md,
eureka-body-plane-cut.md, eureka-pipeline-state-cut.md, eureka-pipeline-state-target.md,
eureka-query-prior-art.md, eureka-read-side-cut.md, postmortem-eureka-schema-phase.md.
Huginn has no LICENSE file (ruling `license-split` wants AGPL for the substrate; not
this campaign's cuts, noted for Self).
