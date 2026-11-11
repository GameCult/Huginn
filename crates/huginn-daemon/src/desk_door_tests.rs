//! The desk door's proofs: the peer check and the framing at the unit seam
//! (a counting stream), the door and the loop over a real unix socket in a
//! tempdir (pipeline smokes), and the startup gates. The Mind's own rules are
//! `huginn-mind`'s tests; what is pinned here is who is read, what is bounded
//! and which Mind method answers.

use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, CultNetMessage, CultNetWireContract,
    decode_cultnet_message_from_slice, encode_cultnet_message_to_vec, encode_frame,
};
use huginn_mind::desk_wire::{
    DESK_MAX_RESPONSE_BYTES, DESK_REQUEST_SCHEMA, DESK_SERVICE_ID, DeskRequest, DeskResponse, ForkAnswer, ForkAnswerCode, ForkAnswerRequest, ForkCursor,
    ForkEntry, ForkLimit, ForkList, ListStatus, RequestSchema,
};
use huginn_mind::envelope::{OperationFailure, decode_desk_response, decode_response, encode_desk_request, encode_request};
use huginn_mind::eureka_pipeline::{DeskAuthority, PipelineDocument, Short};
use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse, MIND_REQUEST_SCHEMA, MIND_SERVICE_ID, MindStatus};
use huginn_mind::{MindRefusal, OwnedRedbMessagePackBackingStore, options_hash};
use tempfile::TempDir;

use crate::daemon::tests::{INSTANCE, OTHER, RecordingIndex, batch, campaign_seed, now, question_and_ruling, slug};
use crate::daemon::Daemon;
use crate::desk_door::{DESK_CONNECTION_DEADLINE, DeskDoor, Served, answer_desk_message, bind_desk, desk_registry, serve_connection};
use crate::index::Backoff;
use crate::serve::{Options, ServeOptions, bind, parse_options, run, schema_registry, startup};

type Store = OwnedRedbMessagePackBackingStore;
const FORK: &str = "eureka-state:question:Q1";

/// The uid this process runs as: the owner of a file it just made.
fn my_uid() -> u32 {
    let dir = tempfile::tempdir().unwrap();
    std::fs::metadata(dir.path()).unwrap().uid()
}

fn short(value: &str) -> Short {
    Short(value.into())
}

/// A daemon over a fresh mind holding the campaign and one answerable question, and that
/// question's current `options_hash`. The recording index forgets the seeding.
fn seeded(root: &Path) -> (Daemon<Store, RecordingIndex>, String) {
    let mind = huginn_mind::Mind::open(root, &slug(INSTANCE)).unwrap();
    let index = RecordingIndex::default();
    let mut daemon = Daemon::new(mind, index.clone());
    let (question, _) = question_and_ruling();
    let PipelineDocument::Question(held) = &question else { unreachable!() };
    let hash = options_hash(held).0;
    for documents in [campaign_seed(), vec![question]] {
        let admitted = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), now()).answered();
        assert!(matches!(admitted, HuginnMindResponse::Admit(huginn_mind::PipelineAdmissionOutcome::Committed { .. })), "{admitted:?}");
    }
    index.seen.lock().unwrap().clear();
    (daemon, hash)
}

fn state(daemon: &mut Daemon<Store, RecordingIndex>) -> (u32, u32) {
    match daemon.handle(HuginnMindRequest::Whoami, now()).answered() {
        HuginnMindResponse::Whoami(MindStatus { documents, receipts, .. }) => (documents, receipts),
        other => panic!("expected a status, got {other:?}"),
    }
}

fn wire(hash: &str, request_id: &str) -> ForkAnswerRequest {
    ForkAnswerRequest {
        schema_version: RequestSchema::V0,
        fork_id: FORK.into(),
        options_hash: hash.into(),
        answer: Some(ForkAnswer { choice: Some("A".into()), note: None }),
        retracted: false,
        retracts: None,
        answered_at: "2026-09-16T11:00:00Z".into(),
        request_id: request_id.into(),
    }
}

const R1: &str = "00000000-0000-4000-8000-000000000001";
const R2: &str = "00000000-0000-4000-8000-000000000002";
const R3: &str = "00000000-0000-4000-8000-000000000003";

fn answer(who: &str, authority: DeskAuthority, deciders: &[&str], request: ForkAnswerRequest) -> DeskRequest {
    DeskRequest::Answer { mind: slug(INSTANCE), subject: short(who), authority, deciders: deciders.iter().map(|d| short(d)).collect(), request }
}

/// A list read by `who` against `deciders`.
fn forks(who: &str, deciders: &[&str], status: Option<ListStatus>, cursor: Option<ForkCursor>, limit: u8) -> DeskRequest {
    DeskRequest::Forks {
        mind: slug(INSTANCE),
        subject: short(who),
        deciders: deciders.iter().map(|d| short(d)).collect(),
        status,
        cursor,
        limit: ForkLimit::new(limit).unwrap(),
    }
}

fn forks_all(deciders: &[&str]) -> DeskRequest {
    forks("x", deciders, None, None, 50)
}

fn fork_read(who: &str, deciders: &[&str], fork_id: &str) -> DeskRequest {
    DeskRequest::Fork { mind: slug(INSTANCE), subject: short(who), deciders: deciders.iter().map(|d| short(d)).collect(), fork_id: fork_id.into() }
}

fn page_of(response: DeskResponse) -> (ForkList, u64) {
    match response {
        DeskResponse::Forks { list, as_of } => (list, as_of),
        other => panic!("expected a page, got {other:?}"),
    }
}

fn entry_of(response: DeskResponse) -> (ForkEntry, u64) {
    match response {
        DeskResponse::Fork { entry, as_of } => (entry, as_of),
        other => panic!("expected an entry, got {other:?}"),
    }
}

/// A plain answer to the seeded fork, with a note.
fn noted(hash: &str, request_id: &str, note: &str) -> ForkAnswerRequest {
    ForkAnswerRequest { answer: Some(ForkAnswer { choice: Some("A".into()), note: Some(note.into()) }), ..wire(hash, request_id) }
}

fn message(id: &str, request: &DeskRequest) -> CultNetMessage {
    encode_desk_request(id, request, None).unwrap()
}

fn framed(message: &CultNetMessage) -> Vec<u8> {
    encode_frame(&encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap()).unwrap()
}

fn read_reply(stream: &mut impl Read) -> Option<CultNetMessage> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).ok()?;
    let mut payload = vec![0_u8; u32::from_be_bytes(header) as usize];
    stream.read_exact(&mut payload).ok()?;
    Some(decode_cultnet_message_from_slice(&payload, CultNetWireContract::CultNetSchemaV0).unwrap())
}

fn connect(path: &Path) -> UnixStream {
    let stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    stream
}

