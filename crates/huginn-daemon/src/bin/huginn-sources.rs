//! The source index's refresh job: one run brings the `huginn_sources`
//! collection to the forge's current state, journals one line per repo, and
//! exits nonzero if any repo failed or the collection could not be used.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, ensure};
use huginn_daemon::index::ollama::OllamaEmbedder;
use huginn_daemon::index::pool::EmbedderPool;
use huginn_daemon::index::qdrant::QdrantIndex;
use huginn_daemon::sources::forge::HttpForge;
use huginn_daemon::sources::refresh;

/// Where the forge answers when the flag is absent: the loopback address of the
/// forge on the host that runs this job.
const DEFAULT_FORGE_URL: &str = "http://127.0.0.1:3310";

struct Options {
    state_root: PathBuf,
    qdrant_url: String,
    /// The GPU hosts: all take batches at once.
    ollama_urls: Vec<String>,
    /// The CPU host: takes batches only while no GPU host can.
    reserve_urls: Vec<String>,
    embedding_model: String,
    forge_url: String,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Options> {
    let mut args = args;
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut ollama_urls = Vec::new();
    let mut reserve_urls = Vec::new();
    while let Some(name) = args.next() {
        let name = name.strip_prefix("--").with_context(|| format!("expected --option, got {name:?}"))?;
        ensure!(
            matches!(name, "state-root" | "qdrant-url" | "ollama-url" | "reserve-ollama-url" | "embedding-model" | "forge-url"),
            "unsupported huginn-sources option --{name}"
        );
        let value = args.next().with_context(|| format!("missing value for --{name}"))?;
        if name == "ollama-url" || name == "reserve-ollama-url" {
            ensure!(value.starts_with("http://"), "--{name} must be a plain http:// address, got {value:?}");
            if name == "ollama-url" { ollama_urls.push(value) } else { reserve_urls.push(value) }
        } else {
            ensure!(values.insert(name.to_owned(), value).is_none(), "duplicate option --{name}");
        }
    }
    let take = |name: &str| values.get(name).cloned().with_context(|| format!("--{name} is required"));
    let url = |name: &str, value: String| {
        ensure!(value.starts_with("http://"), "--{name} must be a plain http:// address, got {value:?}");
        Ok(value)
    };
    let state_root = PathBuf::from(take("state-root")?);
    ensure!(state_root.is_absolute(), "--state-root must be an absolute path");
    let embedding_model = take("embedding-model")?;
    ensure!(!embedding_model.is_empty(), "--embedding-model must name a model");
    Ok(Options {
        state_root,
        qdrant_url: url("qdrant-url", take("qdrant-url")?)?,
        ollama_urls: {
            ensure!(!ollama_urls.is_empty(), "at least one --ollama-url is required; repeat it for each GPU host");
            ollama_urls
        },
        reserve_urls,
        embedding_model,
        forge_url: url("forge-url", values.get("forge-url").cloned().unwrap_or_else(|| DEFAULT_FORGE_URL.to_owned()))?,
    })
}

fn main() -> ExitCode {
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("huginn-sources: {error:#}");
            return ExitCode::from(2);
        }
    };
    let endpoint = |url: &String| (url.clone(), OllamaEmbedder::new(url, &options.embedding_model));
    let mut embedder = EmbedderPool::new(options.ollama_urls.iter().map(endpoint).collect(), options.reserve_urls.iter().map(endpoint).collect());
    let mut index = QdrantIndex::new(&options.qdrant_url);
    let mut forge = HttpForge::new(&options.forge_url);
    let mut journal = |report: &huginn_daemon::sources::Report| println!("{report}");
    match refresh(&mut embedder, &mut index, &mut forge, &options.state_root, &mut journal) {
        Ok(refreshed) if refreshed.failed.is_empty() => ExitCode::SUCCESS,
        Ok(refreshed) => {
            eprintln!("huginn-sources: {} repos failed: {}", refreshed.failed.len(), refreshed.failed.join(", "));
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("huginn-sources: {error:#}");
            ExitCode::from(1)
        }
    }
}
