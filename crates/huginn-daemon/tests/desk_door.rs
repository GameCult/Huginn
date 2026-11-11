//! The desk door through the built binary: a real `huginn-daemon` process, its two sockets and its
//! mind on disk. What the unit and pipeline smokes cannot show: the flags reach the door, the
//! door answers beside the RUDP door in one process, a stopped daemon takes its socket with it,
//! a leftover socket is replaced, a retry after a restart replays the stored answer, and a peer
//! that is not the configured uid is refused (logged by name) with the mind untouched.
//!
//! Needs a non-root uid: the door refuses uid 0.

use std::io::{Read, Write};
use std::net::UdpSocket;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use cultnet_rs::{
    CULTNET_OPERATION_CONNECTION_ID, CultMesh, CultMeshRudpSocketOptions, CultNetMessage, CultNetWireContract,
    decode_cultnet_message_from_slice, encode_cultnet_message_to_vec, encode_frame,
};
use huginn_mind::desk_wire::{DeskRequest, DeskResponse, ForkAnswer, ForkAnswerCode, ForkAnswerRequest, RequestSchema};
use huginn_mind::envelope::{decode_desk_response, decode_response, encode_desk_request, encode_request};
use huginn_mind::eureka_pipeline::{
    Date, DeskAuthority, DocRef, OrgRepo, PipelineCampaign, PipelineDocument, PipelineInstance, PipelineQuestion, PipelineStewardship,
    QuestionOption, Sha, Short, Title,
};
use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse};
use huginn_mind::{Faculty, Mind, PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelineProvenance, options_hash};

const INSTANCE: &str = "yggdrasil";
const FORK: &str = "eureka-state:question:Q1";
const R1: &str = "00000000-0000-4000-8000-000000000001";
const R2: &str = "00000000-0000-4000-8000-000000000002";

/// These tests run one at a time: each forks daemons and times them, and a fork made while another
/// test holds a mind open lends that child the mind's lock until it execs.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn my_uid() -> u32 {
    let dir = tempfile::tempdir().unwrap();
    std::fs::metadata(dir.path()).unwrap().uid()
}

fn short(value: &str) -> Short {
    Short(value.into())
}

/// A mind holding the campaign and one answerable question; returns the question's `options_hash`.
fn seed(root: &Path) -> String {
    let mut mind = Mind::open(root, &INSTANCE.into()).unwrap();
    let question = PipelineQuestion {
        campaign: "eureka-state".into(),
        label: "Q1".into(),
        title: Title("Who owns the state?".into()),
        question: "Who owns the state?".into(),
        options: vec![
            QuestionOption { label: "A".into(), text: "the organ".into() },
            QuestionOption { label: "B".into(), text: "the client".into() },
        ],
        recommended: "A".into(),
        depends: vec![],
        raised_in: None,
        asked_on: Date("2026-09-16".into()),
    };
    let hash = options_hash(&question).0;
    let repo = OrgRepo("GameCult/Epiphany".into());
    let documents = vec![
        PipelineDocument::Instance(PipelineInstance {
            instance: INSTANCE.into(),
            display_name: short("yggdrasil mind"),
            created_at: Date("2026-09-16".into()),
            host: short(INSTANCE),
        }),
        PipelineDocument::Stewardship(PipelineStewardship {
            instance: INSTANCE.into(),
            repo: repo.clone(),
            sequence: 1,
            assigned_on: Date("2026-09-16".into()),
            note: "assigned".into(),
            names: vec![],
        }),
        PipelineDocument::Campaign(PipelineCampaign {
            slug: "eureka-state".into(),
            title: Title("Eureka pipeline state".into()),
            repos: vec![repo],
            working_branch: short("codex/eureka-pipeline-state"),
            target_doc: DocRef { path: short("notes/target.md"), start_line: 1, end_line: 9, commit: Sha("5f98228d".into()) },
        }),
        PipelineDocument::Question(question),
    ];
    let provenance = PipelineProvenance {
        faculty: Faculty::Hands,
        agent: short("claude"),
        session: short("session-1"),
        tool: short("admit"),
        imported_from: None,
    };
    let batch = PipelineAdmissionBatch { instance: INSTANCE.into(), provenance, documents };
    let outcome = mind.admit(batch, Utc.with_ymd_and_hms(2026, 9, 16, 12, 0, 0).unwrap());
    assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    hash
}

/// A daemon process over `root`, its RUDP door on a free local port, its desk door at `desk`.
struct Daemon {
    child: Child,
    rudp: String,
    log: PathBuf,
}

fn free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn spawn(root: &Path, desk: &Path, peer_uid: u32, log: &Path) -> Daemon {
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_huginn-daemon"))
        .args(["--state-root", root.to_str().unwrap(), "--instance", INSTANCE, "--bind", &format!("127.0.0.1:{port}")])
        .args(["--qdrant-url", "http://127.0.0.1:1", "--ollama-url", "http://127.0.0.1:1", "--embedding-model", "a-model"])
        .args(["--desk-socket", desk.to_str().unwrap(), "--desk-peer-uid", &peer_uid.to_string()])
        .stdout(Stdio::null())
        .stderr(std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap())
        .spawn()
        .unwrap();
    let mut daemon = Daemon { child, rudp: format!("rudp://127.0.0.1:{port}"), log: log.to_path_buf() };
    let started = Instant::now();
    // Listening, not merely present: a killed predecessor leaves a socket nobody answers on.
    while UnixStream::connect(desk).is_err() {
        assert!(daemon.child.try_wait().unwrap().is_none(), "the daemon exited early:\n{}", daemon.log_text());
        assert!(started.elapsed() < Duration::from_secs(30), "the desk socket never appeared:\n{}", daemon.log_text());
        std::thread::sleep(Duration::from_millis(20));
    }
    daemon
}

