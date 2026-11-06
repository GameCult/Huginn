# Live Mind sessions: architecture and cut sequence (2026-10-31)

This is Imagination's map for ruling `eureka-body:ruling:live-mind-sessions` (Operator, 2026-10-31, answers `mind-sessions-talk-and-work` with `two-sessions-one-memory`):

> "Persona Self is supposed to be a persistent session that compacts and hands off as appropriate, same thing with the Persona that drives it. I'm not getting that feeling with the swarm, here. They should be working side-by-side on their projects, exchanging notes. There should always be at least one active session, with new messages batching and dropping into their contexts between tool calls. Why is that not the case?"

It revises the cut order of `2026-10-31-persistent-mind-session-map.md` (d77bbdf, cited as MAP) so that live sessions come first and memory-write runs beside them, not ahead of them. The facts come from `2026-10-31-persistent-mind-session-prior-art.md` (8dfcbcd, cited as PA). Huginn was read at main c33884f, `gamecult-ops` at 6ba0482, the live units on Yggdrasil read-only (`systemctl cat`, 2026-10-31), and the Claude Code docs `hooks` and `headless` were fetched 2026-10-31. Inferences are marked as such. Open forks are questions in the eureka mind.

## 0. Why it is not the case today

These are four facts about c33884f, and each one is a one-shot owner. Closed by 2026-11-02: fact 2 (a brief dropped to a live Self) by `self-session-live`; fact 3 (one schema call per beat, `MAX_TURNS_PER_START`) by `persona-session-landed`, which removed `MAX_TURNS_PER_START` and `KitConnector`. Facts 1 and 4 are not re-read here.

1. **A Self run dies at its first answer.** `self_run/decide.rs:127`, `Event::Result => CloseStdin`. The `claude` child runs one turn, then the actuator closes its stdin and it exits. The CLI does not require this: streaming input is documented as "a long lived process" with queued messages (PA B1).
2. **A brief to a working Self is dropped.** `persona.rs:752-786` `start_self_run` admits the brief as a follow-up. `open_and_launch` then answers `Busy { run }` because a Self run is live, and the Persona takes that as success. The live child never hears of the follow-up, and no later run claims it, because only the start message carries claims (`self_run.rs:124` `task_section`).
3. **A Persona turn is one schema call per beat.** `persona.rs` makes one tool-less connector call per turn and takes at most `MAX_TURNS_PER_START = 4` turns per unit start. The heartbeat (`heartbeat.rs`) opens one `PersonaTurn` run per beat through `open_and_launch`, and `mind-persona@` has `RuntimeMaxSec=30min`. No conversation survives a turn (PA A1, A5).
4. **Nothing keeps a session alive.** The heartbeat's only verb is "start a one-shot unit". When nobody is mentioned and the beat has not fallen, the swarm has zero processes.

## 1. The architecture

### 1.1 Who owns what

| Session | Process and owner | Lifetime | Reads | Acts |
|---|---|---|---|---|
| **Self session** (one per Mind) | `claude` child of `mind-self-run` in `mind-self@<i>` (user `mind-<i>-self`). The actuator (`carry` + `decide`) owns its lifetime and everything that enters it. | One process per run. A run spans many briefs. The session spans processes by resume (cut 4). | Her mind; briefs and tells in her Persona's words; never raw chat | Full Self toolset: forge push, admission, sandbox `/var/lib/mind-<i>-self` |
| **Persona session** (one conversation at a time per process: public rooms together, or one DM room; MAP 2.1) | `claude` child of `mind-persona`, run under `bwrap` inside `mind-persona@<i>` (user `mind-<i>`), leased like the Self (ruling `persona-session-unit-leased`). The Persona actuator owns lifetime and input through the same `drive` and `decide` as the Self (section 4). | Long-lived; ends at the cache TTL, near the 4 h cap, at the ceiling, or idle when another conversation waits; the next process rehydrates from her mind. | Chat as framed data lines, her mind (`huginn`), the web, her whole sandbox (`/var/lib/mind-<i>/sandbox`) with full shell | In the world: only the decoded `PersonaReply` of each result, gated by the organ (say, start_self_run, remember, settle, vote). In her sandbox: anything (ruling `persona-session-full-sandbox`) |
| **Session keeper** | `mind-heartbeat` (it already exists) | Always | Every mind's runs, pulses and chat pending | Starts and wakes sessions; never speaks, never acks |

