# Live Mind sessions: architecture and cut sequence (2026-10-31)

This is Imagination's map for ruling `eureka-body:ruling:live-mind-sessions` (Operator, 2026-10-31, answers `mind-sessions-talk-and-work` with `two-sessions-one-memory`):

> "Persona Self is supposed to be a persistent session that compacts and hands off as appropriate, same thing with the Persona that drives it. I'm not getting that feeling with the swarm, here. They should be working side-by-side on their projects, exchanging notes. There should always be at least one active session, with new messages batching and dropping into their contexts between tool calls. Why is that not the case?"

It revises the cut order of `2026-10-31-persistent-mind-session-map.md` (d77bbdf, cited as MAP) so that live sessions come first and memory-write runs beside them, not ahead of them. The facts come from `2026-10-31-persistent-mind-session-prior-art.md` (8dfcbcd, cited as PA). Huginn was read at main c33884f, `gamecult-ops` at 6ba0482, the live units on Yggdrasil read-only (`systemctl cat`, 2026-10-31), and the Claude Code docs `hooks` and `headless` were fetched 2026-10-31. Inferences are marked as such. Open forks are questions in the eureka mind.

## 0. Why it is not the case today

These are four facts about c33884f, and each one is a one-shot owner.

1. **A Self run dies at its first answer.** `self_run/decide.rs:127`, `Event::Result => CloseStdin`. The `claude` child runs one turn, then the actuator closes its stdin and it exits. The CLI does not require this: streaming input is documented as "a long lived process" with queued messages (PA B1).
2. **A brief to a working Self is dropped.** `persona.rs:752-786` `start_self_run` admits the brief as a follow-up. `open_and_launch` then answers `Busy { run }` because a Self run is live, and the Persona takes that as success. The live child never hears of the follow-up, and no later run claims it, because only the start message carries claims (`self_run.rs:124` `task_section`).
3. **A Persona turn is one schema call per beat.** `persona.rs` makes one tool-less connector call per turn and takes at most `MAX_TURNS_PER_START = 4` turns per unit start. The heartbeat (`heartbeat.rs`) opens one `PersonaTurn` run per beat through `open_and_launch`, and `mind-persona@` has `RuntimeMaxSec=30min`. No conversation survives a turn (PA A1, A5).
4. **Nothing keeps a session alive.** The heartbeat's only verb is "start a one-shot unit". When nobody is mentioned and the beat has not fallen, the swarm has zero processes.

## 1. The architecture

### 1.1 Who owns what

| Session | Process and owner | Lifetime | Reads | Acts |
|---|---|---|---|---|
| **Self session** (one per Mind) | `claude` child of `mind-self-run` in `mind-self@<i>` (user `mind-<i>-self`). The actuator (`carry` + `decide`) owns its lifetime and everything that enters it. | One process per run. A run spans many briefs. The session spans processes by resume (cut 4). | Her mind; briefs and tells in her Persona's words; never raw chat | Full Self toolset: forge push, admission, sandbox `/var/lib/mind-<i>-self` |
| **Persona session** (one per conversation: public rooms together, or one DM room; MAP 2.1) | `claude` child of `mind-persona` in `mind-persona@<i>` (user `mind-<i>`), leased like the Self (question `persona-session-owner`). The Persona actuator owns lifetime and input. | Long-lived. It rotates under the unit cap. | Chat as framed data lines, her mind, read tools in bwrap | Only the decoded `PersonaReply` fields, gated by the organ (say, start_self_run, remember, settle, vote) |
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

`persona-turns-read-only-tools` says the worst a hostile chat message can make a turn do is read. The two-session split keeps that true. Chat never enters a session that holds push and admit.

- **The Persona session reads chat** and can act only through decoded reply fields the organ gates. Its tools are read tools and her sandbox git under bwrap (`persona-git-personal-sandbox`). A long session keeps a planted instruction in context for longer (MAP 2.8). The bound is the same, because the reply is still the only act.
- **The Self session never reads chat.** Its inbox is follow-ups her Persona admitted (one brief of at most 1000 chars per `start_self_run`) and, later, memories. The Persona is the laundering point, as today. What changes is reach in time: a brief can now land in a session mid-task. The worst a hostile message can do is talk her Persona into writing a hostile brief, which today's one-shot Self also accepts. Nothing new reaches the Self.
- **Hook injection is data.** The hook prints only spool entries the actuator wrote from the mind, framed as factual statements naming their source document (the docs warn that "out-of-band system commands" framing trips injection defences). In a subagent (hook input carries an agent id) it is a no-op, so tells reach only the main thread.
- **Refresh purges** anything that never reached memory. Memory poisoning is the durable vector, and each memory carries its sources and is withdrawable (MAP 2.8).

