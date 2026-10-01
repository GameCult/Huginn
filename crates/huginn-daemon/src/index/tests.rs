//! The projection's rules, against in-memory ports; the adapters' request and
//! answer shapes, against a stub that speaks just enough HTTP. No test needs a
//! live Qdrant or Ollama.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse, IndexStatus};
use huginn_mind::{Mind, OwnedRedbMessagePackBackingStore, PipelineAdmissionOutcome};
use tempfile::TempDir;

use super::fakes::{FakeEmbedder, FakeIndex, Gate, Stored, pair};
use super::ollama::OllamaEmbedder;
use super::qdrant::QdrantIndex;
use super::*;
use crate::daemon::Daemon;
use crate::daemon::tests::{INSTANCE, OTHER, batch, campaign_seed, now, question_and_ruling, slug};

thread_local! {
    /// Runs once, in the worker's own turn, between the inbox being absorbed
    /// and a search being taken, holding the guard `serve_search` holds.
    static AFTER_ABSORB: std::cell::RefCell<Option<Box<dyn FnOnce(&mut Shared)>>> = const { std::cell::RefCell::new(None) };
}

/// The hook `serve_search` calls at the point where a second lock once left a
/// gap. It is the test's only way in: nothing else can reach that point.
pub(super) fn after_absorb(guard: &mut Shared) {
    if let Some(hook) = AFTER_ABSORB.with(|hook| hook.borrow_mut().take()) {
        hook(guard);
    }
}

fn collection() -> String {
    collection_name(&slug(INSTANCE))
}

/// The bound the adapter tests give a call.
const TIME: Duration = Duration::from_secs(30);

/// A mind identity for tests that project entries without a mind behind them.
const MIND: &str = "mind-commit-first";

/// A mind of the same instance name whose first batch differs, so its genesis
/// receipt id does too.
fn other_mind_of_the_same_name() -> (TempDir, Mind<OwnedRedbMessagePackBackingStore>) {
    let root = tempfile::tempdir().unwrap();
    let mut mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
    let (question, _) = question_and_ruling();
    let mut first = campaign_seed();
    first.push(question);
    let outcome = mind.admit(batch(INSTANCE, first), now());
    assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    (root, mind)
}

/// A real mind holding a campaign, a question, the ruling that answers it and
/// the resolution admission derived: four indexable documents, beside the
/// identity and the stewardship, which carry no text.
fn mind_with_documents() -> (TempDir, Mind<OwnedRedbMessagePackBackingStore>) {
    let root = tempfile::tempdir().unwrap();
    let mut mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
    let (question, ruling) = question_and_ruling();
    for documents in [campaign_seed(), vec![question, ruling]] {
        let outcome = mind.admit(batch(INSTANCE, documents), now());
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }
    (root, mind)
}

/// How many times the collection was created and how many upserts it took.
fn counts(index: &FakeIndex) -> (usize, usize) {
    let state = index.state.lock().unwrap();
    (state.recreated.len(), state.upserts.len())
}

/// Steps until the projector has nothing to do. A step that never settles is a
/// failure of the test, not a hang: it is bounded.
fn drain<E: Embedder, V: VectorIndex>(projector: &mut Projector<E, V>) {
    for _ in 0..1000 {
        if projector.advance().unwrap() == Advance::Idle {
            return;
        }
    }
    panic!("the projector was still stepping after 1000 steps");
}

fn point_ids(entries: &[IndexEntry]) -> BTreeSet<String> {
    entries.iter().map(|entry| point_id(&entry.id.id.0)).collect()
}

fn projected(embedder: &FakeEmbedder, index: &FakeIndex, entries: &[IndexEntry]) -> Projector<FakeEmbedder, FakeIndex> {
    let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    projector.want(entries.iter().cloned());
    drain(&mut projector);
    projector
}

/// A projector whose collection was lost while it ran and has been made again:
/// it owes every entry, and is rebuilding.
fn rebuilding(embedder: &FakeEmbedder, index: &FakeIndex, entries: &[IndexEntry]) -> Projector<FakeEmbedder, FakeIndex> {
    let mut projector = projected(embedder, index, &entries[..1]);
    index.state.lock().unwrap().collections.remove(&collection());
    projector.want(entries[1..].iter().cloned());
    projector.advance().expect_err("the write meets no collection");
    assert_eq!(projector.advance().unwrap(), Advance::Progressed, "the reconciliation makes the collection again");
    projector
}

/// P4's function, as voidbot's `toQdrantPointId` computes it: the first sixteen
/// bytes of the SHA-256, version and variant bits set. The fixed vectors were
/// computed independently of this crate.
#[test]
fn point_ids_are_a_pure_function_of_the_document_id() {
    assert_eq!(point_id("eureka-state:question:Q1"), "1e8a661b-4b7e-5245-a9ad-b15a3fa27031");
    assert_eq!(point_id("eureka-state:campaign:self"), "d4135998-051d-57fb-9360-82964874fbc9");
    assert_eq!(point_id("eureka-state:question:Q1"), point_id("eureka-state:question:Q1"));
    assert_ne!(point_id("eureka-state:question:Q1"), point_id("eureka-state:question:Q2"));
}

/// The projection converges by set difference: after a crash that lost two
/// points, a restart embeds and writes those two, and nothing else, and the
/// collection is not rebuilt.
#[test]
fn reconcile_indexes_exactly_the_indexable_documents_the_collection_lacks() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    assert_eq!(entries.len(), 4);

    let (embedder, index) = pair();
    projected(&embedder, &index, &entries);
    assert_eq!(
        index.holding(&collection()),
        point_ids(&entries),
        "one point per indexable document, none for the identity or the stewardship"
    );

    let lost: Vec<IndexEntry> = entries.iter().take(2).cloned().collect();
    {
        let mut state = index.state.lock().unwrap();
        let stored = state.collections.get_mut(&collection()).unwrap();
        for entry in &lost {
            stored.points.remove(&point_id(&entry.id.id.0));
        }
        state.upserts.clear();
        state.recreated.clear();
    }

    let restarted = FakeEmbedder::new("d1");
    projected(&restarted, &index, &entries);
    let state = index.state.lock().unwrap();
    assert!(state.recreated.is_empty(), "a collection that agrees is not rebuilt");
    let written: BTreeSet<String> = state.upserts.concat().into_iter().collect();
    assert_eq!(written, point_ids(&lost), "exactly the missing points were written");
    assert_eq!(state.upserts.concat().len(), 2, "each once");
    assert_eq!(restarted.texts().len(), 2, "and only their text was embedded");
    drop(state);
    assert_eq!(index.holding(&collection()), point_ids(&entries));
}

/// Compatibility is the model by name and digest, its dimensions and the text
/// version. Any of them differing rebuilds the collection whole, and no vector
/// of one model survives beside another's.
#[test]
fn a_model_digest_change_rebuilds_the_collection() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let changes: [(&str, Box<dyn Fn(&mut ModelIdentity)>); 3] = [
        ("digest", Box::new(|identity| identity.digest = "d2".into())),
        ("name", Box::new(|identity| identity.name = "other-model".into())),
        ("dimensions", Box::new(|identity| identity.dimensions = 8)),
    ];
    for (what, change) in changes {
        let (embedder, index) = pair();
        projected(&embedder, &index, &entries);
        assert_eq!(index.state.lock().unwrap().recreated.len(), 1, "the first run creates the collection");

        let second = FakeEmbedder::new("d1");
        change(&mut second.state.lock().unwrap().identity);
        let expected = second.state.lock().unwrap().identity.clone();
        projected(&second, &index, &entries);
        let state = index.state.lock().unwrap();
        assert_eq!(state.recreated.len(), 2, "a changed {what} rebuilds");
        let (_, meta) = state.recreated.last().unwrap();
        assert_eq!(
            (&meta.model, &meta.model_digest, meta.dimensions),
            (&expected.name, &expected.digest, expected.dimensions),
            "{what}"
        );
        assert_eq!(second.texts().len(), 4, "every document is embedded again under {what}");
        assert_eq!(state.collections[&collection()].points.len(), 4);
    }

    // The same model, digest and dimensions: nothing is rebuilt or rewritten.
    let (embedder, index) = pair();
    projected(&embedder, &index, &entries);
    let same = FakeEmbedder::new("d1");
    projected(&same, &index, &entries);
    assert_eq!(index.state.lock().unwrap().recreated.len(), 1);
    assert!(same.texts().is_empty());

    // A text version the collection was not built with is a rebuild too.
    {
        let mut state = index.state.lock().unwrap();
        let Described::Labelled(meta) = &mut state.collections.get_mut(&collection()).unwrap().label else { panic!() };
        meta.index_text_version += 1;
    }
    projected(&FakeEmbedder::new("d1"), &index, &entries);
    assert_eq!(index.state.lock().unwrap().recreated.len(), 2);
}

/// No cross-mind writes: a collection that another writer or another instance
/// made, or that carries no label, is left exactly as it was.
#[test]
fn a_collection_owned_by_another_instance_is_never_recreated() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let ours = |managed_by: &str, instance: &str, mind: &str| CollectionMeta {
        managed_by: managed_by.into(),
        instance: instance.into(),
        mind: mind.into(),
        model: "model".into(),
        model_digest: "d1".into(),
        dimensions: 4,
        index_text_version: INDEX_TEXT_VERSION,
    };
    for foreign in [
        Described::Unlabelled,
        Described::Labelled(ours(MANAGED_BY, OTHER, MIND)),
        Described::Labelled(ours("voidbot", INSTANCE, MIND)),
        Described::Labelled(ours(MANAGED_BY, INSTANCE, "mind-commit-another")),
        Described::Labelled(ours(MANAGED_BY, INSTANCE, "")),
    ] {
        let (embedder, index) = pair();
        index.plant(&collection(), foreign.clone());
        let sentinel = point_id("someone-elses-document");
        index.state.lock().unwrap().collections.get_mut(&collection()).unwrap().points.insert(
            sentinel.clone(),
            Point {
                id: sentinel.clone(),
                vector: vec![0.0; 4],
                payload: PointPayload {
                    doc_id: "someone-elses-document".into(),
                    kind: "x".into(),
                    root: "x".into(),
                    ordinal: 1,
                    text_sha256: "x".into(),
                },
            },
        );
        let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
        projector.want(entries.iter().cloned());
        let error = projector.reconcile().expect_err("a foreign collection is refused");
        assert!(matches!(error, StepError::Refused(_)), "{foreign:?}: {error}");
        assert!(matches!(projector.failure_status(&error, 1), IndexStatus::Refused { pending: 4, .. }));
        let state = index.state.lock().unwrap();
        assert!(state.recreated.is_empty() && state.upserts.is_empty(), "{foreign:?} was written to");
        assert_eq!(state.collections[&collection()].label, foreign);
        assert!(state.collections[&collection()].points.contains_key(&sentinel), "{foreign:?} lost a point");
        assert!(embedder.texts().is_empty());
    }
}