impl Daemon {
    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn signal(&mut self, name: &str) {
        let status = Command::new("kill").args([&format!("-{name}"), &self.child.id().to_string()]).status().unwrap();
        assert!(status.success());
        let started = Instant::now();
        while self.child.try_wait().unwrap().is_none() {
            assert!(started.elapsed() < Duration::from_secs(20), "the daemon did not stop on {name}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn documents(&self) -> u32 {
        let mut client = CultMesh::create_rudp_client_for_endpoint(
            "desk-e2e".to_string(),
            CULTNET_OPERATION_CONNECTION_ID,
            &self.rudp,
            CultMeshRudpSocketOptions::default(),
        )
        .unwrap();
        client.connect(Vec::new()).unwrap();
        for _ in 0..2000 {
            let _ = client.receive_once();
            if client.connected() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(client.connected(), "the RUDP door never accepted:\n{}", self.log_text());
        client.send_schema_message(&encode_request("w-1", &HuginnMindRequest::Whoami, None).unwrap()).unwrap();
        for _ in 0..2000 {
            client.poll_resends().unwrap();
            if let Some(reply) = client.receive_schema_message_once().unwrap() {
                return match decode_response(&reply).unwrap().1.unwrap() {
                    HuginnMindResponse::Whoami(status) => status.documents,
                    other => panic!("{other:?}"),
                };
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("whoami was not answered");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn answer(who: &str, hash: &str, request_id: &str) -> DeskRequest {
    DeskRequest::Answer {
        mind: INSTANCE.into(),
        subject: short(who),
        authority: DeskAuthority::Operator,
        deciders: vec![short(who)],
        request: ForkAnswerRequest {
            schema_version: RequestSchema::V0,
            fork_id: FORK.into(),
            options_hash: hash.into(),
            answer: Some(ForkAnswer { choice: Some("A".into()), note: None }),
            retracted: false,
            retracts: None,
            answered_at: "2026-09-16T11:00:00Z".into(),
            request_id: request_id.into(),
        },
    }
}

fn forks_request(who: &str, deciders: &[&str]) -> DeskRequest {
    DeskRequest::Forks {
        mind: INSTANCE.into(),
        subject: short(who),
        deciders: deciders.iter().map(|d| short(d)).collect(),
        status: None,
        cursor: None,
        limit: huginn_mind::desk_wire::ForkLimit::new(50).unwrap(),
    }
}

fn framed(message: &CultNetMessage) -> Vec<u8> {
    encode_frame(&encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap()).unwrap()
}

fn desk_bytes(id: &str, request: &DeskRequest) -> Vec<u8> {
    framed(&encode_desk_request(id, request, None).unwrap())
}

fn read_reply(stream: &mut impl Read) -> Option<CultNetMessage> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).ok()?;
    let mut payload = vec![0_u8; u32::from_be_bytes(header) as usize];
    stream.read_exact(&mut payload).ok()?;
    Some(decode_cultnet_message_from_slice(&payload, CultNetWireContract::CultNetSchemaV0).unwrap())
}

fn exchange(path: &Path, bytes: &[u8]) -> Option<CultNetMessage> {
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let _ = stream.write_all(bytes);
    read_reply(&mut stream)
}

fn ask(path: &Path, id: &str, request: &DeskRequest) -> DeskResponse {
    let reply = exchange(path, &desk_bytes(id, request)).expect("the door replied");
    decode_desk_response(&reply).unwrap().1.unwrap_or_else(|failure| panic!("{failure:?}"))
}

fn counted_id(response: DeskResponse) -> String {
    match response {
        DeskResponse::Answered { answer, .. } => {
            assert_eq!(answer.outcome, "Counted", "{answer:?}");
            answer.answer_id
        }
        other => panic!("{other:?}"),
    }
}

fn catalog_request() -> CultNetMessage {
    CultNetMessage::SchemaCatalogRequest { message_id: "c-1".into(), include_schema_json: Some(false), schema_ids: None, kinds: None }
}

#[test]
fn the_binary_serves_both_doors_and_survives_restart() {
    let _serial = serial();
    assert_ne!(my_uid(), 0, "the door refuses uid 0, so this test cannot run as root");
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hash = seed(root.path());
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));

    let mut daemon = spawn(root.path(), &desk, my_uid(), &log);
    let seeded = daemon.documents();
    // The catalog is the two desk schemas.
    let catalog = format!("{:?}", exchange(&desk, &framed(&catalog_request())).expect("a catalog"));
    assert!(catalog.contains("huginn.desk_request.v1") && catalog.contains("huginn.desk_response.v1"), "{catalog}");
    assert!(!catalog.contains("huginn.mind_request.v1"), "{catalog}");
    // The reads answer and store nothing; a decider reading its own list is told the ids.
    let (list, listed_at) = match ask(&desk, "m-r1", &forks_request("x", &["x"])) {
        DeskResponse::Forks { list, as_of } => (list, as_of),
        other => panic!("{other:?}"),
    };
    assert_eq!((list.open_count, list.recorded_count, list.forks.len(), list.stamp.clone()), (1, 0, 1, format!("s{listed_at}")));
    assert_eq!(list.forks[0].card.options_hash, hash);
    let DeskResponse::Fork { entry, as_of } = ask(&desk, "m-r2", &DeskRequest::Fork { mind: INSTANCE.into(), subject: short("x"), deciders: vec![short("x")], fork_id: FORK.into() })
    else {
        panic!("a read of the seeded fork")
    };
    assert_eq!((entry, as_of), (list.forks[0].clone(), listed_at));
    assert_eq!(daemon.documents(), seeded);
    // `authority` is no field of a read: the payload is refused whole and nothing is answered or stored.
    for (operation, request) in [("forks", forks_request("x", &["x"])), ("fork", DeskRequest::Fork { mind: INSTANCE.into(), subject: short("x"), deciders: vec![short("x")], fork_id: FORK.into() })] {
        let mut value = serde_json::to_value(&request).unwrap();
        let variant = if operation == "forks" { "Forks" } else { "Fork" };
        value[variant]["authority"] = serde_json::json!("Operator");
        let message = CultNetMessage::OperationRequest {
            message_id: "m-ra".into(),
            service_id: huginn_mind::desk_wire::DESK_SERVICE_ID.into(),
            operation: operation.into(),
            payload_schema: huginn_mind::desk_wire::DESK_REQUEST_SCHEMA.into(),
            payload_encoding: "messagepack-base64".into(),
            payload: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, rmp_serde::to_vec_named(&value).unwrap()),
            source_runtime_id: None,
            target_runtime_id: None,
        };
        let reply = exchange(&desk, &framed(&message)).expect("the door replied");
        let failure = decode_desk_response(&reply).unwrap().1.unwrap_err();
        assert_eq!(failure.code, "payload-not-a-request", "{operation}");
    }
    assert_eq!(daemon.documents(), seeded);
    // An answer, then a stale one.
    let first = answer("x", &hash, R1);
    assert_eq!(counted_id(ask(&desk, "m-1", &first)), "n1");
    let DeskResponse::Answered { answer: stale, counted_answer_id, .. } = ask(&desk, "m-2", &answer("x", &"0".repeat(64), R2)) else {
        panic!("a stale answer is stored")
    };
    assert_eq!((stale.code, counted_answer_id.as_deref()), (Some(ForkAnswerCode::StaleFork), Some("n1")));
    let stored = daemon.documents();
    assert_eq!(stored, seeded + 2);
    // A subject the call's deciders do not name reads both answers, the first with its choice and the stale one with its code, and is told no account id.
    let DeskResponse::Fork { entry, .. } =
        ask(&desk, "m-r3", &DeskRequest::Fork { mind: INSTANCE.into(), subject: short("outsider"), deciders: vec![short("y")], fork_id: FORK.into() })
    else {
        panic!("a read")
    };
    let seen: Vec<_> = entry.record.answers.iter().map(|answer| (answer.answer_id.as_str(), answer.answerer.as_deref(), answer.code)).collect();
    assert_eq!(seen, [("n1", None, None), ("n2", None, Some(ForkAnswerCode::StaleFork))]);
    assert_eq!((entry.record.counted_answer_id.as_deref(), entry.record.answerer_count), (Some("n1"), 1));
    assert_eq!((entry.card.deciders.as_slice(), entry.card.decider_count), (&[][..], 1));
    assert_eq!(daemon.documents(), stored);

    // A stop takes the socket with it; the stored answers are the retry's memory.
    daemon.signal("TERM");
    assert!(!desk.exists(), "a stopped daemon unlinks its socket");
    let mut daemon = spawn(root.path(), &desk, my_uid(), &log);
    assert_eq!(daemon.documents(), stored);
    assert_eq!(counted_id(ask(&desk, "m-1", &first)), "n1", "the same request id returns the stored answer");
    assert_eq!(daemon.documents(), stored, "the replay stored nothing");

    // A killed daemon leaves its socket; the next one replaces it.
    daemon.signal("KILL");
    assert!(desk.exists(), "a killed daemon leaves its socket behind");
    let daemon = spawn(root.path(), &desk, my_uid(), &log);
    assert_eq!(counted_id(ask(&desk, "m-1", &first)), "n1");
    assert_eq!(daemon.documents(), stored);
}

#[test]
fn the_binary_refuses_another_uid() {
    let _serial = serial();
    let mine = my_uid();
    assert_ne!(mine, 0);
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hash = seed(root.path());
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));
    let mut daemon = spawn(root.path(), &desk, mine + 1, &log);
    let seeded = daemon.documents();
    assert!(exchange(&desk, &desk_bytes("m-1", &answer("x", &hash, R1))).is_none(), "a refused peer reads EOF");
    assert!(exchange(&desk, &framed(&catalog_request())).is_none(), "not even the catalog");
    assert_eq!(daemon.documents(), seeded);
    daemon.signal("TERM");
    let wanted = format!("desk door refused a peer with uid {mine} (expects {})", mine + 1);
    assert!(daemon.log_text().contains(&wanted), "the log names both uids:\n{}", daemon.log_text());
    // Read-only reopen: the mind is as it was.
    let reopened = Mind::open(root.path(), &INSTANCE.into()).unwrap();
    assert_eq!(reopened.status(huginn_mind::wire::IndexStatus::Current).documents, seeded);
}

