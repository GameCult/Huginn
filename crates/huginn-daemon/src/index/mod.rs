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
//! Stated limits. The inbox is unbounded: it holds at most the documents
//! admitted while the index is unreachable, each of which is also in the mind.
//! The model identity is read at reconciliation only, so a model re-pulled
//! while the daemon runs takes effect at the next start, and vectors of the
//! wrong length are refused rather than written.

pub mod ollama;
pub mod qdrant;

#[cfg(test)]
pub(crate) mod fakes;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use huginn_mind::epiphany_pipeline::{PipelineRef, Slug};
use huginn_mind::wire::IndexStatus;
use huginn_mind::{INDEX_TEXT_VERSION, IndexEntry, Mind, MindStore};
use sha2::{Digest, Sha256};

use crate::daemon::IndexSink;

/// The `managed_by` a collection carries when this organ made it.
pub const MANAGED_BY: &str = "huginn";

/// Documents embedded and written per call.
pub const BATCH: usize = 32;

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

pub trait Embedder {
    fn model_identity(&mut self) -> Result<ModelIdentity>;
    /// One vector per text, in order.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub trait VectorIndex {
    fn describe(&mut self, collection: &str) -> Result<Described>;
    /// Drops the collection if it exists and creates it empty with this
    /// metadata.
    fn recreate(&mut self, collection: &str, meta: &CollectionMeta) -> Result<()>;
    /// Every point id the collection holds.
    fn ids(&mut self, collection: &str) -> Result<BTreeSet<String>>;
    fn upsert(&mut self, collection: &str, points: &[Point]) -> Result<()>;
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

/// The projection's state: the entries not yet known to be in the collection,
/// and, once reconciled, the length of vector it accepts.
pub struct Projector<E: Embedder, V: VectorIndex> {
    embedder: E,
    index: V,
    instance: String,
    collection: String,
    wanted: BTreeMap<String, IndexEntry>,
    dimensions: Option<u32>,
}

impl<E: Embedder, V: VectorIndex> Projector<E, V> {
    pub fn new(embedder: E, index: V, instance: &Slug) -> Self {
        Self {
            embedder,
            index,
            instance: instance.0.clone(),
            collection: collection_name(instance),
            wanted: BTreeMap::new(),
            dimensions: None,
        }
    }

    /// Entries the collection may lack. Idempotent: an entry already wanted is
    /// the same document.
    pub fn want(&mut self, entries: impl IntoIterator<Item = IndexEntry>) {
        for entry in entries {
            self.wanted.insert(entry.id.id.0.clone(), entry);
        }
    }

    pub fn pending(&self) -> u32 {
        u32::try_from(self.wanted.len()).unwrap_or(u32::MAX)
    }

    /// One step: reconcile first, then one batch at a time until nothing is
    /// wanted.
    pub fn advance(&mut self) -> Result<Advance, StepError> {
        if self.dimensions.is_none() {
            self.reconcile()?;
            return Ok(Advance::Progressed);
        }
        if self.wanted.is_empty() {
            return Ok(Advance::Idle);
        }
        self.flush_batch()?;
        Ok(Advance::Progressed)
    }

    /// The only place a point is judged missing. Reads the model identity,
    /// makes the collection agree with it (creating it, or rebuilding it when
    /// the model, its digest, its dimensions or the text version differ),
    /// refuses a collection that is not this instance's, then drops from
    /// `wanted` every entry whose point the collection already holds.
    pub fn reconcile(&mut self) -> Result<(), StepError> {
        let identity = self.embedder.model_identity().context("reading the embedding model's identity")?;
        let wanted = CollectionMeta {
            managed_by: MANAGED_BY.into(),
            instance: self.instance.clone(),
            model: identity.name.clone(),
            model_digest: identity.digest.clone(),
            dimensions: identity.dimensions,
            index_text_version: INDEX_TEXT_VERSION,
        };
        match self.index.describe(&self.collection).context("describing the collection")? {
            Described::Absent => self.recreate(&wanted)?,
            Described::Unlabelled => {
                return Err(StepError::Refused(format!(
                    "collection {} exists and carries no Huginn metadata; it is not this organ's to rewrite",
                    self.collection
                )));
            }
            Described::Labelled(found) if found.managed_by != MANAGED_BY || found.instance != self.instance => {
                return Err(StepError::Refused(format!(
                    "collection {} is managed by {:?} for instance {:?}; this organ serves {:?}",
                    self.collection, found.managed_by, found.instance, self.instance
                )));
            }
            Described::Labelled(found) if found != wanted => self.recreate(&wanted)?,
            Described::Labelled(_) => {}
        }
        let present = self.index.ids(&self.collection).context("listing the collection's points")?;
        self.wanted.retain(|document_id, _| !present.contains(&point_id(document_id)));
        self.dimensions = Some(identity.dimensions);
        Ok(())
    }

    fn recreate(&mut self, meta: &CollectionMeta) -> Result<()> {
        self.index.recreate(&self.collection, meta).context("creating the collection")
    }

    /// The one embed-and-upsert routine, for startup and live entries alike:
    /// the first `BATCH` wanted entries are embedded, written, and only then
    /// no longer wanted.
    pub fn flush_batch(&mut self) -> Result<(), StepError> {
        self.flush().map_err(StepError::Unavailable)
    }

