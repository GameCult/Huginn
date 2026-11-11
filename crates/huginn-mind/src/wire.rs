//! The organ's request and response vocabulary: one operation per `Mind`
//! method and `whoami`. Payloads are the admission and read types unchanged.
//! The daemon carries these in CultNet's operation envelope; the client
//! constructs them. Nothing here validates a document, derives a status or
//! names a transport.

use cultcache_rs::DatabaseEntry;
use cultnet_rs::{CultMeshCdnArtifactManifest, Selection};
use eureka_pipeline::{Line, OrgRepo, PIPELINE_SCHEMA_EPOCH, PipelineKind, PipelineRef, Short, Slug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::admission::{PipelineAdmissionBatch, PipelineAdmissionOutcome};
use crate::mind::Mind;
use crate::persona::{PersonaPutOutcome, PersonaStateView};
use crate::query::{PipelineDocumentView, PipelineSelectionPage, RootTouched, SemanticQuery};
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
    /// What others did in `repos` on the forge since the `after` cursor: the
    /// daemon reads the forge and no document of the mind, and answers
    /// `Refused(Unavailable)` when it has no forge or the forge does not answer
    /// in time. The repos are the asker's to name (her stewardships); the
    /// instance's own login is dropped from the answer.
    StewardedActivity { instance: Slug, repos: Vec<OrgRepo>, after: Option<u64> },
    /// Every root the mind holds a document under, with its newest admission and a campaign's repos in force: the mind's own
    /// records, one pass at the head.
    RootsTouched { instance: Slug },
}

/// The most repos one activity read names.
pub const ACTIVITY_REPOS_MAX: usize = 64;

/// What a pull-request event did. The words are the forge's op names, nothing
/// a person wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PrAction {
    Opened,
    Merged,
    Closed,
    Reopened,
    Commented,
    Approved,
    Rejected,
    AutoMerge,
}

/// What an event was. No field carries text a person wrote: a title, a comment
/// or a commit message is another person's word and has no place here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RepoChange {
    /// A push to the repo's default branch of `commits` commits.
    Pushed { commits: u32 },
    Pr { number: u32, action: PrAction },
}

/// One thing someone else did in a repo of the asker's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoEvent {
    /// The forge's own monotonic feed id.
    pub id: u64,
    /// When it happened, Unix milliseconds.
    pub at: u64,
    pub repo: OrgRepo,
    /// The forge login that did it.
    pub actor: Short,
    pub change: RepoChange,
}

/// A pull request open on the forge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenPr {
    pub repo: OrgRepo,
    pub number: u32,
    pub author: Short,
    /// Unix milliseconds.
    pub updated_at: u64,
}

/// The answer to an activity read. `events` are other people's changes only,
/// the asker's own login already dropped, and on a read with no `after` only
/// those of the last 72 hours: the reader filters nothing again. `head` is the
/// newest feed id the read saw in those repos, whether or not any event
/// survived the filter, an entry older than the window too: the cursor to ask
/// `after` next. `capped` names the repos whose feed the read stopped reading at its page limit: their older entries were not read.
/// `open_prs` comes only on a read with no `after`; `unseen` and `moved` come on every read, because a repo can go dark in the middle of a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoActivity {
    pub head: u64,
    pub events: Vec<RepoEvent>,
    pub capped: Vec<OrgRepo>,
    pub open_prs: Vec<OpenPr>,
    /// Repos asked about that the anonymous read cannot see: their feed answered 404 or a redirect, and on a read with no `after` the org listing does not hold them either. A repo the listing holds whose feed is dark, and a read with an `after` in which every repo is dark, are not told here: they fail the read.
    pub unseen: Vec<OrgRepo>,
    /// Of those, the ones whose feed answered a redirect to the forge's own feed route for another repo. The redirect is not followed; a `Location` that is anything else names nothing.
    pub moved: Vec<RepoMoved>,
}

/// A redirect the forge answered for a repo's feed: it points at `to`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoMoved {
    pub from: OrgRepo,
    pub to: OrgRepo,
}

/// The most hits a source search answers.
pub const SOURCE_TOP_K_MAX: u32 = 50;

/// The most texts one source search takes.
pub const SOURCE_TEXTS_MAX: usize = 8;