/// Soul's probe, from the built binary: a client that is uid 0 inside its own user namespace is
/// the real uid to the kernel, so the door sees the real uid and namespace root cannot pass as root.
#[test]
fn namespace_root_is_seen_as_the_real_uid() {
    let _serial = serial();
    let probe = Command::new("unshare").args(["-r", "true"]).status();
    if !probe.is_ok_and(|status| status.success()) || Command::new("socat").arg("-V").stdout(Stdio::null()).status().is_err() {
        eprintln!("skipped: needs `unshare -r` (user namespaces) and socat");
        return;
    }
    let mine = my_uid();
    assert_ne!(mine, 0);
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    seed(root.path());
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));
    let frame = run.path().join("catalog.frame");
    std::fs::write(&frame, framed(&catalog_request())).unwrap();
    let through_namespace = || {
        Command::new("unshare")
            .args(["-r", "socat", "-t", "2", "-", &format!("UNIX-CONNECT:{}", desk.display())])
            .stdin(std::fs::File::open(&frame).unwrap())
            .output()
            .unwrap()
            .stdout
    };

    let mut allowed = spawn(root.path(), &desk, mine, &log);
    let reply = through_namespace();
    assert!(!reply.is_empty(), "the real uid is the configured one, so namespace root is answered:\n{}", allowed.log_text());
    allowed.signal("TERM");

    let mut other = spawn(root.path(), &desk, mine + 1, &log);
    assert!(through_namespace().is_empty(), "a door configured for another uid answers nobody");
    other.signal("TERM");
    let text = other.log_text();
    assert!(text.contains(&format!("desk door refused a peer with uid {mine} (expects {})", mine + 1)), "{text}");
    assert!(!text.contains("refused a peer with uid 0"), "the door never sees namespace root as 0:\n{text}");
}

