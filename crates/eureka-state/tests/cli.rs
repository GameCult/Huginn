//! The `huginn` binary as an agent drives it: a real process, its stdout, its
//! stderr and its exit code, against a real daemon in-process or a scripted
//! server where the daemon cannot be made to misbehave.

use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use cultnet_rs::Selection;
use huginn_mind::envelope::{OperationFailure, decode_request, encode_failure, encode_response};
use huginn_mind::eureka_pipeline::{
    CodeLocation, CommitRange, CutVerification, Date, Evidence, EvidenceKind, MutationRecord, OrgRepo, PipelineCutReport,
    PipelineCutSpec, PipelineDocument, PipelineKind, PipelineRef, PipelineRuling, Promise, ReportCommit, RulingAuthority, Short,
    StructuralDelta, pipeline_key,
};
use huginn_mind::{
    AdmissionFacts, HuginnMindRequest, HuginnMindResponse, IndexStatus, MindStatus, PipelineAdmissionOutcome, PipelineDocumentView,
    PipelinePageItems, PipelineProvenance, PipelineSelectionPage, PipelineStatus,
};
use serde_json::{Value, json};
use tempfile::TempDir;

mod common;
use common::*;

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn run(env: &[(&str, &str)], args: &[&str], stdin: Option<&str>) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_huginn"));
    command
        .env_remove("EUREKA_INSTANCE")
        .env_remove("HUGINN_ENDPOINT")
        .envs(env.iter().copied())
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("the binary starts");
    if let Some(text) = stdin {
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    Run {
        code: output.status.code().expect("exited, not killed"),
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
    }
}

/// Configured for the test instance against a daemon at `addr`.
fn at(addr: SocketAddr, args: &[&str], stdin: Option<&str>) -> Run {
    run(&[("EUREKA_INSTANCE", INSTANCE), ("HUGINN_ENDPOINT", &format!("rudp://{addr}"))], args, stdin)
}

/// A loopback UDP port nothing listens on.
fn closed_port() -> SocketAddr {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

fn write(dir: &TempDir, name: &str, value: &Value) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, value.to_string()).unwrap();
    path.to_str().unwrap().to_string()
}

fn campaign_json() -> Value {
    serde_json::to_value(campaign()).unwrap()
}

fn live() -> Server {
    let (root, daemon) = mind(vec![vec![stewardship()]]);
    serve(root, daemon)
}

fn admit_args(file: &str) -> Vec<&str> {
    vec!["admit", "--faculty", "Hands", "--agent", "claude", "--session", "session-7", file]
}

fn delta() -> StructuralDelta {
    StructuralDelta {
        lines_added: 0,
        lines_removed: 0,
        dependencies_added: vec![],
        dependencies_removed: vec![],
        formats_added: vec![],
        formats_removed: vec![],
        targets_added: vec![],
        targets_removed: vec![],
    }
}

fn spec(cut: &str) -> Value {
    serde_json::to_value(PipelineDocument::CutSpec(PipelineCutSpec {
        campaign: slug(CAMPAIGN),
        cut: cut.into(),
        revision: 1,
        title: "A cut".into(),
        repo: OrgRepo(REPO.into()),
        branch: "codex/eureka-pipeline-state".into(),
        base: sha(),
        depends_on: vec![],
        first: vec![],
        deletes: vec![],
        keeps_moves: vec![],
        adds: vec![],
        file_changes: vec![],
        reads: vec![],
        authority_map: None,
        verification: CutVerification { builds: vec![], tests: vec![], negative: vec![], operator: vec![] },
        estimate: delta(),
        rulings: vec![],
        questions: vec![],
    }))
    .unwrap()
}

fn report(cut: &str) -> Value {
    serde_json::to_value(PipelineDocument::CutReport(PipelineCutReport {
        campaign: slug(CAMPAIGN),
        cut_spec: Short(format!("{CAMPAIGN}:cut_spec:cut-{cut}.r1")),
        attempt: 1,
        repo: OrgRepo(REPO.into()),
        branch: "codex/eureka-pipeline-state".into(),
        commits: vec![ReportCommit { sha: sha(), subject: "Land the cut".into(), builds: true }],
        range: CommitRange { base: sha(), head: sha() },
        verification: vec![Evidence { kind: EvidenceKind::Test, locator: "cargo test".into(), result: "ok".into() }],
        mutations: vec![MutationRecord {
            label: "M1".into(),
            rule: "the rule".into(),
            location: CodeLocation { path: Short("crates/x.rs".into()), line: 1, end_line: None, symbol: None },
            before: "before".into(),
            after: "after".into(),
            commit: sha(),
            failed_as_expected: true,
        }],
        deviations: vec![],
        forks: vec![],
        structural_delta: delta(),
        landed_names: vec![],
        undone: vec![],
        promises: vec![Promise { label: "P1".into(), text: "One derived key per document.".into() }],
    }))
    .unwrap()
}

