# eureka-pipeline

The pipeline leaf: typed documents, bounds, keys and the cultcache wrappers
that define the mind's wire. Library only.

## Provenance and license

Copied once from GameCult/Epiphany at `ef956865`, where it was licensed
PolyForm Noncommercial. Its sole copyright holder relicensed it AGPL-3.0-only
on 2026-10-09; the root `LICENSE` is the AGPL-3.0 text and covers this crate
like the rest of the workspace.

## Wire parity checks

Committed (runs in `cargo test -p eureka-pipeline`): `encoded_envelopes_match_the_committed_golden`
asserts key, cultcache type id, schema name and payload bytes of every kind
and every encoding-relevant variant against `golden/envelopes.txt`. A diff
there is a wire change; do not regenerate it to make the test pass. An additive
field adds golden lines and never changes an existing one.

Opt-in, against a real store: `stored_documents_read_back` decodes, validates
and keys every stored pipeline document and asserts that it re-encodes to the
stored key, type and payload bytes. Take a copy of the store (the test copies
the file again before opening it, so the copy may be read-only), never open the
live one:

```sh
sudo cp /var/lib/gamecult/huginn/minds/eureka/mind.redb "$SNAP/minds/eureka/mind.redb"
HUGINN_MIND_SNAPSHOT="$SNAP" cargo test -p eureka-pipeline -- --ignored --exact tests::stored_documents_read_back --nocapture
```
