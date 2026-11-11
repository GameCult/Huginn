# CultLib desk-feed vectors, vendored

Source: `GameCult/CultLib` at commit `d3025d3c19674803ad32eb2335461b8cbc22bd28` (#118, on
`3a51bbab` #117, on `af914253` #115), file `contracts/cultnet/governance/fork-card-vectors.json`. Each `.msgpack` file is
the vector's `messagePackHex` decoded to bytes, nothing else changed. Do not edit them or rewrite them
from Huginn's encoders: they are the reference Huginn's types are checked against (see
`docs/desk-feed-door.md`). To re-pin, take the new commit's vectors and update the sha here, in
`docs/desk-feed-door.md` and in `CULTLIB_SHA` in `desk_feed_fixtures.rs`.

| file | upstream vector | what it pins |
|---|---|---|
| `card-operator-fork` | `card.valid` `operator_fork` | 18 keys (`DE 00 12`), `raised_at` a string, a decider's `deciders` and `decider_count` 1 |
| `card-raised-at-nil` | `card.valid` `raised_at_nil` | `raised_at` nil, key kept |
| `card-withheld-deciders` | `card.valid` `withheld_deciders` | empty `deciders`, `decider_count` 2 |
| `answer-counted` | `deskAnswer.valid` `counted` | 16 keys (`DE 00 10`), `answerer` a string |
| `answer-revision` | `deskAnswer.valid` `revision` | a `Revision`, `answerer` a string |
| `answer-retraction` | `deskAnswer.valid` `retraction` | a `Retraction`, `answerer` a string |
| `answer-withheld-answerer` | `deskAnswer.valid` `withheld_answerer` | `answerer` nil |
| `answer-withheld-answerer-revision` | `deskAnswer.valid` `withheld_answerer_revision` | a `Revision`, `answerer` nil |
| `answer-withheld-answerer-retraction` | `deskAnswer.valid` `withheld_answerer_retraction` | a `Retraction`, `answerer` nil |
| `answer-refused-stale` | `deskAnswer.valid` `refused_stale` | a `Refused` answer, `answerer` a string |
| `entry-open-counted` | `transport.entry.valid` `open_counted` | decider view: entry `83`, record `86`, `answerer_count` 1 |
| `entry-withheld-open-counted` | `transport.entry.valid` `withheld_open_counted` | non-decider twin: `deciders` empty, `answerer` nil, count 1 |
| `entry-closed-with-ruling-and-a-refusal` | `transport.entry.valid` `closed_with_ruling_and_a_refusal` | decider view, a refusal among the answers, `answerer_count` 2 |
| `entry-withheld-closed-with-ruling-and-a-refusal` | `transport.entry.valid` `withheld_closed_with_ruling_and_a_refusal` | its non-decider twin, count 2 |

The test `desk_feed_fixtures.rs` checks each file's SHA-256, decodes it with a decoder of its own
(not `rmp_serde`), then decodes it into Huginn's real type and re-encodes it, byte for byte.
