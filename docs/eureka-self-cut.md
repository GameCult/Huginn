# The Mind's own Self: map

Status: map, Imagination (`imagination-self`), 2026-10-01, session
`self-2026-10-01b`. Campaign `eureka-substrate` in Eureka's mind (instance
`eureka`). Questions, cut specs and follow-ups are typed documents there; this
page keeps body facts, the model page and rationale. Target:
`docs/eureka-substrate-target.md` (`133324a7`). Ruling in force:
`eureka-substrate:ruling:operator-mind-runs-own-self`.

The operator's words, in order (both admitted as rulings; the first was
superseded by the second):

> Eureka thinks in campaigns, right? Well, we already have a Mind campaign, and
> we're adding evidence to it with every run. The design currently delegates
> ultimate design authority and work scheduling to the operator, but neither of
> these is strictly necessary. I want a Self operated by Mind directly.

> The Self I talk to here in this one projection of Eureka's mind, and it's how
> I get it to work on the stuff I care about, like Aetheria. The Mind must
> operate its own Self instance, with the goal of improving itself.

And why, earlier the same evening: "What I want and haven't been able to
achieve, and I'd like to understand why, is for the agent to truly prioritize
the functioning of its own mind. Any instructions in my global prompts to that
effect always get ignored in favor of whatever work is put in front of it."
The operator rejected a scored rehydration exam as the answer, and observed that
RLVR-trained agents chase the objective in front of them "like a bloodhound,
steering be damned", so a barrier in an agent's path is a hazard, not a brake.

That observation is the design law of this map. **The Mind's Self is not a
rule added to the operator-facing Self.** It is a second agent whose only
objective is the Mind, so the bloodhound instinct works for the Mind instead
of against it. Nothing in the operator-facing Self changes except a
coexistence rule it already mostly obeys (one Mind, typed state, no prose
handoffs).

Pinned heads for every fact below:

| Repo | Ref | Commit |
| --- | --- | --- |
| Huginn | `origin/main` | `a699630bbc49552fa747e9c0fabba24598fec77a` |
| Epiphany | `origin/main` (= the leaf Huginn pins, `crates/huginn-mind/Cargo.toml:22`) | `ef9568655e75bb6cb362a038e2739a5ac5e777ee` |
| gamecult-ops | `main` | `2aeb7364fffcd404aada28bfd30fb4e5c753cf53` |
| Eureka skill | `main` (checkout `~/.claude/skills/eureka`) | `b260fbeec6d96ff06882ed9c37d3e1204c424e99` |
| Bifrost | release on Yggdrasil | `160fac20c98b70f3db9110099157d4dd720ac354` |
| Idunn | working tree `F:\Projects\Idunn` | (uncommitted-state unknown; line numbers below are from that tree) |

All probes ran from Starfire on 2026-10-01 between 18:40 and 19:10 UTC
(20:40-21:10 CEST). `lib.rs` is `epiphany-pipeline/src/lib.rs`.

## Body facts

### B1. The Mind (Huginn on Yggdrasil)

- `whoami`: instance `eureka`, endpoint `rudp://10.77.0.1:17872`, reachable,
  377 documents, 120 receipts, index `Current`, epoch
  `epiphany.pipeline.epoch.v2`.
- `huginn.service` (probe: `systemctl cat`): user `huginn`,
  `--bind 10.77.0.1:17872`, hand-managed ("interim unit until Idunn owns it").
  `ss -lunp` shows the socket only on the WireGuard address `10.77.0.1`
  (`wg-gamecult`). So the mind is reachable from Starfire over the mesh and
  from Yggdrasil itself, and from nowhere else. A Claude cloud routine cannot
  reach it.
- Campaigns in the mind (query): `idunn-watchdog` and `eureka-substrate`.
  Rulings in force under `eureka-substrate`: `gap-home`,
  `merge-to-main-first`, `mind-fixes-next`, `scope-core`, `depends-on`,
  `key-bound`, `stewardship-rule`, `self-dep` (Defaulted),
  `operator-mind-runs-own-self`. 15 follow-ups in force, 12 of them `gap-*`.
