//! The embedder port over Ollama's HTTP API. This file and `qdrant.rs` are the
//! xenos boundary: JSON exists here and nowhere else in the organ.

use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

use super::{Embedder, Http, Method, ModelIdentity};

/// The instruction Qwen3 embedding models take in front of a query (never in
/// front of a document). Cut 11b's semantic read prepends it as
/// `Instruct: <this>\nQuery: <text>`.
pub const QUERY_INSTRUCTION: &str =
    "Given a question about a project's pipeline state, retrieve the documents that answer it";

/// What an embed call may take, sized to the batch and never beyond the
/// caller's budget: a host that froze after accepting the batch is given up on
/// after `floor + per_text * texts`, not after the caller's whole `within`.
#[derive(Clone, Copy)]
pub struct EmbedBounds {
    /// The bound on connecting alone.
    pub connect: Duration,
    /// What any batch is allowed, a model load included.
    pub floor: Duration,
    /// What each text adds.
    pub per_text: Duration,
}

impl EmbedBounds {
    pub const DEFAULT: Self =
        Self { connect: Duration::from_secs(3), floor: Duration::from_secs(20), per_text: Duration::from_millis(500) };

    fn for_batch(&self, texts: usize, within: Duration) -> Duration {
        (self.floor + self.per_text.saturating_mul(u32::try_from(texts).unwrap_or(u32::MAX))).min(within)
    }
}

/// One model on one Ollama. Every call carries its own bound.
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    bounds: EmbedBounds,
}

impl OllamaEmbedder {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self { base_url: base_url.to_owned(), model: model.to_owned(), bounds: EmbedBounds::DEFAULT }
    }

    pub fn bounded_by(mut self, bounds: EmbedBounds) -> Self {
        self.bounds = bounds;
        self
    }

    fn ask(&self, http: &Http, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let text = body.map(Value::to_string);
        let (status, answer) = http.request(method, path, text.as_deref())?;
        ensure!(status == 200, "Ollama answered {status} to {path}: {}", answer.chars().take(300).collect::<String>());
        serde_json::from_str(&answer).with_context(|| format!("Ollama's answer to {path} is not JSON"))
    }
}

impl Embedder for OllamaEmbedder {
    /// The configured name, the digest Ollama lists for it, and the model's
    /// embedding length. The address is not part of the identity.
    fn model_identity(&mut self, within: Duration) -> Result<ModelIdentity> {
        let http = Http::new(&self.base_url, within);
        let tags = self.ask(&http, Method::Get, "/api/tags", None)?;
        let listed = tags["models"].as_array().context("Ollama's tag list has no models")?;
        let digest = listed
            .iter()
            .find(|entry| entry["name"] == self.model.as_str() || entry["model"] == self.model.as_str())
            .and_then(|entry| entry["digest"].as_str())
            .with_context(|| format!("Ollama does not list the model {}", self.model))?
            .to_owned();
        let shown = self.ask(&http, Method::Post, "/api/show", Some(&json!({ "model": self.model })))?;
        let info = shown["model_info"].as_object().context("Ollama's model description has no model_info")?;
        let Some(length) = info.iter().find(|(key, _)| key.ends_with(".embedding_length")).and_then(|(_, value)| value.as_u64())
        else {
            bail!("Ollama does not say how long {}'s embeddings are", self.model);
        };
        let dimensions = u32::try_from(length).context("an embedding length beyond u32")?;
        Ok(ModelIdentity { name: self.model.clone(), digest, dimensions })
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        let allowed = self.bounds.for_batch(texts.len(), within);
        let bounded = Http::connecting_within(&self.base_url, allowed, Some(self.bounds.connect.min(allowed)));
        let answer = self.ask(&bounded, Method::Post, "/api/embed", Some(&json!({ "model": self.model, "input": texts })))?;
        let embeddings = answer["embeddings"].as_array().context("Ollama's answer has no embeddings")?;
        embeddings
            .iter()
            .map(|vector| {
                vector
                    .as_array()
                    .context("an embedding is not a list")?
                    .iter()
                    .map(|number| number.as_f64().map(|value| value as f32).context("an embedding holds a non-number"))
                    .collect()
            })
            .collect()
    }
}

/// One pool over Ollama endpoints serving `model`: the `primaries` (GPU hosts)
/// share the work and the `reserve` (CPU hosts) serve only when none of them
/// can. The one embedder client of every Huginn process that embeds.
pub fn pool(primaries: &[String], reserve: &[String], model: &str) -> super::pool::EmbedderPool<OllamaEmbedder> {
    let endpoint = |url: &String| (url.clone(), OllamaEmbedder::new(url, model));
    super::pool::EmbedderPool::new(primaries.iter().map(endpoint).collect(), reserve.iter().map(endpoint).collect())
}
