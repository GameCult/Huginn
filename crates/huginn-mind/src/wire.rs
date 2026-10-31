//! The organ's request and response vocabulary: one operation per `Mind`
//! method and `whoami`. Payloads are the admission and read types unchanged.
//! The daemon carries these in CultNet's operation envelope; the client
//! constructs them. Nothing here validates a document, derives a status or
//! names a transport.

use cultcache_rs::DatabaseEntry;
use cultnet_rs::{CultMeshCdnArtifactManifest, Selection};
use eureka_pipeline::{Line, PIPELINE_SCHEMA_EPOCH, PipelineKind, PipelineRef, Short, Slug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::admission::{PipelineAdmissionBatch, PipelineAdmissionOutcome};
use crate::mind::Mind;
use crate::persona::{PersonaPutOutcome, PersonaStateView};
use crate::query::{PipelineDocumentView, PipelineSelectionPage, SemanticQuery};
use crate::receipt::{HuginnCommitReceipt, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The envelope's `service_id`: one service, one mind, whichever mind the
/// process opened.
pub const MIND_SERVICE_ID: &str = "huginn.mind";
pub const MIND_REQUEST_SCHEMA: &str = "huginn.mind_request.v1";
pub const MIND_RESPONSE_SCHEMA: &str = "huginn.mind_response.v1";

/// The largest answer that is deferred rather than refused, measured on the
/// encoded payload. The daemon refuses past it and a client caps its fetch at
/// it, so both sides read one number.
pub const MAX_DEFERRED_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// The published JSON schema of each wire type, as the catalog advertises it.
/// The files are derived from the types below and pinned byte for byte by
/// `published_wire_schemas_match_derivation`, so the catalog cannot advertise
/// a shape the crate does not speak.
pub const MIND_REQUEST_SCHEMA_JSON: &str =
    include_str!("../../../schemas/cultnet/huginn.mind_request.v1.schema.json");
pub const MIND_RESPONSE_SCHEMA_JSON: &str =
    include_str!("../../../schemas/cultnet/huginn.mind_response.v1.schema.json");

/// The request's `selection` is CultNet's, and its published schema is
/// CultLib's: a `$ref` to the `$id` it publishes, so one owner holds the
/// selection's shape and nothing here copies it.
fn selection_ref(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "$ref": "https://github.com/GameCult/CultLib/contracts/cultnet/cultnet.selection.schema.json"
    })
}

/// The deferred answer's manifest is CultMesh's own type, a positional array
/// the C# reference defines (`CultMeshCdnArtifactManifest`, schema version
/// `gamecult.mesh.cdn_artifact_manifest.v1`). CultLib publishes no JSON schema
/// for it, so the schema says what the reference says: an array of its ten
/// fields in order, and nothing here copies their meaning.
fn manifest_array(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "description": "gamecult.mesh.cdn_artifact_manifest.v1, the C# reference's CultMeshCdnArtifactManifest as its positional array: [artifactId, kind, version, contentHash, sizeBytes, mimeType, createdAtUtc, chunks[[chunkHash, offset, sizeBytes, recordKey]], tags, metadata].",
        "type": "array"
    })
}

/// Every operation a mind answers. `Admit` carries the batch whole because the
/// batch already names the instance and the asker; a second `instance` beside
/// it would be two declarations. Every read names the instance because ruling
/// 14 refuses another mind's identity whatever the transport, and a read of
/// the wrong mind is that same collision on the read side. `Whoami` names
/// none: it is how a client learns which mind it reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HuginnMindRequest {
    Whoami,
    Admit(PipelineAdmissionBatch),
    View { instance: Slug, id: PipelineRef },
    Query {
        instance: Slug,
        #[schemars(schema_with = "selection_ref")]
        selection: Selection,
        semantic: Option<SemanticQuery>,
    },
    /// The mind's persona document, if one is stored.
    PersonaGet { instance: Slug },
    /// Replaces the mind's persona document whole. `expected_updated_at` is
    /// the stored document's `updatedAt`, none for the first put. The
    /// provenance is the asker's, as for `Admit`.
    PersonaPut {
        instance: Slug,
        provenance: PipelineProvenance,
        state: Value,
        expected_updated_at: Option<String>,
    },
    /// Semantic search over the source index: the public forge repos' code and
    /// docs, which no mind holds. The daemon embeds the text and asks the
    /// index it does not write.
    SearchSources { instance: Slug, query: SourceQuery },
}

