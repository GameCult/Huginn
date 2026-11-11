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

Hex is lowercase with one trailing newline.

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
| `unavailable.response` | Huginn to Bifrost | `Refused` `Unavailable` (Bifrost: 503 for this Mind) |
| `failure.response` | Huginn to Bifrost | operation failure, `payloadSchema` `gamecult.cultnet.operation_failure.v1` (Bifrost: 503) |
| `error.response` | Huginn to Bifrost | CultNet `Error` message, no payload (Bifrost: 503) |
| `record-whole` | fragment | `gamecult.fork.record.v0` with a counted answer and a revision, answerer ids and notes raw, as the door sends them (Bifrost masks for a reader who is not a decider) |

No case carries a card (`gamecult.fork.card.v0`). The card's bytes wait on CultLib's re-pinned card
vectors for `raised_at`, so no entry-bearing list page or read is pinned yet.