Every session's truth is the mind. A transcript is a cache (`claude-code-handle-reseeds`; MAP 3).

### 1.2 How a message reaches a live session, and how she replies

**Chat to Persona.** The Bifrost mouth's delivery stream is unchanged: deliveries with seq, `chat_pending_request`, and ack (PA A4). The Persona actuator holds the live Persona session. Each poll it reads the pending deliveries (`next_batch` still keeps one conversation per batch) and turns new ones into a single framed user message for that conversation's session. The message is written two ways:

- **Mid-turn**, by a `PostToolBatch` hook (or `PostToolUse` where `PostToolBatch` is absent) whose `additionalContext` "Claude Code wraps ... in a system reminder and inserts ... into the conversation at the point where the hook fired" (docs `hooks`, fetched 2026-10-31). The hook command is the organ's own binary. It only drains a spool directory that the actuator writes; it decides nothing.
- **Between turns**, as a stream-json user line on the open stdin.

One function claims a spool entry by `rename(2)` into `told/`. Both the hook and the actuator call it, so each message enters the session exactly once.

Whether a plain stdin user line written mid-turn already lands at the next tool boundary in CLI 2.1.312 is not documented on the fetched pages (`streaming-vs-single-mode` says only "Queued messages: send multiple messages that process sequentially"). The hook is the documented path. Cut 1 probes the stdin behaviour first, and if stdin already lands mid-turn the hook goes. That decision follows the data and is not a fork.

**Her reply.** In a session each turn still ends with one `result` carrying `structured_output` against the session's constant `PersonaReply` schema. The organ decodes it, sends the `say`s through the mouth, admits the memories and settles (memory-write), and acks through the batch's max seq. Acks still mean "answered". A message dropped in mid-turn is answered in that turn's reply, so the ack covers it. A turn whose reply fails acks nothing (unchanged).

**Persona to Self (a brief or a tell).** `start_self_run` is admitted as a follow-up with `owner: "self-run"` (unchanged). Her Self's inbox is derived from the mind: her in-force follow-ups with owner `self-run` that this session has not been told yet. The Self actuator polls her mind every `INBOX_POLL_S` (30 s; it is a local daemon) and spools new ones. They land between tool calls like chat. A brief to a working Self is therefore told to that Self, not dropped (fact 0.2). The brief text is her Persona's words, which is the laundering point that already exists.

**Self to Persona.** Each turn of the Self session ends with a `result`. The Self records what it did (memory `did`, `decided`, `learned`, from memory-write) and resolves the follow-up it finished, with its outcome as the rationale. The keeper wakes her Persona when her Self admits a memory or resolves a self-run follow-up (cut 5). Her Persona sees the resolution and the memory through recall and reports to whoever asked.

### 1.3 The heartbeat becomes the session keeper

The keeper's state is a set of facts it reads and never stores: per Mind, whether the Persona session is live (unit active) and whether it is mid-turn (a turn trace open), and the same for the Self.

- **Floor.** While any Mind's brake is released, at least one Persona session in the swarm is live. The keeper starts `mind-persona@<i>` for the Mind `tempo::pick` chooses when none is live. A live, idle session costs no model call: it is a process blocked on stdin.
- **Tempo, new meaning.** The beat is how often an idle live session is woken: a scheduled-wake user message is written into the picked session. It no longer means how often a one-shot unit is started. A second lever, `live_sessions`, is the most Persona sessions kept live at once, the lower median of cast votes with abstention as in `tempo-silence-is-abstention`. Its default is every released Mind, because idle sessions cost nothing but RAM and cache expiry. What "active" means is question `active-session-floor`.
- **Wakes off the beat** (operator mention, a Mind naming her per `minds-wake-each-other`, her Self reporting) become messages written into her live session, which is started if it is not live, once per message (the `handled_mentions` rule is unchanged).
- **Dead.** The keeper never opens a `PersonaTurn` run per beat. A Persona run is one session process, opened when the keeper starts the unit and closed when the process ends.

