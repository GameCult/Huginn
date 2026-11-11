//! The source index: the public GameCult forge repos' code and docs, and the
//! Minds' chat rooms, as chunks in one Qdrant collection, `huginn_sources`,
//! for semantic search. Each point's `corpus` payload (`forge` or `chat`) keeps
//! the two apart; a forge search filters `forge` and a chat search `chat`.
//!
//! Two truths, one writer. The forge is the truth of which repos exist and what
//! they hold; the collection is derived from it and can be deleted whenever.
//! `refresh` is the only writer of the collection, run by the `huginn-sources`
//! oneshot job: first build, incremental refresh, rebuild after a model or
//! chunker change, and retirement are all the one routine (delete by filter,
//! ingest at a commit, rewrite the cursors). The daemon only reads: `search`
//! is its whole part, and nothing in `serve` or the worker calls a write.
//!
//! A cursor is the commit whose chunks the collection holds for a repo. It
//! lives in the collection's own metadata, so it is lost with the collection
//! and the two cannot disagree about what is held. It is advanced only after
//! the repo's deletes and upserts are acknowledged: it may lag and never leads.
//! It is dropped before the repo's first delete, so a run that fails between
//! the delete and the upserts leaves no cursor, and the repo is ingested whole.
//! The cursors object is written whole by this job alone, once per repo.

pub mod chat;
pub mod chat_chunk;
pub mod chunk;
pub mod forge;
pub mod git;
pub mod room_scope;

#[cfg(test)]
mod tests;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use huginn_mind::{ChatHit, ChatHits, ChatMessage, ChatQuery, ChatWindow, ChatWindowQuery, MindRefusal, SOURCE_TOP_K_MAX, SourceHit, SourceHits, SourceModel, SourceQuery};

use crate::index::{BATCH, BULK_EMBED_TIMEOUT, Budget, Embedder, INDEX_CALL_TIMEOUT, MANAGED_BY, ModelIdentity, point_id};
use chunk::{CHUNKER_VERSION, embedded_text};
use git::{BLOB_BYTES_MAX, Mirror, TreeFile};

/// The collection this organ's source index lives in.
pub const COLLECTION: &str = "huginn_sources";

/// The paths, exactly and at a repo's root, that are its front page: what an
/// agent reads first to learn what the repo is. The only owner of the list.
pub const FRONT_PAGE_FILES: [&str; 6] = ["README.md", "README", "readme.md", "README.markdown", "CLAUDE.md", "AGENTS.md"];

/// What the collection says its corpus is: the set of corpora it holds, each
/// point's `corpus` payload naming which (`forge` or `chat`). A collection
/// labelled with any other set is rebuilt.
pub const CORPUS: &str = "forge:GameCult+chat:rooms";

/// The `corpus` payload of a forge point, and of a chat point.
pub const CORPUS_FORGE: &str = "forge";
pub const CORPUS_CHAT: &str = "chat";

/// The instruction Qwen3 embedding models take in front of a question about
/// past conversation.
pub const CHAT_QUERY_INSTRUCTION: &str =
    "Given a question about past conversation in chat rooms, retrieve the chat passages that answer it";

/// The instruction Qwen3 embedding models take in front of a query about code.
pub const SOURCE_QUERY_INSTRUCTION: &str =
    "Given a question about a codebase, retrieve the source code and documentation passages that answer it";

/// The constant of reciprocal rank fusion, score = sum over lists of
/// 1 / (FUSE_K + rank), rank 1-based: Cormack, Clarke and Buttcher, SIGIR 2009,
/// whose value this is. The one calibration constant of a multi-text search.
pub const FUSE_K: u32 = 60;

/// How many chunks each text's list holds before the lists are fused.
pub const FUSE_DEPTH: u32 = SOURCE_TOP_K_MAX;

/// Paths named in one delete call.
const DELETE_PATHS_PER_CALL: usize = 256;

/// What the collection says about how it was made. A model, a digest, a length
/// or a chunker version that differs from the embedder's and this build's makes
/// every vector in it incomparable, so it is rebuilt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcesMeta {
    pub managed_by: String,
    pub corpus: String,
    pub model: String,
    pub model_digest: String,
    pub dimensions: u32,
    pub chunker_version: u32,
}

impl SourcesMeta {
    /// The model the collection was built with.
    pub fn model_identity(&self) -> ModelIdentity {
        ModelIdentity { name: self.model.clone(), digest: self.model_digest.clone(), dimensions: self.dimensions }
    }
}

/// What the vector store holds under `COLLECTION`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescribedSources {
    Absent,
    /// A collection that carries no metadata this organ wrote.
    Unlabelled,
    Labelled { meta: SourcesMeta, cursors: BTreeMap<String, String> },
}

