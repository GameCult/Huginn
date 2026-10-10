# eureka-body: map (Imagination, 2026-10-08)

Status: map, Imagination (`imagination-eureka-body`), session `self-2026-10-08`.
Campaign `eureka-body` in Eureka's mind (instance `eureka`). Target in force
`eureka-body:target:r1`, whose doc is Huginn `docs/eureka-self-cut.md` at
`570f369` (map r4 of 2026-10-01). This page keeps body facts, the model page
(step 0b) and rationale. Questions, cut specs, follow-ups and rulings are typed
documents in the mind; read them by id. Self commits this page where the
target doc lives (Huginn `docs/`), probably as `docs/eureka-body-map.md`.

Revised 2026-10-09 (Modeling, session `self-2026-10-09-morning`) after
`leaf-into-huginn` (Huginn `afc35f9`) and the Epiphany body teardown
(gamecult-ops `9c12adb`; leftovers in follow-up
`eureka-body:follow_up:epiphany-body-leftovers`). Pinned heads and facts F1-F14 are
as read on 2026-10-08 and keep their tense; each carries a note where a cut has since
changed it. F15-F17 are new.

Revised again on 2026-10-09 by Imagination (`imagination-brake-stop`, session
`self-2026-10-09-eureka`) at Huginn `df7507b`, after leaf-into-huginn, mind-authority,
mind-control-store and run-kind had merged. New facts are F18-F20, which name the
owner of the brake stop. The Brake, Units and run rows changed, and a rationale
section was added.

Revised again on 2026-10-09 by Imagination (`imagination-launch-r3`, session
`self-2026-10-09-eureka`) at the mind-launch head `8dee875`, after Soul's verdict
`eureka-body:verdict:cut-mind-launch.s1` falsified that cut with nine findings. New fact
F21. The run, Repetition breaker, Brake and Units rows changed, and the rationale gained
"Who decides one live run, and who ends a dead one" and "What the breaker counts".
Specs: `cut-run-live-admission.r1`, `cut-mind-launch.r3`, `cut-mind-wake.r3`,
`cut-brake-stop.r2`.

Revised again on 2026-10-10 by Imagination (`imagination-mind-units`, session
`self-2026-10-10-eureka`) at Huginn `0c6f6cb` and gamecult-ops `6b4a874`, for cut
`mind-units-install`. New fact F22. New section "The mind units: identity, lifecycle and
authority", written before the cut was mapped. Specs: `cut-mind-unit-gate.r1` (Huginn) and
`cut-mind-units-install.r1` (gamecult-ops). Questions: `mind-unit-names` and
`mind-unit-start-grant`.

## Pinned heads (fetched 2026-10-08 ~12:10 UTC)

| Repo | Ref | Commit | Note |
| --- | --- | --- | --- |
| Huginn | `origin/main` = local `main`, `F:\Projects\Huginn`, clean | `6b496eb` | last commit 2026-10-02; deployed daemon is `107552d` (docs-only diff to main) |
| Epiphany | `origin/main` | `ef956865` | was Huginn's leaf pin (no pin since `afc35f9`); local checkout is on `codex/eureka-pipeline-state` `4006b14b`. GitHub `isArchived: false` |
| CodexConnector | `origin/main` | `3ddc12e` | local checkout is on `codex/ghostlight-release-binding` `6519289`; Ghostlight pins `68fe94b` |
| gamecult-ops | `origin/main` | `7e23387` | clean |
| Bifrost | `origin/main` | `160fac2` | clean |
| Eureka (forge) | `main` | `0bb414d` | scratch clone, removed after |
| Idunn | `origin/main` | `6667f0f` | read only; not a campaign repo |

## Body facts

Each line: what was run, where, result.

