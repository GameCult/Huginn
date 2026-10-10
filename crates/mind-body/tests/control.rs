//! Behavioural tests for the brake, the dial and the one reader. Every test
//! works in a temporary directory through the same file door the units use;
//! nothing touches `/etc`, including the window probe, which points the real
//! binary at a tempdir with `--root`.
//!
//! The permission boundary is the filesystem's, but the writer refuses to write
//! through a directory, lock or store that is not root's or that group or world
//! can write, and creates its own files at fixed modes. The suite therefore runs
//! as root (the verify container does); the tests that need a second user become
//! uid 65534 with setpriv and skip with a message when they cannot.

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
    let root = Path::new(mind_body::control::CONTROL_ROOT);
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
        let instance = std::env::var("MIND_BODY_INSTANCE").unwrap_or_else(|_| "eureka".into());
        let path = Path::new(&std::env::var(ROOT).unwrap()).join(instance).join("control.cc");
        match role.as_str() {
            "write" => {
                let mut writer = ControlWriter::open(&path).unwrap();
                writer.set_brake(true, at(0), "op").unwrap();
                writer.set_dial(dial("1", 3600, "1")).unwrap();
            }
            "brake" => {
                ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
            }
            "attacker" => {
                // Spins on an open-for-write of the live store; any success is a hit.
                let stop = std::env::var("MIND_BODY_STOP").unwrap();
                let started = std::time::Instant::now();
                let (mut sweeps, mut seen) = (0u64, 0usize);
                while !Path::new(&stop).exists() && started.elapsed() < Duration::from_secs(120) {
                    // The live store, and anything else in its directory (the staged
                    // file CultCache writes before its rename is the wider window).
                    for _ in 0..200 {
                        let mut targets = vec![path.clone()];
                        if let Ok(entries) = std::fs::read_dir(path.parent().unwrap()) {
                            targets.extend(entries.flatten().map(|entry| entry.path()));
                        }
                        sweeps += 1;
                        seen = seen.max(targets.len());
                        for target in targets {
                            if std::fs::OpenOptions::new().write(true).open(&target).is_ok() {
                                std::process::exit(3);
                            }
                        }
                    }
                }
                eprintln!("attacker: {sweeps} sweeps, at most {seen} entries seen");
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

#[cfg(unix)]
mod unsafe_installs {
    use super::*;
    use mind_body::control::UnsafeStore;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn running_as_root() -> bool {
        Command::new("id").arg("-u").output().map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0").unwrap_or(false)
    }

    fn chmod(path: &Path, mode: u32) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    /// An instance directory at `dir_mode`, optionally with a lock and a store
    /// at the given modes, all root's (the suite runs as root).
    fn install(dir_mode: u32, lock_mode: Option<u32>, store_mode: Option<u32>) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let instance = root.path().join("eureka");
        std::fs::create_dir(&instance).unwrap();
        for (name, mode) in [("control.cc.lock", lock_mode), ("control.cc", store_mode)] {
            if let Some(mode) = mode {
                std::fs::write(instance.join(name), b"").unwrap();
                chmod(&instance.join(name), mode);
            }
        }
        chmod(&instance, dir_mode);
        (root, instance)
    }

    fn refusal(instance: &Path) -> UnsafeStore {
        let path = instance.join("control.cc");
        let error = ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").expect_err("an unsafe install must be refused");
        error.downcast::<UnsafeStore>().expect("the refusal is the typed UnsafeStore error")
    }

    type Case = (&'static str, u32, Option<u32>, Option<u32>, &'static str);

    #[test]
    fn a_group_or_world_writable_directory_lock_or_store_is_refused_and_nothing_is_written() {
        if !running_as_root() {
            eprintln!("SKIPPED: needs root to own the fixtures");
            return;
        }
        let cases: [Case; 5] = [
            ("world-writable directory", 0o777, None, None, ""),
            ("group-writable directory", 0o775, None, None, ""),
            ("world-writable lock", 0o755, Some(0o666), None, "control.cc.lock"),
            ("group-writable store", 0o755, Some(0o644), Some(0o664), "control.cc"),
            ("world-writable store", 0o755, Some(0o644), Some(0o666), "control.cc"),
        ];
        for (what, dir_mode, lock_mode, store_mode, culprit) in cases {
            let (_root, instance) = install(dir_mode, lock_mode, store_mode);
            let before: Vec<_> = std::fs::read_dir(&instance).unwrap().map(|entry| entry.unwrap().file_name()).collect();
            let unsafe_store = refusal(&instance);
            let expected = if culprit.is_empty() { instance.clone() } else { instance.join(culprit) };
            assert_eq!(unsafe_store.path, expected, "{what}: names the offending path");
            assert!(unsafe_store.mode & 0o022 != 0, "{what}: names the mode");
            assert!(unsafe_store.to_string().contains(&expected.display().to_string()), "{what}: the message names the path");
            let mut after: Vec<_> = std::fs::read_dir(&instance).unwrap().map(|entry| entry.unwrap().file_name()).collect();
            let mut before = before;
            before.sort();
            after.sort();
            assert_eq!(before, after, "{what}: a refused write creates nothing");
        }
    }

    #[test]
    fn a_directory_not_owned_by_root_is_refused() {
        if !running_as_root() {
            eprintln!("SKIPPED: needs root to chown");
            return;
        }
        let (_root, instance) = install(0o755, None, None);
        std::os::unix::fs::chown(&instance, Some(65534), Some(65534)).unwrap();
        let unsafe_store = refusal(&instance);
        assert_eq!((unsafe_store.path, unsafe_store.uid), (instance, 65534));
    }

    #[test]
    fn a_root_owned_install_without_group_or_world_write_is_accepted() {
        if !running_as_root() {
            eprintln!("SKIPPED: needs root to own the fixtures");
            return;
        }
        for dir_mode in [0o755, 0o750, 0o700] {
            let (_root, instance) = install(dir_mode, Some(0o644), Some(0o644));
            ControlWriter::open(instance.join("control.cc")).unwrap().set_brake(true, at(0), "op").unwrap();
        }
    }

    /// The window between CultCache's rename and the writer's chmod, probed by
    /// a non-root process spinning open-for-write on the live store while the
    /// real binary writes it under a starting umask of 000. The binary sets
    /// its own umask, so the staged file is never writable by anyone else.
    #[test]
    fn no_non_root_open_for_write_succeeds_while_the_binary_writes_under_umask_000() {
        if !running_as_root() || Command::new("setpriv").arg("--version").output().is_err() {
            eprintln!("SKIPPED: needs root and setpriv");
            return;
        }
        let instance = format!("window-probe-{}", std::process::id());
        // A root-owned 0755 tempdir is the control root; its drop removes whatever
        // the probe left, even after a failed assert.
        let work = tempfile::tempdir().unwrap();
        chmod(work.path(), 0o755);
        let minds = work.path().join("minds");
        std::fs::create_dir(&minds).unwrap();
        chmod(&minds, 0o755);
        let attacker_bin = work.path().join("attacker");
        std::fs::copy(std::env::current_exe().unwrap(), &attacker_bin).unwrap();
        chmod(&attacker_bin, 0o755);
        let stop = work.path().join("stop");
        let mut attacker = Command::new("setpriv")
            .args(["--reuid=65534", "--regid=65534", "--clear-groups"])
            .arg(&attacker_bin)
            .args(["--exact", "modes::mode_child", "--nocapture"])
            .env("MIND_BODY_MODE_CHILD", "attacker")
            .env("MIND_BODY_MODE_ROOT", &minds)
            .env("MIND_BODY_STOP", &stop)
            .env("MIND_BODY_INSTANCE", &instance)
            .spawn()
            .unwrap();
        let bin = env!("CARGO_BIN_EXE_mind-control");
        for round in 0..100 {
            let verb = if round % 2 == 0 { "release" } else { "hold" };
            let status = Command::new("sh")
                .args(["-c", "umask 000; exec \"$@\"", "sh", bin, "--root", minds.to_str().unwrap(), "--instance", &instance, "brake", verb])
                .status()
                .unwrap();
            assert!(status.success());
        }
        std::fs::write(&stop, b"").unwrap();
        let attacker_status = attacker.wait().unwrap();
        assert_eq!(attacker_status.code(), Some(0), "a non-root process opened the live store for write");
    }
}

#[cfg(unix)]
#[test]
fn directories_the_writer_creates_are_0755_ancestors_included_under_a_restrictive_umask() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let top = dir.path().join("gamecult");
    let path = top.join("minds").join("eureka").join("control.cc");
    // Create nothing but run the writer; the umask is the test process's own, so
    // the child role does the write under 077.
    let status = std::process::Command::new("sh")
        .args(["-c", "umask 077; exec \"$@\" --exact modes::mode_child --nocapture", "sh"])
        .arg(std::env::current_exe().unwrap())
        .env("MIND_BODY_MODE_CHILD", "brake")
        .env("MIND_BODY_MODE_ROOT", top.join("minds"))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(path.exists());
    for created in [&top, &top.join("minds"), &top.join("minds").join("eureka")] {
        assert_eq!(mode(created), 0o755, "{}", created.display());
    }
}