/// What travels beside a chunk's vector.
#[derive(Clone, Debug, PartialEq)]
pub struct SourcePayload {
    pub repo: String,
    pub path: String,
    pub commit: String,
    pub blob: String,
    pub line_start: u32,
    pub line_end: u32,
    pub chunk_index: u32,
    pub ext: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourcePoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: SourcePayload,
}

/// What travels beside a chat chunk's vector: consecutive messages of one
/// room, never spanning rooms. The single owner of the chat point's shape; the
/// ingest job is its only writer (through `upsert_chat`) and nothing else of the
/// payload is read. `messages` are the display truth and hold, in order, at
/// least one message. The chunk's span, its first and last event ids and its
/// `event_ids` are derived from them by the codec, so they cannot disagree.
/// `text` is what was embedded: the room's name, then `time sender: body` per
/// message.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatPayload {
    pub room_id: String,
    /// Absent when the room has no canonical alias.
    pub room_alias: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: ChatPayload,
}

/// One chat chunk a vector search returned and how near it scored.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatRow {
    pub score: f32,
    pub payload: ChatPayload,
}

/// Which chat chunks a read keeps; the selectors compose as AND. `rooms` are
/// any-of room ids (`!`) or aliases (`#`); the span keeps chunks whose own
/// span overlaps [`after_ms`, `before_ms`]. `event_id` keeps the chunk holding
/// that message. `starts_from_ms` and `starts_to_ms` bound a chunk's own start
/// (inclusive), which is what a neighbour read walks by.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatFilter {
    pub rooms: Vec<String>,
    pub after_ms: Option<u64>,
    pub before_ms: Option<u64>,
    pub event_id: Option<String>,
    pub starts_from_ms: Option<u64>,
    pub starts_to_ms: Option<u64>,
}

/// Which end of the room's history a chat scroll begins at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatOrder {
    OldestFirst,
    NewestFirst,
}

impl ChatFilter {
    /// Whether a chunk of `room_id`/`room_alias` spanning [`start`, `end`]
    /// passes. The fake index and the tests read the filter through this; the
    /// Qdrant index states the same thing as conditions.
    pub fn keeps(&self, room_id: &str, room_alias: Option<&str>, start: u64, end: u64) -> bool {
        (self.rooms.is_empty() || self.rooms.iter().any(|room| room == room_id || Some(room.as_str()) == room_alias))
            && self.after_ms.is_none_or(|after| end >= after)
            && self.before_ms.is_none_or(|before| start <= before)
            && self.starts_from_ms.is_none_or(|from| start >= from)
            && self.starts_to_ms.is_none_or(|to| start <= to)
    }
}

/// A chat chunk as the index holds it, with its point id: what the ingest job
/// reads back to rewrite a chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredChat {
    pub id: String,
    pub payload: ChatPayload,
}

/// One chunk a vector search returned and how near it scored.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRow {
    pub score: f32,
    pub payload: SourcePayload,
}

/// The vector store as the source index uses it. The writes are
/// `huginn-sources`'s alone; `describe_sources` and `search_sources` are what a
/// daemon calls.
pub trait SourceIndex {
    fn describe_sources(&mut self, within: Duration) -> Result<DescribedSources>;
    /// Drops the collection if it exists and creates it empty with this
    /// metadata, no cursors, keyword indexes on `repo`, `path`, `corpus`,
    /// `room_id`, `room_alias` and `event_ids`, and integer indexes on
    /// `ts_start_ms` and `ts_end_ms`.
    fn recreate_sources(&mut self, meta: &SourcesMeta) -> Result<()>;
    /// Replaces the cursors object whole.
    fn set_cursors(&mut self, cursors: &BTreeMap<String, String>) -> Result<()>;
    /// Deletes a repo's chunks: those at `paths`, or all of them when `None`;
    /// with `keeping`, only those not stamped with that commit. Acknowledged
    /// before it returns.
    fn delete_by(&mut self, repo: &str, paths: Option<&[String]>, keeping: Option<&str>) -> Result<()>;
    /// Writes chunks, acknowledged before it returns.
    fn upsert_sources(&mut self, points: &[SourcePoint]) -> Result<()>;
    /// The `limit` nearest chunks, best first, in `repo` alone when given, and
    /// among the `FRONT_PAGE_FILES` alone when `front`.
    fn search_sources(&mut self, vector: &[f32], repo: Option<&str>, front: bool, limit: u32, within: Duration) -> Result<Vec<SourceRow>>;
    /// Writes chat chunks, acknowledged before it returns. The ingest job's
    /// alone: the daemon never calls it. A chunk with no messages is refused.
    fn upsert_chat(&mut self, points: &[ChatPoint]) -> Result<()>;
    /// The `limit` chat chunks nearest `vector`, best first, among those
    /// `filter` keeps. Reads only `corpus=chat` points; the daemon's.
    fn chat_nearest(&mut self, vector: &[f32], filter: &ChatFilter, limit: u32, within: Duration) -> Result<Vec<ChatRow>>;
    /// Up to `limit` chat chunks `filter` keeps, by span start in `order`.
    /// Reads only `corpus=chat` points; the daemon's.
    fn chat_scroll(&mut self, filter: &ChatFilter, order: ChatOrder, limit: u32, within: Duration) -> Result<Vec<ChatPayload>>;
    /// The room's chunk with the greatest `ts_end_ms`, if it has any. The ingest job's alone.
    fn chat_tail(&mut self, room_id: &str) -> Result<Option<StoredChat>>;
    /// The room's chunks holding any of `event_ids`. The ingest job's alone.
    fn chat_chunks_holding(&mut self, room_id: &str, event_ids: &[String]) -> Result<Vec<StoredChat>>;
    /// The room's chunks whose span reaches into [`from_ms`, `to_ms`): `ts_end_ms >= from_ms`
    /// and `ts_start_ms < to_ms`. The ingest job's alone.
    fn chat_chunks_overlapping(&mut self, room_id: &str, from_ms: u64, to_ms: u64) -> Result<Vec<StoredChat>>;
    /// Deletes a room's chat chunks: those with the point ids `ids`, or all of them when `None`.
    /// Acknowledged before it returns. Never touches a forge point. The ingest job's alone.
    fn delete_chat(&mut self, room_id: &str, ids: Option<&[String]>) -> Result<()>;
}

