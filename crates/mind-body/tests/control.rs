//! Behavioural tests for the brake, the dial and the one reader. Every test
//! works in a temporary directory through the same file door the units use;
//! nothing touches `/etc`.
//!
//! The permission boundary (no user she runs as can write the file) belongs to
//! the filesystem, not to this crate, and is not tested here: the verify
//! container runs as root, where a permission failure cannot be provoked.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};
use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, SingleFileMessagePackBackingStore};
use mind_body::control::{
    BRAKE_KEY, BurnRate, ControlSource, ControlWriter, Effective, FileSource, HeldReason, control_path, load_state, read_effective,
};
use rust_decimal::Decimal;
use std::str::FromStr;

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap()
}

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0).unwrap()
}

fn dial(heat: &str, cooldown: u32, run_usd: &str) -> BurnRate {
    BurnRate { heat: dec(heat), base_cooldown_s: cooldown, base_run_usd: dec(run_usd), set_at: at(0), set_by: "op".into() }
}

fn store_path(dir: &Path) -> PathBuf {
    dir.join("control.cc")
}

fn held(reason: HeldReason) -> Effective {
    Effective::Held { reason }
}

/// A written store: brake released and the given dial, through the real writer.
fn released_store(dir: &Path, dial: BurnRate) -> PathBuf {
    let path = store_path(dir);
    let mut writer = ControlWriter::open(&path).unwrap();
    writer.set_brake(true, at(0), "op").unwrap();
    writer.set_dial(dial).unwrap();
    path
}

fn effective(path: &Path) -> Effective {
    read_effective(&FileSource::new(path))
}

/// A dial written around the writer's bounds check, so the reader's own check
/// is what is under test.
fn write_unchecked_dial(path: &Path, dial: &BurnRate) {
    let mut cache = CultCache::new();
    cache.register_entry_type::<BurnRate>().unwrap();
    cache.register_entry_type::<mind_body::control::Brake>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put(mind_body::control::DIAL_KEY, dial).unwrap();
}

#[test]
fn an_absent_store_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(effective(&store_path(dir.path())), held(HeldReason::BrakeAbsent));
    assert!(!store_path(dir.path()).exists(), "reading must not create the store");
}

#[test]
fn an_absent_brake_reads_as_held_even_with_a_dial() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(dir.path());
    ControlWriter::open(&path).unwrap().set_dial(dial("1", 3600, "1")).unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeAbsent));
}

#[test]
fn an_undecodable_store_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let good = std::fs::read(&path).unwrap();

    // Garbage in place of the store.
    std::fs::write(&path, b"this is not a cultcache store").unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));

    // A store cut off mid-document.
    std::fs::write(&path, &good[..good.len() / 2]).unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));

    // The untouched store is still released, so the two reads above were the damage.
    std::fs::write(&path, &good).unwrap();
    assert!(matches!(effective(&path), Effective::Released { .. }));
}

#[test]
fn a_path_that_cannot_be_read_as_a_file_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    // A directory sits where the store should be: it exists and cannot be read as a file.
    let path = store_path(dir.path());
    std::fs::create_dir(&path).unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));
}

#[test]
fn a_source_that_fails_reads_as_held() {
    struct Failing;
    impl ControlSource for Failing {
        fn snapshot(&self) -> Result<Vec<CultCacheEnvelope>> {
            Err(anyhow!("disk gone"))
        }
    }
    assert_eq!(read_effective(&Failing), held(HeldReason::Undecodable));
}

#[test]
fn an_unknown_document_type_makes_the_store_undecodable() {
    #[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
    #[cultcache(type = "eureka.control.something_else.v1", schema = "Other")]
    struct Other {
        #[cultcache(key = 0)]
        value: u32,
    }
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let mut cache = CultCache::new();
    cache.register_entry_type::<Other>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path)).unwrap();
    cache.put("x", &Other { value: 1 }).unwrap();
    // The store now holds a released brake, a good dial and a foreign document.
    assert_eq!(effective(&path), held(HeldReason::Undecodable));
}

#[test]
fn a_brake_payload_that_does_not_decode_makes_the_store_undecodable() {
    // Same type id as the brake, a different shape: the payload cannot decode as a Brake.
    #[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
    #[cultcache(type = "eureka.control.brake.v1", schema = "ControlBrake")]
    struct WrongBrake {
        #[cultcache(key = 0)]
        released: String,
    }
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(dir.path());
    let mut cache = CultCache::new();
    cache.register_entry_type::<WrongBrake>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(&path)).unwrap();
    cache.put(BRAKE_KEY, &WrongBrake { released: "yes".into() }).unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));
}

#[test]
fn a_decode_failure_outranks_a_held_brake() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    ControlWriter::open(&path).unwrap().set_brake(false, at(5), "op").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld));
    std::fs::write(&path, b"junk").unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));
}

#[test]
fn a_released_brake_with_no_dial_reads_held_dial_absent() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(dir.path());
    ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
    assert_eq!(effective(&path), held(HeldReason::DialAbsent));
}