/// One connection: these bytes, then the reply if there is one.
fn exchange(path: &Path, bytes: &[u8]) -> Option<CultNetMessage> {
    let mut stream = connect(path);
    let _ = stream.write_all(bytes);
    read_reply(&mut stream)
}

fn reply_of(path: &Path, id: &str, request: &DeskRequest) -> DeskResponse {
    let reply = exchange(path, &framed(&message(id, request))).expect("the door replied");
    let (correlation, response) = decode_desk_response(&reply).unwrap();
    assert_eq!(correlation, id);
    response.unwrap_or_else(|failure| panic!("{failure:?}"))
}

fn failure_of(path: &Path, bytes: &[u8]) -> OperationFailure {
    let reply = exchange(path, bytes).expect("the door replied");
    decode_desk_response(&reply).unwrap().1.unwrap_err()
}

fn counted(response: DeskResponse) -> (String, Option<String>) {
    match response {
        DeskResponse::Answered { answer, counted_answer_id, .. } => {
            assert_eq!(answer.outcome, "Counted", "{answer:?}");
            (answer.answer_id, counted_answer_id)
        }
        other => panic!("expected an answer, got {other:?}"),
    }
}

/// A door on a real socket in a tempdir, a seeded daemon, and the recording index.
struct Rig {
    _dir: TempDir,
    _root: TempDir,
    path: PathBuf,
    daemon: Daemon<Store, RecordingIndex>,
    door: DeskDoor,
    hash: String,
    index: RecordingIndex,
}

fn rig(peer_uid: u32) -> Rig {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (daemon, hash) = seeded(root.path());
    let path = dir.path().join("desk.sock");
    let door = bind_desk(&path, peer_uid).unwrap();
    let index = daemon.index().clone();
    Rig { _dir: dir, _root: root, path, daemon, door, hash, index }
}

impl Rig {
    /// Runs `client` on its own thread while this one serves the door, then drains what is still waiting.
    fn serving<T: Send>(&mut self, client: impl FnOnce(&Path) -> T + Send) -> T {
        let path = self.path.clone();
        std::thread::scope(|scope| {
            let handle = scope.spawn(move || client(&path));
            while !handle.is_finished() {
                self.door.serve_ready(&mut self.daemon, now());
                std::thread::sleep(Duration::from_millis(1));
            }
            let out = handle.join().unwrap();
            while self.door.serve_ready(&mut self.daemon, now()) > 0 {}
            out
        })
    }

    fn state(&mut self) -> (u32, u32) {
        state(&mut self.daemon)
    }
}

// ---- flags ----

mod serve {
    use super::*;

    #[test]
    fn desk_flags_parse_both_or_neither() {
        let root = "/state";
        let base = [
            "--state-root", root, "--instance", "yggdrasil", "--bind", "127.0.0.1:17872", "--qdrant-url", "http://127.0.0.1:6333",
            "--ollama-url", "http://10.77.0.4:11434", "--embedding-model", "m",
        ];
        let parse = |extra: &[&str]| parse_options([base.to_vec(), extra.to_vec()].concat().into_iter().map(String::from));
        assert_eq!(parse(&[]).unwrap().desk, None);
        let desk = parse(&["--desk-socket", "/run/huginn/desk.sock", "--desk-peer-uid", "993"]).unwrap().desk.unwrap();
        assert_eq!((desk.socket, desk.peer_uid), (PathBuf::from("/run/huginn/desk.sock"), 993));
        for bad in [
            vec!["--desk-socket", "/run/huginn/desk.sock"],
            vec!["--desk-peer-uid", "993"],
            vec!["--desk-socket", "desk.sock", "--desk-peer-uid", "993"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-peer-uid", "bifrost"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-peer-uid", "-1"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-peer-uid", "4294967296"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-peer-uid", "0"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-peer-uid", "993", "--desk-peer-uid", "994"],
            vec!["--desk-socket", "/run/desk.sock", "--desk-socket", "/run/other.sock", "--desk-peer-uid", "993"],
        ] {
            assert!(parse(&bad).is_err(), "{bad:?}");
        }
    }
}

// ---- the unit seam: a counting stream ----

/// A stream that counts what the door reads and writes through it.
struct Counting {
    inner: UnixStream,
    reads: Arc<std::sync::atomic::AtomicU32>,
    writes: Arc<std::sync::atomic::AtomicU32>,
}

impl Read for Counting {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read(buf)
    }
}
impl Write for Counting {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl AsFd for Counting {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

mod desk_door {
    use super::*;

    fn pair() -> (UnixStream, Counting, Arc<std::sync::atomic::AtomicU32>, Arc<std::sync::atomic::AtomicU32>) {
        let (client, server) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let (reads, writes) = (Arc::default(), Arc::default());
        (client, Counting { inner: server, reads: Arc::clone(&reads), writes: Arc::clone(&writes) }, reads, writes)
    }

    #[test]
    fn a_refused_peer_has_nothing_read() {
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, hash) = seeded(root.path());
        let registry = desk_registry().unwrap();
        let before = state(&mut daemon);
        let mine = my_uid();
        let (mut client, server, reads, writes) = pair();
        client.write_all(&framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))))).unwrap();
        let served = serve_connection(server, mine + 1, &mut daemon, &registry, now(), |_| Ok(()));
        assert_eq!(served, Served::Refused { uid: mine, expected: mine + 1 });
        assert_eq!(served.to_string(), format!("desk door refused a peer with uid {mine} (expects {})", mine + 1));
        assert_eq!((reads.load(Ordering::Relaxed), writes.load(Ordering::Relaxed)), (0, 0), "no byte read or written");
        assert!(read_reply(&mut client).is_none(), "the stream was dropped");
        assert_eq!(state(&mut daemon), before);
    }

    #[test]
    fn the_configured_peer_is_answered() {
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, hash) = seeded(root.path());
        let registry = desk_registry().unwrap();
        let (mut client, server, reads, _) = pair();
        client.write_all(&framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))))).unwrap();
        let served = serve_connection(server, my_uid(), &mut daemon, &registry, now(), |_| Ok(()));
        assert_eq!(served, Served::Answered);
        assert!(reads.load(Ordering::Relaxed) > 0);
        let reply = read_reply(&mut client).expect("a reply");
        let (_, response) = decode_desk_response(&reply).unwrap();
        assert_eq!(counted(response.unwrap()), ("n1".to_string(), Some("n1".to_string())));
    }

    /// A header over the request bound is refused on the header alone: the transport reads the four
    /// bytes, sees the claim exceed the profile's maximum and never reads for the payload.
    #[test]
    fn an_oversize_header_is_refused_before_the_payload_is_read() {
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, _) = seeded(root.path());
        let registry = desk_registry().unwrap();
        for length in [65537_u32, u32::MAX] {
            let (mut client, server, reads, writes) = pair();
            client.write_all(&length.to_be_bytes()).unwrap();
            drop(client); // a payload read, if one were made, would end in EOF rather than wait
            let served = serve_connection(server, my_uid(), &mut daemon, &registry, now(), |_| Ok(()));
            let Served::Dropped(why) = served else { panic!("{served:?}") };
            assert!(why.contains("exceeds"), "{why}");
            assert_eq!((reads.load(Ordering::Relaxed), writes.load(Ordering::Relaxed)), (1, 0), "the header was the only read");
        }
    }

    /// The pure core answers the catalog with the two desk schemas and nothing else, and a
    /// reply past the response bound is a typed refusal that still names the request.
    #[test]
    fn the_catalog_is_the_desk_schemas_and_an_oversize_reply_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, _) = seeded(root.path());
        let registry = desk_registry().unwrap();
        let catalog = CultNetMessage::SchemaCatalogRequest {
            message_id: "c-1".into(),
            include_schema_json: Some(true),
            schema_ids: None,
            kinds: None,
        };
        let reply = answer_desk_message(&mut daemon, &registry, catalog, now());
        let text = format!("{reply:?}");
        assert!(text.contains("huginn.desk_request.v1") && text.contains("huginn.desk_response.v1"), "{text}");
        assert!(!text.contains("huginn.mind_request.v1"), "{text}");
        let other = answer_desk_message(&mut daemon, &registry, CultNetMessage::Error { error: "x".into(), code: None, details: None }, now());
        assert!(matches!(other, CultNetMessage::Error { .. }));
        let sized = crate::desk_door::respond(&DeskResponse::Refused(MindRefusal::Unavailable { detail: "x".repeat(2000) }), "m-9", "answer", "huginn-yggdrasil", 512);
        let (id, answered) = decode_desk_response(&sized).unwrap();
        assert_eq!(id, "m-9");
        assert!(matches!(answered.unwrap(), DeskResponse::Refused(MindRefusal::ResponseTooLarge { limit: 512, .. })));
    }
}

