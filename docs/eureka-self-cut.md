# The Mind's own Self: map

Status: map, Imagination (`imagination-self`), revision 3 of 2026-10-01,
session `self-2026-10-01b`. Campaign `eureka-substrate` in Eureka's mind
(instance `eureka`). Questions, cut specs, follow-ups and rulings are typed
documents there; this page keeps body facts, the model page and rationale.
Target: `docs/eureka-substrate-target.md` (`133324a7`). Rulings this revision
rests on: `eureka-substrate:ruling:operator-means-shape-ends`,
`:operator-eureka-adopts-epiphany-body`, `:operator-this-is-epiphany`,
`:operator-provider-portable-connector`, `:operator-mind-runs-own-self`,
`:operator-mind-mission`, `:mind-self-burn-rate`, `:operator-persona-in-mind`,
`:mind-rules-which-forks`, `:mind-self-merge-authority`,
`:charters-into-eureka-repo`.

The operator's words, in order. The first two were the first revision's law
(both admitted as rulings; the first superseded by the second):

> Eureka thinks in campaigns, right? Well, we already have a Mind campaign, and
> we're adding evidence to it with every run. The design currently delegates
> ultimate design authority and work scheduling to the operator, but neither of
> these is strictly necessary. I want a Self operated by Mind directly.

> The Self I talk to here in this one projection of Eureka's mind, and it's how
> I get it to work on the stuff I care about, like Aetheria. The Mind must
> operate its own Self instance, with the goal of improving itself.

The second revision's law:

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

The third revision's law:

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

So: Eureka is the first Mind's name (instance `eureka`, forge user `eureka`,
the forge itself). Epiphany is her body and the open project. The faculty
method (pipeline, charters, briefs, tools; today the `GameCult/Eureka` repo)
is one of her organs, not a sibling product. Where this map says "Epiphany",
read "Eureka's body"; where it says "her", read Eureka.

