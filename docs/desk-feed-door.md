# Desk feed door: the list and read wire between Bifrost and a Mind's Huginn

Status, 2026-11-11: the list and read wire described here is on `main` (`DeskRequest::Forks`,
`DeskRequest::Fork`, `DeskResponse::Forks`, `DeskResponse::Fork`; `crates/huginn-mind/src/desk_wire.rs`
and `desk_wire/reads.rs`; the socket is `crates/huginn-daemon/src/desk_door.rs`). No running daemon
binds a desk socket yet (follow-up `eureka-substrate:follow_up:desk-door-ops-root-run`). The reader
shapes are built: the card's `raised_at` and `decider_count`, the record's `answerer_count`, the
nullable `answerer`, and the withholding of account ids from a non-decider (below). One part is
planned and not built: the listing gate that decides which forks a reader sees (cut spec
`cut-desk-feed-listing` in the `eureka-substrate` campaign).

**Not live: the listing gate.** On `main` today every showable question is returned to every
reader, whatever the call's `deciders` say. The default of not listed, described under Listing below,
holds only once `cut-desk-feed-listing` lands. Until then, neither Bifrost's receiver nor the ops
root-run (`desk-door-ops-root-run`) may go live on a Mind that holds private questions.

This page is the wire contract for the consumer, Bifrost's `UnixSocketDeskDoor` (Bifrost PR #67,
`docs/desk-feed-design.md`). The golden bytes are in `crates/huginn-mind/tests/fixtures/desk-feed/`.
Huginn's own encoders write them (`crates/huginn-mind/tests/desk_feed_fixtures.rs`), so a decoder
that reads them reads the door. The reader-count shapes are CultLib's bytes, vendored in
`cultlib-d3025d3c/` (below); Huginn's types decode and re-encode every vendored vector byte for byte. The document shapes inside the payload (`gamecult.fork.list.v0`,
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
shape at decode is owed by `cut-desk-feed-listing`. The test
`every_account_id_in_the_fixtures_is_a_lowercase_d_format_uuid` pins the grammar. The fixtures use
synthetic UUIDs.

## Account ids by reader

A reader is a **decider** when the call's `subject` is in the call's `deciders`, compared byte for
byte; both values are in the account-id format above. Any other reader is a non-decider. The
reader is worked out per call and stored nowhere.

- **Ruled (Persona, 2026-11-11; follow-up `chat-mind-20261111T150417226Z-t167`):** the door, not
  Bifrost, withholds account ids from a non-decider and returns a count instead. Bifrost cannot do
  it, because the card is opaque to it. A decider sees the ids. The ruling is
  `eureka-substrate:ruling:desk-read-visibility-withheld-ids`.
- **The shape**, pinned to CultLib commit `d3025d3c19674803ad32eb2335461b8cbc22bd28` (#118 "null-answerer vectors for
  Revision and Retraction", additive over `3a51bbab` #117 "Fork card: decider_count, answerer_count,
  nullable answerer (reader views)", on `af914253` #115 `raised_at`;
  ruling `eureka-substrate:ruling:desk-reader-counts-shape`). The bytes are CultLib's own vectors
  (`contracts/cultnet/governance/fork-card-vectors.json`, `messagePackHex`), vendored unedited in
  `crates/huginn-mind/tests/fixtures/desk-feed/cultlib-d3025d3c/` (its README lists each file and
  its vector name). Huginn's `ForkCard`, `ForkRecord`, `ForkDeskAnswer` and `ForkEntry` decode all
  14 of them and re-encode them byte for byte. Read off those bytes:
  - **Card:** 18 keys, header `DE 00 12` (map16): `schemaVersion` plus 17 fields, in this order:
    `fork_id, title, context, options, recommendation, depends, leans, flag, raised_in, raised_at,
    deciders, decider_count, status, ruling, ruled_from, order, options_hash`. `raised_at` is an
    RFC3339 string or nil, always written. `decider_count` is an integer from 1 to 64 right after
    `deciders`: the length of the call's list, for every reader. An empty `deciders` means the ids
    are withheld from this reader, never "no deciders"; a non-empty list holds exactly
    `decider_count` ids. The card decoder refuses a count outside 1..64 or a non-empty list of
    another length.
  - **Record:** 6 keys, header `86`: `schemaVersion, fork_id, options_hash, counted_answer_id,
    answerer_count, answers`, so `answerer_count` is the fifth key, right before `answers`. It is
    the number of distinct answerers over ALL the fork's stored answers, refusals included, and is
    the same for both readers. In a non-decider's view every answerer is nil, and the count is then
    between 1 and the number of answers (0 with no answers).
  - **Answer:** `gamecult.desk_answer.v0` keeps its 16 keys (header `DE 00 10`); `answerer` is a
    string or nil, nil only in a non-decider's view, with the key kept. The bytes pin a nil `answerer`
    on all four variants, `Counted` (`answer-withheld-answerer`), `Revision`
    (`answer-withheld-answerer-revision`), `Retraction` (`answer-withheld-answerer-retraction`) and
    `Refused` (the withheld entries), beside each variant's string-`answerer` vector. Stored answers
    always carry it, and so does the reply to an answer (`Answered`), which goes to the answerer.
  - **Entry:** header `83` (`schemaVersion, card, record`). The view is read off `deciders`: a
    non-empty `deciders` names every answerer, an empty one names none.
  - `options_hash` does not change (still five members).
  - **Fixtures:** Huginn's own encoders write entry-bearing fixtures for both views:
    `list-open.response` and `read.response` (a decider's; `read.response` has `raised_at` nil),
    `list-open-non-decider.response`, `read-non-decider.response` and `record-withheld`, with the
    requests `list-non-decider.request` and `read-non-decider.request`. A test searches the
    non-decider payloads' bytes for every account id and finds none.
  - **Consumers:** Bifrost branch `heimdall/desk-card-repin` (`f977a43`) vendors Huginn's desk
    fixtures from `79d954c` and needs a re-pin to this change. Bifrost's `DeskDocuments.cs` marks
    `answerer_count` `JsonRequired`; the door now sends it.

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

## Card fields

`raised_at` is a required card key whose value is an RFC3339 string or nil (CultLib #115, `af914253`).
Nil means unknown. A string is the question's first-admission time, which never moves: a UTC `Z` instant at seconds resolution (the form CultLib requires), and nil when the time is unknown, does not parse, or its UTC year is outside 0..9999. It is outside
`options_hash`, which keeps its five members, so the hash vectors do not change. Huginn derives it
from the question's admission: the `committed_at` of the receipt that first admitted the question, or
for an imported question the source mind's `committed_at` (`imported_from.committed_at`). A value that
does not parse as RFC3339 is served as nil. One known limit: after a campaign has moved twice (A to B
to C), C sees B's admission time, because an import quotes its source's receipt and B's receipt is B's
own (follow-up `desk-raised-at-import-chain`).

The card's key order and bytes are pinned by the vendored CultLib vectors (above): `card-operator-fork`
(`raised_at` a string), `card-raised-at-nil` and `card-withheld-deciders`, and Huginn's own entry-bearing
fixtures carry it (`list-open.response` a string, `read.response` nil).

## Trust boundary: what Huginn sends, what Bifrost does

- The peer-uid check is the whole authentication, so the socket must belong to Bifrost's uid alone.
- Huginn sends each record with every answer's note (fixture `record-whole`, a decider's view) and
  withholds the account ids from a non-decider (above, fixture `record-withheld`). Notes stay whole
  on the wire.
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