- **F1 Huginn never left Epiphany (at 6b496eb; cut at `afc35f9`, no build edge remains).** `grep epiphany crates/*/Cargo.toml` (Huginn 6b496eb):
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
- **F3 The leaf at the pin (now Huginn `crates/eureka-pipeline`, `afc35f9`).** Epiphany `ef956865`: `epiphany-pipeline/Cargo.toml`
  (lib `epiphany_pipeline`, deps anyhow, chrono, cultcache-rs at CultLib `8fc74c7`
  = Huginn's own CultLib pin, rmp-serde, schemars, serde; dev serde_json),
  `epiphany-pipeline/src/lib.rs` (schema-publication test reads
  `CARGO_MANIFEST_DIR/../schemas/cultnet` at lib.rs:2725). `schemas/cultnet/` holds
  13 `epiphany.pipeline.*.v2` schemas, `epiphany.work_organ_state.v0`,
  `gamecult.persona_state.v0`, `index.json`, README; `schemas/README.md`.
- **F4 Mind deployment.** `ssh yggdrasil`: `huginn.service` active, ExecStart
  `/opt/gamecult/huginn/current/huginn-daemon --state-root /var/lib/gamecult/huginn
  --instance eureka --bind 10.77.0.1:17872 ...`; `current ->
  releases/107552dc...` (Huginn merge of huginn-admission-rules; on 2026-10-08 main added docs only, and since then main has moved code: see F17).
  UFW admits 17872/udp on wg-gamecult from 10.77.0.2 only (runbook
  `huginn-yggdrasil.md:38,124`). The daemon has no peer authentication
  (`serve.rs`: one session per peer, `max_peers = 256`); any reachable peer admits
  as any faculty.
- **F5 Epiphany residue on Yggdrasil (12:13 UTC, before the teardown; what the teardown left is follow-up `eureka-body:follow_up:epiphany-body-leftovers`).** Units `epiphany.service`,
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
- **F11 gamecult-ops Epiphany tooling, before the teardown** (`git ls-files | grep -i epiphany`, 7e23387):
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
  gamecult-ops base `2aeb7364`. None has a report. Since `leaf-into-huginn` they point at
  a repo nothing builds from; `mind-authority` and `run-kind` have r2 specs against Huginn
  (see Undone).
- **F14 Discord developer policy** on training with message content: the policy page
  (support-dev.discord.com article 8563934450327) answered 403 to a fetch on 2026-10-08.
  Self's recollection, unverified: the policy forbids using message content obtained
  through the API to train AI models without express permission. An Eyes pass must
  verify before question `persona-trace-retention` is ruled.
- **F15 Selection order is admission ordinal.** CultLib `packages/cultnet-rs/src/selection.rs:1998-2005`
  (`order_rows`: ordinal, then schema id, then record key; reversed when descending). So
  "newest" is `descending: true, limit: 1`, and a run label needs no order of its own.
- **F16 Huginn's leaf holds `leaf-key-bound`.** `crates/eureka-pipeline/src/lib.rs`
  at `afc35f9`: `SUBJECT_LOCAL_MAX` (:898), `RESOLUTION_LOCAL_MAX` derived from it (:912),
  `Sha::names_same_commit` (:272). The literal `64` at :915 (the root bound in the
  fits-`Short` assert) is follow-up `eureka-substrate:follow_up:leaf-bound-hygiene`.
  `leaf-key-bound` landed at Epiphany `ef956865` and came in with the leaf.
- **F17 The live Huginn release is still `107552dc`.** Read-only `ssh yggdrasil readlink
  /opt/gamecult/huginn/current` on 2026-10-09: `releases/107552dc4b27b8856b043616ae74c2ebe7908b66`.
  Main (`afc35f9`) is ahead of it by the leaf move and the relicense; no deploy has happened since.
- **F18 At Huginn `df7507b` nothing stops a unit of hers.** Read from `git show df7507b:`
  on 2026-10-09 (Imagination `imagination-brake-stop`). `crates/mind-body` holds
  `control.rs`, `cli.rs`, `main.rs` and `lib.rs`. `cli::run` (cli.rs:40-61) dispatches `show`,
  `brake hold`, `brake release` and `dial set`. `brake hold` is `ControlWriter::open(&path)?.set_brake(false, ..)`
  (cli.rs:53) and does nothing else. `git grep systemctl` over `crates/mind-body` finds nothing.
  The binary sets umask 077 and calls `cli::run` with the root `CONTROL_ROOT` (main.rs:21-37).
  The window probe runs the real binary with `brake release` and `brake hold` 100 times
  (tests/control.rs:573-581), so a hold that ran `systemctl` would run it in the verify container.
  No `launch.rs` exists yet: the Launcher is cut mind-launch. RunTurn and RunOperator carry
  `NAMES` (crates/eureka-pipeline/src/lib.rs:507-508).
- **F19 Yggdrasil has no unit, user or control store of hers.** Read-only `ssh yggdrasil`
  as `gamecultadmin` on 2026-10-09:
  - `systemctl list-units --all 'mind-*'` and `list-unit-files 'mind-*'` both list nothing.
  - `ls /etc/gamecult/minds` gives "No such file or directory". `command -v mind-control`
    finds nothing. `id mind-eureka` gives "no such user".
  - The host runs systemd 257 (257.13-1~deb13u1). `systemctl --help` lists
    `stop UNIT...` and "--no-block  Do not wait until operation finished", so a stop
    waits by default.
  - `systemctl stop --no-ask-password 'zz-probe-none@eureka:*.service'` as non-root returned 0:
    a glob that matches no loaded unit is a no-op. `systemctl is-active 'huginn*.service'`
    matched by glob (`active`).
  - Operators act through `sudo`: the gamecult-ops runbook `huginn-yggdrasil.md:86` uses
    `sudo systemctl stop huginn`, and `/etc/sudoers.d/` holds `90-gamecultadmin`.
- **F20 The ruling decides when the stop happens.** Ruling `eureka-body:ruling:self-run-actuator` says:
  "Holding the brake stops running units (systemctl stop; SIGTERM exits 143 with the turn
  unfinished, section 1.4)". It cites the OpenClaw inbox incident: a stop sent as a message
  was not acted on in time, and the process was killed at the host (prior-art file
  lines 323-325, secondary). Cut mind-wake r2 has the waker return `Held` before any query,
  admission or unit call, and lists "the waker ... ending a live run" among its forbidden writers.