/// A repo the forge lists as indexable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRepo {
    pub name: String,
    pub default_branch: String,
}

/// The repos the forge listed as indexable, and whether the list is the whole
/// of what the forge holds. A list that is not whole cannot say which repos
/// are gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub repos: Vec<SourceRepo>,
    /// Why the list may be missing repos; `None` when it is the forge's count.
    pub shortfall: Option<String>,
}

/// The forge: which repos exist, and where to fetch them from.
pub trait Forge {
    fn catalog(&mut self) -> Result<Catalog>;
    fn clone_url(&self, name: &str) -> String;
}

/// What one repo's refresh did, for the journal.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub name: String,
    pub old: Option<String>,
    pub new: String,
    pub files: u32,
    pub chunks: u32,
    pub seconds: f64,
}

impl std::fmt::Display for Report {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {}..{} files={} chunks={} seconds={:.1}",
            self.name,
            self.old.as_deref().map_or("-", |old| &old[..old.len().min(7)]),
            &self.new[..self.new.len().min(7)],
            self.files,
            self.chunks,
            self.seconds
        )
    }
}

/// Where the mirrors live under the state root.
pub fn mirrors_dir(state_root: &Path) -> std::path::PathBuf {
    state_root.join("sources").join("repos")
}

/// The id of a chunk: content-addressed by where it is, which blob it was cut
/// from, the chunker and its position, so the same blob chunked the same way
/// is the same point.
pub fn chunk_id(repo: &str, path: &str, blob: &str, index: u32) -> String {
    point_id(&format!("{repo}\0{path}\0{blob}\0{CHUNKER_VERSION}\0{index}"))
}

fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase()).unwrap_or_default()
}

/// What a refresh reports: the names that failed, and why nothing was retired
/// if the catalog could not be trusted to say which repos are gone.
#[derive(Debug, Default)]
pub struct Refreshed {
    pub failed: Vec<String>,
    /// Set when retirement was skipped because the catalog was empty or short.
    pub retirement_withheld: Option<String>,
}

impl Refreshed {
    /// Every repo converged and the catalog could be trusted to retire from.
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.retirement_withheld.is_none()
    }
}

