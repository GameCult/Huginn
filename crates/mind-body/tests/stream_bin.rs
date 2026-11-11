//! The `mind-stream` binary's startup reads the connector key from systemd's credentials directory:
//! an absent directory names the variable, and the key's bytes never reach stderr.

use std::process::{Command, Output};

const CANARY: &str = "canary-key-7f3a91";

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

#[test]
fn the_key_is_never_printed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("connector-key"), format!("{CANARY}\n")).unwrap();
    // A non-loopback connector ends startup after the key was read and handed over.
    let output = run(Some(dir.path()), "10.77.0.1:4103");
    let shown = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(shown.contains("not a loopback address"), "the key was read and startup went on to the endpoint check: {shown}");
    assert!(!shown.contains(CANARY), "{shown}");
}