// ---- the loop's bounds, from the built binary and from run() in process ----

fn setup() -> (tempfile::TempDir, tempfile::TempDir, String, PathBuf, PathBuf) {
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hash = seed(root.path());
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));
    (root, run, hash, desk, log)
}

/// A peer that sends a valid frame one byte per 300 ms: inside any per-read timeout, so only a
/// per-connection deadline cuts it off. Returns when the door has closed on it, with the bytes sent.
fn drip(desk: &Path, hash: &str) -> std::thread::JoinHandle<(usize, Duration)> {
    let (desk, bytes) = (desk.to_path_buf(), desk_bytes("m-1", &answer("x", hash, R1)));
    std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&desk).unwrap();
        let started = Instant::now();
        let mut sent = 0;
        for byte in bytes.iter().take(60) {
            if stream.write_all(&[*byte]).is_err() {
                break;
            }
            sent += 1;
            std::thread::sleep(Duration::from_millis(300));
        }
        (sent, started.elapsed())
    })
}

/// The bound the door states for one connection is one second, so a dripping peer neither delays
/// another door's answer nor a stop beyond it (the sum is checked with the margin a loaded host needs).
#[test]
fn a_dripping_peer_does_not_delay_the_rudp_door() {
    let _serial = serial();
    let (root, _run, hash, desk, log) = setup();
    let daemon = spawn(root.path(), &desk, my_uid(), &log);
    let dripper = drip(&desk, &hash);
    std::thread::sleep(Duration::from_millis(400));
    let asked = Instant::now();
    assert!(daemon.documents() > 0);
    assert!(asked.elapsed() < Duration::from_millis(2500), "the RUDP whoami waited {:?} behind a drip", asked.elapsed());
    let (sent, lasted) = dripper.join().unwrap();
    assert!(lasted < Duration::from_millis(1800), "the drip was held for {lasted:?} ({sent} bytes), not cut at one second");
}

#[test]
fn a_dripping_peer_does_not_delay_a_stop() {
    let _serial = serial();
    let (root, _run, hash, desk, log) = setup();
    let mut daemon = spawn(root.path(), &desk, my_uid(), &log);
    let dripper = drip(&desk, &hash);
    std::thread::sleep(Duration::from_millis(300));
    let signalled = Instant::now();
    let status = Command::new("kill").args(["-TERM", &daemon.child.id().to_string()]).status().unwrap();
    assert!(status.success());
    while daemon.child.try_wait().unwrap().is_none() {
        assert!(signalled.elapsed() < Duration::from_millis(2500), "the daemon was still held by a drip {:?} after SIGTERM", signalled.elapsed());
        std::thread::sleep(Duration::from_millis(20));
    }
    dripper.join().unwrap();
}

fn ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let rest = &stat[stat.rfind(')').unwrap() + 2..];
    let fields: Vec<&str> = rest.split(' ').collect();
    fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap()
}

/// An idle daemon sleeps between passes: a loop that spins when nothing waits burns the core.
#[test]
fn an_idle_daemon_does_not_spin() {
    let _serial = serial();
    let (root, _run, _hash, desk, log) = setup();
    let mut daemon = spawn(root.path(), &desk, my_uid(), &log);
    std::thread::sleep(Duration::from_millis(1500));
    let before = ticks(daemon.child.id());
    std::thread::sleep(Duration::from_secs(2));
    let burned = ticks(daemon.child.id()) - before;
    // 100 ticks a second: a spinning loop burns about 200 in two seconds, an idle one a handful.
    assert!(burned < 30, "an idle daemon burned {burned} ticks in 2 s");
    daemon.signal("TERM");
}

/// WouldBlock on an idle listener is not an error and is not logged; a real accept error is.
#[test]
fn an_idle_door_logs_no_accept_error_and_a_real_one_is_logged() {
    let _serial = serial();
    let (root, _run, _hash, desk, log) = setup();
    let mut daemon = spawn(root.path(), &desk, my_uid(), &log);
    std::thread::sleep(Duration::from_millis(500));
    assert!(!daemon.log_text().contains("could not accept"), "an idle door logged:\n{}", daemon.log_text());
    // An accept error: the daemon's soft fd limit is 3, stdin, stdout and stderr, so no
    // descriptor number is free whatever else it holds open; then a peer connects.
    let pid = daemon.child.id();
    let limit = |soft: usize| {
        let status = Command::new("prlimit").args([&format!("--pid={pid}"), &format!("--nofile={soft}:4096")]).status();
        status.is_ok_and(|status| status.success())
    };
    if !limit(3) {
        eprintln!("skipped: needs prlimit");
        return;
    }
    let _peer = UnixStream::connect(&desk).unwrap();
    let started = Instant::now();
    while !daemon.log_text().contains("could not accept") && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    limit(1024);
    assert!(daemon.log_text().contains("could not accept"), "an accept error went unlogged:\n{}", daemon.log_text());
    daemon.signal("TERM");
}

