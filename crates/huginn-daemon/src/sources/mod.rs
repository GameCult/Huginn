//! The source index: the public GameCult forge repos' code and docs as chunks
//! in one Qdrant collection, `huginn_sources`, for semantic search.
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
//! The cursors object is written whole by this job alone, once per repo.

pub mod chunk;
pub mod forge;
pub mod git;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use huginn_mind::{SourceHit, SourceHits, SourceModel, SourceQuery};

use crate::index::{BATCH, BULK_EMBED_TIMEOUT, Budget, Embedder, INDEX_CALL_TIMEOUT, MANAGED_BY, ModelIdentity, point_id};
use chunk::{CHUNKER_VERSION, embedded_text};
use git::{BLOB_BYTES_MAX, Mirror, TreeFile};

/// The collection this organ's source index lives in.
pub const COLLECTION: &str = "huginn_sources";

/// What the collection says its corpus is.
pub const CORPUS: &str = "forge:GameCult";

/// The instruction Qwen3 embedding models take in front of a query about code.
pub const SOURCE_QUERY_INSTRUCTION: &str =
    "Given a question about a codebase, retrieve the source code and documentation passages that answer it";

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
    /// metadata, no cursors, and keyword indexes on `repo` and `path`.
    fn recreate_sources(&mut self, meta: &SourcesMeta) -> Result<()>;
    /// Replaces the cursors object whole.
    fn set_cursors(&mut self, cursors: &BTreeMap<String, String>) -> Result<()>;
    /// Deletes a repo's chunks: those at `paths`, or all of them when `None`.
    /// Acknowledged before it returns.
    fn delete_by(&mut self, repo: &str, paths: Option<&[String]>) -> Result<()>;
    /// Writes chunks, acknowledged before it returns.
    fn upsert_sources(&mut self, points: &[SourcePoint]) -> Result<()>;
    /// The `limit` nearest chunks, best first, in `repo` alone when given.
    fn search_sources(&mut self, vector: &[f32], repo: Option<&str>, limit: u32, within: Duration) -> Result<Vec<SourceRow>>;
}

/// A repo the forge lists as indexable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRepo {
    pub name: String,
    pub default_branch: String,
}

/// The forge: which repos exist, and where to fetch them from.
pub trait Forge {
    fn catalog(&mut self) -> Result<Vec<SourceRepo>>;
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

/// The names the refresh reports as failed; empty when every repo converged.
#[derive(Debug, Default)]
pub struct Refreshed {
    pub failed: Vec<String>,
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
    retire(index, &catalog, &mirrors, &mut cursors)?;

    let mut refreshed = Refreshed::default();
    for repo in &catalog {
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
fn retire<S: SourceIndex>(
    index: &mut S,
    catalog: &[SourceRepo],
    mirrors: &Path,
    cursors: &mut BTreeMap<String, String>,
) -> Result<()> {
    let listed = |name: &str| catalog.iter().any(|repo| repo.name.eq_ignore_ascii_case(name));
    let mut gone: BTreeSet<String> = cursors.keys().filter(|name| !listed(name)).cloned().collect();
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
        index.delete_by(&name, None).with_context(|| format!("deleting the chunks of retired {name}"))?;
        if cursors.remove(&name).is_some() {
            index.set_cursors(cursors).context("dropping a retired repo's cursor")?;
        }
        let mirror = Mirror::path_for(mirrors, &name);
        if mirror.exists() {
            std::fs::remove_dir_all(&mirror).with_context(|| format!("removing {}", mirror.display()))?;
        }
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
    match &changed {
        Some(paths) => {
            let paths: Vec<String> = paths.iter().cloned().collect();
            for slice in paths.chunks(DELETE_PATHS_PER_CALL) {
                index.delete_by(&repo.name, Some(slice)).context("deleting the changed paths' chunks")?;
            }
        }
        None => index.delete_by(&repo.name, None).context("deleting the repo's chunks")?,
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

/// A source search, which a daemon answers from the collection as it finds it:
/// it embeds the query with its own embedder, and refuses if the collection
/// was built under another model, because vectors of two models do not compare.
pub fn search<E: Embedder, S: SourceIndex>(embedder: &mut E, index: &mut S, query: &SourceQuery, within: Duration) -> Result<SourceHits> {
    let budget = Budget::new(within);
    let built = match index.describe_sources(budget.left()?).context("describing the source collection")? {
        DescribedSources::Absent => bail!("source index not built"),
        DescribedSources::Unlabelled => bail!("source index not built: {COLLECTION} carries no Huginn metadata"),
        DescribedSources::Labelled { meta, .. } => meta,
    };
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
    let text = format!("Instruct: {SOURCE_QUERY_INSTRUCTION}\nQuery: {}", query.text.0);
    let mut vectors = embedder.embed(&[text], budget.left()?).context("embedding the query")?;
    ensure!(vectors.len() == 1, "the embedder returned {} vectors for one query", vectors.len());
    let vector = vectors.remove(0);
    ensure!(vector.len() == identity.dimensions as usize, "the embedder returned a query vector of {} where the collection holds {}", vector.len(), identity.dimensions);
    let rows = index.search_sources(&vector, query.repo.as_ref().map(|repo| repo.0.as_str()), query.top_k, budget.left()?)?;
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