What stands from revision 1: the bloodhound observation ("RLVR-trained agents
chase the objective in front of them, steering be damned") means the Mind's
Self is a second agent whose only objective is the Mind, not a rule added to
the operator-facing Self. What stands from revision 2: the waking organ is a
Persona with initiative pressure, memory and a mouth, who dispatches her own
Eureka Self when her pressure says so; the timer is dead. What revision 3
changes: the organism is Epiphany. Her resident Self (`epiphany-swarm`), her
native Persona (`epiphany-persona-service`, Projector, Persona, Interpreter),
her permit issuer and her typed Discord crossing are the body. VoidBot's
scheduler, Interpreter prompt, registry and heat dial are demoted to prior art
(B7). Model access goes through one typed connector daemon with Claude Code
print mode as one provider behind it (B13-B14). The Persona document in Huginn
stays the one Persona state; Epiphany reads and writes it (B15).

Pinned heads for every fact below:

| Repo | Ref | Commit |
| --- | --- | --- |
| Huginn | `origin/main` | `bf73c42` (docs only over `a699630bbc49552fa747e9c0fabba24598fec77a`; crate line numbers hold at both) |
| Epiphany | local `main`, `F:\Projects\Epiphany` (clean) | `4006b14bf29b49410bd18b43acb4d42c3c4e2443` (2026-09-30). Huginn's leaf pin is `ef956865`; the two differ only in notes and the leaf's CultLib pin. |
| CodexConnector | `main`, `F:\Projects\CodexConnector` | `65192892f0ed829b8a228eabba44a4f3ba0f7268` (2026-09-04; frozen since) |
| Ghostlight | `main`, `F:\Projects\Ghostlight` | `eeb3a68` (2026-09-23) |
| Bifrost | `main` `F:\Projects\Bifrost`; release on Yggdrasil `cb3239aa-f67f5122` | `160fac2` (2026-10-01) |
| gamecult-ops | `main` | `2aeb7364fffcd404aada28bfd30fb4e5c753cf53` |
| Eureka skill | `main` (checkout `~/.claude/skills/eureka`) | `b260fbeec6d96ff06882ed9c37d3e1204c424e99` |
| VoidBot | `main`, working tree `F:\Projects\VoidBot` (clean) | `46d891b` (2026-09-13) |
| Idunn | `main` | `6667f0f` (2026-10-01) |

Probes of revision 1 ran from Starfire 2026-10-01 18:40-19:10 UTC; revision 2
(B7-B10) 19:40-20:10 UTC; revision 3 (B11-B16 and the re-checks in B3, B5)
after 20:15 UTC, from Starfire over `ssh ygg` (read-only; `sudo -n` only for
`ls`, `cat` of deployment records and `journalctl`). Repo facts for Epiphany,
CodexConnector and Ghostlight come from two Explore passes over the working
trees (nothing built, nothing run) and my own reads. `lib.rs` is
`epiphany-pipeline/src/lib.rs`.

## Body facts

### B1. The Mind (Huginn on Yggdrasil)

- `whoami` (revision 3): instance `eureka`, endpoint `rudp://10.77.0.1:17872`,
  reachable, 422 documents, 133 receipts, index `Current`, epoch
  `epiphany.pipeline.epoch.v2`.
- `huginn.service` (probe: `systemctl cat`): user `huginn`,
  `--bind 10.77.0.1:17872`, hand-managed ("interim unit until Idunn owns it").
  `ss -lunp` shows the socket only on the WireGuard address `10.77.0.1`
  (`wg-gamecult`). So the mind is reachable from Starfire over the mesh and
  from Yggdrasil itself, and from nowhere else.
- Campaigns in the mind: `idunn-watchdog` and `eureka-substrate`.
  `eureka-substrate`'s repos are `GameCult/Huginn`, `GameCult/Epiphany`,
  `GameCult/Eureka`, `GameCult/gamecult-ops`; a campaign has no resolution and
  no revision, so a CodexConnector or Bifrost cut cannot be a `cut_spec` under
  it (B16). Its target r1 lists "Persona-state reading in Huginn" under
  `not_in_scope`; the Persona ruling contradicts that line (proposed r2 in the
  rationale).
- Kinds: thirteen (`lib.rs:760-790`). None records a Self session, a claim on
  a spec, a burn rate, Persona state or a landing (`gap-landing-untyped`).
- `RulingAuthority { Operator, Standing, Defaulted }` (`lib.rs:458`); a unit
  enum variant added by name is a widening (map `eureka-substrate-map.md` B1).
- `Faculty` has `Operator` and `SelfFaculty` (`rows.rs:42`); admission does not
  check faculty (`gap-faculty-attribution`). Nothing in the mind today can tell
  the Mind's Self from the operator-facing Self except the `session` label on
  the receipt; follow-up `operator-authority-needs-operator-credential`.
- Instance-rooted kinds are the pattern for a run record (`lib.rs:913-946`;
  Huginn admission for `hand_off` at `admission.rs:231-262, 328-330`).
- **The Rust client exists.** `crates/eureka-state` is a library plus a binary:
  `HuginnClient { endpoint, instance, timeout }` with one method
  `call(HuginnMindRequest) -> Result<HuginnMindResponse, ClientError>` over
  `cultnet-rs` RUDP, resolving `Deferred` bodies (Huginn Cut 13a, merged
  `33641f4`; `notes/huginn-organ-cut.md:447-456, 474-482`). Epiphany is Rust,
  so it reaches the mind through this crate as a git dependency on Huginn. No
  TypeScript client is needed by this design (revision 2's `huginn-ts-client`
  is withdrawn).

### B2. Claude Code on Starfire: auth, headless mode, usage

- `claude --version`: 2.1.268. Account (`~/.claude.json` `oauthAccount`,
  secrets not read): Max plan, `billingType: stripe_subscription`, rate-limit
  tier `default_claude_max_20x`, extra usage off. No API key: every model call
  the Persona or her Self makes spends the operator's subscription quota.
- The interactive OAuth access token lives eight hours
  (`~/.claude/.credentials.json` `expiresAt`); a running subagent does not
  survive the parent's token expiring (`401 OAuth access token has expired`,
  this session's transcript).
- `claude setup-token` makes a long-lived subscription token read from
  `CLAUDE_CODE_OAUTH_TOKEN` (61 references in the binary). This is the
  headless credential. Its lifetime and revocation are the operator's.
- The token reaches `claude` as an environment variable, so the agent inside
  that process can read it with one shell call. What protects it is the reach
  of the user it runs as, not secrecy from the agent (revision 2 demotion).
  Revision 3 narrows where it lives: the connector daemon's user (B14) and the
  Self-run unit's user (B12), never Epiphany's own user.
- Print mode: `claude -p` with `--output-format json|stream-json`,
  `--json-schema`, `--agents <json>`, `--mcp-config`, `--allowedTools`,
  `--permission-mode`, `--max-budget-usd <amount>`, `--bg`, `--resume`,
  `--fallback-model`. The `json` output carries `total_cost_usd` and usage. The
  Claude Agent SDK (Ghostlight's sidecar, B13) exposes the same: per-model
  `modelUsage`, `total_cost_usd` as a client-side estimate, and a
  `MaxBudgetUsd` fault, so a per-request spend cap is enforced by the provider
  child itself.
- Plan usage as data: readable only through the desktop app's `get_usage`
  (`gap-usage-not-typed`, in force). The CLI binary knows no usage endpoint.
  **There is no typed reading of plan usage reachable from Yggdrasil.** The
  connector's cost field (B14) is an estimate summed per caller, not the plan.
- Local scheduled tasks and cloud routines: dead for this purpose (revision
  2: workstation load budget, no overnight burns, off-mesh, no forge key).

### B3. Yggdrasil: what runs, what is dead, and whose body it is

`systemctl list-units --all` for `epiphany* codex* ghostlight* huginn*
bifrost*` (revision 3):

| Unit | State | Note |
| --- | --- | --- |
| `huginn.service` | active | the mind (B1) |
| `bifrost.service`, `bifrost-persona-feedback.service` | active | Bifrost alpha; the Persona-feedback provider (user `bifrost-feedback`) |
| `bifrost-persona-mouth.service` | **active** | "Bifrost authenticated Epiphany Persona Discord mouth" (B5) |
| `bifrost-watchdog-notice.timer` | active | once a minute |
| `epiphany.service`, `epiphany-swarm.service`, `epiphany-heartbeat.service` | dead (enabled) | the Aug 22 body (below) |
| `epiphany-model-connector.service` | dead (disabled) | Epiphany's first connector, Aug 24 (B14) |
| `codex-connector.service` | dead (disabled) | the standalone connector, last alive 2026-09-05 21:51 UTC, stopped on a Codex `401 token_expired` (journal) |
| `ghostlight-dungeon.service` | dead | its unit still names `GHOSTLIGHT_MODEL_PROVIDER=codex-connector` at `127.0.0.1:4103`; its code no longer opens that lane (B13) |

- **The deployed Epiphany body is older than the repo.** `/srv/epiphany/app/current`
  (Aug 22) holds 18 binaries, among them `epiphany-daemon-supervisor`,
  `epiphany-heartbeat-store`, `epiphany-model-runtime`,
  `epiphany-persona-discord-permit`, `epiphany-persona-feedback-ingress`. Head's
  `Cargo.toml:60-100` builds nine: `epiphany-release`, `epiphany-state`,
  `epiphany-repository-body`, `epiphany-swarm`, `epiphany-persona-service`,
  `epiphany-persona-discord-permit`, `epiphany-mvp-coordinator`,
  `epiphany-openai-runtime` (the model runtime bin) and the tool runtime. The
  heartbeat scheduler authority was deleted on 2026-08-23 (`4d0db2c7`), the
  semantic projector on the same day (`856648de`), and `epiphany_specialists.toml`
  with "fictional contracts and dead prompt surfaces" (`ab3cacfb`). So
  `epiphany.service` (supervisor) and `epiphany-heartbeat.service` cannot be
  rebuilt from head: they are dead units with no source, and the deletion line
  of this map removes them. `epiphany-swarm.service` can be rebuilt; its unit
  ran `--model-provider openrouter --model stealth/ox-alpha` with an OpenRouter
  key via `LoadCredential`, `--codex-home`, `--max-steps 1`,
  `--cooldown-seconds 120 --idle-sleep-seconds 15`, and read Bifrost Persona
  feedback from `/srv/bifrost/persona-feedback/outbox/deliveries.cc`. Its last
  journal (Aug 22 17:46) reports `status: braked` and the wake authority
  quoted in B11.
- Users that exist: `epiphany` (988, home `/var/lib/gamecult/epiphany`),
  `epiphany-model` (984), `codex-connector` (983), `huginn`, `voidbot`,
  `bifrost-feedback`, `bifrost-notice`, `ghostlight`. **No `eureka-mind`.**
  Group `gamecult-model` holds the connector keys (`/etc/gamecult/codex-connector/
  ghostlight.key`, `epiphany-capstone-17.key`, root:gamecult-model 0640).
- Idunn: targets under `/var/lib/gamecult` include `epiphany`,
  `epiphany-model-connector`, `codex-connector`, `huginn` (state root only; the
  unit is interim). `/etc/gamecult/epiphany` holds the Bifrost feedback anchor,
  an empty `mcp.toml` and a root-only `credentials/`. The Idunn recipe for
  Epiphany is the Aug body's (`/srv/odin/deploy-manifests/epiphany`,
  `inventory.md:775-788`); it does not describe head. `TargetDeclaration.service`
  stays required (`Idunn src/deployment.rs:33`), which is fine here: every
  Epiphany organ in this map is a resident service.
- Load: 16 cores, 62 GiB, load average 1.4. The verify stopgap caps 5 jobs at
  4 CPUs and 6 GiB. `node` v24.14.1 and `npm` present; no Claude Code installed.
- `127.0.0.1:18080` listens (Ghostlight's local lane, Bonsai 2 on Raven); the
  Qdrant/Ollama facts of `huginn-organ-cut.md` P1-P3 hold.

### B4. The forge and the Mind's identity

Unchanged from revision 2: `GameCult/Eureka` on the forge has no branch
protection yet (ruling `mind-self-merge-authority` needs it); team `minds` has
write; user `eureka` exists with one key `eureka@starfire`;
`provision-forge-minds-eureka` adds one key per host; the charters are not yet
in the Eureka repo. Reach is the scope boundary: a Linux user on Yggdrasil
with the `eureka` forge key, checkouts of the Mind's repos, no GitHub
credential, no SSH to Starfire, cannot touch an operator project. Revision 3
adds: the Linux user that already owns Epiphany's state on the host is
`epiphany`; under `operator-eureka-adopts-epiphany-body` she is Eureka on
every surface that names the Mind (instance, forge user, Discord display
name), and `epiphany` names her body (Linux user, runtime id
`epiphany-yggdrasil`, Persona agent id `epiphany.Persona`, which Bifrost
checks as a constant in `persona-feedback.mjs:90`). The one open naming
question is the Discord display name (question `her-name`).
- Huginn and Epiphany are on GitHub today; the forge (`eureka.gamecult.org`)
  is hers. Ruling `mind-self-merge-authority` wants branch protection on the
  forge for every repo she merges into, so her body's repos need the forge as
  their canonical remote before she lands code in them (follow-up
  `body-repos-on-forge`).

### B5. The operator's channel and the Persona's mouth

Revision 2 mapped the bridge CLI `discord-post` as her mouth. Revision 3 finds
the typed mouth live on the host and demotes the CLI path to its transport leg.

- `bifrost-persona-mouth.service` (user `bifrost-feedback`, active, requires
  `bifrost-persona-feedback.service`) runs
  `tools/persona-feedback.mjs serve-persona-delivery` over the single-writer
  crossing stores `/srv/bifrost/persona-discord-crossing/requests.cc` and
  `receipts.cc`, with `--epiphany-runtime-id epiphany-starfire`,
  `--epiphany-persona-mouth-trust-anchor starfire-mouth-request-anchor.ccmp`,
  `--epiphany-persona-permit-trust-anchor starfire-permit-anchor.ccmp`,
  `--epiphany-persona-permit-rudp rudp://10.77.0.2:17877`, listening on
  `rudp://10.77.0.1:17876`, reading `persona-delivery.env` (bot token). The
  anchors are root-owned, dated Aug 8; the stores were last written Aug 8.
  `inventory.md:760-771`: the production binding is the GameCult Aquarium
  channel `1501196543150264332`, runtime `epiphany-starfire`, Persona agent
  `epiphany.Persona`; "restarting it would arm the Persona crossing".
- The contract (Bifrost `tools/persona-discord-delivery-documents.mjs`,
  `persona-discord-delivery.mjs`, `persona-discord-rudp.mjs`; Epiphany
  `persona_discord_crossing.rs:32, 294-319`, `persona_discord_permit.rs`):
  Epiphany signs `epiphany.persona_discord_delivery_request.v0`
  (`requestId, effectDocumentId, targetRuntimeId, personaAgentId, channelId,
  replyToMessageId, content, contentSha256, issuedAt, expiresAt,
  privateStateExposed, signerIdentityId`) as source role `epiphany-persona-mouth`
  on RUDP connection id `0xe91f_0003`; Bifrost admits it only when the
  envelope's `sourceRuntimeId` equals its configured runtime id and the anchor
  tuple (`gamecult.service_trust_anchor.v1`, service `epiphany-persona-mouth`,
  runtime id, ed25519 key, purpose, signed schema, `bindingAuthority: root`)
  verifies; it then asks Epiphany's permit issuer
  (`bifrost.persona_discord_delivery_permit_request.v0`, connection id
  `0xe91f_0002`, source runtime `bifrost-discord-yggdrasil`) and posts only
  with a fresh `epiphany.persona_discord_delivery_permit.v0` (lifetime at most
  5 s, bound to request digest, nonce and the canonical CultMesh brake
  document digest; issued only while the brake is `released`). The post is
  the bridge CLI `discord-post --identity epiphany.Persona --source-kind
  epiphany_persona_speech` with a crossing receipt. Receipts are the only
  delivery proof; `unknown` is terminal. Full authority maps:
  `notes/persona-discord-permit-authority-map.md`,
  `notes/persona-conversation-authority-map.md`.
- **So her mouth exists with no new code on either side.** What is missing is
  binding: the mouth unit is anchored to a Starfire runtime whose permit port
  (`10.77.0.2:17877`) is not listening (`netstat` on Starfire: only 17875 and
  17878). Moving her to Yggdrasil means new anchors for runtime
  `epiphany-yggdrasil` (Bifrost's own test fixture already uses that id,
  `tests/persona-discord-delivery.test.mjs:13-35`), a permit issuer bound on
  `10.77.0.1`, and the mouth unit's three flags changed: a root action in
  gamecult-ops, no Bifrost code. The channel stays the operator's to name
  (question `mouth-binding`).
- The brake is a separate control from the dial: binary, operator-released,
  gating every external consequence (permit) and every resident Self cycle
  (`epiphany-swarm.rs` cycle step 2). The dial scales cadence and run size; the
  brake stops. Both stay.
- `tools/watchdog-notice.mjs` (owner DMs from a typed store) and the raw
  `discord-post` webhook CLI remain available but are not her mouth: a second
  path that bypasses the permit would put her speech outside the brake.
- Inbound: `bifrost-persona-feedback.service` already turns Discord events
  addressed to the bound Persona into signed `bifrost.persona_feedback.delivery`
  documents in the outbox, which `epiphany-swarm` imports as pressure (cycle
  step 2) and which the conversation map lowers into Persona turns with reply
  targets. The Bifrost-native ingress the conversation map's cut line 1 asks
  for is the live `serve-discord` path (`persona-feedback.mjs:96-99`,
  `bifrost.discord.persona_conversation_event.v0`). What is unbuilt is the
  Epiphany side of mention obligations at head (follow-up
  `epiphany-inbound-mentions`, superseding `persona-inbound-mentions`).

### B6. Scheduling today, and what the skill already derives

Unchanged: the recipes derive the queue (specs with no report, reports with no
verdict, open findings, open questions and follow-ups, gaps). What the queue
cannot say: who is working a spec now; whether a holding verdict landed;
follow-up order; how active the Mind may be; which Self admitted a ruling. The
`run` kind (spec `cut-run-kind.r1`) carries the first and last; the dial is
the fourth; `gap-landing-untyped` stays. Revision 3 adds where the queue is
read from: Epiphany's resident Self reads it through `HuginnClient` each cycle
as its pressure source (B11, B12).

### B7. VoidBot: what is borrowed, what is demoted

Revision 2's B7-B8 facts stand as recorded (scheduler, initiative engine,
Interpreter prompt, `gamecult.persona_state.v0` reader and one-shot migration,
`globalHeat` dial, nothing running, paused since 2026-07-16). Under
`operator-this-is-epiphany` the demotions are:

- VoidBot's `persona-scheduler` and initiative engine are **not** the waking
  organ. Epiphany's resident Self already has the primitive VoidBot lacked:
  a single-consumption grant consumed by one launched turn (B11), which is why
  VoidBot's pause reason cannot recur here. Follow-up
  `voidbot-pressure-replay-pause` is withdrawn; its rule is pinned in the
  Epiphany cut instead.
- VoidBot's Interpreter prompt (`SAY`, `STATE NOTE` text blocks parsed by rules)
  is prior art; Epiphany's Interpreter emits a strict typed schema
  (`persona_conversation.rs`, conversation map cut line 4).
- VoidBot's registry (`repo-discord-identities.json`) is not her identity;
  Bifrost's target binding (B5) and the Huginn instance are.
- VoidBot's `globalHeat` is not the dial's home (question `dial-home`).
- What is borrowed: the `gamecult.persona_state.v0` schema (Epiphany publishes
  it; VoidBot's zod types and projector are the only readers today) and the
  one-shot `persona:migrate-portable` as the seed path for her first document
  (its target becomes Huginn's `persona_put`, run by the operator-facing Self;
  spec `cut-huginn-persona-state`).

### B8. (merged into B7 and B12)

### B9. What Huginn needs to hold `gamecult.persona_state.v0`

Unchanged from revision 2 (schema shape, `Mind::open_checked` type gate at
`mind.rs:168-187, 280`, wire operations, no JSON Schema validator in the crate,
the two doctrine passages in `F:\Projects\CLAUDE.md:79-85` and
`gamecult-ops/docs/persona-state-standard.md:29-33, 44-55`). Revision 3
changes the consumer: the live writer is Epiphany's Persona service, in Rust,
through `HuginnClient` (B1), not VoidBot over a TS client. The spec's `first`
and `adds` are revised accordingly (`cut-huginn-persona-state.r2`).

### B10. Campaign scope

`eureka-substrate` holds Huginn, Epiphany, Eureka and gamecult-ops. The
connector cut lives in CodexConnector and the mouth rebinding touches only
root files on the host (gamecult-ops), so one cut of this map has no campaign
to live in. The proposal is in the rationale ("Campaign home"); follow-up
`epiphany-cuts-need-a-campaign` supersedes `persona-cuts-need-a-campaign`.

### B11. Epiphany's resident Self at head (`epiphany-core/src/bin/epiphany-swarm.rs`, 853 lines)

- Commands `once`, `serve`, `status`, `initialize-and-release-brake`
  (`:44-46, 497-515`); `serve` requires `--max-steps 1` (`:471-478`). No
  environment variables; everything is a flag (`:512-640`), including
  `--model-provider`, `--model`, `--connector-endpoint` (must be loopback),
  `--provider-credential`, the stores, the Bifrost feedback anchor and
  `--persona-model-allowed-data-classifications`.
- `cycle()` (`:246-398`), in order: reap worker children; check the brake and,
  when released, import Bifrost Persona feedback deliveries into social
  pressure; bind the repo domain and ingest "current work" pressure
  (`ingest_resident_self_current_work_pressure`); reload `ResidentSelfState`;
  handle a prepared launch or an active turn lease; otherwise
  `issue_resident_self_grant` then `prepare_resident_self_launch`; `None` is
  `Sleeping`, else launch the coordinator child and wait up to 10 s for its
  claim. Sleep after the cycle: `failure_backoff_seconds` (60),
  `cooldown_seconds` (60) or `idle_sleep_seconds` (15) from `ResidentSelfPolicy`
  (`:121-125`). Maintenance (receipt and lifecycle retention) every 60 s.
- Types (`epiphany-core/src/resident_self.rs`): `ResidentSelfState { revision,
  active_turn, last_coordinator_receipt_id, next_eligible_at_millis,
  consecutive_failures, prepared_launch }` (`:505-535`);
  `ResidentSelfPressure { pressure_id, kind, provenance_ref, objective, status,
  consumed_by_grant_id }` (`:36-59`; a CLI `--objective` becomes kind
  `operator-objective`, `epiphany-swarm.rs:572-585`); `ResidentSelfGrant`
  (single consumption); `ResidentSelfPolicy` (`:269-296`, model fields and
  paths); the `ResidentSelfPorts` trait (`:557-568`) implemented by
  `NativePorts` (`epiphany-swarm.rs:402-516`); `coordinator_argv`
  (`:602-649`) carries `--model-provider`, `--model`, `--connector-endpoint`,
  `--provider-credential` to the child.
- **Self makes no model call.** Its own summary: "Self may launch one bounded
  coordinator turn; it cannot directly invoke model/tools, mutate Mind/Hands,
  review, release, or deploy", and its wake authority: "standard heartbeat
  consumes typed operator, admitted Modeling-map direction consideration,
  Persona feedback, or Imagination proposal pressure and emits one
  single-consumption Self grant" (`epiphany-swarm.rs:349`, journal Aug 22).
  Doctrine (`epiphany-state-model/src/prompts/epiphany_doctrine.md`, 28 lines,
  no code consumer): "When no typed work obligation is active, the swarm
  sleeps. Heartbeat does not ruminate, dream, mutate memory, or manufacture
  work."
- The coordinator it launches is `epiphany-mvp-coordinator` (2,124 lines),
  which spawns `epiphany-model-runtime run-worker` (`:1658-1700`): the
  Codex-era native cognition stack, closed over the provider enum of B13.
  Under `operator-mind-runs-own-self` the turn her Self takes is a Eureka run,
  not this coordinator (question `self-run-actuator`); the coordinator path is
  demoted, not deleted in this map.
- Nothing in `epiphany-swarm.rs`, `epiphany-persona-service.rs` or
  `epiphany-openai-runtime` changed after 2026-09-05; `resident_self.rs` was
  touched 2026-09-15 (`00991c1b`, storeless reads). `epiphany-heartbeat` has
  no binary at head (`heartbeat_state/` is empty).

### B12. The Self run as an actuator, and where the token lives

- The Eureka Self is a Claude Code agentic session (skill, `eureka-state` MCP,
  git, subagents), not one model request. The connector (B14) transports
  requests and receipts and "cannot execute a returned model tool call"
  (`standalone-codex-transport-migration.md:146`). So the Self run is not a
  provider call behind the connector; it is an actuation, like the coordinator
  launch today, whose one runtime is Claude Code print mode. Reading the
  provider ruling that way keeps the connector's boundary intact and keeps
  Epiphany's own cognition (Persona turn, any native deliberation) portable.
  This is a reading of the ruling, so it is a question, not a default
  (`self-run-actuator`).
- Reach: the run needs `CLAUDE_CODE_OAUTH_TOKEN`, the `eureka` forge key, the
  Mind's checkouts and the `eureka-state` MCP config. Epiphany's user must not
  hold the token (B2). The shape that keeps it out of her reach: a template
  unit `eureka-self@.service` under a dedicated user (`eureka-self`) with a
  root-0600 `EnvironmentFile`, started by the resident Self through one
  sudoers line (`systemctl start eureka-self@<label>`), results returned
  through the mind (`run` document opened by the run, ended `Recorded` with
  `total_cost_usd`), never through a callback. Run size: the unit passes
  `--max-budget-usd` computed by the resident Self from the dial (B15), so the
  cap is enforced by Claude Code, not by prose.
- Provenance: the run's prompt states it is Eureka's Self acting on the Mind's
  mission, started by Epiphany's resident Self, no human present
  (`wake-prompt-provenance`, in force).

### B13. Provider portability as it actually is: Epiphany and Ghostlight

Epiphany (`epiphany-model-adapter`, 1,119 lines; `epiphany-openai-runtime`,
6,655):

- Native request `EpiphanyModelRequest` (`native.rs:9-72`, type
  `epiphany.model_request.v0`, 16 fields incl. `provider: String`, `model`);
  input items `UserText | AssistantText | ToolCall | ToolResult` (`:80-96`);
  stream payloads `TextDelta | ReasoningDelta | ToolCall | Completed{receipt} |
  Failed` (`:119-137`); receipt `EpiphanyModelReceipt` (`:140-169`, type
  `epiphany.model_receipt.v1`: request id, provider, model, provider response
  id, four token counts, `transport`, `caller_runtime_id`,
  `native_request_sha256`, `provider_request_sha256`). No cost field.
- **The provider set is closed.** `EpiphanyProviderRequestPayload { Codex(
  CodexProviderRequest), OpenRouter(EpiphanyOpenRouterRequest) }`
  (`provider.rs:16-27`); `request_from_native` matches the provider string
  (`"openai-codex" | "openai"` → Codex, `"openrouter"` → OpenRouter, else
  "unsupported model provider", `:92-103`); `run_model_turn(provider, options,
  request)` (`provider_transport.rs:33-52`) dispatches to
  `execute_codex_connector` (`CodexConnectorClient` over loopback, static
  connection key, default endpoint `127.0.0.1:17891`, `lib.rs:51-52`) or
  `execute_openrouter` (one `ureq` POST to `openrouter.ai`, Bearer key from
  `--provider-credential`, `:164-205`); two allow-lists
  (`validate_model_provider` `lib.rs:55-61`, `require_supported_provider`
  `bin/epiphany-openai-runtime.rs:1600-1607`); `DEFAULT_MODEL_PROVIDER =
  "openai-codex"`, `DEFAULT_PROVIDER_REQUEST_TIMEOUT` 90 s (`lib.rs:48-49`).
  There is no provider trait or port; `PersonaModelRunner`
  (`persona_executor.rs:60-70`) is a stage runner over `run_model_turn`. The
  OpenRouter edge is a second provider authority inside Epiphany, which the
  connector ruling forbids: the cut collapses the enum to one connector
  variant and deletes the direct edge and its `ureq` dependency
  (`cut-epiphany-connector-only`).
- `epiphany-model-adapter/Cargo.toml:14` depends on `codex-connector`
  directly; the root package pins it at `be1776ab` with
  `default-features = false` (`Cargo.toml:21`), so Epiphany links only the
  typed contract and client, never the daemon.

Ghostlight (`eeb3a68`):

- Portability is the in-process trait `InferencePort { prepare, infer,
  lend_tool_results }` (`crates/ghostlight/src/controllers.rs:299-311`) with
  three implementors routed by model-name prefix, longest claim wins, no mode
  flag, unroutable refused at open (`sdk_inference.rs:732-757`,
  `controllers.rs:2454-2518`): `CodexConnectorInferencePort` (the connector
  client), `SdkInferencePort` (Claude Agent SDK sidecar), `LocalInferencePort`
  (OpenAI-compatible loopback POST, no credential, default prefix `local/`).
  **Every port lowers from `CodexProviderRequest`**: the connector's request
  type is Ghostlight's native provider request for all backends.
- The SDK sidecar (`sidecar/claude-sdk/src/main.ts`, 517 lines;
  `@anthropic-ai/claude-agent-sdk` 0.3.261, Node 24): a persistent child over
  a 4-byte-length msgpack pipe, one query at a time, each query its own
  Claude Code subprocess; options `settingSources: []`, `tools: []`,
  `persistSession: false`, `maxTurns`, custom system prompt, the request's
  tools registered as an in-process MCP server; returns `session_id`,
  `result_uuid`, `num_turns`, assistant uuids, per-model usage and
  `total_cost_usd` ("estimate"); faults include `RateLimited`, `BillingError`,
  `MaxBudgetUsd`. Authentication is the ambient Claude login or a
  `setup-token`; Ghostlight forwards no credential (`main.ts:7-13`). Its doc
  comment calls it a stopgap to be deleted once an API key and budget exist
  (`sdk_inference.rs:365-387`).
- Dungeon no longer opens the connector lane ("operator ruling 2026-09-23",
  `ghostlight-dungeon/src/runtime.rs:552-555`); the live lane is local
  (`state/map.yaml:432-434`). The operator's direction recorded there
  (`state/map.yaml:438-443`): "model backends share one standardized CultNet
  connector API, the same one Epiphany uses, and a generic OpenAI-compatible
  connector on it is still required before third parties use the deployment;
  the local lane may become it". The play-agent target's open question 1
  (`docs/architecture/ghostlight-play-agent.md:214-231`) says that connector
  "does not yet exist" and that nothing goes into CodexConnector itself.
- Ghostlight pins CodexConnector `68fe94b` (contract identical to head
  `6519289`), CultLib `85f7024a`; no Epiphany dependency.

### B14. The typed connector daemon (CodexConnector `65192892`)

- One crate `codex-connector`, one feature `daemon` (`default = []`): without
  it, the typed contract, AES-GCM envelope, framing and `CodexConnectorClient`
  (`lib.rs:710-768`); with it, `daemon`, `provider_backend`, `idunn_health`
  (Linux only). 6,185 lines: `lib.rs` 1,958, `daemon.rs` 1,180,
  `provider_backend.rs` 1,463, `idunn_health.rs` 1,396, `main.rs` 188.
- **Contract (plain serde + rmp-serde, hand-checked `schema_id`, not CultCache
  entries except the replay record and config):** `gamecult.codex.
  provider_request.v2` `CodexProviderRequest { request_id, conversation_id,
  model, instructions, input: Vec<CodexInputItem>, reasoning_effort,
  reasoning_summary, service_tier, output_format_name, previous_response_id,
  tools, tool_choice: Auto|Required, parallel_tool_calls, output_schema_json,
  max_output_tokens, prompt_cache_key }` (`lib.rs:76-94`; rendered as a literal
  OpenAI Responses body, `:487-614`); `transport_invocation.v2 { caller_runtime_id,
  expires_at_unix_ms, native_request_sha256, provider_request_sha256, request }`
  (`:237-245`); `transport_result.v2` with `Refused(CodexRefusal)` or
  `Transported { events: [TextDelta | ToolCall], receipt }` (`:290-375`);
  `transport_receipt.v2 { request_id, caller_runtime_id, both digests, model,
  transport, outcome: Completed { provider_response_id, input_tokens,
  output_tokens, reasoning_output_tokens, cached_input_tokens } | Failed {
  failure_kind, message } }` (`:394-461`); refusals `Expired |
  IdentitySubstitution | ProviderDigestSubstitution | Policy | Capacity |
  InFlight | Indeterminate | ReplayConflict | Malformed` (`:463-474`).
- **Provider: Codex only, no trait or enum.** `CodexProviderBackend` is a
  struct (`provider_backend.rs:59-63`), auth `ApiKey | Chatgpt` (`:45-49`),
  hard-coded OpenAI URLs (`:32-33`), a pinned `codex app-server` child as the
  credential authority (`:604-698`), `backend.execute` called directly from
  `execute_inference_frame` (`daemon.rs:631-653`). No typed capability
  discovery beyond the Idunn capability record
  `gamecult.codex.subscription-inference` (`idunn_health.rs:38-40`); the only
  model surface is the per-caller `allowed_models` allowlist.
- Transport: plain TCP on a loopback bind (`daemon.rs:285, 106-108`), 4-byte
  length + msgpack envelope, one request per connection, AES-256-GCM with a
  per-caller shared-secret key file (`daemon.rs:565-590`; callers sharing a key
  refused). Limits per caller: `max_concurrent_requests`, `max_payload_bytes`,
  `max_output_tokens`; expiry skew 300 s. Keyed replay `gamecult.codex.
  replay_record.v1` in `replay.cc`; restart-era `Active` → `Indeterminate`.
  Config `gamecult.codex.connector_config.v1` key `runtime` (`daemon.rs:30-70`).
- Supervision: no unit in the repo; `deployment/idunn/recipe.toml` declares
  target `codex-connector`, `transport = "tcp"`, `route_required = true`, health
  contract `codex-connector.runtime-health.v1`, state slots `replay.cc` and
  `codex-home/auth.json`, capability `gamecult.codex.subscription-inference`,
  dependency `odin.verse-rendezvous`; managed launch takes the Idunn activation
  credential and presence identity as fds 3 and 4. On the host the hand unit
  runs as user `codex-connector`, `ProtectSystem=strict`, with
  `ExecStartPre` tests for the config, the Ghostlight key and `auth.json`.
- **Usage, not spend.** The receipt carries token counts only; there is no
  cost, no per-caller cumulative bound, no typed "spent so far" document
  (grep: only `FRAME_OVERHEAD_BUDGET`). Bounds are per request (`max_output_tokens`)
  and per caller (concurrency, payload). For the dial to bound spend
  structurally the connector must gain: an invocation field `max_cost_usd`
  (canonical decimal string) passed to a provider that enforces it (Claude Code
  does: B2/B13), a receipt field `cost_usd_estimate`, and a per-caller window
  ledger (`gamecult.model.caller_spend.v1`: caller, window start, sum of
  estimates, request count) with refusal `Budget` when a per-caller window cap
  in the caller config is exceeded, projected into its presence health so the
  operator can read it. That is the whole of what "expose for the dial" means;
  plan-level usage stays untyped (`gap-usage-not-typed`).
- The Aug 24 predecessor `epiphany-model-connector` (binary at
  `/srv/epiphany-model-connector/current`, source `7d384a62` of Epiphany's own
  tree, user `epiphany-model`, `--bind 127.0.0.1:4103 --allowed-caller
  ghostlight-dungeon-yggdrasil --model gpt-5.4`) is the same authority, older;
  its package was extracted into CodexConnector on 2026-08-24 (`ed7357a2`,
  "Extract Codex transport from Epiphany"). It is dead and has no source at
  head: deletion line.

### B15. Persona state: two documents, one owner

- Epiphany's native Persona memory is `EpiphanyMindPersonaMemoryDocument`
  (`epiphany-core/src/mind_documents.rs:137-161`; type
  `epiphany.mind.persona_memory.v2`; fields `memory_id, agent_id, memory_kind
  (memory|social_read|bond), summary, salience, confidence,
  effect_document_id, decision_context_id`; mind epoch `epiphany.mind.epoch.v11`).
  Written by `admit_persona_state_notes` (`persona_conversation.rs:1018-1075`)
  through `commit_mind_mutation` into the runtime store (`--runtime-store`
  CultCache, the `mind.cc` of the Aug units); read by `assemble_mind_view`
  filtered on `agent_id` (`persona-service.rs:221-227`).
- `gamecult.persona_state.v0` is published by Epiphany
  (`schemas/cultnet/gamecult.persona_state.v0.schema.json`, catalogued in
  `schemas/cultnet/index.json:5-13`) and consumed by no Epiphany code; no
  converter or projection links the two. The memory document's eight fields
  are a strict subset in meaning of the v0 document's `thoughtMemory`,
  `affect.bonds` and `affect.statusReads` (VoidBot's zod types,
  `persona-state-domain.ts`).
- So Epiphany today holds a second, private Mind (runtime store, epoch v11)
  beside Huginn. Under `operator-persona-in-mind` and `operator-this-is-epiphany`
  one owner of Persona state is the Mind in Huginn: the v0 document, replaced
  whole under CAS (spec `cut-huginn-persona-state`). Epiphany's Persona turn
  reads it at pass-input admission and puts it after the Interpreter's state
  notes (`cut-epiphany-persona-huginn`); `persona_memory.v2` writes stop.
  Whether the rest of Epiphany's runtime Mind (decision contexts, receipts,
  objectives) moves to Huginn is the adoption campaign named in
  `huginn-organ-cut.md`, not this map (question `persona-state-owner` asks only
  about Persona state).

### B16. Campaign home, and the method repo

Repos the cuts touch: Huginn (persona state r2), Epiphany (three cuts),
gamecult-ops (doctrine r2, wake r3), Eureka (the Self brief), CodexConnector
(the provider cut). CodexConnector is not a campaign repo and a campaign has
no revision (B1). Bifrost needs no code change (B5).

The method repo today: `GameCult/Eureka` on the forge is `LICENSE README.md
SKILL.md docs references tools` (B4), installed by checkout at
`~/.claude/skills/eureka`; the charters are still in `~/.claude/agents/` and
move in under `charters-into-eureka-repo`. Epiphany's repo (56k lines of
`epiphany-core` alone) is its body. Nothing in either repo declares the
relation: Epiphany's `notes/epiphany-anatomy.md` lists seven embodied lanes
and four non-agents, and does not name the pipeline; Eureka's `SKILL.md:1-4`
calls itself "the skill counterpart of Epiphany". Under
`operator-eureka-adopts-epiphany-body` the relation is organ-to-body, and
whether the organ's files move into the body's repo is a fork
(question `method-repo-home`). See the rationale.