/// A pass that served desk connections is followed at once by the next pass, not by the idle
/// sleep: eight queued connections are answered in two passes of four, with no sleep between.
#[test]
fn a_busy_pass_is_not_followed_by_the_idle_sleep() {
    let _serial = serial();
    use huginn_daemon::index::Backoff;
    use huginn_daemon::index::ollama::pool;
    use huginn_daemon::index::qdrant::QdrantIndex;
    use huginn_daemon::serve::{ServeOptions, Started, parse_options, run, startup};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let (root, _run, _hash, desk, _log) = setup();
    let args = [
        "--state-root", root.path().to_str().unwrap(), "--instance", INSTANCE, "--bind", &format!("127.0.0.1:{}", free_port()),
        "--qdrant-url", "http://127.0.0.1:1", "--ollama-url", "http://127.0.0.1:1", "--embedding-model", "a-model",
        "--desk-socket", desk.to_str().unwrap(), "--desk-peer-uid", &my_uid().to_string(),
    ];
    let options = parse_options(args.iter().map(|arg| arg.to_string())).unwrap();
    let embedder = pool(&options.ollama_urls, &options.reserve_ollama_urls, &options.embedding_model);
    let Started { mut daemon, mut hub, registry, desk: mut door } =
        startup(&options, embedder, QdrantIndex::new(&options.qdrant_url), Backoff::default()).unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let loop_stopping = Arc::clone(&stopping);
    let serving = std::thread::spawn(move || {
        let serve = ServeOptions { idle_sleep: Duration::from_millis(600), ..ServeOptions::default() };
        run(&mut daemon, &mut hub, &registry, door.as_mut(), &loop_stopping, &serve)
    });
    std::thread::sleep(Duration::from_millis(100));
    let frame = framed(&catalog_request());
    let mut peers: Vec<UnixStream> = (0..8)
        .map(|_| {
            let mut peer = UnixStream::connect(&desk).unwrap();
            peer.write_all(&frame).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            peer
        })
        .collect();
    let started = Instant::now();
    let answered: Vec<Duration> = peers
        .iter_mut()
        .map(|peer| {
            assert!(read_reply(peer).is_some());
            started.elapsed()
        })
        .collect();
    stopping.store(true, Ordering::SeqCst);
    serving.join().unwrap().unwrap();
    let gap = answered[4] - answered[3];
    assert!(gap < Duration::from_millis(300), "the second batch waited {gap:?}: the loop slept after a busy pass ({answered:?})");
}

// ---- fork_too_large over the real socket ----

fn forks_page(who: &str, cursor: Option<&str>) -> DeskRequest {
    let mut value = serde_json::to_value(forks_request(who, &[who])).unwrap();
    value["Forks"]["cursor"] = cursor.map_or(serde_json::Value::Null, |text| serde_json::json!(text));
    serde_json::from_value(value).unwrap()
}

fn fork_read(who: &str, fork_id: &str) -> DeskRequest {
    DeskRequest::Fork { mind: INSTANCE.into(), subject: short(who), deciders: vec![short(who)], fork_id: fork_id.into() }
}

/// The wire spelling of a not-stored refusal's code, and its text.
fn refusal_of(response: &DeskResponse) -> Option<(String, String)> {
    let DeskResponse::NotStored(refusal) = response else { return None };
    Some((serde_json::to_value(refusal.code).unwrap().as_str().unwrap().to_string(), refusal.error.clone()))
}