/// The vector's length is checked against the collection's, so a flush that
/// the model did not change is still refused when the embedder answers wrongly.
#[test]
fn a_vector_of_the_wrong_length_is_refused_and_nothing_is_dropped() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    projector.want(entries.iter().cloned());
    projector.reconcile().unwrap();
    embedder.state.lock().unwrap().vector_length = Some(8);
    let error = projector.flush_batch().expect_err("a longer vector is refused");
    assert!(matches!(error, StepError::Unavailable(_)), "{error}");
    assert_eq!(projector.pending(), 4);
    assert!(index.state.lock().unwrap().upserts.is_empty());
}

/// A flush embeds through the same check as a search: a model that changed
/// before the embed is never asked, a model that changed during it has its
/// vectors dropped, and a collection that is no longer this mind's is refused.
/// In each case nothing is written, the documents stay wanted, and the
/// reconciliation is forgotten.
#[test]
fn a_flush_is_refused_as_a_search_is_when_the_model_or_the_collection_changed() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let reconciled = || {
        let (embedder, index) = pair();
        let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
        projector.want(entries.iter().cloned());
        projector.reconcile().unwrap();
        (embedder, index, projector)
    };

    let (embedder, index, mut projector) = reconciled();
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    projector.flush_batch().expect("a changed model is not a failure");
    assert!(embedder.texts().is_empty(), "before: the texts were not embedded under the new model");
    assert!(index.state.lock().unwrap().upserts.is_empty());
    assert_eq!((projector.pending(), projector.stale()), (4, true), "before");

    let (embedder, index, mut projector) = reconciled();
    change_label(&index, |meta| meta.mind = "mind-commit-another".into());
    let error = projector.flush_batch().expect_err("another mind's label is refused");
    assert!(error.to_string().contains("no longer the one this mind's index was built as"), "{error}");
    assert!(embedder.texts().is_empty(), "label: nothing was embedded");
    assert!(index.state.lock().unwrap().upserts.is_empty());
    assert_eq!((projector.pending(), projector.stale()), (4, true), "label");

    let gate = Gate::shut();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let index = FakeIndex::default();
    let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    projector.want(entries.iter().cloned());
    projector.reconcile().unwrap();
    let flushing = std::thread::spawn(move || {
        let flushed = projector.flush_batch();
        (projector, flushed)
    });
    gate.wait_arrived(1);
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    gate.open();
    let (projector, flushed) = flushing.join().unwrap();
    flushed.expect("a changed model is not a failure");
    assert!(index.state.lock().unwrap().upserts.is_empty(), "during: the vectors were not written");
    assert_eq!((projector.pending(), projector.stale()), (4, true), "during");
}

fn change_label(index: &FakeIndex, change: impl FnOnce(&mut CollectionMeta)) {
    let mut state = index.state.lock().unwrap();
    let Described::Labelled(meta) = &mut state.collections.get_mut(&collection()).unwrap().label else { panic!() };
    change(meta);
}
/// Every call on the search path is bounded by what remains of the search's own
/// time: the identity reads, the label read, the embed and the query. A search
/// whose embed cannot be recalled therefore cannot hold the writes off past the
/// moment nobody is waiting for it. A bulk flush keeps its own, longer, bound.
#[test]
fn every_call_on_the_search_path_is_bounded_by_the_searchs_time_and_the_flush_by_its_own() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    {
        let state = embedder.state.lock().unwrap();
        assert!(!state.bounds.is_empty());
        let near = |bound: &Duration| *bound <= BULK_EMBED_TIMEOUT && *bound > BULK_EMBED_TIMEOUT - Duration::from_secs(60);
        assert!(state.bounds.iter().all(near), "the flushes' embeds: {:?}", state.bounds);
        assert!(state.identity_bounds.iter().all(near), "the reconciliation's and the flushes' identity reads: {:?}", state.identity_bounds);
    }
    embedder.state.lock().unwrap().bounds.clear();
    embedder.state.lock().unwrap().identity_bounds.clear();
    index.state.lock().unwrap().bounds.clear();

    let within = Duration::from_secs(20);
    projector.search("anything", 3, within).unwrap();
    let embedder = embedder.state.lock().unwrap();
    let index = index.state.lock().unwrap();
    assert_eq!(embedder.identity_bounds.len(), 2, "the model is read before and after the embed");
    assert_eq!(embedder.bounds.len(), 1);
    assert_eq!(index.bounds.iter().map(|(call, _)| *call).collect::<Vec<_>>(), ["describe", "search"]);
    let bounds: Vec<Duration> = embedder
        .identity_bounds
        .iter()
        .chain(&embedder.bounds)
        .copied()
        .chain(index.bounds.iter().map(|(_, bound)| *bound))
        .collect();
    assert!(bounds.iter().all(|bound| !bound.is_zero() && *bound <= within), "{bounds:?}");
    assert_eq!(Backoff::default().search_deadline, SEARCH_DEADLINE);
    assert!(SEARCH_DEADLINE < BULK_EMBED_TIMEOUT);
}

/// A hung Ollama does not hold the worker past the search that asked: the
/// identity read never answers, and the search is answered, and the worker
/// free, when the search's own time has passed. A search that waited past its
/// time in the queue is answered without asking the embedder anything. A
/// ticket's time is stamped by the sink (see `the_sink_owns_the_searchs_deadline_and_tells_the_loop`).
#[test]
fn a_hung_embedder_holds_the_worker_no_longer_than_the_search_that_asked() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut sink = WorkerSink::spawn(embedder.clone(), index, &slug(INSTANCE), Some(MIND.into()), entries, quick());
    sink.wait_status(|status| *status == IndexStatus::Current);

    ask_sink(&mut sink, "the ticket's time", 3).unwrap();
    answers(&mut sink, 1);

    embedder.state.lock().unwrap().hang_identity = true;
    let started = Instant::now();
    let due = started + Duration::from_millis(400);
    lock(&sink.shared).searches.push_back(Queued { ticket: SearchTicket(70), text: "hung".into(), top_k: 3, due });
    sink.shared.1.notify_all();
    let answered = answers(&mut sink, 1);
    assert_eq!(answered[0].0, SearchTicket(70));
    assert!(answered[0].1.is_err(), "{answered:?}");
    assert!(started.elapsed() < Duration::from_secs(5), "the worker was held for {:?}", started.elapsed());

    embedder.state.lock().unwrap().hang_identity = false;
    let reads = embedder.state.lock().unwrap().identity_bounds.len();
    lock(&sink.shared).searches.push_back(Queued {
        ticket: SearchTicket(71),
        text: "late".into(),
        top_k: 3,
        due: Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or_else(Instant::now),
    });
    sink.shared.1.notify_all();
    let late = answers(&mut sink, 1);
    let Err(why) = &late[0].1 else { panic!("{late:?}") };
    assert!(why.contains("has run out"), "{why}");
    assert_eq!(embedder.state.lock().unwrap().identity_bounds.len(), reads, "nothing was asked for a search nobody waits for");

    let ticket = ask_sink(&mut sink, "the worker is free", 3).expect("the worker is still current");
    assert_eq!(answers(&mut sink, 1)[0].0, ticket);
}

/// The Ollama adapter enforces the bound it is given: an embed that is never
/// answered fails at the bound, not at the adapter's default.
#[test]
fn an_ollama_embed_that_is_never_answered_fails_at_its_bound() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let held = std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().take(1) {
            held.push(stream.unwrap());
            std::thread::sleep(Duration::from_secs(8));
        }
    });
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut embedder = OllamaEmbedder::new(&url, "m:1");
        sent.send(embedder.embed(&["a".into()], Duration::from_millis(300)).is_err()).ok();
    });
    assert_eq!(received.recv_timeout(Duration::from_secs(5)), Ok(true), "the embed did not fail at its bound");
    drop(held);
}
fn whoami_index(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, WorkerSink>) -> IndexStatus {
    match daemon.handle(HuginnMindRequest::Whoami, now()).answered() {
        HuginnMindResponse::Whoami(status) => status.index,
        other => panic!("expected a status, got {other:?}"),
    }
}

fn committed(response: HuginnMindResponse) {
    assert!(
        matches!(response, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })),
        "{response:?}"
    );
}

fn quick() -> Backoff {
    Backoff { initial: Duration::from_millis(1), max: Duration::from_millis(2), recheck: Duration::from_secs(3600), search_deadline: Duration::from_secs(30) }
}

/// The index never delays or refuses admission: the embedder is shut, the
/// worker is inside it, and two more admissions are answered at once. The gate
/// counts arrivals, so this observes where the work is and not how long it
/// took.
#[test]
fn an_admission_replies_while_the_embedder_is_blocked() {
    let root = tempfile::tempdir().unwrap();
    let mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
    let gate = Gate::shut();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let index = FakeIndex::default();
    let sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), mind.genesis_receipt_id().unwrap(), mind.index_entries(None).unwrap(), quick());
    let mut daemon = Daemon::new(mind, sink);
    daemon.index().wait_status(|status| *status == IndexStatus::Current);

    committed(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, campaign_seed())), now()).answered());
    gate.wait_arrived(1);
    let (question, ruling) = question_and_ruling();
    committed(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![question, ruling])), now()).answered());
    assert_eq!(
        whoami_index(&mut daemon),
        IndexStatus::Behind { pending: 4 },
        "the campaign is in the embedder and three more are queued"
    );
    assert!(index.holding(&collection()).is_empty(), "nothing has been written yet");

    gate.open();
    daemon.index().wait_status(|status| *status == IndexStatus::Current);
    assert_eq!(index.holding(&collection()).len(), 4);
    assert_eq!(whoami_index(&mut daemon), IndexStatus::Current);
}

/// A failing embedder is reported by `whoami`, retried on the backoff, and
/// nothing is dropped: when it returns, every document is written.
#[test]
fn a_failing_embedder_is_visible_in_whoami_and_retried_never_dropped() {
    let root = tempfile::tempdir().unwrap();
    let mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
    let (embedder, index) = pair();
    let sink = WorkerSink::spawn(embedder.clone(), index.clone(), &slug(INSTANCE), None, Vec::new(), quick());
    let mut daemon = Daemon::new(mind, sink);
    daemon.index().wait_status(|status| *status == IndexStatus::Current);

    embedder.state.lock().unwrap().down = true;
    let (question, ruling) = question_and_ruling();
    for documents in [campaign_seed(), vec![question, ruling]] {
        committed(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), now()).answered());
    }
    let seen = daemon.index().wait_status(|status| matches!(status, IndexStatus::Failing { attempts, .. } if *attempts >= 2));
    let IndexStatus::Failing { pending, error, .. } = seen else { panic!() };
    assert_eq!(pending, 4);
    assert!(error.contains("the embedder is down"), "{error}");
    assert!(matches!(whoami_index(&mut daemon), IndexStatus::Failing { .. }));
    assert!(index.holding(&collection()).is_empty());

    embedder.state.lock().unwrap().down = false;
    daemon.index().wait_status(|status| *status == IndexStatus::Current);
    assert_eq!(index.holding(&collection()).len(), 4, "every admitted document was kept and written");
}

