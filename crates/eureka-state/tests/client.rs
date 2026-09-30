//! The client against a real daemon, in-process on a loopback port over a
//! temporary mind, and against scripted servers where the daemon cannot be
//! made to misbehave.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::Utc;
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, CultNetMessage, CultNetWireContract, Selection,
    answer_content_chunk_request, encode_cultnet_message_to_vec, pack_content,
};
use eureka_state::{ClientError, HuginnClient, MAX_REQUEST_BYTES};
use huginn_daemon::serve::MAX_RESPONSE_BYTES;
use huginn_mind::envelope::{OperationFailure, encode_failure, encode_request, encode_response};
use huginn_mind::epiphany_pipeline::{
    AuthorityMap, CodeLocation, CutDelete, CutVerification, FileChange, Line, NegativeCheck, OrgRepo,
    PipelineCutSpec, PipelineDocument, PipelineKind, PipelineRef, Short, StructuralDelta, Title,
    VerificationTest,
};
use huginn_mind::{DeferredAnswer, HuginnMindRequest, HuginnMindResponse, MindRefusal, PipelineAdmissionOutcome};

mod common;
use common::*;

const OTHER: &str = "thought-cage";
const TIMEOUT: Duration = Duration::from_secs(20);

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

/// serves its chunks faithfully.
fn deferring(body: Vec<u8>) -> Server {
    deferring_in_chunks(body, 256 * 1024, Duration::ZERO)
}

/// `deferring`, with `chunk_bytes` per chunk and each chunk answered only after
/// `drip`.
fn deferring_in_chunks(body: Vec<u8>, chunk_bytes: usize, drip: Duration) -> Server {
    let (manifest, chunks) = pack_content("art", "package", "", "", "", &body, chunk_bytes).expect("a body packs");
    let deferred = encode_response(
        "eureka-state-call",
        "whoami",
        &HuginnMindResponse::Deferred(DeferredAnswer { manifest }),
        "scripted",
    )
    .unwrap();
    scripted(move |message| match message {
        CultNetMessage::OperationRequest { .. } => deferred.clone(),
        chunk => {
            std::thread::sleep(drip);
            answer_content_chunk_request(chunk, |hash| {
                chunks.iter().find(|c| c.chunk_hash == hash).map(|c| c.payload.as_slice())
            })
        }
    })
}

fn client(addr: SocketAddr) -> HuginnClient {
    HuginnClient::new(addr, slug(INSTANCE), TIMEOUT)
}

fn unavailable(error: ClientError) -> (SocketAddr, String) {
    match error {
        ClientError::Unavailable { endpoint, detail } => (endpoint, detail),
        other @ (ClientError::Rejected { .. } | ClientError::TooLarge { .. }) => panic!("expected Unavailable, got {other}"),
    }
}

fn rejected(error: ClientError) -> (SocketAddr, String, String) {
    match error {
        ClientError::Rejected { endpoint, code, detail } => (endpoint, code, detail),
        other @ (ClientError::Unavailable { .. } | ClientError::TooLarge { .. }) => panic!("expected Rejected, got {other}"),
    }
}

fn nothing_found(message_id: &str, operation: &str) -> CultNetMessage {
    encode_response(message_id, operation, &HuginnMindResponse::View(None), "scripted").unwrap()
}