/// Three forks, the middle one's record grown over the socket until its card alone is past the page
/// budget. The list names it and its order (`fork_too_large`), the cursor `c<order>` walks on to the
/// older fork and ends at order 0, a READ of it never carries an order or cursor (an entry while it fits the
/// response bound, the fork-only `fork_too_large` past it), and `authority` on a read is still
/// refused.
#[test]
fn a_fork_too_large_to_page_is_named_by_the_list_and_never_by_the_read() {
    let _serial = serial();
    assert_ne!(my_uid(), 0, "the door refuses uid 0, so this test cannot run as root");
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hash = seed(root.path());
    {
        let mut mind = Mind::open(root.path(), &INSTANCE.into()).unwrap();
        let documents = ["Q2", "Q3"]
            .into_iter()
            .map(|label| PipelineDocument::Question(PipelineQuestion {
                campaign: "eureka-state".into(),
                label: label.into(),
                title: Title("Who owns the state?".into()),
                question: "Who owns the state?".into(),
                options: vec![
                    QuestionOption { label: "A".into(), text: "the organ".into() },
                    QuestionOption { label: "B".into(), text: "the client".into() },
                ],
                recommended: "A".into(),
                depends: vec![],
                raised_in: None,
                asked_on: Date("2026-09-16".into()),
            }))
            .collect();
        let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: short("claude"), session: short("session-2"), tool: short("admit"), imported_from: None };
        let outcome = mind.admit(PipelineAdmissionBatch { instance: INSTANCE.into(), provenance, documents }, Utc.with_ymd_and_hms(2026, 9, 16, 12, 5, 0).unwrap());
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));
    let _daemon = spawn(root.path(), &desk, my_uid(), &log);
    let (older, big, newer) = (FORK, "eureka-state:question:Q2", "eureka-state:question:Q3");

    // Notes of about 300 bytes, until the big fork's list page is the refusal.
    let note = "a long reason, ".repeat(20);
    let mut sent = 0_u32;
    let refused = loop {
        let listed = ask(&desk, "m-g", &forks_page("x", Some("c2")));
        if refusal_of(&listed).is_some() {
            break listed;
        }
        assert!(matches!(listed, DeskResponse::Forks { .. }), "{listed:?}");
        sent += 1;
        assert!(sent < 400, "the record does not grow");
        let mut grow = answer("x", &hash, &format!("00000000-0000-4000-8000-{sent:012}"));
        if let DeskRequest::Answer { request, .. } = &mut grow {
            request.fork_id = big.into();
            request.answer = Some(ForkAnswer { choice: None, note: Some(note.clone()) });
        }
        assert!(matches!(ask(&desk, "m-w", &grow), DeskResponse::Answered { .. }));
    };

    // (1) The list names the fork and its order, and nothing else is new in the text.
    let (code, text) = refusal_of(&refused).unwrap();
    assert_eq!(code, "fork_too_large", "{refused:?}");
    assert!(text.contains(big) && text.contains("order 1"), "{text}");

    // (2) The cursor c<order> resumes to the older fork, and order 0 is the end. The newest fork pages
    // alone ahead of it; the big one is named from the first page too.
    let first = ask(&desk, "m-p0", &forks_page("x", None));
    let DeskResponse::Forks { list, .. } = first else { panic!("{first:?}") };
    assert_eq!((list.forks.iter().map(|e| e.card.fork_id.as_str().to_string()).collect::<Vec<_>>(), list.next_cursor.map(|c| c.order())), (vec![newer.to_string()], Some(2)));
    let rest = ask(&desk, "m-p1", &forks_page("x", Some("c1")));
    let DeskResponse::Forks { list, .. } = rest else { panic!("{rest:?}") };
    assert_eq!((list.forks.iter().map(|e| e.card.fork_id.as_str().to_string()).collect::<Vec<_>>(), list.next_cursor.is_none()), (vec![older.to_string()], true));
    assert_eq!(list.forks[0].card.order, 0);

    // (3) A read of the same fork: its whole entry while it fits the response bound.
    let read = ask(&desk, "m-r", &fork_read("x", big));
    let DeskResponse::Fork { entry, .. } = &read else { panic!("a read of the oversize fork is its entry, not a refusal: {read:?}") };
    assert_eq!((entry.card.fork_id.as_str(), entry.card.order), (big, 1));
    // Past DESK_MAX_RESPONSE_BYTES the read is the fork-only fork_too_large (no order, no cursor).
    let mut more = 0_u32;
    let too_big = loop {
        let read = ask(&desk, "m-r", &fork_read("x", big));
        if matches!(read, DeskResponse::NotStored(_)) {
            break read;
        }
        assert!(matches!(read, DeskResponse::Fork { .. }), "{read:?}");
        more += 1;
        assert!(more < 400, "the record does not grow");
        let mut grow = answer("x", &hash, &format!("00000000-0000-4000-8000-{:012}", 1000 + more));
        if let DeskRequest::Answer { request, .. } = &mut grow {
            request.fork_id = big.into();
            request.answer = Some(ForkAnswer { choice: None, note: Some(note.clone()) });
        }
        assert!(matches!(ask(&desk, "m-w", &grow), DeskResponse::Answered { .. }));
    };
    let (code, text) = refusal_of(&too_big).unwrap();
    assert!(code == "fork_too_large" && text.contains(big) && !text.contains("order"), "{text}");
    assert!(refusal_of(&ask(&desk, "m-g", &forks_page("x", Some("c2")))).is_some_and(|(code, _)| code == "fork_too_large"));

    // (4) `authority` on a read is still refused whole (the contract's request_invalid is the door's payload-not-a-request).
    for (operation, request) in [("forks", forks_request("x", &["x"])), ("fork", fork_read("x", big))] {
        let mut value = serde_json::to_value(&request).unwrap();
        value[if operation == "forks" { "Forks" } else { "Fork" }]["authority"] = serde_json::json!("Operator");
        let message = CultNetMessage::OperationRequest {
            message_id: "m-ra".into(),
            service_id: huginn_mind::desk_wire::DESK_SERVICE_ID.into(),
            operation: operation.into(),
            payload_schema: huginn_mind::desk_wire::DESK_REQUEST_SCHEMA.into(),
            payload_encoding: "messagepack-base64".into(),
            payload: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, rmp_serde::to_vec_named(&value).unwrap()),
            source_runtime_id: None,
            target_runtime_id: None,
        };
        let reply = exchange(&desk, &framed(&message)).expect("the door replied");
        assert_eq!(decode_desk_response(&reply).unwrap().1.unwrap_err().code, "payload-not-a-request", "{operation}");
    }
}

/// Q1 (order 0), Q2, Q3 held; the daemon up. Returns what the tests need.
fn three_forks() -> (tempfile::TempDir, tempfile::TempDir, String, PathBuf, Daemon) {
    let (root, run) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hash = seed(root.path());
    {
        let mut mind = Mind::open(root.path(), &INSTANCE.into()).unwrap();
        let documents = ["Q2", "Q3"]
            .into_iter()
            .map(|label| PipelineDocument::Question(PipelineQuestion {
                campaign: "eureka-state".into(),
                label: label.into(),
                title: Title("Who owns the state?".into()),
                question: "Who owns the state?".into(),
                options: vec![
                    QuestionOption { label: "A".into(), text: "the organ".into() },
                    QuestionOption { label: "B".into(), text: "the client".into() },
                ],
                recommended: "A".into(),
                depends: vec![],
                raised_in: None,
                asked_on: Date("2026-09-16".into()),
            }))
            .collect();
        let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: short("claude"), session: short("session-2"), tool: short("admit"), imported_from: None };
        let outcome = mind.admit(PipelineAdmissionBatch { instance: INSTANCE.into(), provenance, documents }, Utc.with_ymd_and_hms(2026, 9, 16, 12, 5, 0).unwrap());
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }
    let (desk, log) = (run.path().join("desk.sock"), run.path().join("daemon.log"));
    let daemon = spawn(root.path(), &desk, my_uid(), &log);
    (root, run, hash, desk, daemon)
}

