//! The persona document: one `gamecult.persona_state.v0` per mind, stored
//! opaque and replaced whole.
//!
//! The shape is the published schema's, `schemas/cultnet/`, read here with
//! `include_bytes!`; the mind holds no Rust type for it, only the value that
//! passed the schema. `put_persona` (admission.rs) owns whether a document may
//! enter; this module owns what it is checked against and how it is stored and
//! read back. Nothing here echoes a caller's value: a refusal names a schema
//! location and a keyword, never the text that failed.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use cultcache_rs::{CultCacheEnvelope, DatabaseEntry};
use jsonschema::paths::LocationSegment;
use jsonschema::error::ValidationErrorKind;
use jsonschema::Validator;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mind::Mind;
use crate::receipt::HuginnCommitReceipt;
use crate::refusal::MindRefusal;
use crate::store::MindStore;

const PERSONA_SCHEMA: &[u8] = include_bytes!("../../../schemas/cultnet/gamecult.persona_state.v0.schema.json");

/// The stored envelope's payload: the document, one slot, keyed by the mind's
/// instance. The cache needs a registered entry type to hold the envelope; the
/// document inside stays a value.
#[derive(Clone, Debug, PartialEq, DatabaseEntry)]
#[cultcache(type = "gamecult.persona_state.v0", schema = "HuginnPersonaEntry")]
pub struct HuginnPersonaEntry {
    #[cultcache(key = 0)]
    pub value: Value,
}

/// What `PersonaGet` answers: the stored document, its `updatedAt`, and the
/// receipt of the put that wrote it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PersonaStateView {
    pub value: Value,
    pub updated_at: String,
    pub receipt_id: String,
}

/// How a put ended. A refusal is an answer, as for `admit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PersonaPutOutcome {
    Committed { receipt_id: String },
    Refused(MindRefusal),
}

struct Compiled {
    validator: Validator,
    /// Every property name the schema declares. A path segment outside this
    /// set is a caller's key (`traitMap` keys are free), so it is withheld.
    declared: BTreeSet<String>,
}

fn compiled() -> Result<&'static Compiled, MindRefusal> {
    static COMPILED: OnceLock<Result<Compiled, String>> = OnceLock::new();
    COMPILED
        .get_or_init(|| {
            let schema: Value = serde_json::from_slice(PERSONA_SCHEMA)
                .map_err(|error| format!("the published persona schema does not parse: {error}"))?;
            let validator = jsonschema::draft7::new(&schema)
                .map_err(|error| format!("the published persona schema does not compile: {error}"))?;
            let mut declared = BTreeSet::new();
            declared_names(&schema, &mut declared);
            Ok(Compiled { validator, declared })
        })
        .as_ref()
        .map_err(|detail| MindRefusal::Unavailable { detail: detail.clone() })
}

fn declared_names(node: &Value, names: &mut BTreeSet<String>) {
    match node {
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get("properties") {
                names.extend(properties.keys().cloned());
            }
            map.values().for_each(|child| declared_names(child, names));
        }
        Value::Array(items) => items.iter().for_each(|child| declared_names(child, names)),
        _ => {}
    }
}

/// Rule 1: the document is valid against the published schema. The first
/// failure is the refusal; it names where and which keyword, never the value.
pub(crate) fn validate_shape(state: &Value) -> Result<(), MindRefusal> {
    let compiled = compiled()?;
    let Err(error) = compiled.validator.validate(state) else { return Ok(()) };
    let mut path = error
        .instance_path()
        .segments()
        .map(|segment| match segment {
            LocationSegment::Index(index) => index.to_string(),
            LocationSegment::Property(name) if compiled.declared.contains(name.as_ref()) => name.into_owned(),
            LocationSegment::Property(_) => "*".to_string(),
        })
        .collect::<Vec<_>>();
    match error.kind() {
        ValidationErrorKind::Required { property } => path.extend(property.as_str().map(str::to_string)),
        ValidationErrorKind::AdditionalProperties { .. } => path.push("*".to_string()),
        _ => {}
    }
    Err(MindRefusal::PersonaInvalid {
        path: format!("/{}", path.join("/")),
        message: format!("fails the published schema's {} rule", error.kind().keyword()),
    })
}

