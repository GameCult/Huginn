//! The `mind-control` command driven end to end through `mind_body::cli::run`
//! against a temporary root, and the binary once as a process for its exit
//! status. Nothing touches `/etc`.

use chrono::{TimeZone, Utc};
use mind_body::cli::{operator_name, run};

fn go(root: &std::path::Path, args: &[&str], set_by: &str, secs: i64) -> (anyhow::Result<()>, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let mut out = Vec::new();
    let now = Utc.timestamp_opt(1_800_000_000 + secs, 0).unwrap();
    let result = run(&args, root, set_by, now, &mut out);
    (result, String::from_utf8(out).unwrap())
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
    go(root.path(), &["--instance", "eureka", "dial", "set", "--heat", "1", "--base-cooldown-s", "60", "--base-run-usd", "1"], "op", 1)
        .0
        .unwrap();
    let path = root.path().join("eureka").join("control.cc");
    let before = std::fs::read(&path).unwrap();
    for heat in ["2.0001", "0.0499", "0", "-1"] {
        let (r, _) = go(root.path(), &["--instance", "eureka", "dial", "set", "--heat", heat, "--base-cooldown-s", "60", "--base-run-usd", "1"], "op", 9);
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