fn ruling(quote: Option<&str>) -> PipelineDocument {
    PipelineDocument::Ruling(PipelineRuling {
        campaign: slug(CAMPAIGN),
        label: "R1".into(),
        title: "An instance owns its mind".into(),
        answers: None,
        choice: None,
        ruling: "An instance owns its mind.".into(),
        operator_quote: quote.map(Into::into),
        ruled_on: Date("2026-10-10".into()),
        precedents: vec![],
        authority: RulingAuthority::Standing,
    })
}

fn empty_page() -> PipelineSelectionPage {
    PipelineSelectionPage { matched: 0, as_of: 1, next: None, items: PipelinePageItems::Headers(vec![]), edges: None }
}

fn answer(operation: &str, response: &HuginnMindResponse) -> cultnet_rs::CultNetMessage {
    encode_response("eureka-state-call", operation, response, "scripted").unwrap()
}

/// A scripted server that records every request it is sent and answers a
/// query with an empty page.
fn recording() -> (Server, Arc<Mutex<Vec<HuginnMindRequest>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let server = scripted(move |message| {
        let (_, request) = decode_request(message).expect("a mind request");
        sink.lock().unwrap().push(request);
        answer("query", &HuginnMindResponse::Query(empty_page()))
    });
    (server, seen)
}

/// The selection a command sent, and its semantic part.
fn sent(args: &[&str]) -> (Selection, Option<(String, u32)>) {
    let (server, seen) = recording();
    let ran = at(server.addr, args, None);
    assert_eq!(ran.code, 0, "{args:?}: {}", ran.err);
    assert!(ran.out.contains("matched 0 as_of 1 next -"), "{}", ran.out);
    let seen = seen.lock().unwrap();
    let [HuginnMindRequest::Query { instance, selection, semantic }] = &seen[..] else {
        panic!("one query expected: {seen:?}");
    };
    assert_eq!(instance.0, INSTANCE);
    (selection.clone(), semantic.as_ref().map(|asked| (asked.text.0.clone(), asked.top_k)))
}

/// The page's literal as the selection it spells, with the CLI's default page.
fn page_literal(literal: Value) -> Selection {
    let mut selection: Selection = serde_json::from_value(literal).unwrap();
    selection.limit = Some(50);
    selection
}

fn counting(replies: HuginnMindResponse, operation: &'static str) -> (Server, Arc<Mutex<usize>>) {
    let count = Arc::new(Mutex::new(0));
    let counter = Arc::clone(&count);
    let server = scripted(move |_| {
        *counter.lock().unwrap() += 1;
        answer(operation, &replies)
    });
    (server, count)
}

#[test]
fn without_configuration_whoami_reports_it_and_every_other_command_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&dir, "campaign.json", &json!([campaign_json()]));

    let whoami = run(&[], &["whoami"], None);
    assert_eq!(whoami.code, 2);
    assert!(whoami.out.contains("instance unset\nendpoint unset\nclient "), "{}", whoami.out);
    assert!(whoami.out.contains("reachable no"), "{}", whoami.out);
    assert!(whoami.err.starts_with("Misconfigured: EUREKA_INSTANCE is not set"), "{}", whoami.err);

    for args in [
        vec!["view", "eureka-state:campaign:self"],
        vec!["query", "{}"],
        vec!["rulings", "eureka-state"],
        admit_args(&file),
    ] {
        let ran = run(&[], &args, None);
        assert_eq!(ran.code, 2, "{args:?}");
        assert!(ran.err.starts_with("Misconfigured:"), "{args:?}: {}", ran.err);
        assert_eq!(ran.out, "", "{args:?}");
    }
}

#[test]
fn a_bad_environment_is_misconfigured_with_its_own_reason() {
    let cases: [(&[(&str, &str)], &str); 4] = [
        (&[("HUGINN_ENDPOINT", "rudp://127.0.0.1:1")], "EUREKA_INSTANCE is not set"),
        (&[("EUREKA_INSTANCE", "Not A Slug"), ("HUGINN_ENDPOINT", "rudp://127.0.0.1:1")], "EUREKA_INSTANCE is not a valid"),
        (&[("EUREKA_INSTANCE", "eureka")], "HUGINN_ENDPOINT is not set"),
        (&[("EUREKA_INSTANCE", "eureka"), ("HUGINN_ENDPOINT", "127.0.0.1:1")], "HUGINN_ENDPOINT is not rudp://"),
    ];
    for (env, expected) in cases {
        let ran = run(env, &["whoami"], None);
        assert_eq!(ran.code, 2, "{expected}");
        assert!(ran.err.starts_with("Misconfigured: "), "{}", ran.err);
        assert!(ran.err.contains(expected), "{expected}: {}", ran.err);
    }
    let named = run(&[("EUREKA_INSTANCE", "eureka"), ("HUGINN_ENDPOINT", "rudp://not-an-address")], &["whoami"], None);
    assert_eq!(named.code, 2);
    assert!(named.out.contains("endpoint rudp://not-an-address"), "{}", named.out);
    assert!(named.out.contains("instance eureka"), "{}", named.out);
}

