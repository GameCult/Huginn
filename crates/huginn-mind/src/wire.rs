//! The organ's request and response vocabulary: one operation per `Mind`
//! method and `whoami`. Payloads are the admission and read types unchanged.
//! The daemon carries these in CultNet's operation envelope; the client
//! constructs them. Nothing here validates a document, derives a status or
//! names a transport.

use cultcache_rs::DatabaseEntry;
use epiphany_pipeline::{PIPELINE_SCHEMA_EPOCH, PipelineKind, PipelineRef, Slug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::admission::{PipelineAdmissionBatch, PipelineAdmissionOutcome};
use crate::mind::Mind;
use crate::query::{PipelineDocumentView, PipelineQuery, PipelineQueryPage};
use crate::receipt::HuginnCommitReceipt;
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The envelope's `service_id`: one service, one mind, whichever mind the
/// process opened.
pub const MIND_SERVICE_ID: &str = "huginn.mind";
pub const MIND_REQUEST_SCHEMA: &str = "huginn.mind_request.v1";
pub const MIND_RESPONSE_SCHEMA: &str = "huginn.mind_response.v1";

/// The published JSON schema of each wire type, as the catalog advertises it.
/// The files are derived from the types below and pinned byte for byte by
/// `published_wire_schemas_match_derivation`, so the catalog cannot advertise
/// a shape the crate does not speak.
pub const MIND_REQUEST_SCHEMA_JSON: &str =
    include_str!("../../../schemas/cultnet/huginn.mind_request.v1.schema.json");
pub const MIND_RESPONSE_SCHEMA_JSON: &str =
    include_str!("../../../schemas/cultnet/huginn.mind_response.v1.schema.json");

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
    Query { instance: Slug, query: PipelineQuery },
}

impl HuginnMindRequest {
    /// The envelope's `operation`.
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Whoami => "whoami",
            Self::Admit(_) => "admit",
            Self::View { .. } => "view",
            Self::Query { .. } => "query",
        }
    }

    /// The instance the request declares; `Whoami` declares none.
    pub fn instance(&self) -> Option<&Slug> {
        match self {
            Self::Whoami => None,
            Self::Admit(batch) => Some(&batch.instance),
            Self::View { instance, .. } | Self::Query { instance, .. } => Some(instance),
        }
    }
}

/// Every answer a mind gives. A refusal is data in the response, never a
/// transport failure: the read side carries `Refused`, the write side carries
/// the outcome's own `Refused`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HuginnMindResponse {
    Whoami(MindStatus),
    Admit(PipelineAdmissionOutcome),
    View(Option<PipelineDocumentView>),
    Query(PipelineQueryPage),
    Refused(MindRefusal),
}

impl HuginnMindResponse {
    /// The envelope's `status`, in the C# reference's vocabulary: `rejected`
    /// exactly for a refusal, `accepted` otherwise. `Conflict` and
    /// `AlreadyAdmitted` are answers about the mind, not refusals of the
    /// request.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Refused(_) | Self::Admit(PipelineAdmissionOutcome::Refused(_)) => "rejected",
            _ => "accepted",
        }
    }
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
}

impl<S: MindStore> Mind<S> {
    /// `whoami`, derived from the image every time it is asked. Nothing stores
    /// a status and no configuration supplies one.
    pub fn status(&self) -> MindStatus {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            HuginnMindRequest::Query { instance: slug(OTHER_INSTANCE), query: PipelineQuery::default() },
        ]
    }

    fn responses() -> Vec<HuginnMindResponse> {
        let refusal = MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() };
        vec![
            HuginnMindResponse::Whoami(seeded().status()),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: "r1".into() }),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict { identities: vec![] }),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal.clone())),
            HuginnMindResponse::View(None),
            HuginnMindResponse::Query(PipelineQueryPage { items: vec![], matched: 0 }),
            HuginnMindResponse::Refused(refusal),
        ]
    }

    /// The vocabulary is the mind's methods, the declared instance is on every
    /// request that names a mind, and the envelope's status is derived from
    /// the response rather than chosen by whoever sends it.
    #[test]
    fn the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response() {
        let names: Vec<&str> = requests().iter().map(|request| request.operation()).collect();
        assert_eq!(names, ["whoami", "admit", "view", "query"]);
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
                HuginnMindResponse::Refused(_) | HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(_))
            );
            assert_eq!(response.status(), if rejected { "rejected" } else { "accepted" }, "{response:?}");
            let bytes = rmp_serde::to_vec_named(&response).unwrap();
            assert_eq!(rmp_serde::from_slice::<HuginnMindResponse>(&bytes).unwrap(), response);
        }
    }

    /// The two files the catalog publishes are the derivation of the two
    /// types, byte for byte.
    #[test]
    fn published_wire_schemas_match_derivation() {
        let request = serde_json::to_string_pretty(&schemars::schema_for!(HuginnMindRequest)).unwrap();
        let response = serde_json::to_string_pretty(&schemars::schema_for!(HuginnMindResponse)).unwrap();
        assert_eq!(MIND_REQUEST_SCHEMA_JSON.replace("\r\n", "\n"), request);
        assert_eq!(MIND_RESPONSE_SCHEMA_JSON.replace("\r\n", "\n"), response);
    }
}