/// Brings the collection to the forge's current state. A collection that
/// exists without Huginn's label is refused untouched. One repo's failure is
/// logged and the others go on; the caller exits nonzero if any failed.
pub fn refresh<E: Embedder, S: SourceIndex, F: Forge>(
    embedder: &mut E,
    index: &mut S,
    forge: &mut F,
    state_root: &Path,
    on_repo: &mut dyn FnMut(&Report),
) -> Result<Refreshed> {
    let described = index.describe_sources(INDEX_CALL_TIMEOUT).context("describing the source collection")?;
    embedder.built_under(if let DescribedSources::Labelled { meta, .. } = &described { Some(meta.model_identity()) } else { None }.as_ref());
    let identity = embedder.model_identity(BULK_EMBED_TIMEOUT).context("reading the embedding model's identity")?;
    let wanted = SourcesMeta {
        managed_by: MANAGED_BY.into(),
        corpus: CORPUS.into(),
        model: identity.name.clone(),
        model_digest: identity.digest.clone(),
        dimensions: identity.dimensions,
        chunker_version: CHUNKER_VERSION,
    };
    let mut cursors = match described {
        DescribedSources::Absent => {
            index.recreate_sources(&wanted).context("creating the source collection")?;
            BTreeMap::new()
        }
        DescribedSources::Unlabelled => {
            bail!("collection {COLLECTION} exists and carries no Huginn metadata; it is not this job's to rewrite")
        }
        DescribedSources::Labelled { meta, cursors } if meta == wanted => cursors,
        DescribedSources::Labelled { .. } => {
            index.recreate_sources(&wanted).context("rebuilding the source collection")?;
            BTreeMap::new()
        }
    };

    let catalog = forge.catalog().context("listing the forge's repos")?;
    let mirrors = mirrors_dir(state_root);
    let mut refreshed = Refreshed::default();
    // Retirement reads the catalog as the complete list of what exists. An
    // empty or short list would retire everything it fails to name, so then
    // nothing is retired and the run says so.
    let withheld = match (&catalog.shortfall, catalog.repos.is_empty()) {
        (Some(why), _) => Some(format!("the forge's catalog is incomplete ({why})")),
        (None, true) => Some("the forge's catalog lists no indexable repos".to_owned()),
        (None, false) => None,
    };
    match withheld {
        Some(why) => {
            eprintln!("huginn-sources: retiring nothing: {why}");
            refreshed.retirement_withheld = Some(why);
        }
        None => retire(index, &catalog.repos, &mirrors, &mut cursors, &mut refreshed.failed),
    }

    for repo in &catalog.repos {
        let started = Instant::now();
        match refresh_repo(embedder, index, forge, &mirrors, repo, &identity.dimensions, &mut cursors) {
            Ok(mut report) => {
                report.seconds = started.elapsed().as_secs_f64();
                on_repo(&report);
            }
            Err(error) => {
                eprintln!("huginn-sources: {} failed, the others go on: {error:#}", repo.name);
                refreshed.failed.push(repo.name.clone());
            }
        }
    }
    Ok(refreshed)
}

/// Drops every repo the catalog no longer lists: its chunks, its cursor and
/// its mirror, in that order, so a failure leaves the cursor to find it again.
/// One repo's failure is logged and named in `failed`; the others go on. Names
/// are compared as written: a repo renamed only by case is a new repo to the
/// index (its points carry the old spelling), so the old one is retired.
fn retire<S: SourceIndex>(
    index: &mut S,
    catalog: &[SourceRepo],
    mirrors: &Path,
    cursors: &mut BTreeMap<String, String>,
    failed: &mut Vec<String>,
) {
    let listed = |name: &str| catalog.iter().any(|repo| repo.name == name);
    let mut gone: BTreeSet<String> = cursors.keys().filter(|name| !chat::is_chat_key(name) && !listed(name)).cloned().collect();
    if let Ok(entries) = std::fs::read_dir(mirrors) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_string_lossy().strip_suffix(".git")
                && !listed(name)
            {
                gone.insert(name.to_owned());
            }
        }
    }
    for name in gone {
        if let Err(error) = retire_one(index, &name, mirrors, cursors) {
            eprintln!("huginn-sources: retiring {name} failed, the others go on: {error:#}");
            failed.push(name);
        }
    }
}

fn retire_one<S: SourceIndex>(index: &mut S, name: &str, mirrors: &Path, cursors: &mut BTreeMap<String, String>) -> Result<()> {
    index.delete_by(name, None, None).with_context(|| format!("deleting the chunks of retired {name}"))?;
    if cursors.remove(name).is_some() {
        index.set_cursors(cursors).context("dropping a retired repo's cursor")?;
    }
    let mirror = Mirror::path_for(mirrors, name);
    if mirror.exists() {
        std::fs::remove_dir_all(&mirror).with_context(|| format!("removing {}", mirror.display()))?;
    }
    Ok(())
}

