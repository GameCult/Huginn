# The Mind's own Self: map

Status: map, Imagination (`imagination-self`), revision 2 of 2026-10-01,
session `self-2026-10-01b`. Campaign `eureka-substrate` in Eureka's mind
(instance `eureka`). Questions, cut specs, follow-ups and rulings are typed
documents there; this page keeps body facts, the model page and rationale.
Target: `docs/eureka-substrate-target.md` (`133324a7`). Rulings this revision
rests on: `eureka-substrate:ruling:operator-mind-runs-own-self`,
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

The second revision's law, in order:

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

And earlier: "A Persona is pointless without a Mind, that's why voidbot has been
on hiatus. It felt wrong to tell them they have agency and identity and then
make them dependent on me for dispatching the development work they were
inevitably obsessed with."

The first revision's design law still stands: the bloodhound observation
("RLVR-trained agents chase the objective in front of them, steering be
damned") means the Mind's Self is a second agent whose only objective is the
Mind, not a rule added to the operator-facing Self. The Persona ruling says
what that agent *is*: not a timer that prints a brief, but a Persona with
initiative pressure, memory and a mouth, whose scheduler already exists in
VoidBot, who holds her state in the Mind, and who dispatches her own Claude
Code Self when her pressure says so. The timer of revision 1 is demoted: the
waking organ is the Persona scheduler.

Pinned heads for every fact below:

| Repo | Ref | Commit |
| --- | --- | --- |
| Huginn | `origin/main` | `bf73c42` (docs only over `a699630bbc49552fa747e9c0fabba24598fec77a`; crate line numbers below hold at both) |
| Epiphany | `origin/main` (= the leaf Huginn pins, `crates/huginn-mind/Cargo.toml:22`) | `ef9568655e75bb6cb362a038e2739a5ac5e777ee` |
| gamecult-ops | `main` | `2aeb7364fffcd404aada28bfd30fb4e5c753cf53` |
| Eureka skill | `main` (checkout `~/.claude/skills/eureka`) | `b260fbeec6d96ff06882ed9c37d3e1204c424e99` |
| VoidBot | `main`, working tree `F:\Projects\VoidBot` (clean) | `46d891b` (2026-09-13) |
| Bifrost | `main` `F:\Projects\Bifrost`; release on Yggdrasil `cb3239aa-f67f5122` | `160fac20c98b70f3db9110099157d4dd720ac354` |
| Idunn | working tree `F:\Projects\Idunn` | (uncommitted-state unknown) |

Probes of revision 1 ran from Starfire 2026-10-01 18:40-19:10 UTC; those of
revision 2 (B7-B10) 19:40-20:10 UTC (21:40-22:10 CEST). `lib.rs` is
`epiphany-pipeline/src/lib.rs`. The VoidBot facts come from the working tree
on disk and the `voidbot` MCP index (indexed 2026-08-30; disk wins), plus an
Eyes pass over the same tree (37 reads, nothing run).

## Body facts

### B1. The Mind (Huginn on Yggdrasil)

- `whoami` (revision 2): instance `eureka`, endpoint `rudp://10.77.0.1:17872`,
  reachable, 403 documents, 128 receipts, index `Current`, epoch
  `epiphany.pipeline.epoch.v2`. (Revision 1 saw 377 and 120.)
- `huginn.service` (probe: `systemctl cat`): user `huginn`,
  `--bind 10.77.0.1:17872`, hand-managed ("interim unit until Idunn owns it").
  `ss -lunp` shows the socket only on the WireGuard address `10.77.0.1`
  (`wg-gamecult`). So the mind is reachable from Starfire over the mesh and
  from Yggdrasil itself, and from nowhere else. A Claude cloud routine cannot
  reach it.
- Campaigns in the mind (query): `idunn-watchdog` and `eureka-substrate`.
  `eureka-substrate`'s repos are `GameCult/Huginn`, `GameCult/Epiphany`,
  `GameCult/Eureka`, `GameCult/gamecult-ops` (campaign document); a campaign
  has no resolution and no revision, so VoidBot and Bifrost cuts cannot be
  `cut_spec`s under it. Its target r1 lists "Persona-state reading in Huginn"
  under `not_in_scope`; the Persona ruling contradicts that line, so the
  target needs an r2 (Self admits targets; the proposed text is in the
  rationale).
- Kinds: thirteen (`lib.rs:760-790`). None records a Self session, a claim on
  a spec, a burn rate, Persona state or a landing (`gap-landing-untyped` is in
  force).
- `RulingAuthority { Operator, Standing, Defaulted }` (`lib.rs:458`, in
  `unit_enums!`). Huginn's selection alias domain repeats it
  (`crates/huginn-mind/src/rows.rs:46`, `AUTHORITIES`), the published schema
  repeats it (`schemas/cultnet/epiphany.pipeline.ruling.v2.schema.json:75-78`),
  and the skill repeats it (`references/campaign-state.md:172`). A unit enum
  variant added by name is a widening: stored rulings decode unchanged, which
  the target's `stored-documents-valid` truth requires. (`value_types!` emits
  no serde default on fields, so adding a *field* would not be; map
  `eureka-substrate-map.md` B1. No cut here adds a field to an existing kind.)
- `Faculty` has `Operator` and `SelfFaculty` (`rows.rs:42`); admission does not
  check faculty (`gap-faculty-attribution`, in force). So nothing in the mind
  today can tell the Mind's Self from the operator-facing Self except the
  `session` label on the receipt. Follow-up
  `operator-authority-needs-operator-credential` (in force) asks for a
  credential the operator holds and the Mind's agent does not.
- Instance-rooted kinds are the pattern for a run record: `instance`,
  `stewardship` and `hand_off` key off the instance (`lib.rs:913-946`), and
  Huginn's admission rules for `hand_off` sit at
  `crates/huginn-mind/src/admission.rs:231-262, 328-330`. Per-kind aliases are
  declared at `rows.rs:362`.

### B2. Claude Code on Starfire: auth, headless mode, usage

- `claude --version`: 2.1.268. Binary
  `%AppData%\npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe`.