/// A collection that is not this mind's is reported as refused, never
/// recreated, and re-read on the backoff: once it is gone the projection
/// completes.
#[test]
fn a_refused_collection_is_reported_and_reread() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    index.plant(&collection(), Described::Unlabelled);
    let sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), mind.genesis_receipt_id().unwrap(), entries, quick());
    let mut daemon = Daemon::new(mind, sink);
    daemon.index().wait_status(|status| matches!(status, IndexStatus::Refused { attempts, .. } if *attempts >= 2));
    assert!(matches!(whoami_index(&mut daemon), IndexStatus::Refused { pending: 4, .. }));
    assert!(index.state.lock().unwrap().recreated.is_empty());

    index.state.lock().unwrap().collections.remove(&collection());
    daemon.index().wait_status(|status| *status == IndexStatus::Current);
    assert_eq!(index.holding(&collection()).len(), 4);
}

// The adapters, against a stub that answers the routes it is given.

struct Stub {
    url: String,
    seen: Arc<Mutex<Vec<(String, String, String)>>>,
}

type Route = (&'static str, u16, &'static str);

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

/// Serves each route once, in order, then repeats the last route that matched.
/// A route matches `"<METHOD> <path>"` by prefix.
fn stub(routes: &[Route]) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let routes: Arc<Mutex<Vec<(Route, bool)>>> = Arc::new(Mutex::new(routes.iter().map(|route| (*route, false)).collect()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let (routes, log) = (Arc::clone(&routes), Arc::clone(&log));
            std::thread::spawn(move || answer_connection(stream, &routes, &log));
        }
    });
    Stub { url, seen }
}

fn answer_connection(
    mut stream: TcpStream,
    routes: &Mutex<Vec<(Route, bool)>>,
    log: &Mutex<Vec<(String, String, String)>>,
) {
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let header_end = loop {
            if let Some(at) = find(&buffer, b"\r\n\r\n") {
                break at + 4;
            }
            let mut chunk = [0_u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        };
        let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let length = head
            .lines()
            .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|value| value.trim().parse::<usize>().unwrap()))
            .unwrap_or(0);
        while buffer.len() < header_end + length {
            let mut chunk = [0_u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        }
        let body = String::from_utf8_lossy(&buffer[header_end..header_end + length]).into_owned();
        buffer.drain(..header_end + length);
        let mut first = head.lines().next().unwrap().split(' ');
        let (method, path) = (first.next().unwrap().to_owned(), first.next().unwrap().to_owned());
        log.lock().unwrap().push((method.clone(), path.clone(), body));
        let key = format!("{method} {path}");
        let (status, answer) = {
            let mut routes = routes.lock().unwrap();
            let matching: Vec<usize> =
                routes.iter().enumerate().filter(|(_, ((prefix, ..), _))| key.starts_with(prefix)).map(|(at, _)| at).collect();
            let chosen = matching.iter().copied().find(|at| !routes[*at].1).or(matching.last().copied());
            match chosen {
                Some(at) => {
                    routes[at].1 = true;
                    (routes[at].0.1, routes[at].0.2)
                }
                None => (404, "{}"),
            }
        };
        let response =
            format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{answer}", answer.len());
        if stream.write_all(response.as_bytes()).is_err() {
            return;
        }
    }
}

const TAGS: &str = r#"{"models":[{"name":"other:1","model":"other:1","digest":"zzz"},{"name":"m:1","model":"m:1","digest":"ac6d"}]}"#;
const SHOW: &str = r#"{"model_info":{"general.architecture":"qwen3","qwen3.embedding_length":4}}"#;
const EMBED: &str = r#"{"embeddings":[[0.5,1.0,2.0,3.0],[0.5,1.0,2.0,3.0],[0.5,1.0,2.0,3.0],[0.5,1.0,2.0,3.0]]}"#;

