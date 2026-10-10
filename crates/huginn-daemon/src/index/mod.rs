//! The semantic index's write side: a projection of the mind into a vector
//! collection, kept by one worker thread.
//!
//! The mind is the truth and this is derived. `huginn-mind` decides what a
//! document's text is (`IndexEntry`); this module owns when it is embedded and
//! written, and nothing here is stored in the mind: the worker's health lives
//! in memory and a restart recomputes it.
//!
//! One owner per decision. `Projector::reconcile` is the only place a point is
//! judged missing (the mind against the collection, by set difference, at
//! startup), and the worker is the only writer of points. Live admissions
//! reach the worker through `WorkerSink::committed`, which builds the entries
//! (local and cheap), leaves them in an inbox and returns: admission never
//! waits on an embedder or a vector store. Live entries and startup entries
//! share one embed-and-upsert routine, `Projector::flush_batch`.
//!
//! The ports are traits so the projector is tested with in-memory fakes:
//! `Embedder` (`ollama`) and `VectorIndex` (`qdrant`). Those two files are the
//! xenos boundary; nothing else in the organ speaks HTTP or JSON.
//!
//! Reconciliation is not a startup event. After any failed step the worker
//! reconciles again before it writes anything else: a collection lost while the
//! daemon runs is recreated and refilled, and a model re-pulled with other
//! dimensions rebuilds the collection, all without a restart. Two checks keep
//! that true without a failure to announce it. Before every flush the worker
//! re-reads the model's identity (name, digest, dimensions; about 0.3 s against
//! Ollama) and, if it is not the one the collection was reconciled under,
//! forgets the reconciliation instead of writing: a model re-pulled under the
//! same dimensions never mixes its vectors with the old model's. The worker
//! forgets the reconciliation every `Backoff::recheck` (60 s by default) on a
//! schedule that neither admissions nor searches move, which re-reads the model,
//! describes the collection and lists its points: a collection lost while nothing was being written is found within
//! that interval, and until then `Current` is what the worker last verified.
//! There is one judge either way: `reconcile`. Refilling needs
//! the text, so the projector keeps every entry it was given (`known`) beside
//! the ids it still owes (`wanted`): the whole indexable mind's text is held in
//! memory for the process's life.
//!
//! A collection belongs to a mind, not to a name. It is labelled with the
//! instance and with the mind's genesis receipt id (`Mind::genesis_receipt_id`),
//! and a collection labelled for another mind of the same instance name, or
//! for none, is refused and left untouched. A mind with no receipt yet has no
//! identity, so nothing is reconciled until its first admission.
//!
//! Stated limits.
//!
//! - The inbox is unbounded: it holds at most the documents admitted while the
//!   index is unreachable, each of which is also in the mind.
//! - Vectors of the wrong length are refused rather than written.
//! - The real input limit is the embedding model's context, not the 16 KiB
//!   `INDEX_TEXT_MAX_BYTES` bound. `qwen3-embedding:0.6b` has a 4096 token
//!   context, so dense text is cut at about 4k tokens and the tail of a long
//!   document is not searchable. Ollama truncates silently by default; its
//!   `truncate: false` option refuses over-context input instead, which would
//!   fail the whole batch forever on one dense document, so it is not set.
//!   There is no chunking.
//! - Points are never deleted. Withdrawn and superseded documents are indexed
//!   on purpose and the payload carries no status. The semantic read therefore
//!   takes only candidate ids and scores from a search: `Daemon::finish` joins
//!   them back through `Mind::rank`, which drops ids the mind does not hold and
//!   lets the selection decide the rest, in-force derivation included.
//! - A search runs on the worker between batches, so it waits for the batch in
//!   flight, and at most one runs between two steps of the writes. The serve
//!   loop never waits: it holds the reply and answers when the worker has
//!   (`IndexSink::search_deadline` bounds the wait), and takes back a search
//!   it has stopped waiting for. At most `SEARCHES_QUEUED_MAX` wait; one more is
//!   refused. A search whose embed is already running cannot be recalled, so it
//!   can hold the writes off for as long as it has left: the ticket carries the
//!   moment `Backoff::search_deadline` after it was asked, and every call the search
//!   makes (the model's identity, the collection's label, the embed, the
//!   query) is bounded by what remains of it.
//! - A search reads its own writes. Admissions and searches meet under one
//!   lock, and the worker judges a search and takes it in one critical
//!   section, admissions first: a search waits in the queue while anything
//!   ahead of it is owed (documents in the inbox, documents to write, a first
//!   reconciliation to make), and is answered once the index has caught up. Lag
//!   is not a rebuild and refuses nothing, and a mind's first build is lag:
//!   there is no earlier index to protect. Under a steady stream of admissions
//!   a search may never see the index caught up; its deadline then answers it.
//! - A search is refused only while the index is being rebuilt or cannot be
//!   asked: `Reconciling` (a collection made again because the model, its
//!   digest or the text version changed or because it was lost, until every
//!   document it owes is written again), `Failing` or `Refused`. The refusal
//!   is an `Unavailable` that names the state and how many documents are
//!   pending; it is never answered
//!   from a collection being rebuilt. A search that fails for its own reasons
//!   (a timeout, a store error) fails that search alone and the
//!   reconciliation stands. Only what the check-then-embed finds changed (the
//!   model, the collection's label) forgets it.

pub mod ollama;
pub mod qdrant;

