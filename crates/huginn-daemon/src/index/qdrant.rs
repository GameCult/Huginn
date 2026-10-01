//! The vector-store port over Qdrant's HTTP API. With `ollama.rs`, the xenos
//! boundary: JSON exists here and nowhere else in the organ.

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use super::{CollectionMeta, INDEX_CALL_TIMEOUT, Described, Hit, Http, Method, Point, VectorIndex};

/// One Qdrant. Every path names a collection this organ was told to own; it
/// never lists or touches another.
pub struct QdrantIndex {
    http: Http,
}

/// The ids fetched per scroll page.
const SCROLL_PAGE: u64 = 1000;

impl QdrantIndex {
    pub fn new(base_url: &str) -> Self {
        Self { http: Http::new(base_url, INDEX_CALL_TIMEOUT) }
    }

    fn ask(&self, method: Method, path: &str, body: Option<&Value>, ok: &[u16]) -> Result<(u16, Value)> {
        self.ask_with(&self.http, method, path, body, ok)
    }

    fn ask_with(&self, http: &Http, method: Method, path: &str, body: Option<&Value>, ok: &[u16]) -> Result<(u16, Value)> {
        let text = body.map(Value::to_string);
        let (status, answer) = http.request(method, path, text.as_deref())?;
        ensure!(ok.contains(&status), "Qdrant answered {status} to {path}: {}", answer.chars().take(300).collect::<String>());
        let parsed = if answer.is_empty() { Value::Null } else { serde_json::from_str(&answer).unwrap_or(Value::Null) };
        Ok((status, parsed))
    }
}

fn metadata_json(meta: &CollectionMeta) -> Value {
    json!({
        "managed_by": meta.managed_by,
        "instance": meta.instance,
        "mind": meta.mind,
        "model": meta.model,
        "model_digest": meta.model_digest,
        "dimensions": meta.dimensions,
        "index_text_version": meta.index_text_version,
    })
}

/// What a collection's `config.metadata` says. A collection whose metadata
/// does not name a writer and an instance is unlabelled; one that does but is
/// missing the rest reads as an empty mind or model, which will not match: an
/// empty mind is refused as another's, an empty model is rebuilt when the
/// writer, the instance and the mind are ours.
fn described(metadata: &Value) -> Described {
    let text = |key: &str| metadata[key].as_str().map(str::to_owned);
    let (Some(managed_by), Some(instance)) = (text("managed_by"), text("instance")) else {
        return Described::Unlabelled;
    };
    let number = |key: &str| metadata[key].as_u64().and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    Described::Labelled(CollectionMeta {
        managed_by,
        instance,
        mind: text("mind").unwrap_or_default(),
        model: text("model").unwrap_or_default(),
        model_digest: text("model_digest").unwrap_or_default(),
        dimensions: number("dimensions"),
        index_text_version: number("index_text_version"),
    })
}

impl VectorIndex for QdrantIndex {
    fn describe(&mut self, collection: &str, within: Duration) -> Result<Described> {
        let http = self.http.bounded(within);
        let (status, answer) = self.ask_with(&http, Method::Get, &format!("/collections/{collection}"), None, &[200, 404])?;
        if status == 404 {
            return Ok(Described::Absent);
        }
        Ok(described(&answer["result"]["config"]["metadata"]))
    }

    fn recreate(&mut self, collection: &str, meta: &CollectionMeta) -> Result<()> {
        let path = format!("/collections/{collection}");
        self.ask(Method::Delete, &format!("{path}?wait=true"), None, &[200, 404])?;
        let body = json!({
            "vectors": { "size": meta.dimensions, "distance": "Cosine", "on_disk": true },
            "metadata": metadata_json(meta),
        });
        self.ask(Method::Put, &format!("{path}?wait=true"), Some(&body), &[200])?;
        Ok(())
    }

    fn ids(&mut self, collection: &str) -> Result<BTreeSet<String>> {
        let mut ids = BTreeSet::new();
        let mut offset = Value::Null;
        loop {
            let mut body = json!({ "limit": SCROLL_PAGE, "with_payload": false, "with_vector": false });
            if !offset.is_null() {
                body["offset"] = offset;
            }
            let (_, answer) =
                self.ask(Method::Post, &format!("/collections/{collection}/points/scroll"), Some(&body), &[200])?;
            let points = answer["result"]["points"].as_array().context("Qdrant's scroll has no points")?;
            for point in points {
                ids.insert(point["id"].as_str().context("a point id is not a string")?.to_owned());
            }
            offset = answer["result"]["next_page_offset"].clone();
            if offset.is_null() {
                return Ok(ids);
            }
        }
    }

    fn upsert(&mut self, collection: &str, points: &[Point]) -> Result<()> {
        let points: Vec<Value> = points
            .iter()
            .map(|point| {
                json!({
                    "id": point.id,
                    "vector": point.vector,
                    "payload": {
                        "doc_id": point.payload.doc_id,
                        "kind": point.payload.kind,
                        "root": point.payload.root,
                        "ordinal": point.payload.ordinal,
                        "text_sha256": point.payload.text_sha256,
                    },
                })
            })
            .collect();
        self.ask(
            Method::Put,
            &format!("/collections/{collection}/points?wait=true"),
            Some(&json!({ "points": points })),
            &[200],
        )?;
        Ok(())
    }

    fn search(&mut self, collection: &str, vector: &[f32], limit: u32, within: Duration) -> Result<Vec<Hit>> {
        let http = self.http.bounded(within);
        let body = json!({ "vector": vector, "limit": limit, "with_payload": ["doc_id", "kind"], "with_vector": false });
        let (_, answer) =
            self.ask_with(&http, Method::Post, &format!("/collections/{collection}/points/search"), Some(&body), &[200])?;
        let found = answer["result"].as_array().context("Qdrant's search has no result")?;
        found
            .iter()
            .map(|point| {
                Ok(Hit {
                    doc_id: point["payload"]["doc_id"].as_str().context("a hit carries no doc_id")?.to_owned(),
                    kind: point["payload"]["kind"].as_str().context("a hit carries no kind")?.to_owned(),
                    score: point["score"].as_f64().context("a hit carries no score")? as f32,
                })
            })
            .collect()
    }
}