#[test]
fn ollama_identity_is_the_listed_digest_and_the_models_embedding_length() {
    let ollama = stub(&[("GET /api/tags", 200, TAGS), ("POST /api/show", 200, SHOW), ("POST /api/embed", 200, EMBED)]);
    let mut embedder = OllamaEmbedder::new(&ollama.url, "m:1");
    assert_eq!(embedder.model_identity(TIME).unwrap(), ModelIdentity { name: "m:1".into(), digest: "ac6d".into(), dimensions: 4 });
    let vectors = embedder.embed(&["a".into(), "b".into(), "c".into(), "d".into()], Duration::from_secs(5)).unwrap();
    assert_eq!(vectors.len(), 4);
    assert_eq!(vectors[0], vec![0.5, 1.0, 2.0, 3.0]);
    let seen = ollama.seen.lock().unwrap();
    let (_, _, body) = seen.iter().find(|(_, path, _)| path == "/api/embed").unwrap();
    assert!(body.contains("\"model\":\"m:1\"") && body.contains("\"input\":[\"a\",\"b\",\"c\",\"d\"]"), "{body}");
    drop(seen);

    // A listing entry that carries only `name` is still the model.
    let bare = stub(&[
        ("GET /api/tags", 200, r#"{"models":[{"name":"m:1","digest":"ac6d"}]}"#),
        ("POST /api/show", 200, SHOW),
    ]);
    assert_eq!(OllamaEmbedder::new(&bare.url, "m:1").model_identity(TIME).unwrap().digest, "ac6d");

    let mut absent = OllamaEmbedder::new(&ollama.url, "not-pulled:1");
    assert!(format!("{:#}", absent.model_identity(TIME).unwrap_err()).contains("does not list the model"));
    let refusing = stub(&[("POST /api/embed", 500, r#"{"error":"boom"}"#)]);
    assert!(OllamaEmbedder::new(&refusing.url, "m:1").embed(&["a".into()], Duration::from_secs(5)).is_err());
}

/// Compatibility never reads the address: the same model at another address
/// leaves the collection alone, and the same address serving another digest
/// rebuilds it.
#[test]
fn the_embedders_address_is_not_part_of_the_collections_compatibility() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let routes = [("GET /api/tags", 200, TAGS), ("POST /api/show", 200, SHOW), ("POST /api/embed", 200, EMBED)];
    let repulled = TAGS.replace("ac6d", "beef");
    let index = FakeIndex::default();

    let mut first = Projector::new(OllamaEmbedder::new(&stub(&routes).url, "m:1"), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    first.want(entries.iter().cloned());
    drain(&mut first);
    assert_eq!(counts(&index), (1, 1));

    // Another address, the same model and digest.
    let mut moved = Projector::new(OllamaEmbedder::new(&stub(&routes).url, "m:1"), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    moved.want(entries.iter().cloned());
    drain(&mut moved);
    assert_eq!(counts(&index), (1, 1));

    // The same model name re-pulled: another digest.
    let leaked: &'static str = Box::leak(repulled.into_boxed_str());
    let other = [("GET /api/tags", 200, leaked), routes[1], routes[2]];
    let mut rebuilt = Projector::new(OllamaEmbedder::new(&stub(&other).url, "m:1"), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    rebuilt.want(entries.iter().cloned());
    drain(&mut rebuilt);
    assert_eq!(counts(&index), (2, 2));
}

#[test]
fn qdrant_describes_creates_lists_and_writes_what_the_projector_asks() {
    let name = collection();
    let absent = stub(&[("GET /collections/", 404, r#"{"status":{"error":"not found"}}"#)]);
    assert_eq!(QdrantIndex::new(&absent.url).describe(&name, TIME).unwrap(), Described::Absent);

    let unlabelled = stub(&[("GET /collections/", 200, r#"{"result":{"config":{"metadata":null}}}"#)]);
    assert_eq!(QdrantIndex::new(&unlabelled.url).describe(&name, TIME).unwrap(), Described::Unlabelled);
    let voidbot = stub(&[("GET /collections/", 200, r#"{"result":{"config":{"metadata":{"managedBy":"voidbot"}}}}"#)]);
    assert_eq!(QdrantIndex::new(&voidbot.url).describe(&name, TIME).unwrap(), Described::Unlabelled);

    let labelled = stub(&[(
        "GET /collections/",
        200,
        r#"{"result":{"config":{"metadata":{"managed_by":"huginn","instance":"yggdrasil","mind":"mind-commit-first","model":"m:1","model_digest":"ac6d","dimensions":4,"index_text_version":1}}}}"#,
    )]);
    assert_eq!(
        QdrantIndex::new(&labelled.url).describe(&name, TIME).unwrap(),
        Described::Labelled(CollectionMeta {
            managed_by: "huginn".into(),
            instance: "yggdrasil".into(),
            mind: "mind-commit-first".into(),
            model: "m:1".into(),
            model_digest: "ac6d".into(),
            dimensions: 4,
            index_text_version: 1,
        })
    );

    // A scroll is followed to its last page.
    let paged = stub(&[
        ("POST /collections/", 200, r#"{"result":{"points":[{"id":"a"},{"id":"b"}],"next_page_offset":"b"}}"#),
        ("POST /collections/", 200, r#"{"result":{"points":[{"id":"c"}],"next_page_offset":null}}"#),
    ]);
    let ids = QdrantIndex::new(&paged.url).ids(&name).unwrap();
    let expected: BTreeSet<String> = ["a", "b", "c"].map(String::from).into_iter().collect();
    assert_eq!(ids, expected);
    let seen = paged.seen.lock().unwrap();
    assert!(!seen[0].2.contains("offset") && seen[1].2.contains("\"offset\":\"b\""), "{seen:?}");
    drop(seen);

    // A rebuild drops, then creates with the metadata and the vector shape.
    let writing = stub(&[("DELETE /collections/", 200, "{}"), ("PUT /collections/", 200, "{}")]);
    let meta = CollectionMeta {
        managed_by: MANAGED_BY.into(),
        instance: INSTANCE.into(),
        mind: MIND.into(),
        model: "m:1".into(),
        model_digest: "ac6d".into(),
        dimensions: 4,
        index_text_version: INDEX_TEXT_VERSION,
    };
    let mut store = QdrantIndex::new(&writing.url);
    store.recreate(&name, &meta).unwrap();
    let point = Point {
        id: point_id("d"),
        vector: vec![1.0, 2.0, 3.0, 4.0],
        payload: PointPayload { doc_id: "d".into(), kind: "question".into(), root: "eureka-state".into(), ordinal: 7, text_sha256: "h".into() },
    };
    store.upsert(&name, &[point]).unwrap();
    let seen = writing.seen.lock().unwrap();
    let paths: Vec<String> = seen.iter().map(|(method, path, _)| format!("{method} {path}")).collect();
    assert_eq!(
        paths,
        [
            format!("DELETE /collections/{name}?wait=true"),
            format!("PUT /collections/{name}?wait=true"),
            format!("PUT /collections/{name}/points?wait=true"),
        ]
    );
    for expected in ["\"size\":4", "\"distance\":\"Cosine\"", "\"on_disk\":true", "\"managed_by\":\"huginn\"", "\"index_text_version\":1"] {
        assert!(seen[1].2.contains(expected), "{expected} in {}", seen[1].2);
    }
    for expected in ["\"doc_id\":\"d\"", "\"kind\":\"question\"", "\"root\":\"eureka-state\"", "\"ordinal\":7", "\"text_sha256\":\"h\""] {
        assert!(seen[2].2.contains(expected), "{expected} in {}", seen[2].2);
    }

    let failing = stub(&[("PUT /collections/", 500, r#"{"status":{"error":"disk full"}}"#)]);
    assert!(QdrantIndex::new(&failing.url).upsert(&name, &[]).is_err());
}

/// The collection's name is the contract with the store: one per instance,
/// `huginn_mind_<instance>`.
#[test]
fn the_collection_is_named_for_the_instance() {
    assert_eq!(collection_name(&slug("yggdrasil")), "huginn_mind_yggdrasil");
    assert_eq!(collection_name(&slug("a.b")), "huginn_mind_a.b");
}

#[test]
fn a_step_error_says_what_stopped_it() {
    assert_eq!(StepError::Refused("not ours".into()).to_string(), "not ours");
    let unavailable = StepError::Unavailable(anyhow::anyhow!("inner").context("outer"));
    assert_eq!(unavailable.to_string(), "outer: inner");
}

/// What the sink reports is the worker's health with the inbox added to the
/// pending count, and a current index with something queued is behind.
#[test]
fn queued_entries_count_toward_every_status_that_has_a_pending_count() {
    use IndexStatus as S;
    assert_eq!(including(&S::Current, 0), S::Current);
    assert_eq!(including(&S::Current, 3), S::Behind { pending: 3 });
    assert_eq!(including(&S::Reconciling { pending: 2 }, 3), S::Reconciling { pending: 5 });
    assert_eq!(including(&S::Behind { pending: 2 }, 3), S::Behind { pending: 5 });
    assert_eq!(
        including(&S::Failing { pending: 2, attempts: 4, error: "e".into() }, 3),
        S::Failing { pending: 5, attempts: 4, error: "e".into() }
    );
    assert_eq!(
        including(&S::Refused { pending: 2, attempts: 4, reason: "r".into() }, 3),
        S::Refused { pending: 5, attempts: 4, reason: "r".into() }
    );
}

/// Dropping the sink stops the worker: the worker's hold on the shared state
/// is released. A worker that never stopped would hold it forever.
#[test]
fn dropping_the_sink_stops_the_worker() {
    let (embedder, index) = pair();
    let sink = WorkerSink::spawn(embedder, index, &slug(INSTANCE), Some(MIND.into()), Vec::new(), quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    let shared = Arc::downgrade(&sink.shared);
    drop(sink);
    eventually("the worker letting go of the shared state", || shared.upgrade().is_none());
}

/// F1. The promised curve, pure: 30 s doubling to a 10 min cap, never below
/// the first delay and never past the cap. A retry that shrank would hot-loop
/// against a dead Ollama.
#[test]
fn the_backoff_doubles_from_thirty_seconds_to_a_ten_minute_cap() {
    let backoff = Backoff::default();
    assert_eq!((backoff.initial, backoff.max), (Duration::from_secs(30), Duration::from_secs(600)));
    assert_eq!(backoff.recheck, Duration::from_secs(60), "an idle worker verifies the model and the collection each minute");
    let mut delay = backoff.initial;
    let mut seen = vec![delay.as_secs()];
    for _ in 0..7 {
        let next = backoff.after(delay);
        assert!(next >= delay && next >= backoff.initial, "the delay never shrinks: {delay:?} -> {next:?}");
        assert!(next <= backoff.max, "the delay never passes the cap: {next:?}");
        delay = next;
        seen.push(delay.as_secs());
    }
    assert_eq!(seen, [30, 60, 120, 240, 480, 600, 600, 600]);
    let small = Backoff { initial: Duration::from_millis(1), max: Duration::from_millis(3), recheck: Duration::from_secs(3600), search_deadline: Duration::from_secs(30) };
    assert_eq!(small.after(Duration::from_millis(1)), Duration::from_millis(2));
    assert_eq!(small.after(Duration::from_millis(2)), Duration::from_millis(3));
}

/// F2. A collection lost while the daemon runs is found by the next failed
/// write, recreated and refilled from what the projector knows, without a
/// restart.
#[test]
fn a_collection_lost_after_startup_is_recreated_and_refilled() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);
    assert_eq!(projector.progress_status(), IndexStatus::Current);
    assert_eq!(index.holding(&collection()).len(), 3);

    index.state.lock().unwrap().collections.remove(&collection());
    projector.want(entries[3..].iter().cloned());
    let error = projector.advance().expect_err("the write meets no collection");
    assert!(matches!(error, StepError::Unavailable(_)), "{error}");
    assert!(matches!(projector.failure_status(&error, 1), IndexStatus::Failing { pending: 1, .. }));

    drain(&mut projector);
    assert_eq!(index.state.lock().unwrap().recreated.len(), 2, "the collection was made again");
    assert_eq!(index.holding(&collection()), point_ids(&entries), "and holds every document, not only the new one");
    assert_eq!(projector.progress_status(), IndexStatus::Current);
}

/// F2. A model re-pulled with other dimensions is seen before anything is
/// written: no vector of the new length is offered to the old collection, and
/// the next step rebuilds the collection to match.
#[test]
fn a_model_that_changes_dimensions_after_startup_rebuilds_the_collection() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);

    embedder.state.lock().unwrap().identity.dimensions = 8;
    projector.want(entries[3..].iter().cloned());
    let writes = index.state.lock().unwrap().upserts.len();
    assert_eq!(projector.advance().unwrap(), Advance::Progressed, "the change is seen, not written through");
    assert_eq!(index.state.lock().unwrap().upserts.len(), writes, "no vector of 8 was offered to a collection of 4");
    drain(&mut projector);

    let state = index.state.lock().unwrap();
    assert_eq!(state.recreated.len(), 2);
    assert_eq!(state.recreated.last().unwrap().1.dimensions, 8);
    let stored = &state.collections[&collection()];
    assert_eq!(stored.points.len(), 4);
    assert!(stored.points.values().all(|point| point.vector.len() == 8));
}

/// F3. Two minds that share an instance name never share a collection: the
/// second is refused, and the first's points, label and history are untouched.
#[test]
fn a_second_mind_of_the_same_name_is_refused_and_leaves_the_first_alone() {
    let (_first_root, first) = mind_with_documents();
    let (_second_root, second) = other_mind_of_the_same_name();
    let (first_id, second_id) = (first.genesis_receipt_id().unwrap().unwrap(), second.genesis_receipt_id().unwrap().unwrap());
    assert_ne!(first_id, second_id);

    let (embedder, index) = pair();
    let mut ours = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(first_id.clone()));
    ours.want(first.index_entries(None).unwrap());
    drain(&mut ours);
    let held = index.holding(&collection());
    assert_eq!(held.len(), 4);

    let intruder = FakeEmbedder::new("d1");
    let mut theirs = Projector::new(intruder.clone(), index.clone(), &slug(INSTANCE), Some(second_id));
    theirs.want(second.index_entries(None).unwrap());
    let error = theirs.advance().expect_err("another mind's collection is refused");
    assert!(matches!(error, StepError::Refused(_)), "{error}");
    assert!(matches!(theirs.failure_status(&error, 1), IndexStatus::Refused { .. }));

    let state = index.state.lock().unwrap();
    assert_eq!(state.recreated.len(), 1, "never rebuilt by the second mind");
    assert_eq!(state.upserts.len(), 1, "never written by the second mind");
    assert!(intruder.texts().is_empty());
    let Described::Labelled(label) = &state.collections[&collection()].label else { panic!() };
    assert_eq!(label.mind, first_id);
    drop(state);
    assert_eq!(index.holding(&collection()), held);
}

/// A mind with no receipt has no identity: nothing is reconciled and no
/// collection is made, until the first admission reveals one.
#[test]
fn a_mind_without_an_identity_makes_no_collection_until_its_first_admission() {
    let root = tempfile::tempdir().unwrap();
    let mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
    assert_eq!(mind.genesis_receipt_id().unwrap(), None);
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), None);
    assert_eq!(projector.progress_status(), IndexStatus::Current);
    assert_eq!(projector.advance().unwrap(), Advance::Idle);
    let identified = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    assert_eq!(identified.progress_status(), IndexStatus::Behind { pending: 0 }, "an identified mind has yet to build its first index: lag, not a rebuild");
    assert!(index.state.lock().unwrap().collections.is_empty());

    let sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), None, Vec::new(), quick());
    let mut daemon = Daemon::new(mind, sink);
    let seed = campaign_seed();
    let without_text = vec![seed[0].clone(), seed[1].clone()];
    committed(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, without_text)), now()).answered());
    for _ in 0..500 {
        if index.state.lock().unwrap().collections.contains_key(&collection()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let state = index.state.lock().unwrap();
    let Described::Labelled(label) = &state.collections[&collection()].label else { panic!("no collection was made") };
    assert_eq!(Some(&label.mind), daemon.mind().genesis_receipt_id().unwrap().as_ref());
}

/// Waits for a condition the worker will make true, without a clock in the
/// assertion: it polls for at most ten seconds and then says what never
/// happened.
fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{what} never happened");
}

/// A model re-pulled under the same name and dimensions but a new digest must
/// not mix its vectors with the old model's. The check is before the write,
/// not after a failure: nothing is offered to the old collection, the
/// collection is rebuilt under the new digest, and every document is embedded
/// again.
#[test]
fn a_model_re_pulled_with_the_same_dimensions_never_writes_beside_the_old_vectors() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);

    embedder.state.lock().unwrap().identity.digest = "d2".into();
    projector.want(entries[3..].iter().cloned());
    let writes = index.state.lock().unwrap().upserts.len();
    assert_eq!(projector.advance().unwrap(), Advance::Progressed);
    assert_eq!(index.state.lock().unwrap().upserts.len(), writes, "the new vector was not written into the old collection");
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 1 }, "the reconciliation was forgotten, not the entry");

    drain(&mut projector);
    let state = index.state.lock().unwrap();
    assert_eq!(state.recreated.len(), 2);
    assert_eq!(state.recreated.last().unwrap().1.model_digest, "d2");
    let stored = &state.collections[&collection()];
    assert_eq!(stored.points.len(), 4, "every document is in the rebuilt collection");
    assert_eq!(embedder.texts().len(), 3 + 4, "and was embedded again under the new digest");
}

/// Nothing is written while idle, so no write can fail to announce that the
/// collection is gone or the model has changed. The idle worker looks again on
/// its own clock: a lost collection is refilled and a re-pulled model rebuilds
/// it, with no admission to prompt either.
#[test]
fn an_idle_worker_finds_a_lost_collection_and_a_changed_model_without_a_write() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let recheck = Backoff { recheck: Duration::from_millis(20), ..quick() };
    let sink = WorkerSink::spawn(embedder.clone(), index.clone(), &slug(INSTANCE), mind.genesis_receipt_id().unwrap(), entries.clone(), recheck);
    sink.wait_status(|status| *status == IndexStatus::Current);
    assert_eq!(index.holding(&collection()), point_ids(&entries));

    index.state.lock().unwrap().collections.remove(&collection());
    eventually("the lost collection being made again and refilled", || {
        index.state.lock().unwrap().recreated.len() == 2 && index.holding(&collection()) == point_ids(&entries)
    });

    embedder.state.lock().unwrap().identity.digest = "d2".into();
    eventually("the collection being rebuilt under the new digest", || {
        let state = index.state.lock().unwrap();
        state.recreated.len() == 3
            && state.recreated.last().is_some_and(|(_, meta)| meta.model_digest == "d2")
            && state.collections[&collection()].points.len() == 4
    });
    sink.wait_status(|status| *status == IndexStatus::Current);
}

/// A search embeds the query behind the model's instruction, asks the index
/// with no filter for `min(top_k * 4, 200)`, and hands back candidates by id
/// and kind; a hit of a kind this organ has no name for is left out.
#[test]
fn a_search_embeds_the_query_with_its_instruction_and_oversamples() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    index.state.lock().unwrap().hits = vec![
        Hit { doc_id: "eureka-state:question:Q1".into(), kind: "question".into(), score: 0.9 },
        Hit { doc_id: "eureka-state:mystery:X".into(), kind: "mystery".into(), score: 0.8 },
    ];

    let hits = projector.search("who owns the state", 3, SEARCH_DEADLINE).unwrap();
    assert_eq!(hits.len(), 1, "the unknown kind is left out");
    assert_eq!(hits[0].0.id.0, "eureka-state:question:Q1");
    assert_eq!(hits[0].0.kind, PipelineKind::Question);
    assert_eq!(hits[0].1, 0.9);
    let embedded = embedder.texts();
    assert_eq!(embedded.last().unwrap(), &format!("Instruct: {QUERY_INSTRUCTION}\nQuery: who owns the state"));
    projector.search("again", 100, SEARCH_DEADLINE).unwrap();
    let searches = index.state.lock().unwrap().searches.clone();
    assert_eq!(searches.iter().map(|(name, length, limit)| (name.as_str(), *length, *limit)).collect::<Vec<_>>(), [
        (collection().as_str(), 4, 12),
        (collection().as_str(), 4, 200),
    ]);
}

/// A search is never run against a collection that is not known to fit the
/// model: before reconciliation, and after the model has changed, it refuses
/// by saying so rather than comparing vectors of two models.
#[test]
fn a_search_refuses_when_the_index_is_not_reconciled_with_the_model() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut fresh = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    assert!(format!("{:#}", fresh.search("anything", 3, SEARCH_DEADLINE).unwrap_err()).contains("not been built"));
    assert!(embedder.texts().is_empty() && index.state.lock().unwrap().searches.is_empty());

    let mut projector = projected(&embedder, &index, &entries);
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    let before = embedder.texts().len();
    assert!(format!("{:#}", projector.search("anything", 3, SEARCH_DEADLINE).unwrap_err()).contains("model changed"));
    assert_eq!(embedder.texts().len(), before, "the query was not embedded under the new model");
    assert!(index.state.lock().unwrap().searches.is_empty());
    assert!(projector.stale(), "and the reconciliation is to be redone");
}

/// The search asks for the nearest points to a vector with no filter of any
/// kind (which documents may be shown is the mind's), and reads back the
/// document id, the kind and the score, in the order Qdrant gave them. A hit
/// missing what the join needs is an error, not a guess.
#[test]
fn qdrant_search_sends_no_filter_and_reads_ids_kinds_and_scores() {
    let name = collection();
    let answering = stub(&[(
        "POST /collections/",
        200,
        r#"{"result":[{"id":"p1","score":0.75,"payload":{"doc_id":"eureka-state:question:Q1","kind":"question"}},{"id":"p2","score":0.5,"payload":{"doc_id":"eureka-state:ruling:R1","kind":"ruling"}}]}"#,
    )]);
    let hits = QdrantIndex::new(&answering.url).search(&name, &[0.5, 1.0, 2.0, 3.0], 12, TIME).unwrap();
    assert_eq!(
        hits,
        [
            Hit { doc_id: "eureka-state:question:Q1".into(), kind: "question".into(), score: 0.75 },
            Hit { doc_id: "eureka-state:ruling:R1".into(), kind: "ruling".into(), score: 0.5 },
        ]
    );
    let seen = answering.seen.lock().unwrap();
    assert_eq!((seen[0].0.as_str(), seen[0].1.as_str()), ("POST", format!("/collections/{name}/points/search").as_str()));
    for expected in ["\"vector\":[0.5,1.0,2.0,3.0]", "\"limit\":12", "\"with_vector\":false"] {
        assert!(seen[0].2.contains(expected), "{expected} in {}", seen[0].2);
    }
    assert!(!seen[0].2.contains("filter"), "no predicate is lowered into the index: {}", seen[0].2);
    drop(seen);

    let nameless = stub(&[("POST /collections/", 200, r#"{"result":[{"id":"p","score":0.5,"payload":{"kind":"question"}}]}"#)]);
    assert!(QdrantIndex::new(&nameless.url).search(&name, &[1.0], 1, TIME).is_err());
    let failing = stub(&[("POST /collections/", 404, r#"{"status":{"error":"not found"}}"#)]);
    assert!(QdrantIndex::new(&failing.url).search(&name, &[1.0], 1, TIME).is_err());
}

fn queued(ticket: u64, text: &str) -> Queued {
    Queued { ticket: SearchTicket(ticket), text: text.into(), top_k: 3, due: Instant::now() + SEARCH_DEADLINE }
}

fn shared_with(
    inbox: Vec<IndexEntry>,
    mind: Option<String>,
    searches: Vec<Queued>,
    closed: bool,
) -> Lock {
    Arc::new((
        Mutex::new(Shared {
            inbox,
            mind,
            status: IndexStatus::Current,
            closed,
            searches: searches.into(),
            found: Vec::new(),
            next_ticket: 0,
        }),
        Condvar::new(),
    ))
}

/// The idle wait is for work: an entry, the mind's identity, a search or the
/// sink closing ends it at once, and only a quiet worker waits out its
/// deadline. The deadline is the caller's: a deadline already past ends the
/// wait at once too, and waking for work does not extend it.
#[test]
fn an_idle_wait_ends_for_work_and_otherwise_at_its_deadline() {
    let (_root, mind) = mind_with_documents();
    let entry = mind.index_entries(None).unwrap().remove(0);
    let waits = |shared: &Lock| {
        let started = Instant::now();
        wait_for_work(shared, Wait::Work { until: Instant::now() + Duration::from_secs(30) });
        started.elapsed() < Duration::from_secs(10)
    };
    assert!(waits(&shared_with(vec![entry], None, Vec::new(), false)), "an entry");
    assert!(waits(&shared_with(Vec::new(), Some(MIND.into()), Vec::new(), false)), "the mind's identity");
    assert!(waits(&shared_with(Vec::new(), None, vec![queued(0, "q")], false)), "a search");
    assert!(waits(&shared_with(Vec::new(), None, Vec::new(), true)), "the sink closing");

    let quiet = shared_with(Vec::new(), None, Vec::new(), false);
    let started = Instant::now();
    wait_for_work(&quiet, Wait::Work { until: started + Duration::from_millis(60) });
    assert!(started.elapsed() >= Duration::from_millis(60), "a quiet worker waits for its deadline");
    let started = Instant::now();
    wait_for_work(&quiet, Wait::Work { until: started.checked_sub(Duration::from_secs(1)).unwrap_or(started) });
    assert!(started.elapsed() < Duration::from_secs(10), "a deadline already past is not waited for");
}

/// A backoff is the whole delay, whatever arrives: a failing index is retried
/// on its schedule.
#[test]
fn a_backoff_wait_is_the_whole_delay_whatever_arrives() {
    let (_root, mind) = mind_with_documents();
    let entry = mind.index_entries(None).unwrap().remove(0);
    let busy = shared_with(vec![entry], Some(MIND.into()), vec![queued(0, "q")], false);
    let started = Instant::now();
    wait_for_work(&busy, Wait::Backoff(Duration::from_millis(80)));
    assert!(started.elapsed() >= Duration::from_millis(80), "the delay was waited out although work was queued");
    let closed = shared_with(Vec::new(), None, Vec::new(), true);
    let started = Instant::now();
    wait_for_work(&closed, Wait::Backoff(Duration::from_secs(30)));
    assert!(started.elapsed() < Duration::from_secs(10), "a closed sink ends even a backoff");
}

/// Searches accepted while the index was healthy are answered, with the
/// reason, when the worker fails: nothing is left to wait for its deadline.
#[test]
fn queued_searches_are_answered_with_why_when_the_worker_gives_up() {
    let shared = shared_with(Vec::new(), None, vec![queued(3, "a"), queued(4, "b")], false);
    refuse_searches(&shared, "the index could not advance: down");
    let guard = lock(&shared);
    assert!(guard.searches.is_empty());
    assert_eq!(
        guard.found,
        [
            (SearchTicket(3), Err("the index could not advance: down".to_string())),
            (SearchTicket(4), Err("the index could not advance: down".to_string())),
        ]
    );
}

// The semantic read's index side: when a search is answered, how many wait,
// and what the worker does between them.

type Answers = Vec<(SearchTicket, Result<Hits, String>)>;

/// The sink's own calls, named: the trait is generic over the store and the
/// worker does not care which.
fn ask_sink(sink: &mut WorkerSink, text: &str, top_k: u32) -> Result<SearchTicket, String> {
    <WorkerSink as IndexSink<OwnedRedbMessagePackBackingStore>>::search(sink, text, top_k)
}

fn collected(sink: &mut WorkerSink) -> Answers {
    <WorkerSink as IndexSink<OwnedRedbMessagePackBackingStore>>::searched(sink)
}

fn abandon(sink: &mut WorkerSink, ticket: SearchTicket) {
    <WorkerSink as IndexSink<OwnedRedbMessagePackBackingStore>>::abandon(sink, ticket);
}

/// Waits for `count` answers and returns them, in the order they were given.
fn answers(sink: &mut WorkerSink, count: usize) -> Answers {
    let mut all = Vec::new();
    eventually("the searches being answered", || {
        all.extend(collected(sink));
        all.len() >= count
    });
    all
}

/// `count` documents that share one document's text and differ in id, for the
/// tests whose subject is how many there are.
fn entries_of(count: usize) -> Vec<IndexEntry> {
    let (_root, mind) = mind_with_documents();
    let template = mind.index_entries(None).unwrap().remove(0);
    (0..count)
        .map(|n| {
            let mut entry = template.clone();
            entry.id.id.0 = format!("{}#{n}", entry.id.id.0);
            entry
        })
        .collect()
}

fn queries(embedder: &FakeEmbedder) -> Vec<String> {
    embedder.texts().into_iter().filter(|text| text.starts_with("Instruct: ")).collect()
}

/// A search is refused only while the index is being rebuilt or cannot be
/// asked, and the words name the state and how many documents are pending. Lag
/// is not a rebuild: the sink and the projector ask the one predicate and both
/// let it through.
#[test]
fn a_search_is_refused_only_for_a_rebuild_or_a_failure() {
    use IndexStatus as S;
    assert_eq!(unsearchable(&S::Current), None);
    assert_eq!(unsearchable(&S::Behind { pending: 5 }), None, "lag is waited out, not refused");
    let says = |status: S, wanted: &[&str]| {
        let why = unsearchable(&status).unwrap_or_else(|| panic!("{status:?} answered"));
        for word in wanted {
            assert!(why.contains(word), "{status:?}: {word} in {why}");
        }
    };
    says(S::Reconciling { pending: 7 }, &["being rebuilt", "reconciling", "pending: 7"]);
    says(S::Failing { pending: 3, attempts: 2, error: "boom".into() }, &["failing", "boom", "pending: 3"]);
    says(S::Refused { pending: 4, attempts: 2, reason: "not ours".into() }, &["refused", "not ours", "pending: 4"]);

    // The sink: what the worker last published, and the documents admitted
    // since, which are lag whatever the published state says of a current index.
    let sink_in = |status: S, inbox: usize| {
        let shared = shared_with(entries_of(inbox), None, Vec::new(), false);
        lock(&shared).status = status;
        WorkerSink { shared, identified: true, deadline: SEARCH_DEADLINE }
    };
    assert!(ask_sink(&mut sink_in(S::Current, 2), "anything", 3).is_ok(), "admitted a moment ago and not yet absorbed");
    assert!(ask_sink(&mut sink_in(S::Behind { pending: 4 }, 0), "anything", 3).is_ok(), "the worker is writing");
    let refusal = ask_sink(&mut sink_in(S::Reconciling { pending: 4 }, 2), "anything", 3).unwrap_err();
    assert!(refusal.contains("being rebuilt") && refusal.contains("pending: 6"), "{refusal}");
    let refusal = ask_sink(&mut sink_in(S::Failing { pending: 1, attempts: 1, error: "boom".into() }, 0), "anything", 3).unwrap_err();
    assert!(refusal.contains("failing") && refusal.contains("boom"), "{refusal}");

    // The projector: a rebuild refuses without touching the ports to find out.
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let (lost_embedder, lost_index) = pair();
    let mut rebuilding_now = rebuilding(&lost_embedder, &lost_index, &entries);
    let searches = lost_index.state.lock().unwrap().searches.len();
    let refusal = format!("{:#}", rebuilding_now.search("anything", 3, SEARCH_DEADLINE).unwrap_err());
    assert!(refusal.contains("being rebuilt") && refusal.contains("pending: 4"), "{refusal}");
    assert_eq!(lost_index.state.lock().unwrap().searches.len(), searches, "the rebuilt collection was not asked");
    assert!(embedder.texts().is_empty() && index.state.lock().unwrap().searches.is_empty());

    // Lag it lets through: catching up is the worker's, before the search is taken.
    let mut lagging = projected(&embedder, &index, &entries[..3]);
    lagging.want(entries[3..].iter().cloned());
    assert_eq!(lagging.progress_status(), IndexStatus::Behind { pending: 1 });
    assert!(lagging.search("anything", 3, SEARCH_DEADLINE).is_ok());
}

/// A search that fails for its own reasons (the vector store errs, the
/// embedder is down, an identity read times out) fails that search alone: the
/// reconciliation stands, the status stays `Current`, the collection is not
/// listed again, and the next search is asked and answered.
#[test]
fn a_failed_search_fails_that_search_alone() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    type Fault = Box<dyn Fn(&FakeEmbedder, &FakeIndex, bool)>;
    let faults: [(&str, Fault); 3] = [
        ("the vector store fails the search", Box::new(|_, index, on| index.state.lock().unwrap().fail_search = on)),
        ("the embedder is down", Box::new(|embedder, _, on| embedder.state.lock().unwrap().down = on)),
        ("the identity read never answers", Box::new(|embedder, _, on| embedder.state.lock().unwrap().hang_identity = on)),
    ];
    for (what, fault) in faults {
        let (embedder, index) = pair();
        let mut projector = projected(&embedder, &index, &entries);
        let listed = index.state.lock().unwrap().listed;
        fault(&embedder, &index, true);
        assert!(projector.search("anything", 3, Duration::from_millis(50)).is_err(), "{what}");
        assert_eq!(projector.progress_status(), IndexStatus::Current, "{what}");
        assert!(!projector.stale(), "{what}");
        fault(&embedder, &index, false);
        assert!(projector.search("anything", 3, SEARCH_DEADLINE).is_ok(), "{what}: the next search is answered");
        assert_eq!(index.state.lock().unwrap().listed, listed, "{what}: the collection was not listed again");
    }

    // In the worker the failed search is answered and nothing else moves.
    let (embedder, index) = pair();
    let mut sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries, quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    let listed = index.state.lock().unwrap().listed;
    index.state.lock().unwrap().fail_search = true;
    let ticket = ask_sink(&mut sink, "anything", 3).unwrap();
    let failed = answers(&mut sink, 1);
    assert_eq!(failed[0].0, ticket);
    assert!(failed[0].1.is_err());
    index.state.lock().unwrap().fail_search = false;
    let next = ask_sink(&mut sink, "anything", 3).expect("the index is still current, not being rebuilt");
    let answered = answers(&mut sink, 1);
    assert_eq!((answered[0].0, answered[0].1.is_ok()), (next, true));
    assert_eq!(index.state.lock().unwrap().listed, listed, "the worker did not list the collection again");
}

/// A model the search finds changed is a rebuild, and its refusal names the
/// state and how many documents are pending, as every other refusal does.
#[test]
fn a_model_changed_refusal_names_the_state_and_the_pending_count() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);
    projector.want(entries[3..].iter().cloned());
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    let refusal = format!("{:#}", projector.search("anything", 3, SEARCH_DEADLINE).unwrap_err());
    assert!(refusal.contains("model changed") && refusal.contains("being rebuilt") && refusal.contains("pending: 1"), "{refusal}");
}

/// A search asks the store what the collection says about itself: a label for
/// another mind, no label, or no collection is refused before the query is
/// embedded, and the reconciliation is to be redone.
#[test]
fn a_search_re_reads_the_collections_label() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let cases: [(&str, Box<dyn Fn(&mut Stored)>); 3] = [
        (
            "another mind's label",
            Box::new(|stored| {
                let Described::Labelled(meta) = &mut stored.label else { panic!() };
                meta.mind = "mind-commit-another".into();
            }),
        ),
        ("no label", Box::new(|stored| stored.label = Described::Unlabelled)),
        ("another text version", Box::new(|stored| {
            let Described::Labelled(meta) = &mut stored.label else { panic!() };
            meta.index_text_version += 1;
        })),
    ];
    for (what, change) in cases {
        let (embedder, index) = pair();
        let mut projector = projected(&embedder, &index, &entries);
        change(index.state.lock().unwrap().collections.get_mut(&collection()).unwrap());
        let embedded = embedder.texts().len();
        let refusal = format!("{:#}", projector.search("anything", 3, SEARCH_DEADLINE).unwrap_err());
        assert!(refusal.contains("no longer the one this mind's index was built as"), "{what}: {refusal}");
        assert_eq!(embedder.texts().len(), embedded, "{what}: nothing was embedded");
        assert!(index.state.lock().unwrap().searches.is_empty(), "{what}");
        assert!(projector.stale(), "{what}");
    }
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    index.state.lock().unwrap().collections.clear();
    assert!(projector.search("anything", 3, SEARCH_DEADLINE).is_err(), "no collection");
    assert!(projector.stale());
}

/// The model is read again after the query is embedded: a vector made while the
/// model changed is never compared with the collection.
#[test]
fn a_model_that_changes_while_the_query_is_embedded_is_refused() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let gate = Gate::shut();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let index = FakeIndex::default();
    gate.open();
    let mut projector = projected(&embedder, &index, &entries);
    gate.shut_again();
    let searching = std::thread::spawn(move || {
        let found = projector.search("anything", 3, SEARCH_DEADLINE);
        (projector, found)
    });
    gate.wait_arrived(1);
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    gate.open();
    let (projector, found) = searching.join().unwrap();
    let refusal = format!("{:#}", found.unwrap_err());
    assert!(refusal.contains("while the texts were embedded"), "{refusal}");
    assert!(index.state.lock().unwrap().searches.is_empty(), "the vector was not used");
    assert!(projector.stale());
}

/// Each search has a ticket of its own, and its answer comes back once, as the
/// worker gave it: nothing is added to it and nothing is left to be
/// collected twice.
#[test]
fn every_search_has_its_own_ticket_and_its_answer_comes_back_exactly_once() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    index.state.lock().unwrap().hits =
        vec![Hit { doc_id: "eureka-state:question:Q1".into(), kind: "question".into(), score: 0.9 }];
    let mut sink = WorkerSink::spawn(embedder, index, &slug(INSTANCE), Some(MIND.into()), entries, quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    let expected: Hits = vec![(
        PipelineRef { kind: PipelineKind::Question, id: Short("eureka-state:question:Q1".into()) },
        0.9,
    )];
    let mut tickets = Vec::new();
    for text in ["a", "b", "c"] {
        let ticket = ask_sink(&mut sink, text, 3).unwrap();
        assert_eq!(answers(&mut sink, 1), vec![(ticket, Ok(expected.clone()))]);
        assert!(collected(&mut sink).is_empty(), "an answer is handed over once");
        tickets.push(ticket);
    }
    assert_eq!(tickets, [SearchTicket(0), SearchTicket(1), SearchTicket(2)]);
}

/// The queue of searches is bounded and the asker who does not fit is told.
/// One search is inside the embedder and the queue is full; the next is
/// refused by name, and when the worker has answered some it takes more.
#[test]
fn the_search_queue_is_bounded_and_the_asker_is_told_when_it_is_full() {
    let gate = Gate::shut();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let mut sink = WorkerSink::spawn(embedder, index, &slug(INSTANCE), Some(MIND.into()), Vec::new(), quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    ask_sink(&mut sink, "in flight", 3).unwrap();
    gate.wait_arrived(1);
    for n in 0..SEARCHES_QUEUED_MAX {
        ask_sink(&mut sink, &format!("waiting {n}"), 3).unwrap_or_else(|why| panic!("{n} of {SEARCHES_QUEUED_MAX}: {why}"));
    }
    let refusal = ask_sink(&mut sink, "one too many", 3).unwrap_err();
    assert!(refusal.contains(&format!("{SEARCHES_QUEUED_MAX} searches waiting")), "{refusal}");

    gate.open();
    let answered = answers(&mut sink, SEARCHES_QUEUED_MAX + 1);
    assert_eq!(answered.len(), SEARCHES_QUEUED_MAX + 1);
    assert!(ask_sink(&mut sink, "room again", 3).is_ok());
}

/// A search the asker has stopped waiting for is taken out of the queue and
/// never embedded; the ones on either side of it are.
#[test]
fn an_abandoned_search_is_never_embedded() {
    let gate = Gate::shut();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let mut sink = WorkerSink::spawn(embedder.clone(), index, &slug(INSTANCE), Some(MIND.into()), Vec::new(), quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    let first = ask_sink(&mut sink, "first", 3).unwrap();
    gate.wait_arrived(1);
    let second = ask_sink(&mut sink, "second", 3).unwrap();
    let third = ask_sink(&mut sink, "third", 3).unwrap();
    abandon(&mut sink, second);
    gate.open();
    let answered = answers(&mut sink, 2);
    let tickets: Vec<SearchTicket> = answered.iter().map(|(ticket, _)| *ticket).collect();
    assert_eq!(tickets, [first, third]);
    let texts = queries(&embedder);
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts[0].ends_with("Query: first") && texts[1].ends_with("Query: third"), "{texts:?}");
}

/// The worker serves one queued search and returns to its writes: three
/// waiting searches leave two after one turn.
#[test]
fn one_queued_search_is_served_per_turn() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    let shared = shared_with(Vec::new(), None, vec![queued(1, "a"), queued(2, "b"), queued(3, "c")], false);
    serve_search(&mut projector, &shared);
    let guard = lock(&shared);
    assert_eq!(guard.searches.iter().map(|queued| queued.ticket).collect::<Vec<_>>(), [SearchTicket(2), SearchTicket(3)]);
    assert_eq!(guard.found.iter().map(|(ticket, _)| *ticket).collect::<Vec<_>>(), [SearchTicket(1)]);
}

/// A search reads its own writes. Asked while a batch is in the embedder and
/// eight more documents wait, it is accepted, waits behind them, and is
/// answered only once every one is in the collection.
#[test]
fn a_search_waits_for_the_writes_ahead_of_it_and_reads_them() {
    let gate = Gate::shut();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let mut sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries_of(BATCH + 8), quick());
    gate.wait_arrived(1);
    let ticket = ask_sink(&mut sink, "anything", 3).expect("lag is not a rebuild");
    gate.open();
    let answered = answers(&mut sink, 1);
    assert_eq!(answered[0].0, ticket);
    assert!(answered[0].1.is_ok(), "{answered:?}");
    assert_eq!(index.state.lock().unwrap().held_when_searched, [BATCH + 8], "the search saw every document admitted before it");
    sink.wait_status(|status| *status == IndexStatus::Current);
}

/// The worker takes admissions before the search, and leaves the search queued
/// while the writes ahead of it are unfinished: a search asked with a document
/// still in the inbox absorbs it and waits, and once the index has caught up
/// it is answered. An index being rebuilt is not waited on: the search is taken
/// and refused, naming the state and the pending count.
#[test]
fn the_worker_absorbs_admissions_before_a_search_and_waits_for_lag_but_not_for_a_rebuild() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);
    let shared = shared_with(vec![entries[3].clone()], None, vec![queued(1, "a")], false);

    serve_search(&mut projector, &shared);
    {
        let guard = lock(&shared);
        assert!(guard.inbox.is_empty(), "the admission ahead of the search was absorbed");
        assert_eq!(guard.searches.len(), 1, "the search waits for it to be written");
        assert!(guard.found.is_empty());
    }
    assert_eq!(projector.pending(), 1);

    drain(&mut projector);
    serve_search(&mut projector, &shared);
    let guard = lock(&shared);
    assert!(guard.searches.is_empty());
    assert!(matches!(guard.found.as_slice(), [(SearchTicket(1), Ok(_))]), "{:?}", guard.found);
    assert_eq!(index.state.lock().unwrap().held_when_searched, [4]);
    drop(guard);

    // A rebuild: the collection was lost and made again, and every entry is owed.
    let (rebuild_embedder, rebuild_index) = pair();
    let mut rebuilding = rebuilding(&rebuild_embedder, &rebuild_index, &entries);
    let shared = shared_with(Vec::new(), None, vec![queued(2, "b")], false);
    serve_search(&mut rebuilding, &shared);
    let guard = lock(&shared);
    let [(SearchTicket(2), Err(refusal))] = guard.found.as_slice() else { panic!("{:?}", guard.found) };
    assert!(refusal.contains("being rebuilt") && refusal.contains("pending: 4"), "{refusal}");
}

/// A collection made again with nothing to refill owes nothing, so the rebuild
/// ends inside `reconcile`: the next entry is lag, and a search waits for it
/// rather than being refused as a rebuild.
#[test]
fn a_rebuild_that_owes_nothing_settles_when_it_is_made() {
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder.clone(), index, &slug(INSTANCE), Some(MIND.into()));
    drain(&mut projector);
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    assert_eq!(projector.recheck().unwrap(), Advance::Progressed, "the collection is made again under the new digest");
    assert_eq!(projector.pending(), 0);

    projector.want(entries_of(1));
    assert_eq!(projector.progress_status(), IndexStatus::Behind { pending: 1 }, "nothing was owed, so nothing is being rebuilt");
    let shared = shared_with(Vec::new(), None, vec![queued(1, "a")], false);
    serve_search(&mut projector, &shared);
    let guard = lock(&shared);
    assert_eq!(guard.searches.len(), 1, "the search waits for the write");
    assert!(guard.found.is_empty(), "and is not refused: {:?}", guard.found);
}