- **F21 What the mind can attribute a document to, at the mind-launch head `8dee875`.** Read
  from a throwaway worktree on 2026-10-09 (Imagination `imagination-launch-r3`).
  - A receipt's provenance is `{ faculty, agent, session, tool }`, all declared by the writer.
    No selection alias reads `session`, so attributing by session means scanning headers.
    `open_and_launch` admits a run with session equal to its run label
    (`crates/mind-body/src/launch.rs:205`). Nothing makes a Self run's later admissions carry
    that label: the Self organ is not built.
  - Citations are typed content. A `cites` selection with no role over every schema is
    supported and tested (`crates/huginn-mind/src/query.rs:1499`). A run's header facts carry
    `turn`, `operated_by` and `claims` (query.rs:139).
  - The breaker at 8dee875 scans every header in the mind, newest first, back to the oldest
    counted run (`crates/mind-body/src/queue.rs:210`). Its cost grows with the whole mind.
  - `Mind::admit` takes `&mut self` (`crates/huginn-mind/src/admission.rs:81`), so
    admissions are serialized. The run rule is `run_claims_rule` (admission.rs:537-547). It is
    applied when a run opens (admission.rs:526-529) and when a withdrawal reinstates a closed
    run (admission.rs:685-694). It checks claims only.
  - `MindRefusal` is huginn-mind's (`crates/huginn-mind/src/refusal.rs`), published in
    `schemas/cultnet/huginn.mind_response.v1.schema.json` and pinned by
    `published_wire_schemas_match_derivation` (wire.rs:276). The leaf's `PipelineRefusal` has
    five variants (`crates/eureka-pipeline/src/lib.rs:59-65`) and is not where admission's rules
    refuse.
  - `Effective::Released { cadence, run_cap_usd }` is a public struct variant
    (`crates/mind-body/src/control.rs:117-120`), so any caller can build one without reading the
    store. `ControlState::effective` is called only by `read_effective` (control.rs:192-197).
  - huginn-mind's `fixtures::run` builds an in-force SelfRun with `operated_by: Mind`
    (fixtures.rs:331-342). The claim tests open several of them at once
    (admission.rs:1987-2015).
  - `Slug::validate_slug` and `Label::validate_label` are public
    (crates/eureka-pipeline/src/lib.rs:287, 386). `RunTurn` and `RunOperator` spell their
    variants through `Debug`, which is also their `NAMES` (lib.rs:436-452).
- **F22 What the mind units would land on, 2026-10-10.** Read-only `ssh yggdrasil` as
  `gamecultadmin` and an Eyes pass over Idunn `6667f0f` (Imagination `imagination-mind-units`).
  - Still no unit, user or store of hers: `systemctl list-unit-files 'mind-*'` lists 0;
    `getent passwd` has no `mind-*` user; `/etc/gamecult/minds` is absent; no `mind-control`
    on PATH. `/opt/gamecult/huginn/current` is `releases/107552dc...` and holds only
    `huginn-daemon` (0755 root). The Release step builds only `-p huginn-daemon`
    (gamecult-ops `runbooks/huginn-yggdrasil.md:57`).
  - systemd 257 (257.13-1~deb13u1): `ExecCondition=` (since 243) exists. Per
    systemd.service(5), an ExecCondition exit of 1-254 skips the unit (inactive, not failed)
    and 255 or a signal fails it; the `+` prefix runs that command with full privileges.
  - **polkit is not installed:** `polkitd` "Installed: (none)", candidate 126-2;
    `/etc/polkit-1/rules.d` absent (`/usr/share/polkit-1/actions` exists, from systemd).
    Without polkitd, systemd refuses an unprivileged caller's `StartUnit`, so no user of hers
    can start a unit today. `/etc/sudoers.d` holds only `90-gamecultadmin` and README.
  - systemd cannot derive a per-instance user from a `<instance>:<label>` instance: no
    specifier splits `%i`, and dash-truncated drop-ins truncate the unit prefix (the part
    before `@`) and re-attach the whole instance (systemd v257 `src/shared/dropin.c`,
    `unit_file_find_dirs`: `unit_name_to_prefix`, `strrchr(prefix, '-')`,
    `unit_name_build_from_type(prefix, instance, ...)`). So `User=mind-%i` works only when
    the instance is the mind's slug alone.
  - `SystemdLauncher::alive` counts `active`, `activating` and `reloading` as alive
    (mind-launch head `origin/eureka-body/mind-launch`, launch.rs:96-99), and starts with
    `systemctl start --no-block`, calling `systemctl` directly (no sudo).
  - Idunn has no target, binding or reference for huginn, mind-control or `mind-*`
    (`/etc/gamecult/idunn/bindings/` holds ghostlight, heimdall, odin, raven-muninn and two
    streampixels). Idunn supervises only workloads it admitted and launched itself as
    transient `systemd-run` units (`Type=exec`, `Restart=no`, `DynamicUser=yes`; Idunn
    `src/drivers.rs:3628-3650`); its continuity loop restarts an admitted workload that is not
    running, up to 6 times an hour, unless its lifecycle brake is engaged
    (`src/control_plane.rs` ~5880-6190, 7932-7956). It has no one-shot, timer or template
    workload (`docs/watchdog-reporting-map.md` row C9) and installs no unit file, polkit rule,
    sudoers file or system user (`src/drivers.rs:3620-3720`). A unit Idunn did not launch is
    never observed or restarted by it.

## The model page (step 0b)

One row per persistent kind this body introduces or changes. Rows for the thirteen
pipeline kinds are in Huginn `docs/eureka-substrate-map.md`.

