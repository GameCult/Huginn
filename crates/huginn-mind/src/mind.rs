//! The mind: one instance's store, opened fail-closed, and its image.
//!
//! Ruling 14: a store is canonical to exactly one instance, and the identity
//! lives in the state, in the `instance` document, not in a path. The path
//! under the state root is derived from the slug for convenience; the
//! document is the authority, and a store moved to another instance's
//! directory is refused. Ruling 15: one writer, the owned redb store's
//! lifetime-long exclusive lock. Ruling 20's analogue: the opener validates
//! every gate before it attaches, so a refused store is never read into an
//! image.

use std::path::{Path, PathBuf};

use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, OwnedRedbMessagePackBackingStore};
use epiphany_pipeline::{
    PIPELINE_SCHEMA_EPOCH, PipelineDocument, PipelineKind, Slug, register_pipeline_document_types,
};

use crate::receipt::HuginnCommitReceipt;
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The store's record of the schema epoch it was written at, keyed by the
/// epoch string. Derived by admission on the first write; read by the opener,
/// which refuses a store written at any other epoch (`ForeignEpoch`) so a
/// breaking schema bump refuses the old store instead of misreading it.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "huginn.mind_epoch.v1", schema = "HuginnMindEpoch")]
pub struct HuginnMindEpoch {
    #[cultcache(key = 0)]
    pub schema_epoch: String,
}

impl HuginnMindEpoch {
    /// The one envelope the record ever has: the current epoch, keyed by
    /// itself. Admission appends it to the first write.
    pub(crate) fn envelope(cache: &CultCache) -> Result<CultCacheEnvelope, MindRefusal> {
        let record = Self { schema_epoch: PIPELINE_SCHEMA_EPOCH.to_string() };
        cache
            .prepare_entry_named(PIPELINE_SCHEMA_EPOCH, &record)
            .map(|(envelope, _)| envelope)
            .map_err(unavailable)
    }
}

/// A store or cache error, carried as data.
pub(crate) fn unavailable(error: anyhow::Error) -> MindRefusal {
    MindRefusal::Unavailable { detail: format!("{error:#}") }
}

/// A cache that knows the fifteen types a mind's store may hold: the leaf's
/// thirteen through its registrar, the epoch record and the commit receipt.
/// It is the cache every envelope is prepared against.
pub(crate) fn schema_cache() -> Result<CultCache, MindRefusal> {
    let mut cache = CultCache::new();
    register_pipeline_document_types(&mut cache).map_err(unavailable)?;
    cache.register_entry_type::<HuginnMindEpoch>().map_err(unavailable)?;
    cache.register_entry_type::<HuginnCommitReceipt>().map_err(unavailable)?;
    Ok(cache)
}

fn is_known_type(type_id: &str) -> bool {
    type_id == HuginnMindEpoch::TYPE
        || type_id == HuginnCommitReceipt::TYPE
        || PipelineKind::ALL.iter().any(|kind| kind.type_id() == type_id)
}

/// One instance's mind: the declared instance, the store, the registered
/// cache attached to it, and the image (the store's envelopes as last pulled,
/// in identity order). The image is a cache of the store and is re-pulled
/// after every commit; nothing writes it directly.
pub struct Mind<S: MindStore> {
    instance: Slug,
    store: S,
    cache: CultCache,
    image: Vec<CultCacheEnvelope>,
}

impl Mind<OwnedRedbMessagePackBackingStore> {
    /// `<state_root>/minds/<instance>/mind.redb`. Derived for convenience;
    /// the `instance` document inside is the identity.
    pub fn path_for(state_root: &Path, instance: &Slug) -> PathBuf {
        state_root.join("minds").join(&instance.0).join("mind.redb")
    }