- Account (`~/.claude.json` `oauthAccount`, secrets not read): Max plan,
  `billingType: stripe_subscription`, rate-limit tier `default_claude_max_20x`,
  extra usage off. There is no API key and no per-token billing: every model
  call the Persona or her Self makes spends the operator's subscription quota.
  VoidBot's own providers (B7) cannot reach Claude at all.
- `~/.claude/.credentials.json` `claudeAiOauth`: an access token with
  `expiresAt` 21:33:57 CEST, file written 13:33 CEST, so the interactive
  access token lives eight hours and is refreshed by the interactive client.
- The failure the brief names, from this session's transcript
  (`~/.claude/projects/F--Projects-CultLib/b471ff9b-….jsonl`):
  `401 OAuth access token has expired. Re-authenticate to continue.`
  (`authentication_error`), reported as "Agent terminated early due to an API
  error" for subagents on `claude-sonnet-5-5`. A running subagent does not
  survive the parent's token expiring.
- `claude setup-token`: "Set up a long-lived authentication token (requires
  Claude subscription)". The binary carries the string "setup-token creates a
  long-lived Claude.ai subscription token" and reads `CLAUDE_CODE_OAUTH_TOKEN`
  (61 references). This is the headless credential for a subscription account:
  no eight-hour expiry, no interactive refresh. Its lifetime and revocation are
  the operator's (it is their subscription); not probed further because
  creating one is the operator's act.
- **Demotion of a revision-1 claim.** Revision 1 said "never read by any
  agent". The token reaches `claude` as an environment variable, and the agent
  inside that process can read its own environment with one shell call. What
  protects the credential is the reach of the user it runs as (B4), not
  secrecy from the agent. Follow-up `persona-credential-in-process-env`.
- Print mode: `claude -p` with `--output-format json|stream-json`,
  `--json-schema`, `--agents <json>`, `--mcp-config`, `--allowedTools`,
  `--permission-mode`, `--max-budget-usd <amount>` ("Maximum dollar amount to
  spend on API"), `--bg`, `--resume`, `--fallback-model`. The `json` output
  carries the run's cost (`total_cost_usd`) and usage. So a headless run has a
  structural spend cap and a measured spend, per run, without any rule in a
  prompt. This is the only Claude path a Node process on Yggdrasil has.
- Plan usage as data: the desktop app's `get_usage` returned, at 18:43 UTC:
  5-hour window 2%, weekly all models 46% (resets 2026-10-05 22:00 UTC),
  weekly Fable 0%. That tool exists only inside the desktop app. The CLI
  binary knows no usage endpoint (`api/oauth/files/`, `api/oauth/organizations/`
  only); it reads `anthropic-ratelimit-unified` response headers (44
  references), which a headless wrapper does not see. **There is no typed
  reading of plan usage reachable from Yggdrasil.** Follow-up
  `gap-usage-not-typed`.
- Local scheduled tasks exist in the desktop app
  (`~/.claude/scheduled-tasks/streampixels-m1-checkin`, disabled, last run
  2026-09-18): a cron inside the desktop app on Starfire. They run only while
  the app is open on the operator's workstation, which the load budget
  (memory `workstation-load-budget.md`: one job at a time, nothing unannounced)
  and "no overnight burns" (memory `token-budget.md`) both argue against for a
  recurring unattended agent. Cloud routines (`schedule` skill) run off-mesh
  (B1) and hold no forge key (B6): dead for this purpose.

### B3. Yggdrasil as the waking host

- No Claude Code installed (`claude: command not found` for `gamecultadmin`;
  no `~/.claude`). `node` v24.14.1 and `npm` present, so `npm i -g
  @anthropic-ai/claude-code` under a dedicated user is the install. No user
  `eureka-mind` exists yet (`id` fails); `/srv/eureka` exists (the forge).
- Load: 16 cores, 62 GiB, load average 1.4, 54 GiB free. The verify stopgap
  runs at most 5 jobs at 4 CPUs and 6 GiB each (`tools/stopgap/ygg-verify.sh`
  header). A Claude Code process is light; its Hands and Soul push their
  heavy work into the same stopgap containers they use from Starfire. The
  stopgap pushes to `~/eureka-verify` over `ssh ygg`; run on Yggdrasil itself
  that alias must resolve to localhost for the unit user, or the script gets
  a local mode. Recorded in the wake cut's `first`.
- Non-resident workloads already run here hand-installed, outside Idunn
  (`systemctl list-timers`): `bifrost-watchdog-notice.timer` (every minute,
  oneshot, user `bifrost-notice`: system user, no shell, no home, credentials
  through `EnvironmentFile=/srv/bifrost/env/persona-delivery.env` root 0600,
  release under `/srv/bifrost/watchdog-notice/releases/<sha>-<sha>` with a
  `current` symlink, `StateDirectory` for journal and receipts),
  `gamecult-state-backup.timer` (nightly), `voidbot-source-refresh.timer`.
  The runbook `bifrost-watchdog-notice-yggdrasil.md` calls this "an interim
  root action on Yggdrasil … until Idunn owns non-resident workloads", and
  ruling `idunn-watchdog:ruling:reader-deploy-owner` chose
  `ops-hand-install`. **That is the precedent the Persona's unit follows**,
  with the same deletion line.
- Idunn: `TargetDeclaration.service: ServiceDeclaration` is a required field
  (`src/deployment.rs:33`), so a recipe without a resident service does not
  parse. Follow-up `idunn-watchdog:follow_up:idunn-non-resident-workloads`
  (in force) is exactly this gap; Idunn-managed units on the host today are
  ghostlight, odin, streampixels-service and streampixels-web.
- Epiphany's own resident Self exists as prior art and is dead: units
  `epiphany-swarm.service` ("Epiphany Yggdrasil resident Self"),
  `epiphany-heartbeat.service` and `epiphany.service` are `inactive dead`. The
  binary (`epiphany-core/src/bin/epiphany-swarm.rs`) runs a 60 s maintenance
  loop over `ResidentSelfState`, `ResidentSelfPolicy`, pressure and typed work
  obligations, on the Codex spine (`epiphany-openai-codex-spine`,
  `codex-connector`). Its doctrine (`epiphany_doctrine.md`): "Heartbeats are
  paced scheduling physiology, not cognition … When no typed work obligation
  is active, the swarm sleeps. Heartbeat does not ruminate, dream, mutate
  memory, or manufacture work." Codex left as supplier 2026-09-05 (memory
  `codex-retirement.md`). The Epiphany Persona permit service that
  `bifrost-persona-mouth.service` expects on Starfire (`10.77.0.2:17877`) is
  not listening either (`netstat`: only 17875 and 17878 on localhost). So two
  native Persona bodies are dead; the operator's ruling names VoidBot's, not
  Epiphany's.