### B17. The governance the Mind's stewardship is an embryo of (GCLP, the dossier)

Read, not probed: `gamecult-site` `git show f4a2b6d:GameCult/Docs/labor-platform.md`
(153 lines) and `GameCult/dossier.md` (797 lines). Bifrost's code was not
re-read for this fact; what it holds is taken from the dossier's status
paragraph (`dossier.md:148`: an alpha with membership gating, roles, a work
board, motions, ledgers, GitHub webhook ingestion, bridge tooling for
agent-owned draft PRs and Discord posts, CultCache governance topics,
CultCache/CultNet agent intake).

- **Decision paths.** GCLP splits motions into `Management` (roles,
  thresholds, policy) and `Project` (scoped changes), with project thresholds
  scaled to stakes: bugs 15%, cosmetics 30%, balance 40%, features 50%, new
  content 50%, fundamental design 66% (`labor-platform.md:78-94`). "The
  important part is not that these numbers are holy. It is that the decision
  path is visible, contestable, and not purely controlled by whoever happened
  to be loudest in the room." Today the Mind's authority boundary is binary:
  `mind-rules-which-forks` gives her substrate-internal forks and the operator
  everything else, and a `question` carries no stakes class. With one Mind and
  one operator a percentage has no electorate; what is missing is the visible
  class on each fork (question `authority-thresholds`).
