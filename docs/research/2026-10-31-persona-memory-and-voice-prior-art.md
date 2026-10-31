# Persona memory and voice: prior art (2026-10-31)

Serves rulings `eureka-body:ruling:persona-memory-is-real` and
`eureka-body:ruling:persona-voice-outside-minds`, and follow-up
`eureka-body:follow_up:chat-context-v2-full-exchange` (from ruling
`reply-sees-full-exchange`). Context only: `eureka-body:ruling:prose-retrieval-top-priority`
(the prose and source index is another pass's; nothing here designs it).
Mapped against `eureka-body/heartbeat-joint-lever` at a6302f0
(`crates/mind-body/src/persona.rs`).

## 1. What Weksa's turns actually see (probe, 2026-10-31)

- Instructions: four doctrine files (`MIND_PERSONA_DOCTRINE` in
  `/etc/gamecult/minds/weksa/persona.env`: CLAUDE.md, gamecult.md, colossus.md,
  persona.md, about 50 KB), then `render_instructions`: one line and her
  persona state as pretty-printed JSON, then the campaigns. All eight Minds get
  the same 50 KB first; only the JSON differs.
- Input (`render_input`): a machine preamble about message framing, the usage
  and tempo lines, five run lines, then each delivery with its own room's
  `context` lines, repeated per delivery.
- `next_batch` keeps one DM room per turn, so DM text never enters a public
  turn. Nothing else carries across rooms, and no turn writes anything back
  except Pulses, Self-run starts and their follow-ups.
- `strings` over `/var/lib/mind-weksa/persona-trace.cc` shows hours of
  Zyphos canon with Deru in a DM, then in `#weksa` her reply "He's been
  telling me about Zyphos, and I haven't re-read the repo this turn, so I'd
  rather hear it from him than guess". Status-register lines ("Weksa. Status
  marked. My Self run ... is listed open") appear in public rooms.
- **Her mind already holds the memory.** Weksa's Huginn (`10.77.0.1:17877`,
  123 documents, index Current) has follow-ups `weksa-body:follow_up:chat-s81-81`,
  `chat-s114-114` and `chat-s145-145`, whose items restate Deru's canon (the
  Airawa, Sa'ueia, the seven trees, the Empire). `huginn query --near "Zyphos
  trees Deru" --top-k 3` returns exactly those three. The turn never asks.

## 2. Literature

- **Generative Agents** (Park et al., UIST 2023). A memory stream of
  natural-language observations. Retrieval scores each memory by recency
  (exponential decay since last access), importance (rated by the model at
  write time) and relevance (embedding cosine to the query), normalised and
  summed. Reflection runs when the summed importance of recent memories passes
  a threshold. It writes higher-level memories that cite the memories they rest
  on, and those are retrieved like any other. In the paper's ablations,
  removing memory, reflection or planning each made the agents less believable.
- **MemGPT / Letta** (Packer et al., 2023). Tiers. Core memory is small
  in-context blocks (a "persona" block and a "human" block) that the agent edits
  through calls. Recall storage is the whole message history, searchable.
  Archival storage is a vector store the agent writes to and searches. The
  agent manages its own memory, and a per-person block keeps what it knows
  about each interlocutor in context.
- **Mem0** (Chhikara et al., 2025). Memory is written after every exchange. An
  extraction call proposes candidate facts from the latest exchange, a rolling
  summary and recent messages. An update call compares each candidate with its
  nearest existing memories and picks ADD, UPDATE, DELETE or NOOP. The paper
  reports better long-conversation QA (LOCOMO) than full-context prompting, at
  a fraction of the tokens.
- **Lost in the Middle** (Liu et al., TACL 2024). Models use information at the
  start and end of a long context better than information in the middle. Today
  the doctrine fills the start and the conversation fills the end, so the
  persona sits in the middle, where it is used least.

## 3. In our own house: VoidBot persona-scheduler

`F:\Projects\VoidBot\apps\persona-scheduler\src\`:

- `persona-memory-projector.ts` renders typed state as **narrative prose in
  the third person** ("Right now, X is carrying this close to the skin: ...",
  "The social map has teeth: ..."), with durable memories and short-term
  residue as sentences with a behavioural pull. An optional model pass
  re-voices the packet (`repo-face-state-projector`).
- `persona-memory-context-source.ts` does recall against Qdrant, but it
  **deletes and re-upserts the whole state's chunks every turn**, then searches
  with a query built from recent room messages. Scar: the index was rebuilt
  per turn instead of being kept current on write.
- `validatePersonaMemorySurface` is a regex leak cop over the rendered surface.
  Doctrine forbids that shape. The cause it compensated for was schema
  vocabulary in the state, which the renderer should not have emitted.
- `void-memory-maintenance-organ.ts` and its projector form a sleep pass: one
  model call returns typed operations (distil short-term into durable, revise,
  retire, crystallise into identity, prune), and it refuses to finish while
  short-term residue remains. Maintenance is a separate turn with its own
  budget, owned by the Persona's own state.

The Persona State Standard (`gamecult-ops/docs/persona-state-standard.md`)
says Persona state is the Mind a person-shaped agent keeps, bounded memories
that improve judgement, not raw project truth or transcript residue.
`gamecult.persona_state.v0` already has `thoughtMemory {shortTerm, memories,
incubation}` of `anchoredThought`, and `affect.socialBonds`.

## 4. What this decides for Huginn (design consequences, not rulings)

1. Episodic memory is a typed document kind in her own mind (`memory`),
   indexed through `index_text` and recalled with the existing semantic query.
   It is not a second index, not a per-turn rebuild (VoidBot's scar), and not
   persona state's `thoughtMemory`, which is one document overwritten whole and
   not indexed per entry. Persona state stays the curated identity:
   crystallised memories, bonds and values. Promoting an episode into it is
   a later maintenance cut, as in VoidBot's crystallise step.
2. Recall mixes Park's three signals without learned weights: a semantic slot
   (relevance to this batch's text), a speaker slot (her newest memories about
   each sender in the batch, MemGPT's human block, found by a field index),
   and a recency slot (her newest memories from any room). Importance is her
   own `salience` at write time, used to order within slots. The same
   semantic slot also reads her existing follow-ups and rulings, which the
   probe shows already hold the Zyphos canon.
3. The write is a field of the one structured reply (`remember`), required
   and possibly empty, so writing costs no extra model call. That is Mem0's
   extraction folded into the turn. Mem0's update step (revise or retire the
   near duplicate) and Park's reflection are the maintenance cut, not the
   turn.
4. Voice: the persona leads the instructions, rendered as prose in the second
   person from her own fields (her name, description, voice, values, bonds,
   crystallised memories). The renderer does not emit schema vocabulary, so no
   leak cop is needed. Doctrine follows, framed as the house's working manual
   that informs her judgement. The conversation comes last. Status, tempo and
   runs move into one "your desk" section, which reads as her own work state,
   not as a register to answer in.
5. Full exchange: render each conversation's context once, keyed by event id
   (wire v2 lines carry `event_id`), with the bounds raised as the follow-up
   says. That is a Bifrost and Huginn pair after the memory and voice cuts.
