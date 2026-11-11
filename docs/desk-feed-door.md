# Desk feed door: the list and read wire between Bifrost and a Mind's Huginn

Status, 2026-11-11: the list and read wire described here is on `main` (`DeskRequest::Forks`,
`DeskRequest::Fork`, `DeskResponse::Forks`, `DeskResponse::Fork`; `crates/huginn-mind/src/desk_wire.rs`
and `desk_wire/reads.rs`; the socket is `crates/huginn-daemon/src/desk_door.rs`). No running daemon
binds a desk socket yet (follow-up `eureka-substrate:follow_up:desk-door-ops-root-run`). Two parts
are planned and not built: `raised_at` on the card, and the listing gate that decides which forks a
reader sees. They are cut specs `cut-desk-feed-raised-at` and `cut-desk-feed-listing` in the
`eureka-substrate` campaign.

**Not live: the listing gate.** On `main` today every showable question is returned to every
reader, whatever the call's `deciders` say (pinned by
`the_whole_record_comes_back_whoever_the_deciders_list_leaves_out` in `desk_wire/reads.rs`).
The default of not listed, described under Listing below, holds only once `cut-desk-feed-listing`
lands. Until then, neither Bifrost's receiver nor the ops root-run (`desk-door-ops-root-run`) may
go live on a Mind that holds private questions.

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

## Where forks come from

A fork is any question the Mind holds, in any of its campaigns, that is showable. Showable
(`showable`, `desk_wire.rs`) means at least two options, no blank option text or `depends` line,
and at least one decider on the call. An in-force question is `open`. A question with any
resolution in force is `recorded` (a closed card, `ruling` set when an in-force ruling answers it).
That set includes the questions holding a spec (`huginn blocked`) and the open questions of every
campaign (`huginn open`). Nothing else in the mind is a fork: follow-ups, findings, research and
Persona state never reach the desk.

The fixtures use the fork id `eureka-substrate:question:desk-listing-authority`, a question Eureka's
mind holds. The record's answers, notes and `options_hash` are synthetic: the hash is CultLib's
sample digest.

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
- Operation messages (requests, responses, failures) have their keys written in sorted order,
  with absent optionals omitted. The CultNet `Error` message does not follow that: its keys come in
  CultNet's raw order (`schemaVersion`, `error`, `routingHint`, `code`, `details`), and absent
  values are written as nil (fixture `error.response`). Decode by key, never by position.
- An operation failure carries `diagnostics`, a list holding the failure code (fixture
  `failure.response`). It is absent on every other response.
- The door also answers `cultnet.schema_catalog_request.v0` with the catalog of the two desk
  schemas (`huginn.desk_request.v1`, `huginn.desk_response.v1`, with their JSON Schemas). No
  fixture pins that reply, because it embeds the schema text.
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

**Account ids.** `subject`, every `deciders` entry and every stored `answerer` are Bifrost account ids:
`UserAccount.Id` in lowercase D-format, 8-4-4-4-12 lowercase hex (Bifrost renders it in
`DeskEndpoints.cs` with `accountId.Value.ToString("D")`, and `StartupConfigurationValidator.cs`
refuses any decider not in that form). That is the same grammar Huginn names for `RequestId`
(`uuid_text`). These ids are opaque random UUIDs. A Matrix id, an email, a display name or any other
handle is never one of them. Bifrost enforces the grammar; Huginn's door does not yet: it reads these
fields as `Short` (any text up to 200 bytes) and would accept `@x:server` or an email. A subject
is stored as `answerer` on an answer, so a wrong value would enter the mind. Refusing every other
shape at decode is owed by `cut-desk-feed-listing.r6`. The test
`every_account_id_in_the_fixtures_is_a_lowercase_d_format_uuid` pins the grammar. The fixtures use
synthetic UUIDs.

## Account ids by reader (planned; needs a CultLib change)

A reader is a **decider** when the call's `subject` is in the call's `deciders`, compared byte for
byte; both values are in the account-id format above. Any other reader is a non-decider.

- **On `main` today**, every reader gets the ids. `card.deciders` carries the call's decider list on
  every card, and every answer carries its `answerer` id. This is not live yet: no receiver is
  deployed.
- **Ruled (Persona, 2026-11-11; follow-up `chat-mind-20261111T150417226Z-t167`):** the door, not
  Bifrost, withholds account ids from a non-decider and returns a count instead. Bifrost cannot do
  it, because the card is opaque to it. A decider sees the ids. The superseding ruling is in force:
  `eureka-substrate:ruling:desk-read-visibility-withheld-ids`.