// ---- the door over a real socket ----

mod door_smoke {
    use super::*;

    #[test]
    fn a_counted_answer_over_the_socket_reads_back() {
        assert_ne!(my_uid(), 0, "the door refuses uid 0, so these tests cannot run as root");
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let response = rig.serving(|path| reply_of(path, "m-1", &answer("x", DeskAuthority::Operator, &["x", "y"], wire(&hash, R1))));
        let DeskResponse::Answered { answer, counted_answer_id, .. } = &response else { panic!("{response:?}") };
        assert_eq!((answer.outcome.as_str(), counted_answer_id.as_deref(), answer.authority), ("Counted", Some("n1"), DeskAuthority::Operator));
        assert_eq!(rig.state().0, before.0 + 1, "the answer is stored");
        assert!(rig.index.seen.lock().unwrap().is_empty(), "a desk answer is not indexed");
    }

    /// A pass takes at most `DESK_ACCEPTS_PER_PASS` waiting connections and says how many it took.
    #[test]
    fn a_pass_takes_four_connections_and_the_rest_wait() {
        let mut rig = rig(my_uid());
        let hash = rig.hash.clone();
        let mut clients: Vec<UnixStream> = (0..6)
            .map(|n| {
                let mut stream = connect(&rig.path);
                let request = answer("x", DeskAuthority::Member, &["x"], wire(&hash, &format!("00000000-0000-4000-8000-0000000001{n:02}")));
                stream.write_all(&framed(&message(&format!("m-{n}"), &request))).unwrap();
                stream
            })
            .collect();
        assert_eq!(rig.door.serve_ready(&mut rig.daemon, now()), 4);
        assert_eq!(rig.door.serve_ready(&mut rig.daemon, now()), 2);
        assert_eq!(rig.door.serve_ready(&mut rig.daemon, now()), 0, "an empty queue ends the pass");
        for client in &mut clients {
            assert!(read_reply(client).is_some());
        }
    }

