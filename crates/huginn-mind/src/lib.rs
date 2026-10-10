//! `huginn-mind`: one instance's mind as typed CultCache state.
//!
//! The crate owns three decisions and nothing else. `Mind::open` owns "may
//! this store be this instance's mind": the opener refuses a foreign type, a
//! foreign epoch, a missing identity or a foreign identity before it attaches
//! anything, and takes the store's per-path lock first. `receipt::commit` owns "did a batch enter, whole, with a
//! receipt": one compare-and-swap over the store, a receipt naming the exact
//! bytes read and written, and a typed conflict when the swap loses.
//! `Mind::admit_prepared` owns "may this batch enter": every cross-field and
//! cross-document rule, every derived write, and the replay check, in one
//! ordered path that `admit`, the daemon's sink and the hand-off all share.
//!
//! The read side derives status, joins the receipts and answers one typed
//! selection (CultNet's, over rows this crate supplies) through the same
//! `docs` the rules use; nothing is stored for it.
//!
//! The wire vocabulary and its operation envelope (`envelope`) are here too,
//! so the daemon and the client share one set
//! of types.
//!
//! Document shape, bounds, formats and keys are `eureka-pipeline`'s; the
//! organ registers, prepares, decodes and validates through the leaf's four
//! doors and never re-derives a key. The store is CultLib's owned redb
//! CultCache, whose lifetime-long exclusive lock is the single-writer
//! invariant's mechanism. No function here reads a clock or the environment:
//! `now` is passed in.

pub mod admission;
mod docs;
pub mod envelope;
pub mod index;
pub mod mind;
pub mod persona;
mod rows;
pub mod query;
pub mod receipt;
pub mod refusal;
pub mod store;
pub mod wire;

#[cfg(test)]
pub(crate) mod fixtures;

pub use docs::CitationRole;
pub use index::{INDEX_TEXT_MAX_BYTES, INDEX_TEXT_VERSION, IndexEntry, index_text};
pub use admission::{BATCH_MAX, PipelineAdmissionBatch, PipelineAdmissionOutcome};
pub use mind::{HuginnMindEpoch, Mind};
pub use persona::{PersonaPutOutcome, PersonaStateView};
pub use query::{
    AdmissionFacts, PipelineDocumentSummary, PipelineDocumentView, PipelineEdge, PipelineFacts, PipelinePageItems,
    PipelineSelectionPage, PipelineStatus, PipelineStatusSummary, Ranked, SUMMARY_MAX_BYTES, SemanticQuery,
};
pub use receipt::{DocumentVersion, Faculty, HuginnCommitReceipt, PipelineProvenance, RECEIPT_SCHEMA_VERSION};
pub use refusal::MindRefusal;
pub use store::{MindStore, OwnedRedbMessagePackBackingStore};
pub use wire::{
    DeferredAnswer, HuginnMindRequest, HuginnMindResponse, IndexStatus, MAX_DEFERRED_BODY_BYTES, MIND_REQUEST_SCHEMA, MIND_REQUEST_SCHEMA_JSON,
    MIND_RESPONSE_SCHEMA, MIND_RESPONSE_SCHEMA_JSON, MIND_SERVICE_ID, MindStatus,
};

/// The leaf, whole, through the one crate that pins its rev: the daemon and
/// the client name `Slug`, `PipelineRef` and the document types from here, so
/// one `[dependencies]` entry decides which revision the workspace speaks.
pub use eureka_pipeline;