#[test]
fn whoami_reads_the_live_mind() {
    let server = live();
    let ran = at(server.addr, &["whoami"], None);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(lines[0], format!("instance {INSTANCE}"));
    assert_eq!(lines[1], format!("endpoint rudp://{}", server.addr));
    assert!(lines[2].starts_with("client "), "{lines:?}");
    assert_eq!(lines[3], "reachable yes");
    assert_eq!(lines[4], "documents 2 receipts 2 index Current epoch epiphany.pipeline.epoch.v2");
    assert_eq!(lines.len(), 5);
    assert_eq!(ran.err, "");
}

#[test]
fn an_index_that_is_behind_shows_its_count() {
    let status = MindStatus {
        instance: slug(INSTANCE),
        schema_epoch: "epoch".into(),
        documents: 7,
        receipts: 9,
        index: IndexStatus::Behind { pending: 3 },
    };
    let (server, _) = counting(HuginnMindResponse::Whoami(status), "whoami");
    let ran = at(server.addr, &["whoami"], None);
    assert_eq!(ran.code, 0);
    assert!(ran.out.contains("documents 7 receipts 9 index Behind pending=3 epoch epoch"), "{}", ran.out);
}

#[test]
fn admit_round_trips_and_is_idempotent() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let file = write(&dir, "campaign.json", &json!([campaign_json()]));

    let first = at(server.addr, &admit_args(&file), None);
    assert_eq!(first.code, 0, "{}", first.err);
    let lines: Vec<&str> = first.out.lines().collect();
    assert!(lines[0].starts_with("Committed mind-commit-"), "{}", first.out);
    assert!(lines[0].contains(" at "), "{}", first.out);
    assert_eq!(lines[1], format!("wrote {CAMPAIGN}:campaign:self"));
    assert_eq!(lines[2], "read back 1 equal");
    assert_eq!(lines.len(), 3, "{}", first.out);

    let again = at(server.addr, &admit_args(&file), None);
    assert_eq!(again.code, 0, "{}", again.err);
    let lines: Vec<&str> = again.out.lines().collect();
    assert!(lines[0].starts_with("AlreadyAdmitted mind-commit-"), "{}", again.out);
    assert_eq!(lines[1], "read back 1 equal");

    let id = format!("{CAMPAIGN}:campaign:self");
    let viewed = at(server.addr, &["view", &id], None);
    assert_eq!(viewed.code, 0, "{}", viewed.err);
    let lines: Vec<&str> = viewed.out.lines().collect();
    assert_eq!(lines[0], format!("{id} InForce"));
    assert!(lines[1].starts_with("admitted "), "{}", viewed.out);
    assert!(lines[1].contains(" ordinal 3 Hands/claude session session-7 tool huginn receipt mind-commit-"), "{}", lines[1]);
    let shown: Value = serde_json::from_str(&lines[2..].join("\n")).unwrap();
    assert_eq!(shown, campaign_json());

    let raw = at(server.addr, &["view", &id, "--raw"], None);
    assert_eq!(raw.code, 0);
    assert_eq!(serde_json::from_str::<Value>(&raw.out).unwrap(), campaign_json());
    assert!(raw.out.starts_with('{'), "{}", raw.out);

    let fed_back = at(server.addr, &admit_args("-"), Some(&raw.out));
    assert_eq!(fed_back.code, 0, "{}", fed_back.err);
    assert!(fed_back.out.starts_with("AlreadyAdmitted "), "{}", fed_back.out);

    let status = at(server.addr, &["whoami"], None);
    assert!(status.out.contains("documents 3 receipts 3"), "{}", status.out);
}

#[test]
fn unknown_keys_are_refused_by_path_and_never_by_value() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let mut nested = campaign_json();
    nested["value"]["target_doc"]["pathh"] = json!("CANARY-NESTED");
    let mut top = campaign_json();
    top["surplus"] = json!("CANARY-TOP");
    let file = write(&dir, "bad.json", &json!([nested, top]));

    let ran = at(server.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 2);
    assert_eq!(ran.out, "");
    assert!(ran.err.starts_with("InvalidInput: "), "{}", ran.err);
    assert!(ran.err.contains("documents[0].value.target_doc.pathh"), "{}", ran.err);
    assert!(ran.err.contains("documents[1].surplus"), "{}", ran.err);
    assert!(!ran.err.contains("CANARY"), "{}", ran.err);

    let status = at(server.addr, &["whoami"], None);
    assert!(status.out.contains("receipts 2 "), "nothing was sent: {}", status.out);

    let selection = at(server.addr, &["query", r#"{"limt": 5}"#], None);
    assert_eq!(selection.code, 2);
    assert!(selection.err.contains("selection.limt"), "{}", selection.err);
}

#[test]
fn an_explicitly_empty_unknown_key_is_not_a_loss() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let mut doc = campaign_json();
    doc["value"]["ghost"] = Value::Null;
    doc["value"]["ghosts"] = json!([]);
    doc["value"]["ghost_map"] = json!({});
    doc["value"]["ghost_text"] = json!("");
    let file = write(&dir, "ghosts.json", &json!([doc]));
    let ran = at(server.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(ran.out.contains("read back 1 equal"), "{}", ran.out);
}

#[test]
fn a_falsy_unknown_value_is_still_a_loss() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let mut doc = campaign_json();
    doc["value"]["zero"] = json!(0);
    doc["value"]["no"] = json!(false);
    let file = write(&dir, "falsy.json", &json!([doc]));
    let ran = at(server.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 2);
    assert!(ran.err.contains("documents[0].value.zero") && ran.err.contains("documents[0].value.no"), "{}", ran.err);
}