    /// Opens the instance's mind under the state root, taking the store's
    /// exclusive lock for the mind's lifetime. A second owner of the same
    /// path, in this process or another, is `MindAlreadyOwned`. An empty
    /// store is a valid open; only the first admission may write into it.
    pub fn open(state_root: &Path, instance: &Slug) -> Result<Self, MindRefusal> {
        let path = Self::path_for(state_root, instance);
        let store = OwnedRedbMessagePackBackingStore::new(&path).map_err(|error| {
            // The store reports a held lock as an error like any other; its
            // message is the only signal, so it is matched here, once.
            if format!("{error:#}").contains("already has an active owner") {
                MindRefusal::MindAlreadyOwned { path: path.display().to_string() }
            } else {
                unavailable(error)
            }
        })?;
        Self::open_with(store, instance)
    }
}

impl<S: MindStore> Mind<S> {
    /// Fail-closed, in this order, nothing attached until every step passes:
    /// pull the raw envelopes; every type is one of the fifteen; if anything
    /// is stored, exactly one epoch record at the current epoch and exactly
    /// one `instance` document naming the declared instance; then register,
    /// attach and pull.
    pub fn open_with(store: S, instance: &Slug) -> Result<Self, MindRefusal> {
        let raw = store.pull_all().map_err(unavailable)?;
        refuse_foreign_types(&raw)?;
        refuse_foreign_epoch(&raw)?;
        refuse_foreign_identity(&raw, instance)?;
        let (cache, image) = attach(store.clone())?;
        Ok(Self { instance: instance.clone(), store, cache, image })
    }

    pub fn instance(&self) -> &Slug {
        &self.instance
    }

    /// Ruling 14 across every transport: the declared instance is this mind's,
    /// else `ForeignInstance { declared, mind }`. A1 for admission; the daemon
    /// asks it for every read that names an instance, and compares nothing
    /// itself.
    pub fn require_instance(&self, declared: &Slug) -> Result<(), MindRefusal> {
        if declared != self.instance() {
            return Err(MindRefusal::ForeignInstance {
                declared: declared.0.clone(),
                mind: self.instance().0.clone(),
            });
        }
        Ok(())
    }

    /// The image: every envelope the store held at the last pull, in
    /// identity order. Cut 10's snapshot source reads this.
    pub fn envelopes(&self) -> &[CultCacheEnvelope] {
        &self.image
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty()
    }

    /// The stored bytes of one document, by kind and id.
    pub fn envelope(&self, kind: PipelineKind, id: &str) -> Option<&CultCacheEnvelope> {
        self.raw_envelope(kind.type_id(), id)
    }

    /// One document, decoded through the leaf.
    pub fn get(&self, kind: PipelineKind, id: &str) -> Result<Option<PipelineDocument>, MindRefusal> {
        self.envelope(kind, id)
            .map(|envelope| PipelineDocument::decode(envelope).map_err(MindRefusal::Document))
            .transpose()
    }