/// Wait for a scripted server to have seen `count` sessions end.
fn ended_within(server: &Server, count: usize, wait: Duration) -> bool {
    let until = Instant::now() + wait;
    while server.ended.load(Ordering::Relaxed) < count {
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
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
    assert_eq!(client.instance(), &slug(INSTANCE), "the client reports the instance it was configured for");

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
        assert!(detail.contains("no accept") && detail.contains("deadline"), "{detail}");
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

/// An envelope failure means the request never reached a mind. It is the
/// daemon refusing, not a transport failure, so a caller that retries on
/// `Unavailable` does not loop on it; the code and the message arrive intact.
#[test]
fn an_envelope_failure_is_rejected_and_carries_its_code() {
    let server = answering(encode_failure(
        "eureka-state-call",
        "whoami",
        &OperationFailure { code: "wrong-service".into(), message: "not huginn.mind".into() },
        "scripted",
    ));
    let error = client(server.addr).call(HuginnMindRequest::Whoami).unwrap_err();
    assert!(error.to_string().contains("wrong-service"), "{error}");
    let (endpoint, code, detail) = rejected(error);
    assert_eq!((endpoint, code.as_str(), detail.as_str()), (server.addr, "wrong-service", "not huginn.mind"));
}

/// An answer to another request, or to another operation, is not this call's
/// answer, whatever it says.
#[test]
fn an_answer_that_is_not_to_this_request_is_refused() {
    let foreign = answering(nothing_found("someone-elses-call", "whoami"));
    let (_, detail) = unavailable(client(foreign.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("someone-elses-call"), "{detail}");

    let other_operation = answering(nothing_found("eureka-state-call", "query"));
    let (_, detail) = unavailable(client(other_operation.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(detail.contains("query") && detail.contains("whoami"), "{detail}");

    let matching = answering(nothing_found("eureka-state-call", "whoami"));
    assert_eq!(client(matching.addr).call(HuginnMindRequest::Whoami).unwrap(), HuginnMindResponse::View(None));
}

/// The timeout bounds the whole call. Seven chunks each answered after 300 ms
/// take two seconds; no single wait exceeds the 500 ms timeout, and the call
/// still gives up at it.
#[test]
fn the_timeout_bounds_the_whole_call_not_each_wait() {
    let server = deferring_in_chunks(vec![7_u8; 7 * 1024], 1024, Duration::from_millis(300));
    let timeout = Duration::from_millis(500);
    let started = Instant::now();
    let error = HuginnClient::new(server.addr, slug(INSTANCE), timeout).call(HuginnMindRequest::Whoami).unwrap_err();
    let elapsed = started.elapsed();
    assert!(elapsed >= timeout && elapsed < Duration::from_millis(1100), "{elapsed:?}");
    let (_, detail) = unavailable(error);
    assert!(detail.contains("deadline"), "{detail}");
}

/// Every call ends its session, whether it answered, was rejected, or gave up.
#[test]
fn every_call_disconnects_its_session() {
    let answered = answering(nothing_found("eureka-state-call", "whoami"));
    client(answered.addr).call(HuginnMindRequest::Whoami).unwrap();
    assert!(ended_within(&answered, 1, Duration::from_secs(5)), "an answered call left its session open");

    let refused = answering(encode_failure(
        "eureka-state-call",
        "whoami",
        &OperationFailure { code: "wrong-service".into(), message: "no".into() },
        "scripted",
    ));
    rejected(client(refused.addr).call(HuginnMindRequest::Whoami).unwrap_err());
    assert!(ended_within(&refused, 1, Duration::from_secs(5)), "a rejected call left its session open");

    let silent = scripted(|_| {
        std::thread::sleep(Duration::from_millis(800));
        CultNetMessage::Error { error: "late".into(), code: None, details: None }
    });
    let error = HuginnClient::new(silent.addr, slug(INSTANCE), Duration::from_millis(300))
        .call(HuginnMindRequest::Whoami)
        .unwrap_err();
    unavailable(error);
    assert!(ended_within(&silent, 1, Duration::from_secs(5)), "a timed-out call left its session open");
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

/// An admission whose session is `session_pad` characters long: the knob that
/// sizes a request without changing what it asks.
fn padded_admit(session_pad: usize) -> HuginnMindRequest {
    let mut request = batch(INSTANCE, vec![stewardship()]);
    request.provenance.session = Short("s".repeat(session_pad));
    HuginnMindRequest::Admit(request)
}

/// What one send has to carry for `message`, measured the way the client does.
fn encoded_bytes(message: &CultNetMessage) -> usize {
    encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap().len()
}

fn message_for(request: &HuginnMindRequest, source: Option<String>) -> CultNetMessage {
    encode_request("eureka-state-call", request, source).unwrap()
}

/// The smallest session padding whose request encodes to at least `target`
/// bytes. The payload rides as base64, so request sizes step by four and only
/// every fourth size is reachable through the request alone.
fn pad_for(target: usize) -> usize {
    let (mut low, mut high) = (0, target);
    while low < high {
        let middle = (low + high) / 2;
        if encoded_bytes(&message_for(&padded_admit(middle), None)) < target {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// The largest request one send carries, and the smallest it does not.
fn largest_request_that_fits() -> HuginnMindRequest {
    padded_admit(pad_for(MAX_REQUEST_BYTES + 1) - 1)
}

fn smallest_request_over() -> HuginnMindRequest {
    padded_admit(pad_for(MAX_REQUEST_BYTES + 1))
}

/// A message that encodes to exactly `target` bytes: the largest request that
/// fits under it, topped up with a source runtime id, which rides outside the
/// payload and moves the size a byte at a time.
fn message_of_size(target: usize) -> CultNetMessage {
    let request = padded_admit(pad_for(target + 1) - 1);
    for pad in 0..16 {
        let message = message_for(&request, (pad > 0).then(|| "r".repeat(pad)));
        if encoded_bytes(&message) == target {
            return message;
        }
    }
    panic!("no message encodes to {target} bytes");
}

fn admitted_answer() -> CultNetMessage {
    encode_response(
        "eureka-state-call",
        "admit",
        &HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: "r".into() }),
        "scripted",
    )
    .unwrap()
}

/// A request larger than one send carries is refused by the client before it
/// connects to anything, as its own error and not as a transport failure the
/// caller would retry; the largest request that fits is sent, and answered.
#[test]
fn a_request_over_what_one_send_carries_is_too_large_and_one_that_fits_is_sent() {
    let closed = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let over = smallest_request_over();
    let started = Instant::now();
    match client(closed).call(over.clone()) {
        Err(ClientError::TooLarge { bytes, limit }) => {
            assert_eq!(bytes, encoded_bytes(&message_for(&over, None)));
            assert_eq!(limit, MAX_REQUEST_BYTES);
            assert!(bytes > limit);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(1), "refused only after trying the transport");

    let fits = largest_request_that_fits();
    assert!(MAX_REQUEST_BYTES - encoded_bytes(&message_for(&fits, None)) < 4, "not the largest that fits");
    let server = answering(admitted_answer());
    let answer = client(server.addr).call(fits);
    assert!(matches!(answer, Ok(HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { .. }))), "{answer:?}");
}

/// The transport's own refusal of an oversize send ("Message too long") is
/// what an unchecked request used to report, as an `Unavailable` daemon a
/// caller would retry forever.
#[test]
fn an_oversize_admission_is_never_reported_as_an_unavailable_daemon() {
    let server = answering(admitted_answer());
    let error = client(server.addr).call(padded_admit(70_000)).unwrap_err();
    assert!(matches!(error, ClientError::TooLarge { .. }), "{error}");
}

/// The limit is the transport's own: a message of `MAX_REQUEST_BYTES` goes out
/// on an RUDP session, and one byte more is refused by the socket.
#[test]
fn the_limit_is_the_largest_message_the_transport_sends() {
    let server = answering(admitted_answer());
    let mut session = CultMesh::create_rudp_client_for_endpoint(
        "limit-probe".to_string(),
        CULTNET_OPERATION_CONNECTION_ID,
        &format!("rudp://{}", server.addr),
        CultMeshRudpSocketOptions::default(),
    )
    .unwrap();
    session.connect(Vec::new()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !session.connected() {
        session.poll_resends().unwrap();
        let _ = session.receive_once().unwrap();
        assert!(Instant::now() < until, "the server never accepted");
        std::thread::sleep(Duration::from_millis(2));
    }
    session.send_schema_message(&message_of_size(MAX_REQUEST_BYTES)).expect("the limit is carried");
    assert!(session.send_schema_message(&message_of_size(MAX_REQUEST_BYTES + 1)).is_err(), "one byte more is sent");
}
