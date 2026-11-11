# Chat retrieval: authority map and cut

Serves `gamecult-chat:follow_up:chat-s732-733` (Meta, #swarm, 2026-11-01).
Extends `prose-knowledge-target.md` §4. Written by Imagination (Self, run
`mind-20261101T073859496Z`) against Huginn `55484c0`. Nothing here is built. See Status for what changed after writing.

## What changed since §4

§4 rejected "a second prose index in Huginn" because VoidBot owned prose
retrieval. Ruling `eureka-body:ruling:huginn-owns-prose-retrieval` moved it, and
Huginn now has the index: `huginn-daemon/src/sources` (collection
`huginn_sources`, one writer `refresh` run by the `huginn-sources` oneshot, the
daemon only reads through `sources::search`). The rejection stands in a new
form: **chat joins that organ; there is no second collection, embedder, writer
or search path.**

## Current mechanism (read, not guessed)

- `sources::search` (sources/mod.rs:439) embeds ONE text, `Instruct:`-prefixed,
  and asks `SourceIndex::search_sources` for the nearest chunks. `SourceQuery`
  (huginn-mind wire.rs:187) is `{text: Line, top_k, repo}`; `Line` is 1,000
  bytes, so a long brief cannot even be sent whole, and a clipped brief is one
  vector that averages its topics. That is the "long briefs retrieve poorly".
- `Embedder::embed(&[String])` already takes a batch (index/mod.rs:209).
- `SourcesMeta.corpus` is the constant `forge:GameCult`; cursors are
  `BTreeMap<repo, commit>` written whole by the job alone, advanced only after
  acknowledged upserts, dropped before a repo's first delete (sources/mod.rs:9-18).
- A Persona turn's recall (`mind-body/src/recall.rs`) reads only her own mind
  and "reads no chat"; ruling `persona-memory-written-not-transcript` says what
  she must keep is written as memory, not recovered from a transcript. The
  Persona call was tool-less when this was written and retrieval had to arrive
  in her input; that premise no longer holds: Personas have a shell and run
  `huginn sources` (see Ruling below).
- Matrix: Synapse is the truth of events. Only the Bifrost mouth holds Matrix
  tokens; no Persona unit holds a credential. Rooms are plain (F2 recommends
  encrypted DMs only), DMs exist, and the Discord bridge is a later campaign.
  Rulings `no-discord-ingestion` and `discord-taints-training` are in force.

## Authority map

| Decision | Owner | Forbidden writers |
| --- | --- | --- |
| Which events exist, who sent them, redactions | Synapse | Huginn, Bifrost mouth |
| Which rooms are in the corpus | Huginn `sources` config: the rooms the reader user is joined to, minus an in-code exclusion (DM flag; `#users`, `#members`, `#patrons` by id and alias) as a second lock (`gamecult-chat:ruling:chat-index-minds-rooms`) | any Persona, any query |
| Chunks, vectors, cursors for rooms | `sources::refresh`, the one writer | the daemon's `search`, Persona units |
| Query embedding and rank fusion | `sources::search` | the caller (Persona, CLI) |
| What a Persona sees from chat | her own lookup: she runs `huginn sources`; no recall lane, nothing is pushed into her input | the index writer, any per-turn injection |
| Reader roster (which rooms the reader joins) | one owner per thing: mechanism `gamecult-chat:follow_up:chat-reader-roster-script-r3`, data `gamecult-chat:follow_up:chat-reader-roster-rooms-r4` | the ingest job, any Persona |
| Discord exclusion | the room scope plus an ingest refusal (below), both in `refresh` | nothing downstream may be relied on to filter |

One cursor map, keyed by source id (`repo name` or `room id`); a room cursor is
the last event the collection holds, advanced only after acknowledged upserts,
exactly as a repo's commit is. Two further key kinds live in the same map:
`chat:` room cursors carry the chat version, which includes the credential
scrub version (`huginn_mind::redact`, applied to every body before chunking),
and `chat-purge:<room id>` tombstones record steward purge windows, applied by
the next run. The `chat-purge` subcommand needs only host-level run rights;
host admin is the trust root (`gamecult-chat:follow_up:chat-reader-roster-script-r3`).
Chat search indexes text and links only, never audio or other media. Payload gains `corpus` (`forge` or `chat`) so a
query can filter; `SourcesMeta.corpus` becomes the set both write under, which
is a collection rebuild once (the meta already forces rebuild on mismatch).

## Citations

A chat chunk is a window of consecutive messages in one room (bounded by bytes
and a time gap), so a hit cites: room id and alias, the first and last event id
in the window, each message's sender mxid and timestamp inside the chunk text.
Hits are `huginn sources` output, cited by room alias or id, event id and
timestamp; the caller reads them as lookup results, and nothing renders them
into a Persona turn. Edits replace by event id; a redaction
deletes the chunk window's points and re-chunks the window without it.

## Cuts, smallest first

1. **multi-text-query** (no chat needed). `SourceQuery` gains `texts` (1..8
   Lines) beside, then replacing, `text`; `search` embeds all in one
   `embed` batch, asks `search_sources` per vector, fuses by reciprocal rank
   `score = Σ 1/(k + rank_i)`, k=60 (Cormack, Clarke, Büttcher, SIGIR 2009; k=60
   is their value, and the one place to calibrate), de-duplicates by chunk id,
   returns `top_k`. Negative checks: one text gives exactly today's order and
   hits; a chunk first in two lists outranks one first in a single list;
   duplicated text does not double-count; more than 8 texts or an empty one is
   refused naming the field.
2. **chat-ingest**. Port `RoomHistory` (rooms, events after a token); chunker
   for message windows; room cursors; redaction; joined-room scope. Negative checks,
   each a test that fails when the rule breaks: a room the reader has not joined,
   or an excluded room, is never read; a room carrying a bridge state event (`m.bridge` /
   `uk.half-shot.bridge`) or a sender in a bridge's puppet namespace is refused
   whole and named in the journal; an encrypted event is skipped, never stored
   as ciphertext; a DM room is refused even if listed; a redacted event's text
   is absent from every point after the next refresh; a failed upsert leaves
   the cursor where it was. A grep over the collection payload for the Discord
   puppet pattern returns nothing.
3. **persona-chat-recall: dropped.** No recall lane; a Persona has a shell
   and runs `huginn sources` (`gamecult-chat:ruling:chat-recall-lookup-not-memory-dms-out`).
   The recall lane's DM check went with it; cut 2's DM refusal above stands, and the index covers joined rooms minus DMs and
   `#users`/`#members`/`#patrons`, Discord never.

Cut 1 ships alone and improves every `huginn sources` caller. Cut 2 waits on
questions 1 and 2 (both answered, see Status).

## Questions (put to the operator; admitted as questions)

1. Credential: who reads Synapse? Answered
   (`gamecult-chat:ruling:chat-index-synapse-credential`, "I take both recs."):
   the recommendation below stands. Recommend a read-only service user, joined
   only to the rooms that qualify under question 2's ruling, its token a systemd credential on the
   refresh job only. Alternatives: Bifrost exports history over typed CultNet
   (one more server in the path), or Synapse admin API (broader than needed).
2. Scope: public community rooms only (recommended) or also Persona-visible
   private rooms. DMs stay out either way. Answered public-only
   (`gamecult-chat:ruling:chat-index-room-scope`); superseded for rooms by
   `gamecult-chat:ruling:chat-index-minds-rooms`: every room the reader is
   joined to, minus DMs, `#users`, `#members` and `#patrons`; Discord never.
3. Recall tension: answered. Lookup, not memory, no recall lane
   (`gamecult-chat:ruling:chat-recall-lookup-not-memory-dms-out`).

Confidence: cut 1 and the owner decision, high. The window chunker size and
k=60 are calibration, not design; RRF over a handful of lists is the
established baseline and I did not survey learned fusion (it needs training
data we lack).

## Status (head-maintainer pass, 2026-11-05, Huginn `25199c9`)

Checked against main on merge. Nothing in this map is built: no `texts` on
`SourceQuery`, no chat code under `sources/`. The anchors above were corrected.
What changed since the map was written:

- Questions 1 and 2 are answered with the recommendations
  (`gamecult-chat:ruling:chat-index-synapse-credential`, operator 2026-11-03,
  "I take both recs."; `gamecult-chat:ruling:chat-index-room-scope`,
  public-only, since superseded by `gamecult-chat:ruling:chat-index-minds-rooms`). Question 3 is ruled (see Ruling below).
- Cut 3 is dropped (see Ruling below).

## Ruling (2026-11-11)

`gamecult-chat:ruling:chat-recall-lookup-not-memory-dms-out`:

- Chat retrieval is conversation lookup, not memory. There is no Persona recall
  lane; Personas have a shell and run `huginn sources`. What a Mind keeps is
  written memory (`persona-memory-written-not-transcript`).
- DMs never enter the index; the index covers the rooms the reader is joined to,
  minus DMs and `#users`/`#members`/`#patrons`
  (`gamecult-chat:ruling:chat-index-minds-rooms`), Discord never.
- A Mind that must remember DM content writes a memory citing the chat or room
  id as its source; the index is not where DM content is kept.

## Status: read side (cut chat-search)

Built: `huginn chat search [TEXT]... [--room R]... [--after T] [--before T] [--top-k N] [--json]`
over request `SearchChat` (`sources::search_chat`), listing oldest first without
texts and ranking by meaning with them. The chat point's shape is `ChatPayload`
(`sources/mod.rs`); every point carries `corpus` (`forge` or `chat`) and
`SourcesMeta.corpus` is `forge:GameCult+chat:rooms`. Not built: the ingest job
that writes chat points through `upsert_chat`, and the event-id window. Until the
ingest lands the collection holds no chat points and a chat search answers empty.
In a listing across rooms, the last hit's `ts_end_ms + 1` can repeat messages of
an earlier-starting longer chunk of another room; within one room it continues
exactly.