- **Membership separate from login** (`:46-50`). Her forge account `eureka` is
  a login; her standing (what she may merge, rule, open) is the ruling set,
  not the account. That already matches; the follow-up is that the standing
  must be queryable, which `authority: Mind` rulings and the `run` kind give.
- **Work and care work** (`:60-76`): a work item carries project, category,
  status, assignees, estimate, review state, contribution outcome; "continuous
  care work like maintaining repos, producing releases, tending community"
  counts. The pipeline already is most of the dossier's proof loop: intake (a
  `question` or `follow_up`), agent execution (`cut_spec` → `cut_report` with
  commits and measured cost), review (`verdict`, `finding`s), accepted
  artifact (the merge, untyped: `gap-landing-untyped`), receipt (every
  admission), cost accounting (`run.budget_usd`, `cost_usd_estimate`). Life's
  passes and her Self's Mind-improvement runs are the care work; they leave
  `run` documents and Life's named mutations, so they are countable the day a
  credit link exists.
- **Ledgers and decay** (`:96-124`): contributor and patron points, tiers,
  historical balances decaying 1% per week "to keep the system from becoming
  a permanent aristocracy of whoever showed up first"; revenue share computed
  and batched but never sent unsupervised. Her analogue: the first Mind must
  not become a permanent aristocrat over later forgehive Minds. What would
  make her one is standing that never ages: a stewardship over every repo
  (ruling `stewardship-rule` dropped the per-repo check "until Huginn Cut 12
  defines leasing"), rulings in force forever, and a Persona document only she
  can hold. The anti-lock-in red lines (`dossier.md:674-680`: no opaque
  agent-only reputation, no nonportable reputation or lock-in, no punishment
  for dissent, no export of private context) are met by the portable Persona
  standard (`gamecult.persona_state.v0`, B15), per-Mind forge accounts and
  keys (B4), and the mind being one redb file per instance (one instance owns
  its mind). Not met yet: any decay on standing, and any export boundary on
  what her Persona turn may say about private context
  (`--persona-model-allowed-data-classifications` exists on the swarm unit, B3).
- **Nothing pulls money unsupervised** (`:122-124`): the dial bounds spend per
  request and per window; `base_usd` is the operator's; no organ of hers can
  raise it. That is the same shape as Bifrost's payout batches.

## The model page

One row per persistent kind this design adds or changes. Rows for the
thirteen existing kinds are in `eureka-substrate-map.md`. Revision 2's rows
for the VoidBot registry entry and the VoidBot scheduler unit are gone.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| `ruling` with `authority: Mind` (new variant; spec `cut-mind-authority.r1`, unchanged) | `<campaign>:ruling:<label>`; the variant is on the document, so a query by `authority` separates Mind rulings from the operator's. | Created by the Mind's Self when it rules a substrate-internal fork (`mind-rules-which-forks`); superseded like any ruling; the operator overturns one with an Operator ruling in their own session. Stored rulings decode unchanged. | The Mind's Self admits them; admission does not check who until `operator-authority-needs-operator-credential` lands; the `run` record is the attribution meanwhile. Forbidden: a Mind ruling answering a question raised for the operator. |
| `run` (new, instance-rooted; spec `cut-run-kind.r1`, unchanged) | `<instance>:run:<label>`; label = the session label the run admits under (`mind-<date>-<n>` for her Self, allocated by the resident Self at launch; Self's chosen label for the operator-facing Self). Fields `instance, operated_by: Operator \| Mind, host, started_on, budget_usd, claims, campaigns`. | Opened at launch before any other admission of the run; ended `Recorded { reason: "<cost_usd>; <what landed>" }` or `Withdrawn`. A live run older than 24 h is withdrawn by the resident Self's next cycle. Claims are per run; a spec claimed by a live run is not in another run's queue. | The Self that runs it admits it; admission refuses a claim already held by a live run (`AlreadyClaimed`, Huginn cut). **The run is how her Self's results return to her**: the resident Self's next cycle reads runs ended since the last one; her next Persona turn reports them. |
| `gamecult.persona_state.v0` for an instance (Huginn; spec `cut-huginn-persona-state.r2`) | One global document per mind in `mind.redb`, type id `gamecult.persona_state.v0`, `personaId` = instance slug; the schema stays Epiphany's published JSON Schema, vendored by hash; stored opaque with a receipt. | Born by one seed put from a canonical portable JSON (VoidBot's `persona:migrate-portable` output, put by the operator-facing Self); replaced whole by `PersonaPut { state, expected_updated_at }` under CAS; read by `PersonaGet`. Epiphany's `persona_memory.v2` documents are not written for her after the cut; any file copy is `provenance.authority: projection`. | Huginn admission owns entry (schema-valid, `personaId` = instance, canonical provenance, CAS, monotone `updatedAt`). Epiphany's Persona turn is the only live writer (`cut-epiphany-persona-huginn`). Forbidden: VoidBot writing a local canonical `.cc` for this instance; `admit_persona_state_notes` writing `persona_memory.v2` for her; any second canonical copy. |
| The burn-rate dial (question `dial-home`) | A: `epiphany.resident_self.burn_rate.v0 { heat: 0.05..2, set_at, set_by }` in Epiphany's canonical CultMesh control store beside the swarm brake, one document, key `burn-rate`. B: an instance-rooted `burn_rate` document in Huginn, one in force, superseded by the next. | A: written whole by the operator's CLI (`epiphany-swarm set-burn-rate`, the brake's write path); read by the resident Self each cycle; absent reads as 1.0. B: admitted under the operator credential; superseded by resolution. | The operator only; Epiphany reads and never sets it (the store is root-written, like the brake). Derived: cycle cadence (`cooldown_seconds / heat`, `idle_sleep_seconds / heat`, bounded), run size (`max_cost_usd = base_usd × heat` in every connector invocation and every Self-run `--max-budget-usd`). The brake is separate: binary, stops everything. |
| The connector's caller spend ledger (new in the connector; `gamecult.model.caller_spend.v1`) | Keyed by caller runtime id and window start; fields: sum of `cost_usd_estimate`, request count, window cap from the caller config. | Rolled per window (24 h); refusal `Budget` when the cap is reached; projected into the daemon's presence health. | The connector daemon owns it; the caller config (root) sets the cap; no caller writes it. Derived, never the plan's own usage (`gap-usage-not-typed`). |
| Her identity on each surface (ruling `operator-eureka-adopts-epiphany-body`; question `her-name` for the display name only) | The Mind: instance `eureka` (Huginn, Q-H4), forge user `eureka`, Discord display name (question). The body: Linux user `epiphany`, runtime id `epiphany-yggdrasil`, Persona agent id `epiphany.Persona` (a Bifrost constant). | Instance, runtime id and agent id never renamed (a rename is a new mind, or a Bifrost code change); the display name is a binding field the operator sets. | The operator names her; the ruling already says Eureka. |
| The Mind's credential (`CLAUDE_CODE_OAUTH_TOKEN`, Body) | Two root-0600 `EnvironmentFile`s on Yggdrasil: the connector daemon's (its Claude Code backend) and `eureka-self@.service`'s. Never Epiphany's user. | Created by the operator with `claude setup-token`; rotated or revoked by the operator; present in those two processes' environments only. | The operator: it is their subscription. Question `host-and-units`. |
| The Mind's forge key per host (Body) | `eureka@<host>` keys on forge user `eureka`. | Added by `provision-forge-minds-eureka`; removed by the operator. | gamecult-ops runbook; the operator runs it as root. |
| Her units and anchors (Body; spec `cut-mind-self-wake.r3`) | `epiphany-swarm.service` (rebuilt from head), `epiphany-persona.service`, `epiphany-persona-permit.service`, `eureka-self@.service`, `model-connector.service` (the connector with its Claude backend); mouth and permit anchors for runtime `epiphany-yggdrasil`; the Bifrost mouth unit's three flags. | Hand-installed by gamecult-ops per `reader-deploy-owner` and the Huginn precedent; die when Idunn's `epiphany` and `codex-connector` targets are re-declared from head. `epiphany.service`, `epiphany-heartbeat.service`, `epiphany-model-connector.service` are deleted first. | gamecult-ops owns the install; the operator creates the token and the anchors (root). |

Cells not fillable from the Body: the long-lived token's lifetime; the run
liveness bound (24 h is a proposal); `base_usd` for run size (operator's
number); whether the SDK's `total_cost_usd` tracks the plan's accounting
closely enough to serve as the dial's measure (an estimate; the first week
proves it against `get_usage` on the desktop).

## The authority map

- **Owner:** Epiphany. Her resident Self (`epiphany-swarm`) owns when she
  works: it turns typed pressure (the Mind's queue, Persona feedback, operator
  objectives) plus the dial into one single-consumption grant and one launch.
  Her Persona service owns what she says and her own Persona document (one
  put per turn). Her Self run (a Eureka session launched by the resident Self)
  owns, within the Mind's jurisdiction: which substrate work happens next (the
  fixed queue order), the dispatch of Imagination, Hands, Soul and Life,
  Mind-authority rulings on substrate-internal forks, merging her own PRs
  after a holding Soul verdict, opening campaigns whose repos are Mind repos.