#[cfg(test)]
pub(crate) mod fakes;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use huginn_mind::eureka_pipeline::{PipelineKind, PipelineRef, Short, Slug};
use huginn_mind::wire::IndexStatus;
use huginn_mind::{INDEX_TEXT_VERSION, IndexEntry, Mind, MindStore};
use sha2::{Digest, Sha256};

use crate::daemon::{Hits, IndexSink, SearchTicket};
use ollama::QUERY_INSTRUCTION;

/// The `managed_by` a collection carries when this organ made it.
pub const MANAGED_BY: &str = "huginn";

/// Documents embedded and written per call.
pub const BATCH: usize = 32;

/// How long a search may take, queue wait included, by default
/// (`Backoff::search_deadline`). The sink stamps it on every search when it is
/// asked, and the serve loop asks the sink for it (`IndexSink::search_deadline`),
/// so the loop's wait and the worker's bound are one value.
pub const SEARCH_DEADLINE: Duration = Duration::from_secs(30);

/// The bound on a bulk index-flush embed, the identity and label reads around
/// it, and the reconciliation's identity read. A cold model takes seconds to
/// load. It does not block serving: the serve loop never waits on the worker,
/// and a search queued behind a flush is answered by its own deadline.
pub const BULK_EMBED_TIMEOUT: Duration = Duration::from_secs(300);

/// The bound on a call to the vector store that is not a search: the
/// adapter's default.
pub const INDEX_CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The most candidates one search asks the index for.
pub const OVERSAMPLE_MAX: u32 = 200;

/// What an embedder is: the model by name and by digest, and the length of the
/// vectors it returns. Compatibility of a collection is decided on this and
/// on nothing else: never on the address the embedder was reached at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelIdentity {
    pub name: String,
    pub digest: String,
    pub dimensions: u32,
}

/// What a collection says about how it was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionMeta {
    pub managed_by: String,
    pub instance: String,
    /// The mind's genesis receipt id: the identity the instance name lacks.
    pub mind: String,
    pub model: String,
    pub model_digest: String,
    pub dimensions: u32,
    pub index_text_version: u32,
}

/// What the vector store holds under a collection's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Described {
    Absent,
    /// A collection that carries no metadata this organ wrote.
    Unlabelled,
    Labelled(CollectionMeta),
}

/// What travels beside a vector. The id of the point is derived from `doc_id`.
#[derive(Clone, Debug, PartialEq)]
pub struct PointPayload {
    pub doc_id: String,
    pub kind: String,
    pub root: String,
    pub ordinal: u64,
    pub text_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: PointPayload,
}

/// One candidate a vector search returned: the document it names, by the id and
/// kind the payload carries, and how near it scored. Nothing else about the
/// document is the index's to say.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub doc_id: String,
    pub kind: String,
    pub score: f32,
}

pub trait Embedder {
    /// The model's identity, or an error once `within` has passed.
    fn model_identity(&mut self, within: Duration) -> Result<ModelIdentity>;
    /// One vector per text, in order, or an error once `within` has passed.
    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>>;
}

pub trait VectorIndex {
    /// What the collection says about itself, or an error once `within` has
    /// passed.
    fn describe(&mut self, collection: &str, within: Duration) -> Result<Described>;
    /// Drops the collection if it exists and creates it empty with this
    /// metadata.
    fn recreate(&mut self, collection: &str, meta: &CollectionMeta) -> Result<()>;
    /// Every point id the collection holds.
    fn ids(&mut self, collection: &str) -> Result<BTreeSet<String>>;
    fn upsert(&mut self, collection: &str, points: &[Point]) -> Result<()>;
    /// The `limit` nearest points to `vector`, best first, with no filter: which
    /// documents may be shown is the mind's to decide. An error once `within`
    /// has passed.
    fn search(&mut self, collection: &str, vector: &[f32], limit: u32, within: Duration) -> Result<Vec<Hit>>;
}

/// The mind's collection: one per instance.
pub fn collection_name(instance: &Slug) -> String {
    format!("huginn_mind_{}", instance.0)
}

/// The point id of a document: the first sixteen bytes of the SHA-256 of the
/// document id, shaped as a version 5 UUID, which is the convention voidbot's
/// collections use. The real id travels in the payload.
pub fn point_id(document_id: &str) -> String {
    let mut digest: [u8; 32] = Sha256::digest(document_id.as_bytes()).into();
    digest[6] = (digest[6] & 0x0f) | 0x50;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    let hex: String = digest[..16].iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Why a step did not finish. Both are retried; they differ in what the
/// operator is told.
#[derive(Debug)]
pub enum StepError {
    /// The collection is another writer's and is left alone.
    Refused(String),
    /// The embedder or the vector store did not answer, or answered wrongly.
    Unavailable(anyhow::Error),
}

impl From<anyhow::Error> for StepError {
    fn from(error: anyhow::Error) -> Self {
        Self::Unavailable(error)
    }
}

impl std::fmt::Display for StepError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(reason) => formatter.write_str(reason),
            Self::Unavailable(error) => write!(formatter, "{error:#}"),
        }
    }
}

/// Whether a step left work to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    Progressed,
    Idle,
}

/// The time one operation has left. Every call the operation makes is bounded
/// by what remains, not by a bound of its own, so their sum cannot outlast it.
struct Budget {
    end: Instant,
    total: Duration,
}

