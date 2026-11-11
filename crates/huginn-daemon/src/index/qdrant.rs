//! The vector-store port over Qdrant's HTTP API. With `ollama.rs`, the xenos
//! boundary: JSON exists here and nowhere else in the organ.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use huginn_mind::ChatMessage;

use crate::sources::{
    COLLECTION, CORPUS_CHAT, CORPUS_FORGE, ChatFilter, ChatOrder, ChatPayload, ChatPoint, ChatRow, DescribedSources, FRONT_PAGE_FILES, SourceIndex, SourcePayload, SourcePoint, SourceRow,
    SourcesMeta,
};

use super::{CollectionMeta, INDEX_CALL_TIMEOUT, Described, Hit, Http, Method, Point, VectorIndex};

/// One Qdrant. Every path names a collection this organ was told to own; it
/// never lists or touches another.
pub struct QdrantIndex {
    http: Http,
    /// The collection the source port reads and writes: `huginn_sources`,
    /// unless a live test points it at a scratch one.
    sources: String,
}

/// The ids fetched per scroll page.
const SCROLL_PAGE: u64 = 1000;

impl QdrantIndex {
    pub fn new(base_url: &str) -> Self {
        Self { http: Http::new(base_url, INDEX_CALL_TIMEOUT), sources: COLLECTION.to_owned() }
    }

