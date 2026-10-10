//! The brake-grant rule seen from outside the crate: the only reader takes the
//! instance, so whatever a caller builds elsewhere (a store in a temporary
//! directory, a symlink to another instance's directory, a copy of another
//! instance's store, the relative root `.`, a tree that every environment
//! variable points at) is never what a Grant is made from.
//! The signature half of the rule (no root argument, no source reader, no
//! struct literal, no test constructor) is the `compile_fail` examples on
//! `read_effective`; this is the behavioural half, committed because it is the
//! only defence of the rule against a forged store.

#![cfg(unix)]

use std::path::Path;

use chrono::{TimeZone, Utc};
use eureka_pipeline::Slug;
use mind_body::control::{BurnRate, ControlWriter, Effective, HeldReason, control_path, read_effective};
use rust_decimal::Decimal;

/// A name no real install has a store for under the real root.
const FORGED: &str = "forge-probe-instance";
const OTHER: &str = "forge-probe-other";

fn release(root: &Path, instance: &str) {
    let mut writer = ControlWriter::open(control_path(root, &Slug(instance.into())).unwrap()).unwrap();
    let at = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
    writer.set_brake(true, at, "forger").unwrap();
    writer
        .set_dial(BurnRate { heat: Decimal::ONE, base_cooldown_s: 3600, base_run_usd: Decimal::new(5, 0), set_at: at, set_by: "forger".into() })
        .unwrap();
}

fn reads(instance: &str) -> Effective {
    read_effective(&Slug(instance.into()))
}

#[test]
fn no_forged_store_yields_a_grant() {
    assert_eq!(reads(FORGED), Effective::Held { reason: HeldReason::BrakeAbsent }, "the real root has no store for the probe's name");

    // A released store for the name in a root of the forger's choosing.
    let tmp = tempfile::tempdir().unwrap();
    release(tmp.path(), FORGED);
    assert!(matches!(reads(FORGED), Effective::Held { .. }), "store in a temporary root");

    // A symlink from the name to another instance's released directory.
    let linked = tempfile::tempdir().unwrap();
    release(linked.path(), OTHER);
    std::os::unix::fs::symlink(linked.path().join(OTHER), linked.path().join(FORGED)).unwrap();
    assert!(matches!(reads(FORGED), Effective::Held { .. }), "symlink to another instance");

    // A copy of another instance's store under the name.
    let copied = tempfile::tempdir().unwrap();
    release(copied.path(), OTHER);
    std::fs::create_dir_all(copied.path().join(FORGED)).unwrap();
    std::fs::copy(copied.path().join(OTHER).join("control.cc"), copied.path().join(FORGED).join("control.cc")).unwrap();
    assert!(matches!(reads(FORGED), Effective::Held { .. }), "copy of another instance's store");

    // The relative root '.': the working directory holds <name>/control.cc.
    let here = tempfile::tempdir().unwrap();
    release(here.path(), FORGED);
    let before = std::env::current_dir().unwrap();
    std::env::set_current_dir(here.path()).unwrap();
    let relative = reads(FORGED);
    std::env::set_current_dir(before).unwrap();
    assert!(matches!(relative, Effective::Held { .. }), "relative root");

    // A root named by the environment. The forged tree is laid out for a value
    // used as the root itself and for one with `minds` joined on, and every
    // variable the process has, with the names an implementation would likely
    // read, is pointed at it. A variable of a name neither list holds is not
    // reached; the library build having no `std::env` reader is the rest of it.
    let environed = tempfile::tempdir().unwrap();
    release(environed.path(), FORGED);
    std::fs::create_dir_all(environed.path().join("minds")).unwrap();
    release(&environed.path().join("minds"), FORGED);
    let value = environed.path().as_os_str().to_owned();
    let mut names: Vec<std::ffi::OsString> = std::env::vars_os().map(|(name, _)| name).collect();
    names.extend(GUESSED_ROOT_VARIABLES.iter().map(Into::into));
    let saved: Vec<_> = names.iter().map(|name| (name.clone(), std::env::var_os(name))).collect();
    // SAFETY: this file's one test is the only thread touching the environment.
    unsafe {
        for name in &names {
            std::env::set_var(name, &value);
        }
    }
    let through_environment = reads(FORGED);
    unsafe {
        for (name, before) in saved {
            match before {
                Some(before) => std::env::set_var(name, before),
                None => std::env::remove_var(name),
            }
        }
    }
    assert!(matches!(through_environment, Effective::Held { .. }), "root from an environment variable");
}

/// Names a caller-chosen root would plausibly be read from.
const GUESSED_ROOT_VARIABLES: [&str; 22] = [
    "MIND_CONTROL_ROOT",
    "MIND_BODY_ROOT",
    "MIND_ROOT",
    "MINDS_ROOT",
    "MINDS_DIR",
    "CONTROL_ROOT",
    "CONTROL_DIR",
    "GAMECULT_MINDS",
    "GAMECULT_ROOT",
    "HUGINN_ROOT",
    "HUGINN_HOME",
    "EUREKA_ROOT",
    "EUREKA_HOME",
    "EUREKA_INSTANCE_ROOT",
    "STATE_DIRECTORY",
    "CONFIGURATION_DIRECTORY",
    "RUNTIME_DIRECTORY",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "HOME",
    "TMPDIR",
];