/// A stored persona envelope's document.
pub(crate) fn decode(envelope: &CultCacheEnvelope) -> Result<Value, MindRefusal> {
    rmp_serde::from_slice::<HuginnPersonaEntry>(&envelope.payload)
        .map(|entry| entry.value)
        .map_err(|error| MindRefusal::Unavailable { detail: format!("the stored persona does not decode: {error}") })
}

/// A document's `updatedAt`; the schema makes it a required string.
pub(crate) fn updated_at(value: &Value) -> Option<&str> {
    value.get("updatedAt").and_then(Value::as_str)
}

impl<S: MindStore> Mind<S> {
    /// This mind's persona envelope, if one is stored.
    pub(crate) fn persona_envelope(&self) -> Option<&CultCacheEnvelope> {
        self.raw_envelope(HuginnPersonaEntry::TYPE, &self.instance().0)
    }

    /// The stored `updatedAt`; none when no persona is stored.
    pub(crate) fn stored_updated_at(&self) -> Result<Option<String>, MindRefusal> {
        let Some(envelope) = self.persona_envelope() else { return Ok(None) };
        let value = decode(envelope)?;
        updated_at(&value)
            .map(|stamp| Some(stamp.to_string()))
            .ok_or_else(|| MindRefusal::Unavailable { detail: "the stored persona has no updatedAt".into() })
    }