/// A source search: the texts to find chunks near (one to `SOURCE_TEXTS_MAX`,
/// each a topic; several answer with their ranked lists fused), how many,
/// optionally the one forge repo (by name) to look in, and whether to look
/// only in repos' front-page files (`FRONT_PAGE_FILES` at the repo root). An
/// old client's request has no `front` and means false.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceQuery {
    /// Transitional: an old client's single text, read as `texts` of one. Exactly
    /// one of `text` and `texts` is given. To be deleted once no client sends it.
    #[serde(default)]
    pub text: Line,
    /// The topics to search by, one to `SOURCE_TEXTS_MAX`.
    #[serde(default)]
    pub texts: Vec<Line>,
    pub top_k: u32,
    pub repo: Option<Short>,
    #[serde(default)]
    pub front: bool,
}

impl SourceQuery {
    /// The door: exactly one of `text` and `texts`, each text saying something,
    /// at most `SOURCE_TEXTS_MAX` texts, and between one and
    /// `SOURCE_TOP_K_MAX` hits. A refusal names the field.
    pub fn check(&self) -> Result<(), MindRefusal> {
        let refused = |field: &str, value: Option<String>, message: &str| MindRefusal::SelectionInvalid {
            field: field.into(),
            value,
            message: message.into(),
        };
        if !self.text.0.is_empty() && !self.texts.is_empty() {
            return Err(refused("texts", Some(self.texts.len().to_string()), "a source search takes `text` or `texts`, not both"));
        }
        if self.texts.is_empty() && self.text.0.is_empty() {
            return Err(refused("texts", None, "a source search needs texts to find"));
        }
        if self.texts.is_empty() && self.text.0.trim().is_empty() {
            return Err(refused("text", None, "a source search needs text to find"));
        }
        if self.texts.len() > SOURCE_TEXTS_MAX {
            return Err(refused("texts", Some(self.texts.len().to_string()), "a source search takes 1 to 8 texts"));
        }
        if self.texts.iter().any(|text| text.0.trim().is_empty()) {
            return Err(refused("texts", None, "every text of a source search says something"));
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
    /// Cosine similarity for a search of one text; the reciprocal-rank sum
    /// (k = 60) over the texts' lists for a search of several.
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
            Self::StewardedActivity { .. } => "stewarded_activity",
            Self::RootsTouched { .. } => "roots_touched",
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
            | Self::SearchSources { instance, .. }
            | Self::StewardedActivity { instance, .. }
            | Self::RootsTouched { instance } => Some(instance),
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
    Activity(RepoActivity),
    RootsTouched(Vec<RootTouched>),
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
        let count = |type_id: &str| self.envelopes.get(type_id).map_or(0, |keyed| keyed.len() as u32);
        let receipts = count(HuginnCommitReceipt::TYPE);
        let documents = PipelineKind::ALL.iter().map(|kind| count(kind.type_id())).sum();
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
                query: SourceQuery { text: Line("reconcile".into()), texts: vec![], top_k: 5, repo: Some(Short("Huginn".into())), front: true },
            },
            HuginnMindRequest::SearchSources {
                instance: slug(INSTANCE),
                query: SourceQuery { text: Line::default(), texts: vec![Line("reconcile".into()), Line("fuse".into())], top_k: 5, repo: None, front: false },
            },
            HuginnMindRequest::RootsTouched { instance: slug(INSTANCE) },
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
            HuginnMindResponse::RootsTouched(seeded().roots_touched().unwrap()),
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
        assert_eq!(names, ["whoami", "admit", "view", "query", "persona_get", "persona_put", "search_sources", "search_sources", "roots_touched"]);
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
            texts: vec![],
            top_k,
            repo: repo.map(|repo| Short(repo.into())),
            front: false,
        };
        let field = |query: SourceQuery| match query.check() {
            Err(MindRefusal::SelectionInvalid { field, .. }) => field,
            other => panic!("expected a refusal, got {other:?}"),
        };
        assert_eq!(query("x", 1, None).check(), Ok(()));
        assert_eq!(query("x", SOURCE_TOP_K_MAX, Some("Huginn")).check(), Ok(()));
        assert_eq!(field(query("  ", 5, None)), "text");
        let many = |texts: &[&str]| SourceQuery { text: Line::default(), texts: texts.iter().map(|text| Line((*text).into())).collect(), ..query("", 5, None) };
        assert_eq!(many(&["a", "b"]).check(), Ok(()));
        assert_eq!(many(&["a"; SOURCE_TEXTS_MAX]).check(), Ok(()));
        assert_eq!(field(query("", 5, None)), "texts", "neither text nor texts");
        assert_eq!(field(many(&[])), "texts");
        assert_eq!(field(SourceQuery { text: Line("x".into()), ..many(&["a"]) }), "texts", "both");
        assert_eq!(field(many(&["a", " "])), "texts", "a blank entry");
        match many(&["a"; SOURCE_TEXTS_MAX + 1]).check() {
            Err(MindRefusal::SelectionInvalid { field, value, .. }) => assert_eq!((field.as_str(), value.as_deref()), ("texts", Some("9"))),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(field(query("x", 0, None)), "top_k");
        assert_eq!(field(query("x", SOURCE_TOP_K_MAX + 1, None)), "top_k");
        assert_eq!(field(query("x", 5, Some(" "))), "repo");
    }