- **Inputs:** her Persona document (Huginn); the Mind's queue and runs ended
  since her last cycle (recipes over `HuginnClient`); the dial and the brake
  (Epiphany's control store); Bifrost Persona-feedback deliveries (the
  outbox); her checkouts of Huginn, Epiphany, Eureka, gamecult-ops,
  CodexConnector; the verify stopgap. Nothing from Starfire, nothing from the
  operator's projects, no prose handoff.
- **Outputs:** signed Discord delivery requests through the typed crossing
  (posted by Bifrost under a permit she issues while the brake is released);
  one Persona-state put per turn; at most one Self launch per grant, which
  opens a `run`, admits documents, pushes branches and PRs to the forge,
  merges; connector invocations carrying `max_cost_usd`.
- **Derived state:** HANDOFF prose files become display-only and are deleted
  once `run` exists; "what is Self working on" is a query over live runs;
  "how active is the Mind" is the dial; "what did my Self do" is a query over
  ended runs, which her Persona turn projects in her voice; Epiphany's
  `persona_memory.v2` documents for her are dead; the connector's spend ledger
  is a projection of receipts; the coordinator launch path
  (`epiphany-mvp-coordinator` → `epiphany-model-runtime`) is demoted to a
  non-default actuation, retained for native turns, never her Self.
