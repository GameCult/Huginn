//! The `mind-persona` binary as the unit runs it: arguments `--instance`, the rest
//! from `persona.env`. The control root is a constant that does not exist where
//! this runs, so a start that is configured right ends dead at the gate (exit 0);
//! a start configured wrong ends 2 and names the setting, never a value.

use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, drop: &[&str], extra: &[(&str, &str)], args: &[&str]) -> Output {
    let mut env = vec![
        ("HUGINN_ENDPOINT", "rudp://127.0.0.1:9".to_string()),
        ("STATE_DIRECTORY", dir.to_string_lossy().into_owned()),
        ("MIND_PERSONA_ROOMS", "#swarm:chat,#ops:chat".to_string()),
        ("MIND_PERSONA_OPERATOR", "@op:chat".to_string()),
        ("MIND_PERSONA_MINDS", "@eureka:chat,@libby:chat".to_string()),
        ("MIND_PERSONA_SELF_SENDER", "@eureka:chat".to_string()),
        ("MIND_PERSONA_MODEL", "claude-test-model".to_string()),
    ];
    env.retain(|(name, _)| !drop.contains(name) && !extra.iter().any(|(set, _)| set == name));
    env.extend(extra.iter().map(|(name, value)| (*name, (*value).to_string())));
    Command::new(env!("CARGO_BIN_EXE_mind-persona")).args(args).env_clear().envs(env).output().unwrap()
}

/// What to drop from the environment, what to set instead, and the setting the error must name.
type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)], &'a str);

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_unit_configured_from_its_environment_alone_reaches_the_gate_and_ends_dead() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &[], &[], &["--instance", "eureka"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).starts_with("dead:"), "{}", text(&output.stdout));
}

#[test]
fn a_setting_that_is_missing_or_wrong_ends_two_naming_the_setting_and_no_value() {
    let dir = tempfile::tempdir().unwrap();
    let cases: [Case; 4] = [
        (&["MIND_PERSONA_ROOMS"], &[], "usage"),
        (&["MIND_PERSONA_MINDS"], &[], "usage"),
        (&["MIND_PERSONA_MODEL"], &[], "MIND_PERSONA_MODEL"),
        (&[], &[("HUGINN_ENDPOINT", "canary-endpoint")], "endpoint"),
    ];
    for (drop, extra, names) in cases {
        let output = run(dir.path(), drop, extra, &["--instance", "eureka"]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{drop:?} {extra:?}: {stderr}");
        assert!(stderr.contains(names), "{drop:?} {extra:?}: {stderr}");
        assert!(!stderr.contains("canary-endpoint") && !stderr.contains("#ops:chat"), "no value is echoed: {stderr}");
    }
}

#[test]
fn the_retired_max_turns_flag_is_refused_and_its_value_is_not_echoed() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &[], &[], &["--instance", "eureka", "--max-turns", "canary-turns"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("usage") && !stderr.contains("canary-turns"), "{stderr}");
}

/// The harness's read-only proof at the binary: with a room fixture and a state file, `project` runs with no persona.env,
/// no mouth and the daemon's endpoint unreachable, and never needs them.
#[test]
fn project_over_a_room_fixture_and_a_state_file_needs_no_daemon_and_no_mouth() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/harness");
    let room = fixtures.join("rooms/mind-teases.toml");
    let state = fixtures.join("states/control.json");
    let args = ["project", "--instance", "eureka", "--room", room.to_str().unwrap(), "--state", state.to_str().unwrap(), "--select-only"];
    // Nothing listens on the discard port: a read of her stored state would fail the run.
    let output = Command::new(env!("CARGO_BIN_EXE_mind-persona")).args(args).env_clear().env("HUGINN_ENDPOINT", "rudp://127.0.0.1:9").env("MIND_PERSONA_TUNING", "/nonexistent/persona-tuning.toml").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!((report["state"].as_str(), report["persona_receipt"].is_null()), (Some("given"), true), "{report}");
    assert!(report["packet"]["identity"].as_str().unwrap().starts_with("Tobiah Fenn"), "{report}");
    assert!(report["packet"]["stimulus"].as_str().unwrap().contains("Said to him"), "Kiko's tagged reply is for him, in his own pronouns: {report}");
    // Without any endpoint at all it runs the same.
    let output = Command::new(env!("CARGO_BIN_EXE_mind-persona")).args(args).env_clear().env("MIND_PERSONA_TUNING", "/nonexistent/persona-tuning.toml").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
}

#[test]
fn a_state_file_a_put_would_refuse_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/harness");
    let mut state: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixtures.join("states/control.json")).unwrap()).unwrap();
    state["personaId"] = "libby".into();
    let file = dir.path().join("foreign.json");
    std::fs::write(&file, state.to_string()).unwrap();
    let room = fixtures.join("rooms/quiet-beat.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_mind-persona")).args(["project", "--instance", "eureka", "--room", room.to_str().unwrap(), "--state", file.to_str().unwrap(), "--select-only"]).env_clear().output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("refused by a put"), "{}", text(&output.stderr));
}
