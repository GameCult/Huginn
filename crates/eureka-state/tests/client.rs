//! The client against a real daemon, in-process on a loopback port over a
//! temporary mind, and against scripted servers where the daemon cannot be
//! made to misbehave.

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::Utc;
use cultnet_rs::{
    CultNetMessage, CultNetRudpServerEvent, CultNetWireContract, Selection, answer_content_chunk_request,
    decode_cultnet_message_from_slice, encode_cultnet_message_to_vec, pack_content,
};
use eureka_state::{ClientError, HuginnClient};
use huginn_daemon::serve::{MAX_RESPONSE_BYTES, bind, run, schema_registry};
use huginn_daemon::{Daemon, Handled, Hits, IndexSink, SearchTicket, ServeOptions};
use huginn_mind::envelope::{OperationFailure, encode_failure, encode_response};
use huginn_mind::epiphany_pipeline::{
    AuthorityMap, CodeLocation, CutDelete, CutVerification, Date, DocRef, FileChange, Line, NegativeCheck, OrgRepo,
    PipelineCampaign, PipelineCutSpec, PipelineDocument, PipelineInstance, PipelineKind, PipelineRef,
    PipelineStewardship, Sha, Short, Slug, StructuralDelta, Title, VerificationTest,
};
use huginn_mind::{
    DeferredAnswer, Faculty, HuginnMindRequest, HuginnMindResponse, IndexStatus, Mind, MindRefusal, MindStore,
    OwnedRedbMessagePackBackingStore, PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelineProvenance,
};
use tempfile::TempDir;

const INSTANCE: &str = "eureka";
const OTHER: &str = "thought-cage";
const CAMPAIGN: &str = "eureka-state";
const REPO: &str = "GameCult/Epiphany";
const TIMEOUT: Duration = Duration::from_secs(20);

fn slug(value: &str) -> Slug {
    Slug(value.into())
}

fn sha() -> Sha {
    Sha("5f98228d".into())
}

fn identity(name: &str) -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: slug(name),
        display_name: Short(format!("{name} mind")),
        created_at: Date("2026-09-29".into()),
        host: Short(name.into()),
    })
}

fn stewardship() -> PipelineDocument {
    PipelineDocument::Stewardship(PipelineStewardship {
        instance: slug(INSTANCE),
        repo: OrgRepo(REPO.into()),
        sequence: 1,
        assigned_on: Date("2026-09-29".into()),
        note: "assigned".into(),
    })
}

fn campaign() -> PipelineDocument {
    PipelineDocument::Campaign(PipelineCampaign {
        slug: slug(CAMPAIGN),
        title: Title("Eureka pipeline state".into()),
        repos: vec![OrgRepo(REPO.into())],
        working_branch: Short("hands/cut13a".into()),
        target_doc: DocRef { path: Short("notes/target.md".into()), start_line: 1, end_line: 9, commit: sha() },
    })
}

fn batch(instance: &str, documents: Vec<PipelineDocument>) -> PipelineAdmissionBatch {
    PipelineAdmissionBatch {
        instance: slug(instance),
        provenance: PipelineProvenance {
            faculty: Faculty::Hands,
            agent: Short("claude".into()),
            session: Short("session-1".into()),
            tool: Short("admit".into()),
        },
        documents,
    }
}

/// A `Line` and a `Short` at the leaf's own bound.
fn line(n: usize) -> Line {
    Line(format!("{n:0>1000}"))
}

fn short(n: usize) -> Short {
    Short(format!("{n:0>200}"))
}

/// A cut spec with every list the leaf bounds filled: at 256 file changes a
/// megabyte of field content, more than one send carries.
fn cut_spec(cut: &str, file_changes: usize) -> PipelineDocument {
    let lines = |count: usize| (0..count).map(line).collect::<Vec<_>>();
    let shorts = |count: usize| (0..count).map(short).collect::<Vec<_>>();
    PipelineDocument::CutSpec(PipelineCutSpec {
        campaign: slug(CAMPAIGN),
        cut: cut.into(),
        revision: 1,
        title: Title(short(0).0),
        repo: OrgRepo(REPO.into()),
        branch: short(1),
        base: sha(),
        depends_on: vec![],
        first: lines(16),
        deletes: (0..64).map(|n| CutDelete { path: short(n), lines: 9, note: line(n) }).collect(),
        keeps_moves: lines(64),
        adds: lines(64),
        file_changes: (0..file_changes)
            .map(|n| FileChange {
                location: CodeLocation { path: short(n), line: 1, end_line: Some(9) },
                change: line(n),
            })
            .collect(),
        authority_map: Some(AuthorityMap {
            owner: line(0),
            inputs: lines(16),
            outputs: lines(16),
            derived_state: lines(16),
            forbidden_writers: lines(16),
            shared_paths: lines(16),
            deletion_line: line(1),
        }),
        verification: CutVerification {
            builds: lines(64),
            tests: (0..64).map(|n| VerificationTest { name: short(n), pins: line(n) }).collect(),
            negative: (0..64).map(|n| NegativeCheck { pattern: short(n), scope: line(n) }).collect(),
            operator: lines(64),
        },
        estimate: StructuralDelta {
            lines_added: 0,
            lines_removed: 0,
            dependencies_added: shorts(64),
            dependencies_removed: shorts(64),
            formats_added: shorts(64),
            formats_removed: shorts(64),
            targets_added: shorts(64),
            targets_removed: shorts(64),
        },
        rulings: vec![],
        questions: vec![],
    })
}