    #[test]
    fn another_uid_is_refused_and_the_store_is_unchanged() {
        let mut rig = rig(my_uid() + 1);
        let (before, hash) = (rig.state(), rig.hash.clone());
        let reply = rig.serving(|path| exchange(path, &framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))))));
        assert!(reply.is_none(), "a refused peer reads EOF");
        assert_eq!(rig.state(), before);
        assert!(rig.index.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn an_oversize_header_is_dropped_and_the_next_peer_is_served() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let (dropped, answered) = rig.serving(|path| {
            let dropped: Vec<_> = [65537_u32, u32::MAX]
                .iter()
                .map(|length| {
                    let mut stream = connect(path);
                    stream.write_all(&length.to_be_bytes()).unwrap();
                    read_reply(&mut stream).is_none()
                })
                .collect();
            (dropped, reply_of(path, "m-2", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))))
        });
        assert_eq!(dropped, [true, true]);
        counted(answered);
        assert_eq!(rig.state().0, before.0 + 1);
    }

    /// A peer that writes two header bytes and holds is cut off by the read timeout, the next
    /// connection is answered in time, and the RUDP door is served in the same loop.
    #[test]
    fn a_stalled_peer_is_cut_off() {
        let Rig { _dir, _root, path, daemon, door, hash, .. } = rig(my_uid());
        let mut daemon = daemon;
        let mut door = door;
        let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
        let endpoint = format!("rudp://{}", hub.local_addr().unwrap());
        let registry = schema_registry().unwrap();
        let stopping = Arc::new(AtomicBool::new(false));
        let loop_stopping = Arc::clone(&stopping);
        let serving = std::thread::spawn(move || {
            run(&mut daemon, &mut hub, &registry, Some(&mut door), &loop_stopping, &ServeOptions::default()).unwrap();
        });

        let mut stalled = connect(&path);
        stalled.write_all(&[0, 0]).unwrap();
        let started = Instant::now();
        let mut rudp = CultMesh::create_rudp_client_for_endpoint(
            "desk-stall-test".to_string(),
            CULTNET_OPERATION_CONNECTION_ID,
            &endpoint,
            CultMeshRudpSocketOptions::default(),
        )
        .unwrap();
        rudp.connect(Vec::new()).unwrap();
        for _ in 0..2000 {
            let _ = rudp.receive_once();
            if rudp.connected() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(rudp.connected(), "the RUDP door was served during the stall");
        rudp.send_schema_message(&encode_request("w-1", &HuginnMindRequest::Whoami, None).unwrap()).unwrap();
        let answered = reply_of(&path, "m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1)));
        assert!(started.elapsed() < Duration::from_millis(4250), "{:?}", started.elapsed());
        counted(answered);
        let mut whoami = None;
        for _ in 0..2000 {
            rudp.poll_resends().unwrap();
            if let Some(reply) = rudp.receive_schema_message_once().unwrap() {
                whoami = Some(reply);
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let (_, whoami) = decode_response(&whoami.expect("whoami answered")).unwrap();
        assert!(matches!(whoami.unwrap(), HuginnMindResponse::Whoami(_)));
        assert!(read_reply(&mut stalled).is_none(), "the stalled connection was closed by the door");
        stopping.store(true, Ordering::Relaxed);
        serving.join().unwrap();
    }

    fn raw(service: &str, schema: &str, operation: &str, id: &str, payload: Vec<u8>) -> CultNetMessage {
        CultNetMessage::OperationRequest {
            message_id: id.into(),
            service_id: service.into(),
            operation: operation.into(),
            payload_schema: schema.into(),
            payload_encoding: "messagepack-base64".into(),
            payload: STANDARD.encode(payload),
            source_runtime_id: None,
            target_runtime_id: None,
        }
    }

    #[test]
    fn malformed_frames_get_typed_failures_or_are_dropped() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let good = answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1));
        let good_payload = rmp_serde::to_vec_named(&good).unwrap();
        let mut with_answerer = serde_json::to_value(&good).unwrap();
        with_answerer["Answer"]["answerer"] = serde_json::json!("someone-else");
        let long_id = "m".repeat(257);
        let outcomes = rig.serving(|path| {
            let garbage = {
                let mut payload = vec![0xc1, 0xc1, 0xc1];
                payload.extend_from_slice(b"not cultnet");
                let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
                bytes.extend(payload);
                exchange(path, &bytes).is_none()
            };
            let failure = |m: CultNetMessage| failure_of(path, &framed(&m));
            (
                garbage,
                failure(raw(MIND_SERVICE_ID, MIND_REQUEST_SCHEMA, "answer", "m-1", good_payload.clone())).code,
                failure(raw(DESK_SERVICE_ID, MIND_REQUEST_SCHEMA, "answer", "m-2", good_payload.clone())).code,
                failure(raw(DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, "forks", "m-3", good_payload.clone())).code,
                failure(raw(DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, "answer", "m-4", rmp_serde::to_vec_named(&with_answerer).unwrap())),
                exchange(path, &framed(&raw(DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, "answer", &long_id, good_payload.clone()))),
            )
        });
        let (garbage, mind_service, mind_schema, operation, answerer, long) = outcomes;
        assert!(garbage, "bytes that are not CultNet are dropped");
        assert_eq!(mind_service, "wrong-service");
        assert_eq!(mind_schema, "wrong-payload-schema");
        assert_eq!(operation, "operation-mismatch");
        assert_eq!(answerer.code, "payload-not-a-request");
        assert!(answerer.message.contains("answerer") || answerer.message.contains("Answer"), "{}", answerer.message);
        assert!(matches!(long, Some(CultNetMessage::Error { .. })), "{long:?}");
        assert_eq!(rig.state(), before, "nothing stored in any case");
    }

    /// The RUDP door has no desk method: a desk answer document in an Admit batch is refused,
    /// and a desk request payload is the wrong payload schema for the mind service.
    #[test]
    fn the_rudp_door_cannot_write_or_route_desk() {
        use huginn_mind::eureka_pipeline::{PipelineDeskAnswer, DeskOutcome, RequestId, Sha256Hex};
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, hash) = seeded(root.path());
        let before = state(&mut daemon);
        let forged = PipelineDocument::DeskAnswer(PipelineDeskAnswer {
            campaign: slug("eureka-state"),
            fork: short(FORK),
            sequence: 1,
            options_hash: Sha256Hex(hash.clone()),
            choice: Some(short("A")),
            note: None,
            answered_at: short("2026-09-16T11:00:00Z"),
            received_at: short("2026-09-16T11:00:00Z"),
            claim_in_future: false,
            request_id: RequestId(R1.into()),
            body_sha256: Sha256Hex("0".repeat(64)),
            answerer: short("x"),
            authority: DeskAuthority::Operator,
            outcome: DeskOutcome::Counted,
        });
        let admitted = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![forged])), now()).answered();
        assert!(matches!(admitted, HuginnMindResponse::Admit(huginn_mind::PipelineAdmissionOutcome::Refused(_))), "{admitted:?}");
        assert_eq!(state(&mut daemon), before);

        let registry = schema_registry().unwrap();
        let mut bodies = crate::bodies::DeferredBodies::new(1 << 20, Duration::from_secs(60));
        let desk = raw(MIND_SERVICE_ID, DESK_REQUEST_SCHEMA, "answer", "m-1", rmp_serde::to_vec_named(&answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))).unwrap());
        let CultNetMessage::OperationResponse { payload_schema, diagnostics, .. } = crate::serve::answer(&mut daemon, &registry, &mut bodies, desk, now()).reply() else {
            panic!("expected a response")
        };
        assert_eq!(payload_schema, huginn_mind::envelope::FAILURE_SCHEMA);
        assert_eq!(diagnostics, ["wrong-payload-schema"]);
        assert_eq!(state(&mut daemon), before);
    }

    #[test]
    fn a_stale_answer_carries_no_card() {
        let mut rig = rig(my_uid());
        let response = rig.serving(|path| reply_of(path, "m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&"0".repeat(64), R1))));
        let DeskResponse::Answered { answer, counted_answer_id, .. } = response else { panic!("a stale answer is stored") };
        assert_eq!(answer.code, Some(ForkAnswerCode::StaleFork));
        assert_eq!(answer.outcome, "Refused");
        assert_eq!(counted_answer_id, None);
        // The reply type has no field a card could ride in; the encoded reply holds nothing but the answer.
        assert_eq!(answer.options_hash, "0".repeat(64));
    }

    #[test]
    fn deciders_ride_each_request() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let out = rig.serving(|path| {
            let ask = |id: &str, who: &str, deciders: &[&str], request_id: &str| {
                reply_of(path, id, &answer(who, DeskAuthority::Operator, deciders, wire(&hash, request_id)))
            };
            let first = ask("m-1", "x", &["x"], R1);
            let other_list = ask("m-2", "x", &["y"], R2);
            let first_list_again = ask("m-3", "x", &["x"], R3);
            let mut empty = answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R3));
            let DeskRequest::Answer { deciders, .. } = &mut empty else { unreachable!() };
            deciders.clear();
            let empty_bytes = framed(&message("m-4", &empty));
            (first, other_list, first_list_again, failure_of(path, &empty_bytes))
        });
        let (first, other_list, again, empty) = out;
        let DeskResponse::Answered { answer, .. } = &first else { panic!("{first:?}") };
        assert_eq!((answer.outcome.as_str(), answer.authority), ("Counted", DeskAuthority::Operator));
        let DeskResponse::Answered { answer, .. } = &other_list else { panic!("{other_list:?}") };
        assert_eq!(answer.code, Some(ForkAnswerCode::NotADecider), "the second list does not name x");
        let DeskResponse::Answered { answer, .. } = &again else { panic!("{again:?}") };
        assert_eq!(answer.outcome, "Revision", "nothing was kept between calls: the first list counts again");
        assert_eq!((empty.code.as_str(), empty.message.contains("deciders")), ("payload-not-a-request", true));
        assert_eq!(rig.state().0, before.0 + 3, "three answers landed: two counted, one refused");
    }

    #[test]
    fn startup_replaces_a_stale_socket_and_refuses_a_live_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.sock");
        // A stale socket file: bound once, listener gone.
        drop(UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        let door = bind_desk(&path, my_uid()).expect("a stale socket is replaced");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o660);
        // A live listener refuses a second bind.
        let error = format!("{:#}", bind_desk(&path, my_uid()).err().expect("a live socket refuses"));
        assert!(error.contains("live"), "{error}");
        assert!(path.exists(), "the live server's socket was not unlinked");
        drop(door);
        assert!(!path.exists(), "drop unlinks the path");
        // A socket the door cannot probe is neither stale nor free: it is left alone.
        let guarded = dir.path().join("guarded.sock");
        let _held = UnixListener::bind(&guarded).unwrap();
        std::fs::set_permissions(&guarded, std::fs::Permissions::from_mode(0o000)).unwrap();
        let error = format!("{:#}", bind_desk(&guarded, my_uid()).err().expect("an unprobeable socket refuses"));
        assert!(error.contains("probing"), "{error}");
        assert!(guarded.exists());
        // A path the door cannot even look at is not a free path.
        let sealed = dir.path().join("sealed");
        std::fs::create_dir(&sealed).unwrap();
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).unwrap();
        let error = format!("{:#}", bind_desk(&sealed.join("desk.sock"), my_uid()).err().expect("an unreadable path refuses"));
        assert!(error.contains("reading"), "{error}");
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o700)).unwrap();
        // A regular file is not a socket.
        std::fs::write(&path, b"x").unwrap();
        assert!(bind_desk(&path, my_uid()).is_err());
        assert!(path.is_file(), "the file was left alone");
        // A relative path and uid 0 are refused.
        assert!(bind_desk(Path::new("desk.sock"), my_uid()).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(bind_desk(&path, 0).is_err());
        assert!(!path.exists());

        // A held mind binds no desk socket, and a free one binds it after the RUDP socket.
        let root = tempfile::tempdir().unwrap();
        let held = crate::daemon::tests::open_unindexed(root.path(), &slug(INSTANCE)).unwrap();
        let options = Options {
            state_root: root.path().to_path_buf(),
            instance: slug(INSTANCE),
            bind: "127.0.0.1:0".parse().unwrap(),
            qdrant_url: "http://127.0.0.1:1".into(),
            ollama_urls: vec!["http://127.0.0.1:1".into()],
            reserve_ollama_urls: Vec::new(),
            embedding_model: "a-model".into(),
            forge_url: None,
            desk: Some(crate::serve::DeskOptions { socket: path.clone(), peer_uid: my_uid() }),
        };
        let started = |options: &Options| {
            let (embedder, index) = crate::index::fakes::pair();
            startup(options, embedder, index, Backoff::default())
        };
        assert!(started(&options).is_err());
        assert!(!path.exists(), "a mind that will not open binds neither");
        drop(held);
        let up = started(&options).expect("both gates open");
        assert!(up.desk.is_some() && path.exists());
        drop(up);
        assert!(!path.exists());
    }

    #[test]
    fn a_reply_lost_after_commit_replays_on_retry() {
        let mut rig = rig(my_uid());
        let hash = rig.hash.clone();
        let bytes = framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))));
        let (replayed, first_state) = {
            let sent = bytes.clone();
            rig.serving(move |path| {
                let mut stream = connect(path);
                stream.write_all(&sent).unwrap();
                drop(stream); // closes without reading the reply
            });
            let first_state = rig.state();
            let replayed = rig.serving(|path| exchange(path, &bytes).expect("the retry is answered"));
            (replayed, first_state)
        };
        let (_, response) = decode_desk_response(&replayed).unwrap();
        assert_eq!(counted(response.unwrap()).0, "n1", "the retry is answered with the stored answer");
        assert_eq!(rig.state(), first_state, "the retry stored nothing");
        assert!(rig.index.seen.lock().unwrap().is_empty(), "a desk answer is not indexed");
    }

    #[test]
    fn a_frame_cut_off_before_the_door_has_it_stores_nothing() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let bytes = framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1))));
        let cut = rig.serving(|path| {
            let mut stream = connect(path);
            stream.write_all(&bytes[..bytes.len() / 2]).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            read_reply(&mut stream).is_none()
        });
        assert!(cut);
        assert_eq!(rig.state(), before);
        assert!(rig.index.seen.lock().unwrap().is_empty());
        let full = rig.serving(|path| exchange(path, &bytes).expect("answered"));
        let (_, response) = decode_desk_response(&full).unwrap();
        assert_eq!(counted(response.unwrap()).0, "n1");
    }

    #[test]
    fn answer_deciders_64_accepted_65_refused() {
        let mut rig = rig(my_uid());
        let hash = rig.hash.clone();
        let list = |n: usize| (0..n).map(|d| format!("d{d}")).collect::<Vec<_>>();
        let (sixty_four, sixty_five_bytes) = {
            let (sixty_four, sixty_five) = (list(64), list(65));
            let sixty_four_refs: Vec<&str> = sixty_four.iter().map(String::as_str).collect();
            let sixty_five_refs: Vec<&str> = sixty_five.iter().map(String::as_str).collect();
            (
                answer("d0", DeskAuthority::Member, &sixty_four_refs, wire(&hash, R1)),
                framed(&message("m-2", &answer("d0", DeskAuthority::Member, &sixty_five_refs, wire(&hash, R2)))),
            )
        };
        let served = rig.serving(|path| reply_of(path, "m-1", &sixty_four));
        counted(served);
        let after_first = rig.state();
        let refused = rig.serving(|path| failure_of(path, &sixty_five_bytes));
        assert_eq!(refused.code, "payload-not-a-request");
        assert!(refused.message.contains("deciders"), "{}", refused.message);
        assert_eq!(rig.state(), after_first, "count and head unchanged");
    }

    #[test]
    fn answer_deciders_empty_or_duplicate_refused() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let empty = framed(&message("m-1", &answer("x", DeskAuthority::Member, &[], wire(&hash, R1))));
        let twice = framed(&message("m-2", &answer("x", DeskAuthority::Member, &["x", "x"], wire(&hash, R2))));
        let (empty, twice, one) = rig.serving(|path| {
            (failure_of(path, &empty), failure_of(path, &twice), reply_of(path, "m-3", &answer("x", DeskAuthority::Member, &["x"], wire(&hash, R3))))
        });
        for refused in [empty, twice] {
            assert_eq!(refused.code, "payload-not-a-request");
            assert!(refused.message.contains("deciders"), "{}", refused.message);
        }
        counted(one);
        assert_eq!(rig.state().0, before.0 + 1, "only the one-id list stored an answer");
    }

    #[test]
    fn a_mind_refusal_and_a_stored_refusal_are_typed_replies() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let mut foreign = answer("x", DeskAuthority::Member, &["x"], wire(&hash, R1));
        let DeskRequest::Answer { mind, .. } = &mut foreign else { unreachable!() };
        *mind = slug(OTHER);
        let (foreign, not_a_decider) = rig.serving(|path| {
            (reply_of(path, "m-1", &foreign), reply_of(path, "m-2", &answer("x", DeskAuthority::Member, &["y"], wire(&hash, R2))))
        });
        assert_eq!(foreign, DeskResponse::Refused(MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() }));
        assert_eq!(rig.state().0, before.0 + 1, "only the stored refusal landed");
        let DeskResponse::Answered { answer, counted_answer_id, .. } = not_a_decider else { panic!("a stored refusal is an answer") };
        assert_eq!((answer.code, counted_answer_id), (Some(ForkAnswerCode::NotADecider), None));
    }

    #[test]
    fn forks_and_fork_read_the_stored_fork_and_store_nothing() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let (forks, fork) = rig.serving(|path| (reply_of(path, "m-1", &forks_all(&["x", "y"])), reply_of(path, "m-2", &fork_read("x", &["x", "y"], FORK))));
        let (list, listed_at) = page_of(forks);
        let (entry, read_at) = entry_of(fork);
        assert_eq!((list.open_count, list.recorded_count, list.forks.len(), list.next_cursor), (1, 0, 1, None));
        assert_eq!(list.mind, INSTANCE);
        assert_eq!((listed_at, read_at), (before.1 as u64, before.1 as u64), "both read at the head the mind held");
        assert_eq!(list.stamp, format!("s{listed_at}"));
        assert_eq!(list.forks[0], entry);
        assert_eq!((entry.card.fork_id.as_str(), entry.card.options_hash.as_str(), entry.card.deciders.as_slice()), (FORK, hash.as_str(), ["x".to_string(), "y".to_string()].as_slice()));
        assert_eq!(rig.state(), before, "a read stores nothing");
        assert!(rig.index.seen.lock().unwrap().is_empty());
        // An unknown fork is the answer path's refusal.
        let unknown = rig.serving(|path| reply_of(path, "m-3", &fork_read("x", &["x"], "eureka-state:question:Q404")));
        assert!(matches!(unknown, DeskResponse::NotStored(ref refusal) if refusal.code == huginn_mind::desk_wire::ForkRefusalCode::ForkUnknown), "{unknown:?}");
        // The mind is the instance's own.
        let mut foreign = forks_all(&["x"]);
        let DeskRequest::Forks { mind, .. } = &mut foreign else { unreachable!() };
        *mind = slug(OTHER);
        let refused = rig.serving(|path| reply_of(path, "m-4", &foreign));
        assert_eq!(refused, DeskResponse::Refused(MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() }));
        assert_eq!(rig.state(), before);
    }

    /// The daemon hands a list's `status` and `cursor` to the Mind, whole: a recorded-only list is the
    /// one closed fork, an open-only list the open ones, and a cursor page is the forks below it; read
    /// both in the daemon and over the socket.
    #[test]
    fn the_status_and_the_cursor_reach_the_mind() {
        let mut rig = rig(my_uid());
        let (question, ruling) = question_and_ruling();
        let PipelineDocument::Question(held) = &question else { unreachable!() };
        let more: Vec<PipelineDocument> = (2..=3)
            .map(|n| {
                let mut next = held.clone();
                next.label = format!("Q{n}").as_str().into();
                PipelineDocument::Question(next)
            })
            .collect();
        for documents in [more, vec![ruling]] {
            let admitted = rig.daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), now()).answered();
            assert!(matches!(admitted, HuginnMindResponse::Admit(huginn_mind::PipelineAdmissionOutcome::Committed { .. })), "{admitted:?}");
        }
        let ids = |list: &ForkList| list.forks.iter().map(|entry| entry.card.fork_id.clone()).collect::<Vec<_>>();
        let q = |n: u32| format!("eureka-state:question:Q{n}");
        let cases = [
            (Some(ListStatus::Recorded), None, vec![q(1)]),
            (Some(ListStatus::Open), None, vec![q(3), q(2)]),
            (None, None, vec![q(3), q(2), q(1)]),
            (None, ForkCursor::new(2), vec![q(2), q(1)]),
            (Some(ListStatus::Open), ForkCursor::new(2), vec![q(2)]),
        ];
        let now = now();
        for (status, cursor, expected) in cases {
            let request = forks("x", &["x"], status, cursor, 50);
            let (list, _) = page_of(rig.daemon.handle_desk(request.clone(), now));
            assert_eq!(ids(&list), expected, "{status:?} {cursor:?} in the daemon");
            let (list, _) = page_of(rig.serving(|path| reply_of(path, "m-1", &request)));
            assert_eq!(ids(&list), expected, "{status:?} {cursor:?} over the socket");
            assert_eq!((list.open_count, list.recorded_count), (2, 1));
        }
    }

    /// Ruling `desk-read-visibility-withheld-ids`: a subject the call's deciders do not name gets counts
    /// and no account id, over the socket too; notes stay whole (Bifrost strips them). A decider gets the ids.
    #[test]
    fn a_subject_not_in_deciders_gets_counts_and_no_account_id() {
        let mut rig = rig(my_uid());
        let hash = rig.hash.clone();
        let note = "Keep it local: the author's call.";
        rig.serving(|path| counted(reply_of(path, "m-1", &DeskRequest::Answer {
            mind: slug(INSTANCE),
            subject: short("y"),
            authority: DeskAuthority::Member,
            deciders: vec![short("y")],
            request: noted(&hash, R1, note),
        })));
        let (list, read) = rig.serving(|path| (reply_of(path, "m-2", &forks("outsider", &["x"], None, None, 50)), reply_of(path, "m-3", &fork_read("outsider", &["x"], FORK))));
        let (list, _) = page_of(list);
        let (read, _) = entry_of(read);
        for entry in [&list.forks[0], &read] {
            assert_eq!(entry.record.answers.len(), 1);
            assert_eq!(entry.record.answers[0].note.as_deref(), Some(note));
            assert_eq!(entry.record.answers[0].answerer, None);
            assert_eq!(entry.record.answerer_count, 1);
            assert!(entry.card.deciders.is_empty());
            assert_eq!(entry.card.decider_count, 1);
        }
        let seen = rig.serving(|path| reply_of(path, "m-4", &fork_read("x", &["x"], FORK)));
        let (seen, _) = entry_of(seen);
        assert_eq!(seen.card.deciders, ["x"]);
        assert_eq!(seen.record.answers[0].answerer.as_deref(), Some("y"));
    }

    #[test]
    fn forks_and_fork_deciders_64_accepted_65_refused() {
        let mut rig = rig(my_uid());
        let before = rig.state();
        let names = |n: usize| (0..n).map(|d| format!("d{d}")).collect::<Vec<_>>();
        let (sixty_four, sixty_five) = (names(64), names(65));
        let (sixty_four, sixty_five): (Vec<&str>, Vec<&str>) =
            (sixty_four.iter().map(String::as_str).collect(), sixty_five.iter().map(String::as_str).collect());
        let accepted = rig.serving(|path| (reply_of(path, "m-1", &forks("d0", &sixty_four, None, None, 50)), reply_of(path, "m-2", &fork_read("d0", &sixty_four, FORK))));
        assert_eq!(page_of(accepted.0).0.forks[0].card.deciders.len(), 64);
        assert_eq!(entry_of(accepted.1).0.card.deciders.len(), 64);
        for (what, request) in [("forks", forks("d0", &sixty_five, None, None, 50)), ("fork", fork_read("d0", &sixty_five, FORK))] {
            let bytes = framed(&message("m-3", &request));
            let refused = rig.serving(|path| failure_of(path, &bytes));
            assert_eq!(refused.code, "payload-not-a-request", "{what}");
            assert!(refused.message.contains("deciders"), "{what}: {}", refused.message);
            assert!(!refused.message.contains("d64"), "{what} echoes a decider: {}", refused.message);
        }
        assert_eq!(rig.state(), before, "nothing stored, head unchanged");
    }

    #[test]
    fn forks_and_fork_deciders_empty_or_duplicate_refused() {
        let mut rig = rig(my_uid());
        let before = rig.state();
        for (what, list) in [("empty", vec![]), ("duplicate", vec!["x", "x"])] {
            for (name, request) in [("forks", forks("x", &list, None, None, 50)), ("fork", fork_read("x", &list, FORK))] {
                let bytes = framed(&message("m-1", &request));
                let refused = rig.serving(|path| failure_of(path, &bytes));
                assert_eq!(refused.code, "payload-not-a-request", "{what} {name}");
                assert!(refused.message.contains("deciders"), "{what} {name}: {}", refused.message);
            }
        }
        // One decider is a list.
        let ok = rig.serving(|path| reply_of(path, "m-2", &forks_all(&["x"])));
        assert_eq!(page_of(ok).0.forks.len(), 1);
        assert_eq!(rig.state(), before);
    }

    /// Chat's reads carry no `authority`: CultLib's list and read requests are closed and have none, so a
    /// payload that names it is no request. Nothing is answered and the head does not move.
    #[test]
    fn a_read_that_names_an_authority_is_refused_at_the_door() {
        let mut rig = rig(my_uid());
        let before = rig.state();
        for (operation, request) in [("forks", forks_all(&["x"])), ("fork", fork_read("x", &["x"], FORK))] {
            let mut value = serde_json::to_value(&request).unwrap();
            let variant = if operation == "forks" { "Forks" } else { "Fork" };
            value[variant]["authority"] = serde_json::json!("Operator");
            let payload = rmp_serde::to_vec_named(&value).unwrap();
            let sent = framed(&raw(DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, operation, "m-1", payload));
            let refused = rig.serving(|path| failure_of(path, &sent));
            assert_eq!(refused.code, "payload-not-a-request", "{operation}");
            assert!(refused.message.contains(variant), "{operation}: {}", refused.message);
        }
        assert_eq!(rig.state(), before);
        // The same reads without it are answered.
        let (list, entry) = rig.serving(|path| (reply_of(path, "m-2", &forks_all(&["x"])), reply_of(path, "m-3", &fork_read("x", &["x"], FORK))));
        assert_eq!((page_of(list).0.forks.len(), entry_of(entry).0.card.fork_id.as_str().to_string()), (1, FORK.to_string()));
    }

    /// Fifty wordy forks, one commit each. A limit-50 page of them would pass the response bound: the Mind
    /// stops the page short at its budget and names the cursor, so the door sends it whole; the door's own
    /// measure stays exact at the bound. Fifty of them admitted in one commit page like any other: the
    /// ranks are unique, so the Mind stops the page at its budget inside the commit.
    #[test]
    fn a_page_is_short_of_the_response_bound_and_the_door_still_measures_it_exactly() {
        let mut rig = rig(my_uid());
        let (question, _) = question_and_ruling();
        let PipelineDocument::Question(held) = question else { unreachable!() };
        let documents: Vec<PipelineDocument> = (2..=51)
            .map(|n| {
                let mut wordy = held.clone();
                wordy.label = format!("Q{n}").as_str().into();
                wordy.question = "Should a slot mark be shared between authors? ".repeat(18).as_str().into();
                PipelineDocument::Question(wordy)
            })
            .collect();
        let admitted = rig.daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents.clone())), now()).answered();
        assert!(matches!(admitted, HuginnMindResponse::Admit(huginn_mind::PipelineAdmissionOutcome::Committed { .. })), "{admitted:?}");
        let before = rig.state();
        let grouped = rig.serving(|path| reply_of(path, "m-0", &forks_all(&["x"])));
        let (grouped, _) = page_of(grouped);
        assert!(grouped.forks.len() > 1 && grouped.forks.len() < 50 && grouped.next_cursor.is_some(), "{} entries", grouped.forks.len());
        assert_eq!(rig.state(), before);
        // Then fifty more, each in its own commit, newer than the group.
        for n in 52..=101 {
            let PipelineDocument::Question(mut wordy) = documents[0].clone() else { unreachable!() };
            wordy.label = format!("Q{n}").as_str().into();
            let admitted = rig.daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![PipelineDocument::Question(wordy)])), now()).answered();
            assert!(matches!(admitted, HuginnMindResponse::Admit(huginn_mind::PipelineAdmissionOutcome::Committed { .. })), "{admitted:?}");
        }
        let before = rig.state();
        let (big, small) = rig.serving(|path| (reply_of(path, "m-1", &forks_all(&["x"])), reply_of(path, "m-2", &forks("x", &["x"], None, None, 10))));
        let (big, _) = page_of(big);
        assert!(big.forks.len() > 10 && big.forks.len() < 50 && big.next_cursor.is_some(), "{} entries", big.forks.len());
        let (small, _) = page_of(small);
        assert_eq!((small.forks.len(), small.open_count), (10, 101));
        assert!(small.next_cursor.is_some());
        assert_eq!(rig.state(), before);
        // The bound is exact on a read: a response of N bytes passes at N and is refused at N - 1.
        let response = DeskResponse::Forks { list: small, as_of: 1 };
        let size = {
            let message = huginn_mind::envelope::encode_desk_response("m-9", "forks", &response, "huginn-yggdrasil").unwrap();
            encode_cultnet_message_to_vec(&message, CultNetWireContract::CultNetSchemaV0).unwrap().len()
        };
        let at = |limit: usize| decode_desk_response(&crate::desk_door::respond(&response, "m-9", "forks", "huginn-yggdrasil", limit)).unwrap().1.unwrap();
        assert_eq!(at(size), response);
        assert!(matches!(at(size - 1), DeskResponse::Refused(MindRefusal::ResponseTooLarge { .. })));
    }
}

