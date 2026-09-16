# Huginn's CultNet contracts

`huginn.mind_request.v1` and `huginn.mind_response.v1` are the organ's own wire
schemas, published from here because Huginn owns them. `huginn-daemon` serves
them on a `cultnet.schema_catalog_request.v0`; nothing else reads them.

Each file is `serde_json::to_string_pretty` of `schemars::schema_for!` over the
type of the same name in `crates/huginn-mind/src/wire.rs`. Regenerate by hand
and the test refuses it:
`cargo test -p huginn-mind --lib -- --exact wire::tests::published_wire_schemas_match_derivation`
compares file to derivation byte for byte.