/// The most hits a source search answers.
pub const SOURCE_TOP_K_MAX: u32 = 50;

/// A source search: the text to find chunks near, how many, and optionally the
/// one forge repo (by name) to look in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceQuery {
    pub text: Line,
    pub top_k: u32,
    pub repo: Option<Short>,
}

impl SourceQuery {
    /// The door: text that says something, and between one and
    /// `SOURCE_TOP_K_MAX` hits. A refusal names the field.
    pub fn check(&self) -> Result<(), MindRefusal> {
        let refused = |field: &str, value: Option<String>, message: &str| MindRefusal::SelectionInvalid {
            field: field.into(),
            value,
            message: message.into(),
        };
        if self.text.0.trim().is_empty() {
            return Err(refused("text", None, "a source search needs text to find"));
        }
        if !(1..=SOURCE_TOP_K_MAX).contains(&self.top_k) {
            return Err(refused("top_k", Some(self.top_k.to_string()), "a source search asks for 1 to 50 hits"));
        }
        if self.repo.as_ref().is_some_and(|repo| repo.0.trim().is_empty()) {
            return Err(refused("repo", None, "a repo filter names a repo or is absent"));
        }
        Ok(())
    }
}

/// The model a source collection was built under, and so the one a query must
/// be embedded by. The address of the embedder is no part of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceModel {
    pub name: String,
    pub digest: String,
}

/// One chunk the index holds, as the index holds it: the repo it belongs to,
/// the path in that repo, the commit whose blob it was cut from, its 1-based
/// inclusive line range, and its text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SourceHit {
    pub repo: String,
    pub path: String,
    pub commit: String,
    pub line_start: u32,
    pub line_end: u32,
    pub score: f32,
    pub text: String,
}

/// The answer to a source search, nearest first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SourceHits {
    pub indexed_under: SourceModel,
    pub hits: Vec<SourceHit>,
}

impl HuginnMindRequest {
    /// The envelope's `operation`.
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Whoami => "whoami",
            Self::Admit(_) => "admit",
            Self::View { .. } => "view",
            Self::Query { .. } => "query",
            Self::PersonaGet { .. } => "persona_get",
            Self::PersonaPut { .. } => "persona_put",
            Self::SearchSources { .. } => "search_sources",
        }
    }

    /// The instance the request declares; `Whoami` declares none.
    pub fn instance(&self) -> Option<&Slug> {
        match self {
            Self::Whoami => None,
            Self::Admit(batch) => Some(&batch.instance),
            Self::View { instance, .. }
            | Self::Query { instance, .. }
            | Self::PersonaGet { instance }
            | Self::PersonaPut { instance, .. }
            | Self::SearchSources { instance, .. } => Some(instance),
        }
    }
}

/// Every answer a mind gives. A refusal is data in the response, never a
/// transport failure: the read side carries `Refused`, the write side carries
/// the outcome's own `Refused`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum HuginnMindResponse {
    Whoami(MindStatus),
    Admit(PipelineAdmissionOutcome),
    View(Option<Box<PipelineDocumentView>>),
    Query(PipelineSelectionPage),
    Refused(MindRefusal),
    Persona(Option<PersonaStateView>),
    PersonaPut(PersonaPutOutcome),
    Sources(SourceHits),
    /// The answer did not fit one send and is being held for the client to
    /// fetch. Nothing in a mind produces this: the daemon's serve module does,
    /// in place of an answer, and the answer it stands for is the named
    /// MessagePack of the `HuginnMindResponse` the mind gave.
    Deferred(DeferredAnswer),
}