// ---- the bounds, pinned as the numbers the spec states ----

mod bounds {
    use crate::desk_door::Deadlined;
    use cultnet_rs::{CultNetSchemaKind, CultNetSchemaRegistration, CultNetSchemaRegistry};
    use huginn_mind::desk_wire::DESK_MAX_RESPONSE_BYTES;

    use super::*;

    /// One connection costs the loop one second and no more, whatever the peer does: the stated
    /// worst stall is `DESK_ACCEPTS_PER_PASS` of them, four seconds. The lower edge fails a
    /// shorter bound, the upper a longer one or one that restarts on every byte.
    fn served_within_one_second(client: impl FnOnce(UnixStream) + Send) {
        let mut rig = rig(my_uid());
        let peer = connect(&rig.path);
        std::thread::scope(|scope| {
            scope.spawn(move || client(peer));
            let started = Instant::now();
            assert_eq!(rig.door.serve_ready(&mut rig.daemon, now()), 1);
            let took = started.elapsed();
            assert!(took >= Duration::from_millis(950), "the connection was cut after {took:?}, short of the one-second deadline (the kernel timeout is jiffy-granular)");
            assert!(took < Duration::from_millis(1250), "the connection held the loop for {took:?}");
        });
    }

    #[test]
    fn a_silent_peer_is_cut_off_at_one_second() {
        served_within_one_second(|peer| {
            std::thread::sleep(Duration::from_millis(1500));
            drop(peer);
        });
    }