fn cut_ref(cut: &str) -> PipelineRef {
    PipelineRef { kind: PipelineKind::CutSpec, id: Short(format!("{CAMPAIGN}:cut_spec:cut-{cut}.r1")) }
}

/// No index: the client's subject is the transport, not the projection.
struct Inert;

impl<S: MindStore> IndexSink<S> for Inert {
    fn committed(&mut self, _mind: &Mind<S>, _writes: &[PipelineRef]) -> anyhow::Result<()> {
        Ok(())
    }
    fn status(&self) -> IndexStatus {
        IndexStatus::Current
    }
    fn search(&mut self, _text: &str, _top_k: u32) -> Result<SearchTicket, String> {
        Err("this index cannot search".into())
    }
    fn search_deadline(&self) -> Duration {
        Duration::ZERO
    }
    fn searched(&mut self) -> Vec<(SearchTicket, Result<Hits, String>)> {
        Vec::new()
    }
    fn abandon(&mut self, _ticket: SearchTicket) {}
}

type TestDaemon = Daemon<OwnedRedbMessagePackBackingStore, Inert>;

fn answered(handled: Handled) -> HuginnMindResponse {
    match handled {
        Handled::Answered(response) => response,
        Handled::Searching(_) => panic!("the inert index cannot search"),
    }
}

/// A daemon over a temporary mind whose identity is admitted, and whatever
/// else `seed` admits.
fn mind(seed: Vec<Vec<PipelineDocument>>) -> (TempDir, TestDaemon) {
    let root = tempfile::tempdir().unwrap();
    let mut daemon = Daemon::new(Mind::open(root.path(), &slug(INSTANCE)).unwrap(), Inert);
    for documents in std::iter::once(vec![identity(INSTANCE)]).chain(seed) {
        let outcome = answered(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), Utc::now()));
        assert!(matches!(outcome, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })), "{outcome:?}");
    }
    (root, daemon)
}