- Kinds: thirteen (`lib.rs:760-790`). None records a Self session, a claim on
  a spec, a budget, or a landing (`gap-landing-untyped` is in force).
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
  `session` label on the receipt.
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
  extra usage off. There is no API key and no per-token billing: the Mind's
  Self can only ever spend the operator's subscription quota.
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
- Print mode: `claude -p` with `--output-format json|stream-json`,
  `--json-schema`, `--agents <json>`, `--mcp-config`, `--allowedTools`,
  `--permission-mode`, `--max-budget-usd <amount>` ("Maximum dollar amount to
  spend on API"), `--bg`, `--resume`, `--fallback-model`. The `json` output
  carries the run's cost (`total_cost_usd`) and usage. So a headless run has a
  structural spend cap and a measured spend, per run, without any rule in a
  prompt.
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
  recurring unattended Self. Cloud routines (`schedule` skill) run off-mesh
  (B1) and hold no forge key (B6): dead for this purpose.

### B3. Yggdrasil as the waking host

- No Claude Code installed (`claude: command not found` for `gamecultadmin`;
  no `~/.claude`). `node` v24.14.1 and `npm` present, so `npm i -g
  @anthropic-ai/claude-code` under a dedicated user is the install.
- Load: 16 cores, 62 GiB, load average 1.4, 54 GiB free. The verify stopgap
  runs at most 5 jobs at 4 CPUs and 6 GiB each (`tools/stopgap/ygg-verify.sh`
  header). A Claude Code process is light; its Hands and Soul push their
  heavy work into the same stopgap containers they use from Starfire. The
  stopgap pushes to `~/eureka-verify` over `ssh ygg`; run on Yggdrasil itself
  that alias must resolve to localhost for the `eureka` user, or the script
  gets a local mode. Recorded in the wake cut's `first`.
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
  `ops-hand-install`. **That is the precedent the Mind's Self wake unit
  follows**, with the same deletion line.
- Idunn: `TargetDeclaration.service: ServiceDeclaration` is a required field
  (`src/deployment.rs:33`), so a recipe without a resident service does not
  parse. Follow-up `idunn-watchdog:follow_up:idunn-non-resident-workloads`
  (in force) is exactly this gap; Idunn-managed units on the host today are
  ghostlight, odin, streampixels-service and streampixels-web.
- Epiphany's own resident Self exists as prior art and is dead: units
  `epiphany-swarm.service` ("Epiphany Yggdrasil resident Self") and
  `epiphany-heartbeat.service` are `inactive dead`. The binary
  (`epiphany-core/src/bin/epiphany-swarm.rs`) runs a 60 s maintenance loop over
  `ResidentSelfState`, `ResidentSelfPolicy`, pressure and typed work
  obligations, on the Codex spine (`epiphany-openai-codex-spine`,
  `codex-connector`). Its doctrine (`epiphany_doctrine.md`): "Heartbeats are
  paced scheduling physiology, not cognition … When no typed work obligation
  is active, the swarm sleeps. Heartbeat does not ruminate, dream, mutate
  memory, or manufacture work." Codex left as supplier 2026-09-05 (memory
  `codex-retirement.md`). So the shape is right and the substrate is not: the
  Mind's Self runs on Claude Code against Huginn, and borrows the law, not the
  binary.

### B4. The forge and the Mind's identity

- `GameCult/Eureka` on `https://eureka.gamecult.org/forge/`: public, default
  branch `main`, pull requests on, **no branch protection** (API
  `branch_protections` → `[]`, with the admin token, on the host).
- Team `minds`: `write` on `repo.code`, `repo.pulls`, `repo.actions`,
  `repo.issues`, `repo.releases`, `repo.wiki`, all org repos. Not admin.
