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
    assert!(out.contains("brake: absent") && out.contains("BrakeAbsent"), "{out}");

    go(root.path(), &["--instance", "eureka", "brake", "release"], "alice", 1).0.unwrap();
    let (_, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 2);
    assert!(out.contains("brake: released") && out.contains("set_by alice") && out.contains("BrakeAbsent"), "the swarm is still unreleased: {out}");

    go(root.path(), &["swarm", "brake", "release"], "bob", 3).0.unwrap();
    go(root.path(), &["swarm", "dial", "set", "--heat", "0.5", "--base-rate", "4", "--base-run", "5", "--k", "2"], "bob", 3).0.unwrap();
    let (_, out) = go(root.path(), &["--instance", "eureka", "show"], "op", 4);
    assert!(out.contains("Released") && out.contains("run_cap_usd: 2.5"), "{out}");

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
fn malformed_commands_are_refused_and_write_nothing() {
    let root = tempfile::tempdir().unwrap();
    let bad: &[&[&str]] = &[
        &[],
        &["show"],
        &["--instance", "eureka"],
        &["--instance", "eureka", "brake"],
        &["--instance", "eureka", "brake", "toggle"],
        &["--instance", "eureka", "dial", "set", "--heat", "1"],
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
fn the_binary_exits_2_for_check_while_held_and_0_after_release_with_a_swarm() {
    let root = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_mind-control"))
            .arg("--root")
            .arg(root.path())
            .args(["--instance", "eureka"])
            .args(args)
            .output()
            .unwrap()
    };
    // Absent store: refused, and the gate wrote nothing.
    let output = run(&["check"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("held: BrakeAbsent"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);

    // Released with no swarm is still refused; with a released swarm it opens.
    assert!(run(&["brake", "release"]).status.success());
    assert_eq!(run(&["check"]).status.code(), Some(2));
    let swarm = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_mind-control")).arg("--root").arg(root.path()).arg("swarm").args(args).output().unwrap()
    };
    assert!(swarm(&["brake", "release"]).status.success());
    assert!(swarm(&["dial", "set", "--heat", "0.5", "--base-rate", "4", "--base-run", "5", "--k", "2"]).status.success());
    let open = run(&["check"]);
    assert_eq!(open.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&open.stdout).contains("Released"));

    // Holding closes it again. The brake is written through the library: the
    // binary's `brake hold` would also ask systemd to stop units.
    mind_body::control::ControlWriter::open(root.path().join("eureka").join("control.cc"))
        .unwrap()
        .set_brake(false, Utc.timestamp_opt(1_800_000_009, 0).unwrap(), "op")
        .unwrap();
    let closed = run(&["check"]);
    assert_eq!(closed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&closed.stderr).contains("held: BrakeHeld"));
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

fn listing(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn check_out(root: &std::path::Path) -> (anyhow::Result<()>, String) {
    go(root, &["--instance", "eureka", "check"], "op", 0)
}

#[test]
fn check_refuses_every_reading_but_released() {
    let root = tempfile::tempdir().unwrap();
    let held = |root: &std::path::Path, reason: &str| {
        let (result, out) = check_out(root);
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains(reason), "{reason}: {error}");
        assert!(out.contains(reason), "{reason}: {out}");
    };
    held(root.path(), "BrakeAbsent");
    go(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 1).0.unwrap();
    held(root.path(), "BrakeHeld");
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 2).0.unwrap();
    held(root.path(), "BrakeAbsent");
    go(root.path(), &["swarm", "brake", "release"], "op", 3).0.unwrap();
    held(root.path(), "DialAbsent");
    // DialOutOfBounds cannot be written through the CLI: it is pinned in the crate's own tests.
    // An undecodable store is a held reading too.
    let broken = tempfile::tempdir().unwrap();
    std::fs::create_dir(broken.path().join("eureka")).unwrap();
    std::fs::write(broken.path().join("eureka").join("control.cc"), b"not a store").unwrap();
    held(broken.path(), "Undecodable");
}

