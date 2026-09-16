//! The commit receipt: what a batch read and what it wrote, byte for byte.
//!
//! This is the bounded duplicate of Epiphany's Mind receipt (FU-3), smaller
//! by design: no authority enum, no companion documents, no `invariant_owner`,
//! no `store_id`. Provenance is a field on the receipt, not a companion, and
//! the digest excludes it, so an exact replay across sessions and faculties
//! is idempotent. If a third consumer appears, extract the primitive into
//! CultLib rather than copy it again.
//!
//! Digest: `receipt_id = "mind-commit-" + hex(sha256(msgpack_named((instance,
//! strong_reads, writes))))`, where each list is `DocumentVersion`s in
//! `(document_type, document_key)` order and a version carries the exact
//! payload bytes and their SHA-256. `committed_at` and `provenance` are not
//! digested.

use chrono::{DateTime, SecondsFormat, Utc};
use cultcache_rs::{CultCacheEnvelope, DatabaseEntry};
use epiphany_pipeline::{PipelineKind, PipelineRef, Short, Slug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mind::{Mind, unavailable};
use crate::refusal::MindRefusal;
use crate::store::MindStore;

pub const RECEIPT_SCHEMA_VERSION: &str = "huginn.mind_commit_receipt.v1";

/// Which faculty a batch was admitted as. Attribution, not authority
/// (ruling 18): no admission rule trusts it, and Cut 9's views read it back
/// from the receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Faculty {
    SelfFaculty,
    Imagination,
    Hands,
    Soul,
    MindSteward,
    Eyes,
    Operator,
}

/// Who admitted a batch: the faculty, the agent, its session and the tool.
/// A field on the receipt and nothing else; the digest excludes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineProvenance {
    pub faculty: Faculty,
    pub agent: epiphany_pipeline::Short,
    pub session: epiphany_pipeline::Short,
    pub tool: epiphany_pipeline::Short,
}

/// The exact bytes of one stored document, as read or as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentVersion {
    pub document_type: String,
    pub document_key: String,
    pub schema_id: Option<String>,
    pub payload_msgpack: Vec<u8>,
    pub payload_sha256: String,
}

impl DocumentVersion {
    pub fn from_envelope(envelope: &CultCacheEnvelope) -> Self {
        Self {
            document_type: envelope.r#type.clone(),
            document_key: envelope.key.clone(),
            schema_id: envelope.schema_id.clone(),
            payload_msgpack: envelope.payload.clone(),
            payload_sha256: sha256_hex(&envelope.payload),
        }
    }

    pub fn identity(&self) -> (&str, &str) {
        (&self.document_type, &self.document_key)
    }

    fn validate(&self) -> Result<(), MindRefusal> {
        if self.document_type.is_empty() || self.document_key.is_empty() {
            return Err(integrity("a document version names no identity"));
        }
        if self.payload_sha256 != sha256_hex(&self.payload_msgpack) {
            return Err(integrity(&format!(
                "document {}/{} payload digest mismatch",
                self.document_type, self.document_key
            )));
        }
        Ok(())
    }
}

/// The receipt of one admitted batch, stored beside the documents it names
/// and keyed by its id.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "huginn.mind_commit_receipt.v1", schema = "HuginnCommitReceipt")]
pub struct HuginnCommitReceipt {
    #[cultcache(key = 0)]
    pub schema_version: String,
    #[cultcache(key = 1)]
    pub receipt_id: String,
    #[cultcache(key = 2)]
    pub instance: String,
    #[cultcache(key = 3)]
    pub provenance: PipelineProvenance,
    #[cultcache(key = 4)]
    pub strong_reads: Vec<DocumentVersion>,
    #[cultcache(key = 5)]
    pub writes: Vec<DocumentVersion>,
    #[cultcache(key = 6)]
    pub committed_at: String,
}

impl HuginnCommitReceipt {
    /// The digest, from what was read and written and never from who wrote
    /// it or when.
    pub fn digest(&self) -> Result<String, MindRefusal> {
        let bytes = rmp_serde::to_vec_named(&(&self.instance, &self.strong_reads, &self.writes))
            .map_err(|error| integrity(&format!("receipt digest input does not encode: {error}")))?;
        Ok(format!("mind-commit-{}", sha256_hex(&bytes)))
    }