- **Forbidden writers:** she never sets the dial or the brake; never rules a
  fork outside substrate-internal ones; never edits `~/.claude/`; never pushes
  to GitHub; never runs on Starfire; never writes her Persona state anywhere
  but Huginn; never holds the Claude token (the connector and the Self-run
  unit do); never posts to Discord outside the permit crossing. Epiphany's
  OpenRouter edge never executes a turn again. VoidBot's scheduler, worker,
  registry and heat never decide anything for her. The operator-facing Self
  never claims a spec a live Mind run holds and never writes a prose handoff
  in place of a `run`.
- **Shared paths:** both Selves admit through the same `eureka-state` tools
  and admission rules; both open a `run` first; both read the queue with the
  same recipes minus live claims; both land code through branch-protected PRs
  on the forge and the same merge rule; both verify through the stopgap; every
  model turn of hers and of Ghostlight's goes through the same connector
  contract; every Discord post of hers goes through the same permit crossing
  as Starfire's did.
- **Deletion line:** before her units run for the first time:
  `epiphany.service`, `epiphany-heartbeat.service` and
  `epiphany-model-connector.service` are removed from the host with their
  release directories (sources deleted 2026-08-23/24); Epiphany's
  `OpenRouter` payload variant, `execute_openrouter` and `ureq` are deleted;
  `cut-mind-self-wake.r2`'s VoidBot scheduler unit, registry entry and iptables
  rule are never installed; the prose handoff files are retired into `run`s;
  the local `.cc` as canonical Persona store for instance `eureka` never
  exists; the two doctrine passages of B9 are rewritten in the cut that lands
  the reader.