/// Grows `fork`'s record over the socket until the list page at `cursor` is the refusal; returns it.
fn grown_until_refused(desk: &Path, hash: &str, fork: &str, cursor: Option<&str>) -> DeskResponse {
    let note = "a long reason, ".repeat(20);
    for sent in 1..400_u32 {
        let listed = ask(desk, "m-g", &forks_page("x", cursor));
        if refusal_of(&listed).is_some() {
            return listed;
        }
        assert!(matches!(listed, DeskResponse::Forks { .. }), "{listed:?}");
        let mut grow = answer("x", hash, &format!("00000000-0000-4000-8000-{sent:012}"));
        if let DeskRequest::Answer { request, .. } = &mut grow {
            request.fork_id = fork.into();
            request.answer = Some(ForkAnswer { choice: None, note: Some(note.clone()) });
        }
        assert!(matches!(ask(desk, "m-w", &grow), DeskResponse::Answered { .. }));
    }
    panic!("the record does not grow")
}

fn page_ids(response: DeskResponse) -> (Vec<String>, Option<u64>) {
    let DeskResponse::Forks { list, .. } = response else { panic!("{response:?}") };
    (list.forks.iter().map(|e| e.card.fork_id.as_str().to_string()).collect(), list.next_cursor.map(|c| c.order()))
}

/// The oldest fork (order 0) oversize: the list names it and its order, and says it is the end, not a cursor.
#[test]
fn the_oldest_fork_too_large_says_the_list_ends_and_names_no_cursor() {
    let _serial = serial();
    let (_root, _run, hash, desk, _daemon) = three_forks();
    let refused = grown_until_refused(&desk, &hash, FORK, Some("c1"));
    let (code, text) = refusal_of(&refused).unwrap();
    assert_eq!(code, "fork_too_large");
    assert!(text.contains(FORK) && text.contains("order 0") && !text.contains("c0"), "{text}");
}

/// The newest fork (order 2) oversize: the first page is the refusal, and c2 resumes to the older forks.
#[test]
fn the_newest_fork_too_large_is_named_and_its_cursor_resumes_to_the_older_forks() {
    let _serial = serial();
    let (_root, _run, hash, desk, _daemon) = three_forks();
    let newest = "eureka-state:question:Q3";
    let refused = grown_until_refused(&desk, &hash, newest, None);
    let (code, text) = refusal_of(&refused).unwrap();
    assert_eq!(code, "fork_too_large");
    assert!(text.contains(newest) && text.contains("order 2") && text.contains("c2"), "{text}");
    let rest = page_ids(ask(&desk, "m-p", &forks_page("x", Some("c2"))));
    assert_eq!(rest, (vec!["eureka-state:question:Q2".to_string(), FORK.to_string()], None));
}

/// Grows `fork` over the socket until a READ of it no longer fits the reply cap; returns the raw reply.
fn grown_until_read_overflows(desk: &Path, hash: &str, fork: &str) -> CultNetMessage {
    let note = "a long reason, ".repeat(60);
    for sent in 1..400_u32 {
        let reply = exchange(desk, &desk_bytes("m-read", &fork_read("x", fork))).expect("the door replied");
        if !matches!(decode_desk_response(&reply).unwrap().1, Ok(DeskResponse::Fork { .. })) {
            return reply;
        }
        let mut grow = answer("x", hash, &format!("00000000-0000-4000-8000-{sent:012}"));
        if let DeskRequest::Answer { request, .. } = &mut grow {
            request.fork_id = fork.into();
            request.answer = Some(ForkAnswer { choice: None, note: Some(note.clone()) });
        }
        assert!(matches!(ask(desk, "m-w", &grow), DeskResponse::Answered { .. }));
    }
    panic!("the record does not grow")
}

/// A READ of a fork whose entry is past the reply cap is `fork_too_large` naming the fork, with no
/// order and no cursor; a fitting fork still reads; the list's refusal is unchanged; the generic
/// response-too-large stays for every other reply.
#[test]
fn a_read_of_an_oversize_fork_is_fork_too_large_naming_only_the_fork() {
    let _serial = serial();
    let (_root, _run, hash, desk, _daemon) = three_forks();
    let big = "eureka-state:question:Q2";
    let reply = grown_until_read_overflows(&desk, &hash, big);
    let (id, response) = decode_desk_response(&reply).unwrap();
    assert_eq!(id, "m-read");
    let (code, text) = refusal_of(&response.unwrap()).expect("a not-stored refusal");
    assert_eq!(code, "fork_too_large");
    assert!(text.contains(big) && !text.contains("order") && !text.contains("cursor") && !text.contains(" c1"), "{text}");
    // Fitting forks read, the others untouched.
    for fork in [FORK, "eureka-state:question:Q3"] {
        assert!(matches!(ask(&desk, "m-ok", &fork_read("x", fork)), DeskResponse::Fork { .. }));
    }
    // The list still names the fork and its order, and the cursor goes on.
    let listed = ask(&desk, "m-l", &forks_page("x", Some("c2")));
    let (code, text) = refusal_of(&listed).unwrap();
    assert_eq!(code, "fork_too_large");
    assert!(text.contains(big) && text.contains("order 1") && text.contains("c1"), "{text}");
}