#[test]
fn a_document_that_does_not_decode_is_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (json!([{ "kind": "nonsense", "value": {} }]), "documents[0] does not decode"),
        (json!("text"), "JSON array"),
        (json!([]), "1 to 64"),
        (json!((0..65).map(|_| campaign_json()).collect::<Vec<_>>()), "1 to 64"),
    ];
    for (n, (value, expected)) in cases.into_iter().enumerate() {
        let file = write(&dir, &format!("case{n}.json"), &value);
        let ran = at(closed_port(), &admit_args(&file), None);
        assert_eq!(ran.code, 2, "{expected}: {}", ran.err);
        assert!(ran.err.starts_with("InvalidInput: "), "{}", ran.err);
        assert!(ran.err.contains(expected), "{expected}: {}", ran.err);
    }
    let broken = at(closed_port(), &admit_args("-"), Some("{ not json"));
    assert_eq!(broken.code, 2);
    assert!(broken.err.contains("not JSON"), "{}", broken.err);
    let missing = at(closed_port(), &admit_args("/no/such/file.json"), None);
    assert_eq!(missing.code, 2);
    assert!(missing.err.contains("cannot read /no/such/file.json"), "{}", missing.err);
}

fn view_of(document: &PipelineDocument) -> PipelineDocumentView {
    PipelineDocumentView {
        id: PipelineRef { kind: document.kind(), id: Short(pipeline_key(document).unwrap()) },
        document: document.clone(),
        admission: AdmissionFacts {
            receipt_id: "mind-commit-x".into(),
            admitted_at: "2026-10-10T00:00:00Z".into(),
            provenance: PipelineProvenance {
                faculty: huginn_mind::Faculty::Hands,
                agent: Short("a".into()),
                session: Short("s".into()),
                tool: Short("huginn".into()),
            },
            ordinal: 1,
        },
        status: PipelineStatus::InForce,
    }
}

/// Admits `Committed` and answers the keys query with `stored`.
fn committing(stored: Vec<PipelineDocumentView>) -> Server {
    scripted(move |message| {
        let (_, request) = decode_request(message).expect("a mind request");
        match request {
            HuginnMindRequest::Admit(_) => answer(
                "admit",
                &HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed {
                    receipt_id: "mind-commit-x".into(),
                    committed_at: "2026-10-10T00:00:00Z".into(),
                    writes: vec![],
                }),
            ),
            HuginnMindRequest::Query { selection, .. } => {
                assert_eq!(selection.projection, "document");
                assert_eq!(selection.limit, Some(u32::try_from(selection.keys.as_ref().unwrap().len()).unwrap()));
                let page = PipelineSelectionPage {
                    matched: u32::try_from(stored.len()).unwrap(),
                    as_of: 2,
                    next: None,
                    items: PipelinePageItems::Documents(stored.clone()),
                    edges: None,
                };
                answer("query", &HuginnMindResponse::Query(page))
            }
            other => panic!("unexpected {other:?}"),
        }
    })
}

#[test]
fn a_field_the_mind_dropped_is_reported_with_the_receipt_printed() {
    let dir = tempfile::tempdir().unwrap();
    let sent = ruling(Some("CANARY-QUOTE"));
    let file = write(&dir, "ruling.json", &json!([sent]));
    let key = pipeline_key(&sent).unwrap();

    let server = committing(vec![view_of(&ruling(None))]);
    let ran = at(server.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 5);
    assert!(ran.out.starts_with("Committed mind-commit-x at 2026-10-10T00:00:00Z"), "{}", ran.out);
    assert!(!ran.out.contains("read back"), "{}", ran.out);
    assert!(ran.err.starts_with(&format!("ReadBackDiffers: {key}: value.operator_quote;")), "{}", ran.err);
    assert!(ran.err.contains("supersede"), "{}", ran.err);

    let missing = committing(vec![]);
    let ran = at(missing.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 5);
    assert!(ran.err.contains(&format!("{key}: missing from the read-back")), "{}", ran.err);

    let equal = committing(vec![view_of(&sent)]);
    let ran = at(equal.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(ran.out.ends_with("read back 1 equal\n"), "{}", ran.out);
}

#[test]
fn a_difference_deep_in_a_list_names_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let sent = serde_json::from_value::<PipelineDocument>(spec("A")).unwrap();
    let mut stored = sent.clone();
    if let PipelineDocument::CutSpec(value) = &mut stored {
        value.rulings = vec![Short("x:ruling:R1".into())];
        value.title = "Another".into();
    }
    let file = write(&dir, "spec.json", &json!([sent]));
    let server = committing(vec![view_of(&stored)]);
    let ran = at(server.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 5);
    assert!(ran.err.contains("value.rulings[0]") || ran.err.contains("value.rulings"), "{}", ran.err);
    assert!(ran.err.contains("value.title"), "{}", ran.err);
}

