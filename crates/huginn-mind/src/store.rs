//! The store port. `cultcache-rs` exposes compare-and-swap as inherent methods
//! on its concrete stores, not on `CacheBackingStore`, so the one narrow trait
//! here is how the commit primitive gets it injected: the owned redb store in
//! the daemon, a memory store in every admission test, a refusing store in the
//! atomicity test. Nothing else is abstracted.

use anyhow::Result;
use cultcache_rs::CultCacheEnvelope;

/// The one store a daemon opens. Re-exported so the crate that serves a mind
/// names the store type without declaring `cultcache-rs` a second time: one
/// dependency entry, one rev, one place to move it.
pub use cultcache_rs::OwnedRedbMessagePackBackingStore;

/// CultCache's own row traits, re-exported for the same reason: the crate
/// that serves a mind reaches a store's rows through here rather than
/// declaring `cultcache-rs` a second time. The daemon's store-integrity tests
/// use it to break a row on purpose, over a real store, so a read over the
/// broken store raises the refusal its dispatch arm must carry — but that is
/// not the only door a row already has: `MindStore`'s own supertrait is
/// `CacheBackingStore`, and its public `compare_and_swap_batch` already lets
/// non-test code outside this crate write a row directly. Sealing the store
/// so admission is its only write path is a follow-up, its own cut; this
/// re-export does not widen anything that door does not already reach. It
/// carries two traits, `CacheBackingStore` and `DatabaseEntry`, plus
/// `DatabaseEntry`'s own derive macro, which shares its name in a different
/// namespace and rides along unnamed.
pub use cultcache_rs::{CacheBackingStore, DatabaseEntry};

/// A store a mind can be opened over. `Clone` shares ownership of the same
/// store (the owned redb store's clones share its lock and handle), which is
/// how the cache image and the commit path hold one store between them.
pub trait MindStore: CacheBackingStore + Clone + 'static {
    /// One transactional step: every `expected` row must be stored exactly as
    /// given, no `replacements` row outside `expected` may already exist, and
    /// then every replacement lands, or nothing does and `Ok(false)` says so.
    /// The store's own rule that every expected identity is also replaced is
    /// kept: a strong read is re-inserted unchanged beside the writes.
    fn compare_and_swap_batch(
        &self,
        expected: &[CultCacheEnvelope],
        replacements: Vec<CultCacheEnvelope>,
    ) -> Result<bool>;
}

impl MindStore for OwnedRedbMessagePackBackingStore {
    fn compare_and_swap_batch(
        &self,
        expected: &[CultCacheEnvelope],
        replacements: Vec<CultCacheEnvelope>,
    ) -> Result<bool> {
        OwnedRedbMessagePackBackingStore::compare_and_swap_batch(self, expected, replacements)
    }
}

/// Test stores: a memory store with the redb store's batch semantics, and one
/// that can be told to lose or fail its next swap.
#[cfg(test)]
pub(crate) mod test_stores {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use anyhow::{Result, anyhow};
    use cultcache_rs::{CacheBackingStore, CultCacheEnvelope, PushAllOptions};

    use super::MindStore;

    type Identity = (String, String);

