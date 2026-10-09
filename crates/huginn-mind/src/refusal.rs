//! Typed refusals of the organ. A refusal is data with a field an agent can
//! act on, never a transport error: the outcome carries it, the wire (Cut 10)
//! serialises it, the tools (Cut 13) show it.

use cultnet_rs::SelectionRefusal;
use eureka_pipeline::{PipelineKind, PipelineRefusal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Every way a mind refuses. `Document` wraps the leaf's four (bounds,
/// formats, key identity, foreign type); the rest are the organ's: store
/// identity, epoch, ownership, batch shape, references, and every cross-field
/// and cross-document rule of admission.
///
/// The two sequenced kinds refuse in the same pair of shapes: an
/// `OutOfSequence` when the writer's `sequence` is not the previous plus one,
/// and `AlreadyResolved`/`AlreadyStewarded` when a record of that scope is
/// still in force.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MindRefusal {
    Document(PipelineRefusal),
    ForeignInstance { declared: String, mind: String },
    MissingIdentity,
    ForeignEpoch { found: String, expected: String },
    ForeignStore { r#type: String },
    MindAlreadyOwned { path: String },
    BatchSize { actual: u32 },
    IdentityCollision { kind: PipelineKind, id: String },
    MissingReference { kind: PipelineKind, id: String },
    AlreadyResolved { subject: String },
    ResolutionOutOfSequence { subject: String, expected: u32, actual: u32 },
    AlreadyStewarded { repo: String },
    StewardshipOutOfSequence { repo: String, expected: u32, actual: u32 },
    IncompatibleResolution { subject_kind: PipelineKind, outcome: String },
    WouldReinstateOverLater { subject: String, later: String },
    CitesResolvedDocument { kind: PipelineKind, id: String },
    EmptySupersession,
    UnknownSupersessor { id: String },
    RevisionWithoutSupersession { kind: PipelineKind, revision: u32 },
    DuplicateLabel { field: String, label: String },
    InvalidOptions { question: String },
    InvalidChoice { ruling: String, choice: String },
    QuoteWithoutOperator { ruling: String },
    EmptyRepos { campaign: String },
    RepoNotInCampaign { repo: String },
    UnknownDependency { cut: String },
    CutReportWithoutSpec { report: String },
    SpecMismatch { field: String },
    RangeOutsideCommits { head: String },
    FalsifiedClaimWithoutConfirmedFinding { claim: String },
    UnprovenClaimWithConfirmedFinding { claim: String },
    PromiseWithoutVerdict { promise: String },
    UnknownMutationLabel { label: String },
    FindingWithoutRange,
    FindingWithoutEvidence,
    UnknownInvariant { label: String },
    NotStewarded { repo: String },
    /// A run claims work another run in force already holds: `item` is the
    /// claimed document's id and `run` the id of the run that holds it.
    AlreadyClaimed { item: String, run: String },
    /// The one refusal a mind never raises: the answer exceeds the largest
    /// body the organ will deliver by any plane, one send or a deferred body.
    /// It lives here because a refusal rides the response schema and there is
    /// one of those, so a client reads it the way it reads every other refusal
    /// rather than parsing a second vocabulary. `bytes` is the encoded answer
    /// (the payload, not its envelope) and `limit` is the largest body the
    /// daemon defers, both so a caller can narrow its own request; nothing is
    /// truncated or paginated on its behalf. The daemon owns it and its
    /// deferral bound decides it; no rule of the mind is involved.
    ResponseTooLarge { bytes: u64, limit: u64 },
    /// A selection the substrate or the organ's value door refused: `field`
    /// is the selection field the client sent, `value` the offending part.
    SelectionInvalid { field: String, value: Option<String>, message: String },
    /// A cursor that does not decode, does not belong to this selection or
    /// was not minted by this process, or names a snapshot this mind has not
    /// reached.
    CursorInvalid { message: String },
    Unavailable { detail: String },
}

impl std::fmt::Display for MindRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "mind refusal: {self:?}")
    }
}

impl std::error::Error for MindRefusal {}

impl From<PipelineRefusal> for MindRefusal {
    fn from(refusal: PipelineRefusal) -> Self {
        Self::Document(refusal)
    }
}

/// The one total mapping from the substrate's refusals, with no wildcard arm.
/// `CursorStale` is unreachable: the organ answers at the cursor's own `asOf`,
/// so a stale refusal is an organ defect and says so. `ReferenceOutsideTarget`
/// is integrity: A7 admits only referents of the declared kind, so a stored
/// edge outside its target means the store is not what admission wrote.
impl From<SelectionRefusal> for MindRefusal {
    fn from(refusal: SelectionRefusal) -> Self {
        match refusal {
            SelectionRefusal::Invalid(invalid) => {
                Self::SelectionInvalid { field: invalid.field, value: invalid.value, message: invalid.message }
            }
            SelectionRefusal::CursorInvalid { message } => Self::CursorInvalid { message },
            SelectionRefusal::CursorStale { as_of, current } => Self::Unavailable {
                detail: format!("cursor_stale: the organ answers at the cursor's asOf ({as_of}), yet was asked at {current}"),
            },
            reference @ SelectionRefusal::ReferenceOutsideTarget { .. } => {
                Self::Unavailable { detail: format!("stored reference outside its declared target: {reference}") }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cultnet_rs::SelectionInvalid;

    /// The mapping is the organ's word on each substrate refusal: a bad
    /// selection and a bad cursor cross as themselves, and the two the organ
    /// is built never to raise cross as integrity faults, not as anything a
    /// client could act on.
    #[test]
    fn each_substrate_refusal_maps_to_one_organ_refusal() {
        let invalid = SelectionInvalid { field: "keys".into(), value: Some("x".into()), message: "no".into() };
        assert_eq!(
            MindRefusal::from(SelectionRefusal::Invalid(invalid)),
            MindRefusal::SelectionInvalid { field: "keys".into(), value: Some("x".into()), message: "no".into() }
        );
        assert_eq!(
            MindRefusal::from(SelectionRefusal::CursorInvalid { message: "bad".into() }),
            MindRefusal::CursorInvalid { message: "bad".into() }
        );
        assert!(matches!(
            MindRefusal::from(SelectionRefusal::CursorStale { as_of: 3, current: 4 }),
            MindRefusal::Unavailable { detail } if detail.contains("cursor_stale")
        ));
        let outside = SelectionRefusal::ReferenceOutsideTarget {
            from_schema_id: "a".into(),
            from_key: "k".into(),
            role: "answers".into(),
            to_schema_id: "b".into(),
            to_key: "j".into(),
        };
        assert!(matches!(MindRefusal::from(outside), MindRefusal::Unavailable { .. }));
    }
}