fn refresh_repo<E: Embedder, S: SourceIndex, F: Forge>(
    embedder: &mut E,
    index: &mut S,
    forge: &F,
    mirrors: &Path,
    repo: &SourceRepo,
    dimensions: &u32,
    cursors: &mut BTreeMap<String, String>,
) -> Result<Report> {
    let mirror = Mirror::new(mirrors, &repo.name, &repo.default_branch);
    let head = mirror.fetch(&forge.clone_url(&repo.name))?;
    let old = cursors.get(&repo.name).cloned();
    let mut report = Report { name: repo.name.clone(), old: old.clone(), new: head.clone(), files: 0, chunks: 0, seconds: 0.0 };
    if old.as_deref() == Some(head.as_str()) {
        return Ok(report);
    }
    // Only an ancestor's diff says what changed; anything else (a first
    // build, a rewritten history) starts the repo over.
    let changed: Option<BTreeSet<String>> = match &old {
        Some(old) if mirror.is_ancestor(old, &head) => Some(mirror.changed_paths(old, &head)?.into_iter().collect()),
        _ => None,
    };
    // From the first upsert on, the collection holds no single commit's chunks
    // for this repo until the run finishes. Say so before changing anything:
    // the cursor is dropped, so a run that dies half way leaves a repo the next
    // run re-ingests whole, whatever head it finds then. A cursor left at the
    // old commit would let a later head that reverts the files hide the hole.
    // Nothing is deleted until the new chunks are written (below), so a failed
    // embed leaves what the index held.
    if cursors.remove(&repo.name).is_some() {
        index.set_cursors(cursors).context("dropping the cursor before the repo's chunks change")?;
    }
    let files: Vec<TreeFile> = mirror
        .files(&head)?
        .into_iter()
        .filter(|file| changed.as_ref().is_none_or(|paths| paths.contains(&file.path)))
        .filter(|file| file.size <= BLOB_BYTES_MAX && !git::skipped_path(&file.path))
        .collect();
    let mut blobs = mirror.blobs()?;
    let mut batch = Batch::default();
    for file in &files {
        let Some(text) = git::indexable_text(&blobs.read(&file.blob)?) else { continue };
        let chunks = chunk::chunk(&text);
        if chunks.is_empty() {
            continue;
        }
        report.files += 1;
        for (position, held) in chunks.iter().enumerate() {
            let chunk_index = u32::try_from(position).unwrap_or(u32::MAX);
            batch.texts.push(embedded_text(&repo.name, &file.path, held));
            batch.payloads.push((
                chunk_id(&repo.name, &file.path, &file.blob, chunk_index),
                SourcePayload {
                    repo: repo.name.clone(),
                    path: file.path.clone(),
                    commit: head.clone(),
                    blob: file.blob.clone(),
                    line_start: held.line_start,
                    line_end: held.line_end,
                    chunk_index,
                    ext: extension(&file.path),
                    text: held.text.clone(),
                },
            ));
            report.chunks += 1;
            if batch.texts.len() >= BATCH {
                batch.write(embedder, index, *dimensions)?;
            }
        }
    }
    batch.write(embedder, index, *dimensions)?;
    // Every chunk at this head now carries it (a chunk id is its content, so a
    // rewritten one replaced its old self). What is left in scope with another
    // commit is stale: edited or deleted paths' chunks, or a repo's old self.
    match &changed {
        Some(paths) => {
            let paths: Vec<String> = paths.iter().cloned().collect();
            for slice in paths.chunks(DELETE_PATHS_PER_CALL) {
                index.delete_by(&repo.name, Some(slice), Some(&head)).context("deleting the changed paths' stale chunks")?;
            }
        }
        None => index.delete_by(&repo.name, None, Some(&head)).context("deleting the repo's stale chunks")?,
    }
    cursors.insert(repo.name.clone(), head);
    index.set_cursors(cursors).context("advancing the cursor")?;
    Ok(report)
}

#[derive(Default)]
struct Batch {
    texts: Vec<String>,
    payloads: Vec<(String, SourcePayload)>,
}

impl Batch {
    /// Embeds and writes what has gathered; the batch is empty afterwards.
    fn write<E: Embedder, S: SourceIndex>(&mut self, embedder: &mut E, index: &mut S, dimensions: u32) -> Result<()> {
        if self.texts.is_empty() {
            return Ok(());
        }
        let vectors = embedder.embed(&self.texts, BULK_EMBED_TIMEOUT).context("embedding chunks")?;
        ensure!(vectors.len() == self.texts.len(), "the embedder returned {} vectors for {} chunks", vectors.len(), self.texts.len());
        let mut points = Vec::with_capacity(vectors.len());
        for ((id, payload), vector) in self.payloads.drain(..).zip(vectors) {
            ensure!(vector.len() == dimensions as usize, "the embedder returned a vector of {} where the collection holds {dimensions}", vector.len());
            points.push(SourcePoint { id, vector, payload });
        }
        self.texts.clear();
        index.upsert_sources(&points).context("writing chunks")
    }
}

/// Reciprocal rank fusion of ranked lists (best first): an item's score is the
/// sum over the lists that hold it of `1 / (k + rank)`, rank 1-based, and items
/// with the same `key` are one item (the first list's copy is kept). Best first,
/// the first-seen item ahead on a tie, cut to `top_k`. Pure: the one fuse, which
/// every search that ranks across lists calls and none copies.
pub fn fuse<T>(lists: Vec<Vec<T>>, key: impl Fn(&T) -> String, k: u32, top_k: usize) -> Vec<(f32, T)> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut fused: Vec<(f32, T)> = Vec::new();
    for list in lists {
        for (rank, item) in list.into_iter().enumerate() {
            let share = 1.0 / (k as f32 + rank as f32 + 1.0);
            match seen.entry(key(&item)) {
                Entry::Occupied(at) => fused[*at.get()].0 += share,
                Entry::Vacant(at) => {
                    at.insert(fused.len());
                    fused.push((share, item));
                }
            }
        }
    }
    fused.sort_by(|a, b| b.0.total_cmp(&a.0));
    fused.truncate(top_k);
    fused
}