/// The encoded size of the reply to a read of `fork` sent under `id`, and the reply. The door echoes the id,
/// so its length moves the reply one byte at a time where a record's growth moves it in base64 steps of four.
fn read_size(desk: &Path, fork: &str, id: &str) -> (usize, CultNetMessage) {
    let mut stream = UnixStream::connect(desk).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.write_all(&desk_bytes(id, &fork_read("x", fork))).unwrap();
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).unwrap();
    let mut payload = vec![0_u8; u32::from_be_bytes(header) as usize];
    stream.read_exact(&mut payload).unwrap();
    (payload.len(), decode_cultnet_message_from_slice(&payload, CultNetWireContract::CultNetSchemaV0).unwrap())
}

/// The reply cap is exact on a READ: a reply of exactly DESK_MAX_RESPONSE_BYTES is the entry, and the same
/// read one byte longer is `fork_too_large` naming the fork. The record is grown to within 250 bytes of the
/// cap, then the echoed message id is padded byte by byte to land on it.
#[test]
fn a_read_reply_of_exactly_the_cap_is_the_entry_and_one_byte_more_is_fork_too_large() {
    let _serial = serial();
    let cap = huginn_mind::desk_wire::DESK_MAX_RESPONSE_BYTES;
    let (_root, _run, hash, desk, _daemon) = three_forks();
    let big = "eureka-state:question:Q2";
    let mut sent = 0_u32;
    loop {
        let (size, reply) = read_size(&desk, big, "m-size");
        assert!(matches!(decode_desk_response(&reply).unwrap().1, Ok(DeskResponse::Fork { .. })), "grew past the cap at {size}");
        let remaining = cap - size;
        if remaining <= 250 {
            break;
        }
        // An answer costs about 513 plus 4/3 of its note (base64); aim the last one at 170 bytes short of the cap.
        let note = if remaining > 5000 { 2000 } else { (remaining - 170 - 513) * 3 / 4 };
        sent += 1;
        let mut request = answer("x", &hash, &format!("00000000-0000-4000-8000-{sent:012}"));
        if let DeskRequest::Answer { request, .. } = &mut request {
            request.fork_id = big.into();
            request.answer = Some(ForkAnswer { choice: None, note: Some("a".repeat(note)) });
        }
        assert!(matches!(ask(&desk, "m-w", &request), DeskResponse::Answered { .. }));
    }
    let sizes: Vec<(usize, String)> = (0..250).map(|pad| format!("m-size{}", "x".repeat(pad))).map(|id| (read_size(&desk, big, &id).0, id)).collect();
    let (at_cap, id) = sizes.iter().find(|(size, _)| *size == cap).expect("a padded id lands exactly on the cap").clone();
    let (_, reply) = read_size(&desk, big, &id);
    let response = decode_desk_response(&reply).unwrap().1;
    assert!(matches!(response, Ok(DeskResponse::Fork { .. })), "{at_cap} bytes: {response:?}");
    let (over, reply) = read_size(&desk, big, &format!("{id}x"));
    let (_, response) = decode_desk_response(&reply).unwrap();
    let (code, text) = refusal_of(&response.unwrap()).expect("a not-stored refusal");
    assert!(code == "fork_too_large" && text.contains(big) && !text.contains("cursor") && !text.contains("order"), "{text}");
    assert!(over < cap, "the refusal itself is small: {over}");
}

/// A stale answer is STORED as a refusal (`stale_fork`), never counted, and its replay returns the same
/// stored refusal from the door even once the fork is too large to read: nothing new is stored, and the
/// reply is neither `fork_too_large` nor `ResponseTooLarge` (a stored record carries no card, so it is small).
#[test]
fn a_stale_answer_is_stored_refused_and_its_replay_survives_the_fork_growing_too_large() {
    let _serial = serial();
    let (_root, _run, hash, desk, daemon) = three_forks();
    let fork = "eureka-state:question:Q2";
    let mut stale = answer("x", &"0".repeat(64), "00000000-0000-4000-8000-0000ffff0001");
    if let DeskRequest::Answer { request, .. } = &mut stale {
        request.fork_id = fork.into();
    }
    let before = daemon.documents();
    let DeskResponse::Answered { answer: first, counted_answer_id, .. } = ask(&desk, "m-s1", &stale) else { panic!("a stale answer is stored") };
    assert_eq!((first.code, first.outcome.as_str(), counted_answer_id), (Some(ForkAnswerCode::StaleFork), "Refused", None));
    let stored = daemon.documents();
    assert_eq!(stored, before + 1, "the refusal is a stored record");
    let DeskResponse::Fork { entry, .. } = ask(&desk, "m-s2", &fork_read("x", fork)) else { panic!("a read") };
    assert!(entry.record.answers.iter().any(|a| a.answer_id == first.answer_id && a.code == Some(ForkAnswerCode::StaleFork)));
    assert_eq!(entry.record.counted_answer_id, None);
    // The fork grows until it cannot be read.
    let reply = grown_until_read_overflows(&desk, &hash, fork);
    let (_, response) = decode_desk_response(&reply).unwrap();
    assert_eq!(refusal_of(&response.unwrap()).unwrap().0, "fork_too_large");
    let grown = daemon.documents();
    // The identical request replays the stored refusal: same answer id, nothing stored.
    let DeskResponse::Answered { answer: replay, counted_answer_id, .. } = ask(&desk, "m-s1", &stale) else { panic!("the replay is the stored answer") };
    assert_eq!((replay.answer_id.clone(), replay.code, replay.outcome.as_str()), (first.answer_id.clone(), Some(ForkAnswerCode::StaleFork), "Refused"));
    assert_ne!(counted_answer_id.as_deref(), Some(replay.answer_id.as_str()), "the refusal is never the counted answer");
    assert_eq!(daemon.documents(), grown, "the replay stored nothing");
}