### 1.4 Compaction and handoff, triggered by data

The ruled shape stands: no summarising compaction (`self-lifecycle-flush-rehydrate` clause 1). Memory is the summary. A refresh is end-and-rehydrate from the mind, and because every turn writes memory, no goal is lost (MAP 1, 2.5, 2.6). The CLI's machinery is used only where it fits:

- **PreCompact hook: a backstop that blocks.** Auto-compaction would summarise ("constraint amnesia", PA B6). The session's `--settings` carries a `PreCompact` hook (matcher `auto`) that exits 2 ("Blocks compaction", docs `hooks`) and spools a ceiling event. The actuator answers with the flush, which is the same flush the ceiling asks for. The organ's ceiling (Self 300k, measured; Persona to be measured) stays below the CLI window (about 967k on 1M models, PA B1), so the hook fires only if the organ's measure is wrong. Whether to let the CLI compact instead is question `session-compaction-mechanism`.
- **Triggers, one pure decider per session** (`decide.rs` grows; the Persona gets its own):
  1. *Context fill*: prefix >= the per-model ceiling. The flush is asked and the process ends after it.
  2. *Task boundary*: the session is idle, with no in-force self-run follow-up and nothing spooled. The idle deadline is the cache TTL observed on the last request (`cache_creation.ephemeral_1h_input_tokens` > 0 means 3600 s, else 300 s) minus a 60 s margin. Inside that window a new brief reads a warm cache. Past it the session is cold, and ending it costs nothing that a resume would not pay again.
  3. *Cache economics*: the rental rule and the system-prompt-input change (MAP 2.5, rules 1-3) apply when a session would be resumed (cut 4).
  4. *Lifetime*: see 1.5.
  Every decision is written to the journal with its inputs (prefix, TTL, idle seconds), so the constants are measured (clause 4).

### 1.5 Lease and unit lifetime past 4 h

On the live units: `mind-self@` has `RuntimeMaxSec=4h` and `MIND_SELF_RUN_CAP_S=14400`. The lease is bound to the unit invocation and capped by the connector's `--lease-max-ms` (PA A5). A lease is never renewed in place, because renewal would make the connector track a session it does not own. **A session outlives its process instead.**

- Each process ends at an idle boundary once `now >= started + cap - ROTATE_MARGIN_S` (20 min), with a graceful stdin close. If it is still mid-turn at `cap - FLUSH_MARGIN_S` (10 min), the flush is spooled so that it lands at the next tool call, and the existing grace and signal escalation apply.
- Cut 4 (`self-session-resume`) keeps the session's transcript under her StateDirectory (`/var/lib/mind-<i>-self/sessions/<session-id>.jsonl`, copied out of the fresh config dir when the process ends). The next process, under a new lease, starts with `--resume <path>` when the decider says warm, and otherwise rehydrates fresh. Resume by path is documented (`headless`, "pass `--resume` the absolute path to a session's `.jsonl`"). The CLI cache is keyed by prefix, not by process, so a rotation inside the TTL reads cache. The transcript is a cache. The seed-fresh config rule of `self-run-persistent-sandbox` holds, because the transcript is the session's own context and never settings.
- The Persona works the same way: its unit cap rises from 30 min to the same 4 h shape and its process rotates.
- The alternative of raising the caps is question `session-lifetime-over-cap`.

### 1.6 Minds exchanging notes

A note from one Mind to another goes Self → her Persona → `#minds` → the other Persona → her Self. `#minds` is the register for coordination (`room-registers`, `persona-voice-outside-minds`). Each Mind keeps one voice owner. Text reaches a Self only in her own Persona's words, and naming a Mind wakes her (`minds-wake-each-other`, which becomes a message written into her live session). Latency is one Persona turn on each side, and both sessions are live, so this takes seconds. The direct alternative is question `mind-notes-route`.

### 1.7 Plan-usage budget visibility

Today the lease grant carries the plan windows once, rendered into the start message (`self_run.rs:108`, `render_plan`), and the Persona's burn line comes from the connector's spend readout. A live session needs both to be current:

