//! The `mind-self-run` binary as a process: its arguments, its exit codes and
//! what it never echoes. The carrying itself is tested in the library against a
//! real mind and a scripted child. One test, `#[ignore]`d outside a container,
//! puts the real binary, the real daemon and a fake `claude` first on `PATH`
//! together, which is where the real child's argument list and stream can be
//! seen end to end.

// The test hands the binary its environment, and reads the one variable the
// harness gives it. That is test plumbing: it reaches no library code.
#![allow(clippy::disallowed_methods, reason = "the suite sets the binary's environment")]

use std::process::{Command, Output};

const NO_SUCH_INSTANCE: &str = "self-run-test-no-such-instance";

fn self_run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mind-self-run")).args(args).env_remove("HUGINN_ENDPOINT").output().unwrap()
}

fn self_run_with_env_endpoint(args: &[&str], endpoint: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mind-self-run")).args(args).env("HUGINN_ENDPOINT", endpoint).output().unwrap()
}

fn all_output(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

#[test]
fn the_run_cap_must_be_a_positive_number_of_seconds_and_the_runtime_directory_must_be_named() {
    let with_cap = |cap: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mind-self-run"));
        command.args(["--instance", NO_SUCH_INSTANCE, "--endpoint", "rudp://127.0.0.1:9"]).env_remove("HUGINN_ENDPOINT").env_remove("RUNTIME_DIRECTORY").env("CLAUDE_LEASE_SOCKET", "/nonexistent/lease.sock");
        match cap {
            Some(cap) => command.env("MIND_SELF_RUN_CAP_S", cap),
            None => command.env_remove("MIND_SELF_RUN_CAP_S"),
        };
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        all_output(&output)
    };
    for bad in [None, Some("0"), Some(""), Some("abc"), Some("-5")] {
        assert!(with_cap(bad).contains("MIND_SELF_RUN_CAP_S is not set to"), "{bad:?}");
    }
    // A positive cap passes that check and meets the next: the unit names no scratch directory.
    let message = with_cap(Some("1"));
    assert!(!message.contains("MIND_SELF_RUN_CAP_S") && message.contains("RUNTIME_DIRECTORY is not set"), "{message}");
}