- **The shape** (ruling `eureka-substrate:ruling:desk-reader-counts-shape`, from Libby's CultLib
  reader-counts cut). CultLib main does not have it yet: on main the card has 16 required keys and
  `deciders` needs at least one item, and `answerer` is a required, non-null id.
  - **Card:** `decider_count` is an integer from 1 to 64, placed right after `deciders`. An empty
    `deciders` means the ids are withheld from this reader; a non-empty list holds exactly
    `decider_count` ids. With `raised_at` the card has 18 keys (MessagePack header `DE 00 12`).
  - **Record:** `answerer_count` is the number of distinct answerers over ALL the fork's stored
    answers, refusals included. It is the same for both readers. The record has 6 keys (header `86`).
  - **Answers:** `answerer` is nullable on all four variants (`Counted`, `Revision`, `Retraction`,
    `Refused`). It is null only in a non-decider's view, with the key kept. Stored answers always
    carry it.
  - `options_hash` does not change (still five members).
- **Mirror fixtures.** These come from types that exist only in the test. They are neither CultLib's
  bytes nor Huginn's output:
  - `card-viewers-decider.synthetic` and `card-viewers-non-decider.synthetic` hold only `deciders`
    and `decider_count`. They are not cards.
  - `record-decider.mirror` and `record-non-decider.mirror` are a record with four answers by three accounts, one of
    each variant (`Counted`, `Revision`, `Refused`, `Retraction`), with one account answering twice, as each reader sees it.

  The tests check:
  - a non-decider's bytes hold no account id anywhere, and a decider's hold every id;
  - `answerer_count` is 3 over four answers, counting the refusal, for both readers, and each record view starts with
    `86`;
  - a non-empty `deciders` must match `decider_count`, and the count stays within 1 to 64;
  - `answerer` is null exactly in a non-decider's view.

  Where `answerer_count` sits among the record's six keys is CultLib's to fix. The mirror puts it
  before `answers`.
- **Pending:** Libby's re-pinned vectors. When she posts the CultLib sha, Huginn's real types are
  decoded against those vectors and these mirrors are retired (`cut-desk-feed-listing`). No byte
  beyond Huginn's own envelope is pinned until then.

Response (`huginn.desk_response.v1`):

| variant | meaning | Bifrost answers |
|---|---|---|
| `{"Forks": {list, as_of}}` | one page, `list` is `gamecult.fork.list.v0` | 200 |
| `{"Fork": {entry, as_of}}` | one fork, `entry` is `gamecult.fork.entry.v0` | 200 |
| `{"NotStored": refusal}` | `gamecult.fork.refusal.v0`: `fork_unknown`, or `fork_too_large` | 404 / 422 |
| `{"Refused": <MindRefusal>}` | a Mind refusal (store fault, wrong instance, a list reply over the cap) | 503 `unavailable` |

The `Refused` value is opaque to Bifrost: any `Refused` is a 503. Its shape depends on the
variant: a variant with fields is a one-key map (`{"Unavailable": {"detail": ...}}`,
`{"ResponseTooLarge": {"bytes": ..., "limit": ...}}`), and a unit variant is a bare string
(`"MissingIdentity"`, fixture `refused-unit.response`).

**`fork_too_large` has two forms, and both are live on `main`.**

- The list form comes from `Mind::desk_forks` when the first entry of a page is alone past the
  page budget: `fork_too_large: fork <fork_id> at order <N> is past the page budget by itself;
  continue with cursor c<N>`, or `...; this is the end of the list` at order 0 (fixture
  `list-too-large.response`).