    /// `PersonaGet`: the document, its `updatedAt` and the receipt of the put
    /// that wrote exactly these bytes.
    pub fn persona(&self) -> Result<Option<PersonaStateView>, MindRefusal> {
        let Some(envelope) = self.persona_envelope() else { return Ok(None) };
        let value = decode(envelope)?;
        let integrity = |detail: &str| MindRefusal::Unavailable { detail: detail.into() };
        let updated_at = updated_at(&value).ok_or_else(|| integrity("the stored persona has no updatedAt"))?.to_string();
        let receipt_id = self
            .receipts()?
            .into_iter()
            .find(|receipt: &HuginnCommitReceipt| {
                receipt.writes.iter().any(|write| {
                    write.document_type == envelope.r#type
                        && write.document_key == envelope.key
                        && write.payload_msgpack == envelope.payload
                })
            })
            .ok_or_else(|| integrity("the stored persona has no receipt"))?
            .receipt_id;
        Ok(Some(PersonaStateView { value, updated_at, receipt_id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{INSTANCE, OTHER_INSTANCE, epoch, instance, opened, prepare, provenance, seed, slug};
    use crate::receipt::Faculty;
    use crate::store::test_stores::{MemoryStore, RefusingStore, SwapCommand};
    use eureka_pipeline::PipelineKind;
    use serde_json::json;

    const T1: &str = "2026-10-10T10:00:00Z";
    const T2: &str = "2026-10-10T10:00:01Z";
    const T3: &str = "2026-10-10T10:00:02Z";
    const CANARY: &str = "CANARY-7f3a91-do-not-echo";
    const NOT_LATER: &str = "must be later than the stored updatedAt";

    /// A minimal document the published schema accepts. The test that follows
    /// holds it to the schema file's own required list.
    fn persona(id: &str, updated_at: &str) -> Value {
        let traits = json!({
            "underlyingOrganization": {}, "stableDispositions": {}, "behavioralDimensions": {},
            "presentationStrategy": {}, "voiceStyle": {}, "situationalState": {}
        });
        json!({
            "schemaVersion": "gamecult.persona_state.v0",
            "provenance": {
                "sourceSystem": "huginn", "sourceDocumentId": id, "sourceUpdatedAt": updated_at,
                "exportedAt": updated_at, "authority": "canonical"
            },
            "personaId": id,
            "publicName": "Test Persona",
            "presentation": { "voiceSummary": "plain" },
            "activationProfile": traits,
            "thoughtMemory": { "shortTerm": [], "memories": [], "incubation": [] },
            "agencyPressure": { "pressures": [] },
            "candidateActions": { "actions": [] },
            "affect": {
                "needs": [], "socialBonds": [], "statusReads": [], "moodDimensions": [],
                "socialBiases": [], "doctrineStances": []
            },
            "updatedAt": updated_at
        })
    }

    fn schema_file() -> Value {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/cultnet/gamecult.persona_state.v0.schema.json");
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn put<S: MindStore>(mind: &mut Mind<S>, state: Value, expected: Option<&str>) -> PersonaPutOutcome {
        let instance = mind.instance().clone();
        mind.put_persona(&instance, state, expected.map(str::to_string), provenance(Faculty::Hands), crate::fixtures::now())
    }

    fn committed(outcome: PersonaPutOutcome) -> String {
        match outcome {
            PersonaPutOutcome::Committed { receipt_id } => receipt_id,
            other => panic!("expected a commit, got {other:?}"),
        }
    }

    fn refused(outcome: PersonaPutOutcome) -> MindRefusal {
        match outcome {
            PersonaPutOutcome::Refused(refusal) => refusal,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn invalid(path: &str, rule: &str) -> MindRefusal {
        MindRefusal::PersonaInvalid { path: path.into(), message: format!("fails the published schema's {rule} rule") }
    }

    fn not_later() -> MindRefusal {
        MindRefusal::PersonaInvalid { path: "/updatedAt".into(), message: NOT_LATER.into() }
    }

    fn stale(stored: Option<&str>) -> MindRefusal {
        MindRefusal::PersonaStale { stored: stored.map(str::to_string) }
    }

    fn seeded_over(store: &MemoryStore) -> Mind<MemoryStore> {
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        mind
    }

    fn persona_in_status<S: MindStore>(mind: &Mind<S>) -> Option<String> {
        mind.status(crate::wire::IndexStatus::Current).persona
    }

    #[test]
    fn the_fixture_is_valid_against_the_published_file_and_covers_its_required_fields() {
        let schema = schema_file();
        let validator = jsonschema::draft7::new(&schema).unwrap();
        let document = persona(INSTANCE, T1);
        assert!(validator.is_valid(&document));
        for name in schema["required"].as_array().unwrap() {
            assert!(document.get(name.as_str().unwrap()).is_some(), "the fixture lacks {name}");
        }
    }

    #[test]
    fn a_persona_document_is_validated_at_the_door() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        let before = store.rows();

        for name in schema_file()["required"].as_array().unwrap() {
            let name = name.as_str().unwrap();
            let mut document = persona(INSTANCE, T1);
            document.as_object_mut().unwrap().remove(name);
            assert_eq!(refused(put(&mut mind, document, None)), invalid(&format!("/{name}"), "required"), "{name}");
        }

        let mut extra = persona(INSTANCE, T1);
        extra["notInTheSchema"] = json!(1);
        assert_eq!(refused(put(&mut mind, extra, None)), invalid("/*", "additionalProperties"));

        let mut wrong_type = persona(INSTANCE, T1);
        wrong_type["publicName"] = json!(5);
        assert_eq!(refused(put(&mut mind, wrong_type, None)), invalid("/publicName", "type"));

        let mut bad_stamp = persona(INSTANCE, T1);
        bad_stamp["updatedAt"] = json!("yesterday");
        assert_eq!(refused(put(&mut mind, bad_stamp, None)), invalid("/updatedAt", "format"));

        let mut nested = persona(INSTANCE, T1);
        nested["thoughtMemory"]["memories"] = json!([7]);
        assert_eq!(refused(put(&mut mind, nested, None)), invalid("/thoughtMemory/memories/0", "type"));

        let mut free_key = persona(INSTANCE, T1);
        free_key["activationProfile"]["voiceStyle"]["any-key"] = json!({});
        assert_eq!(
            refused(put(&mut mind, free_key, None)),
            invalid("/activationProfile/voiceStyle/*/mean", "required"),
            "a key the schema does not declare is withheld from the path"
        );

        assert_eq!(store.rows(), before, "no refusal wrote anything");
        assert_eq!(mind.persona().unwrap(), None);
    }

    #[test]
    fn a_refusal_never_echoes_what_the_caller_sent() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        committed(put(&mut mind, persona(INSTANCE, T2), None));

        let mut free_key = persona(INSTANCE, T3);
        free_key["activationProfile"]["voiceStyle"][CANARY] = json!({});
        let mut extra_key = persona(INSTANCE, T3);
        extra_key[CANARY] = json!(CANARY);
        let mut bad_value = persona(INSTANCE, T3);
        bad_value["updatedAt"] = json!(CANARY);
        let mut bad_enum = persona(INSTANCE, T3);
        bad_enum["provenance"]["authority"] = json!(CANARY);
        let mut foreign = persona(CANARY, T3);
        foreign["publicName"] = json!(CANARY);
        let mut imported = persona(INSTANCE, T3);
        imported["provenance"]["authority"] = json!("import");
        imported["provenance"]["sourceSystem"] = json!(CANARY);
        let mut late = persona(INSTANCE, T1);
        late["publicName"] = json!(CANARY);

        let cases: Vec<(Value, Option<&str>)> = vec![
            (free_key, Some(T2)),
            (extra_key, Some(T2)),
            (bad_value, Some(T2)),
            (bad_enum, Some(T2)),
            (foreign, Some(T2)),
            (imported, Some(T2)),
            (late, Some(T2)),
            (persona(INSTANCE, T3), Some(CANARY)),
        ];
        for (document, expected) in cases {
            let refusal = refused(put(&mut mind, document, expected));
            assert!(!format!("{refusal:?}").contains(CANARY), "{refusal:?}");
            assert!(!serde_json::to_string(&refusal).unwrap().contains(CANARY));
        }
    }

    #[test]
    fn persona_id_must_be_the_instance() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        let before = store.rows();
        let refusal = refused(put(&mut mind, persona(OTHER_INSTANCE, T1), None));
        assert_eq!(refusal, MindRefusal::PersonaForeign { instance: INSTANCE.into() });
        assert_eq!(store.rows(), before);
        assert_eq!(mind.persona().unwrap(), None);
    }

    #[test]
    fn only_a_canonical_document_is_the_minds_persona() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        for authority in ["projection", "import"] {
            let mut document = persona(INSTANCE, T1);
            document["provenance"]["authority"] = json!(authority);
            assert_eq!(refused(put(&mut mind, document, None)), MindRefusal::PersonaNotCanonical, "{authority}");
        }
        assert_eq!(mind.persona().unwrap(), None);
    }

    #[test]
    fn a_persona_waits_for_the_minds_identity() {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T1), None)), MindRefusal::MissingIdentity);
        assert!(mind.is_empty());
    }

    #[test]
    fn a_put_must_name_this_mind_and_a_named_asker() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        let before = store.rows();
        let other = slug(OTHER_INSTANCE);
        let outcome = mind.put_persona(&other, persona(INSTANCE, T1), None, provenance(Faculty::Hands), crate::fixtures::now());
        assert!(matches!(refused(outcome), MindRefusal::ForeignInstance { .. }));
        let mut unnamed = provenance(Faculty::Hands);
        unnamed.agent = eureka_pipeline::Short("  ".into());
        let instance = mind.instance().clone();
        let outcome = mind.put_persona(&instance, persona(INSTANCE, T1), None, unnamed, crate::fixtures::now());
        assert!(matches!(refused(outcome), MindRefusal::Document(_)));
        assert_eq!(store.rows(), before);
    }

    #[test]
    fn a_put_is_a_compare_and_swap_on_updated_at() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        assert_eq!(persona_in_status(&mind), None);
        let receipts = |mind: &Mind<MemoryStore>| mind.receipts().unwrap().len();
        let base = receipts(&mind);

        // A first put expects nothing; claiming a stored document is stale.
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T1), Some(T1))), stale(None));
        let first = committed(put(&mut mind, persona(INSTANCE, T1), None));
        assert_eq!(receipts(&mind), base + 1);
        let view = mind.persona().unwrap().unwrap();
        assert_eq!((view.updated_at.as_str(), view.receipt_id.as_str()), (T1, first.as_str()));
        assert_eq!(view.value, persona(INSTANCE, T1));
        assert_eq!(persona_in_status(&mind).as_deref(), Some(T1));

        // Two puts that read the same stored document: the second loses.
        let second = committed(put(&mut mind, persona(INSTANCE, T2), Some(T1)));
        assert_eq!(receipts(&mind), base + 2);
        let rows = store.rows();
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T3), Some(T1))), stale(Some(T2)));
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T3), None)), stale(Some(T2)));
        assert_eq!(store.rows(), rows, "a stale put wrote nothing");

        // The right expectation with an updatedAt that is not later.
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T2), Some(T2))), not_later());
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T1), Some(T2))), not_later());
        // Instants compare, not spellings: the same instant at another offset is not later.
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, "2026-10-10T12:00:01+02:00"), Some(T2))), not_later());
        assert_eq!(store.rows(), rows);
        assert_eq!(receipts(&mind), base + 2);
        committed(put(&mut mind, persona(INSTANCE, "2026-10-10T12:00:02+02:00"), Some(T2)));
        assert_eq!(receipts(&mind), base + 3);

        // The second put's receipt read the first document and wrote the second.
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == second).unwrap();
        assert_eq!((receipt.strong_reads.len(), receipt.writes.len()), (1, 1));
        assert_eq!(receipt.strong_reads[0].document_key, INSTANCE);
        assert_ne!(receipt.strong_reads[0].payload_msgpack, receipt.writes[0].payload_msgpack);
        assert_eq!(receipt.provenance, provenance(Faculty::Hands));
    }

    #[test]
    fn a_put_whose_swap_is_lost_is_stale_and_changes_nothing() {
        let store = RefusingStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        committed(put(&mut mind, persona(INSTANCE, T1), None));
        let rows = store.rows();
        store.command(SwapCommand::Lose);
        assert_eq!(refused(put(&mut mind, persona(INSTANCE, T2), Some(T1))), stale(Some(T1)));
        assert_eq!(store.rows(), rows);
    }

    #[test]
    fn the_persona_is_not_a_pipeline_document_and_a_batch_cannot_carry_it() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        let documents = mind.status(crate::wire::IndexStatus::Current).documents;
        committed(put(&mut mind, persona(INSTANCE, T1), None));
        assert_eq!(mind.status(crate::wire::IndexStatus::Current).documents, documents);
        assert!(PipelineKind::ALL.iter().all(|kind| kind.type_id() != HuginnPersonaEntry::TYPE));
        let forged = mind.persona_envelope().unwrap().clone();
        let instance = mind.instance().clone();
        let outcome = mind.admit_prepared(&instance, provenance(Faculty::Hands), vec![forged], crate::fixtures::now());
        assert!(matches!(outcome, crate::admission::PipelineAdmissionOutcome::Refused(_)), "{outcome:?}");
    }

    #[test]
    fn one_persona_per_mind_and_the_store_still_opens() {
        let yggdrasil = slug(INSTANCE);
        let cache = crate::mind::schema_cache().unwrap();
        let entry = |key: &str, value: Value| cache.prepare_entry_named(key, &HuginnPersonaEntry { value }).unwrap().0;
        let planted = |rows: Vec<CultCacheEnvelope>| {
            let store = MemoryStore::new();
            rows.into_iter().for_each(|row| store.plant(row));
            store
        };
        let base = || vec![epoch(), prepare(&instance(INSTANCE))];

        // A store written before this type existed opens unchanged.
        assert!(Mind::open_with(planted(base()), &yggdrasil).is_ok());

        // One persona opens and reads back.
        let mut with_one = base();
        with_one.push(entry(INSTANCE, persona(INSTANCE, T1)));
        let mind = Mind::open_with(planted(with_one.clone()), &yggdrasil).unwrap();
        assert_eq!(mind.envelopes().len(), 3);
        assert_eq!(persona_in_status(&mind).as_deref(), Some(T1));

        // Two are no mind's.
        with_one.push(entry("second", persona(INSTANCE, T2)));
        assert_eq!(
            Mind::open_with(planted(with_one), &yggdrasil).err(),
            Some(MindRefusal::ForeignStore { r#type: HuginnPersonaEntry::TYPE.into() })
        );
        // The opener still refuses a type that is not one of the seventeen.
        let mut stranger = base();
        let mut foreign = entry(INSTANCE, persona(INSTANCE, T1));
        foreign.r#type = "gamecult.persona_state.v1".into();
        stranger.push(foreign);
        assert!(matches!(Mind::open_with(planted(stranger), &yggdrasil).err(), Some(MindRefusal::ForeignStore { .. })));
    }

    #[test]
    fn the_stored_bytes_are_the_one_slot_array_the_catalogue_describes() {
        let store = MemoryStore::new();
        let mut mind = seeded_over(&store);
        committed(put(&mut mind, persona(INSTANCE, T1), None));
        let envelope = mind.persona_envelope().unwrap();
        assert_eq!((envelope.r#type.as_str(), envelope.key.as_str()), ("gamecult.persona_state.v0", INSTANCE));
        let slots: (Value,) = rmp_serde::from_slice(&envelope.payload).unwrap();
        assert_eq!(slots.0, persona(INSTANCE, T1));
    }
}
