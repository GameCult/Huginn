# Desk feed door: the list and read wire between Bifrost and a Mind's Huginn

Status, 2026-11-11: the list and read wire described here is on `main` (`DeskRequest::Forks`,
`DeskRequest::Fork`, `DeskResponse::Forks`, `DeskResponse::Fork`; `crates/huginn-mind/src/desk_wire.rs`
and `desk_wire/reads.rs`; the socket is `crates/huginn-daemon/src/desk_door.rs`). No running daemon
binds a desk socket yet (follow-up `eureka-substrate:follow_up:desk-door-ops-root-run`). Two parts
are planned and not built: `raised_at` on the card, and the listing gate that decides which forks a
reader sees. They are cut specs `cut-desk-feed-raised-at` and `cut-desk-feed-listing` in the
`eureka-substrate` campaign.

This page is the wire contract for the consumer, Bifrost's `UnixSocketDeskDoor` (Bifrost PR #67,
`docs/desk-feed-design.md`). The golden bytes are in `crates/huginn-mind/tests/fixtures/desk-feed/`.
Huginn's own encoders write them (`crates/huginn-mind/tests/desk_feed_fixtures.rs`), so a decoder
that reads them reads the door. The document shapes inside the payload (`gamecult.fork.list.v0`,
`gamecult.fork.entry.v0`, `gamecult.fork.card.v0`, `gamecult.fork.record.v0`,
`gamecult.desk_answer.v0`, `gamecult.fork.refusal.v0`) are CultLib's
(`contracts/cultnet/governance/fork-card.md`) and are not restated here.

## One socket per Mind

- Each Mind's Huginn daemon binds its own socket, given by `--desk-socket <absolute path>` and
  `--desk-peer-uid <uid>` (both or neither). The ops plan names `/run/huginn-desk/<mind>.sock`, mode
  0660, group `huginn-desk`.
- The kernel's peer credential is checked before a byte is read. Only the configured uid (Bifrost's)
  is answered; any other peer is closed with nothing read or written. That check is the whole
  authentication: Bifrost vouches for `subject` and `deciders`, and Huginn never sees the member's
  token.
- Each connection carries one request frame and gets one reply frame, all within 1 second. A
  retry is a new connection. Huginn answers each read from its live image at one snapshot. It keeps
  no cache, and Bifrost needs none: the list's `stamp` (`s<N>`) is the change signal.
- Huginn serves only its own Mind. A request whose `mind` is another instance is refused
  (`Refused(...)`).

## Framing and envelope

```
frame    = u32 big-endian length || message
message  = MessagePack map, CultNet schema v0 (cultnet-rs encode_cultnet_message_to_vec)
payload  = message["payload"], standard base64 of named MessagePack (rmp_serde::to_vec_named)
```

- Request message: `schemaVersion` `cultnet.operation_request.v0`, `serviceId` `huginn.desk`,
  `operation` `forks` (list) or `fork` (read), `payloadSchema` `huginn.desk_request.v1`,
  `payloadEncoding` `messagepack-base64`, `messageId` (at most 256 bytes, echoed back).
- Response message: `cultnet.operation_response.v0`, the same `serviceId`, `operation` and
  `messageId`, `status` `accepted` or `rejected`, and `sourceRuntimeId` (informational: do not
  compare it). The `payloadSchema` is `huginn.desk_response.v1`, or
  `gamecult.cultnet.operation_failure.v1` when the envelope or payload did not decode.
- Envelope keys are written in sorted order and absent optionals are omitted. Decode by key, not
  by position.
- Frame bound: 65536 bytes each way. A reply past it is refused by name, never truncated.

## Payloads

Rust's externally tagged enums: a one-key map from the variant name to its fields. Every field is
written; `null` where absent.

Request (`huginn.desk_request.v1`):

| operation | payload |
|---|---|
| `forks` | `{"Forks": {mind, subject, deciders, status, cursor, limit}}`. `status` is `"open"`, `"recorded"` or nil (both); `cursor` is `"c<N>"` (N from 1) or nil; `limit` 0..50, where 0 returns counts and stamp only. |
| `fork` | `{"Fork": {mind, subject, deciders, fork_id}}` |

`deciders` is the Mind's admin-set list for this call: 1 to 64 distinct ids, sent on every call.
Huginn keeps no copy. `subject` is the verified Bifrost account id of the reader. The reads carry
no `authority`, and a payload that names it is refused whole.

Response (`huginn.desk_response.v1`):