/// Where the body of an answer that did not fit one send can be fetched: the
/// reference's own manifest, whose `contentHash` is the SHA-256 of the answer's
/// encoded bytes and whose chunks are asked for, one at a time, on the session
/// this arrived on. Row-level facts come from asking again with a header
/// projection; there is no summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeferredAnswer {
    #[schemars(schema_with = "manifest_array")]
    pub manifest: CultMeshCdnArtifactManifest,
}

impl HuginnMindResponse {
    /// The envelope's `status`, in the C# reference's vocabulary: `rejected`
    /// exactly for a refusal, `accepted` otherwise. `Conflict` and
    /// `AlreadyAdmitted` are answers about the mind, not refusals of the
    /// request. A `Deferred` is nominally `accepted`; the daemon sets the
    /// envelope of a deferred answer to the status of the answer it stands for.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Refused(_)
            | Self::Admit(PipelineAdmissionOutcome::Refused(_))
            | Self::PersonaPut(PersonaPutOutcome::Refused(_)) => "rejected",
            _ => "accepted",
        }
    }
}

/// How far the semantic index has caught up with the mind. The mind decides
/// none of it: the daemon's index worker reports it, in memory, and a restart
/// recomputes it. `pending` counts documents not yet known to be in the index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IndexStatus {
    /// The worker has not yet compared the mind with the collection.
    Reconciling { pending: u32 },
    /// Every indexable document is in the collection.
    Current,
    /// Reconciled, and indexing what is still pending.
    Behind { pending: u32 },
    /// The last attempt failed (the embedder or the vector store is
    /// unreachable, or refused); it is retried with a backoff and nothing is
    /// dropped. `attempts` counts consecutive failures.
    Failing { pending: u32, attempts: u32, error: String },
    /// The collection belongs to another writer. The worker never recreates
    /// it; it re-reads it on the same backoff.
    Refused { pending: u32, attempts: u32, reason: String },
}

/// What a mind says about itself: the typed state a dashboard projects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MindStatus {
    pub instance: Slug,
    pub schema_epoch: String,
    /// Pipeline documents in the image; the epoch record and the receipts are
    /// not documents and are not counted here.
    pub documents: u32,
    pub receipts: u32,
    pub index: IndexStatus,
    /// The persona door as `PersonaGet` would find it.
    pub persona: PersonaStatus,
}

/// What the persona door holds, in the one vocabulary the door itself answers
/// in: nothing yet, a document with this `updatedAt`, or a stored document the
/// door cannot read (`PersonaGet` and every put answer `Unavailable`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PersonaStatus {
    Absent,
    Stored { updated_at: String },
    Unreadable,
}

