//! The reader's tests, in the crate because they move the reader's root with
//! the `#[cfg(test)]` seam: no caller outside the crate can. Every test works in
//! a temporary directory through the same file door the units use; nothing
//! touches `/etc`. They run as root in the verify container like the writer's.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use cultcache_rs::{CultCache, DatabaseEntry, SingleFileMessagePackBackingStore};
use eureka_pipeline::Slug;
use rust_decimal::Decimal;
use std::str::FromStr;

use super::*;

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap()
}

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0).unwrap()
}

fn dial(heat: &str, cooldown: u32, run_usd: &str) -> BurnRate {
    BurnRate { heat: dec(heat), base_cooldown_s: cooldown, base_run_usd: dec(run_usd), set_at: at(0), set_by: "op".into() }
}

/// The store of the instance `eureka` under the root `dir`.
fn store_path(dir: &Path) -> PathBuf {
    dir.join("eureka").join("control.cc")
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

/// The real reader's answer for the store at `<root>/<instance>/control.cc`,
/// with the reader's root moved to `<root>` on this thread.
fn effective(path: &Path) -> Effective {
    let directory = path.parent().unwrap();
    let instance = Slug(directory.file_name().unwrap().to_str().unwrap().into());
    with_root(directory.parent().unwrap(), || read_effective(&instance))
}

/// A dial written around the writer's bounds check, so the reader's own check
/// is what is under test.
fn write_unchecked_dial(path: &Path, dial: &BurnRate) {
    let mut cache = CultCache::new();
    cache.register_entry_type::<BurnRate>().unwrap();
    cache.register_entry_type::<Brake>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put(DIAL_KEY, dial).unwrap();
}

#[test]
fn a_source_that_fails_reads_as_held() {
    struct Failing;
    impl ControlSource for Failing {
        fn snapshot(&self) -> Result<Vec<CultCacheEnvelope>> {
            Err(anyhow::anyhow!("disk gone"))
        }
    }
    assert_eq!(derive(&Failing, &Slug("eureka".into())), Effective::Held { reason: HeldReason::Undecodable });
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
    assert!(matches!(effective(&path), Effective::Released(_)));
}

#[test]
fn a_path_that_cannot_be_read_as_a_file_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    // A directory sits where the store should be: it exists and cannot be read as a file.
    let path = store_path(dir.path());
    std::fs::create_dir_all(&path).unwrap();
    assert_eq!(effective(&path), held(HeldReason::Undecodable));
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
            assert!(matches!(got, Effective::Released(_)), "heat {heat}: {got:?}");
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
    assert!(matches!(effective(&smallest), Effective::Released(_)));
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
        let Effective::Released(grant) = effective(&path) else { panic!("heat {heat} cooldown {cooldown} run {run_usd}: not released") };
        assert_eq!(grant.cadence(), cadence, "heat {heat} cooldown {cooldown} run {run_usd}");
        assert_eq!(grant.run_cap_usd(), cap, "heat {heat} cooldown {cooldown} run {run_usd}");
    }
}

#[test]
fn the_operators_last_write_is_what_is_in_force_and_provenance_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    assert!(matches!(effective(&path), Effective::Released(_)));

    ControlWriter::open(&path).unwrap().set_brake(false, at(60), "alice").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld));
    let state = load_state(&FileSource::new(&path)).unwrap();
    let brake = state.brake.unwrap();
    assert_eq!((brake.released, brake.set_at, brake.set_by.as_str()), (false, at(60), "alice"));
    // The dial survives the brake write untouched.
    assert_eq!(state.dial.unwrap().set_by, "op");

    ControlWriter::open(&path).unwrap().set_brake(true, at(120), "bob").unwrap();
    assert!(matches!(effective(&path), Effective::Released(_)));
    let brake = load_state(&FileSource::new(&path)).unwrap().brake.unwrap();
    assert_eq!((brake.released, brake.set_at, brake.set_by.as_str()), (true, at(120), "bob"));
}

#[test]
fn reading_writes_nothing_and_takes_no_lock_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    let store_dir = path.parent().unwrap();
    let listing = |dir: &Path| {
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        names.sort();
        names
    };
    // The writer left its sibling lock file; remove it, so a reader that wants one has to create it.
    for entry in std::fs::read_dir(store_dir).unwrap() {
        let entry = entry.unwrap().path();
        if entry != path {
            std::fs::remove_file(entry).unwrap();
        }
    }
    let before = (listing(store_dir), std::fs::read(&path).unwrap());
    assert!(matches!(effective(&path), Effective::Released(_)));
    assert_eq!((listing(store_dir), std::fs::read(&path).unwrap()), before);
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
    cache.register_entry_type::<crate::control::Brake>().unwrap();
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.put(crate::control::DIAL_KEY, &WrongDial { heat: heat.into() }).unwrap();
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