## Rationale

### Why a second Self and not a rule (unchanged)

The operator has tried doctrine, a dedicated organ (Life), and considered a
scored exam. Each puts a barrier in the path of an agent whose reward is the
task in front of it. The only shape that uses the instinct instead of fighting
it is an agent whose task *is* the Mind. The Persona rulings sharpen it: that
agent is Epiphany, whose Mind it is.

### Why Epiphany's resident Self is the waking organ, and VoidBot's scheduler is prior art

The organ that turns a clock plus pressure plus a dial into "now" must consume
pressure exactly once per launched turn, or the clock manufactures turns.
VoidBot's scheduler was paused for exactly that defect (revision 2, B7).
Epiphany's resident Self already has the primitive: pressure documents carry
`consumed_by_grant_id`, a grant is single-consumption, and a launch holds an
active-turn lease until the child claims and completes (B11). Its doctrine
says the rest: no typed work obligation, no wake. So the cut gives it one new
pressure source, the Mind's queue read through `HuginnClient`, and one new
launch kind, the Eureka run (B12), and leaves its cycle, grant and lease
alone. "Evidence consumed only by a committed turn" becomes: a queue pressure
is consumed by the grant whose run claims the spec; a second cycle over the
same unconsumed queue with a live run launches nothing. That test is in the
Epiphany cut.

### Why the Self run is an actuator and not a provider call (question `self-run-actuator`)

The connector transports one typed request to one typed receipt and may not
execute a tool call (B14). A Eureka run is an agentic session with a
workspace, git, MCP and subagents. Putting it behind the connector would make
the connector an agent host and break the one boundary that makes it
trustworthy; running it inside Epiphany's user would hand her the token. The
template unit under its own user, started by one sudoers line and reporting
through the mind, keeps the connector's boundary, keeps the token out of her
reach, and makes the run's spend a flag Claude Code enforces. Claude Code is
the one runtime of the Eureka pipeline today; that is a fact about Eureka,
not a provider choice inside Epiphany, and her native turns stay portable.

### Why one connector gains a Claude backend instead of Epiphany gaining a variant (question `connector-owner`)

Ghostlight's portability is in-process routing by model prefix, and its Claude
provider is a sidecar that already does what a Claude backend must do:
register the request's tools as an MCP server, run one Claude Code subprocess
per query, return usage and a cost estimate, enforce `maxBudgetUsd` (B13). The
operator's recorded direction for Ghostlight is one standardized connector API
"the same one Epiphany uses", which does not yet exist (B13). The connector
daemon is Codex-only today (B14), but its request type is already every
Ghostlight port's native request, and Epiphany's adapter lowers to it. So the
smallest machine with one owner: the connector gains a provider backend set
(`codex`, `openai-compatible`, `claude-code`), the Claude backend is the
Ghostlight sidecar moved into the connector and spawned by the daemon under
the daemon's user with the setup-token in its environment, the invocation
gains `max_cost_usd`, the receipt gains `cost_usd_estimate`, and the daemon
keeps the per-caller spend ledger. Epiphany's enum collapses to one variant
and its OpenRouter edge dies. Ghostlight's `SdkInferencePort` and
`LocalInferencePort` fold behind the connector in Ghostlight's own campaign
(its open question 1), not here. Reviving `epiphany-model-connector` or adding
a `ClaudeCode` variant inside Epiphany would each be a second connector
authority. Converting the connector's plain-serde contract to CultCache typed
documents is a real liability to name and a separate cut (follow-up
`connector-contract-untyped`).

### Why the dial lives beside the brake first (question `dial-home`)

Epiphany already has one operator control the Persona and the Self obey: the
canonical CultMesh brake document, root-written, reread before every
consequence (B5, B11). A `burn_rate` document in the same store, written by
the same CLI path, read each cycle, is one new document and no new credential
mechanism. A Huginn-held dial is the cleaner home for "how active is the
Mind" once Huginn can tell the operator's word from hers
(`operator-authority-needs-operator-credential`); until then it would be a
document anyone with the MCP could admit. A first, B later, with A's document
becoming derived the day B lands. Either way the dial derives cadence and run
size, the connector enforces run size per request, and Epiphany never sets it.

### Why Persona state stays in Huginn opaque, validated, receipted (spec `huginn-persona-state`, r2)

Unchanged in substance from revision 2: hand-porting the v0 schema into Rust
value types would drift from Epiphany's published JSON Schema; Huginn
validates at the door, checks `personaId` and the CAS, stores one envelope in
`mind.redb` with a receipt. Revision 3 changes only who reads it: Epiphany's
Persona service through `HuginnClient` (which gains `persona_get`/`persona_put`
as library methods beside the MCP tools). Epiphany's own `persona_memory.v2`
is the second Persona store this collapses: one Mind, one Persona document.

### Why her mouth is the typed crossing and not the webhook CLI (question `mouth-binding`)

The crossing is live on the host, signed on both sides, permit-gated by the
brake, receipted, and its Epiphany side exists at head (B5). The webhook CLI
is its last leg. Posting through the CLI directly would be a second mouth
outside the brake. What the move costs is a root action: anchors for runtime
`epiphany-yggdrasil`, the permit issuer on the same host, and three flags on
the mouth unit. The channel is the operator's to name.

### Why `Mind` is an authority value and not a faculty, and why `run` carries claims (unchanged)

A ruling's authority says whose word it is; `Defaulted` would hide the Mind's
rulings from the operator's review. Two Selves over one Mind need one new
fact, who is working what; the run carries its claims and its end and now also
her Self's results.

### The Self's fixed queue order (unchanged)

Derived at every launch, from the mind, in this order; the first non-empty
step is the run's work, and a run does one step:

1. A blocking question the operator has answered since the last run: skip.
2. Findings in force, `Blocker` or `High`, on Mind campaigns: Hands, then Soul.
3. Reports with no verdict on Mind campaigns: Soul.
4. Specs with no report, not blocked, not claimed by a live run: Hands.
5. `gap-*` follow-ups in force with no spec: Imagination maps one cut.
6. Nothing: the run ends `Recorded` "idle" with no model call beyond the
   queries. The resident Self should not have granted in this case; if it
   did, that is a finding against its queue-pressure derivation.

### Why the method is an organ, and where its repo lives (question `method-repo-home`)