impl<S: MindStore> Mind<S> {
    /// `whoami`, derived from the image every time it is asked, and from the
    /// index status the caller holds: the mind stores neither.
    pub fn status(&self, index: IndexStatus) -> MindStatus {
        let mut documents = 0_u32;
        let mut receipts = 0_u32;
        for envelope in self.envelopes() {
            if envelope.r#type == HuginnCommitReceipt::TYPE {
                receipts += 1;
            } else if PipelineKind::ALL.iter().any(|kind| kind.type_id() == envelope.r#type) {
                documents += 1;
            }
        }
        MindStatus {
            instance: self.instance().clone(),
            schema_epoch: PIPELINE_SCHEMA_EPOCH.to_string(),
            documents,
            receipts,
            index,
            persona: match self.persona() {
                Ok(None) => PersonaStatus::Absent,
                Ok(Some(view)) => PersonaStatus::Stored { updated_at: view.updated_at },
                Err(_) => PersonaStatus::Unreadable,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::PipelinePageItems;
    use crate::fixtures::{CAMPAIGN, INSTANCE, OTHER_INSTANCE, instance, provenance, r, seeded, slug};
    use crate::receipt::Faculty;

    fn requests() -> Vec<HuginnMindRequest> {
        let batch = PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Hands),
            documents: vec![instance(INSTANCE)],
        };
        vec![
            HuginnMindRequest::Whoami,
            HuginnMindRequest::Admit(batch),
            HuginnMindRequest::View { instance: slug(INSTANCE), id: r(PipelineKind::Campaign, CAMPAIGN) },
            HuginnMindRequest::Query { instance: slug(OTHER_INSTANCE), selection: Selection::default(), semantic: None },
            HuginnMindRequest::PersonaGet { instance: slug(INSTANCE) },
            HuginnMindRequest::PersonaPut {
                instance: slug(INSTANCE),
                provenance: provenance(Faculty::Hands),
                state: serde_json::json!({ "updatedAt": "2026-10-10T10:00:00Z", "n": [1, 2.5, null, "x"] }),
                expected_updated_at: Some("2026-10-10T09:00:00Z".into()),
            },
            HuginnMindRequest::SearchSources {
                instance: slug(INSTANCE),
                query: SourceQuery { text: Line("reconcile".into()), top_k: 5, repo: Some(Short("Huginn".into())) },
            },
        ]
    }

