# Desk feed golden fixtures

The bytes on a Mind's desk socket for a list or a read. The contract is `docs/desk-feed-door.md`.
`crates/huginn-mind/tests/desk_feed_fixtures.rs` writes these files with Huginn's own encoders and
checks them on every `cargo test -p huginn-mind --test desk_feed_fixtures`. Set
`HUGINN_WRITE_DESK_FIXTURES=1` to rewrite them. Do not edit them by hand.

Each case has up to four files:

- `<case>.frame.bin` and `<case>.frame.hex`: the whole frame on the socket (u32 big-endian length,
  then the CultNet message).
- `<case>.payload.msgpack` and `<case>.payload.hex`: the base64-decoded `payload` of an operation
  message (named MessagePack). The `record-*` cases are bare payload fragments with no frame.

Hex is lowercase with one trailing newline. The fork id is a question Eureka's mind holds. The
record's answers, notes and hash are synthetic.

**The listing gate is not live.** On `main` every showable question goes to every reader. Do not
put a Mind that holds private questions behind a live receiver before `cut-desk-feed-listing`
lands (see `docs/desk-feed-door.md`).

| case | direction | content |
|---|---|---|
| `list-open.request` | Bifrost to Huginn | `Forks`, status open, no cursor, limit 50 |
| `list-open-cursor.request` | Bifrost to Huginn | `Forks`, status open, cursor `c42`, limit 50 |
| `list-counts.request` | Bifrost to Huginn | `Forks`, status open, limit 0 (counts and stamp only) |
| `read.request` | Bifrost to Huginn | `Fork` for one fork id |
| `list-empty.response` | Huginn to Bifrost | an empty Mind, or nothing listed to this reader: counts 0, `forks: []` |
| `list-counts.response` | Huginn to Bifrost | the limit-0 answer: open 3, recorded 5, `forks: []` |
| `read-unknown.response` | Huginn to Bifrost | `NotStored` `fork_unknown` (Bifrost: 404) |
| `list-too-large.response` | Huginn to Bifrost | `NotStored` `fork_too_large` naming the cursor to continue (Bifrost: 422) |
| `read-too-large.response` | Huginn to Bifrost | `NotStored` `fork_too_large`, read form: names the fork, no order or cursor (Bifrost: 422) |
| `list-response-too-large.response` | Huginn to Bifrost | `Refused` `ResponseTooLarge` (Bifrost: 503) |
| `refused-unit.response` | Huginn to Bifrost | `Refused` with a unit variant: a bare string `"MissingIdentity"` (shape only; Bifrost: 503) |
| `unavailable.response` | Huginn to Bifrost | `Refused` `Unavailable` (Bifrost: 503 for this Mind) |
| `failure.response` | Huginn to Bifrost | operation failure, `payloadSchema` `gamecult.cultnet.operation_failure.v1` (Bifrost: 503) |
| `error.response` | Huginn to Bifrost | CultNet `Error` message, no payload (Bifrost: 503) |
| `record-whole` | fragment | `gamecult.fork.record.v0` with a counted answer and a revision, answerer ids and notes raw, as the door sends them (Bifrost masks for a reader who is not a decider) |

No case carries a card (`gamecult.fork.card.v0`). The card's bytes wait on CultLib's re-pinned card
vectors for `raised_at`, so no entry-bearing list page or read is pinned yet.
