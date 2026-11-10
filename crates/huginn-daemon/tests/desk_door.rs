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
    // Reads are not built: Unavailable, nothing stored.
    let forks = DeskRequest::Forks { mind: INSTANCE.into(), subject: short("x"), deciders: vec![short("x")] };
    let fork = DeskRequest::Fork { mind: INSTANCE.into(), subject: short("x"), deciders: vec![short("x")], fork_id: FORK.into() };
    for request in [forks, fork] {
        let DeskResponse::Refused(huginn_mind::MindRefusal::Unavailable { detail }) = ask(&desk, "m-r", &request) else {
            panic!("a read is refused unavailable")
        };
        assert!(detail.contains("desk reads not built"), "{detail}");
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