#[test]
fn check_opens_on_release_with_a_swarm_and_is_per_instance() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    go(root.path(), &["swarm", "brake", "release"], "op", 1).0.unwrap();
    go(root.path(), &["swarm", "dial", "set", "--heat", "0.5", "--base-rate", "4", "--base-run", "5", "--k", "2"], "op", 1).0.unwrap();
    let (result, out) = check_out(root.path());
    result.unwrap();
    assert!(out.contains("Released"), "{out}");
    // Another instance's store is a different file: still absent, still refused.
    let (other, _) = go(root.path(), &["--instance", "other", "check"], "op", 2);
    assert!(format!("{:#}", other.unwrap_err()).contains("BrakeAbsent"));
}

#[test]
fn check_holds_for_the_named_instance_with_a_released_prefix_neighbour() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "hold"], "op", 0).0.unwrap();
    go(root.path(), &["swarm", "brake", "release"], "op", 0).0.unwrap();
    go(root.path(), &["swarm", "dial", "set", "--heat", "0.5", "--base-rate", "4", "--base-run", "5", "--k", "2"], "op", 0).0.unwrap();
    // Neighbours that share the held instance's text as a prefix, or only an edge of it, are released with a dial.
    for (i, neighbour) in ["eureka-two", "eureka2", "xeureka"].into_iter().enumerate() {
        let at = 1 + 2 * i as i64;
        go(root.path(), &["--instance", neighbour, "brake", "release"], "op", at).0.unwrap();
        let (open, _) = go(root.path(), &["--instance", neighbour, "check"], "op", 10);
        open.unwrap_or_else(|error| panic!("{neighbour} is released and must open: {error:#}"));
    }
    let (result, out) = check_out(root.path());
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("BrakeHeld") && out.contains("BrakeHeld"), "{error} / {out}");
}

#[test]
fn check_writes_nothing_and_creates_no_directory() {
    let root = tempfile::tempdir().unwrap();
    assert!(check_out(root.path()).0.is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);

    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    let file = root.path().join("eureka").join("control.cc");
    let before = (std::fs::read(&file).unwrap(), std::fs::metadata(&file).unwrap().modified().unwrap());
    let names_before = listing(&root.path().join("eureka"));
    let _ = check_out(root.path());
    let _ = check_out(root.path());
    assert_eq!((std::fs::read(&file).unwrap(), std::fs::metadata(&file).unwrap().modified().unwrap()), before);
    assert_eq!(listing(&root.path().join("eureka")), names_before);
}

#[test]
fn check_takes_no_extra_word_and_needs_an_instance() {
    let root = tempfile::tempdir().unwrap();
    go(root.path(), &["--instance", "eureka", "brake", "release"], "op", 0).0.unwrap();
    for args in [&["--instance", "eureka", "check", "now"][..], &["--instance", "eureka", "check", "show"][..], &["check"][..]] {
        let (result, out) = go(root.path(), args, "op", 1);
        assert!(format!("{:#}", result.unwrap_err()).contains("usage: mind-control"), "{args:?}");
        assert!(out.is_empty(), "{args:?}: {out}");
    }
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
    assert!(out.contains("units: stopped mind-self@eureka.service mind-persona@eureka.service"), "{out}");
}

