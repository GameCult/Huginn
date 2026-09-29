//! The semantic read through the real serve loop: a UDP hub, real sessions, the
//! worker thread, and fake embedder and vector-store ports whose behaviour the
//! test decides. Nothing here calls `Daemon::handle` directly: the rules under
//! test are about what the loop does while an answer is outstanding.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::Result;
use cultnet_rs::{CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, FieldPredicate, Selection};
use huginn_mind::epiphany_pipeline::{
    Date, PipelineDocument, PipelineKind, PipelineQuestion, PipelineResolution, PipelineRef, PipelineRuling,
    QuestionOption, ResolutionOutcome, RulingAuthority, Short, Title,
};
use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse, IndexStatus};
use huginn_mind::{Mind, MindRefusal, OwnedRedbMessagePackBackingStore, PipelineAdmissionOutcome, PipelinePageItems, SemanticQuery};
use tempfile::TempDir;

use crate::daemon::Daemon;
use crate::daemon::tests::{CAMPAIGN, INSTANCE, batch, campaign_seed, now, slug};
use huginn_mind::envelope::{decode_response, encode_request};
use crate::index::fakes::{FakeEmbedder, FakeIndex, Gate};
use crate::index::{Backoff, Described, Hit, WorkerSink, collection_name};
use crate::serve::{ServeOptions, bind, run, schema_registry};

type Client = cultnet_rs::CultNetRudpSocketTransportConnection;

fn question(label: &str) -> PipelineDocument {
    PipelineDocument::Question(PipelineQuestion {
        campaign: slug(CAMPAIGN),
        label: label.into(),
        title: Title(format!("Question {label}")),
        question: format!("Question {label}?").as_str().into(),
        options: vec![
            QuestionOption { label: "A".into(), text: "the organ".into() },
            QuestionOption { label: "B".into(), text: "the client".into() },
        ],
        recommended: "A".into(),
        depends: vec![],
        raised_in: None,
        asked_on: Date("2026-09-16".into()),
    })
}

/// A ruling, answering a question when it names one.
fn ruling(label: &str, answers: Option<&str>) -> PipelineDocument {
    PipelineDocument::Ruling(PipelineRuling {
        campaign: slug(CAMPAIGN),
        label: label.into(),
        title: Title(format!("Ruling {label}")),
        answers: answers.map(|question| Short(id("question", question))),
        choice: answers.map(|_| "A".into()),
        ruling: format!("Ruling {label}.").as_str().into(),
        operator_quote: None,
        ruled_on: Date("2026-09-16".into()),
        precedents: vec![],
        authority: RulingAuthority::Operator,
    })
}

fn resolution(subject: PipelineKind, key: &str, outcome: ResolutionOutcome) -> PipelineDocument {
    PipelineDocument::Resolution(PipelineResolution {
        subject: PipelineRef { kind: subject, id: Short(key.into()) },
        sequence: 1,
        outcome,
        rationale: "Resolved.".into(),
        resolved_on: Date("2026-09-16".into()),
    })
}

fn id(kind: &str, local: &str) -> String {
    format!("{CAMPAIGN}:{kind}:{local}")
}

fn hit(kind: &str, key: String, score: f32) -> Hit {
    Hit { doc_id: key, kind: kind.into(), score }
}

/// Every way a document stops standing, and two that do: Q1 is answered (so
/// resolved), Q2 was answered and the answer withdrawn (its resolution is the
/// resolved one and the question stands again), R3 is superseded by R4.
fn shelf() -> Vec<Vec<PipelineDocument>> {
    vec![
        campaign_seed(),
        vec![question("Q1"), question("Q2")],
        vec![ruling("R1", Some("Q1"))],
        vec![ruling("R2", Some("Q2"))],
        vec![resolution(PipelineKind::Resolution, &id("resolution", "question.Q2.n1"), ResolutionOutcome::Withdrawn { reason: "moot".into() })],
        vec![ruling("R3", None), ruling("R4", None)],
        vec![resolution(
            PipelineKind::Ruling,
            &id("ruling", "R3"),
            ResolutionOutcome::Superseded { by: vec![PipelineRef { kind: PipelineKind::Ruling, id: Short(id("ruling", "R4")) }] },
        )],
    ]
}

fn standing() -> Selection {
    Selection {
        fields: Some(vec![FieldPredicate {
            index: "in_force".into(),
            op: "any_of".into(),
            values: Some(vec!["true".into()]),
            number: None,
        }]),
        ..Selection::default()
    }
}

