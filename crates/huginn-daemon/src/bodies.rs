//! The deferred bodies: answers too large for one send, held for the client to
//! fetch chunk by chunk. Ephemeral transport state, never a document store and
//! never world truth: it is lost when the process exits, and a client that
//! finds a body gone asks the operation again.
//!
//! Identity is the manifest's `contentHash`, so the same bytes deferred twice
//! are one body. Chunking, hashing and the manifest are `cultnet-rs`'s; what is
//! owned here is only how long and how much is kept.
//!
//! A body expires when it has been untouched for the TTL, where retaining it
//! again and serving any of its chunks both touch it. When the stored bytes
//! exceed the budget the body touched longest ago is evicted first. A chunk
//! that two bodies share is stored once and lives until the last of them goes.
//! Time is an argument, so nothing here reads a clock.

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use chrono::{DateTime, Utc};
use cultnet_rs::{CultMeshCdnArtifactManifest, CultMeshCdnChunk};

/// The size of one chunk of a deferred body. Four fit one send with room for a
/// control-plane answer beside them on the same session.
pub const DEFERRED_CHUNK_BYTES: usize = 256 * 1024;
/// The largest answer that is deferred rather than refused, measured on the
/// encoded payload.
pub const MAX_DEFERRED_BODY_BYTES: u64 = 64 * 1024 * 1024;
/// What the daemon keeps across all deferred bodies.
pub const DEFERRED_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

struct Chunk {
    bytes: Vec<u8>,
    /// The content hashes of the retained bodies that reference this chunk.
    owners: BTreeSet<String>,
}

struct Body {
    chunk_hashes: BTreeSet<String>,
    touched_at: Cell<DateTime<Utc>>,
    /// Orders touches when the clock does not: the largest is the newest.
    touch_order: Cell<u64>,
}

pub struct DeferredBodies {
    budget: u64,
    ttl: chrono::Duration,
    chunks: HashMap<String, Chunk>,
    bodies: HashMap<String, Body>,
    stored: u64,
    touches: Cell<u64>,
}