| variant | meaning | Bifrost answers |
|---|---|---|
| `{"Forks": {list, as_of}}` | one page, `list` is `gamecult.fork.list.v0` | 200 |
| `{"Fork": {entry, as_of}}` | one fork, `entry` is `gamecult.fork.entry.v0` | 200 |
| `{"NotStored": refusal}` | `gamecult.fork.refusal.v0`: `fork_unknown`, or `fork_too_large` with the cursor to continue | 404 / 422 |
| `{"Refused": {"<Variant>": {...}}}` | a Mind refusal (store fault, wrong instance, a response too large) | 503 `unavailable` |

## Failure means 503 for that Mind only

Each of these gives the request 503 `unavailable` with `Retry-After`. Only the Mind addressed is
affected, because each Mind has its own socket and the client asks per Mind:

- the socket is absent, or refuses the connection (`ENOENT`, `ECONNREFUSED`), or the peer closes it
  without a reply frame;
- no whole reply frame arrives within the deadline;
- the reply is a CultNet `Error` message (`cultnet.error.v0`, fixture `error.response`), an
  operation failure (fixture `failure.response`), or a `Refused` payload (fixture
  `unavailable.response`).

An empty list is never served in place of a failure. `open_count: 0` means zero open forks.

## Card fields (pending CultLib's re-pinned card vectors)

`raised_at` is ruled by CultLib's steward (2026-11-11). It is a required card key whose value is an
RFC3339 string or nil, which takes the card from 16 to 17 keys in closed map-mode. Nil means unknown.
A string is the question's first-admission time, which never moves. It is outside `options_hash`,
which keeps its five members, so the hash vectors do not change. Huginn derives it from the
question's admission: the `committed_at` of the receipt that first admitted the question, or for an
imported question the source mind's `committed_at` (`imported_from.committed_at`).

**No fixture here carries card bytes.** The card's key order and hex wait on the CultLib commit that
re-pins the card vectors with `raised_at`. Until then the card is opaque to these fixtures, and
every pinned response carries no entry. The entry-bearing fixtures (an open list page and a read
with `raised_at` as a string and as nil) are added by `cut-desk-feed-raised-at` against that
CultLib commit.

## Trust boundary: what Huginn sends raw, what Bifrost does

- Huginn sends every record whole over the peer-checked socket: every answer's `answerer` id and
  every note, unmasked (fixture `record-whole`). The peer-uid check means only Bifrost's uid reads
  them. A second reader of the socket would leak answerer ids and notes, so the socket must belong
  to Bifrost's uid alone.
- Bifrost owns three things: the per-request membership check, the `desk.read` token, and
  masking on what the door returns. It masks answerer ids for a reader who is not a decider (X3,
  Heimdall 2026-11-11) and strips notes (ruling `desk-read-visibility-whole-record`, `DeskNotes`).
  Huginn masks nothing.

## Listing: which forks a reader sees (Huginn's; planned)

Huginn owns audience and visibility, at its door. The card has no audience key (CultLib), and
Bifrost's feed applies no fork filter.

- **Default: not listed.** A fork (a showable question) appears in a list or read for a reader who
  is not on the call's `deciders` only while a `desk_listing` document for it is in force. Otherwise
  the list omits it, it counts in neither `open_count` nor `recorded_count`, and a read of it
  answers `fork_unknown`, the same bytes as a fork that does not exist. The counts and the cursor
  are computed over the visible set, which is why the gate is Huginn's: a filter applied after
  paging would make both wrong.
- **Deciders** see every showable fork whether or not it is listed (recommended in question
  `eureka-substrate:question:desk-unlisted-to-deciders`), because they must see a fork to answer it.
  The door compares the call's `subject` with its `deciders` to tell. The wire does not change.
- **The listing record** is planned kind `desk_listing`, key `<campaign>:desk_listing:<question-label>`,
  fields `fork` (the question's id) and `reason` (one line: why members may read it). It is
  admitted on the Mind's own door, so its receipt records who listed it and when. Unlisting is a
  `Withdrawn` resolution on it, and relisting withdraws that resolution, which reopens it. Each
  flip is a document with its own receipt, so the history is `huginn history <listing id>`. No flag
  is stored anywhere else.
- **Who may list or unlist** is question `eureka-substrate:question:desk-listing-authority`. The
  recommended answer is the asking Mind herself, at her Self.
- **Open for Heimdall:** if Bifrost wants any part of the listing (an operator override at the
  desk, a per-Mind default, a reason shown to members), that is his decision to raise. Nothing in
  this plan puts listing state in Bifrost.

The fixture `list-empty.response` is also what a reader who is not a decider gets from a Mind
with nothing listed.