| Kind | Identity (what names it) | Lifecycle (what happens over time) | Authority (who decides) |
| --- | --- | --- | --- |
| Pipeline wire ids | `epiphany.pipeline.<kind>.v2`, epoch `epiphany.pipeline.epoch.v2`, defined by the leaf, the Huginn crate `crates/eureka-pipeline` (lib `eureka_pipeline`), ids unchanged. | Renamed once to `eureka.pipeline.<kind>.v3` by one `mind.redb` migration, at the next breaking leaf change or before the first hosted mind (ruling `wire-names-rename-before-hosting`). Adding the `run` kind or the `Mind` authority variant is additive (old stored documents decode; no migration), so it does not trigger the rename; a field added with the leaf's `= absent` marker is additive too (leaf-read-anchors r2: old bytes decode, empty fields encode byte-identically, `golden/envelopes.txt` gains lines and loses none); a field added any other way would. | Huginn owns the leaf. The migration is a live-mind upgrade, which is the operator's (`mind-rules-which-forks`). |
| Leaf crate and schema catalogue | Huginn `crates/eureka-pipeline`, `schemas/cultnet/` (13 pipeline schemas, `gamecult.persona_state.v0`, index, README). `epiphany.work_organ_state.v0` dies: no reader outside Epiphany (grep, F1/F3). | Landed at Huginn `afc35f9` (leaf-into-huginn). Wire pinned by `crates/eureka-pipeline/golden/envelopes.txt` and the opt-in `stored_documents_read_back`. Huginn is AGPL-3.0-only (ruling `eureka-body:ruling:leaf-relicensed-agpl`). | Huginn. Forbidden: any Cargo source resolving into GameCult/Epiphany (`nothing-runs-from-epiphany`). |
| `run` | `<instance>:run:<label>` (cut-run-kind r2). Label `mind-<YYYYMMDD>T<HHMMSS><mmm>Z` from mind-launch's `run_label`; two launches in one millisecond produce the same key with different content, which admission answers `Conflict`, so the name stays injective. Order is admission ordinal (F15), not label. | Each wake opens a PersonaTurn run with no claims (ruling `eureka-substrate:ruling:every-wake-a-run`); the Persona tool opens SelfRun runs with claims; both go through `open_and_launch`, and the unit starts only after the run commits (the run is the grant). The Self in a unit admits under its label and ends it `Recorded { reason }`. A run whose unit is not alive past a 120 s start grace is closed `Recorded { reason: "unit not active" }` by the next `open_and_launch` of the same turn, before it checks anything else. It is Recorded, not Withdrawn, so a unit that keeps dying counts as an empty ended run and trips the breaker. Only a unit whose start failed in the launch itself is Withdrawn. Cadence comes from the newest PersonaTurn run's `admitted_at` (any status), not `started_on` (a date cannot carry seconds). | Opening: `open_and_launch`, called by the waker for PersonaTurn and by the Persona tool for SelfRun, each with a `Grant` that only `read_effective` constructs. One live run of hers per turn and instance: huginn-mind admission refuses a second in-force run with `operated_by: Mind` of the same turn and instance as `AlreadyLive { run }` (ruling `one-live-self-run`, option `one-live-per-turn`; cut run-live-admission). `open_and_launch` answers `Busy` from that refusal, or earlier, when the holder it has just read is alive. Operator runs never count. `operated_by` is declared by the writer (follow-up `eureka-substrate:follow_up:operator-authority-needs-operator-credential`). Ending: the Self that worked it; a dead one is closed by the next launch of its turn (cut mind-launch r3). No sweep exists elsewhere. Forbidden: Hands, Soul, Imagination, Life. |
| Repetition breaker | No state of its own (rulings `self-waker`, `merge-gate-own-soul`). Trip condition derived from one selection: the documents citing the item, in any role. Counted runs are the item's ended SelfRun runs of hers (`operated_by: Mind`, not withdrawn) admitted after the newest question `raised_in` the item. Trip when the last 3 counted runs are empty, or 10 of the last 50. A run is empty when no citing document other than a run has an ordinal in its window (its admission up to the next run's, or now). Documents that cite something else never count, wherever they were admitted (F21). | Evaluated in `open_and_launch` when a Self run claims the item, not by the waker. When tripped, it admits one `question` labelled `breaker-<run label>` under the item's campaign, `raised_in` the item, and launches nothing. The run label is the label the refused launch would have used, so every trip has its own key. The existing blocked-spec recipe then removes the item from the queue. Answered by a ruling or withdrawn: either reopens the item, and that question is the boundary, so a reopened item starts a fresh count. | `open_and_launch` admits the question; any Self may answer it (substrate-internal, `mind-rules-which-forks`). |
| Brake | One document per instance in a root-owned CultCache store `/etc/gamecult/minds/<instance>/control.cc`, type `eureka.control.brake.v1 { released: bool, set_at, set_by }`. | Replaced whole by the operator's CLI; never appended. Absent or undecodable reads as held. `mind-control brake hold` writes it held and then, in the same command, stops her running units: `systemctl stop` (blocking) over `mind-self@<i>:*` and `mind-persona@<i>:*`, through `SystemdLauncher::stop_all` (cut brake-stop; ruling `self-run-actuator`; F18-F20). | Operator only (ruling `brake-dial-root-store-now`); her units read it, and the file is not writable by any user she runs as. The hold is the one stopper of her units. `read_effective` is the one reader. Its `Released` answer carries a `Grant` (cadence and run cap, private fields, not `Clone`), which `open_and_launch` takes by value, so no run opens without a released reading (cut mind-launch r3). |
| Burn-rate dial | Same store, `eureka.control.burn_rate.v1 { heat, base_cooldown_s, base_run_usd, set_at, set_by }`; `heat` in 0.05..=2.0. | Replaced whole by the CLI. Absent or out of bounds reads as heat 0, which launches nothing (fail closed, LiteLLM scar rider of ruling `connector-owner`). Schedules and her own rest arrive later as follow-up `burn-schedule-and-rest`. | Operator. Derived (never stored): wake cadence `base_cooldown_s / heat`, per-run cap `--max-budget-usd = base_run_usd × heat`, per-request `max_cost_usd`. |
| Connector request and transcript items | Request: `(caller_runtime_id, request_id)`, the replay key the daemon already keeps (`replay.cc`). Items: UserText, AssistantText, ToolCall, ToolResult, plus `ReasoningState { provider, model, opaque }` (ruling `connector-opaque-state-item`). Contract ids become provider-neutral `gamecult.model.*.v3` (follow-up `connector-contract-untyped`); the connector serves Ghostlight too, so not `eureka.*`. | Invocation expires at `expires_at_unix_ms`; replay record per key; restart-era `Indeterminate`. The caller stores the transcript and replays it verbatim; foreign-tagged reasoning state inside an in-flight tool exchange is refused typed; completed-turn foreign state is settled per provider in the connector cut (follow-up `connector-reasoning-state-cheap-providers`). | The connector produces, reads or refuses reasoning bytes; callers never branch on provider. |
| Connector spend ledger | `gamecult.model.caller_spend.v1` keyed by caller runtime id and window start, in each daemon's state root. | Rolled per window; refusal `Budget` at the cap; durable, and a missing ledger refuses the request. | Each daemon writes its own; the root caller config sets the cap. |
| Persona turn traces | **Empty: question `persona-trace-retention`.** Candidate: per turn, the request digest, the transcript items and the typed output, keyed by the turn's run. | Empty: how long they are kept, whether Discord-sourced content may enter a training export (F14). | Empty: who may export them. |
| Persona state | `gamecult.persona_state.v0`, one per mind, `personaId` = instance (cut-huginn-persona-state r2, eureka-substrate). | Replaced whole under CAS with a receipt; she may rewrite any field (`identity-write-all-visible`). | The Persona organ is the only writer. |
| Identity diffs | The pair of receipts of two consecutive Persona puts whose identity fields (publicName, publicDescription, presentation, values) differ. Derived, not stored. | Composed by the Persona organ at put time and sent as one delivery request through the crossing; a failed post is retried by Bifrost's delivery journal, not by her. | Persona organ composes; Bifrost posts under the permit. No rate limit. |
| Discord body surfaces | Outbound: crossing ids `epiphany.persona_discord_delivery_request.v0` and `..._permit.v0`, agent `epiphany.Persona`, runtime `epiphany-starfire` (F6), renamed to `eureka.*` and `eureka-yggdrasil` in the mouth rebinding (`mouth-crossing-on-yggdrasil`). Inbound: Bifrost persona-feedback deliveries for target `epiphany` (F6). Persona-session tools reach both (`body-includes-discord`). | Request, permit, post, receipt (Bifrost journal). Inbound deliveries: **unprobed** who records that a delivery was answered; `run-is-the-grant` forbids her organ from holding that fact. Blocks the Persona cuts only, not the cuts admitted today. | Bifrost owns posting and delivery records; her permit issuer grants while the brake is released. |
| Units and instance templates | `mind-wake@<instance>.timer/.service`, `mind-self@<instance>:<label>.service`, `mind-persona@<instance>:<label>.service` (one-shot per Persona turn; `RuntimeMaxSec` set at install bounds a hung unit), `mind-permit@<instance>`; users `mind-<instance>` (organs) and `mind-<instance>-self` (Self runs); state `/var/lib/gamecult/minds/<instance>/`; control `/etc/gamecult/minds/<instance>/`. Instance `eureka`. | Hand-installed by gamecult-ops now; Idunn targets declared before the first hosted member, which deletes the hand install (`mind-units-templated-interim`). | gamecult-ops installs; Idunn later. Starting: `open_and_launch` through `SystemdLauncher::start`, as her users, who are granted start only. The instance in a unit name is always the mind's (`MindPort::instance`), passed to every `Launcher` call; `SystemdLauncher` holds none. The instance and label are validated by the leaf's grammar before any `systemctl` argv is built. Stopping: only the operator's `brake hold`, as root (cut brake-stop). The unit-level brake gate and the start-only grant are follow-up `mind-units-brake-gate`. None of this exists on Yggdrasil yet (F19, F22). Each piece's identity, lifecycle and authority: the section "The mind units" below (cut mind-units-install; unit names pending question `mind-unit-names`). |
| Self-run credential | `CLAUDE_CODE_OAUTH_TOKEN` in a root-0600 EnvironmentFile per instance. | Created by `claude setup-token`; rotated by the operator. | **Open: question `self-run-token-reach`.** The Self run's `claude` process can read its own environment, so `token-out-of-her-reach` holds only if the operator names this unit (F4 of the prior-art file, follow-up `token-reach-two-units`). |
| Wake frame | Fixed prompt frame (provenance, jurisdiction, brake obedience) in `/etc/gamecult/minds/<instance>/wake-frame.md`, root-owned. Composed by the Persona organ from host config; the waker passes the unit only the instance and the run label. | Changed only by an install. | Host config (ruling `self-waker` rider 2); never a file her runs merge into. |
| Epiphany residue | F5, F8, F11, as they stood before the teardown. | Teardown landed (gamecult-ops `9c12adb`); what it did not name is follow-up `eureka-body:follow_up:epiphany-body-leftovers`; the repo is archived last. | gamecult-ops for host and repo tooling; Idunn for its legacy target list (follow-up). |