impl Budget {
    fn new(total: Duration) -> Self {
        Self { end: Instant::now() + total, total }
    }

    fn left(&self) -> Result<Duration> {
        let left = self.end.saturating_duration_since(Instant::now());
        ensure!(!left.is_zero(), "the time allowed ({:?}) has run out", self.total);
        Ok(left)
    }
}

/// Whether the collection holds an index worth protecting, and whether it is
/// being made again. The state that marks a rebuild lasts until the entries it
/// owes have been written; nothing else clears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Build {
    /// No reconciliation has yet found or made this mind's collection. Its
    /// first build has nothing stale to protect, so it is lag.
    Unbuilt,
    /// The collection was built by this model and only lags the mind.
    Built,
    /// The collection was made again (another model, digest or text version,
    /// or lost while the worker ran) and is refilled from what the projector
    /// knows: until every entry it owes is written, it answers nothing.
    Rebuilding,
}

/// The projection's state: the entries not yet known to be in the collection,
/// and, once reconciled, the length of vector it accepts.
pub struct Projector<E: Embedder, V: VectorIndex> {
    embedder: E,
    index: V,
    instance: String,
    /// `None` until the mind's first admission; nothing is reconciled before.
    mind: Option<String>,
    collection: String,
    /// Every entry ever given, by document id: refilling a lost collection
    /// needs text the mind's writes have already gone past.
    known: BTreeMap<String, IndexEntry>,
    /// The ids in `known` whose points the collection may lack.
    wanted: BTreeSet<String>,
    /// The model the collection was reconciled under; `None` until it is, and
    /// again whenever that reconciliation is forgotten.
    reconciled: Option<ModelIdentity>,
    build: Build,
}

impl<E: Embedder, V: VectorIndex> Projector<E, V> {
    pub fn new(embedder: E, index: V, instance: &Slug, mind: Option<String>) -> Self {
        Self {
            embedder,
            index,
            instance: instance.0.clone(),
            mind,
            collection: collection_name(instance),
            known: BTreeMap::new(),
            wanted: BTreeSet::new(),
            reconciled: None,
            build: Build::Unbuilt,
        }
    }

    /// The mind's identity, once it has one. It never changes after that.
    fn identify(&mut self, mind: String) {
        if self.mind.is_none() {
            self.mind = Some(mind);
        }
    }

    /// Entries the collection may lack. Idempotent: an entry already wanted is
    /// the same document.
    pub fn want(&mut self, entries: impl IntoIterator<Item = IndexEntry>) {
        for entry in entries {
            self.wanted.insert(entry.id.id.0.clone());
            self.known.insert(entry.id.id.0.clone(), entry);
        }
    }

    pub fn pending(&self) -> u32 {
        u32::try_from(self.wanted.len()).unwrap_or(u32::MAX)
    }

    /// One step: reconcile first, then one batch at a time until nothing is
    /// wanted.
    ///
    /// Any failure forgets the reconciliation: the next attempt starts from
    /// the model and the collection as they are then.
    pub fn advance(&mut self) -> Result<Advance, StepError> {
        let stepped = self.step();
        if stepped.is_err() {
            self.reconciled = None;
        }
        stepped
    }

    /// `advance` after forgetting the reconciliation on purpose: the idle
    /// worker's periodic look at whether the model and the collection are still
    /// what it last verified.
    pub fn recheck(&mut self) -> Result<Advance, StepError> {
        self.reconciled = None;
        self.advance()
    }

    /// Whether the mind has an identity and the collection has not been
    /// reconciled under the model: `advance` has reconciling to do.
    pub fn stale(&self) -> bool {
        self.mind.is_some() && self.reconciled.is_none()
    }

    fn step(&mut self) -> Result<Advance, StepError> {
        if self.mind.is_none() {
            return Ok(Advance::Idle);
        }
        if self.reconciled.is_none() {
            self.reconcile()?;
            return Ok(Advance::Progressed);
        }
        if self.wanted.is_empty() {
            return Ok(Advance::Idle);
        }
        self.flush_batch()?;
        Ok(Advance::Progressed)
    }

    /// Re-reads the model's identity and compares it with the one the
    /// collection was reconciled under. A different model, digest or length
    /// forgets the reconciliation, so the next step reconciles (and rebuilds)
    /// instead of writing or searching with vectors the collection cannot mix.
    fn model_unchanged(&mut self, budget: &Budget) -> Result<bool> {
        let now = self.embedder.model_identity(budget.left()?).context("re-reading the embedding model's identity")?;
        if self.reconciled.as_ref() == Some(&now) {
            return Ok(true);
        }
        self.reconciled = None;
        Ok(false)
    }