impl DeferredBodies {
    pub fn new(budget: u64, ttl: Duration) -> Self {
        Self {
            budget,
            ttl: chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX),
            chunks: HashMap::new(),
            bodies: HashMap::new(),
            stored: 0,
            touches: Cell::new(0),
        }
    }

    /// Bytes held: each distinct chunk once.
    #[cfg(test)]
    pub fn stored_bytes(&self) -> u64 {
        self.stored
    }

    #[cfg(test)]
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Holds a body, or, when its bytes are already held, touches it and
    /// stores nothing. Then evicts, least recently touched first, while the
    /// stored bytes exceed the budget and another body could be evicted
    /// instead: the body just retained is the last to go, and one that alone
    /// exceeds the budget is kept whole.
    pub fn retain(&mut self, manifest: &CultMeshCdnArtifactManifest, chunks: Vec<CultMeshCdnChunk>, now: DateTime<Utc>) {
        let content_hash = manifest.content_hash.clone();
        if let Some(body) = self.bodies.get(&content_hash) {
            self.touch(body, now);
            return;
        }
        let mut chunk_hashes = BTreeSet::new();
        for chunk in chunks {
            chunk_hashes.insert(chunk.chunk_hash.clone());
            let held = self.chunks.entry(chunk.chunk_hash).or_insert_with(|| {
                self.stored += chunk.payload.len() as u64;
                Chunk { bytes: chunk.payload, owners: BTreeSet::new() }
            });
            held.owners.insert(content_hash.clone());
        }
        let body = Body { chunk_hashes, touched_at: Cell::new(now), touch_order: Cell::new(0) };
        self.touch(&body, now);
        self.bodies.insert(content_hash, body);
        while self.stored > self.budget && self.bodies.len() > 1 {
            let oldest = self
                .bodies
                .iter()
                .min_by_key(|(_, body)| body.touch_order.get())
                .map(|(hash, _)| hash.clone())
                .expect("more than one body is held");
            self.drop_body(&oldest);
        }
    }

    /// One chunk's bytes by its normalised hash, touching every body that
    /// references it. Serving a chunk is what keeps its body alive.
    pub fn chunk(&self, hash: &str, now: DateTime<Utc>) -> Option<&[u8]> {
        let chunk = self.chunks.get(hash)?;
        for owner in &chunk.owners {
            self.touch(&self.bodies[owner], now);
        }
        Some(&chunk.bytes)
    }

    /// Drops every body untouched for the TTL as of `now`. A clock that reads
    /// earlier than a touch expires nothing.
    pub fn expire(&mut self, now: DateTime<Utc>) {
        let expired: Vec<String> = self
            .bodies
            .iter()
            .filter(|(_, body)| now.signed_duration_since(body.touched_at.get()) >= self.ttl)
            .map(|(hash, _)| hash.clone())
            .collect();
        for hash in expired {
            self.drop_body(&hash);
        }
    }

    fn touch(&self, body: &Body, now: DateTime<Utc>) {
        body.touched_at.set(now);
        self.touches.set(self.touches.get() + 1);
        body.touch_order.set(self.touches.get());
    }

    fn drop_body(&mut self, content_hash: &str) {
        let Some(body) = self.bodies.remove(content_hash) else { return };
        for hash in body.chunk_hashes {
            let chunk = self.chunks.get_mut(&hash).expect("a retained body's chunks are held");
            chunk.owners.remove(content_hash);
            if chunk.owners.is_empty() {
                self.stored -= chunk.bytes.len() as u64;
                self.chunks.remove(&hash);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cultnet_rs::pack_content;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    /// A body of `pieces` chunks of 4 bytes each, distinct per `tag`.
    fn body(tag: u8, pieces: usize) -> (CultMeshCdnArtifactManifest, Vec<CultMeshCdnChunk>) {
        let mut bytes = Vec::new();
        for piece in 0..pieces {
            bytes.extend_from_slice(&[tag, piece as u8, 7, 7]);
        }
        pack_content("t", "package", "", "", "", &bytes, 4).unwrap()
    }

    fn held(store: &DeferredBodies, manifest: &CultMeshCdnArtifactManifest, now: DateTime<Utc>) -> bool {
        manifest.chunks.iter().all(|chunk| store.chunk(&chunk.chunk_hash, now).is_some())
    }

    const TTL: Duration = Duration::from_secs(60);

    /// The same bytes deferred twice are one body, counted once; serving a
    /// chunk slides the expiry; an untouched body is gone exactly at the TTL.
    #[test]
    fn retention_is_idempotent_and_sliding() {
        let mut store = DeferredBodies::new(1_000, TTL);
        let (manifest, chunks) = body(1, 3);
        store.retain(&manifest, chunks.clone(), at(0));
        store.retain(&manifest, chunks, at(1));
        assert_eq!((store.body_count(), store.stored_bytes()), (1, 12), "one body, its bytes counted once");

        // Retaining again at t=1 touched it, so it lives to t=61, not t=60.
        store.expire(at(60));
        assert_eq!(store.body_count(), 1, "the second retain refreshed it");
        // Serving a chunk at t=60 touches it, so t=119 is inside a fresh TTL.
        assert!(store.chunk(&manifest.chunks[0].chunk_hash, at(60)).is_some());
        store.expire(at(119));
        assert_eq!(store.body_count(), 1, "serving a chunk slid the expiry");
        store.expire(at(120));
        assert_eq!((store.body_count(), store.stored_bytes()), (0, 0), "untouched for the TTL is gone");

        // An untouched body is gone at the TTL and not before it.
        let (other, chunks) = body(2, 1);
        store.retain(&other, chunks, at(200));
        store.expire(at(259));
        assert_eq!(store.body_count(), 1);
        store.expire(at(260));
        assert_eq!(store.body_count(), 0);
    }

    /// Time going backwards expires nothing.
    #[test]
    fn a_clock_behind_the_last_touch_expires_nothing() {
        let mut store = DeferredBodies::new(1_000, TTL);
        let (manifest, chunks) = body(1, 1);
        store.retain(&manifest, chunks, at(100));
        store.expire(at(0));
        assert_eq!(store.body_count(), 1);
    }

    /// Over the budget the body touched longest ago goes first, not the
    /// newest and not the first inserted, and a chunk two bodies share stays
    /// while either lives.
    #[test]
    fn eviction_is_least_recently_touched_and_bounded_in_bytes() {
        // Budget 12 bytes: three chunks of 4.
        let mut store = DeferredBodies::new(12, TTL);
        let (a, chunks_a) = body(1, 1);
        let (b, chunks_b) = body(2, 1);
        let (c, chunks_c) = body(3, 1);
        store.retain(&a, chunks_a, at(0));
        store.retain(&b, chunks_b, at(1));
        // The oldest-inserted body is touched last, so insertion order and
        // recency disagree about who goes.
        assert!(store.chunk(&a.chunks[0].chunk_hash, at(2)).is_some());
        store.retain(&c, chunks_c, at(3));
        assert_eq!(store.stored_bytes(), 12);
        let (d, chunks_d) = body(4, 1);
        store.retain(&d, chunks_d, at(4));
        assert_eq!(store.stored_bytes(), 12, "bounded by the budget");
        assert!(!held(&store, &b, at(5)), "the least recently touched went");
        assert!(held(&store, &a, at(5)) && held(&store, &c, at(5)) && held(&store, &d, at(5)));

        // A shared chunk survives its first owner.
        let mut store = DeferredBodies::new(12, TTL);
        let (x, chunks_x) = body(5, 2); // chunks x0 x1
        store.retain(&x, chunks_x, at(0));
        let (y, mut chunks_y) = body(6, 1); // chunk y0, plus x1 shared
        let mut manifest_y = y.clone();
        manifest_y.content_hash = "shared-owner".into();
        chunks_y.push(CultMeshCdnChunk {
            chunk_hash: x.chunks[1].chunk_hash.clone(),
            payload: vec![5, 1, 7, 7],
        });
        store.retain(&manifest_y, chunks_y, at(1));
        assert_eq!(store.stored_bytes(), 12, "the shared chunk is stored once");
        // Evict x (older): its private chunk goes, the shared one stays.
        let (z, chunks_z) = body(7, 1);
        store.retain(&z, chunks_z, at(2));
        assert_eq!(store.body_count(), 2, "x was evicted");
        assert!(store.chunk(&x.chunks[0].chunk_hash, at(3)).is_none(), "x's private chunk is gone");
        assert!(store.chunk(&x.chunks[1].chunk_hash, at(3)).is_some(), "the shared chunk lives with y");
    }

    /// A body that alone exceeds the budget is kept whole rather than
    /// evicting itself, so a manifest handed out is never a lie.
    #[test]
    fn a_body_over_the_budget_by_itself_is_kept() {
        let mut store = DeferredBodies::new(4, TTL);
        let (manifest, chunks) = body(1, 3);
        store.retain(&manifest, chunks, at(0));
        assert!(held(&store, &manifest, at(0)));
    }
}