    fn flush(&mut self) -> Result<()> {
        let dimensions = self.dimensions.context("flush before reconcile")? as usize;
        let batch: Vec<IndexEntry> = self.wanted.values().take(BATCH).cloned().collect();
        if batch.is_empty() {
            return Ok(());
        }
        let texts: Vec<String> = batch.iter().map(|entry| entry.text.clone()).collect();
        let vectors = self.embedder.embed(&texts).context("embedding")?;
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
        Ok(())
    }

    /// The health after a step that did not fail.
    pub fn progress_status(&self) -> IndexStatus {
        match (self.dimensions, self.pending()) {
            (None, pending) => IndexStatus::Reconciling { pending },
            (Some(_), 0) => IndexStatus::Current,
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

/// The retry delay: it starts at `initial`, doubles per consecutive failure and
/// stops at `max`.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    pub initial: Duration,
    pub max: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { initial: Duration::from_secs(30), max: Duration::from_secs(600) }
    }
}

impl Backoff {
    pub fn after(&self, delay: Duration) -> Duration {
        (delay * 2).min(self.max)
    }
}

struct Shared {
    inbox: Vec<IndexEntry>,
    status: IndexStatus,
    closed: bool,
}

type Lock = Arc<(Mutex<Shared>, Condvar)>;

fn lock(shared: &Lock) -> MutexGuard<'_, Shared> {
    shared.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// With no timeout, blocks until the inbox holds something or the sink is
/// closed. With one, sleeps out the backoff whatever arrives: a failing index
/// is retried on its schedule, not once per admission.
fn wait_for_work(shared: &Lock, timeout: Option<Duration>) {
    let mut guard = lock(shared);
    match timeout {
        None => {
            while guard.inbox.is_empty() && !guard.closed {
                guard = shared.1.wait(guard).unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        }
        Some(timeout) => {
            let deadline = Instant::now() + timeout;
            while !guard.closed {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return;
                }
                guard = shared.1.wait_timeout(guard, left).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
            }
        }
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

fn work<E: Embedder, V: VectorIndex>(mut projector: Projector<E, V>, shared: Lock, backoff: Backoff) {
    let mut failures = 0_u32;
    let mut delay = backoff.initial;
    loop {
        let inbox = {
            let mut guard = lock(&shared);
            if guard.closed {
                return;
            }
            std::mem::take(&mut guard.inbox)
        };
        projector.want(inbox);
        // While a failure is being reported, the report stands until an
        // attempt succeeds: publishing progress between retries would flicker.
        if failures == 0 {
            publish(&shared, projector.progress_status());
        }
        match projector.advance() {
            Ok(Advance::Progressed) => {
                failures = 0;
                delay = backoff.initial;
                publish(&shared, projector.progress_status());
            }
            Ok(Advance::Idle) => {
                failures = 0;
                delay = backoff.initial;
                publish(&shared, projector.progress_status());
                wait_for_work(&shared, None);
            }
            Err(error) => {
                failures = failures.saturating_add(1);
                eprintln!(
                    "huginn: the index could not advance (attempt {failures}, {} pending, retrying in {:?}): {error}",
                    projector.pending(),
                    delay
                );
                publish(&shared, projector.failure_status(&error, failures));
                wait_for_work(&shared, Some(delay));
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
}

impl WorkerSink {
    /// Starts the worker. `startup` is every indexable document the mind holds
    /// now; the worker's first act is to reconcile the collection against it.
    pub fn spawn<E, V>(embedder: E, index: V, instance: &Slug, startup: Vec<IndexEntry>, backoff: Backoff) -> Self
    where
        E: Embedder + Send + 'static,
        V: VectorIndex + Send + 'static,
    {
        let mut projector = Projector::new(embedder, index, instance);
        projector.want(startup);
        let shared: Lock = Arc::new((
            Mutex::new(Shared { inbox: Vec::new(), status: projector.progress_status(), closed: false }),
            Condvar::new(),
        ));
        let worker = Arc::clone(&shared);
        std::thread::spawn(move || work(projector, worker, backoff));
        Self { shared }
    }

    /// Blocks until the health satisfies `predicate`, and returns it. For
    /// tests and probes that must observe the worker without a clock; it gives
    /// up after two minutes rather than hang a run.
    pub fn wait_status(&self, predicate: impl Fn(&IndexStatus) -> bool) -> IndexStatus {
        let mut guard = lock(&self.shared);
        loop {
            let seen = including(&guard.status, u32::try_from(guard.inbox.len()).unwrap_or(u32::MAX));
            if predicate(&seen) {
                return seen;
            }
            let (next, result) = self
                .shared
                .1
                .wait_timeout(guard, Duration::from_secs(120))
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard = next;
            assert!(!result.timed_out(), "the index never reached the awaited state; last seen {:?}", guard.status);
        }
    }
}

impl Drop for WorkerSink {
    fn drop(&mut self) {
        lock(&self.shared).closed = true;
        self.shared.1.notify_all();
    }
}

impl<S: MindStore> IndexSink<S> for WorkerSink {
    fn committed(&mut self, mind: &Mind<S>, writes: &[PipelineRef]) -> Result<()> {
        let entries = mind.index_entries(Some(writes))?;
        if entries.is_empty() {
            return Ok(());
        }
        lock(&self.shared).inbox.extend(entries);
        self.shared.1.notify_all();
        Ok(())
    }

    fn status(&self) -> IndexStatus {
        let guard = lock(&self.shared);
        including(&guard.status, u32::try_from(guard.inbox.len()).unwrap_or(u32::MAX))
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