#[test]
fn exit_codes_follow_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&dir, "campaign.json", &json!([campaign_json()]));

    // Nothing listening: Unavailable, naming the endpoint, whoami included.
    let port = closed_port();
    let down = at(port, &["view", "eureka-state:campaign:self"], None);
    assert_eq!(down.code, 3);
    assert!(down.err.starts_with("Unavailable: "), "{}", down.err);
    assert!(down.err.contains(&format!("rudp://{port}")), "{}", down.err);
    let whoami = at(port, &["whoami"], None);
    assert_eq!(whoami.code, 3);
    assert!(whoami.out.contains("reachable no"), "{}", whoami.out);

    // The daemon rejecting the envelope.
    let rejecting = scripted(|_| {
        encode_failure(
            "eureka-state-call",
            "view",
            &OperationFailure { code: "wrong-service".into(), message: "not huginn.mind".into() },
            "scripted",
        )
    });
    let rejected = at(rejecting.addr, &["view", "eureka-state:campaign:self"], None);
    assert_eq!(rejected.code, 4);
    assert!(rejected.err.starts_with("Rejected: "), "{}", rejected.err);
    assert!(rejected.err.contains("wrong-service"), "{}", rejected.err);

    // An answer of another operation's shape.
    let status = MindStatus {
        instance: slug(INSTANCE),
        schema_epoch: "e".into(),
        documents: 0,
        receipts: 0,
        index: IndexStatus::Current,
    };
    let (odd, _) = counting(HuginnMindResponse::Whoami(status), "view");
    let internal = at(odd.addr, &["view", "eureka-state:campaign:self"], None);
    assert_eq!(internal.code, 4);
    assert!(internal.err.starts_with("Internal: "), "{}", internal.err);

    // A batch no single send carries, refused before sending.
    let mut huge = campaign_json();
    huge["value"]["working_branch"] = json!("b".repeat(90_000));
    let big = write(&dir, "huge.json", &json!([huge]));
    let too_large = at(closed_port(), &admit_args(&big), None);
    assert_eq!(too_large.code, 2);
    assert!(too_large.err.starts_with("TooLarge: "), "{}", too_large.err);
    assert!(too_large.err.contains(&eureka_state::MAX_REQUEST_BYTES.to_string()), "{}", too_large.err);

    // The mind saying no: a refusal is an answer on stdout.
    let live = live();
    let refused = at(live.addr, &["admit", "--faculty", "Hands", "--agent", "a", "--session", "", &file], None);
    assert_eq!(refused.code, 1);
    assert!(refused.out.starts_with("Refused "), "{}", refused.out);
    assert!(refused.out.contains("provenance.session"), "{}", refused.out);
    assert_eq!(refused.err, "");

    // Absent: the mind answered null.
    let absent = at(live.addr, &["view", "eureka-state:cut_spec:cut-99.r1"], None);
    assert_eq!(absent.code, 1);
    assert_eq!(absent.out, "absent eureka-state:cut_spec:cut-99.r1\n");

    // Another instance's mind: refused on the read side too.
    let foreign = run(
        &[("EUREKA_INSTANCE", "thought-cage"), ("HUGINN_ENDPOINT", &format!("rudp://{}", live.addr))],
        &["query", "{}"],
        None,
    );
    assert_eq!(foreign.code, 1);
    assert!(foreign.out.starts_with("Refused "), "{}", foreign.out);
}

#[test]
fn a_call_that_is_wrong_exits_two_without_sending() {
    let (server, count) = counting(HuginnMindResponse::View(None), "view");
    for args in [
        vec!["view", "eureka-state:nonsense:x"],
        vec!["view", "no-kind-segment"],
        vec!["history", "eureka-state:nonsense:x"],
        vec!["query", "{ not json"],
        vec!["query", "[]"],
        vec!["admit", "--faculty", "Nope", "--agent", "a", "--session", "s", "x.json"],
        vec!["schema", "no-such-kind"],
        vec!["bogus"],
        vec!["view"],
        vec![],
    ] {
        let ran = at(server.addr, &args, None);
        assert_eq!(ran.code, 2, "{args:?}");
        assert!(ran.err.starts_with("InvalidInput: "), "{args:?}: {}", ran.err);
        assert_eq!(ran.out, "", "{args:?}");
    }
    assert_eq!(*count.lock().unwrap(), 0);
}

