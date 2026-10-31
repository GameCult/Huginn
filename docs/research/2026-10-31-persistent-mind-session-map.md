# Persistent Mind session: the map (addendum, 2026-10-31)

Imagination's map over `2026-10-31-persistent-mind-session-prior-art.md` (8dfcbcd, cited as PA with its part numbers) and `2026-10-31-persona-memory-and-voice-prior-art.md` (bfdf4db, cited as MV). Rulings: `mind-self-is-a-persistent-session`, `persona-is-append-only-session`, `persona-memory-written-not-transcript`, `self-lifecycle-flush-rehydrate`, `persona-turns-read-only-tools` (in part superseded by `persona-git-personal-sandbox`), `campaign-history-moves-to-steward`. Huginn read at main e022e53 plus `eureka-body/persona-goals-in-turn` 90bd201 (treated as landed). This is a design addendum: inferences are marked as inferences; the open forks are questions in the eureka mind, not decisions here.

## 1. The failure this answers

The operator asked every Mind to prioritise the Cinny fork. Void later did not know the project or that he stewards it. The ask lived only in room messages (20 context lines per delivery, PA A4); no turn wrote memory (PA A1: the organ admits only follow-ups, run closures and Pulses); the turn never read his stewardships or follow-ups (fixed by goals-in-turn 90bd201); his Self run results were cut to 800 bytes (fixed by 743cbb8). Her rulings about the client sit in Eureka's mind, which Void's turns never read (`campaign-history-bridge-wired` is open).

A persistent session alone would not have saved him: the ask would have scrolled out at the first refresh. The cure has two halves, in this order:

1. **Memory is the truth, the session is a cache.** Every Persona turn and every Self run writes what passed through it into her own mind; every refresh rebuilds from the mind. That makes any refresh lossless, which is what lets refresh be a pure cost decision.
2. **Sessions keep the cache warm** between refreshes, so turns stop paying for a rebuilt prompt.

## 2. The design

### 2.1 Sessions per Mind

| Session | Holds | Tools | Reads chat | Count |
|---|---|---|---|---|
| Persona conversation session | one conversation: the public rooms together, or one DM room | read tools and her sandbox git, under bwrap (`persona-git-personal-sandbox`) | yes, as framed data lines | one for public rooms, one per DM room in use |
| Self session | her work: claims, briefs, tells, results | full Self toolset, lease, budget | never raw chat; only briefs and asks in her own words, and memories | one (`one-live-self-run`) |

Two kinds of session, one identity and one memory. Reasons, in the order they weigh:

- **Injection boundary.** `persona-turns-read-only-tools` (in force in part): "The worst a hostile chat message can make a turn do is read." A session that reads member chat and holds forge push and admission breaks that sentence. The Self is reached only through text she wrote (the brief, the ask she recorded), the laundering point that exists today.
- **Cache.** Two prompts with different system prompts and tools share no conversation cache anyway (PA B7, cache scope); merging them would make every chat turn pay for the Self toolset and doctrine, and every Self request carry chat.
- **Responsiveness.** A Self task runs for minutes to an hour. A single session would hold her chat until the task's turn ended (stream-json queues messages, PA B1). The operator's own session stays responsive the same way: Self talks, background workers do the work.

The operator's ruling asks that the Self "communicates with me about the stuff I asked you to work on". In this design the Persona is that voice and the Self is her worker; the person who asked talks to one Mind. Whether the Self should also speak to the asker directly is question `mind-sessions-talk-and-work`.

Per-conversation Persona sessions follow `persona-memory-written-not-transcript`: one session spanning a DM and a public room would carry the DM's transcript into the public turn, which is the transcript standing in for memory. Letta keeps one agent, many conversations, shared memory (PA B3); `next_batch` already splits DMs from public rooms (PA A1). This is question `persona-session-per-conversation` because `persona-is-append-only-session` speaks of "a" session.

### 2.2 How a message reaches her, and how she replies