    /// The source port on another collection, for a live test's scratch one.
    pub fn with_sources_collection(mut self, collection: &str) -> Self {
        self.sources = collection.to_owned();
        self
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

    fn search(&mut self, collection: &str, vector: &[f32], among: &[String], limit: u32, within: Duration) -> Result<Vec<Hit>> {
        let http = self.http.bounded(within);
        let body = json!({
            "vector": vector,
            "filter": { "must": [{ "has_id": among }] },
            "params": { "exact": true },
            "limit": limit,
            "with_payload": ["doc_id", "kind"],
            "with_vector": false,
        });
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

/// The payload filter of a source search: forge points only, then the repo and
/// the front-page paths (exact matches on the keyword-indexed `path`) when
/// asked. A chat point carries no repo or path and can never pass it.
fn source_filter(repo: Option<&str>, front: bool) -> Value {
    let mut must = vec![json!({ "key": "corpus", "match": { "value": CORPUS_FORGE } })];
    if let Some(repo) = repo {
        must.push(json!({ "key": "repo", "match": { "value": repo } }));
    }
    if front {
        must.push(json!({ "key": "path", "match": { "any": FRONT_PAGE_FILES } }));
    }
    json!({ "must": must })
}

/// The payload filter of a chat read: chat points only; the rooms any-of by id
/// (`!`) or alias (`#`) as a `should`; the span as ranges that keep a chunk
/// overlapping [after, before].
fn chat_filter(filter: &ChatFilter) -> Value {
    let mut must = vec![json!({ "key": "corpus", "match": { "value": CORPUS_CHAT } })];
    if let Some(after) = filter.after_ms {
        must.push(json!({ "key": "ts_end_ms", "range": { "gte": after } }));
    }
    if let Some(before) = filter.before_ms {
        must.push(json!({ "key": "ts_start_ms", "range": { "lte": before } }));
    }
    if let Some(from) = filter.starts_from_ms {
        must.push(json!({ "key": "ts_start_ms", "range": { "gte": from } }));
    }
    if let Some(to) = filter.starts_to_ms {
        must.push(json!({ "key": "ts_start_ms", "range": { "lte": to } }));
    }
    if let Some(event_id) = &filter.event_id {
        must.push(json!({ "key": "event_ids", "match": { "value": event_id } }));
    }
    let mut out = json!({ "must": must });
    let (ids, aliases): (Vec<&String>, Vec<&String>) = filter.rooms.iter().partition(|room| room.starts_with('!'));
    let mut should = Vec::new();
    if !ids.is_empty() {
        should.push(json!({ "key": "room_id", "match": { "any": ids } }));
    }
    if !aliases.is_empty() {
        should.push(json!({ "key": "room_alias", "match": { "any": aliases } }));
    }
    if !should.is_empty() {
        out["should"] = Value::Array(should);
    }
    out
}

/// A chat chunk's payload. The span, first and last ids and the id list are
/// derived from `messages`, so they cannot disagree with them.
fn chat_payload_json(payload: &ChatPayload) -> Result<Value> {
    let (Some(first), Some(last)) = (payload.messages.first(), payload.messages.last()) else {
        anyhow::bail!("a chat chunk of {} carries no messages", payload.room_id);
    };
    let mut out = json!({
        "corpus": CORPUS_CHAT,
        "room_id": payload.room_id,
        "ts_start_ms": payload.messages.iter().map(|message| message.ts_ms).min(),
        "ts_end_ms": payload.messages.iter().map(|message| message.ts_ms).max(),
        "first_event_id": first.event_id,
        "last_event_id": last.event_id,
        "event_ids": payload.messages.iter().map(|message| &message.event_id).collect::<Vec<_>>(),
        "messages": payload.messages.iter().map(|message| json!({
            "event_id": message.event_id, "sender": message.sender, "ts_ms": message.ts_ms, "body": message.body,
        })).collect::<Vec<_>>(),
        "text": payload.text,
    });
    if let Some(alias) = &payload.room_alias {
        out["room_alias"] = json!(alias);
    }
    Ok(out)
}

fn chat_payload_of(payload: &Value) -> Result<ChatPayload> {
    ensure!(payload["corpus"] == CORPUS_CHAT, "a chat read was handed a point of another corpus");
    let text = |value: &Value, key: &str| value[key].as_str().map(str::to_owned).with_context(|| format!("a chat chunk carries no {key}"));
    let messages = payload["messages"]
        .as_array()
        .context("a chat chunk carries no messages")?
        .iter()
        .map(|message| {
            Ok(ChatMessage {
                event_id: text(message, "event_id")?,
                sender: text(message, "sender")?,
                ts_ms: message["ts_ms"].as_u64().context("a chat message carries no ts_ms")?,
                body: text(message, "body")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(!messages.is_empty(), "a chat chunk carries no messages");
    Ok(ChatPayload { room_id: text(payload, "room_id")?, room_alias: payload["room_alias"].as_str().map(str::to_owned), messages, text: text(payload, "text")? })
}

fn payload_json(payload: &SourcePayload) -> Value {
    json!({
        "corpus": CORPUS_FORGE,
        "repo": payload.repo,
        "path": payload.path,
        "commit": payload.commit,
        "blob": payload.blob,
        "line_start": payload.line_start,
        "line_end": payload.line_end,
        "chunk_index": payload.chunk_index,
        "ext": payload.ext,
        "text": payload.text,
    })
}

fn payload_of(payload: &Value) -> Result<SourcePayload> {
    let text = |key: &str| payload[key].as_str().map(str::to_owned).with_context(|| format!("a chunk carries no {key}"));
    let number = |key: &str| {
        payload[key].as_u64().and_then(|value| u32::try_from(value).ok()).with_context(|| format!("a chunk carries no {key}"))
    };
    Ok(SourcePayload {
        repo: text("repo")?,
        path: text("path")?,
        commit: text("commit")?,
        blob: text("blob")?,
        line_start: number("line_start")?,
        line_end: number("line_end")?,
        chunk_index: number("chunk_index")?,
        ext: text("ext")?,
        text: text("text")?,
    })
}

/// The source collection's label and cursors, as they sit in its metadata.
fn sources_described(metadata: &Value) -> DescribedSources {
    let text = |key: &str| metadata[key].as_str().map(str::to_owned);
    let Some(managed_by) = text("managed_by") else { return DescribedSources::Unlabelled };
    let number = |key: &str| metadata[key].as_u64().and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    let cursors: BTreeMap<String, String> = metadata["cursors"]
        .as_object()
        .map(|held| held.iter().filter_map(|(repo, commit)| Some((repo.clone(), commit.as_str()?.to_owned()))).collect())
        .unwrap_or_default();
    DescribedSources::Labelled {
        meta: SourcesMeta {
            managed_by,
            corpus: text("corpus").unwrap_or_default(),
            model: text("model").unwrap_or_default(),
            model_digest: text("model_digest").unwrap_or_default(),
            dimensions: number("dimensions"),
            chunker_version: number("chunker_version"),
        },
        cursors,
    }
}

impl SourceIndex for QdrantIndex {
    fn describe_sources(&mut self, within: Duration) -> Result<DescribedSources> {
        let collection = self.sources.clone();
        let http = self.http.bounded(within);
        let (status, answer) = self.ask_with(&http, Method::Get, &format!("/collections/{collection}"), None, &[200, 404])?;
        if status == 404 {
            return Ok(DescribedSources::Absent);
        }
        Ok(sources_described(&answer["result"]["config"]["metadata"]))
    }

    fn recreate_sources(&mut self, meta: &SourcesMeta) -> Result<()> {
        let collection = self.sources.clone();
        let path = format!("/collections/{collection}");
        self.ask(Method::Delete, &format!("{path}?wait=true"), None, &[200, 404])?;
        let body = json!({
            "vectors": { "size": meta.dimensions, "distance": "Cosine", "on_disk": true },
            "metadata": {
                "managed_by": meta.managed_by,
                "corpus": meta.corpus,
                "model": meta.model,
                "model_digest": meta.model_digest,
                "dimensions": meta.dimensions,
                "chunker_version": meta.chunker_version,
                "cursors": {},
            },
        });
        self.ask(Method::Put, &format!("{path}?wait=true"), Some(&body), &[200])?;
        for (field, schema) in [
            ("repo", "keyword"),
            ("path", "keyword"),
            ("corpus", "keyword"),
            ("room_id", "keyword"),
            ("room_alias", "keyword"),
            ("event_ids", "keyword"),
            ("ts_start_ms", "integer"),
            ("ts_end_ms", "integer"),
        ] {
            let index = json!({ "field_name": field, "field_schema": schema });
            self.ask(Method::Put, &format!("{path}/index?wait=true"), Some(&index), &[200])?;
        }
        Ok(())
    }

    fn set_cursors(&mut self, cursors: &BTreeMap<String, String>) -> Result<()> {
        let collection = self.sources.clone();
        // Qdrant merges metadata by top-level key and replaces a nested object
        // whole, so this is the whole cursors object and nothing beside it.
        let body = json!({ "metadata": { "cursors": cursors } });
        self.ask(Method::Patch, &format!("/collections/{collection}?wait=true"), Some(&body), &[200])?;
        Ok(())
    }

    fn delete_by(&mut self, repo: &str, paths: Option<&[String]>, keeping: Option<&str>) -> Result<()> {
        let collection = self.sources.clone();
        let mut must = vec![json!({ "key": "repo", "match": { "value": repo } })];
        if let Some(paths) = paths {
            must.push(json!({ "key": "path", "match": { "any": paths } }));
        }
        let mut filter = json!({ "must": must });
        if let Some(commit) = keeping {
            filter["must_not"] = json!([{ "key": "commit", "match": { "value": commit } }]);
        }
        let body = json!({ "filter": filter });
        self.ask(Method::Post, &format!("/collections/{collection}/points/delete?wait=true"), Some(&body), &[200])?;
        Ok(())
    }

    fn upsert_sources(&mut self, points: &[SourcePoint]) -> Result<()> {
        let collection = self.sources.clone();
        let points: Vec<Value> = points
            .iter()
            .map(|point| json!({ "id": point.id, "vector": point.vector, "payload": payload_json(&point.payload) }))
            .collect();
        self.ask(Method::Put, &format!("/collections/{collection}/points?wait=true"), Some(&json!({ "points": points })), &[200])?;
        Ok(())
    }

    fn search_sources(&mut self, vector: &[f32], repo: Option<&str>, front: bool, limit: u32, within: Duration) -> Result<Vec<SourceRow>> {
        let collection = self.sources.clone();
        let http = self.http.bounded(within);
        let mut body = json!({ "vector": vector, "limit": limit, "with_payload": true, "with_vector": false });
        body["filter"] = source_filter(repo, front);
        let (_, answer) =
            self.ask_with(&http, Method::Post, &format!("/collections/{collection}/points/search"), Some(&body), &[200])?;
        let found = answer["result"].as_array().context("Qdrant's search has no result")?;
        found
            .iter()
            .map(|point| {
                Ok(SourceRow {
                    score: point["score"].as_f64().context("a hit carries no score")? as f32,
                    payload: payload_of(&point["payload"])?,
                })
            })
            .collect()
    }

    fn upsert_chat(&mut self, points: &[ChatPoint]) -> Result<()> {
        let collection = self.sources.clone();
        let points: Vec<Value> = points
            .iter()
            .map(|point| Ok(json!({ "id": point.id, "vector": point.vector, "payload": chat_payload_json(&point.payload)? })))
            .collect::<Result<_>>()?;
        self.ask(Method::Put, &format!("/collections/{collection}/points?wait=true"), Some(&json!({ "points": points })), &[200])?;
        Ok(())
    }

    fn chat_nearest(&mut self, vector: &[f32], filter: &ChatFilter, limit: u32, within: Duration) -> Result<Vec<ChatRow>> {
        let collection = self.sources.clone();
        let http = self.http.bounded(within);
        let body = json!({ "vector": vector, "limit": limit, "filter": chat_filter(filter), "with_payload": true, "with_vector": false });
        let (_, answer) =
            self.ask_with(&http, Method::Post, &format!("/collections/{collection}/points/search"), Some(&body), &[200])?;
        let found = answer["result"].as_array().context("Qdrant's search has no result")?;
        found
            .iter()
            .map(|point| Ok(ChatRow { score: point["score"].as_f64().context("a hit carries no score")? as f32, payload: chat_payload_of(&point["payload"])? }))
            .collect()
    }

    fn chat_scroll(&mut self, filter: &ChatFilter, order: ChatOrder, limit: u32, within: Duration) -> Result<Vec<ChatPayload>> {
        let collection = self.sources.clone();
        let http = self.http.bounded(within);
        let body = json!({
            "filter": chat_filter(filter),
            "limit": limit,
            "order_by": { "key": "ts_start_ms", "direction": match order { ChatOrder::OldestFirst => "asc", ChatOrder::NewestFirst => "desc" } },
            "with_payload": true,
            "with_vector": false,
        });
        let (_, answer) =
            self.ask_with(&http, Method::Post, &format!("/collections/{collection}/points/scroll"), Some(&body), &[200])?;
        let found = answer["result"]["points"].as_array().context("Qdrant's scroll has no points")?;
        found.iter().map(|point| chat_payload_of(&point["payload"])).collect()
    }
}

#[cfg(test)]
mod source_filter_tests {
    use super::*;

    /// No ask, no filter; a repo alone is the repo's match.
    #[test]
    fn no_ask_is_the_forge_corpus_and_a_repo_is_its_own_match() {
        let forge = json!({ "key": "corpus", "match": { "value": "forge" } });
        assert_eq!(source_filter(None, false), json!({ "must": [forge] }));
        assert_eq!(source_filter(Some("Huginn"), false), json!({ "must": [forge, { "key": "repo", "match": { "value": "Huginn" } }] }));
    }

    /// The front filter matches the path against exactly the front-page files
    /// and nothing else, and composes with the repo: both conditions must hold.
    #[test]
    fn the_front_filter_is_exactly_the_front_files_and_composes_with_the_repo() {
        let any = |filter: &Value| -> Vec<String> {
            let conditions = filter["must"].as_array().unwrap();
            let path: Vec<&Value> = conditions.iter().filter(|c| c["key"] == "path").collect();
            assert_eq!(path.len(), 1);
            path[0]["match"]["any"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_owned()).collect()
        };
        let front = source_filter(None, true);
        assert_eq!(any(&front), ["README.md", "README", "readme.md", "README.markdown", "CLAUDE.md", "AGENTS.md"]);
        assert_eq!(front["must"].as_array().unwrap().len(), 2, "the corpus and the path; no repo condition when none was asked");

        let both = source_filter(Some("Zyphos"), true);
        assert_eq!(any(&both), any(&front));
        let repos: Vec<&Value> = both["must"].as_array().unwrap().iter().filter(|c| c["key"] == "repo").collect();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0]["match"]["value"], "Zyphos");
        assert_eq!(both["must"].as_array().unwrap().len(), 3);
    }

    fn payload() -> ChatPayload {
        let message = |id: &str, ts: u64| ChatMessage { event_id: id.into(), sender: "@a:gamecult.org".into(), ts_ms: ts, body: format!("body {id}") };
        ChatPayload { room_id: "!r:gamecult.org".into(), room_alias: Some("#swarm:gamecult.org".into()), messages: vec![message("$a", 10), message("$b", 20)], text: "swarm\n10 @a: body".into() }
    }

    /// The chat codec is its own inverse, writes the contract's fields derived
    /// from the messages, and refuses a chunk with none and a point of another corpus.
    #[test]
    fn the_chat_payload_round_trips_the_contract() {
        let written = chat_payload_json(&payload()).unwrap();
        assert_eq!(written["corpus"], "chat");
        assert_eq!((written["ts_start_ms"].as_u64(), written["ts_end_ms"].as_u64()), (Some(10), Some(20)));
        assert_eq!((written["first_event_id"].as_str(), written["last_event_id"].as_str()), (Some("$a"), Some("$b")));
        assert_eq!(written["event_ids"], json!(["$a", "$b"]));
        assert_eq!(written["room_alias"], "#swarm:gamecult.org");
        assert_eq!(chat_payload_of(&written).unwrap(), payload());
        let mut no_alias = payload();
        no_alias.room_alias = None;
        assert!(chat_payload_json(&no_alias).unwrap().get("room_alias").is_none());
        assert_eq!(chat_payload_of(&chat_payload_json(&no_alias).unwrap()).unwrap(), no_alias);
        assert!(chat_payload_json(&ChatPayload { messages: vec![], ..payload() }).is_err());
        let forge = payload_json(&SourcePayload { repo: "r".into(), path: "p".into(), commit: "c".into(), blob: "b".into(), line_start: 1, line_end: 2, chunk_index: 0, ext: "rs".into(), text: "t".into() });
        assert_eq!(forge["corpus"], "forge");
        assert!(chat_payload_of(&forge).is_err());
    }

    /// The chat filter is the chat corpus, the rooms as an any-of `should` split
    /// by id and alias, and the two ranges that keep an overlapping span.
    #[test]
    fn the_chat_filter_is_corpus_rooms_and_overlap() {
        let filter = chat_filter(&ChatFilter { rooms: vec!["!r:x".into(), "#a:x".into(), "#b:x".into()], after_ms: Some(5), before_ms: Some(9), ..ChatFilter::default() });
        assert_eq!(
            filter,
            json!({
                "must": [
                    { "key": "corpus", "match": { "value": "chat" } },
                    { "key": "ts_end_ms", "range": { "gte": 5 } },
                    { "key": "ts_start_ms", "range": { "lte": 9 } },
                ],
                "should": [
                    { "key": "room_id", "match": { "any": ["!r:x"] } },
                    { "key": "room_alias", "match": { "any": ["#a:x", "#b:x"] } },
                ],
            })
        );
        assert_eq!(chat_filter(&ChatFilter::default()), json!({ "must": [{ "key": "corpus", "match": { "value": "chat" } }] }));
    }

    /// An event id is a match on the keyword array, and the start bounds are
    /// ranges on the chunk's own start.
    #[test]
    fn the_chat_filter_matches_an_event_and_bounds_chunk_starts() {
        let filter = chat_filter(&ChatFilter { event_id: Some("$e".into()), starts_from_ms: Some(3), starts_to_ms: Some(8), ..ChatFilter::default() });
        assert_eq!(
            filter["must"],
            json!([
                { "key": "corpus", "match": { "value": "chat" } },
                { "key": "ts_start_ms", "range": { "gte": 3 } },
                { "key": "ts_start_ms", "range": { "lte": 8 } },
                { "key": "event_ids", "match": { "value": "$e" } },
            ])
        );
    }
}