mod grant {
    //! A Grant is the proof that this instance's own control store released: the
    //! reader derives the path from the instance, and a launch refuses a Grant
    //! read for another instance before it asks the mind anything.

    use anyhow::{Result, anyhow};
    use cultnet_rs::Selection;
    use eureka_pipeline::{PipelineDocument, RunTurn};
    use huginn_mind::{PipelineAdmissionOutcome, PipelineSelectionPage};

    use super::*;
    use crate::launch::{Clock, Declined, LaunchOutcome, LaunchRequest, Launcher, Ports, open_and_launch};
    use crate::queue::MindPort;

    fn slug(name: &str) -> Slug {
        Slug(name.into())
    }

    fn release(root: &Path, instance: &str) {
        let mut writer = ControlWriter::open(control_path(root, &slug(instance)).unwrap()).unwrap();
        writer.set_brake(true, at(0), "op").unwrap();
        writer.set_dial(dial("1", 3600, "5")).unwrap();
    }

    fn read(root: &Path, instance: &str) -> Effective {
        with_root(root, || read_effective(&slug(instance)))
    }

    /// A mind that answers nothing: a launch that reaches it has gone past the
    /// instance check.
    struct SilentMind(Slug);

    impl MindPort for SilentMind {
        fn instance(&self) -> &Slug {
            &self.0
        }

        fn query(&self, _selection: &Selection) -> Result<PipelineSelectionPage> {
            Err(anyhow!("the launch asked the mind"))
        }

        fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
            Err(anyhow!("the launch admitted"))
        }
    }

    struct NoUnits;

    impl Launcher for NoUnits {
        fn start(&self, _instance: &Slug, _turn: RunTurn) -> Result<()> {
            Err(anyhow!("the launch started a unit"))
        }

        fn alive(&self, _instance: &Slug, _turn: RunTurn) -> bool {
            false
        }
    }

    struct Now;

    impl Clock for Now {
        fn now(&self) -> DateTime<Utc> {
            at(100)
        }
    }

    fn launch_as(mind_instance: &str, grant: Grant, turn: RunTurn) -> Result<LaunchOutcome> {
        let mind = SilentMind(slug(mind_instance));
        let ports = Ports { mind: &mind, launcher: &NoUnits, clock: &Now, host: "host" };
        open_and_launch(&ports, LaunchRequest { turn, claims: vec![], agent: "agent".into(), grant })
    }

    #[test]
    fn the_reader_makes_a_grant_for_the_instance_whose_store_it_read() {
        let root = tempfile::tempdir().unwrap();
        release(root.path(), "void");
        let Effective::Released(grant) = read(root.path(), "void") else { panic!("void's store is released") };
        assert_eq!(grant.instance(), &slug("void"));
        assert_eq!(grant.run_cap_usd(), dec("5"));
    }

    #[test]
    fn another_instances_released_store_yields_no_grant_for_this_one() {
        let root = tempfile::tempdir().unwrap();
        release(root.path(), "void");
        assert_eq!(read(root.path(), "eureka"), held(HeldReason::BrakeAbsent));
        for walk in ["../void", "void/..", "eureka/../void", "/void"] {
            assert_eq!(read(root.path(), walk), held(HeldReason::Undecodable), "{walk}");
        }
    }

    #[test]
    fn a_grant_read_for_one_instance_cannot_launch_another() {
        let root = tempfile::tempdir().unwrap();
        release(root.path(), "void");
        for turn in [RunTurn::SelfRun, RunTurn::PersonaTurn] {
            let Effective::Released(grant) = read(root.path(), "void") else { panic!("void's store is released") };
            // The mind is eureka's; the silent mind and the unit-less launcher fail loudly if the launch goes on.
            let outcome = launch_as("eureka", grant, turn).unwrap();
            assert_eq!(outcome, LaunchOutcome::Refused(Declined::GrantForOtherInstance), "{turn:?}");
        }
    }

    #[test]
    fn the_matching_grant_is_not_stopped_by_the_instance_check() {
        let root = tempfile::tempdir().unwrap();
        release(root.path(), "void");
        let Effective::Released(grant) = read(root.path(), "void") else { panic!("void's store is released") };
        let error = launch_as("void", grant, RunTurn::SelfRun).unwrap_err();
        assert!(format!("{error:#}").contains("asked the mind"), "{error:#}");
    }
}