Unchanged triggers: Bifrost delivery, the heartbeat's beat, an operator mention, her Self run ending (PA A3). The Persona start resumes the batch's conversation session and appends one user message: what is new since that session's last turn (new deliveries, the burn and tempo line, goal and memory lines not already in the session). The reply is still one `PersonaReply` decoded by the organ; the organ still gates, sends, admits and acks. Tools in the session only read.

Inference to probe before the session cut: `--json-schema` is carried as the CLI's StructuredOutput tool (connector `backend.rs:269-272`), and a tool-definition change invalidates the whole cache (PA B1). Today the schema's room and campaign enums vary per batch (`reply_rooms`, `reply_schema`), which would break the prefix every turn. The session cut therefore makes the schema constant per session (room and campaign as strings with the enums checked in `decode_reply`, as it already re-checks rooms).

### 2.3 How work and talk share

- Persona to Self: `start_self_run` opens a Self run, and when her Self session is live it appends the brief to that session as a queued user message (a tell) instead of answering Busy.
- Self to Persona: the run's result rides its closure rationale (f2e3805) and her Self run writes memory (`did`, `decided`, `learned`); her next Persona turn sees both (recent runs, recall). The heartbeat already launches her Persona turn when her Self run ends.
- Both read the same mind: memories, asks, follow-ups, stewardships, rulings.

### 2.4 Memory versus session context

| Lives in memory (her mind, typed) | Lives only in session context |
|---|---|
| what she was asked (`asked`), by whom, where | the exact wording of the exchange |
| what was decided and by whom (`decided`) | the turn-by-turn back-and-forth |
| what she committed to (`committed`) | tool output she read this session |
| what she learned (`learned`) | burn and tempo lines |
| what she did (`did`, mostly from Self runs) | recall lines already shown |
| goals: follow-ups, and open asks | |

An ask is open while its memory is in force. Taking it on resolves the memory `Deferred` to the follow-up that carries it, exactly as a finding is deferred to a follow-up today (`admission.rs` matrix). Doing or declining it in conversation resolves it `Recorded` or `Withdrawn`. So the goals she sees are derived from documents, and nothing scans prose.

### 2.5 Refresh: when, on what data

