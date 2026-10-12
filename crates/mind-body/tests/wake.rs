//! The `mind-wake` binary as a process: its arguments and its exit codes. The
//! brake's root is a constant, so a process test can only ever see the absent
//! store of an instance no host has; the cycle's behaviour is tested in the
//! library against a real mind and a real store. One test, `#[ignore]`d outside a
//! container, puts the real binary, the real daemon and a real store at the
//! brake's root together, which is where the host a run records can be seen.

use std::net::UdpSocket;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::str::FromStr;
use std::time::Duration;

use chrono::Utc;
use cultnet_rs::Selection;
use eureka_pipeline::{
    Date, DocRef, PipelineCampaign, PipelineDocument, PipelineInstance, PipelineKind, PipelineStewardship, PipelineTarget, Sha, Slug, TargetInvariant,
};
use huginn_mind::{Faculty, Mind, PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelinePageItems, PipelineProvenance};
use mind_body::control::{BurnRate, CONTROL_ROOT, ControlWriter, control_path};
use rust_decimal::Decimal;
use tempfile::TempDir;

const NO_SUCH_INSTANCE: &str = "wake-test-no-such-instance";

fn wake(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mind-wake")).args(args).env_remove("HUGINN_ENDPOINT").output().unwrap()
}

/// The binary with `HUGINN_ENDPOINT` set to `endpoint`.
fn wake_with_env_endpoint(args: &[&str], endpoint: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mind-wake")).args(args).env("HUGINN_ENDPOINT", endpoint).output().unwrap()
}