    /// The only place a point is judged missing. Reads the model identity,
    /// makes the collection agree with it (creating it, or rebuilding it when
    /// the model, its digest, its dimensions or the text version differ),
    /// refuses a collection that is not this mind's, then makes `wanted` every
    /// known entry whose point the collection does not hold.
    pub fn reconcile(&mut self) -> Result<(), StepError> {
        let Some(mind) = self.mind.clone() else {
            return Err(StepError::Unavailable(anyhow::anyhow!("the mind has no identity yet")));
        };
        let identity =
            self.embedder.model_identity(BULK_EMBED_TIMEOUT).context("reading the embedding model's identity")?;
        let wanted = self.collection_meta(&mind, &identity);
        match self.index.describe(&self.collection, INDEX_CALL_TIMEOUT).context("describing the collection")? {
            Described::Absent => {
                // A collection lost while the worker ran was an index; one that
                // was never made is a first build.
                let build = if self.build == Build::Unbuilt { Build::Built } else { Build::Rebuilding };
                self.recreate(&wanted, build)?;
            }
            Described::Unlabelled => {
                return Err(StepError::Refused(format!(
                    "collection {} exists and carries no Huginn metadata; it is not this organ's to rewrite",
                    self.collection
                )));
            }
            Described::Labelled(found)
                if found.managed_by != MANAGED_BY || found.instance != self.instance || found.mind != mind =>
            {
                return Err(StepError::Refused(format!(
                    "collection {} is managed by {:?} for instance {:?}, mind {:?}; this organ serves instance {:?}, mind {:?}",
                    self.collection, found.managed_by, found.instance, found.mind, self.instance, mind
                )));
            }
            Described::Labelled(found) if found != wanted => self.recreate(&wanted, Build::Rebuilding)?,
            Described::Labelled(_) if self.build == Build::Unbuilt => self.build = Build::Built,
            Described::Labelled(_) => {}
        }
        let present = self.index.ids(&self.collection).context("listing the collection's points")?;
        self.wanted = self.known.keys().filter(|document_id| !present.contains(&point_id(document_id))).cloned().collect();
        self.reconciled = Some(identity);
        self.settle();
        Ok(())
    }

    /// What the collection must say about itself to be this mind's, under
    /// this model.
    fn collection_meta(&self, mind: &str, identity: &ModelIdentity) -> CollectionMeta {
        CollectionMeta {
            managed_by: MANAGED_BY.into(),
            instance: self.instance.clone(),
            mind: mind.to_owned(),
            model: identity.name.clone(),
            model_digest: identity.digest.clone(),
            dimensions: identity.dimensions,
            index_text_version: INDEX_TEXT_VERSION,
        }
    }

    /// Makes the collection again. `build` is what it is once made, recorded
    /// with the collection so a failure before the refill leaves it recorded.
    fn recreate(&mut self, meta: &CollectionMeta, build: Build) -> Result<()> {
        self.index.recreate(&self.collection, meta).context("creating the collection")?;
        self.build = build;
        Ok(())
    }

    /// A rebuild ends when the entries it owes are all written: the one place
    /// it is cleared.
    fn settle(&mut self) {
        if self.build == Build::Rebuilding && self.wanted.is_empty() {
            self.build = Build::Built;
        }
    }

    /// The one embed-and-upsert routine, for startup and live entries alike:
    /// the first `BATCH` wanted entries are embedded, written, and only then
    /// no longer wanted.
    pub fn flush_batch(&mut self) -> Result<(), StepError> {
        self.flush(&Budget::new(BULK_EMBED_TIMEOUT)).map_err(StepError::Unavailable)
    }

    fn flush(&mut self, budget: &Budget) -> Result<()> {
        let dimensions = self.reconciled.as_ref().context("flush before reconcile")?.dimensions as usize;
        let batch: Vec<IndexEntry> =
            self.wanted.iter().take(BATCH).filter_map(|document_id| self.known.get(document_id)).cloned().collect();
        if batch.is_empty() {
            return Ok(());
        }
        let texts: Vec<String> = batch.iter().map(|entry| entry.text.clone()).collect();
        // A model that changed leaves nothing written and the reconciliation
        // forgotten: the next step reconciles and rebuilds.
        let Ok(vectors) = self.embed_verified(&texts, budget)? else { return Ok(()) };
        ensure!(vectors.len() == batch.len(), "the embedder returned {} vectors for {} texts", vectors.len(), batch.len());
        let mut points = Vec::with_capacity(batch.len());
        for (entry, vector) in batch.iter().zip(vectors) {
            ensure!(
                vector.len() == dimensions,
                "the embedder returned a vector of {} for {} where the collection holds {dimensions}",
                vector.len(),
                entry.id.id.0
            );
            points.push(Point {
                id: point_id(&entry.id.id.0),
                vector,
                payload: PointPayload {
                    doc_id: entry.id.id.0.clone(),
                    kind: entry.id.kind.name().into(),
                    root: entry.root.clone(),
                    ordinal: entry.ordinal,
                    text_sha256: entry.text_sha256.clone(),
                },
            });
        }
        self.index.upsert(&self.collection, &points).context("writing points")?;
        for entry in &batch {
            self.wanted.remove(&entry.id.id.0);
        }
        self.settle();
        Ok(())
    }