## The mind units: identity, lifecycle and authority

Written 2026-10-10 before cut `mind-units-install` was mapped (operator direction: settle
ownership first). `<i>` is the mind's slug (`eureka` now). Unit names follow the
recommended option of question `mind-unit-names` (`instance-only`): the instance is the slug
alone, one unit per instance and turn, because admission already allows only one live run of
hers per turn (`one-live-self-run`) and only then can `User=` name a per-instance user (F22).
"Root" means the operator through `sudo`, or an agent under her deploy instruction.

| Thing | Identity | Lifecycle | Who may start, stop, enable or change it |
| --- | --- | --- | --- |
| Waker template | `/etc/systemd/system/mind-wake@.service` and `mind-wake@.timer`, sources `systemd/` in gamecult-ops. Instance unit `mind-wake@<i>.service`, `User=mind-%i`. | Installed and replaced by `scripts/install-mind-units-yggdrasil.sh`. The timer is a fixed poll (1 min after the last cycle ends); it decides nothing: the cadence is the dial's, read by the waker from the newest Persona run (mind-wake r3). Schedules and rest (follow-up `burn-schedule-and-rest`) belong in the dial, so they need no unit change. Each tick runs one cycle, `Type=oneshot`, `TimeoutStartSec=5min`, `Restart=no`. Not gated at the unit: the waker reads the brake itself and returns `Held` before any consequence, and a second reading would protect nothing. | Install, change: root via the script. Enable or disable `mind-wake@<i>.timer`: root only, by hand in the runbook. The script never enables it, so installing starts nothing. Start the service: the timer (systemd as root). Her users have no grant on `mind-wake@`. |
| Persona template | `/etc/systemd/system/mind-persona@.service`. Instance unit `mind-persona@<i>.service`, `User=mind-%i`, `Type=exec`, no `[Install]`, so it cannot be enabled. | Inactive until started for one Persona turn. Ends by its own exit, at `RuntimeMaxSec=30min`, or by the brake hold's stop (`TimeoutStopSec=30s`, then SIGKILL to the whole cgroup). `Restart=no`, and nothing else restarts it. Every start first runs the gate (below); while the brake is not Released the unit is skipped and ends inactive. `ExecStart` names the Persona organ's binary in the Huginn release; that cut ships the binary and owns the argv. Until then a released start fails at exec (203/EXEC), and the closing launch counts it as a dead run. | Start: `mind-<i>` only, through `open_and_launch` (the waker), by polkit's verb `start` on this one unit name. Stop, kill, restart: root only, which in practice means `mind-control brake hold` (cut brake-stop). Change: root via the script. Enable: nobody. |
| Self template | `/etc/systemd/system/mind-self@.service`. Instance unit `mind-self@<i>.service`, `User=mind-%i-self`, `Type=exec`, no `[Install]`. | As the Persona unit, with `RuntimeMaxSec=4h`. `ExecStart` names the Self-run actuator's binary in the Huginn release; that cut adds the token `EnvironmentFile` (ruling `self-run-token-reach`), the state directory and the argv. | Start: `mind-<i>` only (the Persona's start-Self-run tool through `open_and_launch`), by polkit, verb `start`. Stop, kill, restart: root only. Change: root via the script. Enable: nobody. |
| Organ user | `mind-<i>`: system user and group, shell `/usr/sbin/nologin`, home `/nonexistent`. | Created by the install script if absent and never modified by it after that. Deleted by root at the mind's retirement. Runs the waker and Persona units. | Root creates and deletes it. It may start `mind-persona@<i>` and `mind-self@<i>`, and nothing else. It cannot write the control store (root 0644, dir 0755) or stop a unit. It can signal its own processes with `kill(2)`, as any Unix user can. That only ends spend, and the ended run is closed Recorded by the next launch, so it adds no second stopper of her units. |
| Self user | `mind-<i>-self`: system user and group, nologin, home `/nonexistent`. The install script refuses an instance slug ending in `-self`, so this name never collides with another mind's organ user. | As the organ user. | Root creates and deletes it. It starts no unit. Polkit refuses it every verb, and the rule refuses any subject whose name ends in `-self`. |
| `mind-control` | `/opt/gamecult/huginn/releases/<sha>/mind-control`, root:root 0700, reached by the `current` symlink; `/usr/local/sbin/mind-control` links to `/opt/gamecult/huginn/current/mind-control`. | Built and installed with each Huginn release by the Release step of `runbooks/huginn-yggdrasil.md`. Replaced when `current` flips. The previous release is kept until the new one verifies. | Install, replace: root via the Release step. Run: root only (mode 0700), which covers the operator's verbs and the gate, which systemd runs as root through the `+` prefix. No user of hers can execute it. |
| `mind-wake` | `/opt/gamecult/huginn/releases/<sha>/mind-wake`, root:root 0755. | Same release step and lifecycle as `mind-control`. | Install, replace: root. Run: the waker unit as `mind-<i>`. |
| The gate | `ExecCondition=+/opt/gamecult/huginn/current/mind-control --instance %i check` in both spending templates. The verb is cut mind-unit-gate. | Runs at every start of a Persona or Self unit. It exits 0 only when `read_effective` is `Released`; held, absent, undecodable or out of bounds exit 2 (systemd skips on 1-254). It holds no state and writes nothing. | Owner: `read_effective`, the one derivation. Change: a Huginn release (the verb) or the script (the line). It closes the window in which a launch that read Released before a hold starts its unit after the hold's stop (follow-up `mind-units-brake-gate`). |
| Polkit rule | `/etc/polkit-1/rules.d/60-mind-units.rules`, root 0644, source `polkit/60-mind-units.rules` in gamecult-ops. Needs package `polkitd` (absent today, F22; question `mind-unit-start-grant`). | Installed and replaced by the script. Instance-generic: one file serves every mind. | Root changes it. It grants exactly one thing: action `org.freedesktop.systemd1.manage-units`, verb `start`, unit `mind-persona@<i>.service` or `mind-self@<i>.service`, to subject `mind-<i>`. Every other request falls through to systemd's default, `auth_admin`, which an unattended caller cannot satisfy. |
| Control store | `/etc/gamecult/minds/<i>/control.cc`, root:root 0644 in a 0755 dir (cut mind-control-store). | The install script writes `brake hold` once, if no store exists, so a fresh install reads `held` with a name and time rather than `absent`. Afterwards only the operator's verbs change it. | Operator only (`brake-dial-root-store-now`). |
| A unit on a brake hold | The instance unit above. | `brake hold` writes the brake held, then stops both units of the instance and waits; the unit ends inactive, its run stays in force. A start after the hold is skipped by the gate and ends inactive (dead), not failed. The next `open_and_launch` of that turn after a release closes the run Recorded "unit not active" (mind-launch r3 step 0). Nothing restarts a stopped unit: `Restart=no`, the waker does not launch without a Grant, and Idunn does not know the unit. | Stopper: the hold only. Restarter: none. |
| Idunn's view | No binding, no target. | Idunn observes and restarts only workloads it launched itself, and every Idunn workload is a resident service that continuity restarts. It has no one-shot, timer or template kind (F22). So today it never touches these units. The deletion line of the hand install (ruling `mind-units-templated-interim`: Idunn targets before the first hosted member) needs an Idunn job workload that is never continuity-restarted. That is follow-up `idunn-job-workload`. | Idunn: none until that follow-up lands. Its owner then may install and change the templates and users, and must never start or restart a spending unit. |