Refresh = end the session, start a new one whose first message is rebuilt from the mind (system prompt, goals, open asks, recall, recent runs, the conversation's own delivery context). No summarising compaction (`self-lifecycle-flush-rehydrate` clause 1); claude's auto-compaction is set above the organ's ceiling so the organ always decides first. Because every turn already wrote memory, no flush turn is needed before a Persona refresh; a Self session keeps the ruled flush.

One pure decider per organ, like `self_run/decide.rs`, over measured data recorded per turn (prefix tokens from usage, rebuild size R from the last refresh's first request, cache state from `cache_creation.ephemeral_1h/5m` and the time since the last turn):

1. The system prompt's inputs changed (doctrine hash, persona state `updatedAt`, campaigns in force): refresh. Resumed conversations keep their first system prompt (PA B1), so a changed persona state never reaches a resumed session.
2. Cold cache (no request within the TTL; 1 h for `-p` on the subscription, PA B1) and prefix P > R: refresh. A cold resume rewrites P at the write price; a refresh rewrites R.
3. Rental rule (clause 3): refresh when the read cost above R accumulated since the last refresh reaches one rebuild. With Opus 5.5 or Sonnet 5.5 (read 0.05x, 1 h write 2x, PA B1): refresh when sum over turns of (P_t - R) >= 40 R. At R = 30k and growth of 2k per turn this is about 35 turns, so a busy public session refreshes roughly every 35 turns and a quiet DM rarely.
4. Quality ceiling: P above a per-model ceiling (Self measured ~300k; Persona to be measured, starting lower because chat grows by many small turns and multi-turn degradation is documented, PA B6).
5. Self only: task boundary (no claim left and nothing queued) ends the run; the next run resumes the session if warm (clause 2), else rehydrates.

Each decision is recorded with its inputs on the turn trace and the run's closure, so the constants are measured (clause 4).

### 2.6 How goals survive a refresh

The rebuilt first message carries: stewardships and open follow-ups with the rulings they cite (goals-in-turn), open asks (always, not by relevance), the memory recall lanes (about the senders, relevant, newest), and recent runs with their results. A refresh loses only wording.

### 2.7 Concurrency across rooms

One live Persona run per instance and one beat per Mind (PA A3) serialise her turns; each turn touches one conversation session. Two DMs waiting are two turns. One live Self run (`one-live-self-run`), one Self session. No session is resumed by two processes, so no transcript interleaves (PA B1).

### 2.8 Prompt-injection exposure

A long session keeps a planted instruction in context for many turns, where a fresh turn forgot it. The bounds:

- The Persona session can only read and work in her sandbox; every act with effect is a field of the decoded reply, gated by the organ.
- The Self session never reads chat; only her briefs and asks reach it.
- Memory is the one carrier across refresh, so memory poisoning is the durable vector (the MINJA memory-injection attack, arXiv 2503.03704, is the prior art; not fetched in this pass). Each memory carries the event ids it came from, her own instance only, and is resolvable, so a poisoned memory is traceable and withdrawable. Recall lines render quoted as data, like messages.
- A refresh purges anything that never reached memory.

### 2.9 Rulings reaching the steward's mind

Her words about Cinny are rulings in Eureka's mind; Void's turns read only Void's mind. `campaign-history-moves-to-steward` decides where history ends up (the steward's mind, by Atlas hand-off), not who admits a new ruling about a stewarded domain today. That is question `ruling-home-for-stewarded-campaigns`.

## 3. Persistent kinds

| Kind | Identity | Lifecycle | Owner and writers | Readers | Derived or truth |
|---|---|---|---|---|---|
| `memory` (`epiphany.pipeline.memory.v2`, her mind) | `<instance>:memory:<label>`; label `<run label>-t<turn>-m<i>` from a Persona turn, `<run label>-m<i>` from a Self run | admitted once, never edited; closed by a resolution: `Deferred` to a follow-up (ask taken on), `Recorded` or `Withdrawn` (done, declined, wrong), `Superseded` by a newer memory | her mind's admission; written only by her Persona turn's reply (organ) and her Self runs; foreign instance refused | her recall, her Self runs | truth |
| conversation session record (organ state, `persona-sessions.cc` in her StateDirectory) | (instance, conversation: `public` or DM room id, generation); claude session id = UUID v5 of those | opened at a generation's first turn, appended per turn, ended by the decider | mind-persona only | mind-persona | cache: losing it costs one refresh |
| Self session record (organ state in mind-self's StateDirectory) | (instance, generation) | spans Self runs while warm; ended by the decider or a flush | mind-self only | mind-self | cache |
| claude transcripts (JSONL under the session store) | the claude session id | written by claude; deleted when their generation ends | claude, as her uid | claude on resume | cache: never read as memory |

Nothing about sessions enters the mind except the measurements on traces and run closures: a cache must not pretend to be truth.

## 4. Cut order

1. `persona-memory-write` r2 (spec in the mind): memory kind with `sort`, write and recall on today's turn shape, open asks, ask-to-follow-up, Self-run capture.
2. Ruling home: per question `ruling-home-for-stewarded-campaigns`.
3. `persona-read-tools` r2: the Persona turn is her own leased claude with read tools, schema constant per session, session persistence on.
4. `persona-session`: per-conversation resumed sessions, append-only turn messages, the refresh decider.
5. `self-session`: Self run keeps stdin open across claims and tells, resumes when warm, transcripts outside the fresh config.
6. Memory maintenance: consolidation and promotion into persona state (`consolidates`, MV 4.3).
7. Huginn Cut 12 hand-off import through Atlas, replacing any bridge.