    fn identity(entry: &CultCacheEnvelope) -> Identity {
        (entry.r#type.clone(), entry.key.clone())
    }

    fn unique(entries: &[CultCacheEnvelope], label: &str) -> Result<BTreeSet<Identity>> {
        let ids = entries.iter().map(identity).collect::<BTreeSet<_>>();
        if ids.len() != entries.len() {
            return Err(anyhow!("conditional batch {label} set contains duplicate identities"));
        }
        Ok(ids)
    }

    /// One row per `(type, key)` behind a mutex; clones share it. Counts
    /// `pull_all` calls so the opener test can prove nothing attached.
    #[derive(Clone, Default)]
    pub(crate) struct MemoryStore {
        rows: Arc<Mutex<BTreeMap<Identity, CultCacheEnvelope>>>,
        pulls: Arc<AtomicUsize>,
    }

    impl MemoryStore {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn pull_count(&self) -> usize {
            self.pulls.load(Ordering::SeqCst)
        }

        /// Every row, in identity order: the bytes a test compares before
        /// and after a refused admission.
        pub(crate) fn rows(&self) -> Vec<CultCacheEnvelope> {
            self.rows.lock().unwrap().values().cloned().collect()
        }

        /// Writes a row without the commit path, for the opener tests and the
        /// one replay-planting test.
        pub(crate) fn plant(&self, entry: CultCacheEnvelope) {
            self.rows.lock().unwrap().insert(identity(&entry), entry);
        }
    }

    impl CacheBackingStore for MemoryStore {
        fn pull_all(&self) -> Result<Vec<CultCacheEnvelope>> {
            self.pulls.fetch_add(1, Ordering::SeqCst);
            Ok(self.rows())
        }

        fn push(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
            self.plant(entry.clone());
            Ok(())
        }

        fn delete(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
            self.rows.lock().unwrap().remove(&identity(entry));
            Ok(())
        }

        fn push_all(&mut self, entries: &[CultCacheEnvelope], _options: PushAllOptions) -> Result<()> {
            *self.rows.lock().unwrap() = entries.iter().map(|entry| (identity(entry), entry.clone())).collect();
            Ok(())
        }
    }

    impl MindStore for MemoryStore {
        fn compare_and_swap_batch(
            &self,
            expected: &[CultCacheEnvelope],
            replacements: Vec<CultCacheEnvelope>,
        ) -> Result<bool> {
            if replacements.is_empty() {
                return Err(anyhow!("conditional batch requires a non-empty replacement set"));
            }
            let expected_ids = unique(expected, "expected")?;
            let replacement_ids = unique(&replacements, "replacement")?;
            if !expected_ids.is_subset(&replacement_ids) {
                return Err(anyhow!("conditional batch must replace every expected identity"));
            }
            let mut rows = self.rows.lock().unwrap();
            for row in expected {
                if rows.get(&identity(row)) != Some(row) {
                    return Ok(false);
                }
            }
            for row in &replacements {
                if !expected_ids.contains(&identity(row)) && rows.contains_key(&identity(row)) {
                    return Ok(false);
                }
            }
            for row in replacements {
                rows.insert(identity(&row), row);
            }
            Ok(true)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum SwapCommand {
        Delegate,
        Lose,
        Fail,
    }

    /// A memory store whose next swap can be commanded to lose (`Ok(false)`)
    /// or fail (`Err`), so the commit path's two non-committing exits are
    /// reachable without a second process.
    #[derive(Clone)]
    pub(crate) struct RefusingStore {
        inner: MemoryStore,
        command: Arc<Mutex<SwapCommand>>,
    }

    impl RefusingStore {
        pub(crate) fn new() -> Self {
            Self { inner: MemoryStore::new(), command: Arc::new(Mutex::new(SwapCommand::Delegate)) }
        }

        pub(crate) fn command(&self, command: SwapCommand) {
            *self.command.lock().unwrap() = command;
        }

        pub(crate) fn rows(&self) -> Vec<CultCacheEnvelope> {
            self.inner.rows()
        }
    }

    impl CacheBackingStore for RefusingStore {
        fn pull_all(&self) -> Result<Vec<CultCacheEnvelope>> {
            self.inner.pull_all()
        }

        fn push(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
            self.inner.push(entry)
        }

        fn delete(&mut self, entry: &CultCacheEnvelope) -> Result<()> {
            self.inner.delete(entry)
        }

        fn push_all(&mut self, entries: &[CultCacheEnvelope], options: PushAllOptions) -> Result<()> {
            self.inner.push_all(entries, options)
        }
    }

    impl MindStore for RefusingStore {
        fn compare_and_swap_batch(
            &self,
            expected: &[CultCacheEnvelope],
            replacements: Vec<CultCacheEnvelope>,
        ) -> Result<bool> {
            match *self.command.lock().unwrap() {
                SwapCommand::Delegate => self.inner.compare_and_swap_batch(expected, replacements),
                SwapCommand::Lose => Ok(false),
                SwapCommand::Fail => Err(anyhow!("the store refused the swap on command")),
            }
        }
    }
}