#[test]
fn dial_bounds_are_inclusive_at_both_ends() {
    for (heat, expected_in) in [("0.05", true), ("2.0", true), ("0.0499", false), ("2.0001", false), ("0", false), ("-1", false), ("3", false)] {
        let dir = tempfile::tempdir().unwrap();
        let path = released_store_unchecked(dir.path(), dial(heat, 3600, "1"));
        let got = effective(&path);
        if expected_in {
            assert!(matches!(got, Effective::Released { .. }), "heat {heat}: {got:?}");
        } else {
            assert_eq!(got, held(HeldReason::DialOutOfBounds), "heat {heat}");
        }
    }
}

#[test]
fn a_zero_base_cooldown_or_a_non_positive_run_cost_is_out_of_bounds() {
    for bad in [dial("1", 0, "1"), dial("1", 3600, "0"), dial("1", 3600, "-0.5")] {
        let dir = tempfile::tempdir().unwrap();
        let path = released_store_unchecked(dir.path(), bad.clone());
        assert_eq!(effective(&path), held(HeldReason::DialOutOfBounds), "{bad:?}");
    }
    let dir = tempfile::tempdir().unwrap();
    let smallest = released_store_unchecked(dir.path(), dial("1", 1, "0.000001"));
    assert!(matches!(effective(&smallest), Effective::Released { .. }));
}

fn released_store_unchecked(dir: &Path, bad: BurnRate) -> PathBuf {
    let path = store_path(dir);
    ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
    write_unchecked_dial(&path, &bad);
    path
}

#[test]
fn cadence_and_run_cap_are_the_dial_divided_and_multiplied_by_heat() {
    for (heat, cooldown, run_usd, cadence, cap) in [
        ("0.5", 3600u32, "5", Duration::from_secs(7200), dec("2.5")),
        ("2.0", 3600, "5", Duration::from_secs(1800), dec("10.0")),
        ("1", 600, "3", Duration::from_secs(600), dec("3")),
        ("0.05", 90, "4", Duration::from_secs(1800), dec("0.20")),
        ("1.5", 10, "2", Duration::new(6, 666_666_667), dec("3.0")),
        ("2.0", 1, "0.25", Duration::from_millis(500), dec("0.500")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = released_store(dir.path(), dial(heat, cooldown, run_usd));
        assert_eq!(effective(&path), Effective::Released { cadence, run_cap_usd: cap }, "heat {heat} cooldown {cooldown} run {run_usd}");
    }
}

#[test]
fn the_operators_last_write_is_what_is_in_force_and_provenance_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    assert!(matches!(effective(&path), Effective::Released { .. }));

    ControlWriter::open(&path).unwrap().set_brake(false, at(60), "alice").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld));
    let state = load_state(&FileSource::new(&path)).unwrap();
    let brake = state.brake.unwrap();
    assert_eq!((brake.released, brake.set_at, brake.set_by.as_str()), (false, at(60), "alice"));
    // The dial survives the brake write untouched.
    assert_eq!(state.dial.unwrap().set_by, "op");

    ControlWriter::open(&path).unwrap().set_brake(true, at(120), "bob").unwrap();
    assert!(matches!(effective(&path), Effective::Released { .. }));
    let brake = load_state(&FileSource::new(&path)).unwrap().brake.unwrap();
    assert_eq!((brake.released, brake.set_at, brake.set_by.as_str()), (true, at(120), "bob"));
}

#[test]
fn setting_the_dial_replaces_it_whole_and_keeps_the_brake() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let mut next = dial("0.5", 100, "8");
    next.set_at = at(30);
    next.set_by = "carol".into();
    ControlWriter::open(&path).unwrap().set_dial(next.clone()).unwrap();
    let state = load_state(&FileSource::new(&path)).unwrap();
    assert_eq!(state.dial, Some(next));
    assert!(state.brake.unwrap().released);
}

#[test]
fn the_writer_refuses_an_out_of_bounds_dial_and_leaves_the_store_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let before = std::fs::read(&path).unwrap();
    for bad in [dial("2.0001", 3600, "1"), dial("0.0499", 3600, "1"), dial("1", 0, "1"), dial("1", 3600, "0")] {
        assert!(ControlWriter::open(&path).unwrap().set_dial(bad).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn reading_writes_nothing_and_takes_no_lock_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let listing = |dir: &Path| {
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        names.sort();
        names
    };
    // The writer left its sibling lock file; remove it, so a reader that wants one has to create it.
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap().path();
        if entry != path {
            std::fs::remove_file(entry).unwrap();
        }
    }
    let before = (listing(dir.path()), std::fs::read(&path).unwrap());
    assert!(matches!(effective(&path), Effective::Released { .. }));
    assert_eq!((listing(dir.path()), std::fs::read(&path).unwrap()), before);
}

#[test]
fn the_control_path_is_derived_from_a_valid_instance_only() {
    use eureka_pipeline::Slug;
    let root = Path::new("/etc/gamecult/minds");
    assert_eq!(control_path(root, &Slug("eureka".into())).unwrap(), root.join("eureka").join("control.cc"));
    for bad in ["", "../etc", "a/b", "A b", "eureka/.."] {
        assert!(control_path(root, &Slug(bad.into())).is_err(), "{bad:?}");
    }
}