- The read form comes from the door (`respond` in `desk_door.rs`, `fork_too_large_read`) when a
  read's whole entry is past the 65536-byte reply cap: `fork_too_large: fork <fork_id> is past the
  response bound by itself`, with no order and no cursor (fixture `read-too-large.response`).
  CultLib `fork-card.md` limitation 13 still says that an over-cap read reaches Bifrost as
  `Refused(ResponseTooLarge)`. That text is stale: Huginn main answers the read form. The refusal
  carries no typed order or fork id. A `ForkTooLarge(ForkId[, Order])` in Bifrost must parse them
  out of the informational `error` text (CultLib calls the text informational), or use the route's
  `fork_id` and the request's cursor instead. The second is the safer choice.
- Any other reply over the cap, which in practice means a list, is `Refused(ResponseTooLarge)`, a
  503 (fixture `list-response-too-large.response`). A list should not reach it, because pages are
  budgeted below the cap.

## Failure means 503 for that Mind only

Each of these gives the request 503 `unavailable` with `Retry-After`. Only the Mind addressed is
affected, because each Mind has its own socket and the client asks per Mind:

- the socket is absent, or refuses the connection (`ENOENT`, `ECONNREFUSED`), or the peer closes it
  without a reply frame;
- no whole reply frame arrives within the deadline;
- the reply is a CultNet `Error` message (`cultnet.error.v0`, fixture `error.response`), an
  operation failure (fixture `failure.response`), or a `Refused` payload (fixtures
  `unavailable.response`, `list-response-too-large.response`, `refused-unit.response`).

An empty list is never served in place of a failure. `open_count: 0` means zero open forks.

## Card fields (pending CultLib's re-pinned card vectors)

`raised_at` is ruled by CultLib's steward (2026-11-11). It is a required card key whose value is an
RFC3339 string or nil, which takes the card from 16 to 17 keys in closed map-mode (18 with `decider_count`). Nil means unknown.
A string is the question's first-admission time, which never moves. It is outside `options_hash`,
which keeps its five members, so the hash vectors do not change. Huginn derives it from the
question's admission: the `committed_at` of the receipt that first admitted the question, or for an
imported question the source mind's `committed_at` (`imported_from.committed_at`). A value that does not
parse as RFC3339 is served as nil. One known limit: after a campaign has moved twice (A to B to C),
C sees B's admission time, because an import quotes its source's receipt and B's receipt is B's
own (follow-up `desk-raised-at-import-chain`).

**No fixture here carries card bytes.** The card's key order and hex wait on the CultLib commit that
re-pins the card vectors with `raised_at`. Until then the card is opaque to these fixtures, and
every pinned response carries no entry. The entry-bearing fixtures (an open list page and a read
with `raised_at` as a string and as nil) are added by `cut-desk-feed-raised-at` against that
CultLib commit.

## Trust boundary: what Huginn sends, what Bifrost does

- The peer-uid check is the whole authentication, so the socket must belong to Bifrost's uid alone.
- Today Huginn sends every record whole: every `answerer` id, every note and the card's
  `deciders` (fixture `record-whole`).
- Once `cut-desk-feed-listing` and the CultLib change land, Huginn withholds the account ids from a
  non-decider (above). Notes stay whole on the wire.
- Bifrost owns three things: the per-request membership check, the `desk.read` token, and
  stripping notes for a reader who is not a decider (`DeskNotes`). It no longer masks answerer
  ids.

## Listing: which forks a reader sees (Huginn's; NOT live, see the status above)

Huginn owns audience and visibility, at its door. The card has no audience key (CultLib), and
Bifrost's feed applies no fork filter.

- **Default: not listed.** A fork (a showable question) appears in a list or read for a reader who
  is not on the call's `deciders` only while a `desk_listing` document for it is in force. Otherwise
  the list omits it, it counts in neither `open_count` nor `recorded_count`, and a read of it
  answers `fork_unknown`, the same bytes as a fork that does not exist. The counts and the cursor
  are computed over the visible set, which is why the gate is Huginn's: a filter applied after
  paging would make both wrong.
- **Deciders** see every showable fork whether or not it is listed (ruling
  `eureka-substrate:ruling:desk-deciders-see-all`), because they must see a fork to answer it. The
  door compares the call's `subject` with its `deciders` to tell. The wire does not change. An
  operator or steward who is not on `deciders` sees only listed forks: reads carry no authority, so
  anyone who must see unlisted forks is put on the Mind's decider list in Bifrost's admin.
- **What a non-decider can still infer.** `order` is the fork's rank among all the Mind's
  questions, and `stamp` is the Mind's head. So a non-decider sees gaps in `order` (some unlisted or
  unshowable questions exist between two listed ones) and sees `stamp` move on any admission. The
  gate accepts this leak: no fork's text, id or count leaves the door. Ranking over the visible set
  only would make `order` depend on the reader and move whenever a fork is listed, which breaks
  CultLib's rule that an order never changes and breaks every held cursor.
- **The listing record** is planned kind `desk_listing`, key `<campaign>:desk_listing:<question-label>`,
  fields `campaign`, `listed` (the question's id, in the same campaign) and `reason` (one line: why
  members may read it). It is
  admitted on the Mind's own door, so its receipt records who listed it and when. Unlisting is a
  `Withdrawn` resolution on it, and relisting withdraws that resolution, which reopens it. Each
  flip is a document with its own receipt, so the history is `huginn history <listing id>`. No flag
  is stored anywhere else.
- **Who may list or unlist:** the asking Mind herself (ruling
  `eureka-substrate:ruling:desk-listing-asking-mind`). A fork that names a person's matter is listed
  only with that person's say.
- **CultLib's text disagrees.** `fork-card.md` (Read and List) says no fork is hidden by
  `fork_unknown` and every `desk.read` holder sees every card. That is Libby's to amend for the
  listing gate (follow-up `cultlib-fork-card-listing`).
- **Open for Heimdall:** if Bifrost wants any part of the listing (an operator override at the
  desk, a per-Mind default, a reason shown to members), that is his decision to raise. Nothing in
  this plan puts listing state in Bifrost.

The fixture `list-empty.response` is also what a reader who is not a decider gets from a Mind
with nothing listed.
