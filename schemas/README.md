# Schemas

This folder is the canonical paperwork shrine for Epiphany's shared state
contracts.

If a Persona field or Mind document matters
enough to steer the machine, it should have a receipt here instead of living
only in one Rust struct or one developer's damp recollection.

## Canonical surfaces

- [cultnet/gamecult.persona_state.v0.schema.json](./cultnet/gamecult.persona_state.v0.schema.json):
  portable Persona state contract for Epiphany Persona, VoidBot repo Personas, and
  Ghostlight characters. It carries explicit provenance, public presentation
  metadata, typed `candidateActions`, and a non-authoritative extension bag for
  source-specific fields; social bonds, status reads, and doctrine stances are
  typed affect records rather than generic thought blobs. Timestamps use JSON
  Schema `date-time`, `presentation` is required, and `custom` enum values have
  companion custom-label fields.
- [cultnet/epiphany.work_organ_state.v0.schema.json](./cultnet/epiphany.work_organ_state.v0.schema.json):
  light function-shaped state for Epiphany internal work organs.
- [cultnet/README.md](./cultnet/README.md):
  JSON Schema publication artifacts for typed CultNet boundaries. Live
  providers own their schema-catalog responses.

## Source Of Truth

The living implementation is in code:

- [mind_documents.rs](/F:/Projects/Epiphany/epiphany-core/src/mind_documents.rs)

The rule is simple:

- code owns executable truth
- this folder owns human-readable contract receipts
- vendored copies are downstream, not the throne

## Update Discipline

When changing any of the following, update this folder in the same pass:

- canonical organ-state or Persona family names
- standing role trait names
- schema version identifiers

If a change lands in code without a matching receipt here, assume the machine
has started whispering to itself in the walls again.