/// A dial document under the dial's type id whose payload is not a dial.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.burn_rate.v1", schema = "ControlBurnRate")]
struct WrongDial {
    #[cultcache(key = 0)]
    heat: String,
}

fn write_wrong_dial(path: &Path, heat: &str) {
    let mut cache = CultCache::new();
    cache.register_entry_type::<WrongDial>().unwrap();
    cache.register_entry_type::<mind_body::control::Brake>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put(mind_body::control::DIAL_KEY, &WrongDial { heat: heat.into() }).unwrap();
}

#[test]
fn a_dial_that_does_not_decode_reads_held_whatever_the_brake_says() {
    for brake_released in [true, false] {
        for heat in ["not a number", "NaN", ""] {
            let dir = tempfile::tempdir().unwrap();
            let path = store_path(dir.path());
            ControlWriter::open(&path).unwrap().set_brake(brake_released, at(0), "op").unwrap();
            write_wrong_dial(&path, heat);
            assert_eq!(
                effective(&path),
                held(HeldReason::Undecodable),
                "brake released={brake_released}, dial heat {heat:?}: a dial that cannot be read is never replaced by a default"
            );
        }
    }
}

#[cfg(unix)]
mod modes {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    const CHILD: &str = "MIND_BODY_MODE_CHILD";
    const ROOT: &str = "MIND_BODY_MODE_ROOT";

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// The body of the two re-executed children; with no CHILD set it does
    /// nothing, so a plain run of the suite passes it vacuously.
    #[test]
    fn mode_child() {
        let Ok(role) = std::env::var(CHILD) else { return };
        let path = Path::new(&std::env::var(ROOT).unwrap()).join("eureka").join("control.cc");
        match role.as_str() {
            "write" => {
                let mut writer = ControlWriter::open(&path).unwrap();
                writer.set_brake(true, at(0), "op").unwrap();
                writer.set_dial(dial("1", 3600, "1")).unwrap();
            }
            "brake" => {
                ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
            }
            "intruder" => {
                assert!(ControlWriter::open(&path).and_then(|mut writer| writer.set_brake(false, at(1), "intruder")).is_err());
                assert!(ControlWriter::open(Path::new(&std::env::var(ROOT).unwrap()).join("ghost").join("control.cc"))
                    .and_then(|mut writer| writer.set_brake(true, at(1), "intruder"))
                    .is_err());
                assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
                assert!(std::fs::OpenOptions::new().write(true).open(path.with_file_name("control.cc.lock")).is_err());
                assert!(matches!(effective(&path), Effective::Released { .. }), "a non-root user can still read the store");
            }
            other => panic!("unknown role {other}"),
        }
    }

    fn spawn_child(role: &str, root: &Path, exe: &Path, prefix: &[&str], umask: &str) -> std::process::ExitStatus {
        let script = format!("umask {umask}; exec \"$@\" --exact modes::mode_child --nocapture");
        let mut command = Command::new(prefix.first().copied().unwrap_or("sh"));
        if prefix.is_empty() {
            command.args(["-c", &script, "sh"]).arg(exe);
        } else {
            command.args(&prefix[1..]).args(["sh", "-c", &script, "sh"]).arg(exe);
        }
        command.env(CHILD, role).env(ROOT, root).status().unwrap()
    }

    fn written_under(role: &str, umask: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let exe = std::env::current_exe().unwrap();
        assert!(spawn_child(role, dir.path(), &exe, &[], umask).success());
        let control = dir.path().join("eureka");
        (dir, control)
    }

    #[test]
    fn the_store_is_created_0755_and_0644_whatever_the_operators_umask() {
        for umask in ["000", "022", "077"] {
            let (_dir, control) = written_under("brake", umask);
            // One write only: a second write would set what the first left.
            assert_eq!(mode(&control), 0o755, "directory under umask {umask}");
            assert_eq!(mode(&control.join("control.cc")), 0o644, "store under umask {umask}");
            assert_eq!(mode(&control.join("control.cc.lock")), 0o644, "lock under umask {umask}");
        }
    }

    fn running_as_root() -> bool {
        Command::new("id").arg("-u").output().map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0").unwrap_or(false)
    }

    #[test]
    fn a_non_root_user_cannot_write_the_store_but_can_read_it() {
        if !running_as_root() || Command::new("setpriv").arg("--version").output().is_err() {
            eprintln!("SKIPPED: needs root and setpriv (the verify container has both) to become uid 65534");
            return;
        }
        let (dir, _control) = written_under("write", "000");
        // A copy of the test binary the unprivileged user can execute.
        let exe = dir.path().join("test-bin");
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let status = spawn_child("intruder", dir.path(), &exe, &["setpriv", "--reuid=65534", "--regid=65534", "--clear-groups"], "022");
        assert!(status.success(), "the intruder child asserted a refusal that did not happen");
    }
}