    fn responses() -> Vec<HuginnMindResponse> {
        let one_row = seeded().query(&Selection { limit: Some(1), ..Selection::default() }).unwrap().items;
        let refusal = MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() };
        vec![
            HuginnMindResponse::Whoami(seeded().status(IndexStatus::Current)),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: "r1".into() }),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict { identities: vec![] }),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal.clone())),
            HuginnMindResponse::View(None),
            HuginnMindResponse::Query(PipelineSelectionPage {
                matched: 0,
                as_of: 1,
                next: None,
                items: PipelinePageItems::Headers(vec![]),
                edges: None,
                scores: None,
            }),
            HuginnMindResponse::Query(PipelineSelectionPage {
                matched: 1,
                as_of: 1,
                next: None,
                items: one_row,
                edges: None,
                scores: Some(vec![0.5]),
            }),
            HuginnMindResponse::Refused(refusal),
            HuginnMindResponse::Persona(None),
            HuginnMindResponse::Persona(Some(PersonaStateView {
                value: serde_json::json!({ "personaId": INSTANCE }),
                updated_at: "2026-10-10T10:00:00Z".into(),
                receipt_id: "mind-commit-1".into(),
            })),
            HuginnMindResponse::PersonaPut(PersonaPutOutcome::Committed { receipt_id: "mind-commit-2".into() }),
            HuginnMindResponse::PersonaPut(PersonaPutOutcome::Refused(MindRefusal::PersonaNotCanonical)),
            HuginnMindResponse::Sources(SourceHits {
                indexed_under: SourceModel { name: "m".into(), digest: "d".into() },
                hits: vec![SourceHit {
                    repo: "Huginn".into(),
                    path: "a.rs".into(),
                    commit: "abc".into(),
                    line_start: 1,
                    line_end: 2,
                    score: 0.5,
                    text: "x".into(),
                }],
            }),
            HuginnMindResponse::Deferred(DeferredAnswer {
                manifest: cultnet_rs::pack_content("huginn.mind_response", "package", "", "", "", b"body", 4)
                    .unwrap()
                    .0,
            }),
        ]
    }

    /// The vocabulary is the mind's methods, the declared instance is on every
    /// request that names a mind, and the envelope's status is derived from
    /// the response rather than chosen by whoever sends it.
    #[test]
    fn the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response() {
        let names: Vec<&str> = requests().iter().map(|request| request.operation()).collect();
        assert_eq!(names, ["whoami", "admit", "view", "query", "persona_get", "persona_put", "search_sources"]);
        for request in requests() {
            let declared = request.instance().is_none();
            assert_eq!(declared, matches!(request, HuginnMindRequest::Whoami), "{request:?}");
            let bytes = rmp_serde::to_vec_named(&request).unwrap();
            assert_eq!(rmp_serde::from_slice::<HuginnMindRequest>(&bytes).unwrap(), request);
        }
        assert_eq!(
            requests()[3].instance(),
            Some(&slug(OTHER_INSTANCE)),
            "the request's instance is the one it declares, not the mind's"
        );

        for response in responses() {
            let rejected = matches!(
                response,
                HuginnMindResponse::Refused(_)
                    | HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(_))
                    | HuginnMindResponse::PersonaPut(PersonaPutOutcome::Refused(_))
            );
            assert_eq!(response.status(), if rejected { "rejected" } else { "accepted" }, "{response:?}");
            let bytes = rmp_serde::to_vec_named(&response).unwrap();
            assert_eq!(rmp_serde::from_slice::<HuginnMindResponse>(&bytes).unwrap(), response);
        }
    }

    /// A source search is refused before any index is asked when it says
    /// nothing, asks for no hits or more than the cap, or names a blank repo.
    #[test]
    fn a_source_query_without_text_or_within_one_to_fifty_hits_is_refused_by_field() {
        let query = |text: &str, top_k: u32, repo: Option<&str>| SourceQuery {
            text: Line(text.into()),
            top_k,
            repo: repo.map(|repo| Short(repo.into())),
        };
        let field = |query: SourceQuery| match query.check() {
            Err(MindRefusal::SelectionInvalid { field, .. }) => field,
            other => panic!("expected a refusal, got {other:?}"),
        };
        assert_eq!(query("x", 1, None).check(), Ok(()));
        assert_eq!(query("x", SOURCE_TOP_K_MAX, Some("Huginn")).check(), Ok(()));
        assert_eq!(field(query("  ", 5, None)), "text");
        assert_eq!(field(query("x", 0, None)), "top_k");
        assert_eq!(field(query("x", SOURCE_TOP_K_MAX + 1, None)), "top_k");
        assert_eq!(field(query("x", 5, Some(" "))), "repo");
    }

    /// A new client reads an old daemon's page: the five fields it had, no
    /// `scores`. This is what keeps rollback by flip safe.
    #[test]
    fn a_page_encoded_without_scores_decodes_with_none() {
        #[derive(Serialize)]
        struct OldPage {
            matched: u32,
            as_of: u64,
            next: Option<String>,
            items: PipelinePageItems,
            edges: Option<Vec<crate::query::PipelineEdge>>,
        }
        let old = OldPage { matched: 0, as_of: 7, next: None, items: PipelinePageItems::Headers(vec![]), edges: None };
        let bytes = rmp_serde::to_vec_named(&old).unwrap();
        let page = rmp_serde::from_slice::<PipelineSelectionPage>(&bytes).unwrap();
        assert_eq!((page.matched, page.as_of, page.scores), (0, 7, None));
    }

    /// The two files the catalog publishes are the derivation of the two
    /// types, byte for byte.
    #[test]
    fn published_wire_schemas_match_derivation() {
        let request = serde_json::to_string_pretty(&schemars::schema_for!(HuginnMindRequest)).unwrap();
        let response = serde_json::to_string_pretty(&schemars::schema_for!(HuginnMindResponse)).unwrap();
        assert_eq!(MIND_REQUEST_SCHEMA_JSON.replace("\r\n", "\n"), request);
        assert_eq!(MIND_RESPONSE_SCHEMA_JSON.replace("\r\n", "\n"), response);
        // The selection is CultLib's, referenced by the `$id` it publishes and
        // not copied into this schema.
        let selection = "https://github.com/GameCult/CultLib/contracts/cultnet/cultnet.selection.schema.json";
        assert!(request.contains(&format!("\"$ref\": \"{selection}\"")));
        assert!(!request.contains("\"cursor\""), "no second copy of the selection's shape");
    }
}