/// The texts a query searches by: `texts`, or an old client's `text` as a list
/// of one, trimmed, and each distinct text once, in the order asked.
fn query_texts(query: &SourceQuery) -> Vec<&str> {
    let asked: Vec<&str> = if query.texts.is_empty() { vec![query.text.0.as_str()] } else { query.texts.iter().map(|text| text.0.as_str()).collect() };
    distinct_texts(asked)
}

fn distinct_texts(asked: Vec<&str>) -> Vec<&str> {
    let mut distinct: Vec<&str> = Vec::with_capacity(asked.len());
    for text in asked {
        if !distinct.iter().any(|seen| seen.trim() == text.trim()) {
            distinct.push(text);
        }
    }
    distinct
}

/// The label of the collection a daemon is about to read, if it was built
/// under the model this daemon embeds with. Absent, unlabelled and other-model
/// collections are loud refusals: vectors of two models do not compare, and an
/// empty page is not an answer. A collection built for another corpus set (an
/// old forge-only one) is refused the same way until the rebuild lands, since
/// it would answer a chat search with an empty page.
fn readable_under_this_model<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, budget: &Budget) -> Result<(SourcesMeta, ModelIdentity)> {
    let built = built_label(index, budget)?;
    let identity = embedder.model_identity(budget.left()?).context("reading the embedding model's identity")?;
    ensure!(
        built.model == identity.name && built.model_digest == identity.digest && built.dimensions == identity.dimensions,
        "the source index was built under {} {} ({} dimensions) and this daemon embeds under {} {} ({} dimensions)",
        built.model,
        built.model_digest,
        built.dimensions,
        identity.name,
        identity.digest,
        identity.dimensions
    );
    Ok((built, identity))
}

fn built_label<S: SourceIndex>(index: &mut S, budget: &Budget) -> Result<SourcesMeta> {
    match index.describe_sources(budget.left()?).context("describing the source collection")? {
        DescribedSources::Absent => bail!("source index not built"),
        DescribedSources::Unlabelled => bail!("source index not built: {COLLECTION} carries no Huginn metadata"),
        DescribedSources::Labelled { meta, .. } => {
            ensure!(
                meta.corpus == CORPUS,
                "the source index is stale: it was built for corpus {} and this daemon reads {CORPUS}; the rebuild is pending",
                meta.corpus
            );
            Ok(meta)
        }
    }
}

/// A source search, which a daemon answers from the collection as it finds it:
/// it embeds the query with its own embedder, and refuses if the collection
/// was built under another model, because vectors of two models do not compare.
pub fn search<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, query: &SourceQuery, within: Duration) -> Result<SourceHits> {
    let budget = Budget::new(within);
    let (built, identity) = readable_under_this_model(embedder, index, &budget)?;
    let topics = query_texts(query);
    let prompts: Vec<String> = topics.iter().map(|topic| format!("Instruct: {SOURCE_QUERY_INSTRUCTION}\nQuery: {topic}")).collect();
    let vectors = embedder.embed(&prompts, budget.left()?).context("embedding the query")?;
    ensure!(vectors.len() == prompts.len(), "the embedder returned {} vectors for {} query texts", vectors.len(), prompts.len());
    let repo = query.repo.as_ref().map(|repo| repo.0.as_str());
    let depth = if vectors.len() == 1 { query.top_k } else { FUSE_DEPTH };
    let mut lists = Vec::with_capacity(vectors.len());
    for vector in &vectors {
        ensure!(vector.len() == identity.dimensions as usize, "the embedder returned a query vector of {} where the collection holds {}", vector.len(), identity.dimensions);
        lists.push(index.search_sources(vector, repo, query.front, depth, budget.left()?)?);
    }
    let rows = if lists.len() == 1 {
        lists.remove(0)
    } else {
        let key = |row: &SourceRow| chunk_id(&row.payload.repo, &row.payload.path, &row.payload.blob, row.payload.chunk_index);
        fuse(lists, key, FUSE_K, query.top_k as usize).into_iter().map(|(score, row)| SourceRow { score, ..row }).collect()
    };
    Ok(SourceHits {
        indexed_under: SourceModel { name: built.model, digest: built.model_digest },
        hits: rows
            .into_iter()
            .map(|row| SourceHit {
                repo: row.payload.repo,
                path: row.payload.path,
                commit: row.payload.commit,
                line_start: row.payload.line_start,
                line_end: row.payload.line_end,
                score: row.score,
                text: row.payload.text,
            })
            .collect(),
    })
}