#[test]
fn without_an_instance_or_with_a_stray_argument_it_exits_2_with_the_usage_and_echoes_nothing() {
    let canary = "canary-argument-3f9a";
    for args in [vec![], vec!["--endpoint", "rudp://127.0.0.1:9"], vec!["--instance", NO_SUCH_INSTANCE, canary], vec!["--instance", NO_SUCH_INSTANCE, "--instance", canary]] {
        let output = wake(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage: mind-wake"), "{args:?}: {stderr}");
        assert!(!stderr.contains(canary) && output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn an_instance_that_is_not_a_slug_is_exit_2_and_is_not_echoed() {
    let canary = "Canary/../Instance-5e2b";
    let output = wake(&["--instance", canary, "--endpoint", "rudp://127.0.0.1:9"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("anary"), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn a_held_brake_ends_the_cycle_with_exit_0_before_the_mind_is_contacted() {
    // The endpoint answers no one (port 9 on loopback), so a cycle that
    // reached the mind would exit 2; the absent store holds the cycle first.
    let output = wake(&["--instance", NO_SUCH_INSTANCE, "--endpoint", "rudp://127.0.0.1:9"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "held: BrakeAbsent");
}

#[test]
fn a_malformed_endpoint_or_none_is_exit_2_and_the_endpoint_is_not_echoed() {
    let canary = "canary-endpoint-77c1";
    let bad = wake(&["--instance", NO_SUCH_INSTANCE, "--endpoint", canary]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&bad.stderr).contains(canary));
    let none = wake(&["--instance", NO_SUCH_INSTANCE]);
    assert_eq!(none.status.code(), Some(2));
}

#[test]
fn without_the_flag_the_daemon_is_found_through_huginn_endpoint_and_a_bad_value_is_not_echoed() {
    // With no endpoint at all the cycle exits 2 (above); with the variable set it
    // goes on to the brake, which holds an instance no host has, and exits 0.
    let held = wake_with_env_endpoint(&["--instance", NO_SUCH_INSTANCE], "rudp://127.0.0.1:9");
    assert_eq!(held.status.code(), Some(0), "{}", String::from_utf8_lossy(&held.stderr));
    assert_eq!(String::from_utf8_lossy(&held.stdout).trim(), "held: BrakeAbsent");

    let canary = "canary-env-endpoint-4d02";
    let bad = wake_with_env_endpoint(&["--instance", NO_SUCH_INSTANCE], canary);
    assert_eq!(bad.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&bad.stderr).contains(canary) && bad.stdout.is_empty());

    // The flag wins over the variable: a valid flag with a bad variable still holds.
    let flag = wake_with_env_endpoint(&["--instance", NO_SUCH_INSTANCE, "--endpoint", "rudp://127.0.0.1:9"], canary);
    assert_eq!(flag.status.code(), Some(0));
}

const INSTANCE: &str = "yggdrasil";
const CAMPAIGN: &str = "eureka-body";
const REPO: &str = "GameCult/Huginn";

fn doc_ref() -> DocRef {
    DocRef { path: "docs/eureka-body-target.md".into(), start_line: 1, end_line: 9, commit: Sha("df7507b".into()) }
}

/// The identity, stewardship, campaign and target a Persona turn's run cites.
fn seed() -> Vec<PipelineDocument> {
    let date = || Date("2026-10-09".into());
    vec![
        PipelineDocument::Instance(PipelineInstance { instance: Slug(INSTANCE.into()), display_name: "test mind".into(), created_at: date(), host: "test".into() }),
        PipelineDocument::Stewardship(PipelineStewardship { instance: Slug(INSTANCE.into()), repo: REPO.into(), sequence: 1, assigned_on: date(), note: "assigned".into() }),
        PipelineDocument::Campaign(PipelineCampaign {
            slug: Slug(CAMPAIGN.into()),
            title: "Eureka body".into(),
            repos: vec![REPO.into()],
            working_branch: "main".into(),
            target_doc: doc_ref(),
        }),
        PipelineDocument::Target(PipelineTarget {
            campaign: Slug(CAMPAIGN.into()),
            revision: 1,
            invariants: vec![TargetInvariant { label: "run-is-the-grant".into(), statement: "A run is the grant.".into() }],
            not_in_scope: vec![],
            canonical_implementations: vec![],
            doc: doc_ref(),
        }),
    ]
}

/// The daemon, ended with the test whether it passes or not.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The real binary against the real daemon and a real store at the brake's
/// root. It writes `/etc/gamecult/minds`, so it runs in a container only
/// (`/.dockerenv`), as root, after `cargo build -p huginn-daemon` has put the
/// daemon beside the binary. `PATH` is empty so no unit is ever reached: the
/// run opens, its unit fails to start and the run is withdrawn, which is the
/// outcome printed.
#[test]
#[ignore = "container only: root, /etc/gamecult/minds and a built huginn-daemon beside the binary"]
fn the_real_bin_against_the_real_daemon_opens_a_run_that_records_this_machines_host_and_the_agent() {
    assert!(Path::new("/.dockerenv").exists(), "this test writes the brake's root and runs in a container only");
    let slug = Slug(INSTANCE.into());
    let state = TempDir::new().unwrap();
    {
        let mut mind = Mind::open(state.path(), &slug).unwrap();
        let batch = PipelineAdmissionBatch {
            instance: slug.clone(),
            provenance: PipelineProvenance { faculty: Faculty::Hands, agent: "test".into(), session: "test".into(), tool: "test".into() },
            documents: seed(),
        };
        assert!(matches!(mind.admit(batch, Utc::now()), PipelineAdmissionOutcome::Committed { .. }));
    }
    let mut writer = ControlWriter::open(control_path(Path::new(CONTROL_ROOT), &slug).unwrap()).unwrap();
    writer.set_brake(true, Utc::now(), "op").unwrap();
    let dial = BurnRate { heat: Decimal::from_str("1").unwrap(), base_cooldown_s: 600, base_run_usd: Decimal::from_str("5").unwrap(), set_at: Utc::now(), set_by: "op".into() };
    writer.set_dial(dial).unwrap();

    let port = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let endpoint = format!("rudp://127.0.0.1:{port}");
    let wake_exe = Path::new(env!("CARGO_BIN_EXE_mind-wake"));
    let daemon_exe = wake_exe.with_file_name("huginn-daemon");
    assert!(daemon_exe.exists(), "build huginn-daemon first: {}", daemon_exe.display());
    let daemon = Daemon(
        Command::new(&daemon_exe)
            .args(["--state-root", state.path().to_str().unwrap(), "--instance", INSTANCE, "--bind", &format!("127.0.0.1:{port}")])
            .args(["--qdrant-url", "http://127.0.0.1:1", "--ollama-url", "http://127.0.0.1:1", "--embedding-model", "x"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );

    // Exit 2 before the daemon answers has opened nothing; try again until it does.
    let mut output = None;
    for _ in 0..40 {
        let attempt = Command::new(wake_exe).args(["--instance", INSTANCE, "--endpoint", &endpoint]).env("PATH", "").output().unwrap();
        if attempt.status.code() != Some(2) {
            output = Some(attempt);
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let output = output.expect("the daemon never answered");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "refused: start_failed");
    drop(daemon);

    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap().trim().to_string();
    assert!(!hostname.is_empty());
    let mind = Mind::open(state.path(), &slug).unwrap();
    let runs = Selection { schemas: Some(vec![PipelineKind::Run.type_id().to_string()]), ..Selection::default() };
    let PipelinePageItems::Headers(headers) = mind.query(&runs).unwrap().items else { panic!("documents to a header selection") };
    assert_eq!(headers.len(), 1);
    let view = mind.view(&headers[0].id).unwrap().unwrap();
    let PipelineDocument::Run(run) = &view.document else { panic!("not a run") };
    assert_eq!(run.host.0, hostname);
    assert_eq!(view.admission.provenance.agent.0, "mind-wake");
}