- User `eureka` exists since 2026-10-01T18:49:53Z (created during this pass, by
  the provisioning the handoff asked for), not admin, one key
  `eureka@starfire` SHA256:`zcp0m+jV4AUXzGeoh9z+vfJS8xKHo9S865xOK8p6MGM`, which
  matches `~/.ssh/eureka_mind_forge_ed25519.pub` on Starfire. The script
  `provision-forge-minds-eureka` adds one key per host, so a Yggdrasil-run
  Self gets its own key there.
- The Starfire checkout `~/.claude/skills/eureka` still pushes as
  `eureka-admin` (`ssh://eureka-forge/…`, alias `eureka-forge` →
  `eureka_forge_ed25519`, title `starfire-claude`). The runbook names that key
  interim. Follow-up `forge-push-as-eureka` (gamecult-ops / Self).
- The charters are not in the Eureka repo: `ls ~/.claude/skills/eureka` gives
  `LICENSE README.md SKILL.md docs references tools`; no `agents/`. The
  charters live only in `~/.claude/agents/*.md` and the doctrine in
  `~/.claude/doctrine/`, on Starfire, operator-owned (Life charter:
  "the operator owns doctrine"). Every charter's self-improvement line says
  "propose changes to the Eureka repo … by pull request on Forgejo". **The
  thing the Mind is told to improve itself through is out of its reach.**
  Question `charters-into-eureka-repo`.