## Rationale

### Why the order is leaf, teardown, control, wake

The four specs admitted today depend on nothing the operator has to rule except the
wake target. `leaf-into-huginn` removed the only build edge into Epiphany and is the
base every Huginn-side body cut needs (the `run` kind re-pointed, Persona state, the
v3 rename); it landed at `afc35f9`. The teardown removed the 24.7 GB (F5) and the tooling that would otherwise be
re-read by every later pass; it landed at gamecult-ops `9c12adb`. The control store comes before the waker because the
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

### Who stops her units when the brake is held

Ruling `self-run-actuator` says holding the brake stops her running units. Three cuts each
held part of that and none held all of it (finding
`cut-mind-control-store.s1.brake-hold-stops-no-units`). The control-store CLI writes the brake.
mind-launch owns the unit actuator, which had start and alive. mind-wake reads the brake. The
stop now has one owner: the operator's `mind-control brake hold`. It writes the brake held
and then, in the same command, runs `SystemdLauncher::stop_all`, a blocking `systemctl stop`
over the instance's `mind-self@` and `mind-persona@` units (cut brake-stop).

- **Synchronous, not on the waker's next tick.** The ruling's own precedent is a stop that
  had to happen at the host. A stop sent as a message was not acted on in time (F20). The
  waker's next tick comes one timer interval later, and mind-wake r2 already keeps the waker
  away from units while the brake is held. Prior art agrees. `systemctl stop` waits for its job
  unless `--no-block` (F19). `kubectl drain` cordons the node and then evicts its pods.
  `supervisorctl stop` waits through `stopwaitsecs` and then kills. In each, the operator's
  command does the stop; no reconciler's next pass does.