fn semantic(selection: Selection, top_k: u32) -> HuginnMindRequest {
    semantic_of("who owns the state", selection, top_k)
}

fn semantic_of(text: &str, selection: Selection, top_k: u32) -> HuginnMindRequest {
    HuginnMindRequest::Query {
        instance: slug(INSTANCE),
        selection,
        semantic: Some(SemanticQuery { text: text.into(), top_k }),
    }
}

/// How many documents a plain query finds.
fn documents(client: &mut Client, message_id: &str) -> usize {
    ids(&ask(client, message_id, &plain())).len()
}

fn plain() -> HuginnMindRequest {
    HuginnMindRequest::Query { instance: slug(INSTANCE), selection: Selection::default(), semantic: None }
}

/// The ids on a page, in its order.
fn ids(response: &HuginnMindResponse) -> Vec<String> {
    let HuginnMindResponse::Query(page) = response else { panic!("expected a page, got {response:?}") };
    match &page.items {
        PipelinePageItems::Headers(headers) => headers.iter().map(|header| header.id.id.0.clone()).collect(),
        PipelinePageItems::Documents(views) => views.iter().map(|view| view.id.id.0.clone()).collect(),
    }
}

fn quick() -> Backoff {
    Backoff { initial: Duration::from_millis(1), max: Duration::from_millis(2), recheck: Duration::from_secs(3600), search_deadline: Duration::from_secs(30) }
}

/// A daemon serving a mind over a real socket, with the ports the test gave it.
struct Harness {
    endpoint: String,
    stopping: Arc<AtomicBool>,
    serving: Option<JoinHandle<Result<()>>>,
    embedder: FakeEmbedder,
    index: FakeIndex,
    _root: TempDir,
}