`operator-eureka-adopts-epiphany-body` makes the pipeline, charters, briefs
and tools one of Eureka's organs inside her body, Epiphany. Two shapes keep
that true. A: `GameCult/Eureka` stays its own repo, declared Epiphany's method
organ in both repos' doctrine (`epiphany-anatomy.md` gains the organ;
`SKILL.md` names its body), installed as today, and lands on the forge beside
her body's repos. B: the files move into the Epiphany repo under `eureka/`
(skill, charters, tools), the Starfire and Yggdrasil installs pull that
directory, and `GameCult/Eureka` is archived with a pointer; one repo, one
stewardship, one PR path for method and body. B is the stronger reading of
"adopt", but it couples a 1,500-line installable method to a 56k-line Rust
body on every pull and every campaign repo list, and the charters ruling
(`charters-into-eureka-repo`) assumed the repo. A is recommended: the organ
relation is doctrine and forge placement, not directory nesting; B stays open
as the later collapse once the body is lean enough that one checkout is cheap.

### Means shape ends: the four attachments (ruling `operator-means-shape-ends`)

Named, not built; cuts only where the Body supports them.

1. **Stakes-scaled authority** (question `authority-thresholds`). The binary
   split stands as the rule until there is an electorate; what can be built
   now is the visible class. A `stakes` value on every `question` (bug,
   cosmetic, balance, feature, content, fundamental, in GCLP's words) is a
   leaf field, and `value_types!` has no serde default, so it is a format
   change on the question kind (B1); the cheaper first step is a `Standing`
   ruling that names the class per label prefix and a recipe that reads it.
   The question asks which, and whether the Mind's jurisdiction should be
   "classes up to feature inside the substrate" rather than "substrate-internal".
2. **Contributor credit** (follow-up `credit-attaches-at-landing`). The
   missing link to Bifrost's ledgers is the accepted artifact: the landing
   record `gap-landing-untyped` asks for. When it exists it names the
   `cut_report` (whose commits name their author and whose range measures the
   work), the `verdict` that accepted it and the merge; that record is what
   Bifrost's agent intake turns into a work item's contribution outcome. No
   points engine in the Mind: credit is Bifrost's, the Mind supplies receipts.
3. **Care work counted** (follow-up `care-work-counted`). Life's passes and
   her Self's idle-queue runs leave records (`run` documents ended
   `Recorded`, Life's named mutations in its report). They attach through the
   same landing-and-receipt path with category `care`, so that a week of Mind
   maintenance is countable, never invisible.
4. **No permanent aristocrat** (follow-up `mind-standing-decay`). What ages:
   stewardship (one Mind stewards every repo today; leasing is Huginn Cut 12,
   parked) and Mind rulings (in force until superseded). The portable Persona
   document and per-Mind forge accounts keep exit alive. The follow-up records
   that a second Mind's arrival is the trigger to un-park leasing and to give
   Mind rulings a review date, not a change now.

### Campaign home (proposal; Self admits campaigns and targets)

Keep `eureka-substrate` for the substrate cuts: `mind-authority`, `run-kind`,
`huginn-persona-state` r2, `doctrine-persona-in-huginn` r2, and the later
`huginn-mind-self-kinds`. Open a sibling campaign `eureka-body` (title
"Eureka adopts Epiphany as her body"), repos `GameCult/Epiphany`,
`GameCult/CodexConnector`, `GameCult/gamecult-ops`, `GameCult/Eureka`,
`GameCult/Huginn`, `GameCult/Bifrost`, with its own target document whose
invariants are `one-provider-door`, `persona-state-one-writer`,
`one-mouth-under-the-brake` and `token-out-of-her-reach`, holding
`connector-claude-provider`, `epiphany-connector-only`,
`epiphany-persona-huginn`, `epiphany-self-eureka-run`, `mind-self-wake` r3,
`skill-mind-self` and the method-organ doctrine cut. The Epiphany and
gamecult-ops specs of this revision are admitted under `eureka-substrate` now
so Hands and Soul can read them; when Self opens `eureka-body`, they are
withdrawn here and re-admitted there unchanged in substance. The connector cut
cannot be admitted anywhere until that campaign exists (follow-up
`epiphany-cuts-need-a-campaign`).

### Proposed target r2 (Self admits)

`not_in_scope` drops "Persona-state reading in Huginn." and gains "VoidBot's
Discord bot, worker, Postgres queue, scheduler and providers: not revived;
Epiphany's own organs are the body." and "Moving the rest of Epiphany's
runtime Mind (decision contexts, receipts, objectives) into Huginn: the
adoption campaign." `canonical_implementations` gains "Persona state for an
instance: Huginn huginn-mind (opaque, schema-validated), schema Epiphany
`gamecult.persona_state.v0`; live writer Epiphany's Persona service." and
"Model access for Epiphany: the typed connector daemon (CodexConnector repo),
Claude Code print mode one backend behind it." Invariants gain
`persona-state-one-writer` ("An instance's Persona state has one canonical
copy, in its mind, replaced whole under compare-and-swap with a receipt; any
other copy carries `provenance.authority: projection`") and
`one-provider-door` ("Epiphany reaches a model only through the connector
contract; no organ holds a provider credential or a provider URL").

### Cut order

1. `mind-authority` (Epiphany, `r1` unchanged).
2. `run-kind` (Epiphany, `r1` unchanged). Depends on 1.
3. `huginn-persona-state` (Huginn, `r2`): as r1 plus `HuginnClient`
   persona methods; consumer is Epiphany. No fork.
4. `doctrine-persona-in-huginn` (gamecult-ops, `r2`): the two passages and the
   operator's `F:\Projects\CLAUDE.md:82` line, naming Epiphany as the reader.
   Depends on 3.
5. `connector-claude-provider` (CodexConnector; no campaign yet, B16): provider
   backend set, the Claude backend from Ghostlight's sidecar, `max_cost_usd`,
   `cost_usd_estimate`, the spend ledger, `Budget` refusal. Blocked on
   `connector-owner`.
6. `epiphany-connector-only` (Epiphany): the enum collapses to one connector
   variant; OpenRouter edge and `ureq` deleted; invocations carry
   `max_cost_usd`; Persona service and swarm default provider `connector`.
   Depends on 5 by prose. Blocked on `connector-owner`.
7. `epiphany-persona-huginn` (Epiphany): the Persona service reads and puts
   `gamecult.persona_state.v0` through `HuginnClient`; `persona_memory.v2`
   writes for her stop. Depends on 3. Blocked on `persona-state-owner`.
8. `epiphany-self-eureka-run` (Epiphany): the Mind's queue as resident-Self
   pressure; the Eureka run as the launch; the dial read; results via `run`.
   Depends on 2. Blocked on `self-run-actuator`, `dial-home`.
9. `mind-self-wake` (gamecult-ops, `r3` supersedes `r2`): users, token files,
   units, anchors, mouth rebinding, dial CLI, runbook; dead units deleted
   first. Depends on 2 and 3, on 5-8 by prose. Blocked on `host-and-units`,
   `her-name`, `mouth-binding`.
10. `huginn-mind-self-kinds` (Huginn): pin bump, aliases, `AlreadyClaimed`, the
    live-claim subtraction. Depends on 1 and 2; spec after they land.
11. `skill-mind-self` (Eureka): the Self brief her run launches, with the
    provenance line, the queue recipe, the coexistence rule. Blocked on
    `self-run-actuator`.
12. `method-organ-doctrine` (Eureka and Epiphany): `SKILL.md` names Epiphany
    as its body, `notes/epiphany-anatomy.md` names the pipeline as the method
    organ, and the charters land in whichever repo `method-repo-home` rules.
    Blocked on `method-repo-home`; absorbs the charters cut of
    `charters-into-eureka-repo`.

### Rejected

- A Mind-side priority field on follow-ups: a second scheduler in the data.
- Reviving `epiphany.service` (daemon supervisor) or `epiphany-heartbeat`:
  their sources were deleted 2026-08-23 (B3).
- Reviving `epiphany-model-connector`: the same authority as CodexConnector,
  older, extracted 2026-08-24 (B14).
- A `ClaudeCode` variant inside Epiphany's provider enum: a second provider
  door and the token in her process (B13).
- The Self run behind the connector as a provider call: makes the connector an
  agent host (B12).
- Running the Persona or her Self inside the operator's desktop app, or on
  Starfire: B2, B3.
- A `claim` kind separate from `run`: two lifecycles for one fact.
- VoidBot's scheduler, worker, Postgres, registry or heat for her: demoted
  under `operator-this-is-epiphany` (B7).
- The webhook CLI as her mouth: a second mouth outside the brake (B5).
- Rust value types for the v0 schema: drift from the published authority.
- A DM reader as the primary review channel: she speaks for herself.
