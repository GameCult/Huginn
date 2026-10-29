# VoidBot's heartbeat: in-house prior art for the swarm heartbeat

Read 2026-10-29 for ruling `eureka-body:ruling:heartbeat-routes-persona-turns`,
from VoidBot `46d891b` (code and notes only; no Discord archive was read).
Sources: `apps/persona-scheduler/src/{persona-scheduler-runner,initiative-engine,
turn-routing,semantic-pressure-source,discord-activity-source}.ts`,
`notes/voidbot-current-system-map.md` (lines 172-189),
`notes/agent-swarm-shutdown-audit-2026-05-21.md`.

## What it was

One resident process (`apps/persona-scheduler/src/index.ts`) serialised calls to
one tick function, `runPersonaSchedulerTick`, for every Persona (repo Face). A
tick, in order:

1. **Brake first.** A swarm pause file and a `repoFaceHeartbeats.enabled` flag
   are read before the registry, Discord, embeddings or any model; a paused tick
   writes a skip record and returns.
2. **Reads the chat with one token.** The single Discord bot token reads every
   channel any Persona may speak in (`readDiscordActivitySnapshot`), giving the
   recent human messages and an idle-cooling reading of room activity.
   Mentions, role pings and replies to a Persona were queued separately by the
   bot process into a typed pending-attention inbox.
3. **Who gets a turn: arithmetic, not a model.** Each participant has
   `nextTurnAt` on a wall-clock-advanced initiative clock; a turn pushes it back
   by `recoveryMinutes`, scaled by speed and `heat` (per-Face overrides times a
   swarm `globalHeat` dial). `selectReadyParticipants` takes the ready ones
   (`nextTurnAt <= clock`, not busy, not napping), sorts pending mentions first,
   then lowest `nextTurnAt`, and admits unprompted turns only with "meaningful
   pressure"; at most `maxJobsPerTick` per tick.
4. **Relevance: embeddings, not a model.** `readSemanticPressure` embeds each
   Face's affinity card and scores recent human messages against it (cosine,
   against a per-Face `interruptThreshold`) to raise pressure.
5. **The cheap model sat after the turn, not before it.** The child turn ran on
   the full model; a cheaper "Interpreter" model read the child's prose and
   decided route/retry/drop and translated it into typed state and speech.

Last-turn time per agent was `lastTurnAt` / `activeTurnStartedAt` in one scheduler
state file, with `initiative-engine.ts` its only writer.

## What it did not do

- **No spend accounting.** Nothing in the scheduler reads cost, tokens or money.
  "Burn" was managed only indirectly: heat, recovery minutes, `maxJobsPerTick`
  and the pause file. Its total spend had no bound.
- **No model chose who wakes.** The operator's recollection ("a cheap classifier
  turn can wake agents") matches the shape (one router, a cheap model in the
  loop), not the mechanism. Adopting a classifier for routing is a new design
  decision, not a port.

## Failure modes it recorded

- **Stale busy bookkeeping.** The state file kept an `activeJobId` for a Face
  whose job no longer existed (audit, line 15). A second record of liveness
  beside the job queue decayed; a stale-turn recovery pass was later bolted on.
- **The scheduler's frame leaked into speech.** Prompts named the runtime a
  "heartbeat" repeatedly; Faces posted "Heartbeat: no fresh request..." and a
  regex sanitiser chased the prefixes (audit, lines 77-86). The routing
  decision's vocabulary must never reach the Persona turn.
- **Timer-driven turns produced status reports.** Unprompted turns fed mostly
  work context sounded like work reports and repeated saturated topics; state
  knew the topic was saturated (`saturation 0.71`, `novelty 0.18`) but nothing
  used that to cool it (audit, lines 126-146).
- **Cross-jurisdiction asks** escaped from heartbeat turns (audit, line 118).

## What carries over

- One process, brake read before anything else, at most k turns per tick.
- Liveness from one owner. Here that is the mind (run-live-admission), so the
  heartbeat keeps no busy flag.
- Mentions and operator words take precedence over neglect.
- The router's frame stays out of the Persona prompt: a Persona is told why it
  woke only as typed context (deliveries, idle duration), never "heartbeat".
- New relative to VoidBot: spend is measured (connector ledger, plan quota) and
  bounded by one swarm dial (ruling `swarm-dial-one-budget`), and the choice
  within that bound is a typed classifier turn.
