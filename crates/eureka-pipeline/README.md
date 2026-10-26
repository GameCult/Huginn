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

The real-store gate is huginn-mind's `#[ignore]` `query::tests::stored_documents_read_back`,
which checks these stored bytes and the read path together (the leaf cannot see the read
path); the Huginn Upgrade runbook in gamecult-ops runs it.
