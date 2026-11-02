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

    /// A CPU host. It embeds one text in 16 to 27 s idle and 52 s under load,
    /// so a text is given 30 s and a batch 60 s to load; only the caller's
    /// budget, which `for_batch` never exceeds, ends a slow reserve early.
    pub const RESERVE: Self =
        Self { connect: Duration::from_secs(3), floor: Duration::from_secs(60), per_text: Duration::from_secs(30) };

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
/// can, each tier under the bounds of its kind of host. The one embedder client
/// of every Huginn process that embeds.
pub fn pool(primaries: &[String], reserve: &[String], model: &str) -> super::pool::EmbedderPool<OllamaEmbedder> {
    pool_bounded(primaries, reserve, model, EmbedBounds::DEFAULT, EmbedBounds::RESERVE)
}

fn pool_bounded(
    primaries: &[String],
    reserve: &[String],
    model: &str,
    primary: EmbedBounds,
    reserve_bounds: EmbedBounds,
) -> super::pool::EmbedderPool<OllamaEmbedder> {
    let endpoint = |bounds: EmbedBounds| move |url: &String| (url.clone(), OllamaEmbedder::new(url, model).bounded_by(bounds));
    super::pool::EmbedderPool::new(
        primaries.iter().map(endpoint(primary)).collect(),
        reserve.iter().map(endpoint(reserve_bounds)).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_is_given_its_floor_and_what_each_text_adds_but_never_more_than_the_budget() {
        let bounds = EmbedBounds { connect: Duration::from_secs(1), floor: Duration::from_secs(10), per_text: Duration::from_secs(2) };
        assert_eq!(bounds.for_batch(3, Duration::from_secs(100)), Duration::from_secs(16));
        assert_eq!(bounds.for_batch(3, Duration::from_secs(12)), Duration::from_secs(12));
    }

    /// A CPU host that takes 52 s for one text under load, or 16 to 27 s idle,
    /// must not be given up on at a GPU's 20.5 s, and a full bulk batch of it is
    /// bounded only by the caller's budget; a GPU's bound stays short.
    #[test]
    fn the_reserve_is_given_what_a_cpu_takes_and_a_gpu_is_not() {
        let bulk = Duration::from_secs(300);
        assert!(EmbedBounds::RESERVE.for_batch(1, bulk) >= Duration::from_secs(60));
        assert_eq!(EmbedBounds::RESERVE.for_batch(32, bulk), bulk);
        assert!(EmbedBounds::DEFAULT.for_batch(1, bulk) < Duration::from_secs(25));
    }

    /// An Ollama that answers an embed only after `delay`.
    fn slow_ollama(delay: Duration) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let mut buffer = Vec::new();
                    let (head, length) = loop {
                        let mut chunk = [0_u8; 4096];
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => return,
                            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                        }
                        let text = String::from_utf8_lossy(&buffer).into_owned();
                        if let Some(end) = text.find("\r\n\r\n") {
                            let length = text.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                            break (end + 4, length);
                        }
                    };
                    while buffer.len() < head + length {
                        let mut chunk = [0_u8; 4096];
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => return,
                            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                        }
                    }
                    let request = String::from_utf8_lossy(&buffer[..head]).into_owned();
                    let body = if request.starts_with("GET /api/tags") {
                        r#"{"models":[{"name":"m:1","model":"m:1","digest":"d1"}]}"#
                    } else if request.starts_with("POST /api/show") {
                        r#"{"model_info":{"qwen3.embedding_length":1}}"#
                    } else {
                        std::thread::sleep(delay);
                        r#"{"embeddings":[[1.0]]}"#
                    };
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{body}", body.len());
                });
            }
        });
        url
    }

    /// With every GPU dark, the CPU reserve answers a text slower than a GPU's
    /// bound allows and the pool takes the answer; under the GPU's bound the
    /// same host is given up on. The tier decides the bound.
    #[test]
    fn a_slow_reserve_answers_under_its_own_bound_where_a_gpus_would_have_failed() {
        let dark = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", listener.local_addr().unwrap())
        };
        let cpu = slow_ollama(Duration::from_millis(900));
        let quick = EmbedBounds { connect: Duration::from_secs(1), floor: Duration::from_millis(300), per_text: Duration::ZERO };
        let patient = EmbedBounds { floor: Duration::from_secs(5), ..quick };

        let mut as_reserve = pool_bounded(&[dark.clone()], &[cpu.clone()], "m:1", quick, patient);
        let vectors = as_reserve.embed(&["a".to_owned()], Duration::from_secs(20)).unwrap();
        assert_eq!(vectors, vec![vec![1.0_f32]]);

        let mut as_a_gpu = pool_bounded(&[dark], &[cpu], "m:1", quick, quick);
        assert!(as_a_gpu.embed(&["a".to_owned()], Duration::from_secs(20)).is_err());
    }
}
