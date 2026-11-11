//! The `mind-persona` binary as the unit runs it: arguments `--instance`, the rest
//! from `persona.env` and the credential files. The control root is a constant
//! that does not exist where this runs, so a start that is configured right ends
//! dead at the gate (exit 0) before any transport is touched; a start configured
//! wrong ends 2 and names the setting, never a value.

use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, drop: &[&str], extra: &[(&str, &str)], args: &[&str]) -> Output {
    std::fs::write(dir.join("chat-key"), "chat-secret-text\n").unwrap();
    let key = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let mut env = vec![
        ("HUGINN_ENDPOINT", "rudp://127.0.0.1:9".to_string()),
        ("STATE_DIRECTORY", dir.to_string_lossy().into_owned()),
        ("MIND_PERSONA_ROOMS", "#swarm:chat,#ops:chat".to_string()),
        ("MIND_PERSONA_OPERATOR", "@op:chat".to_string()),
        ("MIND_PERSONA_MINDS", "@eureka:chat,@libby:chat".to_string()),
        ("MIND_PERSONA_SELF_SENDER", "@eureka:chat".to_string()),
        ("MIND_PERSONA_MODEL", "claude-test-model".to_string()),
        ("MIND_PERSONA_RUN_CAP_S", "14400".to_string()),
        ("RUNTIME_DIRECTORY", dir.join("runtime").to_string_lossy().into_owned()),
        ("CLAUDE_LEASE_SOCKET", key("lease.sock")),
        ("BIFROST_CHAT_ENDPOINT", "rudp://127.0.0.1:17884".to_string()),
        ("BIFROST_CHAT_KEY_FILE", key("chat-key")),
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
fn a_flag_overrides_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    // The rooms flag replaces the rooms the environment names.
    let flagged = run(dir.path(), &[], &[], &["--instance", "eureka", "--rooms", "#swarm:chat"]);
    assert_eq!(flagged.status.code(), Some(0), "{}", text(&flagged.stderr));
}

#[test]
fn a_setting_that_is_missing_or_wrong_ends_two_naming_the_setting_and_no_value() {
    let dir = tempfile::tempdir().unwrap();
    let cases: [Case; 9] = [
        (&["MIND_PERSONA_ROOMS"], &[], "usage"),
        (&["MIND_PERSONA_MINDS"], &[], "usage"),
        (&["MIND_PERSONA_MODEL"], &[], "MIND_PERSONA_MODEL"),
        (&["MIND_PERSONA_RUN_CAP_S"], &[], "MIND_PERSONA_RUN_CAP_S"),
        (&[], &[("MIND_PERSONA_RUN_CAP_S", "0")], "MIND_PERSONA_RUN_CAP_S"),
        (&["RUNTIME_DIRECTORY"], &[], "RUNTIME_DIRECTORY"),
        (&["CLAUDE_LEASE_SOCKET"], &[], "CLAUDE_LEASE_SOCKET"),
        (&[], &[("BIFROST_CHAT_ENDPOINT", "127.0.0.1:17884")], "BIFROST_CHAT_ENDPOINT"),
        (&[], &[("BIFROST_CHAT_KEY_FILE", "/nonexistent/chat-key")], "BIFROST_CHAT_KEY_FILE"),
    ];
    for (drop, extra, names) in cases {
        let output = run(dir.path(), drop, extra, &["--instance", "eureka"]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{drop:?} {extra:?}: {stderr}");
        assert!(stderr.contains(names), "{drop:?} {extra:?}: {stderr}");
        assert!(!stderr.contains("secret-text") && !stderr.contains("127.0.0.1:17884") && !stderr.contains("/nonexistent/chat-key"), "no value is echoed: {stderr}");
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

/// The reply-link setting is the mouth's own rule: exactly `1` is on, anything else is off, and none of it ends the unit or reaches a message.
#[test]
fn a_v4_setting_other_than_exactly_1_is_off() {
    let dir = tempfile::tempdir().unwrap();
    for value in ["", "0", "true", "yes", " 1", "canary-v4"] {
        let output = run(dir.path(), &[], &[("BIFROST_CHAT_PENDING_V4", value)], &["--instance", "eureka"]);
        assert_eq!(output.status.code(), Some(0), "{value:?}: {}", text(&output.stderr));
        assert!(!text(&output.stderr).contains("canary-v4") && !text(&output.stdout).contains("canary-v4"));
    }
    assert_eq!(run(dir.path(), &[], &[("BIFROST_CHAT_PENDING_V4", "1")], &["--instance", "eureka"]).status.code(), Some(0));
}