/// An identified mind with nothing owed has still to reconcile once: a search
/// waits for that, and is not taken to fail against an index not yet built.
#[test]
fn a_search_waits_for_the_first_reconcile_of_an_identified_mind_owing_nothing() {
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder, index, &slug(INSTANCE), Some(MIND.into()));
    assert_eq!(projector.pending(), 0);
    assert!(projector.stale());
    let shared = shared_with(Vec::new(), None, vec![queued(1, "a")], false);
    serve_search(&mut projector, &shared);
    {
        let guard = lock(&shared);
        assert_eq!(guard.searches.len(), 1, "the search waits for the reconcile");
        assert!(guard.found.is_empty(), "{:?}", guard.found);
    }
    drain(&mut projector);
    serve_search(&mut projector, &shared);
    assert!(matches!(lock(&shared).found.as_slice(), [(SearchTicket(1), Ok(_))]));
}

/// A search that was queued when the worker failed is answered with why, by
/// the worker's own failure path.
#[test]
fn a_search_queued_before_the_worker_fails_is_answered_with_why() {
    let gate = Gate::shut();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let mut sink = WorkerSink::spawn(embedder.clone(), index, &slug(INSTANCE), Some(MIND.into()), entries_of(1), quick());
    gate.wait_arrived(1);
    lock(&sink.shared).searches.push_back(queued(5, "anything"));
    embedder.state.lock().unwrap().down = true;
    gate.open();
    let answered = answers(&mut sink, 1);
    assert_eq!(answered[0].0, SearchTicket(5));
    let Err(why) = &answered[0].1 else { panic!("{answered:?}") };
    assert!(why.contains("the semantic index is failing") && why.contains("pending: 1") && why.contains("the embedder is down"), "{why}");
}