#[test]
fn without_an_instance_or_with_a_stray_argument_it_exits_2_with_the_usage_and_echoes_nothing() {
    let canary = "canary-argument-3f9a";
    for args in [
        vec![],
        vec!["--endpoint", "rudp://127.0.0.1:9"],
        vec!["--instance", NO_SUCH_INSTANCE, canary],
        vec!["--instance", NO_SUCH_INSTANCE, "--instance", canary],
        vec![canary],
        vec!["--resume", canary],
    ] {
        let output = self_run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage: mind-self-run"), "{args:?}: {stderr}");
        assert!(!all_output(&output).contains(canary) && output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn an_instance_that_is_not_a_slug_is_exit_2_and_is_not_echoed() {
    let canary = "Canary/../Instance-5e2b";
    let output = self_run(&["--instance", canary, "--endpoint", "rudp://127.0.0.1:9"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!all_output(&output).contains("anary"), "{}", all_output(&output));
    assert!(output.stdout.is_empty());
}

#[test]
fn a_malformed_endpoint_or_none_is_exit_2_and_the_endpoint_is_not_echoed() {
    let canary = "canary-endpoint-77c1";
    let bad = self_run(&["--instance", NO_SUCH_INSTANCE, "--endpoint", canary]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(!all_output(&bad).contains(canary) && bad.stdout.is_empty());
    let none = self_run(&["--instance", NO_SUCH_INSTANCE]);
    assert_eq!(none.status.code(), Some(2));
    assert!(none.stdout.is_empty());

    let from_env = self_run_with_env_endpoint(&["--instance", NO_SUCH_INSTANCE], "canary-env-endpoint-4d02");
    assert_eq!(from_env.status.code(), Some(2));
    assert!(!all_output(&from_env).contains("canary-env-endpoint-4d02") && from_env.stdout.is_empty());
}

#[test]
fn a_mind_that_cannot_be_reached_is_exit_2_before_any_child_and_names_no_value() {
    // Port 9 on loopback answers no one; PATH is empty, so a child could not start either.
    let output = Command::new(env!("CARGO_BIN_EXE_mind-self-run"))
        .args(["--instance", NO_SUCH_INSTANCE, "--endpoint", "rudp://127.0.0.1:9"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{}", all_output(&output));
    assert!(output.stdout.is_empty());
    assert!(!all_output(&output).contains("127.0.0.1"), "{}", all_output(&output));
}

#[cfg(unix)]
mod lane {
    use std::io::{IoSlice, Read, Write};
    use std::mem::MaybeUninit;
    use std::net::UdpSocket;
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, Mutex};
    use std::path::Path;
    use std::process::{Child, Command, Output, Stdio};
    use std::time::Duration;

    use chrono::Utc;
    use eureka_pipeline::{
        Date, DocRef, Label, PipelineCampaign, PipelineDocument, PipelineInstance, PipelineRun, PipelineStewardship, PipelineTarget, RunOperator, RunTurn, Sha,
        Slug, TargetInvariant,
    };
    use huginn_mind::{Faculty, Mind, PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelineProvenance};
    use mind_body::self_run::FLUSH_MESSAGE;

    use super::all_output;
    use tempfile::TempDir;

    const INSTANCE: &str = "yggdrasil";
    const CAMPAIGN: &str = "eureka-body";
    const REPO: &str = "GameCult/Huginn";
    // Pinned against Python's uuid.uuid5(uuid.NAMESPACE_OID, "yggdrasil:run:selfrun-lane").
    const LANE_SESSION: &str = "6bbfe181-08ee-5c29-8e32-0ebe48ec4b33";

    fn doc_ref() -> DocRef {
        DocRef { path: "docs/eureka-body-target.md".into(), start_line: 1, end_line: 9, commit: Sha("df7507b".into()) }
    }

    /// The identity, stewardship, campaign and target a run cites, and her one
    /// open Self run with budget 7.25.
    fn seed() -> Vec<PipelineDocument> {
        let date = || Date("2026-10-22".into());
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
                repos: vec![],
            }),
            PipelineDocument::Run(PipelineRun {
                instance: Slug(INSTANCE.into()),
                label: Label("selfrun-lane".into()),
                operated_by: RunOperator::Mind,
                turn: RunTurn::SelfRun,
                host: "test".into(),
                started_on: date(),
                budget_usd: "7.25".into(),
                claims: vec![],
                campaigns: vec![Slug(CAMPAIGN.into())],
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

    /// The daemon over a store seeded with `documents`, and the endpoint it listens on.
    /// It needs a built `huginn-daemon` beside the binary (`cargo build -p huginn-daemon`).
    fn serve(documents: Vec<PipelineDocument>) -> (Daemon, TempDir, String) {
        let slug = Slug(INSTANCE.into());
        let state = TempDir::new().unwrap();
        {
            let mut mind = Mind::open(state.path(), &slug).unwrap();
            let batch = PipelineAdmissionBatch {
                instance: slug.clone(),
                provenance: PipelineProvenance { faculty: Faculty::Hands, agent: "test".into(), session: "test".into(), tool: "test".into() },
                documents,
            };
            assert!(matches!(mind.admit(batch, Utc::now()), PipelineAdmissionOutcome::Committed { .. }));
        }
        let port = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let exe = Path::new(env!("CARGO_BIN_EXE_mind-self-run"));
        let daemon_exe = exe.with_file_name("huginn-daemon");
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
        (daemon, state, format!("rudp://127.0.0.1:{port}"))
    }

    /// The binary with `bin` first on `PATH`, until the daemon answers (exit 2 before that
    /// has started nothing). `SELF_RUN_ENV_CANARY` is a variable of the unit's environment.
    fn ask(endpoint: &str, bin: &Path, socket: &Path) -> Output {
        let exe = Path::new(env!("CARGO_BIN_EXE_mind-self-run"));
        let path = format!("{}:/usr/bin:/bin", bin.display());
        for _ in 0..40 {
            let attempt = Command::new(exe)
                .args(["--instance", INSTANCE, "--endpoint", endpoint])
                .env("PATH", &path)
                .env("SELF_RUN_ENV_CANARY", ENV_CANARY)
                .env("CLAUDE_LEASE_SOCKET", socket)
                .env("MIND_SELF_RUN_CAP_S", "14400")
                .env("RUNTIME_DIRECTORY", bin)
                .output()
                .unwrap();
            if attempt.status.code() != Some(2) {
                return attempt;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!("the daemon never answered");
    }

    const GRANT: &[u8] = include_bytes!("../src/lease_client/lease_v1/grant.bin");
    const SETTLED: &[u8] = include_bytes!("../src/lease_client/lease_v1/settled.bin");
    const LANE_TOKEN: &str = "tok-lane-canary-51c7";

    /// The connector's lease socket: every connection gets the golden grant with a pipe holding
    /// `LANE_TOKEN` attached as SCM_RIGHTS, then the golden settled answer to the settle it reads.
    /// The settle frames it received are kept.
    struct FakeConnector {
        _dir: TempDir,
        path: std::path::PathBuf,
        settles: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    fn read_frame(stream: &mut std::os::unix::net::UnixStream) -> std::io::Result<Vec<u8>> {
        let mut length = [0_u8; 4];
        stream.read_exact(&mut length)?;
        let mut frame = length.to_vec();
        frame.resize(4 + u32::from_be_bytes(length) as usize, 0);
        stream.read_exact(&mut frame[4..])?;
        Ok(frame)
    }

    fn fake_connector() -> FakeConnector {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("lease.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let settles = Arc::new(Mutex::new(Vec::new()));
        let kept = settles.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let kept = kept.clone();
                std::thread::spawn(move || {
                    if read_frame(&mut stream).is_err() {
                        return;
                    }
                    let (read, mut write) = std::io::pipe().unwrap();
                    write.write_all(LANE_TOKEN.as_bytes()).unwrap();
                    drop(write);
                    let read: std::os::fd::OwnedFd = read.into();
                    let fds = [std::os::fd::AsFd::as_fd(&read)];
                    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
                    let mut control = rustix::net::SendAncillaryBuffer::new(&mut space);
                    assert!(control.push(rustix::net::SendAncillaryMessage::ScmRights(&fds)));
                    rustix::net::sendmsg(&stream, &[IoSlice::new(GRANT)], &mut control, rustix::net::SendFlags::empty()).unwrap();
                    if let Ok(settle) = read_frame(&mut stream) {
                        kept.lock().unwrap().push(settle);
                        let _ = stream.write_all(SETTLED);
                    }
                });
            }
        });
        FakeConnector { _dir: dir, path, settles }
    }

    const ENV_CANARY: &str = "canary-unit-environment-b94e";

    /// Writes an executable `claude` into `bin` and returns the path of the record it keeps.
    fn fake_claude(bin: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let record = bin.join("record");
        let script = format!("#!/bin/bash\necho \"$@\" > {record}\n{body}\n", record = record.display());
        let claude = bin.join("claude");
        std::fs::write(&claude, script).unwrap();
        std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
        record
    }

    /// The real binary against the real daemon, with a fake `claude` first on `PATH`.
    /// The fake prints two requests (the second reaches the ceiling), waits for the start
    /// and the flush on stdin, records its argument list, what it read and the unit
    /// variable it inherited, prints a result and exits 0. The bin finds her run, starts
    /// the child for it, asks for the flush and exits 0 without echoing the inherited
    /// variable. It runs in the container lane only; `cargo test` and `cargo mutants`
    /// skip it. Run it with the README's lane command.
    #[test]
    #[ignore = "container only: a built huginn-daemon beside the binary, and a POSIX shell"]
    fn the_real_bin_finds_her_run_starts_claude_for_it_asks_for_the_flush_and_exits_0() {
        let (daemon, _state, endpoint) = serve(seed());
        let bin = TempDir::new().unwrap();
        let request = |id: &str, tokens: u64| {
            format!(
                r#"{{"type":"assistant","parent_tool_use_id":null,"message":{{"id":"{id}","usage":{{"input_tokens":1,"cache_read_input_tokens":{tokens},"cache_creation_input_tokens":0}}}}}}"#
            )
        };
        let body = format!(
            "read -r -u \"$CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR\" cred
echo journal-line >&2
echo '{first}'\necho '{second}'\nread -t 5 start\nread -t 5 flush\necho \"$start\" >> {record}\necho \"$flush\" >> {record}\necho \"$SELF_RUN_ENV_CANARY\" >> {record}\necho \"$cred\" >> {record}\necho '{{\"type\":\"result\"}}'\nwhile read more; do :; done\nexit 0",
            first = request("msg_1", 100),
            second = request("msg_2", 299_999),
            record = bin.path().join("record").display(),
        );
        let record = fake_claude(bin.path(), &body);
        let connector = fake_connector();
        let output = ask(&endpoint, bin.path(), &connector.path);
        drop(daemon);

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        assert_eq!(output.status.code(), Some(0), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(stdout, "request 1 prefix 101\nrequest 2 prefix 300000\nsettled: billed 123456 micro-USD\nended: flush-requested code 0 requests 2 prefix 300000\n");
        assert!(!all_output(&output).contains(ENV_CANARY), "the unit environment is not echoed");
        assert!(String::from_utf8_lossy(&output.stderr).contains("journal-line"), "the child's stderr is the unit's journal");

        let recorded = std::fs::read_to_string(&record).unwrap();
        let mut lines = recorded.lines();
        let argv = lines.next().unwrap();
        assert!(argv.contains(&format!("--session-id {LANE_SESSION}")) && argv.contains("--max-budget-usd 7.25"), "{argv}");
        assert!(argv.starts_with("-p --input-format stream-json"), "{argv}");
        assert!(lines.next().unwrap().contains("yggdrasil:run:selfrun-lane"), "the start message names her run");
        let flush = lines.next().unwrap();
        assert!(flush.contains(FLUSH_MESSAGE) && flush.contains("\"type\":\"user\""), "the flush went to the child's stdin: {flush}");
        assert_eq!(lines.next().unwrap(), "", "the child gets the allow-listed variables only, not the rest of the unit environment");
        assert_eq!(lines.next().unwrap(), LANE_TOKEN, "the child read its credential from the lease's descriptor");
        assert!(!all_output(&output).contains(LANE_TOKEN), "the token is never in the bin's output");
        let settles = connector.settles.lock().unwrap();
        assert_eq!(settles.len(), 1, "the run settled once");
        assert!(String::from_utf8_lossy(&settles[0]).contains("claude_lease_settle"));
    }

    /// With no Self run of hers in the mind the bin starts no child and exits 3.
    #[test]
    #[ignore = "container only: a built huginn-daemon beside the binary, and a POSIX shell"]
    fn the_real_bin_with_no_self_run_of_hers_exits_3_and_never_starts_claude() {
        let mut documents = seed();
        documents.pop();
        let (daemon, _state, endpoint) = serve(documents);
        let bin = TempDir::new().unwrap();
        let record = fake_claude(bin.path(), "exit 0");
        let connector = fake_connector();
        let output = ask(&endpoint, bin.path(), &connector.path);
        drop(daemon);
        assert_eq!(output.status.code(), Some(3), "{}", all_output(&output));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "no run: she has no single open Self run\n");
        assert!(!record.exists(), "claude was started");
    }
}