    pub fn validate(&self) -> Result<(), MindRefusal> {
        if self.schema_version != RECEIPT_SCHEMA_VERSION {
            return Err(integrity(&format!("receipt schema version {:?} is foreign", self.schema_version)));
        }
        if self.instance.is_empty() {
            return Err(integrity("receipt names no instance"));
        }
        if self.writes.is_empty() {
            return Err(integrity("receipt writes nothing"));
        }
        for version in self.strong_reads.iter().chain(&self.writes) {
            version.validate()?;
        }
        if !unique_identities(&self.strong_reads) || !unique_identities(&self.writes) {
            return Err(integrity("receipt names an identity twice"));
        }
        if self.receipt_id != self.digest()? {
            return Err(integrity(&format!("receipt {} does not match its digest", self.receipt_id)));
        }
        Ok(())
    }
}

/// How a commit ended. An exact replay is decided by admission before the
/// commit is attempted, so it is not an exit of the primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CommitOutcome {
    /// The batch and its receipt landed whole.
    Committed(HuginnCommitReceipt),
    /// The swap lost: a strong read changed underneath, or a write's identity
    /// already existed. Named where the kind is a pipeline kind.
    Conflict(Vec<PipelineRef>),
}

/// The receipt a batch would land with: its id is the digest of the exact
/// bytes read and written, computed before anything is stored so admission
/// can look up an exact replay. This is the one construction site of a
/// receipt.
pub(crate) fn candidate(
    instance: &Slug,
    provenance: PipelineProvenance,
    strong_reads: &[CultCacheEnvelope],
    writes: &[CultCacheEnvelope],
    now: DateTime<Utc>,
) -> Result<HuginnCommitReceipt, MindRefusal> {
    let mut receipt = HuginnCommitReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
        receipt_id: String::new(),
        instance: instance.0.clone(),
        provenance,
        strong_reads: versions(strong_reads),
        writes: versions(writes),
        committed_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    receipt.receipt_id = receipt.digest()?;
    Ok(receipt)
}

/// The versions of a set of envelopes in identity order, so the digest does
/// not depend on the order a batch was assembled in.
fn versions(envelopes: &[CultCacheEnvelope]) -> Vec<DocumentVersion> {
    let mut versions = envelopes.iter().map(DocumentVersion::from_envelope).collect::<Vec<_>>();
    versions.sort_by(|left, right| left.identity().cmp(&right.identity()));
    versions
}

/// The stored receipt with the candidate's id, if the batch already landed:
/// its reads and writes must equal the candidate's, or the store holds a
/// receipt this binary cannot vouch for.
pub(crate) fn replay<S: MindStore>(
    mind: &Mind<S>,
    candidate: &HuginnCommitReceipt,
) -> Result<Option<HuginnCommitReceipt>, MindRefusal> {
    let Some(envelope) = mind.raw_envelope(HuginnCommitReceipt::TYPE, &candidate.receipt_id) else {
        return Ok(None);
    };
    let existing: HuginnCommitReceipt = rmp_serde::from_slice(&envelope.payload)
        .map_err(|error| integrity(&format!("receipt {} does not decode: {error}", candidate.receipt_id)))?;
    existing.validate()?;
    if existing.strong_reads != candidate.strong_reads || existing.writes != candidate.writes {
        return Err(integrity(&format!("receipt {} is stored with other content", candidate.receipt_id)));
    }
    Ok(Some(existing))
}