### 1.9 Evidence from the Cinny fork, and what it binds (Self, 2026-10-31 13:4xZ)

The operator: "Aqua was gonna work on the notification sounds and ambient soundscape for it, Libby was working on the math and shaders for the background, Void was putting it all together. They were coordinating. And of course they just forgot all about it as soon as it rolled off the transcript." Self read the minds of void, libby and aqua. None holds a ruling, and nobody's mind holds the shared plan. In-force `chat-*` follow-ups never close: void 24, libby 23, aqua 22. Libby holds about 10 near-identical "CultMath noise survey" briefs (`chat-s239` to `chat-s318`), one asked per turn because she could not see the earlier answer. Real results exist as follow-ups (`cultmath-ts-noise-absent`, `cinny-estate-navigator-footprint`) and as AquaSynth PR #1.

This binds two things.

- **A brief is closed by its result.** An open self-run follow-up must mean open work, or the goals section (goals-in-turn) is noise and cut 1's inbox would re-tell every stale brief. Owner: the Self actuator. At each `result`, every brief told to the session before or during that turn that her Self has not resolved itself is resolved `Recorded` by the actuator, with the result text (cut on a character boundary at 3800 B, as f2e3805) as the rationale. A longer piece of work she takes on is a follow-up of her own, with a different owner. At session start the same rule is applied to the past: a self-run follow-up claimed by a closed run is resolved `Recorded`, citing that run, whose closure rationale already holds the result. Her Persona then sees results where the brief was, not a pile of open asks. In cut `self-session-live` r2.
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
| 6 | `persona-session` (merges MAP's `persona-read-tools` r2 and `persona-session`): `mind-persona` holds a leased long-lived claude per conversation, read tools in bwrap, constant schema, chat spooled mid-turn, rotation; unit cap 4 h | Huginn + gamecult-ops | `persona.rs` one connector `TurnRequest` per turn, `MAX_TURNS_PER_START`, the KitConnector transport for Persona turns | after 2 |
| 7 | `session-keeper`: the heartbeat keeps the floor, writes wakes into live sessions, `live_sessions` vote | Huginn | `heartbeat.rs` `launch()` opening a `PersonaTurn` run per beat | after 6 |
| 8 | Memory maintenance, ruling home, Cut 12 hand-off (MAP 4, items 2, 6, 7) | Huginn | none | any time |
| A1 | `atlas-multi-writer`, one contained cut: (1) `huginn@atlas` accepts batches from any roster Mind; (2) each document is rooted at, and records, its publisher, and only the publisher amends or resolves it (`atlas-is-shared-mind-open-evidence`); (3) the publisher is proven, not declared, by a per-Mind key placed by the unit installer the way the forge token is (readable by `mind-<m>` and `mind-<m>-self` only), with an HMAC over the batch that the daemon checks against the declared instance's key. The bound: proof is one key per Mind and one check at admission, the same on every daemon, so it also closes `mind-admission-publisher-unproven`. No roles, no ACL language, no rotation protocol beyond re-install. Deploy is through Idunn and gamecult-ops. | Huginn + gamecult-ops + Idunn | "a store is canonical to one instance" on Atlas; declared-instance admission on every daemon | 1, 2 (rebase onto r3's admission tables) |
| A2 | `atlas-collab-recall`: collaboration kinds in Atlas (plan, part held by a Mind, agreement, open item, shared idea); every participating Mind's Persona turn (and Self start) carries a "Shared work you are in" section read from Atlas; a Persona reply can publish or amend her own Atlas items; r3's notes on others' parts are demoted to her own commitment citing the Atlas plan | Huginn | others' parts recorded in her own memory (r3 interim) | after A1, 2 |

Cut 1 alone gives "a session with messages dropping in" for the Self. Cuts 6 and 7 give "at least one active session swarm-wide". A1 and A2 give "Atlas records ongoing collaborative work".

## 3. Persistent kinds touched

| Kind | Identity | Owner and writers | Truth or cache |
|---|---|---|---|
| follow-up, owner `self-run` (existing) | `<campaign>:follow_up:chat-<batch>` | her Persona's organ admits; her Self resolves | truth: her Self's inbox |
| spool entry (new, organ state) | `$RUNTIME_DIRECTORY/inbox/<seq>-<id>.json`, claimed into `inbox/told/` | Self actuator writes; claim by rename from hook or actuator | cache: lost with the unit, rebuilt from the mind |
| told set (new, process memory) | follow-up ids told to this process | Self actuator | cache: a new process re-tells open follow-ups, which is correct because they are still open |
| session transcript (cut 4) | `/var/lib/mind-<i>-self/sessions/<uuid>.jsonl` | claude writes; the actuator copies it out | cache: never read as memory |