#[test]
fn the_recipes_send_exactly_the_pages_selections() {
    let schema = |kind: &str| format!("epiphany.pipeline.{kind}.v2");
    let root = |values: &[&str]| json!({ "index": "root", "op": "any_of", "values": values });
    let in_force = json!({ "index": "in_force", "op": "any_of", "values": ["true"] });
    let c = "eureka-substrate";

    let cases = [
        (vec!["campaigns"], json!({ "schemas": [schema("campaign")] })),
        (
            vec!["target", c],
            json!({ "schemas": [schema("target")], "fields": [root(&[c]), in_force], "projection": "document" }),
        ),
        (
            vec!["rulings", c, "eureka-body"],
            json!({ "schemas": [schema("ruling")], "fields": [root(&[c, "eureka-body"]), in_force] }),
        ),
        (
            vec!["open", c],
            json!({ "schemas": [schema("question"), schema("follow_up")], "fields": [root(&[c]), in_force] }),
        ),
        (
            vec!["findings", c],
            json!({
                "schemas": [schema("finding")],
                "fields": [root(&[c]), in_force, { "index": "faculty", "op": "any_of", "values": ["Soul"] }],
            }),
        ),
        (
            vec!["findings", c, "--blocking"],
            json!({
                "schemas": [schema("finding")],
                "fields": [
                    root(&[c]),
                    in_force,
                    { "index": "faculty", "op": "any_of", "values": ["Soul"] },
                    { "index": "severity", "op": "any_of", "values": ["Blocker", "High"] },
                ],
            }),
        ),
        (
            vec!["unreported", c],
            json!({
                "schemas": [schema("cut_spec")],
                "fields": [root(&[c]), in_force],
                "cited": { "role": "cut_spec", "exists": false },
            }),
        ),
        (
            vec!["blocked", "eureka-substrate:cut_spec:cut-x.r1"],
            json!({
                "schemas": [schema("question")],
                "fields": [in_force],
                "cites": {
                    "target": { "schemaId": schema("cut_spec"), "recordKey": "eureka-substrate:cut_spec:cut-x.r1" },
                    "role": "raised_in",
                },
            }),
        ),
        (
            vec!["unjudged", c],
            json!({
                "schemas": [schema("cut_report")],
                "fields": [root(&[c])],
                "cited": { "role": "cut_report", "exists": false },
            }),
        ),
        (
            vec!["history", "eureka-substrate:ruling:R1"],
            json!({
                "schemas": [schema("resolution")],
                "cites": { "target": { "schemaId": schema("ruling"), "recordKey": "eureka-substrate:ruling:R1" }, "role": "subject" },
                "descending": true,
            }),
        ),
        (
            vec!["cut", c, "huginn-cli"],
            json!({
                "schemas": [schema("cut_spec"), schema("cut_report"), schema("verdict"), schema("finding"), schema("resolution")],
                "fields": [root(&[c]), { "index": "cut", "op": "any_of", "values": ["huginn-cli"] }],
            }),
        ),
        (
            vec!["ledger", c],
            json!({ "schemas": [schema("cut_spec"), schema("cut_report")], "fields": [root(&[c])], "projection": "document" }),
        ),
        (
            vec!["precedent", c, "the question in plain words"],
            json!({ "schemas": [schema("ruling"), schema("resolution"), schema("finding")], "fields": [root(&[c])] }),
        ),
        (
            vec!["gaps"],
            json!({ "schemas": [schema("follow_up")], "fields": [root(&["eureka-substrate"]), in_force] }),
        ),
    ];
    for (args, literal) in cases {
        let (selection, semantic) = sent(&args);
        assert_eq!(selection, page_literal(literal), "{args:?}");
        let expected = (args[0] == "precedent").then(|| ("the question in plain words".to_string(), 10));
        assert_eq!(semantic, expected, "{args:?}");
    }
}