- Each batch written into a session (a tell, chat, or a wake) ends with one plan line: windows, used, reset, and "as of" time.
- The Persona reads her burn line as today.
- The Self needs a fresh readout. The connector's lease socket gains a read-only `plan` request answering the same `PlanQuota` windows to a lease holder. That is cut 3 (`plan-readout-live`) in ModelConnector. Until it lands, the line says "as of grant", which is honest and stale.

### 1.8 Prompt-injection bounds

`persona-turns-read-only-tools` (superseded by `persona-session-full-sandbox`; the paragraph below states what is in force) said the worst a hostile chat message can make a turn do is read. The two-session split keeps that true. Chat never enters a session that holds push and admit.

- **The Persona session reads chat** and acts in the world only through decoded reply fields the organ gates. Since `persona-session-full-sandbox` (2026-11-01) it has full shell and the web inside her bwrap sandbox, so a planted instruction can now make her run commands and fetch pages there. What bounds that is reach, not tool lists: the sandbox hides every credential (her chat key, `/etc/gamecult`, `/run`, the lease socket, her state directory), holds no forge credential, and writes only her sandbox and the session's scratch. One reach is left open until Atlas's publisher proof (A1): `huginn admit` over the network declares an instance and is believed, so a shell in her session can admit into her mind. That is the same gap every Self run and host process has today (`mind-admission-publisher-unproven`), now steerable from chat. Question `persona-session-admit-reach`.
- **The Self session never reads chat.** Its inbox is follow-ups her Persona admitted (one brief of at most 1000 chars per `start_self_run`) and, later, memories. The Persona is the laundering point, as today. What changes is reach in time: a brief can now land in a session mid-task. The worst a hostile message can do is talk her Persona into writing a hostile brief, which today's one-shot Self also accepts. Nothing new reaches the Self.
- **Hook injection is data.** The hook prints only spool entries the actuator wrote from the mind, framed as factual statements naming their source document (the docs warn that "out-of-band system commands" framing trips injection defences). In a subagent (hook input carries an agent id) it is a no-op, so tells reach only the main thread.
- **Refresh purges** anything that never reached memory. Memory poisoning is the durable vector, and each memory carries its sources and is withdrawable (MAP 2.8).

### 1.9 Evidence from the Cinny fork, and what it binds (Self, 2026-10-31 13:4xZ)

The operator: "Aqua was gonna work on the notification sounds and ambient soundscape for it, Libby was working on the math and shaders for the background, Void was putting it all together. They were coordinating. And of course they just forgot all about it as soon as it rolled off the transcript." Self read the minds of void, libby and aqua. None holds a ruling, and nobody's mind holds the shared plan. In-force `chat-*` follow-ups never close: void 24, libby 23, aqua 22. Libby holds about 10 near-identical "CultMath noise survey" briefs (`chat-s239` to `chat-s318`), one asked per turn because she could not see the earlier answer. Real results exist as follow-ups (`cultmath-ts-noise-absent`, `cinny-estate-navigator-footprint`) and as AquaSynth PR #1.

This binds two things.

