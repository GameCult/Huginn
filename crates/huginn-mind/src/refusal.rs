//! Typed refusals of the organ. A refusal is data with a field an agent can
//! act on, never a transport error: the outcome carries it, the wire (Cut 10)
//! serialises it, the tools (Cut 13) show it.

use epiphany_pipeline::{PipelineKind, PipelineRefusal};
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
    Document(
        #[serde(with = "DocumentRefusal")]
        #[schemars(with = "DocumentRefusal")]
        PipelineRefusal,
    ),
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
    CitesResolvedDocument { kind: PipelineKind, id: String },
    EmptySupersession,
    UnknownSupersessor { id: String },
    RevisionWithoutSupersession { kind: PipelineKind, revision: u32 },
    DuplicateLabel { field: String, label: String },
    InvalidOptions { question: String },
    InvalidChoice { ruling: String, choice: String },
    QuoteWithoutOperator { ruling: String },
    EmptyRepos { campaign: String },
    RepoNotStewarded { repo: String },
    RepoNotInCampaign { repo: String },
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
    Unavailable { detail: String },
}

/// The leaf's `PipelineRefusal` at the pinned rev derives neither `Serialize`
/// nor `JsonSchema`. This is serde's remote definition of it: the same four
/// variants, field for field, compiled against the leaf's type, so a leaf
/// change that renames or adds a field stops this crate building rather than
/// drifting. It is a projection for the wire, not a second vocabulary; the
/// refusal itself is still the leaf's value. When the leaf derives the two
/// traits this goes.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(remote = "PipelineRefusal")]
#[allow(dead_code)]
enum DocumentRefusal {
    FieldBound { field: String, limit: u32, actual: u32 },
    InvalidFormat { field: String, value: String },
    InvalidIdentity { kind: PipelineKind, key: String, expected: String },
    ForeignStore { r#type: String },
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