#[test]
fn only_brake_hold_calls_the_stopper() {
    let root = tempfile::tempdir().unwrap();
    let dial = ["swarm", "dial", "set", "--heat", "1", "--base-rate", "4", "--base-run", "1", "--k", "2"];
    let commands: &[&[&str]] = &[
        &["--instance", "eureka", "brake", "release"],
        &dial,
        &["--instance", "eureka", "show"],
        &["--instance", "eureka", "brake"],
        &["--instance", "eureka", "brake", "toggle"],
        &["swarm", "dial", "set", "--heat", "99", "--base-rate", "4", "--base-run", "1", "--k", "2"],
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
    assert!(error.contains("the brake is held") && error.contains("exit status 5"), "{error}");
    assert!(!error.contains("were not stopped"), "{error}");
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
    assert!(error.contains("brake was not written") && error.contains("the stop failed") && !error.contains("were not stopped"), "{error}");
}

/// Runs the real binary's `brake hold` with a `systemctl` of ours first on
/// PATH, so the stopper main.rs wires is the one under test. The fake appends
/// its arguments to a record, one per line; `stop` exits with `exit`, and
/// `is-active` answers inactive for the Self unit and active for the Persona
/// unit, as systemd does when it stopped one and failed the other. The root is
/// returned so the store can be read afterwards.
#[cfg(unix)]
fn hold_through_the_binary(exit: i32) -> (std::process::Output, Vec<String>, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt;
    let bin = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let record = bin.path().join("recorded");
    let script = bin.path().join("systemctl");
    let body = format!(
        "#!/bin/sh\nprintf '%s\n' \"$@\" >> '{}'\ncase \"$1\" in\n  is-active) case \"$2\" in mind-self@eureka.service) echo inactive; exit 3;; *) echo active; exit 0;; esac;;\n  *) exit {exit};;\nesac\n",
        record.display()
    );
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mind-control"))
        .env("PATH", bin.path())
        .args(["--root", root.path().to_str().unwrap(), "--instance", "eureka", "brake", "hold"])
        .output()
        .unwrap();
    let recorded = std::fs::read_to_string(&record).map(|text| text.lines().map(str::to_string).collect()).unwrap_or_default();
    (output, recorded, root)
}

#[cfg(unix)]
#[test]
fn the_binary_hold_runs_systemctl_stop_on_exactly_the_units_it_starts() {
    let (output, recorded, _root) = hold_through_the_binary(0);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(recorded, ["stop", "--", "mind-self@eureka.service", "mind-persona@eureka.service"], "a stop that succeeds asks no state");
    assert!(String::from_utf8_lossy(&output.stdout).contains("units: stopped mind-self@eureka.service mind-persona@eureka.service"));
}

#[cfg(unix)]
#[test]
fn a_partial_stop_names_each_units_state_and_keeps_the_brake_held_with_exit_2() {
    let (output, recorded, root) = hold_through_the_binary(5);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        recorded,
        ["stop", "--", "mind-self@eureka.service", "mind-persona@eureka.service", "is-active", "mind-self@eureka.service", "is-active", "mind-persona@eureka.service"]
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exit status 5") && stderr.contains("the brake is held"), "{stderr}");
    assert!(stderr.contains("mind-self@eureka.service is inactive") && stderr.contains("mind-persona@eureka.service is active"), "{stderr}");
    assert!(!stderr.contains("were not stopped"), "{stderr}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("stopped"));
    assert!(shown(root.path()).contains("BrakeHeld"));
}

// ---- the arguments every organ binary shares ----

mod shared_arguments {
    use mind_body::cli::instance_and_endpoint;

    const USAGE: &str = "usage: test-organ --instance <slug>";

    fn parse(args: &[&str], env: Option<&str>) -> anyhow::Result<(eureka_pipeline::Slug, std::net::SocketAddr)> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        instance_and_endpoint(&args, env.map(str::to_string), USAGE)
    }

    #[test]
    fn the_flag_names_the_endpoint_and_wins_over_the_environment_which_is_the_fallback() {
        let (instance, address) = parse(&["--instance", "yggdrasil", "--endpoint", "rudp://10.77.0.1:17872"], Some("rudp://127.0.0.1:1")).unwrap();
        assert_eq!((instance.0.as_str(), address.to_string().as_str()), ("yggdrasil", "10.77.0.1:17872"));
        let (_, address) = parse(&["--endpoint", "rudp://10.77.0.1:17872", "--instance", "yggdrasil"], None).unwrap();
        assert_eq!(address.to_string(), "10.77.0.1:17872", "flag order does not matter");
        let (_, address) = parse(&["--instance", "yggdrasil"], Some("rudp://127.0.0.1:9")).unwrap();
        assert_eq!(address.to_string(), "127.0.0.1:9");
    }

    #[test]
    fn every_refusal_is_fixed_text_that_echoes_no_value() {
        let canary = "canary-value-91e7";
        let cases: Vec<(Vec<&str>, Option<&str>, &str)> = vec![
            (vec![], Some("rudp://127.0.0.1:9"), USAGE),
            (vec!["--instance"], None, USAGE),
            (vec!["--instance", "a", "--instance", canary], Some("rudp://127.0.0.1:9"), USAGE),
            (vec!["--instance", "a", "--endpoint", "rudp://127.0.0.1:9", "--endpoint", canary], None, USAGE),
            (vec!["--instance", "a", canary], Some("rudp://127.0.0.1:9"), USAGE),
            (vec!["--endpoint", "rudp://127.0.0.1:9"], None, USAGE),
            (vec!["--instance", "Canary/../x"], Some("rudp://127.0.0.1:9"), "the instance is not a slug"),
            (vec!["--instance", "a"], None, "no daemon endpoint: --endpoint or HUGINN_ENDPOINT"),
            (vec!["--instance", "a", "--endpoint", canary], None, "the endpoint is not rudp://<ip>:<port>"),
            (vec!["--instance", "a", "--endpoint", "http://127.0.0.1:9"], None, "the endpoint is not rudp://<ip>:<port>"),
            (vec!["--instance", "a", "--endpoint", "rudp://not-an-address"], None, "the endpoint is not rudp://<ip>:<port>"),
            (vec!["--instance", "a"], Some(canary), "the endpoint is not rudp://<ip>:<port>"),
        ];
        for (args, env, message) in cases {
            let error = parse(&args, env).unwrap_err();
            assert_eq!(format!("{error:#}"), message, "{args:?} {env:?}");
        }
    }
}

fn swarm_show(root: &std::path::Path) -> String {
    let (result, out) = go(root, &["swarm", "show"], "op", 0);
    result.unwrap();
    out
}

#[test]
fn swarm_verbs_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let shown = swarm_show(root.path());
    assert!(shown.contains("brake: absent") && shown.contains("dial: absent") && shown.contains("BrakeAbsent"), "{shown}");

    for args in [
        &["swarm", "brake", "release"][..],
        &["swarm", "dial", "set", "--heat", "1.5", "--base-rate", "4", "--base-run", "2", "--k", "3"],
        &["swarm", "share", "set", "weksa", "2.5"],
    ] {
        go(root.path(), args, "alice", 5).0.unwrap();
    }
    let shown = swarm_show(root.path());
    assert!(shown.contains("brake: released") && shown.contains("set_by alice"), "{shown}");
    assert!(shown.contains("dial: heat 1.5 base_rate_usd_per_h 4 base_run_usd 2 k 3"), "{shown}");
    assert!(shown.contains("share: weksa 2.5"), "{shown}");
    assert!(shown.contains("Released") && shown.contains("rate_usd_per_h: 6.0") && shown.contains("run_cap_usd: 3.0"), "{shown}");

    // Heat 0 stops the swarm and is one command away; the hold stops it too.
    go(root.path(), &["swarm", "dial", "set", "--heat", "0", "--base-rate", "4", "--base-run", "2", "--k", "3"], "alice", 6).0.unwrap();
    assert!(swarm_show(root.path()).contains("HeatZero"));
    go(root.path(), &["swarm", "brake", "hold"], "alice", 7).0.unwrap();
    assert!(swarm_show(root.path()).contains("BrakeHeld"));
}