/// While a failure is being reported, the report's pending count is the one
/// the worker holds now, published under the lock that took the entries out of
/// the inbox: never the count from before they moved.
#[test]
fn a_failure_report_carries_the_pending_count_it_has_now() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder, index, &slug(INSTANCE), Some(MIND.into()));
    projector.want(entries[..2].iter().cloned());
    let shared = shared_with(entries[2..].to_vec(), None, Vec::new(), false);
    let error = StepError::Unavailable(anyhow::anyhow!("down"));
    assert!(absorb(&mut projector, &shared, Some((&error, 3))));
    {
        let guard = lock(&shared);
        assert!(guard.inbox.is_empty());
        assert_eq!(guard.status, IndexStatus::Failing { pending: 4, attempts: 3, error: "down".into() });
    }
    assert!(absorb(&mut projector, &shared, None));
    assert_eq!(lock(&shared).status, IndexStatus::Behind { pending: 4 }, "a first build is lag");
    lock(&shared).closed = true;
    assert!(!absorb(&mut projector, &shared, None), "a closed sink ends the worker");
}

/// The worker's recheck is on its own clock: searches arriving faster than the
/// interval do not keep pushing it out.
#[test]
fn steady_searches_do_not_postpone_the_workers_recheck() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let backoff = Backoff { recheck: Duration::from_millis(200), ..quick() };
    let mut sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries, backoff);
    sink.wait_status(|status| *status == IndexStatus::Current);
    let listed = index.state.lock().unwrap().listed;
    let started = Instant::now();
    while index.state.lock().unwrap().listed < listed + 2 {
        assert!(started.elapsed() < Duration::from_secs(5), "the worker never rechecked while searches kept arriving");
        let _ = ask_sink(&mut sink, "anything", 3);
        collected(&mut sink);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `wait_status` gives up at one deadline, not one per wake: a health that
/// keeps changing does not keep it waiting.
#[test]
fn wait_status_gives_up_at_its_overall_deadline_however_often_the_health_changes() {
    let (embedder, index) = pair();
    let sink = WorkerSink::spawn(embedder, index, &slug(INSTANCE), Some(MIND.into()), Vec::new(), quick());
    sink.wait_status(|status| *status == IndexStatus::Current);
    let shared = Arc::clone(&sink.shared);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ticking = Arc::clone(&stop);
    let ticker = std::thread::spawn(move || {
        while !ticking.load(std::sync::atomic::Ordering::Relaxed) {
            shared.1.notify_all();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let (tell, told) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let gave_up = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sink.wait_status_for(Duration::from_millis(300), |status| *status == IndexStatus::Reconciling { pending: 99 })
        }))
        .is_err();
        let _ = tell.send(gave_up);
    });
    let gave_up = told.recv_timeout(Duration::from_secs(10));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    ticker.join().unwrap();
    assert_eq!(gave_up, Ok(true), "the wait did not end at its deadline");
}