    /// Every commit receipt in the image, in receipt-id order.
    pub fn receipts(&self) -> Result<Vec<HuginnCommitReceipt>, MindRefusal> {
        self.image
            .iter()
            .filter(|envelope| envelope.r#type == HuginnCommitReceipt::TYPE)
            .map(|envelope| {
                rmp_serde::from_slice::<HuginnCommitReceipt>(&envelope.payload).map_err(|error| {
                    MindRefusal::Unavailable { detail: format!("receipt {} does not decode: {error}", envelope.key) }
                })
            })
            .collect()
    }

    pub(crate) fn raw_envelope(&self, type_id: &str, key: &str) -> Option<&CultCacheEnvelope> {
        self.image.iter().find(|envelope| envelope.r#type == type_id && envelope.key == key)
    }

    pub(crate) fn cache(&self) -> &CultCache {
        &self.cache
    }

    pub(crate) fn store(&self) -> &S {
        &self.store
    }

    /// Re-pulls the image from the store through the attached cache.
    pub(crate) fn refresh(&mut self) -> Result<(), MindRefusal> {
        self.cache.pull_all_backing_stores().map_err(unavailable)?;
        self.image = snapshot(&self.cache);
        Ok(())
    }
}

/// Step 2: a runtime store, a Mind store or any other file passed by mistake
/// dies on its first foreign type.
fn refuse_foreign_types(raw: &[CultCacheEnvelope]) -> Result<(), MindRefusal> {
    if let Some(foreign) = raw.iter().find(|envelope| !is_known_type(&envelope.r#type)) {
        return Err(MindRefusal::ForeignStore { r#type: foreign.r#type.clone() });
    }
    Ok(())
}

/// Step 3: a non-empty store carries exactly one epoch record, keyed by the
/// epoch it names, at the epoch this binary writes. A store with no epoch
/// record at all is `MissingIdentity`; every other defect is `ForeignEpoch`,
/// whose `found` names the defect: the stored epoch when the value is
/// foreign, the record's key when a record is keyed by anything else, and the
/// record count rendered as `"<n> records"` when more than one is stored. The
/// count is decided before any value is read, so a current record beside a
/// foreign one refuses the same way whichever the store returns first.
fn refuse_foreign_epoch(raw: &[CultCacheEnvelope]) -> Result<(), MindRefusal> {
    if raw.is_empty() {
        return Ok(());
    }
    let foreign = |found: String| MindRefusal::ForeignEpoch { found, expected: PIPELINE_SCHEMA_EPOCH.into() };
    let mut records = raw.iter().filter(|envelope| envelope.r#type == HuginnMindEpoch::TYPE);
    let Some(record) = records.next() else {
        return Err(MindRefusal::MissingIdentity);
    };
    let extra = records.count();
    if extra > 0 {
        return Err(foreign(format!("{} records", extra + 1)));
    }
    if record.key != PIPELINE_SCHEMA_EPOCH {
        return Err(foreign(record.key.clone()));
    }
    let record: HuginnMindEpoch = rmp_serde::from_slice(&record.payload)
        .map_err(|error| MindRefusal::Unavailable { detail: format!("epoch record does not decode: {error}") })?;
    if record.schema_epoch != PIPELINE_SCHEMA_EPOCH {
        return Err(foreign(record.schema_epoch));
    }
    Ok(())
}

/// Step 4: a non-empty store carries exactly one `instance` document, and it
/// names the declared instance. This is where a store moved to another
/// instance's directory is refused.
fn refuse_foreign_identity(raw: &[CultCacheEnvelope], instance: &Slug) -> Result<(), MindRefusal> {
    if raw.is_empty() {
        return Ok(());
    }
    let mut documents = raw.iter().filter(|envelope| envelope.r#type == PipelineKind::Instance.type_id());
    let (Some(envelope), None) = (documents.next(), documents.next()) else {
        return Err(MindRefusal::MissingIdentity);
    };
    let PipelineDocument::Instance(stored) = PipelineDocument::decode(envelope)? else {
        return Err(MindRefusal::MissingIdentity);
    };
    if stored.instance != *instance {
        return Err(MindRefusal::ForeignInstance { declared: instance.0.clone(), mind: stored.instance.0 });
    }
    Ok(())
}

/// Step 5: the registered cache, the store as its one generic home, and the
/// image pulled through it.
fn attach<S: MindStore>(store: S) -> Result<(CultCache, Vec<CultCacheEnvelope>), MindRefusal> {
    let mut cache = schema_cache()?;
    cache.add_generic_backing_store(store).map_err(unavailable)?;
    cache.pull_all_backing_stores().map_err(unavailable)?;
    let image = snapshot(&cache);
    Ok((cache, image))
}

fn snapshot(cache: &CultCache) -> Vec<CultCacheEnvelope> {
    let mut image = cache.snapshot_envelopes();
    image.sort_by(|left, right| (&left.r#type, &left.key).cmp(&(&right.r#type, &right.key)));
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{INSTANCE, epoch, instance, prepare, slug};
    use crate::store::test_stores::MemoryStore;
    use cultcache_rs::CacheBackingStore;

    fn planted(rows: Vec<CultCacheEnvelope>) -> MemoryStore {
        let store = MemoryStore::new();
        for row in rows {
            store.plant(row);
        }
        store
    }

    fn foreign(type_id: &str) -> CultCacheEnvelope {
        let mut envelope = epoch();
        envelope.r#type = type_id.into();
        envelope
    }

    const FOREIGN_EPOCH: &str = "epiphany.pipeline.epoch.v0";

    /// An epoch record under an arbitrary key naming an arbitrary epoch, so a
    /// key defect and a value defect can be planted apart.
    fn epoch_record(key: &str, schema_epoch: &str) -> CultCacheEnvelope {
        let cache = schema_cache().unwrap();
        let record = HuginnMindEpoch { schema_epoch: schema_epoch.into() };
        cache.prepare_entry_named(key, &record).unwrap().0
    }

    fn foreign_epoch() -> CultCacheEnvelope {
        epoch_record(FOREIGN_EPOCH, FOREIGN_EPOCH)
    }

    #[test]
    fn the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching() {
        let yggdrasil = slug(INSTANCE);
        let cases: Vec<(Vec<CultCacheEnvelope>, MindRefusal)> = vec![
            // Step 2 before everything: a runtime store passed by mistake.
            (
                vec![foreign("epiphany.runtime_spine.v47"), epoch(), prepare(&instance(INSTANCE))],
                MindRefusal::ForeignStore { r#type: "epiphany.runtime_spine.v47".into() },
            ),
            // Step 3 before step 4: a foreign epoch with no identity at all.
            // Its key is foreign too, which is what refuses it here.
            (
                vec![foreign_epoch()],
                MindRefusal::ForeignEpoch { found: FOREIGN_EPOCH.into(), expected: PIPELINE_SCHEMA_EPOCH.into() },
            ),
            // Step 3: the value, under the one key the record may have.
            (
                vec![epoch_record(PIPELINE_SCHEMA_EPOCH, FOREIGN_EPOCH), prepare(&instance(INSTANCE))],
                MindRefusal::ForeignEpoch { found: FOREIGN_EPOCH.into(), expected: PIPELINE_SCHEMA_EPOCH.into() },
            ),
            // Step 3: the current epoch under any other key is not the epoch
            // record, whatever it says about itself.
            (
                vec![epoch_record("not-the-epoch", PIPELINE_SCHEMA_EPOCH), prepare(&instance(INSTANCE))],
                MindRefusal::ForeignEpoch { found: "not-the-epoch".into(), expected: PIPELINE_SCHEMA_EPOCH.into() },
            ),
            // Step 3: two records are no record, counted before either value
            // is read.
            (
                vec![epoch(), epoch_record("second", PIPELINE_SCHEMA_EPOCH), prepare(&instance(INSTANCE))],
                MindRefusal::ForeignEpoch { found: "2 records".into(), expected: PIPELINE_SCHEMA_EPOCH.into() },
            ),
            // Step 3: an identity without an epoch record.
            (vec![prepare(&instance(INSTANCE))], MindRefusal::MissingIdentity),
            // Step 4: an epoch record without an identity.
            (vec![epoch()], MindRefusal::MissingIdentity),
            // Step 4: two identities are no identity.
            (
                vec![epoch(), prepare(&instance(INSTANCE)), prepare(&instance("thought-cage"))],
                MindRefusal::MissingIdentity,
            ),
        ];
        for (rows, expected) in cases {
            let store = planted(rows);
            let before = store.rows();
            let refusal = Mind::open_with(store.clone(), &yggdrasil).err().expect("refused");
            assert_eq!(refusal, expected);
            assert_eq!(store.pull_count(), 1, "{expected:?}: refused before attaching");
            assert_eq!(store.rows(), before, "{expected:?}: bytes unchanged");
        }
        // A current record beside a foreign one is refused on the count, not
        // on whichever of the two the store happens to return first.
        let pair = (epoch(), foreign_epoch());
        for rows in [vec![pair.0.clone(), pair.1.clone()], vec![pair.1, pair.0]] {
            assert_eq!(
                refuse_foreign_epoch(&rows).err(),
                Some(MindRefusal::ForeignEpoch { found: "2 records".into(), expected: PIPELINE_SCHEMA_EPOCH.into() })
            );
        }
        // A store that passes every gate attaches, which is the second pull.
        let store = planted(vec![epoch(), prepare(&instance(INSTANCE))]);
        let mind = Mind::open_with(store.clone(), &yggdrasil).unwrap();
        assert_eq!(store.pull_count(), 2);
        assert_eq!(mind.envelopes().len(), 2);
        assert!(!mind.is_empty());
        let empty = MemoryStore::new();
        assert!(Mind::open_with(empty, &yggdrasil).unwrap().is_empty());
    }

    #[test]
    fn the_instance_document_is_the_identity_not_the_path() {
        // The same bytes opened under another name are refused.
        let store = planted(vec![epoch(), prepare(&instance(INSTANCE))]);
        assert_eq!(
            Mind::open_with(store, &slug("thought-cage")).err(),
            Some(MindRefusal::ForeignInstance { declared: "thought-cage".into(), mind: INSTANCE.into() })
        );

        // A real store copied into another instance's directory is still
        // yggdrasil's mind: it opens as yggdrasil, and not as thought-cage.
        let root = tempfile::tempdir().unwrap();
        let written = Mind::path_for(root.path(), &slug(INSTANCE));
        {
            let mut store = OwnedRedbMessagePackBackingStore::new(&written).unwrap();
            store.push(&epoch()).unwrap();
            store.push(&prepare(&instance(INSTANCE))).unwrap();
        }
        let moved = Mind::path_for(root.path(), &slug("thought-cage"));
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::copy(&written, &moved).unwrap();
        assert_eq!(
            Mind::open(root.path(), &slug("thought-cage")).err(),
            Some(MindRefusal::ForeignInstance { declared: "thought-cage".into(), mind: INSTANCE.into() })
        );
        let store = OwnedRedbMessagePackBackingStore::new(&moved).unwrap();
        let mind = Mind::open_with(store, &slug(INSTANCE)).unwrap();
        assert_eq!(mind.instance(), &slug(INSTANCE));
        assert_eq!(mind.envelopes().len(), 2);
    }

    /// One check, one owner: the value admission refuses a foreign instance
    /// with is the value `require_instance` returns, so the daemon asking it
    /// for a read and `admit_steps` asking it for a write cannot disagree.
    #[test]
    fn require_instance_is_the_one_check_admission_and_the_daemon_share() {
        use crate::fixtures::{OTHER_INSTANCE, now, provenance, seeded};
        use crate::receipt::Faculty;

        let mut mind = seeded();
        let foreign = MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() };
        assert_eq!(mind.require_instance(&slug(OTHER_INSTANCE)).err(), Some(foreign.clone()));
        assert_eq!(mind.require_instance(&slug(INSTANCE)), Ok(()));

        let batch = crate::admission::PipelineAdmissionBatch {
            instance: slug(OTHER_INSTANCE),
            provenance: provenance(Faculty::Hands),
            documents: vec![instance(OTHER_INSTANCE)],
        };
        assert_eq!(mind.admit(batch, now()), crate::admission::PipelineAdmissionOutcome::Refused(foreign));
    }

    #[test]
    fn a_mind_has_one_owner_at_a_time() {
        let root = tempfile::tempdir().unwrap();
        let yggdrasil = slug(INSTANCE);
        let first = Mind::open(root.path(), &yggdrasil).unwrap();
        assert!(first.is_empty());
        let path = Mind::path_for(root.path(), &yggdrasil).display().to_string();
        assert_eq!(
            Mind::open(root.path(), &yggdrasil).err(),
            Some(MindRefusal::MindAlreadyOwned { path: path.clone() })
        );
        drop(first);
        let second = Mind::open(root.path(), &yggdrasil).unwrap();
        assert!(second.is_empty());
        assert_eq!(Mind::path_for(root.path(), &yggdrasil), root.path().join("minds").join(INSTANCE).join("mind.redb"));
    }
}