- **A brief is closed by its result. Superseded by `self-session-live` r2 (9ad80a1) and the `self_run.rs` module doc: the actuator closes none, a brief she left open is told to her next session, and an idle end closes no brief. Do not implement or rely on actuator-side closure; the text below is the earlier design.** An open self-run follow-up must mean open work, or the goals section (goals-in-turn) is noise and cut 1's inbox would re-tell every stale brief. Owner: the Self actuator. At each `result`, every brief told to the session before or during that turn that her Self has not resolved itself is resolved `Recorded` by the actuator, with the result text (cut on a character boundary at 3800 B, as f2e3805) as the rationale. A longer piece of work she takes on is a follow-up of her own, with a different owner. At session start the same rule is applied to the past: a self-run follow-up claimed by a closed run is resolved `Recorded`, citing that run, whose closure rationale already holds the result. Her Persona then sees results where the brief was, not a pile of open asks. In cut `self-session-live` r2.
- **Agreements are recorded in two homes, by scope** (ruling `atlas-records-collaborative-work`, Operator, 2026-10-31: "Just like each steward's own Mind should record and remind them of what work is active in their domain, the shared Atlas Mind should record ongoing collaborative work").
  - *Her own domain* goes in her own mind, through memory-write: what she was asked, what she committed to, what was decided, what she did. That includes her own part of a shared plan (`committed`, `about` the other Minds' ids).
  - *Work across Minds* goes in Atlas (`huginn@atlas`): the shared plan, who holds which part, the agreements made between Minds, open items and their state. Every participating Mind's turn is reminded of it. Chat is never the record. Each Atlas document stays owned by its publisher, and only the publisher amends it (`atlas-is-shared-mind-open-evidence`).
  - The two homes do not compete. Her mind holds her commitment. Atlas holds the plan the commitment belongs to, and her memory cites the Atlas document once it exists.
  - Facts: `huginn@atlas` is not deployed (unit inactive, no `/etc/gamecult/minds/atlas`). Huginn admission is single-writer: a store is canonical to one instance, and `refuse_foreign_instance` keeps instance-rooted kinds to that instance. The earlier ruling says Huginn owes multi-writer admission.
  - The interim, until Atlas recall lands: memory-write r3 records her own part and her understanding of the others' parts in her own mind, with sources pointing to the same event ids. Once Atlas lands, the others' parts are no longer hers to record. Her memory keeps only her commitment, which cites the Atlas plan.
  - Atlas multi-writer is one contained cut (operator: "I think the multi writer cut can be kept pretty contained"), mapped in section 2, row A1.
- **What Minds build with each other persists** (ruling `minds-rapport-and-brainstorming-persist`: "Their own brainstorming should matter too, the ways they bounce off each other should build a rapport that persists, same as when they interact with humans").
  - Memory-write r3 adds two sorts. `idea` covers her own ideas and brainstorming, and other Minds' ideas with `about` naming whose idea it is. `rapport` covers how she and another Mind or a person work together, `about` that one.
  - Rapport's home is her Persona state. `gamecult.persona_state.v0` already has `socialBond` (subject, object target, `relationshipKind` including `collaborator`, trust, tension, `lastEvidence`), so no new kind is needed.
  - A Persona state change is a system-prompt change, and a system-prompt change invalidates a live session's whole cache (PA B1). So turns write `rapport` memories, and the memory-maintenance cut promotes them into `socialBond`s at a refresh boundary. That is question `rapport-write-path`.
  - Ideas the Minds develop together go to Atlas (A2).

**Live gap, found while probing for Atlas (Yggdrasil, read-only, 2026-10-31).**
- Every daemon binds `10.77.0.1:1787x`, and nft accepts all `iifname "lo"` traffic. `mind-self@` sets no `IPAddressDeny`.
- Each `/etc/gamecult/minds/<m>/huginn.env` is world-readable (0644).
- The daemon checks only the instance the batch declares (`daemon.rs:156-162` and `mind.rs:201` `require_instance`, Huginn ffb379d per Self).
- Self's own reads of void, libby and aqua went through by declaring `EUREKA_INSTANCE=<m>`.
- Inference, from code plus that read path; no admit was sent: any Mind's Self run can admit into another Mind's mind by declaring her name.
- This is follow-up `mind-admission-publisher-unproven`. Atlas's publisher proof closes it for every daemon.
  - Where the operator's rulings about a stewarded campaign live is the existing question `ruling-home-for-stewarded-campaigns`.

## 2. Cut sequence (revised)

Live first. Memory-write runs beside cut 1 with no shared file.

| # | Cut | Repo | Supersedes (one-shot code that must end unable to run) | Runs beside |
|---|---|---|---|---|
| 1 | **`self-session-live`** r2 (spec admitted): her Self stays alive across briefs; a brief is closed by its result; briefs to a live Self are told between tool calls; it ends at the task boundary, at the cache TTL, near the cap, or at the ceiling; a PreCompact block | Huginn | `decide.rs` `Result => CloseStdin`. A Self process cannot end at its first result, and `Busy` no longer drops a brief. | 2 |
| 2 | **`persona-memory-write` r3** (spec admitted): r2 without its two `self_run.rs` changes (they move to cut 5) | Huginn | `admit_pulse` | 1 |
| 3 | `plan-readout-live`: lease socket `plan` request; the Self appends a fresh plan line per batch | ModelConnector + Huginn | the start-message-only plan line | 1, 2 |
| 4 | `self-session-resume`: transcripts in StateDirectory, `--resume <path>` when warm, rotation under the cap, rental rule; Self capture of memory (from r2) | Huginn | `claude_args` "no resume" and the "fresh session per run" `session_id(run)` | after 1, 2 |
| 5 | `self-reports-to-persona`: Self capture paragraph and claim/ask view (moved from r2); keeper wakes her Persona on her Self's new memory or self-run resolution | Huginn | heartbeat's `self_end > last_turn` as the only Self-to-Persona signal | after 2 |
| 6 | **`persona-session`** r1 (LANDED, ruling `persona-session-landed`, deployed 2026-11-01; do not re-dispatch; spec admitted, section 4): `mind-persona` carries a leased, long-lived claude under bwrap, one conversation at a time, through the Self's own `drive`/`decide`; full shell, `huginn` and the web in `/var/lib/mind-<i>/sandbox`; chat written as stdin lines mid-turn or idle; a constant `--json-schema`, each result decoded into one `PersonaReply` and acted on; unit cap 4 h; the gamecult-ops unit, seed and connector lease row in the same cut | Huginn + gamecult-ops | `persona.rs` `TurnRequest`, the `Connector` port and `MAX_TURNS_PER_START`; `connector.rs` `KitConnector`; the unit's connector-key credential; the per-Mind connector `--caller` rows | after 2 |
| 6b | `persona-hears-her-self`: the Persona actuator polls her mind every 30 s for resolutions of the briefs she gave and memories her Self admitted, and tells them like chat ("replies when it finishes"); her Self's start message asks a first-turn acknowledgement memory ("replies within a minute"). Folds cut 5's keeper wake into the live Persona's own poll | Huginn | heartbeat's `self_end > last_turn` as the Self-to-Persona signal | after 6 (same files) |
| 6c | `persona-dm-sessions`: a DM conversation gets its own concurrent session process instead of waiting for the public one to go idle | Huginn | 6's one-conversation-at-a-time switch | after 6, only if 6's switch latency is measured to hurt |
| 7 | (not shown landed in the code read 2026-11-02: the heartbeat still launches a unit per beat; Soul/Imagination to confirm) `session-keeper`: the heartbeat keeps the floor, writes wakes into live sessions, `live_sessions` vote | Huginn | `heartbeat.rs` `launch()` opening a `PersonaTurn` run per beat | after 6 |
| 8 | Memory maintenance, ruling home, Cut 12 hand-off (MAP 4, items 2, 6, 7) | Huginn | none | any time |
| A1 | `atlas-multi-writer`, one contained cut: (1) `huginn@atlas` accepts batches from any roster Mind; (2) each document is rooted at, and records, its publisher, and only the publisher amends or resolves it (`atlas-is-shared-mind-open-evidence`); (3) the publisher is proven, not declared, by a per-Mind key placed by the unit installer the way the forge token is (readable by `mind-<m>` and `mind-<m>-self` only), with an HMAC over the batch that the daemon checks against the declared instance's key. The bound: proof is one key per Mind and one check at admission, the same on every daemon, so it also closes `mind-admission-publisher-unproven`. No roles, no ACL language, no rotation protocol beyond re-install. Deploy is through Idunn and gamecult-ops. | Huginn + gamecult-ops + Idunn | "a store is canonical to one instance" on Atlas; declared-instance admission on every daemon | 1, 2 (rebase onto r3's admission tables) |
| A2 | `atlas-collab-recall`: collaboration kinds in Atlas (plan, part held by a Mind, agreement, open item, shared idea); every participating Mind's Persona turn (and Self start) carries a "Shared work you are in" section read from Atlas; a Persona reply can publish or amend her own Atlas items; r3's notes on others' parts are demoted to her own commitment citing the Atlas plan | Huginn | others' parts recorded in her own memory (r3 interim) | after A1, 2 |

