//! The `mind-stream` binary's startup reads the connector key from systemd's credentials directory:
//! an absent directory names the variable, and the key's bytes never reach stderr.

use std::process::{Command, Output};

const CANARY: &str = "zq7Kx-9wVtm3Pd";

fn run(credentials: Option<&std::path::Path>, connector: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mind-stream"));
    command
        .args(["--instance", "eureka", "--once"])
        .env_clear()
        .env("HUGINN_ENDPOINT", "rudp://127.0.0.1:9")
        .env("MIND_STREAM_CONNECTOR_ENDPOINT", connector)
        .env("MIND_STREAM_PROJECTOR_MODEL", "claude-haiku-5-5")
        .env("MIND_STREAM_BASE_MODEL", "base:test")
        .env("MIND_STREAM_ENDPOINT", "http://127.0.0.1:1");
    if let Some(dir) = credentials {
        command.env("CREDENTIALS_DIRECTORY", dir);
    }
    command.output().unwrap()
}

#[test]
fn a_missing_credentials_directory_names_the_variable_and_a_missing_key_names_the_credential() {
    let output = run(None, "127.0.0.1:4103");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CREDENTIALS_DIRECTORY"), "{stderr}");
    let empty = tempfile::tempdir().unwrap();
    let stderr = String::from_utf8_lossy(&run(Some(empty.path()), "127.0.0.1:4103").stderr).to_string();
    assert!(stderr.contains("connector-key"), "{stderr}");
}

/// Every four-byte run of the key: a partial key is as much a leak as the whole.
fn leaks(shown: &str) -> Option<&str> {
    CANARY.as_bytes().windows(4).map(|window| std::str::from_utf8(window).unwrap()).find(|run| shown.contains(run))
}

#[test]
fn no_byte_run_of_the_key_is_printed_on_any_startup_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("connector-key"), format!("{CANARY}\n")).unwrap();
    // A non-loopback connector and a configuration the client refuses both end startup after the key was read and handed over.
    for (connector, ending) in [("10.77.0.1:4103", "not a loopback address"), ("127.0.0.1:0", "could not be configured")] {
        let output = run(Some(dir.path()), connector);
        let shown = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(shown.contains(ending), "{connector}: the key was read and startup went on: {shown}");
        assert_eq!(leaks(&shown), None, "{connector}: {shown}");
    }
}