    /// An old client's search has no `front`: it decodes as the whole-index
    /// search it asked for.
    #[test]
    fn a_source_query_encoded_without_front_decodes_as_false() {
        #[derive(Serialize)]
        struct OldQuery {
            text: Line,
            top_k: u32,
            repo: Option<Short>,
        }
        let old = OldQuery { text: Line("x".into()), top_k: 3, repo: None };
        let bytes = rmp_serde::to_vec_named(&old).unwrap();
        assert!(!rmp_serde::from_slice::<SourceQuery>(&bytes).unwrap().front);
    }

    /// An old client's search has `text` and no `texts`: it decodes as the one
    /// text it sent, and passes the door.
    #[test]
    fn a_source_query_encoded_without_texts_decodes_as_its_one_text() {
        #[derive(Serialize)]
        struct OldQuery {
            text: Line,
            top_k: u32,
            repo: Option<Short>,
            front: bool,
        }
        let old = OldQuery { text: Line("x".into()), top_k: 3, repo: None, front: false };
        let bytes = rmp_serde::to_vec_named(&old).unwrap();
        let query = rmp_serde::from_slice::<SourceQuery>(&bytes).unwrap();
        assert_eq!((query.text.0.as_str(), query.texts.len(), query.check()), ("x", 0, Ok(())));
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

    /// The desk's two published files are the derivation of its two types, byte for byte, and
    /// the catalog index lists both as wire messages beside the mind's.
    #[test]
    fn published_desk_schemas_match_derivation() {
        use crate::desk_wire::{DESK_REQUEST_SCHEMA, DESK_REQUEST_SCHEMA_JSON, DESK_RESPONSE_SCHEMA, DESK_RESPONSE_SCHEMA_JSON, DeskRequest, DeskResponse};
        let request = serde_json::to_string_pretty(&schemars::schema_for!(DeskRequest)).unwrap();
        let response = serde_json::to_string_pretty(&schemars::schema_for!(DeskResponse)).unwrap();
        assert_eq!(DESK_REQUEST_SCHEMA_JSON.replace("\r\n", "\n"), request);
        assert_eq!(DESK_RESPONSE_SCHEMA_JSON.replace("\r\n", "\n"), response);
        // The deciders bound is in the published schema, not only in the decoder.
        assert!(request.contains("\"maxItems\": 64") && request.contains("\"uniqueItems\": true") && request.contains("\"minItems\": 1"), "deciders bound");
        let index: serde_json::Value = serde_json::from_str(include_str!("../../../schemas/cultnet/index.json")).unwrap();
        for (id, title) in [(DESK_REQUEST_SCHEMA, "Huginn desk request"), (DESK_RESPONSE_SCHEMA, "Huginn desk response")] {
            let rows: Vec<_> = index["schemas"].as_array().unwrap().iter().filter(|row| row["schemaId"] == id).collect();
            assert_eq!(rows.len(), 1, "{id}");
            let row = rows[0];
            assert_eq!(
                (row["kind"].as_str(), row["schemaVersion"].as_str(), row["title"].as_str(), row["path"].as_str(), row["wireContracts"][0].as_str()),
                (Some("wire_message"), Some(id), Some(title), Some(format!("{id}.schema.json").as_str()), Some("cultnet.schema.v0"))
            );
        }
    }
}