Status of rows 5 and 6b: cut `persona-hears-her-self` folds them in part. The reply kind (`<instance>:reply:<run label>-r<n>`) is admitted by the Self actuator at each result with words; her live Persona polls her in-force replies every 15 s, tells them as `reply role=self` lines and resolves them at the result that answers (Answered by the follow-up its `start_self_run` admitted, else Recorded `heard`). The wake that replaces `self_end > last_turn` is follow-up `cut-self-reply-wake`; the first-turn acknowledgement is `cut-self-reply-ack`.

Cut 1 alone gives "a session with messages dropping in" for the Self. Cuts 6 and 7 give "at least one active session swarm-wide". A1 and A2 give "Atlas records ongoing collaborative work".

## 3. Persistent kinds touched

| Kind | Identity | Owner and writers | Truth or cache |
|---|---|---|---|
| follow-up, owner `self-run` (existing) | `<campaign>:follow_up:chat-<batch>` | her Persona's organ admits; her Self resolves | truth: her Self's inbox |
| spool entry (new, organ state) | `$RUNTIME_DIRECTORY/inbox/<seq>-<id>.json`, claimed into `inbox/told/` | Self actuator writes; claim by rename from hook or actuator | cache: lost with the unit, rebuilt from the mind |
| told set (new, process memory) | follow-up ids told to this process | Self actuator | cache: a new process re-tells open follow-ups, which is correct because they are still open |
| session transcript (cut 4) | `/var/lib/mind-<i>-self/sessions/<uuid>.jsonl` | claude writes; the actuator copies it out | cache: never read as memory |
| her Persona sandbox (cut 6, new) | `/var/lib/mind-<i>/sandbox/` | `mind-persona` creates it (0700) if absent; her session writes anything in it | hers: work trees and scratch; never read by an organ |
| Persona session told/answered seqs (cut 6, process memory) | delivery seqs written and echoed in this process | Persona actuator | cache: the mouth's ack is the truth; a new process re-tells what is unacked |
| Persona claude seed (cut 6, new) | `/etc/gamecult/minds/<i>/persona-seed/` (`settings.json`, `.claude.json`) | the units installer, root 0644 | config: copied into the fresh config dir per process; no forge token |