    /// The one check-then-embed, every call bounded by what `budget` has left:
    /// reads the model's identity and the collection's label, embeds, reads the
    /// identity again, and gives the vectors back only if the model is still
    /// the one the collection was reconciled under. A model that changed
    /// forgets the reconciliation and answers `Err(when)`; a collection that
    /// is no longer this mind's, built as reconciled, is an error and forgets
    /// it too. A call that fails or times out forgets nothing. Search and
    /// flush both embed through here, so neither compares or writes vectors
    /// made by a model the collection cannot mix.
    fn embed_verified(&mut self, texts: &[String], budget: &Budget) -> Result<Result<Vec<Vec<f32>>, &'static str>> {
        let (Some(mind), Some(reconciled)) = (self.mind.clone(), self.reconciled.clone()) else {
            bail!("the mind has no identity yet, so it has no index");
        };
        if !self.model_unchanged(budget)? {
            return Ok(Err("since the collection was built"));
        }
        let described = self.index.describe(&self.collection, budget.left()?).context("describing the collection")?;
        if described != Described::Labelled(self.collection_meta(&mind, &reconciled)) {
            self.reconciled = None;
            bail!("collection {} is no longer the one this mind's index was built as: {described:?}", self.collection);
        }
        let vectors = self.embedder.embed(texts, budget.left()?).context("embedding")?;
        if !self.model_unchanged(budget)? {
            return Ok(Err("while the texts were embedded"));
        }
        Ok(Ok(vectors))
    }

    /// The candidates a query text finds: the text is embedded with the model's
    /// query instruction and the collection is searched, with no filter and an
    /// oversample of `min(top_k * 4, 200)`, because the mind will drop most of
    /// what the index cannot know is not in force. A hit naming a kind this
    /// organ has no name for is logged and left out.
    ///
    /// A search is refused while the index is being rebuilt or cannot be asked
    /// (`unsearchable`), naming the state and how many documents are pending.
    /// Catching up on lag is the worker's: it serves a search only once the
    /// documents admitted before it are written. The answer is also only from
    /// a collection that is still this mind's, built by the model that is
    /// still the embedder: the identity is read before and after the query is
    /// embedded and the label before it, and the query is refused, with the
    /// reconciliation forgotten, if either disagrees with what the collection
    /// was reconciled under. Every call is bounded by what remains of
    /// `within`. A search that fails for any other reason (a timeout, a store
    /// error) fails alone and leaves the reconciliation standing.
    pub fn search(&mut self, text: &str, top_k: u32, within: Duration) -> Result<Hits> {
        if let Some(why) = unsearchable(&self.progress_status()) {
            bail!("{why}");
        }
        let budget = Budget::new(within);
        let Some(reconciled) = self.reconciled.clone() else {
            bail!("the index has not been built yet");
        };
        let dimensions = reconciled.dimensions as usize;
        let query = format!("Instruct: {QUERY_INSTRUCTION}\nQuery: {text}");
        let mut vectors = match self.embed_verified(&[query], &budget)? {
            Ok(vectors) => vectors,
            Err(when) => {
                bail!("the embedding model changed {when}: {}", unsearchable(&self.progress_status()).unwrap_or_default())
            }
        };
        ensure!(vectors.len() == 1, "the embedder returned {} vectors for one query", vectors.len());
        let vector = vectors.remove(0);
        ensure!(vector.len() == dimensions, "the embedder returned a query vector of {} where the collection holds {dimensions}", vector.len());
        let limit = top_k.saturating_mul(4).min(OVERSAMPLE_MAX);
        let found = self.index.search(&self.collection, &vector, limit, budget.left()?)?;
        let mut hits = Vec::with_capacity(found.len());
        for hit in found {
            match PipelineKind::ALL.iter().find(|kind| kind.name() == hit.kind) {
                Some(kind) => hits.push((PipelineRef { kind: *kind, id: Short(hit.doc_id) }, hit.score)),
                None => eprintln!("huginn: the index returned {} under the unknown kind {:?}; it is left out", hit.doc_id, hit.kind),
            }
        }
        Ok(hits)
    }

    /// The health after a step that did not fail.
    pub fn progress_status(&self) -> IndexStatus {
        match (&self.reconciled, self.pending()) {
            (None, 0) if self.mind.is_none() => IndexStatus::Current,
            // The first build of an identified mind has nothing stale to
            // protect: the wait for it is lag.
            (None, pending) if self.mind.is_some() && self.build == Build::Unbuilt => IndexStatus::Behind { pending },
            (None, pending) => IndexStatus::Reconciling { pending },
            (Some(_), 0) => IndexStatus::Current,
            (Some(_), pending) if self.build == Build::Rebuilding => IndexStatus::Reconciling { pending },
            (Some(_), pending) => IndexStatus::Behind { pending },
        }
    }

    /// The health after `attempts` consecutive failures, the last of them
    /// `error`.
    pub fn failure_status(&self, error: &StepError, attempts: u32) -> IndexStatus {
        let pending = self.pending();
        match error {
            StepError::Refused(reason) => IndexStatus::Refused { pending, attempts, reason: reason.clone() },
            StepError::Unavailable(error) => IndexStatus::Failing { pending, attempts, error: format!("{error:#}") },
        }
    }
}

/// The worker's clock: the retry delay starts at `initial`, doubles per
/// consecutive failure and stops at `max`; an idle worker verifies the model
/// and the collection again every `recheck`; a search is worth waiting for
/// `search_deadline` after it was asked.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    pub initial: Duration,
    pub max: Duration,
    pub recheck: Duration,
    pub search_deadline: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { initial: Duration::from_secs(30), max: Duration::from_secs(600), recheck: Duration::from_secs(60), search_deadline: SEARCH_DEADLINE }
    }
}

impl Backoff {
    pub fn after(&self, delay: Duration) -> Duration {
        (delay * 2).min(self.max)
    }
}

/// The most searches the worker holds waiting. A search asked when this many
/// wait is refused, by name, at once: the queue is bounded by the asker being
/// told, not by the worker's patience.
pub const SEARCHES_QUEUED_MAX: usize = 16;

/// A search waiting for the worker: what was asked, and the moment nobody is
/// waiting for the answer any more. Every call the search makes is bounded by
/// what remains until `due`.
struct Queued {
    ticket: SearchTicket,
    text: String,
    top_k: u32,
    due: Instant,
}