/// A chat search. With texts, the chunks nearest them among those the room and
/// span selectors keep, several texts fused by `fuse`; with none, a listing of
/// what the selectors keep, oldest first, `top_k` of it and `truncated` saying
/// more exist (no embedding, so the model's identity is not consulted, though
/// an absent collection is still refused). A hit outside the span is not
/// returned and a boundary hit loses its messages outside [after, before], its
/// span and first and last ids recomputed from what is left. Never writes.
pub fn search_chat<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, query: &ChatQuery, within: Duration) -> Result<ChatHits> {
    let budget = Budget::new(within);
    let filter = ChatFilter { rooms: query.rooms.iter().map(|room| room.0.clone()).collect(), after_ms: query.after_ms, before_ms: query.before_ms, ..ChatFilter::default() };
    let top_k = query.top_k as usize;
    let topics = distinct_texts(query.texts.iter().map(|text| text.0.as_str()).collect());
    let (built, rows, truncated) = if topics.is_empty() {
        let built = built_label(index, &budget)?;
        let mut held = index.chat_scroll(&filter, ChatOrder::OldestFirst, query.top_k + 1, budget.left()?)?;
        let truncated = held.len() > top_k;
        held.truncate(top_k);
        (built, held.into_iter().map(|payload| (None, payload)).collect::<Vec<_>>(), truncated)
    } else {
        let (built, identity) = readable_under_this_model(embedder, index, &budget)?;
        let prompts: Vec<String> = topics.iter().map(|topic| format!("Instruct: {CHAT_QUERY_INSTRUCTION}\nQuery: {topic}")).collect();
        let vectors = embedder.embed(&prompts, budget.left()?).context("embedding the query")?;
        ensure!(vectors.len() == prompts.len(), "the embedder returned {} vectors for {} query texts", vectors.len(), prompts.len());
        let depth = if vectors.len() == 1 { query.top_k } else { FUSE_DEPTH };
        let mut lists = Vec::with_capacity(vectors.len());
        for vector in &vectors {
            ensure!(vector.len() == identity.dimensions as usize, "the embedder returned a query vector of {} where the collection holds {}", vector.len(), identity.dimensions);
            lists.push(index.chat_nearest(vector, &filter, depth, budget.left()?)?);
        }
        let rows: Vec<ChatRow> = if lists.len() == 1 {
            lists.remove(0)
        } else {
            let key = |row: &ChatRow| format!("{}\0{}", row.payload.room_id, row.payload.messages.first().map_or("", |message| message.event_id.as_str()));
            fuse(lists, key, FUSE_K, top_k).into_iter().map(|(score, row)| ChatRow { score, ..row }).collect()
        };
        (built, rows.into_iter().map(|row| (Some(row.score), row.payload)).collect(), false)
    };
    Ok(ChatHits {
        indexed_under: SourceModel { name: built.model, digest: built.model_digest },
        truncated,
        hits: rows.into_iter().filter_map(|(score, payload)| chat_hit(score, payload, &filter)).collect(),
    })
}

/// Chunks a neighbour read asks for at a time, and the most such reads each way.
const WINDOW_CHUNKS_PER_SCROLL: u32 = 5;
const WINDOW_SCROLLS_MAX: usize = 6;