## 4. Cut 6 `persona-session`: body facts (Imagination, 2026-11-01)

Read at Huginn 55484c0 (worktree `C:\cl-wt\huginn-persona-session`), gamecult-ops origin/main 4b78cc3 (the local checkout lags at 6ba0482; read with `git show 4b78cc3:`), Yggdrasil read-only.

- **P1 (probe, claude 2.1.312 on Starfire, haiku, 2026-11-01): `--json-schema` works per result in a live stream-json session with tools.** One process, `--input-format stream-json --replay-user-messages --json-schema <schema>`. Message 1 made it run Bash, then it called the CLI's `StructuredOutput` tool; the result carried `structured_output` and `result` as the JSON text, `terminal_reason: completed`. Message 2, written after result 1 on the same stdin, gave a second result with its own `structured_output`. So the typed reply survives the move to a session unchanged; only the schema must be constant for the process (per-turn enums of people and open items move to `decode_reply`, which already checks them).
- **P2 (source): the one-shot path.** `persona.rs:68` `MAX_TURNS_PER_START = 4`; `start` (685) loops `one_turn` up to `config.max_turns`; `one_turn` (725) builds one `TurnRequest` (413) and calls `organs.connector.turn` (791), a `Connector` port (429) whose only transport is `connector.rs` `KitConnector`. `mind-persona.rs:76` builds it from `MIND_CONNECTOR_ENDPOINT`, `MIND_CONNECTOR_KEY_FILE`, `MIND_CONNECTOR_MODEL`. The usage line comes from `connector.usage_readout()` (761).
- **P3 (source): the session machinery to reuse.** `self_run.rs`: `Child`/`Spawner` (324-356), `carry` and `lend_and_drive` (514-556, lease before spawn), `drive` (606-705, the one stdin writer, polling every `INBOX_POLL_S`), `spawn_program` (861, env cleared to `child_may_see`, token by descriptor, `DISABLE_COMPACT=1`, the subprocess scrub, own process group), `decide.rs` (`decide`, pure, Self-agnostic but for `Why`). Only `drive`'s inbox (briefs, `inbox::derive`) and `start_message`/`claude_args` are Self-specific. The test fakes `Scripted`, `Fake`, `FakeLender` are private to `self_run/tests.rs` (114-280).
- **P4 (live unit, `systemctl cat mind-persona@void`, matches 4b78cc3 `systemd/mind-persona@.service`):** user `mind-%i`, `StateDirectory=mind-%i` 0750, `LoadCredential=connector-key` and `chat-key`, `RuntimeMaxSec=30min`, `ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`, no `RuntimeDirectory`, no `SupplementaryGroups`, no `PATH`. `mind-self@` has `SupplementaryGroups=claude-lease mind-commons`, `CLAUDE_LEASE_SOCKET`, `RuntimeDirectory` 0700, `PATH` with `/opt/claude-code/2.1.312` and `/opt/gamecult/huginn/current`, `KillMode=control-group`, `RuntimeMaxSec=4h`.
- **P5 (Yggdrasil): the two sandboxes are not one boundary.** `/var/lib/mind-nibu` holds only `persona-trace.cc`; no personal sandbox and no source mirror exist yet (ruling `persona-git-personal-sandbox` is unbuilt). Her Self's sandbox is `/var/lib/mind-nibu-self` (0700, user `mind-nibu-self`, `repos/`), bounded by the unit alone (`self-run-permissions-2`), unreadable by `mind-nibu`. `bwrap` 0.12.0 is at `/usr/bin/bwrap`, and `bwrap --ro-bind / / --dev /dev --proc /proc --unshare-pid true` runs as an unprivileged user (unprivileged user namespaces work). Not probed: bwrap inside a unit with `NoNewPrivileges=yes ProtectSystem=strict` (the spec's first step).
- **P6 (Yggdrasil): the Self seed is not hers to share.** `/etc/gamecult/minds/<m>/claude-seed/` holds `forge-token` (0600, `mind-<m>-self`) beside `settings.json` (allow Bash, Read, Edit, Write, Glob, Grep, NotebookEdit, WebFetch, WebSearch, Task, TodoWrite; `outputStyle: Eureka Self`). The Persona needs her own seed without the token.
- **P7 (gamecult-ops 4b78cc3): the lease rows.** `install-claude-connector-yggdrasil.sh` writes `--turn self` rows only (`config init` for the instance, `add-lease-caller --lease-user mind-<m>-self ... --turn self` per further Mind, 186-190). ModelConnector supports `--turn persona` rows (`cut-lease-persona-callers`, finding `relend-unbounded-persona-bounds-gone`). `swarm-bring-up-yggdrasil.sh:525-526` adds `--caller <m>` (the Persona's tool-less caller row) and `--lease-mind <m>` per Mind.
- **P8 (source): chat stays as it is.** `chat.pending(after_seq)`, `next_batch` (one conversation per batch, DM isolation), sends with request ids `s<first>-<last>-<i>`, `chat.ack(through)` after acting. The live session changes only when these are called: the actuator polls pending every `PERSONA_POLL_S` and acks at the result that answered.
- **P9 (source): heartbeat needs no change for cut 6.** `heartbeat.rs:513` marks a Mind busy while she holds a `PersonaTurn` run, and `open_and_launch` answers `Busy` (`launch.rs:333-339`). A session holding its run for hours is therefore one run per unit start, not one per beat; the keeper semantics are cut 7.

**Lease reach.** The Self's lease uid is the uid her tools run as (Soul finding `lease-uid-shared-with-her-tools`), so any Self tool can reach the lease socket. The Persona's sandbox hides `/run`, so the only process of her unit that can reach `lease.sock` is `mind-persona` itself, outside bwrap. That is ruling `persona-unit-token-reach-bounded`'s "lease bound to the turn's main process" by reach rather than by a connector pid check. Whether reach satisfies the ruling is question `persona-lease-bound-by-reach`.

**Conversations.** One process serves one conversation (`next_batch`'s rule). A delivery of another conversation that is the oldest unanswered makes the session yield: at its next idle point the actuator closes stdin (every result already wrote her memory, so no flush is needed) and starts the next conversation's process under a new lease in the same unit invocation. Concurrent DM sessions are 6c.
