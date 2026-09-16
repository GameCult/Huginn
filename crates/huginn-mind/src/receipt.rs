//! The commit receipt: what a batch read and what it wrote, byte for byte.
//!
//! This is the bounded duplicate of `epiphany-core`'s receipt (FU-3), smaller
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

use cultcache_rs::{CultCacheEnvelope, DatabaseEntry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::refusal::MindRefusal;

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
    use super::*;
    use crate::fixtures::{INSTANCE, instance, prepare};
    use epiphany_pipeline::{PipelineDocument, PipelineRefusal};

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