struct Shared {
    inbox: Vec<IndexEntry>,
    /// The mind's identity, once a commit has revealed it to a mind that had none.
    mind: Option<String>,
    status: IndexStatus,
    closed: bool,
    /// Searches started and not yet taken by the worker, at most
    /// `SEARCHES_QUEUED_MAX`; the serve loop takes back one it has stopped
    /// waiting for.
    searches: VecDeque<Queued>,
    /// Searches the worker finished and the serve loop has not collected.
    found: Vec<(SearchTicket, Result<Hits, String>)>,
    next_ticket: u64,
}

type Lock = Arc<(Mutex<Shared>, Condvar)>;

fn lock(shared: &Lock) -> MutexGuard<'_, Shared> {
    shared.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How long the worker waits, and what ends the wait early.
enum Wait {
    /// Idle: until something arrives (an entry, the mind's identity, a search)
    /// or `until`. The deadline is the caller's, fixed when it was set: work
    /// that wakes the worker does not move it.
    Work { until: Instant },
    /// Backing off from a failure: the whole delay, whatever arrives. A failing
    /// index is retried on its schedule, not once per admission.
    Backoff(Duration),
}

fn wait_for_work(shared: &Lock, wait: Wait) {
    let (deadline, wakes_on_work) = match wait {
        Wait::Work { until } => (until, true),
        Wait::Backoff(delay) => (Instant::now() + delay, false),
    };
    let mut guard = lock(shared);
    loop {
        if guard.closed {
            return;
        }
        if wakes_on_work && (!guard.inbox.is_empty() || guard.mind.is_some() || !guard.searches.is_empty()) {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        guard = shared.1.wait_timeout(guard, left).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
    }
}

fn publish(shared: &Lock, status: IndexStatus) {
    lock(shared).status = status;
    shared.1.notify_all();
}

/// The status with what the worker has not yet taken from the inbox added to
/// its pending count.
fn including(status: &IndexStatus, queued: u32) -> IndexStatus {
    use IndexStatus as S;
    match status {
        S::Reconciling { pending } => S::Reconciling { pending: pending + queued },
        S::Current if queued == 0 => S::Current,
        S::Current => S::Behind { pending: queued },
        S::Behind { pending } => S::Behind { pending: pending + queued },
        S::Failing { pending, attempts, error } => {
            S::Failing { pending: pending + queued, attempts: *attempts, error: error.clone() }
        }
        S::Refused { pending, attempts, reason } => {
            S::Refused { pending: pending + queued, attempts: *attempts, reason: reason.clone() }
        }
    }
}

/// Why a status cannot answer a search, or `None` when it can: the one
/// predicate for asking and for answering. An index being rebuilt
/// (`Reconciling`) holds a collection that is not the model's yet, and one
/// that is failing or refused cannot be asked. `Behind` is lag, not a
/// rebuild: the worker writes what was admitted before a search and then
/// answers it. The reason names the state and how many documents are pending.
fn unsearchable(status: &IndexStatus) -> Option<String> {
    use IndexStatus as S;
    match status {
        S::Current | S::Behind { .. } => None,
        S::Reconciling { pending } => Some(format!(
            "the semantic index is being rebuilt: reconciling with the model and the collection (pending: {pending})"
        )),
        S::Failing { pending, error, .. } => Some(format!("the semantic index is failing: {error} (pending: {pending})")),
        S::Refused { pending, reason, .. } => Some(format!("the semantic index is refused: {reason} (pending: {pending})")),
    }
}

/// Runs the search at the front of the queue, if any, and leaves its answer
/// for the loop to collect. Between batches, never inside one, and only one:
/// the worker returns to its writes after each, so a queue of searches cannot
/// hold the index's writes off for the sum of their embeddings.
///
/// Admissions come first, and the judgement and the take are one critical
/// section: the inbox is absorbed and the search taken under one guard, so a
/// document acknowledged before the search was asked is wanted when the search
/// is judged, and none can slip in between. A search is taken only with nothing
/// ahead of it (`take_search`). The search's calls are bounded by what remains
/// of the time it was given when it was asked.
fn serve_search<E: Embedder, V: VectorIndex>(projector: &mut Projector<E, V>, shared: &Lock) {
    let mut guard = lock(shared);
    if !absorb_into(projector, &mut guard, None) {
        return;
    }
    #[cfg(test)]
    tests::after_absorb(&mut guard);
    let Some(Queued { ticket, text, top_k, due }) = take_search(projector, &mut guard) else { return };
    drop(guard);
    let found = projector.search(&text, top_k, due.saturating_duration_since(Instant::now())).map_err(|error| format!("{error:#}"));
    lock(shared).found.push((ticket, found));
}

/// The search at the front of the queue, if it may be served now. Never while
/// a document sits in the inbox, and never while anything ahead of it is
/// owed (documents to write, a reconciliation to make) unless the index
/// cannot be asked at all, in which case the search is taken to be refused:
/// waiting would not change the answer.
fn take_search<E: Embedder, V: VectorIndex>(projector: &Projector<E, V>, guard: &mut Shared) -> Option<Queued> {
    if !guard.inbox.is_empty() {
        return None;
    }
    let caught_up = projector.pending() == 0 && !projector.stale();
    if !caught_up && unsearchable(&projector.progress_status()).is_none() {
        return None;
    }
    guard.searches.pop_front()
}

/// Answers every queued search with why the index cannot: they were accepted
/// while it was healthy and it has failed since.
fn refuse_searches(shared: &Lock, why: &str) {
    let mut guard = lock(shared);
    while let Some(queued) = guard.searches.pop_front() {
        guard.found.push((queued.ticket, Err(why.to_owned())));
    }
}

/// The worker's take of what arrived since it last looked: the inbox is taken,
/// absorbed and its effect published under one lock. Between taking entries
/// out of the inbox and publishing them as wanted, `status` would otherwise
/// read a stale count with nothing queued, and a reader would see the mind
/// caught up (or a failure short of what it is failing to write) while
/// entries were in the worker's hands. `failing` is the failure being
/// reported and how many attempts it has had: the report stands, with the new
/// pending count, until an attempt succeeds. Returns false once the sink has
/// closed.
fn absorb<E: Embedder, V: VectorIndex>(
    projector: &mut Projector<E, V>,
    shared: &Lock,
    failing: Option<(&StepError, u32)>,
) -> bool {
    absorb_into(projector, &mut lock(shared), failing)
}

/// `absorb` under a guard the caller already holds, for the caller that must
/// decide something in the same critical section.
fn absorb_into<E: Embedder, V: VectorIndex>(
    projector: &mut Projector<E, V>,
    guard: &mut Shared,
    failing: Option<(&StepError, u32)>,
) -> bool {
    if guard.closed {
        return false;
    }
    let mind = guard.mind.take();
    let inbox = std::mem::take(&mut guard.inbox);
    if let Some(mind) = mind {
        projector.identify(mind);
    }
    projector.want(inbox);
    guard.status = match failing {
        None => projector.progress_status(),
        Some((error, attempts)) => projector.failure_status(error, attempts),
    };
    true
}

fn work<E: Embedder, V: VectorIndex>(mut projector: Projector<E, V>, shared: Lock, backoff: Backoff) {
    let mut failing: Option<(StepError, u32)> = None;
    let mut delay = backoff.initial;
    // When the worker next verifies the model and the collection whether or
    // not anything has woken it: an absolute time, moved only by a
    // verification, so a steady stream of searches cannot keep pushing it out.
    // The first step is a verification (the startup reconciliation), so it is
    // due now.
    let mut recheck_at = Instant::now();
    loop {
        if !absorb(&mut projector, &shared, failing.as_ref().map(|(error, attempts)| (error, *attempts))) {
            return;
        }
        shared.1.notify_all();
        let due = Instant::now() >= recheck_at;
        let verifies = due || projector.stale();
        let stepped = if due { projector.recheck() } else { projector.advance() };
        match stepped {
            Ok(advance) => {
                failing = None;
                delay = backoff.initial;
                if verifies {
                    recheck_at = Instant::now() + backoff.recheck;
                }
                publish(&shared, projector.progress_status());
                serve_search(&mut projector, &shared);
                // A model or collection the search found changed has left the
                // reconciliation to redo: do it now, not at the next wake.
                if advance == Advance::Idle && !projector.stale() {
                    wait_for_work(&shared, Wait::Work { until: recheck_at });
                }
            }
            Err(error) => {
                let attempts = failing.as_ref().map_or(0, |(_, attempts)| *attempts).saturating_add(1);
                eprintln!(
                    "huginn: the index could not advance (attempt {attempts}, {} pending, retrying in {:?}): {error}",
                    projector.pending(),
                    delay
                );
                publish(&shared, projector.failure_status(&error, attempts));
                refuse_searches(&shared, &unsearchable(&projector.failure_status(&error, attempts)).unwrap_or_default());
                failing = Some((error, attempts));
                wait_for_work(&shared, Wait::Backoff(delay));
                delay = backoff.after(delay);
            }
        }
    }
}

/// The daemon's index: a handle on the one worker thread that owns the ports.
/// `committed` builds entries and queues them; `status` reads the worker's
/// health. Dropping the sink tells the worker to stop and does not wait for
/// it: a call in flight ends with its own timeout or with the process.
pub struct WorkerSink {
    shared: Lock,
    /// Whether the worker has been told the mind's identity.
    identified: bool,
    /// How long after it was asked a search is worth answering.
    deadline: Duration,
}

impl WorkerSink {
    /// Starts the worker. `mind` is the mind's genesis receipt id, `None` for
    /// a mind not yet written. `startup` is every indexable document the mind
    /// holds now; the worker's first act is to reconcile the collection
    /// against it.
    pub fn spawn<E, V>(
        embedder: E,
        index: V,
        instance: &Slug,
        mind: Option<String>,
        startup: Vec<IndexEntry>,
        backoff: Backoff,
    ) -> Self
    where
        E: Embedder + Send + 'static,
        V: VectorIndex + Send + 'static,
    {
        let identified = mind.is_some();
        let mut projector = Projector::new(embedder, index, instance, mind);
        projector.want(startup);
        let shared: Lock = Arc::new((
            Mutex::new(Shared {
                inbox: Vec::new(),
                mind: None,
                status: projector.progress_status(),
                closed: false,
                searches: VecDeque::new(),
                found: Vec::new(),
                next_ticket: 0,
            }),
            Condvar::new(),
        ));
        let worker = Arc::clone(&shared);
        std::thread::spawn(move || work(projector, worker, backoff));
        Self { shared, identified, deadline: backoff.search_deadline }
    }

    /// Blocks until the health satisfies `predicate`, and returns it. For
    /// tests and probes that must observe the worker without a clock; it gives
    /// up after `WAIT_STATUS_MAX` in all, however often the health changes,
    /// rather than hang a run.
    pub fn wait_status(&self, predicate: impl Fn(&IndexStatus) -> bool) -> IndexStatus {
        self.wait_status_for(WAIT_STATUS_MAX, predicate)
    }

    fn wait_status_for(&self, limit: Duration, predicate: impl Fn(&IndexStatus) -> bool) -> IndexStatus {
        let deadline = Instant::now() + limit;
        let mut guard = lock(&self.shared);
        loop {
            let seen = including(&guard.status, u32::try_from(guard.inbox.len()).unwrap_or(u32::MAX));
            if predicate(&seen) {
                return seen;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the index never reached the awaited state; last seen {:?}", guard.status);
            guard = self.shared.1.wait_timeout(guard, left).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
        }
    }
}

/// The longest `WorkerSink::wait_status` waits, in all.
const WAIT_STATUS_MAX: Duration = Duration::from_secs(10);

impl Drop for WorkerSink {
    fn drop(&mut self) {
        lock(&self.shared).closed = true;
        self.shared.1.notify_all();
    }
}

impl<S: MindStore> IndexSink<S> for WorkerSink {
    fn committed(&mut self, mind: &Mind<S>, writes: &[PipelineRef]) -> Result<()> {
        let entries = mind.index_entries(Some(writes))?;
        let revealed = if self.identified { None } else { mind.genesis_receipt_id()? };
        if entries.is_empty() && revealed.is_none() {
            return Ok(());
        }
        let mut guard = lock(&self.shared);
        guard.inbox.extend(entries);
        if let Some(revealed) = revealed {
            guard.mind = Some(revealed);
            self.identified = true;
        }
        drop(guard);
        self.shared.1.notify_all();
        Ok(())
    }

    fn status(&self) -> IndexStatus {
        let guard = lock(&self.shared);
        including(&guard.status, u32::try_from(guard.inbox.len()).unwrap_or(u32::MAX))
    }

    /// Queues the search for the worker and returns its ticket. An index that
    /// is being rebuilt or cannot be asked is not asked, and neither is a full
    /// queue: the refusal is the answer, naming why and how many documents are
    /// pending. Documents admitted and not yet written are not a refusal: the
    /// search waits behind them.
    fn search(&mut self, text: &str, top_k: u32) -> Result<SearchTicket, String> {
        let mut guard = lock(&self.shared);
        let status = including(&guard.status, u32::try_from(guard.inbox.len()).unwrap_or(u32::MAX));
        if let Some(why) = unsearchable(&status) {
            return Err(why);
        }
        if guard.searches.len() >= SEARCHES_QUEUED_MAX {
            return Err(format!(
                "the semantic index has {} searches waiting and takes no more until it has answered some",
                guard.searches.len()
            ));
        }
        let ticket = SearchTicket(guard.next_ticket);
        guard.next_ticket += 1;
        guard.searches.push_back(Queued { ticket, text: text.to_owned(), top_k, due: Instant::now() + self.deadline });
        drop(guard);
        self.shared.1.notify_all();
        Ok(ticket)
    }

    fn search_deadline(&self) -> Duration {
        self.deadline
    }

    fn searched(&mut self) -> Vec<(SearchTicket, Result<Hits, String>)> {
        std::mem::take(&mut lock(&self.shared).found)
    }

    /// A search still queued is taken out of the queue and never embedded. One
    /// the worker already holds cannot be recalled; its answer is collected
    /// and dropped by the loop, which no longer waits for it.
    fn abandon(&mut self, ticket: SearchTicket) {
        lock(&self.shared).searches.retain(|queued| queued.ticket != ticket);
    }
}

/// Plain HTTP to one base address, in text: the adapters own what the text
/// means. There is no TLS in this build; the addresses it serves are the
/// private mesh's.
pub(crate) struct Http {
    agent: ureq::Agent,
    base: String,
}

#[derive(Clone, Copy)]
pub(crate) enum Method {
    Get,
    Put,
    Post,
    Delete,
}

impl Http {
    pub(crate) fn new(base_url: &str, timeout: Duration) -> Self {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).build().into();
        Self { agent, base: base_url.trim_end_matches('/').to_owned() }
    }

    /// The same address under a bound of its own.
    pub(crate) fn bounded(&self, timeout: Duration) -> Self {
        Self::new(&self.base, timeout)
    }

    /// The status and the body, whatever the status is.
    pub(crate) fn request(&self, method: Method, path: &str, body: Option<&str>) -> Result<(u16, String)> {
        let url = format!("{}{path}", self.base);
        let json = "application/json";
        let sent = match method {
            Method::Get => self.agent.get(&url).call(),
            Method::Delete => self.agent.delete(&url).call(),
            Method::Post => self.agent.post(&url).header("content-type", json).send(body.unwrap_or_default()),
            Method::Put => self.agent.put(&url).header("content-type", json).send(body.unwrap_or_default()),
        };
        let mut response = sent.with_context(|| format!("no answer from {url}"))?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().with_context(|| format!("reading the answer from {url}"))?;
        Ok((status, text))
    }
}

#[cfg(test)]
mod tests;