/// Lands a batch whole, or not at all: one compare-and-swap with the strong
/// reads as its expectation and the writes plus the receipt as its
/// replacements. The image is re-pulled either way; a lost swap is diffed
/// against the fresh image and typed `Conflict`.
pub(crate) fn commit<S: MindStore>(
    mind: &mut Mind<S>,
    receipt: HuginnCommitReceipt,
    strong_reads: Vec<CultCacheEnvelope>,
    writes: Vec<CultCacheEnvelope>,
) -> Result<CommitOutcome, MindRefusal> {
    receipt.validate()?;
    let receipt_envelope = mind
        .cache()
        .prepare_entry_named(&receipt.receipt_id, &receipt)
        .map(|(envelope, _)| envelope)
        .map_err(unavailable)?;
    // The store requires every expected identity to be replaced, so each
    // strong read is re-inserted unchanged beside the writes.
    let mut replacements = writes.clone();
    replacements.extend(strong_reads.iter().cloned());
    replacements.push(receipt_envelope);
    let expected: &[CultCacheEnvelope] = &strong_reads;
    let landed = MindStore::compare_and_swap_batch(mind.store(), expected, replacements).map_err(unavailable)?;
    mind.refresh()?;
    if !landed {
        let changed = strong_reads
            .iter()
            .filter(|read| mind.raw_envelope(&read.r#type, &read.key) != Some(read))
            .chain(writes.iter().filter(|write| mind.raw_envelope(&write.r#type, &write.key).is_some()));
        let identities = changed
            .filter_map(|envelope| {
                let kind = PipelineKind::ALL.iter().find(|kind| kind.type_id() == envelope.r#type)?;
                Some(PipelineRef { kind: *kind, id: Short(envelope.key.clone()) })
            })
            .collect();
        return Ok(CommitOutcome::Conflict(identities));
    }
    Ok(CommitOutcome::Committed(receipt))
}

fn unique_identities(versions: &[DocumentVersion]) -> bool {
    let identities = versions.iter().map(DocumentVersion::identity).collect::<std::collections::BTreeSet<_>>();
    identities.len() == versions.len()
}

fn integrity(detail: &str) -> MindRefusal {
    MindRefusal::Unavailable { detail: detail.into() }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::fixtures::{INSTANCE, admit, committed, id, instance, opened, prepare, question, seed};
    use crate::store::test_stores::MemoryStore;
    use cultcache_rs::{CacheBackingStore, PushAllOptions};
    use epiphany_pipeline::{PipelineDocument, PipelineKind, PipelineRefusal};

    /// A memory store that keeps the replacement set of every swap it was
    /// asked for, so a test can count the commit's calls as well as read
    /// their contents.
    #[derive(Clone)]
    struct RecordingStore {
        inner: MemoryStore,
        calls: Arc<Mutex<Vec<Vec<CultCacheEnvelope>>>>,
    }

    impl RecordingStore {
        fn new() -> Self {
            Self { inner: MemoryStore::new(), calls: Arc::new(Mutex::new(Vec::new())) }
        }

        fn forget(&self) {
            self.calls.lock().unwrap().clear();
        }

        fn calls(&self) -> Vec<Vec<CultCacheEnvelope>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CacheBackingStore for RecordingStore {
        fn pull_all(&self) -> anyhow::Result<Vec<CultCacheEnvelope>> {
            self.inner.pull_all()
        }

        fn push(&mut self, entry: &CultCacheEnvelope) -> anyhow::Result<()> {
            self.inner.push(entry)
        }

        fn delete(&mut self, entry: &CultCacheEnvelope) -> anyhow::Result<()> {
            self.inner.delete(entry)
        }

        fn push_all(&mut self, entries: &[CultCacheEnvelope], options: PushAllOptions) -> anyhow::Result<()> {
            self.inner.push_all(entries, options)
        }
    }

    impl MindStore for RecordingStore {
        fn compare_and_swap_batch(
            &self,
            expected: &[CultCacheEnvelope],
            replacements: Vec<CultCacheEnvelope>,
        ) -> anyhow::Result<bool> {
            self.calls.lock().unwrap().push(replacements.clone());
            self.inner.compare_and_swap_batch(expected, replacements)
        }
    }

    /// The receipt is part of the batch's own swap, never a second one after
    /// it: a receipt written separately could land while the documents did
    /// not, or the reverse.
    #[test]
    fn a_commit_is_one_swap_carrying_the_documents_and_the_receipt_together() {
        let store = RecordingStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        store.forget();
        let (receipt_id, _) = committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let calls = store.calls();
        assert_eq!(calls.len(), 1, "one commit, one swap");
        let landed = calls[0].iter().map(|entry| (entry.r#type.clone(), entry.key.clone())).collect::<Vec<_>>();
        assert_eq!(landed.len(), 2, "{landed:?}");
        assert!(landed.contains(&(PipelineKind::Question.type_id().to_string(), id("question", "Q1"))), "{landed:?}");
        assert!(landed.contains(&(HuginnCommitReceipt::TYPE.to_string(), receipt_id)), "{landed:?}");
    }

    /// S5 closed against the real type: the organ's own receipt in a mind's
    /// store is never decoded as a pipeline document.
    #[test]
    fn decode_refuses_the_organs_own_receipt() {
        let mut envelope = prepare(&instance(INSTANCE));
        envelope.r#type = HuginnCommitReceipt::TYPE.into();
        assert_eq!(
            PipelineDocument::decode(&envelope),
            Err(PipelineRefusal::ForeignStore { r#type: "huginn.mind_commit_receipt.v1".into() })
        );
    }
}