/// An idle worker verifies once per interval, not in a loop: the recheck moves
/// its own deadline when it runs.
#[test]
fn an_idle_worker_rechecks_once_per_interval() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let backoff = Backoff { recheck: Duration::from_millis(100), ..quick() };
    let sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries, backoff);
    sink.wait_status(|status| *status == IndexStatus::Current);
    std::thread::sleep(Duration::from_millis(600));
    let listed = index.state.lock().unwrap().listed;
    assert!((2..=40).contains(&listed), "the startup reconciliation and about five rechecks listed the collection {listed} times");
}

// The search's rulings: what may be waiting ahead of it, when a rebuild
// refuses, what a first build is, how time is spent, and who owns the deadline.

/// A search is never taken while a document sits in the inbox. An admission
/// that lands after the inbox was absorbed and before the search was taken (the
/// hook puts one there, where a second lock once left the gap) leaves the
/// search queued; the next turn absorbs it, and the search is answered only
/// once it is written.
#[test]
fn a_search_is_never_taken_while_an_admission_sits_in_the_inbox() {
    let (_root, mind) = mind_with_documents();
    let entries = mind.index_entries(None).unwrap();
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries[..3]);
    let shared = shared_with(Vec::new(), None, vec![queued(1, "a")], false);
    let late = entries[3].clone();
    AFTER_ABSORB.with(|hook| *hook.borrow_mut() = Some(Box::new(move |guard| guard.inbox.push(late))));

    serve_search(&mut projector, &shared);
    {
        let guard = lock(&shared);
        assert_eq!(guard.searches.len(), 1, "the search was not taken past the admission");
        assert!(guard.found.is_empty());
        assert_eq!(guard.inbox.len(), 1);
    }
    serve_search(&mut projector, &shared);
    assert_eq!(lock(&shared).searches.len(), 1, "absorbed, but not yet written");
    drain(&mut projector);
    serve_search(&mut projector, &shared);
    let guard = lock(&shared);
    assert!(matches!(guard.found.as_slice(), [(SearchTicket(1), Ok(_))]), "{:?}", guard.found);
    assert_eq!(index.state.lock().unwrap().held_when_searched, [4], "the search saw the admission");
}