#[test]
fn paging_and_query_flags_shape_the_selection() {
    let (selection, semantic) = sent(&["gaps", "--cursor", "abc", "--documents"]);
    assert_eq!(selection.cursor.as_deref(), Some("abc"));
    assert_eq!(selection.projection, "document");
    assert_eq!(selection.limit, Some(50));
    assert_eq!(semantic, None);

    let (selection, _) = sent(&["gaps"]);
    assert_eq!(selection.cursor, None);
    assert_eq!(selection.projection, "header");

    let (selection, semantic) = sent(&["query", "{}", "--near", "drift", "--top-k", "7"]);
    assert_eq!(selection, page_literal(json!({})));
    assert_eq!(semantic, Some(("drift".to_string(), 7)));
    let (_, semantic) = sent(&["query", "{}", "--near", "drift"]);
    assert_eq!(semantic, Some(("drift".to_string(), 10)));

    let (selection, _) = sent(&["query", r#"{"limit": 5, "descending": true}"#]);
    assert_eq!(selection.limit, Some(5));
    assert!(selection.descending);
    let (selection, _) = sent(&["query", r#"{"projection": "header"}"#, "--documents"]);
    assert_eq!(selection.projection, "document");

    let ran = at(closed_port(), &["query", "{}", "--top-k", "3"], None);
    assert_eq!(ran.code, 2, "--top-k needs --near");

    let dir = tempfile::tempdir().unwrap();
    let file = write(&dir, "selection.json", &json!({ "schemas": ["epiphany.pipeline.campaign.v2"] }));
    let (selection, _) = sent(&["query", &format!("@{file}")]);
    assert_eq!(selection.schemas, Some(vec!["epiphany.pipeline.campaign.v2".to_string()]));
    let (server, seen) = recording();
    let ran = at(server.addr, &["query", "-"], Some(r#"{"keys": ["a:campaign:self"]}"#));
    assert_eq!(ran.code, 0, "{}", ran.err);
    let seen = seen.lock().unwrap();
    let [HuginnMindRequest::Query { selection, .. }] = &seen[..] else { panic!("{seen:?}") };
    assert_eq!(selection.keys, Some(vec!["a:campaign:self".to_string()]));
}

#[test]
fn recipes_list_what_the_mind_holds() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let campaign = write(&dir, "campaign.json", &json!([campaign_json()]));
    let ran = at(server.addr, &admit_args(&campaign), None);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let specs = write(&dir, "specs.json", &json!([spec("A"), spec("B")]));
    let ran = at(server.addr, &admit_args(&specs), None);
    assert_eq!(ran.code, 0, "{}", ran.err);

    let unreported = at(server.addr, &["unreported", CAMPAIGN], None);
    assert_eq!(unreported.code, 0, "{}", unreported.err);
    let mut lines: Vec<&str> = unreported.out.lines().collect();
    assert_eq!(lines.pop().unwrap().split(' ').take(2).collect::<Vec<_>>(), ["matched", "2"]);
    assert_eq!(lines.len(), 2);

    let reports = write(&dir, "report.json", &json!([report("B")]));
    let ran = at(server.addr, &admit_args(&reports), None);
    assert_eq!(ran.code, 0, "{}", ran.err);

    let unreported = at(server.addr, &["unreported", CAMPAIGN], None);
    let lines: Vec<&str> = unreported.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", unreported.out);
    let tokens: Vec<&str> = lines[0].split(' ').collect();
    let id = format!("{CAMPAIGN}:cut_spec:cut-A.r1");
    assert_eq!(tokens[..5], [id.as_str(), "InForce", "Hands", "A", "cut"]);
    for fact in ["cut=A", "repo=GameCult/Epiphany", "revision=1", "base=5f98228d", "branch=codex/eureka-pipeline-state"] {
        assert!(tokens.contains(&fact), "{fact}: {}", lines[0]);
    }
    assert!(lines[1].starts_with("matched 1 as_of "), "{}", lines[1]);
    assert!(lines[1].ends_with(" next -"), "{}", lines[1]);
    assert!(!unreported.out.contains("cut-B.r1"), "{}", unreported.out);

    let unjudged = at(server.addr, &["unjudged", CAMPAIGN], None);
    assert!(unjudged.out.contains(&format!("{CAMPAIGN}:cut_report:cut-B.h1")), "{}", unjudged.out);
    assert!(unjudged.out.contains("matched 1 "), "{}", unjudged.out);

    let campaigns = at(server.addr, &["campaigns"], None);
    assert!(campaigns.out.starts_with(&format!("{CAMPAIGN}:campaign:self InForce Hands Eureka pipeline state repos={REPO}\n")), "{}", campaigns.out);

    let whole = at(server.addr, &["ledger", CAMPAIGN], None);
    assert_eq!(whole.code, 0, "{}", whole.err);
    assert_eq!(whole.out.matches("admitted ").count(), 3, "{}", whole.out);
    assert!(whole.out.contains("\n\n"), "documents are separated by a blank line: {}", whole.out);

    let documents = at(server.addr, &["campaigns", "--documents"], None);
    assert!(documents.out.contains("\nadmitted "), "{}", documents.out);
    assert!(documents.out.contains("\"kind\": \"campaign\""), "{}", documents.out);
}

#[test]
fn a_long_header_is_cut() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let mut doc = campaign_json();
    doc["value"]["title"] = json!("t".repeat(200));
    doc["value"]["working_branch"] = json!("b");
    let file = write(&dir, "long.json", &json!([doc]));
    assert_eq!(at(server.addr, &admit_args(&file), None).code, 0);
    let ran = at(server.addr, &["campaigns"], None);
    let header = ran.out.lines().next().unwrap();
    assert_eq!(header.chars().count(), 240);
}

#[test]
fn a_resolved_document_shows_its_closing_record() {
    let server = live();
    let dir = tempfile::tempdir().unwrap();
    let question = json!({
        "kind": "question",
        "value": {
            "campaign": CAMPAIGN,
            "label": "Q1",
            "title": "Who owns the state?",
            "question": "Who owns the state?",
            "options": [{ "label": "A", "text": "an instance" }, { "label": "B", "text": "a repo" }],
            "recommended": "A",
            "depends": [],
            "raised_in": null,
            "asked_on": "2026-10-10",
        },
    });
    let setup = write(&dir, "setup.json", &json!([campaign_json(), question]));
    let ran = at(server.addr, &admit_args(&setup), None);
    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);
    let resolution = json!({
        "kind": "resolution",
        "value": {
            "subject": { "kind": "question", "id": format!("{CAMPAIGN}:question:Q1") },
            "sequence": 1,
            "outcome": { "Withdrawn": { "reason": "moot" } },
            "rationale": "Resolved.",
            "resolved_on": "2026-10-10",
        },
    });
    let closing = write(&dir, "closing.json", &json!([resolution]));
    let ran = at(server.addr, &admit_args(&closing), None);
    assert_eq!(ran.code, 0, "{}{}", ran.out, ran.err);

    let id = format!("{CAMPAIGN}:question:Q1");
    let closer = format!("{CAMPAIGN}:resolution:question.Q1.n1");
    let viewed = at(server.addr, &["view", &id], None);
    assert_eq!(viewed.out.lines().next().unwrap(), format!("{id} Resolved Withdrawn by {closer}"));

    let history = at(server.addr, &["history", &id], None);
    assert_eq!(history.code, 0, "{}", history.err);
    assert!(history.out.starts_with(&format!("{closer} InForce Hands ")), "{}", history.out);
    assert!(history.out.contains("sequence=1"), "{}", history.out);

    let open = at(server.addr, &["open", CAMPAIGN], None);
    assert!(open.out.contains("matched 0 "), "a resolved question is not open: {}", open.out);

    let header = at(server.addr, &["query", r#"{"schemas": ["epiphany.pipeline.question.v2"]}"#], None);
    assert!(header.out.starts_with(&format!("{id} Resolved:Withdrawn Hands Who owns the state? ")), "{}", header.out);
    assert!(header.out.contains("options=A,B") && header.out.contains("recommended=A"), "{}", header.out);
}

#[test]
fn schema_prints_the_published_files() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for kind in ["cut_spec", "campaign", "hand_off", "run"] {
        let ran = run(&[], &["schema", kind], None);
        assert_eq!(ran.code, 0);
        let published = std::fs::read_to_string(manifest.join(format!("../../schemas/cultnet/epiphany.pipeline.{kind}.v2.schema.json"))).unwrap();
        assert_eq!(ran.out, published, "{kind}");
    }
    let selection = run(&[], &["schema", "selection"], None);
    assert_eq!(selection.out, std::fs::read_to_string(manifest.join("schemas/cultnet.selection.schema.json")).unwrap());
    let names = run(&[], &["schema"], None);
    assert_eq!(names.code, 0);
    let names: Vec<&str> = names.out.lines().collect();
    assert_eq!(names.len(), 15, "{names:?}");
    assert_eq!(names[0], "campaign");
    assert_eq!(names[14], "selection");
    for kind in PipelineKind::ALL {
        assert!(names.contains(&kind.name()), "{}", kind.name());
    }
    let unknown = run(&[], &["schema", "no-such-kind"], None);
    assert_eq!(unknown.code, 2);
    assert!(unknown.err.starts_with("InvalidInput: "), "{}", unknown.err);
}

#[test]
fn help_is_the_schema_surface() {
    let rulings = run(&[], &["rulings", "--help"], None);
    assert_eq!(rulings.code, 0);
    let (_, printed) = rulings.out.split_once("selection:\n").expect("the selection is in the help");
    let selection: Value = serde_json::from_str(printed.trim()).unwrap();
    assert_eq!(
        selection,
        json!({
            "schemas": ["epiphany.pipeline.ruling.v2"],
            "fields": [
                { "index": "root", "op": "any_of", "values": ["<c>"] },
                { "index": "in_force", "op": "any_of", "values": ["true"] },
            ],
        })
    );
    let history = run(&[], &["history", "--help"], None);
    assert!(history.out.contains("epiphany.pipeline.<kind>.v2"), "{}", history.out);
    let findings = run(&[], &["findings", "--help"], None);
    assert!(findings.out.contains("\"severity\"") && findings.out.contains("Blocker"), "{}", findings.out);
    let precedent = run(&[], &["precedent", "--help"], None);
    assert!(precedent.out.contains("\"top_k\": 10"), "{}", precedent.out);
    let admit = run(&[], &["admit", "--help"], None);
    let admit_text = admit.out.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(admit_text.contains("{\"kind\": <kind>, \"value\": {...}}"), "{admit_text}");
    assert!(admit_text.contains("exit 5"), "{admit_text}");

    let version = run(&[], &["--version"], None);
    assert_eq!(version.code, 0);
    assert!(version.out.starts_with(&format!("huginn {} (", env!("CARGO_PKG_VERSION"))), "{}", version.out);
}

#[test]
fn every_recipe_has_help() {
    for recipe in [
        "campaigns", "target", "rulings", "open", "findings", "unreported", "blocked", "unjudged", "history", "cut", "ledger",
        "precedent", "gaps",
    ] {
        let ran = run(&[], &[recipe, "--help"], None);
        assert_eq!(ran.code, 0, "{recipe}");
        assert!(ran.out.contains("epiphany.pipeline."), "{recipe}: {}", ran.out);
        assert!(ran.out.contains("--cursor"), "{recipe}: {}", ran.out);
    }
}

#[test]
fn a_conflicting_batch_is_the_minds_no() {
    let conflict = scripted(|message| {
        let (_, request) = decode_request(message).expect("a mind request");
        assert!(matches!(request, HuginnMindRequest::Admit(_)));
        answer(
            "admit",
            &HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict {
                identities: vec![PipelineRef { kind: PipelineKind::Campaign, id: Short(format!("{CAMPAIGN}:campaign:self")) }],
            }),
        )
    });
    let dir = tempfile::tempdir().unwrap();
    let file = write(&dir, "campaign.json", &json!([campaign_json()]));
    let ran = at(conflict.addr, &admit_args(&file), None);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, format!("Conflict\nconflicts {CAMPAIGN}:campaign:self\n"));
}