- **Write first, then stop, and stop even if the write failed.** This is cordon before drain.
  A unit that starts after the write must already see the brake held. Stopping is always the
  safe direction, so a failed write does not skip it.
- **Privilege.** The hold runs as root because the store is root-owned, and root needs no polkit
  rule to stop a system unit. Her users get start only (follow-up `mind-units-brake-gate`), so no
  organ of hers can stop or restart around the operator.
- **Stop is not on the Launcher port.** `stop_all` is a method on `SystemdLauncher` itself, called
  only from `main.rs`. Code that holds a `Launcher`, such as `open_and_launch` or the waker,
  cannot reach it. The stop patterns come from the same templates as start, so the units
  stopped are exactly the ones started.
- **Not a mind client.** A Self run killed by a hold cannot record its own end. Its run stays in
  force until the next launch of its turn after release closes it as `Recorded` "unit not
  active" (mind-launch r3). The operator's CLI never talks to the mind.
- **One race the CLI cannot close.** A launch can read `Released` (its `Grant`), then the hold
  writes and stops, then the launch starts its unit. The unit's own `ExecCondition=` on the brake closes that, so
  the gate belongs with the units install (follow-up `mind-units-brake-gate`), not in a second
  stopper.
- **Idunn.** Her Self and Persona units are one-shot jobs. Idunn's continuity restarts installed
  daemons, so stopping one of these jobs is not a continuity decision. When Idunn targets
  replace the hand install, Idunn must not restart a unit the hold stopped.

### Who decides one live run, and who ends a dead one

Soul's verdict `cut-mind-launch.s1` found the one-live rule decided in two places, neither
of them sound. `open_and_launch` read for a live run and then admitted later, so two
interleaved launches both passed. The MCP admit door checked nothing, so a second live run
committed directly. Mind-launch r2 had accepted the race, on the grounds that only a Persona
turn starts Self runs. Neither the waker nor the Persona tool exists, so nothing enforced that.

- **Admission owns the rule.** Batches are serialized (`&mut self`, F21), and admission already
  owns the neighbouring exclusivity, `AlreadyClaimed`. The rule is "at most one in-force run
  with `operated_by: Mind` per instance and turn". That is the ruled option's own text: "one
  run cap in flight per turn kind", with Persona turns one at a time in every option. So the
  waker's own PersonaTurn Busy check goes too. The refusal is a new `MindRefusal` variant,
  `AlreadyLive { run }`. `PipelineRefusal` is the leaf's document-shape vocabulary and stays at
  five variants (F21).
- **The primitive owns liveness.** Admission cannot see units. A run whose unit died, never
  started, or lost its admit answer would otherwise hold the turn's slot forever. Before it
  checks anything else, `open_and_launch` reads the turn's live holders of hers. It closes each
  one whose unit is not alive and whose admission is older than the 120 s start grace, and
  answers `Busy` for any that remain. That early `Busy` only refuses, so it cannot make a
  second live run; admission stays the authority for races and for direct admits. The waker's
  sweep in mind-wake r2 was a second owner of the same judgement, and r3 deletes it.
