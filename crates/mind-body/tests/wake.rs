//! The `mind-wake` binary as a process: its arguments and its exit codes. The
//! brake's root is a constant, so a process test can only ever see the absent
//! store of an instance no host has; the cycle's behaviour is tested in the
//! library against a real mind and a real store.

use std::process::{Command, Output};

const NO_SUCH_INSTANCE: &str = "wake-test-no-such-instance";

fn wake(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mind-wake")).args(args).env_remove("HUGINN_ENDPOINT").output().unwrap()
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
