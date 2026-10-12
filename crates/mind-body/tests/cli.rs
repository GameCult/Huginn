//! The `mind-control` command driven end to end through `mind_body::cli::run`
//! against a temporary root, and the binary once as a process for its exit
//! status. Nothing touches `/etc`.

use chrono::{TimeZone, Utc};
use eureka_pipeline::Slug;
use mind_body::cli::{operator_name, run};

fn go(root: &std::path::Path, args: &[&str], set_by: &str, secs: i64) -> (anyhow::Result<()>, String) {
    let (result, out, _) = go_stopping(root, args, set_by, secs, |_| Ok(()));
    (result, out)
}

/// Runs one command with a stopper that records the instance of every call and
/// answers with `stop`. `stop` also sees the call before it returns.
fn go_stopping(
    root: &std::path::Path,
    args: &[&str],
    set_by: &str,
    secs: i64,
    mut stop: impl FnMut(&Slug) -> anyhow::Result<()>,
) -> (anyhow::Result<()>, String, Vec<String>) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let mut out = Vec::new();
    let mut calls = Vec::new();
    let now = Utc.timestamp_opt(1_800_000_000 + secs, 0).unwrap();
    let result = run(&args, root, set_by, now, &mut out, &mut |instance| {
        calls.push(instance.0.clone());
        stop(instance)
    });
    (result, String::from_utf8(out).unwrap(), calls)
}

#[test]
fn show_reports_every_rung_from_an_absent_store_to_released() {
    let root = tempfile::tempdir().unwrap();
    let (r, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 0);
    r.unwrap();
    assert!(out.contains("brake: absent") && out.contains("dial: absent") && out.contains("BrakeAbsent"), "{out}");

    go(root.path(), &["--instance", "eureka", "brake", "release"], "alice", 1).0.unwrap();
    let (_, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 2);
    assert!(out.contains("brake: released") && out.contains("set_by alice") && out.contains("DialAbsent"), "{out}");

    go(root.path(), &["--instance", "eureka", "dial", "set", "--heat", "0.5", "--base-cooldown-s", "3600", "--base-run-usd", "5"], "bob", 3)
        .0
        .unwrap();
    let (_, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 4);
    assert!(out.contains("heat 0.5") && out.contains("base_cooldown_s 3600") && out.contains("set_by bob"), "{out}");
    assert!(out.contains("Released") && out.contains("7200s"), "{out}");

    go(root.path(), &["--instance", "eureka", "brake", "hold"], "alice", 5).0.unwrap();
    let (_, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 6);
    assert!(out.contains("brake: held") && out.contains("BrakeHeld"), "{out}");
}

#[test]
fn instances_do_not_share_a_store() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "one", "brake", "release"], "op", 0).0.unwrap();
    let (_, out) = go(root.path(), &["--instance", "two", "show"], "op", 1);
    assert!(out.contains("brake: absent"), "{out}");
    assert!(root.path().join("one").join("control.cc").exists());
}

#[test]
fn a_dial_set_outside_bounds_refuses_and_leaves_the_store_unchanged() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    go(root.path(), &["--instance", "eureka", "dial", "set", "--heat", "1", "--base-cooldown-s", "60", "--base-run-usd", "1"], "op", 1).0.unwrap();
    let path = root.path().join("eureka").join("control.cc");
    let before = std::fs::read(&path).unwrap();
    for heat in ["2.0001", "0.0499", "0", "-1"] {
        let (r, _) =
            go(root.path(), &["--instance", "eureka", "dial", "set", "--heat", heat, "--base-cooldown-s", "60", "--base-run-usd", "1"], "op", 9);
        assert!(r.is_err(), "heat {heat} must be refused");
        assert_eq!(std::fs::read(&path).unwrap(), before, "heat {heat} changed the store");
    }

    // A refusal on a store that does not exist yet must not create it.
    let fresh = tempfile::tempdir().unwrap();
    let (r, _) = go(fresh.path(), &["--instance", "eureka", "dial", "set", "--heat", "5", "--base-cooldown-s", "60", "--base-run-usd", "1"], "op", 0);
    assert!(r.is_err());
    assert!(!fresh.path().join("eureka").join("control.cc").exists());
}

#[test]
fn malformed_commands_are_refused_and_write_nothing() {
    let root = tempfile::tempdir().unwrap();
    let bad: &[&[&str]] = &[
        &[],
        &["show"],
        &["--instance", "eureka"],
        &["--instance", "eureka", "brake"],
        &["--instance", "eureka", "brake", "toggle"],
        &["--instance", "eureka", "dial", "set", "--heat", "1"],
        &["--instance", "eureka", "dial", "set", "--heat", "1", "--heat", "1", "--base-cooldown-s", "60", "--base-run-usd", "1"],
        &["--instance", "eureka", "dial", "set", "--heat", "x", "--base-cooldown-s", "60", "--base-run-usd", "1"],
        &["--instance", "eureka", "dial", "set", "--heat", "1", "--base-cooldown-s", "1.5", "--base-run-usd", "1"],
        &["--instance", "eureka", "dial", "set", "--heat", "1", "--base-cooldown-s", "60", "--base-run-usd", "1", "--extra", "1"],
        &["--instance", "eureka", "dial", "set", "--heat"],
        &["--instance", "../evil", "brake", "release"],
        &["--instances", "eureka", "brake", "release"],
        &["eureka", "eureka", "brake", "release"],
    ];
    for args in bad {
        assert!(go(root.path(), args, "op", 0).0.is_err(), "{args:?}");
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0, "a refused command wrote something");
}