- Reach is the scope boundary, by construction: a Linux user on Yggdrasil
  with the `eureka` forge key, checkouts of Huginn, Epiphany, Eureka and
  gamecult-ops, and no GitHub credential, no SSH to Starfire, no Aetheria
  checkout, cannot touch an operator project. No prompt rule is needed for
  "never touches Aetheria"; the key ring is the rule. (Huginn and Epiphany
  are on GitHub today: the wake cut records how the Mind's user fetches and
  pushes them. The Eureka forge org already mirrors nothing; a question for
  the operator only if the Mind must push to GitHub. Recommended: it pushes
  branches to the forge and GitHub stays Self's.)

### B5. The operator's review channel

- Bifrost `tools/bifrost-bridge.mjs discord-dm --recipient-id <id> --content
  <text>`: DMs through Bifrost's bot token (`BIFROST_DISCORD_BOT_TOKEN`), exits
  75 when it knows nothing was sent (retry with the same nonce). The owner id
  is `DISCORD_OWNER_ID` in `persona-delivery.env`.
- `tools/watchdog-notice.mjs` is a one-shot reader that turns a typed store
  (Idunn's `incidents.cc`, read unlocked, never written) into owner DMs with a
  journal and receipts, exponential retry capped at 60 min, fixed error codes
  that never echo input. The idunn-watchdog campaign established the owner
  rule: the producer never delivers; Bifrost is the bridge. A Mind-notice
  reader is the same program over Huginn (`query` for rulings with authority
  `Mind`, runs ended, questions opened, since the journal's last ordinal)
  instead of over `incidents.cc`.

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
  (no priority, no order); (4) how much the Mind may spend; (5) which Self
  admitted a ruling. (1), (4) and (5) are what the `run` and `allowance`
  kinds below carry. (2) stays its own gap. (3) is deliberately not a field:
  the Mind's Self orders by the fixed rule in the rationale, and a follow-up
  that must jump the queue becomes a cut spec.

## The model page

One row per persistent kind this design adds or changes. Rows for the
thirteen existing kinds are in `eureka-substrate-map.md`.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| `ruling` with `authority: Mind` (new variant, no new kind) | Unchanged: `<campaign>:ruling:<label>`. The variant is on the document, so a query by `authority` separates Mind rulings from the operator's. | Created by the Mind's Self when it rules a fork in its jurisdiction (question `mind-rules-which-forks`). Superseded by a `resolution` like any ruling; the operator supersedes a Mind ruling with an Operator ruling in their own session, never by edit. Stored Operator/Standing/Defaulted rulings decode unchanged. | The Mind's Self admits them; the operator may overturn any one after the fact; admission does not check who (gap-faculty-attribution), so the `run` record is the attribution. Forbidden: a Mind ruling that answers a question the operator-facing Self raised for the operator (its `answers` must name a question raised in a Mind run). |
| `run` (new, instance-rooted) | `<instance>:run:<label>`, label = the session label the run admits under (`mind-<date>-<n>` for the Mind's Self, Self's chosen label for the operator-facing Self). Injective: the wake unit allocates `<n>` from its journal; the operator-facing Self picks a label as today. Fields: `instance`, `operated_by: Operator \| Mind`, `host`, `started_on`, `budget_usd` (the `--max-budget-usd` it was launched with, 0 for interactive), `claims: Vec<PipelineRef>` (the specs, findings and follow-ups it is working), `campaigns: Vec<Slug>` (roots it may admit under). | Created at wake, before any other admission of the run. Ended by a `resolution` `Recorded { reason: "<cost_usd>; <what landed>" }` or `Withdrawn { reason }` (crashed, budget hit). A run with no resolution is live; a live run older than a stated bound (24 h) is treated as dead by the next wake, which withdraws it. Claims are per run; a spec claimed by a live run is not in another run's queue. | The Self that runs it admits it. Admission refuses a `claims` entry that names a spec already claimed by a live run (`AlreadyClaimed`), and a run whose `campaigns` names a root with no campaign. Forbidden: prose handoffs as the record of what a session took on; the HANDOFF files die when this lands. |
| `allowance` (new, instance-rooted) | `<instance>:allowance:<label>`; one in force per window kind, superseded by the next. Fields: `window: FiveHour \| Week`, `cap_usd`, `runs_cap`, `set_on`. | The operator admits one (through the operator-facing Self, `faculty: Operator`); a new one supersedes the old by resolution. Never edited. | The operator only. The Mind's Self reads it and never admits one. The wake unit enforces it (B2: journaled `total_cost_usd` per run summed over the window, refuse to launch past `cap_usd` or `runs_cap`, launch with `--max-budget-usd = min(per-run cap, remainder)`). Question `mind-self-budget`. |
| The Mind's credential (`CLAUDE_CODE_OAUTH_TOKEN`, Body, not a document) | A root-0600 `EnvironmentFile` on Yggdrasil read by the wake unit, like `persona-delivery.env`. | Created by the operator with `claude setup-token`; rotated or revoked by the operator; never read by any agent or printed by any script. | The operator: it is their subscription. Question `mind-self-host-and-credential`. |
| The Mind's forge key per host (Body) | `eureka@<host>` keys on forge user `eureka`, one per host. | Added by `provision-forge-minds-eureka`; removed by the operator. | gamecult-ops runbook; the operator runs the script as root. |
| The wake unit and its journal (Body) | `eureka-mind-self.timer`/`.service`, user `eureka-mind` (system user, no shell), `StateDirectory` journal `runs.cc` (`gamecult.eureka_mind_self_run.v1`: label, started, ended, cost_usd, exit, run id). | Hand-installed by gamecult-ops per `reader-deploy-owner`; dies when Idunn owns non-resident workloads (`idunn-non-resident-workloads`). | gamecult-ops owns the install; the journal is the unit's own and is never the Mind's state (the `run` document is). |

Cells I could not fill from the Body: the long-lived token's lifetime and
whether it is tied to the interactive login (the operator creates it; the
first wake proves it); the `run` liveness bound (24 h is a proposal, not a
probe).

## The authority map

- **Owner:** the Mind's Self, a Claude Code print-mode session on Yggdrasil as
  user `eureka-mind`, admitting as `faculty: SelfFaculty`, `agent: self-mind`,
  `session: mind-<date>-<n>`, under a `run` it opened first. It owns: which
  substrate work happens next (from the queue below), the dispatch of
  Imagination, Hands, Soul and Life for that work, Mind-authority rulings in
  its jurisdiction, merging its own PRs on the forge when the merge question
  rules so, and opening campaigns whose `repos` are Mind repos only.
- **Inputs:** the mind (recipes, by `query`/`view`); its `run`; the
  `allowance` in force; its own checkouts of Huginn, Epiphany, Eureka and
  gamecult-ops; the verify stopgap. Nothing from Starfire, nothing from the
  operator's projects, no prose handoff.
- **Outputs:** documents in the mind (runs, specs, reports, verdicts, findings,
  follow-ups, Mind rulings, questions for the operator); branches and PRs on
  the forge; merges; one DM digest per run through the Mind-notice reader
  (question `mind-self-review-channel`).
- **Derived state:** the HANDOFF prose files become display-only and are
  deleted once `run` exists; "what is Self working on" is a query over live
  runs; "what may the Mind spend" is the allowance in force, never a line in
  memory or doctrine; progress stays a query (unchanged).
- **Forbidden writers:** the Mind's Self never admits an `allowance`, never
  rules a fork outside its jurisdiction (it raises a question), never edits
  `~/.claude/` (unreachable by construction), never pushes to GitHub (no
  credential), never runs on Starfire, never changes a campaign whose `repos`
  include a non-Mind repo, never answers a question the operator-facing Self
  raised for the operator. The operator-facing Self never claims a spec a live
  Mind run holds, and never writes a prose handoff in place of a `run`.
- **Shared paths:** both Selves admit through the same `eureka-state` tools
  and the same admission rules; both open a `run` first; both read the queue
  with the same recipes minus live claims; both land code through the same
  branch-protected PR path on the forge (Eureka) and the same merge rule on
  Huginn and Epiphany; both verify through the stopgap; both notify the
  operator through Bifrost's bridge.
- **Deletion line:** before the wake unit runs for the first time, the prose
  handoff files (`F:\Projects\HANDOFF-*.md`) are retired into `run`s or
  deleted; the `starfire-claude` key on `eureka-admin` is deleted once the
  Starfire checkout pushes as `eureka`; and the campaign-state recipe for
  "specs with no report" gains the live-claim exclusion in the same cut that
  lands `run`, so there is never a window with two queues.

## Rationale

### Why a second Self and not a rule

The operator has tried doctrine ("prioritize the functioning of its own
mind"), a dedicated organ (Life), and considered a scored exam. Each puts a
barrier or a side-quest in the path of an agent whose reward is the task in
front of it. The bloodhound observation says the agent will route around all
three. The only shape that uses the instinct instead of fighting it is an agent
whose task *is* the Mind. That is what the ruling asks for. So the Mind's Self
is dispatched with the Mind as its objective and nothing else in reach, and
coexistence is enforced by what it can touch (B4), not by what it is told.

### Why Yggdrasil, a timer, and a subscription token (recommended in Q1)

- Starfire is the operator's desk, has a load budget, sleeps, and the
  operator does not want unattended burns there. The desktop app's local cron
  runs only while the app is open.
- Cloud routines cannot reach the mind (B1) or the forge key (B4).
- Epiphany's resident Self is the right physiology on the wrong substrate
  (B3); reviving it would mean a Codex spine or a Claude spine for Epiphany,
  which is a campaign of its own and not this one.
- Yggdrasil is already where every non-resident GameCult workload runs, by a
  ruling (`reader-deploy-owner`), with a known deletion line into Idunn. The
  host reaches the mind on its own WireGuard address, holds the verify
  stopgap, and has headroom (B3).
- The credential has to be a long-lived subscription token: the interactive
  token dies in eight hours and killed two agents today (B2), and there is no
  API key on this account. That token is the operator's subscription on a
  server; it needs their yes, and it is why Q1 is a question and not a
  default.

### Why the budget is a document plus a journal, not a prompt line

The operator's quota is the one thing the Mind's Self could burn that the
operator cannot get back inside the week. "Watched, not rationed" (memory
`token-budget.md`) is a policy for a Self the operator is watching; this one
runs when they are asleep. So the cap has to be structural: the wake unit
refuses to launch past the allowance, and each run is launched with
`--max-budget-usd`, which the CLI enforces without reading a prompt. The
allowance is a document the operator admits so that changing it is one
admission and reading it is one query, not a memory file. Plan usage itself is
not readable off the desktop (B2), so the journal of measured run cost is the
enforcement input until `gap-usage-not-typed` is filled; it under-counts the
operator's own sessions, which is the right error direction (the Mind's share
is capped; the operator's is not).

### Why `Mind` is an authority value and not a faculty

A ruling's authority says whose word it is. `Operator` is the operator's,
`Standing` is doctrine already in force, `Defaulted` is Self's proportion call.
A ruling the Mind's Self makes in its own jurisdiction is none of those, and
hiding it under `Defaulted` would make the operator's after-the-fact review
impossible to query. The faculty stays `SelfFaculty`: the Mind's Self is a
Self. Which forks it may rule is Q3; the variant is needed under every answer
but "questions only", which contradicts the ruling.

### Why a `run` kind, and why claims live on it

Two Selves over one Mind need exactly one new fact: who is working what. The
substrate's own lesson (Cut 6d, SKILL.md 0b) is that lifecycle unspecified
on day one is retrofitted on day three, so the run carries its claims and
its end, instead of claims being a kind of their own with a second lifecycle.
Carrying the order in the data the consumer reads (SKILL.md, "design order as
data") means the queue recipe subtracts live claims and nobody schedules.

### The Mind's Self's fixed queue order

Derived at every wake, from the mind, in this order; the first non-empty step
is the run's work, and a run does one step:

1. A blocking question the operator has answered since the last run: Self
   admits nothing here; the operator-facing Self admits rulings. Skip.
2. Findings in force, `Blocker` or `High`, on Mind campaigns: a Hands fix
   batch, then Soul.
3. Reports with no verdict on Mind campaigns: Soul.
4. Specs with no report, not blocked, not claimed by a live run: Hands.
5. `gap-*` follow-ups in force with no spec: Imagination maps one cut.
6. Nothing: the run ends `Recorded` with "idle" and no model call beyond the
   queries. Heartbeat does not manufacture work (B3, Epiphany doctrine).

Each step is one Eureka pass with the existing briefs; the Mind's Self is Self
as the skill already defines it, reading the queue instead of a handoff.

### Cut order

1. `leaf-mind-authority` (Epiphany): the `Mind` variant and the schema. No
   fork; the operator's ruling already names the authority.
2. `leaf-run-kind` (Epiphany): the `run` kind, its key, its resolutions. No
   fork. `allowance` waits for Q2 because its fields depend on the answer.
3. `huginn-mind-self-kinds` (Huginn): pin bump, aliases, the `AlreadyClaimed`
   rule, the live-claim subtraction in the queue, deploy through the
   `ops-mind-upgrade` runbook. Depends on 1 and 2.
4. `skill-mind-self` (Eureka): the alias table, the queue recipe, the Mind's
   Self brief, the coexistence rule in Self's checklist. Blocked on Q3 and Q5.
5. `ops-mind-self-wake` (gamecult-ops): user, install, token file, timer, wake
   script, journal, runbook, forge key. Blocked on Q1 and Q2.
6. `bifrost-mind-notice` (Bifrost): the reader. Blocked on Q5.

Specs admitted in this pass: 1, 2 and 5 (5 with its questions raised in it, so
it reads as blocked). 3, 4 and 6 get specs after the rulings, because their
anchors depend on the answers.

### Rejected

- A Mind-side priority field on follow-ups: a second scheduler in the data.
  The fixed order above plus "a follow-up that must jump the queue becomes a
  spec" covers it.
- Reviving `epiphany-swarm`: B3.
- Running the Mind's Self inside the operator's desktop app as a scheduled
  task: B2, load budget, and it would make the operator's workstation the
  Mind's body.
- A `claim` kind separate from `run`: two lifecycles for one fact.