    #[test]
    fn a_dripping_peer_is_cut_off_at_one_second() {
        let bytes = framed(&message("m-1", &answer("x", DeskAuthority::Operator, &["x"], wire("0", R1))));
        served_within_one_second(move |mut peer| {
            for byte in bytes.iter().take(20) {
                if peer.write_all(&[*byte]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        });
    }

    /// The write side of the deadline: a peer that stops reading is let go of at one second, the
    /// socket's own send timeout reset to what is left before each write.
    #[test]
    fn a_peer_that_never_reads_is_cut_off_at_one_second() {
        let (_peer, server) = UnixStream::pair().unwrap();
        let mut stream = Deadlined::new(server, DESK_CONNECTION_DEADLINE);
        // The writer runs on its own thread and the test waits for it with a clock of its own: without
        // the deadline the write blocks for good, and that must fail this test, not hang the suite.
        let (sent, heard) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            let (chunk, started) = (vec![0_u8; 64 * 1024], Instant::now());
            let error = loop {
                if let Err(error) = stream.write(&chunk) {
                    break error;
                }
            };
            let took = started.elapsed();
            let again = stream.write(&chunk).unwrap_err().kind();
            let _ = sent.send((error, took, again));
        });
        let (error, took, again) = heard.recv_timeout(Duration::from_secs(5)).expect("a peer that never reads is cut off within seconds, not waited on");
        writer.join().unwrap();
        // The kernel's send timeout reads as WouldBlock; the deadline already spent reads as TimedOut.
        assert!(matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut), "{error:?}");
        assert!(took >= Duration::from_millis(950) && took < Duration::from_millis(1250), "the write was cut after {took:?}");
        assert_eq!(again, std::io::ErrorKind::TimedOut, "once spent, nothing more is written");
    }

    /// A catalog is a reply like any other and is measured against the response bound: one that
    /// encodes to exactly the bound is sent, one byte more is refused naming the size.
    #[test]
    fn the_catalog_reply_is_bounded_at_exactly_the_response_limit() {
        assert_eq!(DESK_MAX_RESPONSE_BYTES, 65_536);
        let root = tempfile::tempdir().unwrap();
        let (mut daemon, _) = seeded(root.path());
        let request = CultNetMessage::SchemaCatalogRequest { message_id: "c-1".into(), include_schema_json: Some(true), schema_ids: None, kinds: None };
        let registry = |filler: usize| {
            let mut registry = desk_registry().unwrap();
            registry
                .register(CultNetSchemaRegistration {
                    schema_id: "filler".into(),
                    kind: CultNetSchemaKind::WireMessage,
                    wire_contracts: vec![CultNetWireContract::CultNetSchemaV0],
                    schema_version: Some("filler".into()),
                    document_type: None,
                    title: None,
                    schema_json: Some(format!("\"{}\"", "x".repeat(filler))),
                })
                .unwrap();
            registry
        };
        let size = |registry: &CultNetSchemaRegistry| {
            encode_cultnet_message_to_vec(&registry.create_catalog_response(&request).unwrap(), CultNetWireContract::CultNetSchemaV0).unwrap().len()
        };
        let mut filler = DESK_MAX_RESPONSE_BYTES - size(&registry(0));
        while size(&registry(filler)) > DESK_MAX_RESPONSE_BYTES {
            filler -= 1;
        }
        while size(&registry(filler + 1)) <= DESK_MAX_RESPONSE_BYTES {
            filler += 1;
        }
        let at_limit = answer_desk_message(&mut daemon, &registry(filler), request.clone(), now());
        assert!(matches!(at_limit, CultNetMessage::SchemaCatalogResponse { .. }), "{at_limit:?}");
        assert!(encode_cultnet_message_to_vec(&at_limit, CultNetWireContract::CultNetSchemaV0).unwrap().len() <= DESK_MAX_RESPONSE_BYTES);
        let over = answer_desk_message(&mut daemon, &registry(filler + 1), request, now());
        let CultNetMessage::Error { error, .. } = over else { panic!("{over:?}") };
        assert!(error.contains("65536"), "{error}");
    }

    /// A fresh answer stores one document; the retry that replays it stores none.
    #[test]
    fn a_fresh_answer_stores_one_document_and_its_replay_none() {
        let mut rig = rig(my_uid());
        let (before, hash) = (rig.state(), rig.hash.clone());
        let request = answer("x", DeskAuthority::Operator, &["x"], wire(&hash, R1));
        let first = rig.daemon.handle_desk(request.clone(), now());
        assert_eq!(counted(first.clone()).0, "n1");
        assert_eq!(rig.state().0, before.0 + 1);
        let replay = rig.daemon.handle_desk(request, now());
        assert_eq!(replay, first);
        assert_eq!(rig.state().0, before.0 + 1, "the replay stored nothing");
    }
}
