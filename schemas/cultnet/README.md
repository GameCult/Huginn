# Huginn's CultNet contracts

`huginn.mind_request.v1` and `huginn.mind_response.v1` are the organ's own wire
schemas, published from here because Huginn owns them. `huginn-daemon` serves
them on a `cultnet.schema_catalog_request.v0`; nothing else reads them.

Each file is `serde_json::to_string_pretty` of `schemars::schema_for!` over the
type of the same name in `crates/huginn-mind/src/wire.rs`. Regenerate by hand
and the test refuses it:
`cargo test -p huginn-mind --lib -- --exact wire::tests::published_wire_schemas_match_derivation`
compares file to derivation byte for byte.

## The pipeline catalogue

`index.json` is the publication manifest for every schema here: the fourteen
`epiphany.pipeline.*.v2` schemas, `gamecult.persona_state.v0` and the two
`huginn.mind_*` wire schemas. The pipeline
schemas are derived from the value types in `crates/eureka-pipeline` and checked
byte for byte by its `pipeline_published_schemas_match_derivation` test. Each
document payload is `[value]`, a one-element MessagePack array whose element is
the named map the schema describes. Text bounds are UTF-8 bytes and enforced by
admission, not by the schema. A new named field with a serde default keeps
`epiphany.pipeline.epoch.v2`; a breaking change bumps the epoch, and the new
binary refuses the old store.