impl Harness {
    fn start(seed: Vec<Vec<PipelineDocument>>, embedder: FakeEmbedder, index: FakeIndex, search_timeout: Duration) -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut mind = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
        for documents in seed {
            let outcome = mind.admit(batch(INSTANCE, documents), now());
            assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
        }
        let sink = WorkerSink::spawn(
            embedder.clone(),
            index.clone(),
            &slug(INSTANCE),
            mind.genesis_receipt_id().unwrap(),
            mind.index_entries(None).unwrap(),
            Backoff { search_deadline: search_timeout, ..quick() },
        );
        let mut daemon: Daemon<OwnedRedbMessagePackBackingStore, WorkerSink> = Daemon::new(mind, sink);
        let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
        let endpoint = format!("rudp://{}", hub.local_addr().unwrap());
        let registry = schema_registry().unwrap();
        let stopping = Arc::new(AtomicBool::new(false));
        let loop_stopping = Arc::clone(&stopping);
        let options = ServeOptions::default();
        let serving = std::thread::spawn(move || run(&mut daemon, &mut hub, &registry, &loop_stopping, &options));
        Self { endpoint, stopping, serving: Some(serving), embedder, index, _root: root }
    }

    fn client(&self) -> Client {
        let mut client = CultMesh::create_rudp_client_for_endpoint(
            "semantic-test".to_string(),
            CULTNET_OPERATION_CONNECTION_ID,
            &self.endpoint,
            CultMeshRudpSocketOptions::default(),
        )
        .unwrap();
        client.connect(Vec::new()).unwrap();
        for _ in 0..1000 {
            let _ = client.receive_once();
            if client.connected() {
                return client;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("the client never connected");
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        // A loop that is stuck waiting on the index is a failure of the test,
        // not a reason to hang the run: give it a moment and leave it behind.
        if let Some(serving) = self.serving.take() {
            for _ in 0..1500 {
                if serving.is_finished() {
                    let _ = serving.join();
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

fn send(client: &mut Client, message_id: &str, request: &HuginnMindRequest) {
    client.send_schema_message(&encode_request(message_id, request, None).unwrap()).unwrap();
}

/// The reply to `message_id` on this client's session, or `None` if none came
/// in `patience` polls of two milliseconds.
fn receive(client: &mut Client, message_id: &str, patience: u32) -> Option<HuginnMindResponse> {
    for _ in 0..patience {
        client.poll_resends().unwrap();
        if let Some(reply) = client.receive_schema_message_once().unwrap() {
            let (correlated, response) = decode_response(&reply).unwrap();
            assert_eq!(correlated, message_id);
            return Some(response.expect("an answer, not an envelope failure"));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

fn ask(client: &mut Client, message_id: &str, request: &HuginnMindRequest) -> HuginnMindResponse {
    send(client, message_id, request);
    receive(client, message_id, 3000).unwrap_or_else(|| panic!("no reply to {message_id}"))
}

fn index_status(client: &mut Client, message_id: &str) -> IndexStatus {
    match ask(client, message_id, &HuginnMindRequest::Whoami) {
        HuginnMindResponse::Whoami(status) => status.index,
        other => panic!("expected a status, got {other:?}"),
    }
}

fn until_index(client: &mut Client, what: &str, predicate: impl Fn(&IndexStatus) -> bool) {
    for at in 0..2000 {
        if predicate(&index_status(client, &format!("status-{at}"))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the index never reported {what}");
}

fn healthy() -> Harness {
    let (embedder, index) = (FakeEmbedder::new("d1"), FakeIndex::default());
    Harness::start(shelf(), embedder, index, Duration::from_secs(30))
}

/// A semantic answer is the mind's: the index names a resolved question, a
/// withdrawn resolution, a superseded ruling and a document the mind does not
/// hold, all scoring above the two that stand, and only the two that stand come
/// back, in score order. The selection's other predicates still cut the
/// candidates, and the embedder saw the query behind its instruction while the
/// index was asked for an oversample.
#[test]
fn a_semantic_query_returns_only_documents_the_mind_holds_and_the_selection_admits() {
    let harness = healthy();
    let mut client = harness.client();
    until_index(&mut client, "Current", |status| *status == IndexStatus::Current);
    harness.index.state.lock().unwrap().hits = vec![
        hit("question", id("question", "Q1"), 0.95),
        hit("resolution", id("resolution", "question.Q2.n1"), 0.9),
        hit("ruling", id("ruling", "R3"), 0.85),
        hit("question", id("question", "Q99"), 0.8),
        hit("ruling", id("ruling", "R4"), 0.5),
        hit("question", id("question", "Q2"), 0.4),
    ];

    let standing_only = ask(&mut client, "m-1", &semantic(standing(), 5));
    assert_eq!(ids(&standing_only), vec![id("ruling", "R4"), id("question", "Q2")]);
    let HuginnMindResponse::Query(page) = &standing_only else { unreachable!() };
    assert_eq!((page.matched, page.next.as_ref()), (2, None));

    let rulings = Selection { schemas: Some(vec![PipelineKind::Ruling.type_id().into()]), ..standing() };
    assert_eq!(ids(&ask(&mut client, "m-2", &semantic(rulings, 5))), vec![id("ruling", "R4")], "the selection's schemas still apply");

    let everything = ids(&ask(&mut client, "m-3", &semantic(Selection::default(), 5)));
    assert!(everything.contains(&id("question", "Q1")) && !everything.contains(&id("question", "Q99")),
        "a hit the selection does not exclude is shown, and one the mind does not hold never is: {everything:?}");

    let embedded = harness.embedder.texts();
    assert!(embedded.iter().any(|text| text.starts_with("Instruct: ") && text.ends_with("\nQuery: who owns the state")), "{embedded:?}");
    let searches = harness.index.state.lock().unwrap().searches.clone();
    assert!(searches.iter().all(|(name, _, limit)| *name == collection_name(&slug(INSTANCE)) && *limit == 20), "{searches:?}");
}

/// A cursor cannot resume a ranked answer and a ranked answer has no order to
/// reverse: each is refused by name, and nothing is embedded or searched for it.
#[test]
fn a_cursor_or_a_reversal_with_a_semantic_query_is_refused_and_asks_the_index_nothing() {
    let harness = healthy();
    let mut client = harness.client();
    until_index(&mut client, "Current", |status| *status == IndexStatus::Current);
    let embedded = harness.embedder.texts().len();

    let with_cursor = Selection { cursor: Some("anything".into()), ..standing() };
    let refusal = ask(&mut client, "m-1", &semantic(with_cursor, 5));
    assert!(
        matches!(&refusal, HuginnMindResponse::Refused(MindRefusal::SelectionInvalid { field, .. }) if field == "cursor"),
        "{refusal:?}"
    );
    assert_eq!(harness.embedder.texts().len(), embedded);
    assert!(harness.index.state.lock().unwrap().searches.is_empty());

    let reversed = Selection { descending: true, ..standing() };
    let refusal = ask(&mut client, "m-2", &semantic(reversed, 5));
    assert!(
        matches!(&refusal, HuginnMindResponse::Refused(MindRefusal::SelectionInvalid { field, .. }) if field == "descending"),
        "{refusal:?}"
    );
    assert_eq!(harness.embedder.texts().len(), embedded);
    assert!(harness.index.state.lock().unwrap().searches.is_empty());
}

/// The loop never waits on the network. The embedder is shut, the worker is
/// inside it with one client's semantic query, and a second client's `whoami`
/// and plain query are answered at once; the first is answered when the gate
/// opens. The gate counts arrivals, so this observes where the work is and not
/// how long it took.
#[test]
fn another_session_is_answered_while_a_semantic_search_is_outstanding() {
    let gate = Gate::shut();
    gate.open();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let harness = Harness::start(shelf(), embedder, FakeIndex::default(), Duration::from_secs(30));
    let mut asker = harness.client();
    let mut second = harness.client();
    let mut other = harness.client();
    until_index(&mut other, "Current", |status| *status == IndexStatus::Current);
    let held = documents(&mut other, "m-count");
    harness.index.state.lock().unwrap().hits = vec![hit("ruling", id("ruling", "R4"), 0.5)];

    gate.shut_again();
    send(&mut asker, "m-ask", &semantic(standing(), 5));
    gate.wait_arrived(1);
    assert!(receive(&mut asker, "m-ask", 20).is_none(), "the question is not answered while its embedding is outstanding");
    // A second question waits behind the first, and each is answered to its own asker.
    send(&mut second, "m-ask-2", &semantic(Selection { schemas: Some(vec![PipelineKind::Question.type_id().into()]), ..Selection::default() }, 5));

    assert!(matches!(ask(&mut other, "m-who", &HuginnMindRequest::Whoami), HuginnMindResponse::Whoami(_)));
    assert_eq!(documents(&mut other, "m-plain"), held, "a plain query is answered too");

    gate.open();
    let answered = receive(&mut asker, "m-ask", 3000).expect("the search is answered once the embedder returns");
    assert_eq!(ids(&answered), vec![id("ruling", "R4")]);
    let second_answer = receive(&mut second, "m-ask-2", 3000).expect("the second search is answered too, on its own session");
    assert_eq!(ids(&second_answer), Vec::<String>::new(), "the index offered a ruling, which its selection of questions does not admit");
}

/// A search the index never finishes is answered at the deadline the client
/// was promised, typed and naming the wait, never as an empty page.
#[test]
fn a_search_the_index_never_finishes_is_answered_unavailable_at_the_deadline() {
    let gate = Gate::shut();
    gate.open();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let harness = Harness::start(shelf(), embedder, FakeIndex::default(), Duration::from_millis(150));
    let mut client = harness.client();
    until_index(&mut client, "Current", |status| *status == IndexStatus::Current);

    gate.shut_again();
    let answered = ask(&mut client, "m-late", &semantic(standing(), 5));
    let HuginnMindResponse::Refused(MindRefusal::Unavailable { detail }) = &answered else { panic!("{answered:?}") };
    assert!(detail.contains("did not finish"), "{detail}");
    gate.open();
    assert!(matches!(ask(&mut client, "m-after", &HuginnMindRequest::Whoami), HuginnMindResponse::Whoami(_)), "the late answer is dropped, not misdelivered");
}

/// An index that cannot answer is a typed `Unavailable` and never an empty
/// page, in each way it cannot: the embedder is down when the query arrives;
/// the worker is failing; the collection belongs to someone else. Plain
/// queries are unaffected throughout.
#[test]
fn an_index_that_cannot_answer_is_unavailable_and_plain_queries_still_work() {
    let unavailable = |response: HuginnMindResponse| match response {
        HuginnMindResponse::Refused(MindRefusal::Unavailable { detail }) => detail,
        other => panic!("expected Unavailable, got {other:?}"),
    };

    // The embedder goes down after the index was Current: the query itself fails.
    let harness = healthy();
    let mut client = harness.client();
    until_index(&mut client, "Current", |status| *status == IndexStatus::Current);
    let held = documents(&mut client, "m-count");
    harness.embedder.state.lock().unwrap().down = true;
    let detail = unavailable(ask(&mut client, "m-down", &semantic(standing(), 5)));
    assert!(detail.contains("the embedder is down"), "{detail}");
    assert_eq!(documents(&mut client, "m-plain"), held);

    // A write while it is down makes the worker Failing, and the next query is
    // refused without being asked of it.
    let admit = HuginnMindRequest::Admit(batch(INSTANCE, vec![question("Q7")]));
    assert!(matches!(ask(&mut client, "m-admit", &admit), HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })));
    until_index(&mut client, "Failing", |status| matches!(status, IndexStatus::Failing { .. }));
    let searches = harness.index.state.lock().unwrap().searches.len();
    let detail = unavailable(ask(&mut client, "m-failing", &semantic(standing(), 5)));
    assert!(detail.contains("failing") && detail.contains("the embedder is down"), "{detail}");
    assert_eq!(harness.index.state.lock().unwrap().searches.len(), searches);
    assert_eq!(documents(&mut client, "m-plain-2"), held + 1, "the admitted question is in the mind whatever the index says");

    // A collection that is another writer's is Refused, and so are queries.
    let (embedder, index) = (FakeEmbedder::new("d1"), FakeIndex::default());
    index.plant(&collection_name(&slug(INSTANCE)), Described::Unlabelled);
    let refused = Harness::start(shelf(), embedder, index, Duration::from_secs(30));
    let mut client = refused.client();
    until_index(&mut client, "Refused", |status| matches!(status, IndexStatus::Refused { .. }));
    let detail = unavailable(ask(&mut client, "m-refused", &semantic(standing(), 5)));
    assert!(detail.contains("refused") && detail.contains("no Huginn metadata"), "{detail}");
    assert_eq!(documents(&mut client, "m-plain-3"), held);
}

/// A search the client has stopped waiting for is not started. The embedder is
/// held with the first search inside it; a second search queues behind it and
/// times out, and when the embedder is released the worker embeds the first,
/// skips the second and embeds a third: the index is never asked for an answer
/// nobody is waiting for.
#[test]
fn a_search_that_timed_out_while_queued_is_never_embedded() {
    let gate = Gate::shut();
    gate.open();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let harness = Harness::start(shelf(), embedder, FakeIndex::default(), Duration::from_millis(300));
    let mut first = harness.client();
    let mut second = harness.client();
    let mut third = harness.client();
    until_index(&mut third, "Current", |status| *status == IndexStatus::Current);
    let queries = |harness: &Harness| -> Vec<String> {
        harness.embedder.texts().into_iter().filter(|text| text.starts_with("Instruct: ")).collect()
    };

    gate.shut_again();
    send(&mut first, "m-first", &semantic_of("first", standing(), 5));
    gate.wait_arrived(1);
    send(&mut second, "m-second", &semantic_of("second", standing(), 5));
    let timed_out = receive(&mut second, "m-second", 3000).expect("the second search is answered at its deadline");
    assert!(matches!(&timed_out, HuginnMindResponse::Refused(MindRefusal::Unavailable { detail }) if detail.contains("did not finish")), "{timed_out:?}");

    gate.open();
    send(&mut third, "m-third", &semantic_of("third", standing(), 5));
    let answered = receive(&mut third, "m-third", 3000).expect("the third search is answered");
    assert!(matches!(answered, HuginnMindResponse::Query(_)), "{answered:?}");
    let embedded = queries(&harness);
    assert_eq!(embedded.len(), 2, "the first and the third were embedded; the second was taken back before it started: {embedded:?}");
    assert!(embedded[0].ends_with("Query: first") && embedded[1].ends_with("Query: third"), "{embedded:?}");
}

/// A search asked while the index lags the mind is not refused: it waits behind
/// the writes ahead of it, through the real serve loop, and reads them. The
/// worker is inside its first batch with the rest of the mind still to write;
/// the reply does not come until the gate opens, and it comes with every
/// document already in the collection.
#[test]
fn a_search_asked_while_the_index_lags_waits_for_the_writes_ahead_of_it_and_reads_them() {
    let gate = Gate::shut();
    let embedder = FakeEmbedder::new("d1").behind(&gate);
    let harness = Harness::start(shelf(), embedder, FakeIndex::default(), Duration::from_secs(30));
    gate.wait_arrived(1);
    let mut client = harness.client();
    let IndexStatus::Behind { pending } = index_status(&mut client, "m-status") else { panic!("the worker is inside its first batch") };
    assert!(pending > 0);
    harness.index.state.lock().unwrap().hits = vec![hit("ruling", id("ruling", "R4"), 0.5)];

    send(&mut client, "m-behind", &semantic(standing(), 5));
    assert!(receive(&mut client, "m-behind", 100).is_none(), "the search waits: it is neither refused nor answered from a half-filled collection");

    gate.open();
    let answered = receive(&mut client, "m-behind", 3000).expect("the search is answered once the index has caught up");
    assert_eq!(ids(&answered), vec![id("ruling", "R4")]);
    until_index(&mut client, "Current", |status| *status == IndexStatus::Current);
    let state = harness.index.state.lock().unwrap();
    let written = state.collections[&collection_name(&slug(INSTANCE))].points.len();
    assert_eq!(state.held_when_searched, [written], "the search saw every document");
    assert!(written >= pending as usize);
}