#[test]
fn swarm_verbs_write_only_the_swarm_file_and_never_call_the_stopper() {
    let root = tempfile::tempdir().unwrap();
    let (result, _, calls) = go_stopping(root.path(), &["swarm", "brake", "hold"], "op", 0, |_| Ok(()));
    result.unwrap();
    assert!(calls.is_empty(), "the swarm hold has no instance to stop: {calls:?}");
    let names = listing(root.path());
    assert!(names.iter().all(|name| name.starts_with("swarm.cc")), "{names:?}");
}

#[test]
fn swarm_commands_refuse_bad_input_and_write_nothing() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        &["swarm"][..],
        &["swarm", "dial", "set", "--heat", "1"],
        &["swarm", "dial", "set", "--heat", "3", "--base-rate", "4", "--base-run", "2", "--k", "3"],
        &["swarm", "dial", "set", "--heat", "1", "--base-rate", "4", "--base-run", "2", "--k", "0"],
        &["swarm", "dial", "set", "--heat", "1", "--heat", "1", "--base-rate", "4", "--base-run", "2", "--k", "3"],
        &["swarm", "share", "set", "weksa", "0"],
        &["swarm", "share", "set", "weksa", "heavy"],
        &["swarm", "share", "set", "Not A Slug", "1"],
        &["swarm", "share", "set", "weksa"],
        &["swarm", "brake", "release", "now"],
    ] {
        assert!(go(root.path(), args, "op", 0).0.is_err(), "{args:?} was accepted");
    }
    assert!(listing(root.path()).is_empty(), "a refused command wrote something");
}