/// Through the sink, an admission acknowledged before the search was asked is
/// in the inbox while the worker is inside a batch; the search reads it.
#[test]
fn a_search_reads_an_admission_acknowledged_before_it_was_asked() {
    let gate = Gate::shut();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let mut sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries_of(2), quick());
    gate.wait_arrived(1);
    let admitted = entries_of(5).split_off(2);
    lock(&sink.shared).inbox.extend(admitted);
    let ticket = ask_sink(&mut sink, "anything", 3).expect("lag is not a rebuild");
    gate.open();
    let answered = answers(&mut sink, 1);
    assert_eq!(answered[0].0, ticket);
    assert!(answered[0].1.is_ok(), "{answered:?}");
    assert_eq!(index.state.lock().unwrap().held_when_searched, [5]);
}

/// A collection made again is a rebuild until every document it owes is
/// written again: searches are refused as rebuilding, however the collection
/// came to be made again, and however many steps or failures the refill takes.
#[test]
fn a_collection_made_again_refuses_searches_until_it_is_refilled() {
    let refused = |projector: &mut Projector<FakeEmbedder, FakeIndex>, pending: u32| {
        let why = format!("{:#}", projector.search("anything", 3, SEARCH_DEADLINE).unwrap_err());
        assert!(why.contains("being rebuilt") && why.contains(&format!("pending: {pending}")), "{why}");
    };
    let entries = entries_of(BATCH + 8);

    // Lost while the worker ran.
    let (embedder, index) = pair();
    let mut projector = rebuilding(&embedder, &index, &entries);
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 40 });
    refused(&mut projector, 40);
    index.state.lock().unwrap().down = true;
    projector.advance().expect_err("the store is down mid-refill");
    index.state.lock().unwrap().down = false;
    assert_eq!(projector.advance().unwrap(), Advance::Progressed, "reconciled again, under a label that already fits");
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 40 }, "a failure does not end the rebuild");
    assert_eq!(projector.advance().unwrap(), Advance::Progressed);
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 8 }, "half refilled is still a rebuild");
    refused(&mut projector, 8);
    assert_eq!(projector.advance().unwrap(), Advance::Progressed);
    assert_eq!(projector.progress_status(), IndexStatus::Current);
    assert!(projector.search("anything", 3, SEARCH_DEADLINE).is_ok(), "answered once refilled");
    assert_eq!(index.state.lock().unwrap().held_when_searched, [40]);
    projector.want(entries_of(1).into_iter().map(|mut entry| {
        entry.id.id.0.push_str("-later");
        entry
    }));
    assert_eq!(projector.progress_status(), IndexStatus::Behind { pending: 1 }, "a later admission is lag, not a rebuild");

    // Made again because the model changed.
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    embedder.state.lock().unwrap().identity.digest = "d2".into();
    assert_eq!(projector.recheck().unwrap(), Advance::Progressed, "the change is seen and the collection made again");
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 40 });
    projector.advance().unwrap();
    assert_eq!(projector.progress_status(), IndexStatus::Reconciling { pending: 8 });
    refused(&mut projector, 8);
    drain(&mut projector);
    assert!(projector.search("anything", 3, SEARCH_DEADLINE).is_ok());
}

/// Through the worker: a collection lost while the worker runs is made again,
/// and a search asked during the refill is refused as rebuilding, naming how
/// many documents are pending; one asked after it is answered.
#[test]
fn a_search_asked_during_a_refill_is_refused_and_one_after_it_is_answered() {
    let gate = Gate::shut();
    gate.open();
    let (_, index) = pair();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let backoff = Backoff { recheck: Duration::from_millis(50), ..quick() };
    let mut sink = WorkerSink::spawn(embedder, index.clone(), &slug(INSTANCE), Some(MIND.into()), entries_of(BATCH + 8), backoff);
    sink.wait_status(|status| *status == IndexStatus::Current);
    gate.shut_again();
    index.state.lock().unwrap().collections.remove(&collection());
    gate.wait_arrived(1);
    let refusal = ask_sink(&mut sink, "anything", 3).unwrap_err();
    assert!(refusal.contains("being rebuilt") && refusal.contains("pending: 40"), "{refusal}");
    gate.open();
    sink.wait_status(|status| *status == IndexStatus::Current);
    let ticket = ask_sink(&mut sink, "anything", 3).expect("refilled");
    let answered = answers(&mut sink, 1);
    assert_eq!((answered[0].0, answered[0].1.is_ok()), (ticket, true));
    assert_eq!(index.state.lock().unwrap().held_when_searched.last(), Some(&40));
}

/// A mind whose index has never been built has nothing stale to protect: its
/// first build is lag, whether the search was asked before the worker had
/// looked at the first admission or after. Only a rebuild of an index that
/// exists refuses.
#[test]
fn a_minds_first_build_is_lag_whoever_looks_first() {
    for asked_before_the_worker_looked in [true, false] {
        let what = if asked_before_the_worker_looked { "asked first" } else { "asked after the worker looked" };
        let (embedder, index) = pair();
        let entries = entries_of(3);
        let (mut projector, shared) = if asked_before_the_worker_looked {
            let projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), None);
            (projector, shared_with(entries, Some(MIND.into()), Vec::new(), false))
        } else {
            let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
            projector.want(entries);
            let shared = shared_with(Vec::new(), None, Vec::new(), false);
            lock(&shared).status = projector.progress_status();
            (projector, shared)
        };
        let mut sink = WorkerSink { shared: Arc::clone(&shared), identified: true, deadline: SEARCH_DEADLINE };
        let ticket = ask_sink(&mut sink, "anything", 3).unwrap_or_else(|why| panic!("{what}: {why}"));

        serve_search(&mut projector, &shared);
        assert_eq!(projector.progress_status(), IndexStatus::Behind { pending: 3 }, "{what}: not yet reconciled, and lag");
        assert_eq!(lock(&shared).searches.len(), 1, "{what}: the search waits for the first reconciliation");
        assert_eq!(projector.advance().unwrap(), Advance::Progressed);
        serve_search(&mut projector, &shared);
        assert_eq!(lock(&shared).searches.len(), 1, "{what}: and for the first writes");
        drain(&mut projector);
        serve_search(&mut projector, &shared);
        let guard = lock(&shared);
        assert!(matches!(guard.found.as_slice(), [(found, Ok(_))] if *found == ticket), "{what}: {:?}", guard.found);
        assert_eq!(index.state.lock().unwrap().held_when_searched, [3], "{what}");
    }

    let entries = entries_of(4);
    let (embedder, index) = pair();
    projected(&embedder, &index, &entries[..3]);
    let mut partial = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), Some(MIND.into()));
    partial.want(entries.iter().cloned());
    partial.advance().unwrap();
    assert_eq!(partial.progress_status(), IndexStatus::Behind { pending: 1 }, "an index found whole at start-up only lags");
    drain(&mut partial);
    index.state.lock().unwrap().collections.remove(&collection());
    assert_eq!(partial.recheck().unwrap(), Advance::Progressed);
    assert_eq!(partial.progress_status(), IndexStatus::Reconciling { pending: 4 }, "an index found at start-up was built: losing it is a rebuild");

    embedder.state.lock().unwrap().identity.digest = "d2".into();
    let mut replaced = Projector::new(embedder, index, &slug(INSTANCE), Some(MIND.into()));
    replaced.want(entries);
    replaced.advance().unwrap();
    assert_eq!(replaced.progress_status(), IndexStatus::Reconciling { pending: 4 }, "an index that exists and is made again is a rebuild");
}

/// A mind with no identity and documents owed has nothing to wait for: the
/// search is taken and refused, not left queued for a worker that would spin
/// on it.
#[test]
fn a_search_is_refused_when_documents_are_owed_to_a_mind_with_no_identity() {
    let (embedder, index) = pair();
    let mut projector = Projector::new(embedder.clone(), index.clone(), &slug(INSTANCE), None);
    projector.want(entries_of(2));
    let shared = shared_with(Vec::new(), None, vec![queued(1, "a")], false);
    serve_search(&mut projector, &shared);
    let guard = lock(&shared);
    assert!(guard.searches.is_empty());
    let [(SearchTicket(1), Err(why))] = guard.found.as_slice() else { panic!("{:?}", guard.found) };
    assert!(why.contains("being rebuilt") && why.contains("pending: 2"), "{why}");
    assert!(embedder.texts().is_empty() && index.state.lock().unwrap().searches.is_empty());
}

/// Every call a search makes is bounded by what its time has left, not by the
/// time it was given: after a slow identity read the embed's bound is smaller
/// by what the read took, and the query's by both reads.
#[test]
fn a_later_calls_bound_shrinks_as_the_search_spends_its_time() {
    let entries = entries_of(3);
    let (embedder, index) = pair();
    let mut projector = projected(&embedder, &index, &entries);
    let delay = Duration::from_millis(150);
    let within = Duration::from_secs(10);
    {
        let mut state = embedder.state.lock().unwrap();
        state.identity_delay = delay;
        state.bounds.clear();
        state.identity_bounds.clear();
    }
    index.state.lock().unwrap().bounds.clear();
    projector.search("anything", 3, within).unwrap();

    let embedder = embedder.state.lock().unwrap();
    let index = index.state.lock().unwrap();
    assert!(embedder.identity_bounds[0] <= within);
    assert!(embedder.bounds[0] <= within - delay, "the embed came after one slow read: {:?}", embedder.bounds);
    assert!(embedder.identity_bounds[1] <= within - delay, "{:?}", embedder.identity_bounds);
    let queried = index.bounds.iter().find(|(call, _)| *call == "search").unwrap().1;
    assert!(queried <= within - delay * 2, "the query came after two slow reads: {queried:?}");
}

/// The deadline has one owner: the sink. It stamps every search with it, and
/// it is what the serve loop is told (`Daemon::search_deadline`), so the loop's
/// wait and the worker's bound are one value.
#[test]
fn the_sink_owns_the_searchs_deadline_and_tells_the_loop() {
    let (_root, mind) = mind_with_documents();
    let deadline = Duration::from_secs(7);
    let shared = shared_with(Vec::new(), None, Vec::new(), false);
    let mut sink = WorkerSink { shared: Arc::clone(&shared), identified: true, deadline };
    let asked = Instant::now();
    ask_sink(&mut sink, "anything", 3).unwrap();
    let due = lock(&shared).searches.back().expect("no worker took it").due;
    assert!(due >= asked + deadline && due <= Instant::now() + deadline, "stamped with the sink's deadline");
    assert_eq!(<WorkerSink as IndexSink<OwnedRedbMessagePackBackingStore>>::search_deadline(&sink), deadline);
    assert_eq!(Daemon::new(mind, sink).search_deadline(), deadline);
}
