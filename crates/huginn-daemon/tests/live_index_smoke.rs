//! A live check of the two adapters against a real Qdrant and a real Ollama.
//! Ignored by default: it needs both services, and it writes to Qdrant, so it
//! touches one scratch collection it creates (`huginn-smoke-<random>`, and a
//! dotted sibling to learn whether Qdrant accepts a `.` in a name) and deletes
//! both at the end. It never names, lists or reads any other collection.
//!
//! HUGINN_SMOKE_QDRANT=http://127.0.0.1:6333 \
//! HUGINN_SMOKE_OLLAMA=http://10.77.0.4:11434 \
//! HUGINN_SMOKE_MODEL=qwen3-embedding:0.6b \
//! cargo test -p huginn-daemon --test live_index_smoke -- --ignored --nocapture

use std::collections::BTreeSet;

use huginn_daemon::index::ollama::OllamaEmbedder;
use huginn_daemon::index::qdrant::QdrantIndex;
use huginn_daemon::index::{
    CollectionMeta, Described, Embedder, MANAGED_BY, Point, PointPayload, VectorIndex, point_id,
};
use huginn_mind::INDEX_TEXT_VERSION;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}

fn raw(method: &str, url: &str, body: Option<&str>) -> (u16, String) {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut response = match (method, body) {
        ("DELETE", _) => agent.delete(url).call(),
        ("POST", Some(body)) => agent.post(url).header("content-type", "application/json").send(body),
        _ => agent.get(url).call(),
    }
    .expect("the request was sent");
    let status = response.status().as_u16();
    (status, response.body_mut().read_to_string().unwrap())
}

#[test]
#[ignore = "needs a live Qdrant and Ollama, and writes a scratch collection"]
fn the_adapters_embed_write_query_and_clean_up_a_scratch_collection() {
    let (qdrant, ollama, model) = (env("HUGINN_SMOKE_QDRANT"), env("HUGINN_SMOKE_OLLAMA"), env("HUGINN_SMOKE_MODEL"));
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let plain = format!("huginn-smoke-{:x}-{stamp:x}", std::process::id());
    let dotted = format!("{plain}.dotted");

    let mut embedder = OllamaEmbedder::new(&ollama, &model);
    let identity = embedder.model_identity().expect("the model is listed");
    println!("identity: {identity:?}");
    let texts = ["The organ owns the mind.".to_string(), "A recipe for lentil soup.".to_string()];
    let vectors = embedder.embed(&texts).expect("two texts embed");
    assert_eq!(vectors.len(), 2);
    assert!(vectors.iter().all(|vector| vector.len() == identity.dimensions as usize));

    let meta = CollectionMeta {
        managed_by: MANAGED_BY.into(),
        instance: "smoke".into(),
        mind: "mind-commit-smoke".into(),
        model: identity.name.clone(),
        model_digest: identity.digest.clone(),
        dimensions: identity.dimensions,
        index_text_version: INDEX_TEXT_VERSION,
    };
    let mut store = QdrantIndex::new(&qdrant);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for name in [&plain, &dotted] {
            assert_eq!(store.describe(name).unwrap(), Described::Absent);
            store.recreate(name, &meta).unwrap();
            assert_eq!(store.describe(name).unwrap(), Described::Labelled(meta.clone()));
            assert!(store.ids(name).unwrap().is_empty());
            let points: Vec<Point> = texts
                .iter()
                .zip(&vectors)
                .enumerate()
                .map(|(at, (text, vector))| Point {
                    id: point_id(&format!("smoke:doc:{at}")),
                    vector: vector.clone(),
                    payload: PointPayload {
                        doc_id: format!("smoke:doc:{at}"),
                        kind: "question".into(),
                        root: "smoke".into(),
                        ordinal: at as u64 + 1,
                        text_sha256: format!("{}", text.len()),
                    },
                })
                .collect();
            store.upsert(name, &points).unwrap();
            let expected: BTreeSet<String> = points.iter().map(|point| point.id.clone()).collect();
            assert_eq!(store.ids(name).unwrap(), expected);
            // The nearest point to the first text is the first point.
            let query = serde_json_free_query(&vectors[0]);
            let (status, answer) = raw("POST", &format!("{qdrant}/collections/{name}/points/search"), Some(&query));
            assert_eq!(status, 200, "{answer}");
            assert!(answer.contains(&points[0].id), "{answer}");
            println!("{name}: described, created, upserted {} points, queried", points.len());
        }
    }));
    for name in [&plain, &dotted] {
        let (status, _) = raw("DELETE", &format!("{qdrant}/collections/{name}?wait=true"), None);
        println!("deleted {name}: {status}");
    }
    let (status, _) = raw("GET", &format!("{qdrant}/collections/{plain}"), None);
    assert_eq!(status, 404, "the scratch collection is gone");
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

/// The nearest-neighbour body, written out by hand so the test needs no JSON
/// crate of its own.
fn serde_json_free_query(vector: &[f32]) -> String {
    let numbers: Vec<String> = vector.iter().map(|value| value.to_string()).collect();
    format!("{{\"vector\":[{}],\"limit\":1}}", numbers.join(","))
}
