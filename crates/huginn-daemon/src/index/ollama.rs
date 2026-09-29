//! The embedder port over Ollama's HTTP API. This file and `qdrant.rs` are the
//! xenos boundary: JSON exists here and nowhere else in the organ.

use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

use super::{BULK_EMBED_TIMEOUT, Embedder, Http, Method, ModelIdentity};

/// The instruction Qwen3 embedding models take in front of a query (never in
/// front of a document). Cut 11b's semantic read prepends it as
/// `Instruct: <this>\nQuery: <text>`.
pub const QUERY_INSTRUCTION: &str =
    "Given a question about a project's pipeline state, retrieve the documents that answer it";

/// One model on one Ollama. A cold model takes seconds to load, so the
/// identity reads get `BULK_EMBED_TIMEOUT`; the worker owns the schedule
/// around it. Each embed call carries its own bound.
pub struct OllamaEmbedder {
    http: Http,
    base_url: String,
    model: String,
}

impl OllamaEmbedder {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self { http: Http::new(base_url, BULK_EMBED_TIMEOUT), base_url: base_url.to_owned(), model: model.to_owned() }
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
    fn model_identity(&mut self) -> Result<ModelIdentity> {
        let tags = self.ask(&self.http, Method::Get, "/api/tags", None)?;
        let listed = tags["models"].as_array().context("Ollama's tag list has no models")?;
        let digest = listed
            .iter()
            .find(|entry| entry["name"] == self.model.as_str() || entry["model"] == self.model.as_str())
            .and_then(|entry| entry["digest"].as_str())
            .with_context(|| format!("Ollama does not list the model {}", self.model))?
            .to_owned();
        let shown = self.ask(&self.http, Method::Post, "/api/show", Some(&json!({ "model": self.model })))?;
        let info = shown["model_info"].as_object().context("Ollama's model description has no model_info")?;
        let Some(length) = info.iter().find(|(key, _)| key.ends_with(".embedding_length")).and_then(|(_, value)| value.as_u64())
        else {
            bail!("Ollama does not say how long {}'s embeddings are", self.model);
        };
        let dimensions = u32::try_from(length).context("an embedding length beyond u32")?;
        Ok(ModelIdentity { name: self.model.clone(), digest, dimensions })
    }

    fn embed(&mut self, texts: &[String], within: Duration) -> Result<Vec<Vec<f32>>> {
        let bounded = Http::new(&self.base_url, within);
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
