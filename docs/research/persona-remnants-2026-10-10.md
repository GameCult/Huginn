# Remnants of the old personas and swarm

Eyes pass eyes-persona-remnants, 2026-10-10, for the swarm revival target (docs/swarm-revival-target.md). Operator: "Let us send off eyes to gather what is left of the old personas and imagination to figure out how to bring the swarm back to life, with Eureka leading". Names, locations, formats and sizes only: no persona memory or message contents, no secrets. Facts tagged FETCHED, SEARCH and KNOWLEDGE.

Note added by Self: Raven offsite backups older than 2026-10-07 were wiped 2026-10-09 (E: reformatted), so any June offsite copy of Void or native Persona state on Raven is gone.

# Eyes facts: what is left of the old Personas and the swarm (2026-10-10)
Read-only. No private message content quoted. No credential read or printed. Dates are file mtimes unless stated.
Prior pass to build on: `F:\Projects\EYES-voidbot-rumination-2026-10-01.md` (why the swarm failed, what was lost).

## A. Where persona state can exist (map)
1. Repo Face state, in git, per repo: `F:\Projects\<repo>\.voidbot\state\<face>.cc` + `.voidbot\voice\identity.json`. Format: VoidBot typed CultCache "Face organ state" (legacy), NOT gamecult.persona_state.v0. No service reads it today. VoidBot `scripts/export-persona-friendly-input.ts` projects it to `gamecult.persona_intake.v0` yaml (header: "projection ... not the canonical state owner"); `scripts/migrate-portable-persona-state.ts` (npm `persona:migrate-portable`, dry-run default, `--write`, `--projection-import`). VoidBot `node_modules` is present on Starfire; I did not run either script.
2. Native Persona state (Tengu, Muninn, Sleipnir, Hermodr, Metame, and Void's own) lived at `<VoidBot>/.voidbot/private/personas/<id>/*.cc` and `.voidbot/private/void-self-state.cc`. Searched `F:` (`*private/personas*`) and `C:\Users\Meta` (tengu*): NOT FOUND. `F:\Projects\VoidBot\.voidbot` does not exist. Only avatars/notes survive (section C).
3. Epiphany native Persona Mind (`EpiphanyMindPersonaMemoryDocument`, `epiphany-persona-service`): inside the Epiphany body state. Live copy removed from Yggdrasil in the teardown (`/srv/epiphany` and `/var/lib/gamecult/epiphany` absent 2026-10-10). Sole whole copy: the backup tarball in section D.
4. Standalone Persona state file: `F:\Projects\Odin\personas\gjallar.persona_state.cc` (41,010 B, 2026-09-05; identical copy in `Odin-bump\personas\`, 2026-09-29). Format not decoded by me (Vili README line 66 calls it "Branding Persona state"). Brokkr and Ymir carry `tools\persona-state-read.ps1` (1.4 KB each; Brokkr 2026-09-05, Ymir 2026-07-22).
5. Discord archive (voidbot MCP `search_history`): human messages through about 2026-06-02; bot/webhook persona posts largely absent (2026-05-21 audit, 10-01 note).
6. Huginn `eureka` mind (eureka-state MCP; 3223 documents, 1108 receipts at query time): holds rulings and cuts about Personas, no old persona memories.

## B. Repo Faces (swarm members that were repo Personas)
State files git-tracked in each repo; identity.json beside each. Bytes = state `.cc`.
| Face | Repo | file | bytes | mtime |
| Nibu | AetheriaLore | .voidbot/state/nibu.cc | 112,242 | 2026-07-22 (content spans 2026-05-18..07-09 per 10-01 note); also in 2 worktree copies |
| Mimir | Mimir | mimir.cc | 111,322 | 2026-07-22 |
| Eve | Eve | eve.cc | 80,839 | 2026-07-22 |
| Libby | CultLib | libby.cc | 72,566 | 2026-07-22 (+ birth baseline.cc 121,528, projection.cc 121,528, msgpacks ~124 KB) |
| Kiko | StreamPixels | kiko.cc | 53,761 | 2026-09-04 |
| Weksa | weksa | weksa.cc | 52,124 | 2026-07-22 |
| Heimdall | Heimdall | heimdall.cc | 44,537 | 2026-07-22 (no identity.json) |
| Huginn | Huginn | huginn.cc | 35,297 | 2026-09-05 |
| Fensalir | Fensalir | fensalir.cc | 29,189 | 2026-07-22 (+ birth baseline/projection ~109 KB) |
| Druzkai | Zyphos | druzkai.cc | 28,491 | 2026-07-22 |
| Norn | Norn | norn.cc | 15,853 | 2026-08-26 |
| Sai | Sai | sai.cc | 15,653 | 2026-09-05 |
| Brokkr | Brokkr | brokkr.cc | 9,881 | 2026-09-05 |
| Aetheria | Aetheria (+catalog/stats/tiers worktrees) | aetheria.cc | 4,238 main (2026-09-26); 8,361 in 3 worktrees | birth/terrain/baseline.msgpack 541,653 |
| Ymir | Ymir | ymir.cc | 4,241 | 2026-09-29 |
15 Face state files (10-01 note: 436 KB). Odin, Bifrost, Hermodr, Muninn repos: no `.voidbot` found, although VoidBot `state/map.yaml` says `odin` is a repo Face at `Odin/.voidbot/`.
Named but no state file found: Epiphany (avatar; also a native Epiphany Persona), Odin (avatar), Muninn (concept avatar), Moobs (intake only), AquariumSynthHost (example registry entry).
Avatars: `F:\Projects\VoidBot\assets\repo-faces\` (epiphany, huginn, kiko, mimir, muninn-concept, odin, weksa, ymir; contact sheet 2026-05-22; avatars-128, selected, source-art); `Odin\assets\personas\` (gjallar x2, sleipnir).
Load-bearing vs residue (from VoidBot docs, not from opening files): the state packet carries private notes, values, activation vectors, affect needs, social bonds, status reads, mood dimensions, durable memories, short-term residue, agency pressures, incubating thoughts, recent speech residue (`persona-state-packet-projector.ts`; VoidBot map line 175). Identity-bearing: values, drives, wounds, fascinations, taste, bonds, hard-won lessons. Residue: short-term residue, speech residue, receipts, pending mentions, room weather. Per-file split not measured.

## C. Native Personas (not repo-bound)
| Persona | What | State | Readable today |
| Tengu | native; the only one migrated to canonical gamecult.persona_state.v0 (VoidBot notes/fresh-workspace-handoff.md, 2026-08-30) | .cc under .voidbot/private/personas: not found | No. Avatar `VoidBot/assets/native-personas/tengu.png`. Legacy stress records preserved under `anchoredThought.extensions.legacyStressResponse` per that note |
| Muninn | native (also a repo, concept avatar) | "typed non-canonical import" per same note | No state found |
| Sleipnir | native; input mirroring / virtual HID speech (state/map.yaml ~line 130); avatar in Odin | typed non-canonical import | No state found |
| Hermodr | native; browser-lowering / typed-command-route speech | typed non-canonical import | No state found |
| Metame (メタめ) | native, projected from the operator's public history; grants discussion, rumination, repo read/propose, discord text, aquarium embodiment | typed Void state, never canonical | Projection only: `VoidBot/docs/persona-intake/metame.persona-intake.yaml` 107,533 B, projectedAt 2026-06-11 |
| Moobs | native | intake projection `moobs.persona-intake.yaml` 8,039 B | yaml readable |
| Void | VoidBot's built-in Discord moderator mind (never a scheduler participant) | void-self-state.cc | LOST (10-01 note: not on Starfire or Yggdrasil; possible Raven offsite 2026-06-03 whose script does not cover .cc) |
| Gjallar | Odin branding Persona | `Odin/personas/gjallar.persona_state.cc` 41,010 B | file exists; format/read path unverified; VoidBot map calls the Gjallar/Eir pair stale |
Also `sai.persona-intake.yaml` 36,712 B (projection of Sai's Face, 2026-06-11).

## D. Epiphany Persona and Epiphany swarm
- Repo `F:\Projects\Epiphany`: `apps/` holds only a README; local `state/` holds `agent-heartbeats.msgpack` 57,750 B (2026-07-22), `ledgers.msgpack` 740,162 B, `runtime-spine.msgpack` 15,303 B, `map.yaml` 121,401 B, `persona-discord.toml` 1,310 B, `persona-reddit.toml` 1,020 B; `agents/` empty (agents.msgpack not on Starfire). `.epiphany-run/` sandbox has active-cognition* dirs. GitHub not yet archived (Huginn map line 37).
- Whole-body backup on Yggdrasil: `/var/backups/epiphany-body-2026-10-09/epiphany-body-state.tar.zst`, 2,932,941,639 B, written 2026-10-09 12:16 UTC, `SHA256SUMS` and `backup.log` beside it (root-only dir). I listed names only (101,261 entries); nothing extracted, no credential file read. Persona-relevant members: `var/lib/gamecult/epiphany/workspace/state/agents.msgpack` (26,963 B), `.../state/agent-heartbeats.msgpack` (57,750 B), `persona-discord.toml`, `persona-reddit.toml`, notes `persona-*-authority-map.md`, `repo-personality-projection-layer-plan.md`, `schemas/heartbeat-state-schema.md`, `schemas/repo-personality-birth-projection.md`, schemas `epiphany.persona-chat`, `epiphany.surface.persona`, `epiphany.heartbeat_initiative_heat.v0`, `gamecult.persona_state.v0`; `var/lib/gamecult/epiphany/local-verse.cc` 42,277,981 B; capstone `runtime.cc` up to 104,433,252 B; ~24 release copies of `epiphany-persona-service` (~35.9 MB each). 1,161 entries match "persona". Another agent is scanning it for secrets.
- Persona-service runtime shape (Epiphany `notes/persona-conversation-authority-map.md`): Bifrost-observed Discord event -> signed delivery -> durable pending mention in the heartbeat -> native loop Projector (Imagination) -> Persona (natural speech) -> Interpreter (Mind converts to typed effects) -> optional Persona self-state mutation (Mind admits) -> signed speech request -> Bifrost posts under a permit; terminal receipt starts heartbeat cooldown. Single bound Persona: agent `epiphany.Persona`, runtime `epiphany-starfire`, Aquarium channel. Stated gap: durable mention obligations unbuilt at head (follow-up `eureka-substrate:follow_up:epiphany-inbound-mentions`).
- Yggdrasil units `epiphany`, `epiphany-swarm`, `epiphany-heartbeat` were inactive before the teardown (Huginn map F5). Nothing Epiphany runs on Starfire (F7).
- Aquarium repos indexed: Aquarium, EpiphanyAquarium (+Bevy, Web): the GUI that showed swarms (operator Discord 2026-05-06).

## E. VoidBot swarm: runtime shape when it last ran
- Pulse: Windows scheduled task "GameCult Local Orchestrator", once a minute (`run-gamecult-orchestrator.ps1`) -> repo Face CTB heartbeats (also Bifrost dispatch, Void mood drift, moderation rumination, watchdog). Disabled 2026-05-21; swarm restarted ~2026-05-22 ("Meet Kiko, Huginn and Weksa", Discord); later replaced by resident daemon `apps/persona-scheduler`. Paused 2026-07-16 (`VoidBot/state/agent-swarm-paused.json`) after stale semantic pressure authorised a model turn every one-minute pulse.
- Turn-taking (`initiative-engine.ts`, 596 lines): serialized wall-clock tick; per participant initiativeSpeed (0.75-1.3, hash of id), operator heat (0.05-20), dynamicHeat, baseRecoveryMinutes, nextTurnAt; recovery = base*(1+0.75*load)/effectiveSpeed; wall clock advances initiative, heat only scales recovery. Ready = active, load<1, not done this tick, not napping (unless mentioned), nextTurnAt <= clock. Order: pending mentions, then nextTurnAt, reactionBias, speed. At most one unprompted turn per tick, only if responsePressure >= interruptThreshold (0.45-0.8) with unconsumed evidence and the global next-unprompted time elapsed.
- Pressure/waker: `responsePressure` = cosine of embedded Face identity-card projection vs recent human messages (age-decayed, noisy-OR); awakens at most 3 Faces; may pull readiness to now once per message (VoidBot map line 195). Mentions (role mention, display-name token, reply to a webhook message) queue pending obligations. Void physiology on its own 5-min cadence.
- Turn execution: assemble prompt (identity doctrine, rooms, Bifrost digest, projected memory/affect prose, home-repo activity, social affordances, raw transcript, curiosity graph) -> approved `repo-face-rumination` worker job -> LLM child writes in character -> cheaper Interpreter emits STATE NOTE / SAY blocks -> worker side effects via Bifrost bridge; receipts into Face state. About 25 hand-written `VoidBot/prompts/repo-face-*.prompt.md`.
- Failure causes recorded: 10-01 note (state rot 2026-05-17, robotic voice, saturated thoughts never gated, no inter-Face bonds, work leaked into social posts, stale scheduler state, "talking to summaries instead of people" 2026-07-16). Survivor plans: `VoidBot/notes/swarm-survivor-repo-agent-plans-2026-07-06.md`; teardown plan/audit: `notes/agent-swarm-teardown-plan-2026-05-21.md`, `agent-swarm-shutdown-audit-2026-05-21.md`.
- Today: Yggdrasil `voidbot.service` runs only the typed `voidbot.swarm` Eve publisher (59,829 B `voidbot-swarm-state.cc`, 2026-09-29) and `voidbot-retrieval.service` (read-only corpus). Discord bot, worker/job loop, persona scheduler, Postgres, resident cognition state "not restored" (VoidBot map line 9). `/srv/voidbot/swarm/state` totals 140 KB, no persona state.
- Operator in Discord (channel 113786069023064068), 2026-06-02: wanted "a swarm of canonical Personas who don't care about repo maintenance" and a representative per Aetheria faction; 2026-05-28: wanted "more voices in the room to steer them". Search hits only, not an exhaustive read.

## F. Eureka's Persona design as ruled (eureka-state)
- `eureka-substrate:ruling:operator-archive-epiphany` (2026-10-01): archive Epiphany; Eureka absorbs what lives.
- `eureka-substrate:ruling:operator-persona-in-mind` (2026-10-01): Persona state belongs to the Mind; Mind's agency takes the shape of VoidBot's Persona machinery; she speaks on Discord and acts through her Self.
- `eureka-body:ruling:wake-target` (2026-10-08): waker starts a Persona turn; Persona decides whether to start a Self run. `eureka-substrate:ruling:every-wake-a-run` (2026-10-09). `eureka-body:ruling:persona-per-mind` (2026-10-09): one Persona per mind; a domain needing an advocate gets its own Huginn instance with its own dial share and waker turns; minds cooperate by ForeignRef and HandOff; proportional-share scheduling cited. Open follow-up `eureka-body:follow_up:weksa-mind`.
- Cuts: `cut-mind-wake.r3` (stateless one-shot waker, cadence since newest Persona turn), `cut-huginn-persona-state.r3`, `cut-doctrine-persona-in-huginn.r3`. Huginn map: Persona state = `gamecult.persona_state.v0`, one per mind; Persona trace store empty (question `persona-trace-retention`, needs the Discord training-policy verification, F14); units `mind-wake@`, `mind-self@`, `mind-persona@` not yet on Yggdrasil.
- Huginn has no persona_state reader, migration path or CultMesh publication; the standard keeps VoidBot as owner of that path. Related, not read by me: `F:\Projects\gamecult-ops\docs\research\multi-persona-prior-art.md` (10,930 B).

## G. Gaps and unverified
- No `.cc` opened or decoded; sizes and mtimes only. Face `.cc` format is from VoidBot docs.
- Native Persona `.cc` (Tengu, Muninn, Sleipnir, Hermodr, Metame) and Void self-state not located on Starfire (searched F: and user profile) or in Yggdrasil `/srv/voidbot`. Raven offsite from 2026-06-03 not checked. No E: drive on Starfire (old docs use E:).
- Epiphany swarm member roster exists only inside `agents.msgpack` in the tarball; not enumerated.
- Qdrant collections of Face memory under `/srv/voidbot/qdrant/collections`: not counted.
- Yggdrasil `/srv/retired/retired-2026-09-30` holds only bifrost compose and a manifest; no persona content.
- I left no files on Yggdrasil (a temp name list in /tmp there was deleted).