### B4. The forge and the Mind's identity

- `GameCult/Eureka` on `https://eureka.gamecult.org/forge/`: public, default
  branch `main`, pull requests on, **no branch protection** (API
  `branch_protections` → `[]`, with the admin token, on the host). Ruling
  `mind-self-merge-authority` requires protection that forbids direct push to
  main for everyone; nothing enforces it yet.
- Team `minds`: `write` on `repo.code`, `repo.pulls`, `repo.actions`,
  `repo.issues`, `repo.releases`, `repo.wiki`, all org repos. Not admin.
- User `eureka` exists since 2026-10-01T18:49:53Z, not admin, one key
  `eureka@starfire` SHA256:`zcp0m+jV4AUXzGeoh9z+vfJS8xKHo9S865xOK8p6MGM`, which
  matches `~/.ssh/eureka_mind_forge_ed25519.pub` on Starfire. The script
  `provision-forge-minds-eureka` adds one key per host, so a Yggdrasil-run
  agent gets its own key there. The handoff records the Starfire checkouts
  now push and commit as `eureka` and `eureka-admin` holds no keys
  (`F:\Projects\HANDOFF-eureka-self-2026-10-01.md`, "Waiting on the
  operator"); revision 1's follow-up `forge-push-as-eureka` is done by that
  record.
- The charters are not yet in the Eureka repo (`ls ~/.claude/skills/eureka`:
  `LICENSE README.md SKILL.md docs references tools`; no `agents/`). Ruling
  `charters-into-eureka-repo` moves them; that cut is not specified here (it
  is an Eureka-repo cut whose anchors are the Starfire `~/.claude` files, and
  Self owns it as the operator-facing side).
- Reach is the scope boundary, by construction: a Linux user on Yggdrasil
  with the `eureka` forge key, checkouts of Huginn, Epiphany, Eureka and
  gamecult-ops (and VoidBot, under option A of `persona-body-shape`), and no
  GitHub credential, no SSH to Starfire, no Aetheria checkout, cannot touch an
  operator project. No prompt rule is needed for "never touches Aetheria";
  the key ring is the rule. Huginn and Epiphany are on GitHub today: the wake
  cut records how the Mind's user fetches them read-only and pushes branches
  to the forge; GitHub stays the operator-facing Self's.

### B5. The operator's channel and the Persona's mouth

- Bifrost `tools/bifrost-bridge.mjs discord-dm --recipient-id <id> --content
  <text>`: DMs through Bifrost's bot token, exits 75 when it knows nothing was
  sent (retry with the same nonce). `discord-post --channel-id <id>
  --content <text> --persona-name <name> [--persona-avatar-url <uri>]`
  (`bifrost-bridge.mjs:399-450`) posts **as a named Persona through a
  per-channel webhook** that the bridge creates and caches
  (`:821-845`; `BIFROST_DISCORD_PERSONA_WEBHOOK_URL_<channel>` overrides,
  `:924`). The token is `BIFROST_DISCORD_BOT_TOKEN` or `DISCORD_BOT_TOKEN`
  (`:401`). On Yggdrasil `/srv/bifrost/env/persona-delivery.env` (root 0600)
  sets exactly `BIFROST_DISCORD_BOT_TOKEN` and `DISCORD_OWNER_ID` (names
  read with `sed`, values not), and the bridge CLI is installed at
  `/srv/bifrost/persona-feedback/runtime/current/Bifrost/tools/`. **So the
  Persona's mouth exists on the host today with no new code**: one CLI call
  under that env file. The VoidBot worker's own post path
  (`postDiscordViaBifrostCultMesh`, `packages/core/src/bifrost-discord-command.ts:39`)
  reaches the same bridge through a CultMesh command whose default URI is
  `cultmesh://asgard.starfire.bifrost/commands/discord-post` and whose store
  defaults to `E:/Projects/Bifrost/.bifrost/provider-store.cc` (VoidBot
  `config:134-135`): a Starfire-era route, dead on Yggdrasil.
- `tools/watchdog-notice.mjs` is a one-shot reader that turns a typed store
  (Idunn's `incidents.cc`) into owner DMs with a journal, receipts and retry.
  Revision 1 proposed the same program over Huginn as the review channel;
  with the Persona speaking for herself that reader is no longer the primary
  channel (question `persona-report-channel`).
- Inbound: a human addressing the Persona on Discord reaches her today only
  through VoidBot's Discord bot (`apps/bot`, gateway, attention-command
  inbox) or Bifrost's persona-feedback observation path, which depends on
  VoidBot observing (`runbooks/bifrost-persona-feedback-yggdrasil.md:4`).
  Neither runs (B7). Follow-up `persona-inbound-mentions`.

### B6. Scheduling today, and what the skill already derives

- The skill's recipes (`campaign-state.md` "Recipes") already derive the
  queue: specs with no report (minus those blocked by a question raised in
  them), reports with no verdict, open findings by severity, open questions and
  follow-ups, substrate gaps. The operator-facing Self reads them at rehydrate
  and then takes direction from the operator and from prose handoffs
  (`F:\Projects\HANDOFF-eureka-self-2026-10-01.md` is one).
- What the queue cannot say: (1) who is working a spec right now (two Selves
  would both take the first unreported spec); (2) whether a holding verdict
  landed (`gap-landing-untyped`); (3) which of fifteen follow-ups comes first
  (no priority, no order); (4) how active the Mind may be; (5) which Self
  admitted a ruling. (1) and (5) are what the `run` kind carries; (4) is the
  dial (B8). (2) stays its own gap. (3) is deliberately not a field: the
  Persona orders her Self's work by the fixed rule in the rationale, and a
  follow-up that must jump the queue becomes a cut spec.

### B7. VoidBot's Persona machinery: what it is, what it needs, where it runs

Source: `F:\Projects\VoidBot` at `46d891b`; the Eyes pass; `ssh ygg`.

- **Shape.** `apps/persona-scheduler/src/index.ts` is a resident Node process:
  `loadConfig()`, an immediate tick, then a `setTimeout` loop every
  `REPO_FACE_HEARTBEAT_INTERVAL_MINUTES` (default 5; Yggdrasil ran 1). Each
  tick (`persona-scheduler-runner.ts`) is gated in order by: the pause file
  `<cwd>/state/agent-swarm-paused.json` (`control-source.ts:12-24`; missing
  means running, unparseable means paused); `REPO_FACE_HEARTBEATS_ENABLED`
  (default false); then it loads the identity registry, reads each Persona's
  state observation (rest/nap), applies `globalHeat` (B8) and
  `baseRecoveryMinutes` (default 10), idle cooling from Discord activity
  (needs the bot token), semantic pressure (needs Qdrant and an embedder), and
  selects ready participants (`initiative-engine.ts:498-525`): mentioned ones
  always, **at most one unprompted per tick**, and only if its initiative
  clock is due, its response pressure crosses the threshold with unconsumed
  evidence, and `nextUnpromptedTurnAllowedAt` has passed. This is the CTB
  initiative engine the handoff describes ("`initiative-engine.ts` owns
  participant reconciliation plus clock, active-turn freeze/recovery …";
  `notes/fresh-workspace-handoff.md:148`).
- **A turn.** `turn-actuator.ts:9` submits a job `repo-face-rumination` to a
  Postgres `JobQueue` (`createJob`, `initialState: "approved"`). `apps/worker`
  executes it (`index.ts:493`, `executeRepoFaceJobWithInterpreter`). The
  model provider is one of `owner_codex | openai_api | local_llm`
  (`packages/shared/src/index.ts:21`); default `owner_codex` (`codex exec`,
  `owner-codex-provider.ts:405-420`); Yggdrasil's last live `.env` used
  `openai_api` with `mimo-v2.5-pro` (handoff `:199`). **There is no Anthropic
  or Claude provider** (grep over apps, packages, scripts: nothing). The
  Codex runtime spawned a CLI and parsed JSON lines from its stdout
  (`owner-codex-runtime.ts:81-130`): the exact shape a `claude -p
  --output-format stream-json` actuator takes.
- **What a turn can do.** The Face writes naturally; the Interpreter prompt
  (`prompts/repo-face-turn-interpreter.prompt.md`) converts it into blocks:
  `INTERPRETATION`, `STATE NOTE` (memory|need|bond|status|mood|agency), `SAY`,
  `ARTICLE`, `REDDIT THREAD`; line 50: "Do not emit governance or dispatch
  blocks." The worker executes at most one consequential action per turn
  (proposal PR > PR comment > article > Reddit > Bifrost topic > update
  request, `index.ts:582-596`), gated by `REPO_FACE_GITHUB_ACTIONS_ENABLED`
  and `REPO_FACE_BIFROST_ENABLED` (both default false), and posts one `SAY`
  only if nothing heavier fired (`:599-606`). Up to four `STATE NOTE`s go
  through `applyVoidSelfStateOperation` into the Face's `.cc` (`:1990-2127`).
  **No action dispatches development work**: nothing spawns Claude, Codex
  or a workflow from a turn; `WORK`/`DISPATCH` does not exist. The worker's
  MCP tool surface for a turn (`mcp-server-tools.ts`, 21 tools under
  `registerIfAllowed`) is retrieval, Odin interface, Discord posting, Face
  state read/apply, shared documents and `notify_owner`.
- **State.** Two families. (a) Void self-state: types `void.self_profile`,
  `void.thought_memory`, `void.agency_pressure`, `void.face_affect`, …
  (`packages/core/src/void-self-state-domain.ts:5-12`) at
  `identity.faceStatePath` or `<STORAGE_ROOT>/private/repo-faces/<id>.cc`.
  (b) `gamecult.persona_state.v0`: a single global CultCache document
  (`persona-state-domain.ts:83-88`, zod types), read by
  `loadGamecultPersonaState` (`persona-state-service.ts:9-15`) from
  `identity.personaStatePath` for `native_persona` identities only
  (`persona-state-source.ts:78-81`, `.cc` only), projected to prompt text by
  `persona-standard-state-projector.ts:3-33`. **Its only writer is the
  one-shot migration** `migrateCanonicalPortablePersonaState`
  (`persona-state-service.ts:58-68`, refuses an existing target, refuses
  non-canonical provenance). A `STATE NOTE` at turn time writes Void
  self-state, never `gamecult.persona_state.v0`. `persona:export-friendly`
  is a YAML projection (`docs/persona-intake/`), `persona:migrate-portable`
  the one-shot CLI, dry-run by default. Huginn's own legacy repo Persona
  lives at `F:\Projects\Huginn\.voidbot\state\huginn.cc` (Void family;
  `AGENTS.md:43` forbids touching it; `README.md:88-93`). Eureka has no
  `.voidbot/` and no registry entry anywhere.
- **Identity.** The registry is `.voidbot/private/repo-discord-identities.json`
  (ignored, not on Starfire's tree); fields per identity: `id`,
  `identityKind: repo_face | native_persona`, `repoName`, `displayName`,
  `repoPath`, `roleId`, `allowedChannelIds`, `channelPermissions`,
  `avatarUrl`, `faceStatePath`, `personaStatePath`,
  `remotePersonaFeedbackTarget` (`epiphany-identities.ts:120-143`). The
  notes list native Personas (Muninn, Sleipnir, Hermodr, Tengu, Metame) and
  repo Faces (Aqua, Nibu, Mimir, Void, Epiphany, Libby, Huginn, …); none is
  Eureka or "the Mind".
- **Where it runs: nowhere.** `voidbot.service` on Yggdrasil
  (`systemctl cat`) runs only the compose service `swarm-publisher` (the Eve
  dashboard publisher, container `voidbot-swarm-swarm-publisher-1`, up 2
  days); the compose file `/srv/compose/voidbot.yggdrasil.yaml` defines no
  other service. `voidbot-retrieval.service` serves Qdrant and the MCP
  (`compose-qdrant-1`, `compose-mcp-1`). No Postgres container, no worker, no
  bot, no scheduler (`docker ps -a`). The migration map
  (`notes/voidbot-daemon-organ-migration-map.md:5-14`) says it: "Discord bot,
  worker, Persona scheduler, Postgres, Qdrant, and resident cognition organs:
  not restored"; the Starfire scheduled tasks that used to pulse it are
  deleted or disabled. `state/agent-swarm-paused.json` is `paused: true`
  since 2026-07-16 with the reason "stale semantic pressure could authorize
  another Persona model turn on every one-minute observation pulse. Keep
  paused until pressure evidence is consumed only by a committed turn and
  cannot be replayed." No commit has touched `apps/persona-scheduler` or
  `persona-state-service.ts` since 2026-08-01 (`git log --since`), so that
  defect is not recorded as fixed. Follow-up `voidbot-pressure-replay-pause`.

So "the agency machinery already built with voidbot" is: an initiative
engine and turn assembler that are sound and idle, a state family that is
the standard, a job queue and worker that need Postgres and a model provider
that no longer exists for this account, and a mouth that Bifrost already
provides on the host. The fork is how much of the dead body to revive
(question `persona-body-shape`).

### B8. The burn-rate dial already exists, and nobody has turned it

- VoidBot's swarm control: document type `voidbot.swarm_control_state`,
  schema `voidbot.swarm_control_state.v1`, key `voidbot-swarm`, field
  `globalHeat` read only when finite and in `0.05..2`
  (`control-source.ts:28-45`); store `VOIDBOT_SWARM_CONTROL_STORE` or
  `<cwd>/.voidbot/private/swarm-controls.cc`.
- Semantics (`initiative-engine.ts:100-102, 568-570`): a participant's
  effective speed is `speed * heat * dynamicHeat`, clamped `0.1..12`, where
  `heat = clamp(globalHeat * overrides, 0.05, 20)`; "heat changes recovery
  speed but does not fast-forward time" (`:126`). So the dial scales how
  fast initiative recovers, i.e. wake cadence, exactly as ruling
  `mind-self-burn-rate` asks.
- Writer: the swarm publisher's CultMesh command `swarm.set_heat`
  (`scripts/serve-voidbot-swarm-cultmesh.cjs:161-174`, validated `0.05..2`,
  writes `{schemaVersion, globalHeat, commandId, updatedAt, updatedBy}`),
  issued by the Eve dashboard slider (`render-voidbot-swarm-dashboard.mjs:1410-1440`:
  `control.slider`, `bind: voidbot.swarm.globalHeat`, min 0.05 max 2 step
  0.05, scale caption "low 0.05 · nominal 1.00 · hot 2.00") or by
  `scripts/smoke-swarm-heat-command.mjs` over `rudp://127.0.0.1:17873`.
- Live: the publisher listens on `127.0.0.1:17873/udp` (`ss -lunp`, pid
  947038) and its private dir `/srv/voidbot/swarm/state/.voidbot/private/` is
  **empty**: no `swarm-controls.cc`, so the dial has never been set on
  Yggdrasil and reads as `null` → config default `1`. Anyone on the host can
  send `swarm.set_heat` to localhost, including a Mind's agent running there;
  the operator reaches the slider only through whatever lowers the
  `voidbot.swarm` Eve surface (Odin; not probed).
- Run size: nothing in VoidBot derives a per-turn budget from heat; the
  Claude Code `--max-budget-usd` flag (B2) is where run size is enforced, so
  "run size from the dial" is one multiplication in the actuator.

### B9. What Huginn needs to hold `gamecult.persona_state.v0`

- The schema: `F:\Projects\Epiphany\schemas\cultnet\gamecult.persona_state.v0.schema.json`,
  draft-07, `additionalProperties: false` at the root, required
  `schemaVersion`, `provenance` (`sourceSystem`, `sourceDocumentId`,
  `sourceUpdatedAt`, `exportedAt`, `authority: canonical|projection|import`),
  `personaId`, `publicName`, `presentation` (`voiceSummary` required;
  `publicHandles[]`, `homeContext`, `jurisdiction`), `activationProfile`
  (six required sub-objects), `thoughtMemory` (`shortTerm`, `memories`,
  `incubation`), `agencyPressure.pressures`, `candidateActions.actions`,
  `affect` (needs, bonds, status reads, mood, biases, doctrine stances),
  `updatedAt`; optional `values[]`, `privateNotes[]`, `voidbotProjection`.
  Epiphany publishes it and does not consume it; VoidBot's zod types
  (`persona-state-domain.ts`) are the only typed reader. No Rust type exists.
- The mind store: `Mind::open_checked` (`crates/huginn-mind/src/mind.rs:168-187`)
  refuses a store holding any type outside the fifteen it knows
  (`refuse_foreign_types`, `:280`; `kind_of_type`, `docs.rs:39`): the
  thirteen leaf kinds, `huginn.mind_epoch.v1` and the commit receipt. The
  wire (`wire.rs:68-115`) has four operations: `Whoami`, `Admit`, `View`,
  `Query`; responses `Refused(MindRefusal)` and `Deferred`. The daemon's
  `handle` dispatches on the request enum (`serve.rs`). The crate depends on
  `cultcache-rs`, `cultnet-rs`, `epiphany-pipeline`, `rmp-serde`,
  `schemars`, `serde`, `sha2`, `serde_json` (`Cargo.toml:17-29`); no JSON
  Schema validator.
- Reaching it from TypeScript: `cultmesh-ts` has `CultMesh.createRudpClient`
  (`CultLib packages/cultmesh-ts/src/index.ts:6370`) and the publisher
  already uses rudp document publish with receipts (B8), but no TS client
  speaks `huginn.mind_request.v1`; `eureka-state` (Rust) is the only client,
  and it is an MCP server for Claude, not a library. Follow-up
  `huginn-ts-client`.
- The doctrine that routes Persona-state work away from Huginn:
  `F:\Projects\CLAUDE.md:79-85` ("VoidBot owns the working read, projection
  and migration path") and `gamecult-ops/docs/persona-state-standard.md`
  lines 29-33 and 44-55 ("Huginn … As a Persona-state steward is still
  unbuilt … no agent should route Persona-state work to Huginn or wait on
  it"). Both change in the cut that lands the reader
  (`doctrine-persona-in-huginn`), not before.

### B10. Campaign scope

- VoidBot and Bifrost are not campaign repos (B1), and the target excludes
  Persona-state reading. The Huginn, Epiphany, gamecult-ops and Eureka cuts
  of this map fit this campaign; the VoidBot cut (`voidbot-persona-eureka`)
  needs a campaign whose `repos` include `GameCult/VoidBot`. Follow-up
  `persona-cuts-need-a-campaign` (owner: Self).

## The model page

One row per persistent kind this design adds or changes. Rows for the
thirteen existing kinds are in `eureka-substrate-map.md`. Revision 1's
`allowance` row is gone: ruling `mind-self-burn-rate` replaced the budget with
a dial, and the dial is not a document unless `burn-rate-dial-home` says so.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| `ruling` with `authority: Mind` (new variant, no new kind) | Unchanged: `<campaign>:ruling:<label>`. The variant is on the document, so a query by `authority` separates Mind rulings from the operator's. | Created by the Mind's Self when it rules a substrate-internal fork (ruling `mind-rules-which-forks`). Superseded by a `resolution` like any ruling; the operator supersedes a Mind ruling with an Operator ruling in their own session, never by edit. Stored Operator/Standing/Defaulted rulings decode unchanged. | The Mind's Self admits them; the operator may overturn any one after the fact; admission does not check who (gap-faculty-attribution) until `operator-authority-needs-operator-credential` lands, so the `run` record is the attribution meanwhile. Forbidden: a Mind ruling that answers a question the operator-facing Self raised for the operator. |
| `run` (new, instance-rooted; spec `cut-run-kind.r1`, unchanged) | `<instance>:run:<label>`, label = the session label the run admits under (`mind-<date>-<n>` for the Persona's Self, allocated by the actuator from her turn; Self's chosen label for the operator-facing Self). Fields: `instance`, `operated_by: Operator \| Mind`, `host`, `started_on`, `budget_usd` (the `--max-budget-usd` it was launched with, 0 for interactive), `claims`, `campaigns`. | Created at launch, before any other admission of the run. Ended by `Recorded { reason: "<cost_usd>; <what landed>" }` or `Withdrawn { reason }`. A live run older than 24 h is withdrawn by the next Persona turn. Claims are per run; a spec claimed by a live run is not in another run's queue. | The Self that runs it admits it. Admission refuses a `claims` entry already claimed by a live run (`AlreadyClaimed`, Huginn cut). Forbidden: prose handoffs as the record of what a session took on. **The run is also how her Self's results return to her**: her next turn reads runs ended since her last turn. |
| `gamecult.persona_state.v0` for an instance (new in Huginn; spec `cut-huginn-persona-state.r1`) | One global document per mind, stored in the instance's `mind.redb` beside the pipeline kinds, type id `gamecult.persona_state.v0`, keyed by the instance (`personaId` must equal the instance slug). The schema stays Epiphany's published JSON Schema; Huginn validates against it and stores the document opaque, with a receipt. | Born by one migration put from a canonical portable JSON (VoidBot's `persona:migrate-portable` pointed at Huginn). Replaced whole by `PersonaPut { instance, state, expected_updated_at }`: compare-and-swap on `updatedAt`, receipted like an admission; never partially edited. Read by `PersonaGet { instance }`. The old `.cc` copy is a projection the day the put lands (`provenance.authority: projection`). | Huginn admission owns "may this document enter" (schema-valid, `personaId` = instance, CAS holds, `provenance.authority: canonical`). The Persona's own turn is the only live writer (her `STATE NOTE`s become one put). VoidBot owns projection to prompt text and the migration source path. Forbidden: VoidBot's `migrateCanonicalPortablePersonaState` writing a local `.cc` for this instance; any second canonical copy. |
| The burn-rate dial (Body or document; question `burn-rate-dial-home`) | Option A: VoidBot `voidbot.swarm_control_state.v1` `globalHeat` in `/srv/voidbot/swarm/state/.voidbot/private/swarm-controls.cc`, set by `swarm.set_heat`. Option B: an instance-rooted `burn_rate` document `<instance>:burn_rate:<label>`, `{ heat: 0.05..2, set_on }`, one in force, superseded by the next. | A: written whole by each `set_heat`; read every tick. B: admitted by the operator, superseded by resolution, never edited. | The operator only. The Persona reads it and never sets it. A: enforced by denying the Mind's unit user the localhost port (`iptables -m owner --uid-owner`). B: enforced by admission requiring the operator credential. Derived: wake cadence (heat scales initiative recovery, B8) and run size (`--max-budget-usd = base_usd × heat`, actuator). |
| The Persona's identity in VoidBot's registry (Body) | One `native_persona` entry `eureka` in the scheduler instance's `repo-discord-identities.json`: `displayName`, `avatarUrl`, `allowedChannelIds`, `personaStatePath` → the Huginn source (not a file path). | Written by the install runbook; changed by the operator. | gamecult-ops runbook; the operator names her channel and name (`persona-report-channel`). |
| The Mind's credential (`CLAUDE_CODE_OAUTH_TOKEN`, Body) | A root-0600 `EnvironmentFile` on Yggdrasil read by the Persona's unit, like `persona-delivery.env`. | Created by the operator with `claude setup-token`; rotated or revoked by the operator; present in the environment of every `claude` process the unit spawns (B2). | The operator: it is their subscription. Question `persona-host-and-credential`. |
| The Mind's forge key per host (Body) | `eureka@<host>` keys on forge user `eureka`, one per host. | Added by `provision-forge-minds-eureka`; removed by the operator. | gamecult-ops runbook; the operator runs the script as root. |
| The Persona's unit and its journal (Body) | `eureka-persona.service` (simple, long-running: the VoidBot scheduler with one participant), user `eureka-mind`, `StateDirectory` holding the scheduler's own `repo-face-heartbeats.cc` and the actuator journal `runs.cc` (`gamecult.eureka_mind_self_run.v1`: label, started, ended, cost_usd, exit, run id). | Hand-installed by gamecult-ops per `reader-deploy-owner`; dies when Idunn owns non-resident workloads. | gamecult-ops owns the install; the journal is the unit's own and never the Mind's state (the `run` document is). |

Cells not fillable from the Body: the long-lived token's lifetime (the
operator creates it; the first turn proves it); the run liveness bound (24 h
is a proposal); whether the pressure-replay defect that paused VoidBot is
fixed (no commit says so: the cut must prove it for her participant).

## The authority map

- **Owner:** the Persona, Eureka: a VoidBot `native_persona` whose initiative
  the scheduler computes and whose turn is a Claude Code print-mode session
  on Yggdrasil as user `eureka-mind`. She owns: when she speaks; what she
  says; her own state (one put per turn); whether a turn opens work; and,
  through the Self she launches, everything revision 1 gave the Mind's Self:
  which substrate work happens next (the fixed queue order), the dispatch of
  Imagination, Hands, Soul and Life, Mind-authority rulings on
  substrate-internal forks, merging her own PRs after a holding Soul verdict,
  opening campaigns whose `repos` are Mind repos only.
- **Inputs:** her Persona state (Huginn); the mind's queue and the runs
  ended since her last turn (recipes); the dial; recent activity in her
  repos (VoidBot's `repo-activity-source` over her checkouts); the Discord
  channel she lives in (read, for conversation; inbound addressing deferred);
  her checkouts of Huginn, Epiphany, Eureka, gamecult-ops, VoidBot; the verify
  stopgap. Nothing from Starfire, nothing from the operator's projects, no
  prose handoff.
- **Outputs:** Discord posts in her own voice through Bifrost's persona
  webhook; one Persona-state put per turn; at most one Self launch per turn,
  which admits a `run` and then documents in the mind (specs, reports,
  verdicts, findings, follow-ups, Mind rulings, questions for the operator),
  branches and PRs on the forge, merges.
- **Derived state:** the HANDOFF prose files become display-only and are
  deleted once `run` exists; "what is Self working on" is a query over live
  runs; "how active is the Mind" is the dial, never a line in memory or
  doctrine; "what did my Self do" is a query over ended runs; her Discord
  report is a projection of that query in her voice; progress stays a query.
  The timer, allowance and DM-digest reader of revision 1 are not built.
- **Forbidden writers:** the Persona never sets the dial, never rules a fork
  outside substrate-internal ones (she raises a question), never edits
  `~/.claude/` (unreachable), never pushes to GitHub (no credential), never
  runs on Starfire, never answers a question the operator-facing Self raised
  for the operator, never writes her state anywhere but Huginn. VoidBot's
  worker, Postgres queue and providers never execute her turn (option A of
  `persona-body-shape`). The operator-facing Self never claims a spec a live
  Mind run holds, and never writes a prose handoff in place of a `run`.
- **Shared paths:** both Selves admit through the same `eureka-state` tools
  and admission rules; both open a `run` first; both read the queue with the
  same recipes minus live claims; both land code through the same
  branch-protected PR path on the forge and the same merge rule on Huginn and
  Epiphany; both verify through the stopgap; the operator-facing Self and
  the Persona both reach Discord through Bifrost's bridge.
- **Deletion line:** before her unit runs for the first time: the prose
  handoff files are retired into `run`s or deleted; the local `.cc` as the
  canonical Persona store for instance `eureka` never exists (she is born in
  Huginn); the queue recipe gains the live-claim exclusion in the same cut
  that lands `run`; the two doctrine passages of B9 are rewritten in the cut
  that lands the reader; `cut-mind-self-wake.r1`'s timer and wake script are
  superseded (resolution in the mind) and never installed.

## Rationale

### Why a second Self and not a rule (unchanged)

The operator has tried doctrine ("prioritize the functioning of its own
mind"), a dedicated organ (Life), and considered a scored exam. Each puts a
barrier or a side-quest in the path of an agent whose reward is the task in
front of it. The bloodhound observation says the agent will route around all
three. The only shape that uses the instinct instead of fighting it is an agent
whose task *is* the Mind. The Persona ruling sharpens it: the agent whose task
is the Mind is the Persona whose Mind it is, so her obsession with "the
development work they were inevitably obsessed with" is finally dispatchable
by her and not by the operator.

### Why the Persona scheduler is the waking organ, and the timer dies

Revision 1's timer woke a Self every 30 minutes and gated it on a journal of
cost. That is a clock manufacturing opportunities with no pressure behind
them, which Epiphany's doctrine and VoidBot's shared invariant 2 ("the
recurring clock only creates an observation opportunity. It cannot
manufacture model intent") both forbid. VoidBot's initiative engine is
exactly the organ that turns a clock plus pressure plus a dial into "now":
it admits at most one unprompted turn per tick, only when the clock is due
and there is unconsumed evidence (B7). With her pressure source being the
Mind's queue (open findings, unverdicted reports, unclaimed specs, gaps) instead
of Discord semantic pressure, "evidence consumed only by a committed turn" is
a `run` claiming the work: the replay defect that paused VoidBot cannot recur
for her by construction, and the cut must prove that (follow-up
`voidbot-pressure-replay-pause`).

### Why the turn is a Claude Code print-mode session (question `persona-body-shape`, option A)

VoidBot's turn path needs Postgres, a worker and a provider; the only provider
this account can pay for is the subscription through Claude Code (B2, B7).
Reviving Postgres and the worker to run a provider that does not exist buys
nothing. The Codex runtime already shows the actuator shape: spawn a CLI,
read JSON lines, hand the text to the Interpreter. So the cut keeps the
scheduler, the state source, the turn assembler and the Interpreter, and
replaces `submitPersonaTurn` with an actuator that runs `claude -p` with her
prompt, the `eureka-state` MCP and the Bifrost bridge as tools, and parses
the same blocks. Option B (full revival) is three campaigns and a provider
nobody can run; option C (the timer, no VoidBot) contradicts the ruling's
"agency machinery already built with voidbot". A is the smallest machine that
preserves the ruling.

### Why two actuations and not one (question `persona-turn-and-self-split`)

A Persona turn is short, speaks in her voice, updates her state and decides.
A Self run is long, admits documents as `SelfFaculty`, dispatches Hands and
Soul, and must not read as a Discord conversation. Folding them into one
session would make the Interpreter membrane (public speech versus typed
effects, `repo-face-turn-interpreter.prompt.md:1-9`) the same context as the
pipeline, and would spend the dial's budget on speech. So her turn's new block
`WORK` (one per turn, the Interpreter's vocabulary gains it) is what launches
the Self: the actuator admits nothing itself; the Self run opens its `run`,
claims, works one queue step, ends. Her next turn reads the ended run and
says what happened, in her voice. Results return through the Mind, by query,
never by callback (SKILL.md "design order as data").

### Why the dial is reused, and the Mind is kept off it (question `burn-rate-dial-home`)

The dial the operator asked for exists, live, with a slider and a 0.05..2
scale (B8). Reusing it is one firewall rule (the Mind's unit user may not
reach `127.0.0.1:17873`) and one multiplication in the actuator (run size).
Putting it in the Mind as a document is cleaner on paper (one credential
mechanism, one store) but costs a new kind, an admission rule that waits on
the operator credential, and a TS read path. The TS read path is needed
anyway for Persona state (`huginn-ts-client`), so option B becomes cheap
once that lands; A first, B as the later home, is the recommendation.
Either way the dial derives cadence (heat scales initiative recovery) and run
size (`--max-budget-usd = base_usd × heat`), and the Persona never sets it.

### Why Persona state goes in Huginn opaque, validated, receipted (spec `huginn-persona-state`)

Hand-porting the v0 schema into Rust value types is several hundred lines
that would drift from Epiphany's JSON Schema, the published authority. Huginn
validates the document against that schema at the door (a JSON Schema
validator crate; the schema file vendored from Epiphany's commit and pinned
by hash), checks `personaId` = instance and the `updatedAt` CAS, stores the
document as one CultCache envelope in `mind.redb`, and writes a receipt. One
store, one lock, one backup, one receipt stream for the whole Mind ("an
instance owns its mind"). The store's type gate widens by one type; nothing
stored is re-keyed. This defaults a substrate-internal fork (same store versus
a second `persona.redb`) rather than asking the operator: it guards only our
own code.

### Why `Mind` is an authority value and not a faculty (unchanged)

A ruling's authority says whose word it is. `Operator` is the operator's,
`Standing` is doctrine already in force, `Defaulted` is Self's proportion call.
A ruling the Persona's Self makes in its jurisdiction is none of those, and
hiding it under `Defaulted` would make the operator's after-the-fact review
impossible to query. The faculty stays `SelfFaculty`: the Mind's Self is a
Self.

### Why a `run` kind, and why claims live on it (unchanged)

Two Selves over one Mind need exactly one new fact: who is working what. The
run carries its claims and its end, so the queue recipe subtracts live claims
and nobody schedules. It now also carries her Self's results back to her.

### The Self's fixed queue order (unchanged)

Derived at every launch, from the mind, in this order; the first non-empty
step is the run's work, and a run does one step:

1. A blocking question the operator has answered since the last run: skip
   (the operator-facing Self admits rulings).
2. Findings in force, `Blocker` or `High`, on Mind campaigns: Hands, then Soul.
3. Reports with no verdict on Mind campaigns: Soul.
4. Specs with no report, not blocked, not claimed by a live run: Hands.
5. `gap-*` follow-ups in force with no spec: Imagination maps one cut.
6. Nothing: the run ends `Recorded` "idle" with no model call beyond the
   queries. The Persona's turn should not have emitted `WORK` in this case;
   if it did, that is a finding against her prompt.

### Proposed target r2 (Self admits)

`not_in_scope` drops "Persona-state reading in Huginn." and gains "VoidBot's
Discord bot, worker, Postgres queue and providers: not revived; her turn runs
through the Claude Code actuator." `canonical_implementations` gains "Persona
state for an instance: Huginn huginn-mind (opaque, schema-validated), schema
Epiphany `gamecult.persona_state.v0`." Invariants gain
`persona-state-one-writer`: "An instance's Persona state has one canonical
copy, in its mind, replaced whole under compare-and-swap with a receipt; any
other copy carries `provenance.authority: projection`."

### Cut order

1. `mind-authority` (Epiphany, `r1` unchanged): the `Mind` variant.
2. `run-kind` (Epiphany, `r1` unchanged): the `run` kind.
3. `huginn-persona-state` (Huginn, `r1`, this revision): the opaque validated
   document, `PersonaGet`/`PersonaPut`, receipt, store gate widened. No fork.
4. `doctrine-persona-in-huginn` (gamecult-ops, `r1`): the two passages of B9
   and the operator's `F:\Projects\CLAUDE.md:82` line. Depends on 3.
5. `huginn-mind-self-kinds` (Huginn): pin bump, aliases, `AlreadyClaimed`, the
   live-claim subtraction. Depends on 1 and 2; spec after they land.
6. `voidbot-persona-eureka` (VoidBot; needs a campaign, B10): the Claude Code
   turn actuator, the Huginn Persona-state source, the `WORK` block, the Mind
   pressure source, the Self launcher. Blocked on `persona-body-shape` and
   `persona-turn-and-self-split`; spec after the rulings.
7. `mind-self-wake` (gamecult-ops, `r2` supersedes `r1`): user, token file,
   her unit, registry entry, dial reach rule, runbook. Depends on 2 and 3,
   and on 6 by prose. Blocked on `persona-host-and-credential`,
   `burn-rate-dial-home`, `persona-report-channel`.
8. `skill-mind-self` (Eureka): the Self brief her `WORK` launches, with the
   provenance line (`wake-prompt-provenance`), the queue recipe, the
   coexistence rule. Blocked on `persona-turn-and-self-split`.

### Rejected

- A Mind-side priority field on follow-ups: a second scheduler in the data.
- Reviving `epiphany-swarm` or the Epiphany Persona permit path: B3.
- Running the Persona or her Self inside the operator's desktop app: B2.
- A `claim` kind separate from `run`: two lifecycles for one fact.
- Reviving Postgres, the worker and a provider for her turn: B7, no provider.
- A DM reader as the primary review channel: she speaks for herself; the
  reader is at most a secondary notice (`persona-report-channel`).
- Rust value types for the v0 schema: drift from the published authority.