/// A server on a loopback port that stops when dropped.
struct Server {
    addr: SocketAddr,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    _root: Option<TempDir>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(root: TempDir, mut daemon: TestDaemon) -> Server {
    let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
    let addr = hub.local_addr().unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let loop_stopping = Arc::clone(&stopping);
    let registry = schema_registry().unwrap();
    let thread = std::thread::spawn(move || {
        run(&mut daemon, &mut hub, &registry, &loop_stopping, &ServeOptions::default()).unwrap();
    });
    Server { addr, stopping, thread: Some(thread), _root: Some(root) }
}

/// A server that answers each schema message with whatever `script` says.
fn scripted(mut script: impl FnMut(&CultNetMessage) -> CultNetMessage + Send + 'static) -> Server {
    let mut hub = bind("127.0.0.1:0".parse().unwrap(), "scripted").unwrap();
    let addr = hub.local_addr().unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let loop_stopping = Arc::clone(&stopping);
    let thread = std::thread::spawn(move || {
        while !loop_stopping.load(Ordering::Relaxed) {
            hub.poll_resends().unwrap();
            while let Some(event) = hub.receive_event_once().unwrap() {
                let CultNetRudpServerEvent::Frame { session, frame } = event else { continue };
                let message =
                    decode_cultnet_message_from_slice(&frame.payload, CultNetWireContract::CultNetSchemaV0).unwrap();
                hub.send_schema_message(&session, &script(&message)).unwrap();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    Server { addr, stopping, thread: Some(thread), _root: None }
}

/// A scripted server that answers any operation with a deferral of `body` and
/// serves its chunks faithfully.
fn deferring(body: Vec<u8>) -> Server {
    let (manifest, chunks) = pack_content("art", "package", "", "", "", &body, 256 * 1024).expect("a body packs");
    let deferred = encode_response(
        "eureka-state-call",
        "whoami",
        &HuginnMindResponse::Deferred(DeferredAnswer { manifest }),
        "scripted",
    )
    .unwrap();
    scripted(move |message| match message {
        CultNetMessage::OperationRequest { .. } => deferred.clone(),
        chunk => answer_content_chunk_request(chunk, |hash| {
            chunks.iter().find(|c| c.chunk_hash == hash).map(|c| c.payload.as_slice())
        }),
    })
}

fn client(addr: SocketAddr) -> HuginnClient {
    HuginnClient::new(addr, slug(INSTANCE), TIMEOUT)
}

fn unavailable(error: ClientError) -> (SocketAddr, String) {
    let ClientError::Unavailable { endpoint, detail } = error;
    (endpoint, detail)
}

fn document_query(instance: &str) -> HuginnMindRequest {
    HuginnMindRequest::Query {
        instance: slug(instance),
        selection: Selection { projection: "document".into(), ..Selection::default() },
        semantic: None,
    }
}

/// Every operation the wire has, asked of a live daemon over the socket, and
/// each answered with the mind's own answer.
#[test]
fn every_operation_round_trips_against_a_live_daemon() {
    let (root, daemon) = mind(vec![]);
    let server = serve(root, daemon);
    let client = client(server.addr);

    let HuginnMindResponse::Whoami(status) = client.call(HuginnMindRequest::Whoami).unwrap() else {
        panic!("whoami is answered with a status");
    };
    assert_eq!((status.instance, status.documents), (slug(INSTANCE), 1));

    let admitted = client.call(HuginnMindRequest::Admit(batch(INSTANCE, vec![stewardship(), campaign()]))).unwrap();
    let HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { writes, .. }) = admitted else {
        panic!("the batch is committed, got {admitted:?}");
    };
    assert_eq!(writes.len(), 2);

    let HuginnMindResponse::Query(page) = client.call(document_query(INSTANCE)).unwrap() else {
        panic!("a query is answered with a page");
    };
    assert_eq!(page.matched, 3, "the identity and the two admitted documents");

    let campaign_ref = PipelineRef { kind: PipelineKind::Campaign, id: Short(format!("{CAMPAIGN}:campaign:self")) };
    let viewed = client.call(HuginnMindRequest::View { instance: slug(INSTANCE), id: campaign_ref.clone() }).unwrap();
    let HuginnMindResponse::View(Some(view)) = viewed else { panic!("the campaign is viewable, got {viewed:?}") };
    assert_eq!(view.id, campaign_ref);
}

/// A refusal is data. A read that names another mind, a write that declares
/// another mind, and a document the mind does not hold each come back as `Ok`.
#[test]
fn a_foreign_instance_is_an_answer_not_an_error() {
    let (root, daemon) = mind(vec![]);
    let server = serve(root, daemon);
    let client = client(server.addr);
    let foreign = MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() };

    assert_eq!(client.call(document_query(OTHER)).unwrap(), HuginnMindResponse::Refused(foreign.clone()));
    assert_eq!(
        client.call(HuginnMindRequest::Admit(batch(OTHER, vec![identity(OTHER)]))).unwrap(),
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(foreign)),
    );
    let absent = PipelineRef { kind: PipelineKind::CutSpec, id: Short(format!("{CAMPAIGN}:cut_spec:cut-none.r1")) };
    assert_eq!(
        client.call(HuginnMindRequest::View { instance: slug(INSTANCE), id: absent }).unwrap(),
        HuginnMindResponse::View(None),
    );
}

/// The wide cut spec's answer is over one send and arrives deferred; the
/// narrow one arrives whole. Both come back equal to what the mind gave.
#[test]
fn a_deferred_answer_resolves_to_the_same_page_as_a_direct_one() {
    let (root, mut daemon) =
        mind(vec![vec![stewardship(), campaign()], vec![cut_spec("wide", 256), cut_spec("fits", 180)]]);
    let measured = |request: &HuginnMindRequest, daemon: &mut TestDaemon| {
        let answer = answered(daemon.handle(request.clone(), Utc::now()));
        let envelope = encode_response("m", request.operation(), &answer, "huginn-eureka").unwrap();
        let size = encode_cultnet_message_to_vec(&envelope, CultNetWireContract::CultNetSchemaV0).unwrap().len();
        (answer, size as u64)
    };
    let wide = HuginnMindRequest::View { instance: slug(INSTANCE), id: cut_ref("wide") };
    let fits = HuginnMindRequest::View { instance: slug(INSTANCE), id: cut_ref("fits") };
    let (wide_answer, wide_size) = measured(&wide, &mut daemon);
    let (fits_answer, fits_size) = measured(&fits, &mut daemon);
    assert!(matches!(wide_answer, HuginnMindResponse::View(Some(_))), "{wide_answer:?}");
    assert!(wide_size > MAX_RESPONSE_BYTES, "the wide answer must be over one send, or nothing was deferred");
    assert!(fits_size <= MAX_RESPONSE_BYTES, "the narrow answer must fit one send");

    let server = serve(root, daemon);
    let client = client(server.addr);
    assert_eq!(client.call(wide).unwrap(), wide_answer);
    assert_eq!(client.call(fits).unwrap(), fits_answer);
}

/// Nothing listens on the port. The call fails naming the endpoint, and it
/// fails when the configured timeout says: two timeouts, two waits.
#[test]
fn a_closed_port_is_unavailable_naming_the_endpoint_within_the_timeout() {
    let closed = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let mut waited = Vec::new();
    for timeout in [Duration::from_millis(300), Duration::from_millis(1200)] {
        let started = Instant::now();
        let error = HuginnClient::new(closed, slug(INSTANCE), timeout).call(HuginnMindRequest::Whoami).unwrap_err();
        let elapsed = started.elapsed();
        assert!(elapsed >= timeout, "gave up after {elapsed:?}, before its {timeout:?}");
        assert!(elapsed < timeout + Duration::from_millis(800), "gave up after {elapsed:?}, long past its {timeout:?}");
        assert!(error.to_string().contains(&closed.to_string()), "{error}");
        let (endpoint, detail) = unavailable(error);
        assert_eq!(endpoint, closed);
        assert!(detail.contains("no accept"), "{detail}");
        waited.push(elapsed);
    }
    assert!(waited[1] > waited[0] + Duration::from_millis(500), "{waited:?}");
}

/// A daemon that accepts and then says nothing is a timeout on the answer,
/// not on the connection.
#[test]
fn a_daemon_that_never_answers_is_unavailable_within_the_timeout() {
    let server = scripted(|_| {
        std::thread::sleep(Duration::from_secs(3));
        CultNetMessage::Error { error: "late".into(), code: None, details: None }
    });
    let started = Instant::now();
    let error = HuginnClient::new(server.addr, slug(INSTANCE), Duration::from_millis(400))
        .call(HuginnMindRequest::Whoami)
        .unwrap_err();
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(400) && elapsed < Duration::from_secs(2), "{elapsed:?}");
    let (endpoint, detail) = unavailable(error);
    assert_eq!(endpoint, server.addr);
    assert!(detail.contains("no answer"), "{detail}");
}

/// An envelope failure means the request never reached a mind; it is not an
/// answer, so the caller sees the code.
#[test]
fn an_envelope_failure_is_unavailable_and_carries_its_code() {
    let server = scripted(|_| {
        encode_failure(
            "eureka-state-call",
            "whoami",
            &OperationFailure { code: "wrong-service".into(), message: "not huginn.mind".into() },
            "scripted",
        )
    });
    let (_, detail) = unavailable(client(server.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("wrong-service"), "{detail}");
}

/// A deferred body that is not a response, and one that is another deferral,
/// are protocol violations named as such.
#[test]
fn a_deferred_body_must_be_a_response_and_not_another_deferral() {
    let garbage = deferring(vec![0xc1; 40_000]);
    let (_, detail) = unavailable(client(garbage.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("not a response"), "{detail}");

    let (inner, _) = pack_content("inner", "package", "", "", "", b"x", 16).unwrap();
    let again = rmp_serde::to_vec_named(&HuginnMindResponse::Deferred(DeferredAnswer { manifest: inner })).unwrap();
    let nested = deferring(again);
    let (_, detail) = unavailable(client(nested.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("itself deferred"), "{detail}");
}

/// A body over the deferral cap is refused before its first chunk is asked
/// for, by the number the daemon defers under.
#[test]
fn a_body_over_the_deferral_cap_is_refused() {
    let over = huginn_mind::MAX_DEFERRED_BODY_BYTES as usize + 1;
    let server = deferring(vec![0_u8; over]);
    let (_, detail) = unavailable(client(server.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("exceeds"), "{detail}");
}