/// The message a chat window is about and its neighbours, read from the chat
/// index alone: the chunk holding `event_id`, then chunks of that room before
/// and after it until the asked count of messages is held, the room ends or
/// `WINDOW_SCROLLS_MAX` reads are spent. An id the index does not hold is a
/// typed refusal naming `event_id`, never an empty window; an id in two chunks
/// is an index fault. Needs no embedder. Never writes.
pub fn window<S: SourceIndex>(index: &mut S, query: &ChatWindowQuery, within: Duration) -> Result<Result<ChatWindow, MindRefusal>> {
    let budget = Budget::new(within);
    built_label(index, &budget)?;
    let id = query.event_id.0.as_str();
    let anchor_filter = ChatFilter { event_id: Some(id.to_owned()), ..ChatFilter::default() };
    let mut found = index.chat_scroll(&anchor_filter, ChatOrder::OldestFirst, 2, budget.left()?)?;
    ensure!(found.len() < 2, "the chat index holds event {id} in two chunks: an ingest fault, not a window");
    let Some(anchor) = found.pop() else {
        return Ok(Err(MindRefusal::SelectionInvalid {
            field: "event_id".into(),
            value: Some(id.to_owned()),
            message: "the chat index does not hold this event: it may not be ingested yet, it may have been removed (redacted, or purged by a steward request), or it may be in a room the index does not read (a DM, #users, #members, #patrons, any room the reader has not joined)".into(),
        }));
    };
    let start_of = |chunk: &ChatPayload| chunk.messages.iter().map(|message| message.ts_ms).min().unwrap_or(0);
    let first_id = |chunk: &ChatPayload| chunk.messages.first().map(|message| message.event_id.clone()).unwrap_or_default();
    let room = ChatFilter { rooms: vec![anchor.room_id.clone()], ..ChatFilter::default() };
    let at = anchor.messages.iter().position(|message| message.event_id == id).context("the chat index returned a chunk that does not hold the event it was asked for")?;
    // Chunks held so far as (first event id, start): the anchor's, then each walk's.
    let mut held = vec![(first_id(&anchor), start_of(&anchor))];
    let mut walk = |forward: bool, want: usize, mine: usize| -> Result<(Vec<ChatPayload>, bool)> {
        let (mut chunks, mut count, mut edge, mut exhausted) = (Vec::new(), mine, start_of(&anchor), false);
        for _ in 0..WINDOW_SCROLLS_MAX {
            if count > want {
                break;
            }
            let (filter, order) = if forward {
                (ChatFilter { starts_from_ms: Some(edge), ..room.clone() }, ChatOrder::OldestFirst)
            } else {
                (ChatFilter { starts_to_ms: Some(edge), ..room.clone() }, ChatOrder::NewestFirst)
            };
            // Held chunks starting at the edge come back first with any others
            // that start there; asking for that many extra means a run of
            // chunks at one millisecond cannot fill a read with chunks held.
            let limit = WINDOW_CHUNKS_PER_SCROLL + held.iter().filter(|(_, start)| *start == edge).count() as u32;
            let read = index.chat_scroll(&filter, order, limit, budget.left()?)?;
            exhausted = (read.len() as u32) < limit;
            for chunk in read {
                let key = (first_id(&chunk), start_of(&chunk));
                if held.contains(&key) {
                    continue;
                }
                edge = if forward { key.1.max(edge) } else { key.1.min(edge) };
                held.push(key);
                count += chunk.messages.len();
                chunks.push(chunk);
            }
            if exhausted {
                break;
            }
        }
        // Stopping short of the asked count without reaching the room's end
        // means messages remain that this read did not reach.
        Ok((chunks, count <= want && !exhausted))
    };
    let (earlier, unreached_before) = walk(false, query.before as usize, at)?;
    let (later, unreached_after) = walk(true, query.after as usize, anchor.messages.len() - at - 1)?;
    // Chronological by (ts, chunk start, chunk first event id, position), so
    // the order does not depend on which message anchored the read.
    let mut keyed: Vec<(u64, u64, String, usize, ChatMessage)> = Vec::new();
    for chunk in earlier.iter().chain(std::iter::once(&anchor)).chain(later.iter()) {
        let (start, first) = (start_of(chunk), first_id(chunk));
        keyed.extend(chunk.messages.iter().enumerate().map(|(position, message)| (message.ts_ms, start, first.clone(), position, message.clone())));
    }
    keyed.sort_by(|a, b| (a.0, a.1, &a.2, a.3).cmp(&(b.0, b.1, &b.2, b.3)));
    let (mut seen, mut messages) = (std::collections::BTreeSet::new(), Vec::new());
    for (.., message) in keyed {
        if seen.insert(message.event_id.clone()) {
            messages.push(message);
        }
    }
    let at = messages.iter().position(|message| message.event_id == id).context("the anchor fell out of its own window")?;
    let (lo, hi) = (at.saturating_sub(query.before as usize), (at + 1 + query.after as usize).min(messages.len()));
    Ok(Ok(ChatWindow {
        room_id: anchor.room_id,
        room_alias: anchor.room_alias,
        anchor_event_id: id.to_owned(),
        more_before: unreached_before || lo > 0,
        more_after: unreached_after || hi < messages.len(),
        messages: messages[lo..hi].to_vec(),
    }))
}

/// A held chunk as a hit: only the messages inside the asked span, the span and
/// ids those messages make, and nothing at all when none is inside.
fn chat_hit(score: Option<f32>, payload: ChatPayload, filter: &ChatFilter) -> Option<ChatHit> {
    let messages: Vec<ChatMessage> = payload
        .messages
        .into_iter()
        .filter(|message| filter.after_ms.is_none_or(|after| message.ts_ms >= after) && filter.before_ms.is_none_or(|before| message.ts_ms <= before))
        .collect();
    let (first, last) = (messages.first()?, messages.last()?);
    Some(ChatHit {
        room_id: payload.room_id,
        room_alias: payload.room_alias,
        first_event_id: first.event_id.clone(),
        last_event_id: last.event_id.clone(),
        ts_start_ms: messages.iter().map(|message| message.ts_ms).min()?,
        ts_end_ms: messages.iter().map(|message| message.ts_ms).max()?,
        score,
        messages,
    })
}
