//! The brake-grant rule seen from outside the crate: the only reader takes the
//! instance, so whatever a caller builds elsewhere (a store in a temporary
//! directory, a symlink to another instance's directory, a copy of another
//! instance's store, the relative root `.`) is never what a Grant is made from.
//! The rest of the rule is held where it is decided, not here: the library's
//! public surface is pinned as a whole (tests/public_surface.rs: no new public
//! way to set a root or build a Grant), the process environment and command line
//! are unreadable by library code (clippy.toml, `#![forbid]` in lib.rs), and the
//! `compile_fail` examples on `read_effective` show the signature half. This is
//! the behavioural half against a forged store.

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
}