- **Recorded, not Withdrawn.** The breaker treats a withdrawn run as one that never ran. A Self
  unit that crashes every time would then never trip, and would loop at the dial's cadence.
  Closing a dead run as `Recorded` makes a crash an empty ended run, so three in a row raise a
  question. Prior art does the same: a Kubernetes Job counts a failed pod against
  `backoffLimit`, and systemd's `StartLimitBurst` counts failed starts. A start that fails
  inside the launch, before any unit ran, is still Withdrawn.
- **Lease prior art, and why there is no lease.** A Kubernetes Lease or an etcd lease makes
  liveness a renewal the holder must keep sending. Here the host's unit table already is that
  signal (`systemctl is-active`), so a renewal would be a second owner of the fact
  (`run-is-the-grant`). The cost is that a dead run is only noticed when the slot is next
  wanted. That is also the only moment its being in force matters, apart from the queue's
  claimed-by view. That view is wrong for at most one launch attempt, because the next Self
  launch closes the dead holder first.
- **Writer-declared `operated_by`.** A writer that declares `Operator` escapes the rule. This is
  the gap already owned by follow-up `operator-authority-needs-operator-credential`, which must
  land before autonomous Self runs. No second guard is added here.

### What the breaker counts

The ruling's rider says a run is empty when it "admitted nothing but their own run". The
built breaker counted every document admitted anywhere in the mind during a run's window.
Under `every-wake-a-run` a PersonaTurn run always lands in that window, so the breaker could
never trip (finding `breaker-counts-every-document`). Two attributions are available (F21):

- **By receipt session.** It is declared by the writer. Nothing makes a Self run's subagents use
  the run label yet. No alias reads it, so the whole-mind scan stays.
- **By citation of the item (chosen).** Citations are typed content, and one role-less `cites`
  selection on the item returns the runs that claim it, the questions raised in it, and every
  report, resolution, follow-up or deferral that names it. That replaces the whole-mind scan
  with a bounded one. A productive run on a cut spec admits a report, a question, a superseding
  revision or a follow-up. Each of those cites the item, and most also take the item out of the
  queue. Work a Self run does that names another item is not progress on this one.
- **The reset is a question, not a ruling.** A ruling that answers the breaker question cites the
  question, not the item. Counting only runs admitted after the newest question raised in the
  item makes the trip itself the boundary. A reopened item then starts a fresh count, and the
  10-of-50 rule cannot trip again after one more run. Every trip gets its own key,
  `breaker-<run label>`, so a second trip raises a second question.
- **Counted runs are hers.** Only `operated_by: Mind` runs count, because the breaker guards her
  loop (`merge-gate-own-soul`), not the operator's sessions.

### Why the kit keeps its Codex backend

Idunn's migration doc says Codex is dead (F9). The kit is lifted out of CodexConnector,
and the Codex backend's existing tests are the only proof that the lift is behaviour
neutral. Keeping the backend compiling costs nothing; deploying it is not proposed.

### Undone (not mapped this pass)

In dependency order, with what blocks each:

1. eureka-substrate re-points, admitted 2026-10-09 against Huginn: `mind-authority` r2
   (landed on branch `eureka-substrate/mind-authority`, Soul pending), `run-kind` r2
   (absorbs `huginn-mind-self-kinds`: `AlreadyClaimed`, the run's resolution row, the
   `claims` edge; question `persona-turn-run` is ruled by `every-wake-a-run`),
   `huginn-persona-state` r3, `doctrine-persona-in-huginn` r3.
   `leaf-key-bound` landed at Epiphany `ef956865` and came in with the leaf (F16).
   `leaf-read-anchors` r2 (absorbs `huginn-read-anchors`, withdrawn), `ops-read-anchors-upgrade`
   r2 and `context-pack-reads` r2 were admitted 2026-10-09; `leaf-read-anchors` depends on
   `mind-authority` merging.
2. `connector-kit-lift`, `connector-spend`, `connector-reasoning-item`,
   `connector-contract-v3`, `claude-connector`, `openai-compatible-connector`: blocked
   on the kit repo joining the campaign (follow-up `body-campaign-repos`).
3. `self-run-unit` and `skill-mind-self` (Eureka): blocked on `self-run-token-reach`
   and `wake-target`. `mind-launch` r2 (Huginn `df7507b`, unblocked: the
   `one-live-self-run` ruling answered its question), then `brake-stop` r1 and
   `mind-wake` r2, which both depend on it.
4. `mind-units-install` (gamecult-ops): after 3; needs `body-repos-on-forge`.
5. `mind-persona` and `mouth-rebinding` (Huginn, Bifrost): blocked on `wake-target`,
   `persona-trace-retention`, the inbound-delivery cell, and the connector cuts.
6. `doctrine-repoint` (Eureka, F12), `epiphany-archive` (last).

## Admitted this pass (receipts in the mind)

Specs, in dependency order: `eureka-body:cut_spec:cut-leaf-into-huginn.r1`,
`cut-epiphany-body-teardown.r1` (independent), `cut-mind-control-store.r1`,
`cut-mind-wake.r1` (superseded by r2 on 2026-10-09).
Questions: `eureka-body:question:wake-target`, `self-run-token-reach`,
`persona-trace-retention`. Follow-ups: `eureka-body:follow_up:substrate-specs-repoint`,
`body-campaign-repos`. H9 sources exist at Epiphany ef956865: notes/huginn-organ-cut.md,
eureka-body-plane-cut.md, eureka-pipeline-state-cut.md, eureka-pipeline-state-target.md,
eureka-query-prior-art.md, eureka-read-side-cut.md, postmortem-eureka-schema-phase.md.
Huginn carries an AGPL-3.0 LICENSE and the workspace license is AGPL-3.0-only
(rulings `license-split`, `eureka-body:ruling:leaf-relicensed-agpl`).