#[test]
fn the_operator_is_sudo_user_then_user_then_unknown() {
    let s = |v: &str| Some(v.to_string());
    assert_eq!(operator_name(s("alice"), s("root")), "alice");
    assert_eq!(operator_name(None, s("bob")), "bob");
    assert_eq!(operator_name(s(""), s("bob")), "bob");
    assert_eq!(operator_name(None, s("")), "unknown");
    assert_eq!(operator_name(None, None), "unknown");
}

#[test]
fn the_binary_exits_nonzero_with_the_usage_when_refused() {
    // A refusal before any path is derived, so this never reaches /etc.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mind-control")).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage: mind-control"));
    assert!(output.stdout.is_empty());
}

#[test]
fn a_leading_root_flag_replaces_the_default_root() {
    let default = tempfile::tempdir().unwrap();
    let chosen = tempfile::tempdir().unwrap();
    let chosen_arg = chosen.path().to_str().unwrap();
    let (result, _) = go(default.path(), &["--root", chosen_arg, "--instance", "eureka", "brake", "release"], "op", 0);
    result.unwrap();
    assert!(chosen.path().join("eureka").join("control.cc").exists(), "the write went to the chosen root");
    assert_eq!(std::fs::read_dir(default.path()).unwrap().count(), 0, "the default root was not touched");
    // Without the flag the default applies.
    go(default.path(), &["--instance", "eureka", "brake", "release"], "op", 1).0.unwrap();
    assert!(default.path().join("eureka").join("control.cc").exists());
}

#[test]
fn a_root_flag_without_a_directory_or_an_instance_is_refused_and_writes_nothing() {
    let default = tempfile::tempdir().unwrap();
    for args in [&["--root"][..], &["--root", "/nonexistent-root-for-test"][..]] {
        assert!(go(default.path(), args, "op", 0).0.is_err(), "{args:?}");
    }
    assert_eq!(std::fs::read_dir(default.path()).unwrap().count(), 0);
    assert!(!std::path::Path::new("/nonexistent-root-for-test").exists());
}

fn shown(root: &std::path::Path) -> String {
    go(root, &["--instance", "eureka", "show"], "op", 0).1
}

#[test]
fn brake_hold_stops_once_with_the_instance_and_the_store_already_held() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    let mut seen_at_stop = String::new();
    let (result, out, calls) = go_stopping(root.path(), &["--instance", "eureka", "brake", "hold"], "alice", 1, |_| {
        seen_at_stop = shown(root.path());
        Ok(())
    });
    result.unwrap();
    assert_eq!(calls, ["eureka"]);
    assert!(seen_at_stop.contains("BrakeHeld"), "the write comes before the stop: {seen_at_stop}");
    assert!(out.contains("units: stopped mind-self@eureka:* mind-persona@eureka:*"), "{out}");
}

#[test]
fn only_brake_hold_calls_the_stopper() {
    let root = tempfile::tempdir().unwrap();
    let dial = ["--instance", "eureka", "dial", "set", "--heat", "1", "--base-cooldown-s", "60", "--base-run-usd", "1"];
    let commands: &[&[&str]] = &[
        &["--instance", "eureka", "brake", "release"],
        &dial,
        &["--instance", "eureka", "show"],
        &["--instance", "eureka", "brake"],
        &["--instance", "eureka", "brake", "toggle"],
        &["--instance", "eureka", "dial", "set", "--heat", "99", "--base-cooldown-s", "60", "--base-run-usd", "1"],
        &["--instance", "../evil", "brake", "release"],
        &["--instance", "eureka", "brake", "hold", "now"],
        &[],
    ];
    for args in commands {
        let (_, _, calls) = go_stopping(root.path(), args, "op", 0, |_| Ok(()));
        assert!(calls.is_empty(), "{args:?} called the stopper");
    }
    let (_, _, calls) = go_stopping(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 1, |_| Ok(()));
    assert_eq!(calls.len(), 1);
}

#[test]
fn a_failing_stopper_fails_the_hold_and_leaves_the_brake_held() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    let (result, out, calls) = go_stopping(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 1, |_| {
        Err(anyhow::anyhow!("systemctl stop failed with exit status 5"))
    });
    let error = format!("{:#}", result.unwrap_err());
    assert_eq!(calls.len(), 1);
    assert!(error.contains("units were not stopped") && error.contains("exit status 5"), "{error}");
    assert!(!out.contains("stopped"), "a failed stop printed success: {out}");
    assert!(shown(root.path()).contains("BrakeHeld"));
}

#[test]
fn a_failing_write_still_stops_and_the_hold_fails() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("eureka").join("control.cc")).unwrap();
    let (result, out, calls) = go_stopping(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 1, |_| Ok(()));
    let error = format!("{:#}", result.unwrap_err());
    assert_eq!(calls, ["eureka"], "stopping does not wait on the write");
    assert!(error.contains("brake was not written"), "{error}");
    assert!(!out.contains("stopped"), "{out}");

    let (result, _, calls) = go_stopping(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 2, |_| Err(anyhow::anyhow!("stop failed")));
    let error = format!("{:#}", result.unwrap_err());
    assert_eq!(calls.len(), 1);
    assert!(error.contains("brake was not written") && error.contains("units were not stopped"), "{error}");
}
