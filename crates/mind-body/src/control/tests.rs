//! The reader's tests, in the crate because they move the reader's root with
//! the `#[cfg(test)]` seam: no caller outside the crate can. Every test works in
//! a temporary directory through the same file door the units use; nothing
//! touches `/etc`. They run as root in the verify container like the writer's.

use std::path::{Path, PathBuf};

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

/// A swarm dial at `heat` whose per-run cap before heat is `run_usd`.
fn dial(heat: &str, _unused: u32, run_usd: &str) -> SwarmDial {
    SwarmDial { heat: dec(heat), base_rate_usd_per_h: dec("5"), base_run_usd: dec(run_usd), k: 2, set_at: at(0), set_by: "op".into() }
}

/// The swarm store at `<root>/swarm.cc`: brake released and the given dial.
fn release_swarm(root: &Path, dial: SwarmDial) {
    let mut writer = SwarmWriter::open(swarm_path(root)).unwrap();
    writer.set_brake(true, at(0), "op").unwrap();
    writer.set_dial(dial).unwrap();
}

/// The store of the instance `eureka` under the root `dir`.
fn store_path(dir: &Path) -> PathBuf {
    dir.join("eureka").join("control.cc")
}

fn held(reason: HeldReason) -> Effective {
    Effective::Held { reason }
}

/// A written instance store with its brake released, and a released swarm.
fn released_store(dir: &Path, dial: SwarmDial) -> PathBuf {
    let path = store_path(dir);
    ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
    release_swarm(dir, dial);
    path
}

/// The real reader's answer for the store at `<root>/<instance>/control.cc`,
/// with the reader's root moved to `<root>` on this thread.
fn effective(path: &Path) -> Effective {
    let directory = path.parent().unwrap();
    let instance = Slug(directory.file_name().unwrap().to_str().unwrap().into());
    with_root(directory.parent().unwrap(), || read_effective(&instance))
}

#[test]
fn a_source_that_fails_reads_as_held() {
    struct Failing;
    impl ControlSource for Failing {
        fn snapshot(&self) -> Result<Vec<CultCacheEnvelope>> {
            Err(anyhow::anyhow!("disk gone"))
        }
    }
    assert_eq!(derive(&Failing, &Failing, &Slug("eureka".into())), Effective::Held { reason: HeldReason::Undecodable });
}

#[test]
fn an_absent_store_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(effective(&store_path(dir.path())), held(HeldReason::BrakeAbsent));
    assert!(!store_path(dir.path()).exists(), "reading must not create the store");
}

#[test]
fn an_absent_brake_reads_as_held_even_with_a_released_swarm() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(dir.path());
    release_swarm(dir.path(), dial("1", 0, "1"));
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
fn the_operators_last_write_is_what_is_in_force_and_provenance_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path(), dial("1", 3600, "1"));
    assert!(matches!(effective(&path), Effective::Released(_)));

    ControlWriter::open(&path).unwrap().set_brake(false, at(60), "alice").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld));
    let state = load_state(&FileSource::new(&path)).unwrap();
    let brake = state.brake.unwrap();
    assert_eq!((brake.released, brake.set_at, brake.set_by.as_str()), (false, at(60), "alice"));

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
        ControlWriter::open(control_path(root, &slug(instance)).unwrap()).unwrap().set_brake(true, at(0), "op").unwrap();
        release_swarm(root, dial("1", 0, "5"));
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

/// The swarm store: `<root>/swarm.cc`, read by `read_envelope`.
mod swarm {
    use super::*;

    fn swarm_file(dir: &Path) -> PathBuf {
        swarm_path(dir)
    }

    fn swarm_dial(heat: &str, rate: &str, run_usd: &str, k: u32) -> SwarmDial {
        SwarmDial { heat: dec(heat), base_rate_usd_per_h: dec(rate), base_run_usd: dec(run_usd), k, set_at: at(0), set_by: "op".into() }
    }

    fn share(instance: &str, weight: &str) -> Share {
        Share { instance: instance.into(), weight: dec(weight) }
    }

    /// A released swarm store with the given dial, through the real writer.
    fn released_swarm(dir: &Path, dial: SwarmDial) -> PathBuf {
        let path = swarm_file(dir);
        let mut writer = SwarmWriter::open(&path).unwrap();
        writer.set_brake(true, at(0), "op").unwrap();
        writer.set_dial(dial).unwrap();
        path
    }

    fn envelope_at(dir: &Path) -> SwarmEffective {
        with_root(dir, read_envelope)
    }

    fn released(dir: &Path) -> Envelope {
        match envelope_at(dir) {
            SwarmEffective::Released(envelope) => envelope,
            other => panic!("expected a released envelope, got {other:?}"),
        }
    }

    /// A swarm dial written around the writer's bounds check.
    fn write_unchecked(path: &Path, dial: &SwarmDial) {
        let mut cache = swarm_cache().unwrap();
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
        cache.pull_all_backing_stores().unwrap();
        cache.put(DIAL_KEY, dial).unwrap();
    }

    fn write_unchecked_share(path: &Path, key: &str, share: &Share) {
        let mut cache = swarm_cache().unwrap();
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path)).unwrap();
        cache.pull_all_backing_stores().unwrap();
        cache.put(key, share).unwrap();
    }

    fn slug(name: &str) -> Slug {
        Slug(name.into())
    }

    #[test]
    fn missing_or_corrupt_swarm_store_is_held() {
        let dir = tempfile::tempdir().unwrap();
        // No file at all.
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::BrakeAbsent));
        // Bytes that are not a store.
        std::fs::write(swarm_file(dir.path()), b"not a cultcache store").unwrap();
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::Undecodable));
        std::fs::remove_file(swarm_file(dir.path())).unwrap();
        // A brake and no dial.
        let path = swarm_file(dir.path());
        SwarmWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::DialAbsent));
        // Every out-of-bounds dial, written around the writer.
        for (what, bad) in [
            ("negative heat", swarm_dial("-0.1", "5", "2", 2)),
            ("heat above the maximum", swarm_dial("2.1", "5", "2", 2)),
            ("zero rate", swarm_dial("1", "0", "2", 2)),
            ("zero run cap", swarm_dial("1", "5", "0", 2)),
            ("zero k", swarm_dial("1", "5", "2", 0)),
        ] {
            write_unchecked(&path, &bad);
            assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::DialOutOfBounds), "{what}");
        }
        // Heat 0 is in bounds and stops everything.
        write_unchecked(&path, &swarm_dial("0", "5", "2", 2));
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::HeatZero));
    }

    #[test]
    fn a_held_swarm_brake_stops_everything_whatever_the_dial_says() {
        let dir = tempfile::tempdir().unwrap();
        let path = released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        let _ = released(dir.path());
        SwarmWriter::open(&path).unwrap().set_brake(false, at(60), "alice").unwrap();
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::BrakeHeld));
        SwarmWriter::open(&path).unwrap().set_brake(true, at(120), "bob").unwrap();
        let _ = released(dir.path());
        // Heat 0 stops it again, with the brake released.
        SwarmWriter::open(&path).unwrap().set_dial(swarm_dial("0", "5", "2", 2)).unwrap();
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::HeatZero));
        // The dial survived the brake writes, provenance included.
        let state = load_swarm_state(&FileSource::new(&path)).unwrap();
        assert_eq!(state.brake.unwrap().set_by, "bob");
        assert_eq!(state.dial.unwrap().heat, dec("0"));
    }

    #[test]
    fn a_share_that_is_out_of_bounds_or_not_keyed_by_its_instance_holds_the_swarm() {
        let dir = tempfile::tempdir().unwrap();
        let path = released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        write_unchecked_share(&path, "weksa", &share("weksa", "0"));
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::ShareOutOfBounds));
        write_unchecked_share(&path, "weksa", &share("weksa", "-1"));
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::ShareOutOfBounds));
        write_unchecked_share(&path, "weksa", &share("weksa", "1"));
        let _ = released(dir.path());
        write_unchecked_share(&path, "mallory", &share("weksa", "9"));
        assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::Undecodable));
    }

    #[test]
    fn the_swarm_dial_has_calibration_ceilings() {
        for (what, dial) in [
            ("base rate over the ceiling", swarm_dial("1", "100.01", "2", 2)),
            ("base run over the ceiling", swarm_dial("1", "5", "50.01", 2)),
            ("k over the ceiling", swarm_dial("1", "5", "2", 17)),
        ] {
            let dir = tempfile::tempdir().unwrap();
            assert!(SwarmWriter::open(swarm_file(dir.path())).unwrap().set_dial(dial.clone()).is_err(), "{what}: the writer refuses it");
            write_unchecked(&swarm_file(dir.path()), &dial);
            SwarmWriter::open(swarm_file(dir.path())).unwrap().set_brake(true, at(0), "op").unwrap();
            assert_eq!(envelope_at(dir.path()), SwarmEffective::Held(HeldReason::DialOutOfBounds), "{what}: the reader holds on it");
        }
        let dir = tempfile::tempdir().unwrap();
        released_swarm(dir.path(), swarm_dial("2", "100", "50", 16));
        assert!(matches!(envelope_at(dir.path()), SwarmEffective::Released(_)), "the ceilings are inclusive");
    }

    #[test]
    fn envelope_scales_by_heat() {
        for (heat, rate, run_cap) in [("0.5", "2.5", "1.25"), ("2", "10", "5")] {
            let dir = tempfile::tempdir().unwrap();
            released_swarm(dir.path(), swarm_dial(heat, "5", "2.5", 3));
            let envelope = released(dir.path());
            assert_eq!(envelope.rate_usd_per_h(), dec(rate), "rate at heat {heat}");
            assert_eq!(envelope.run_cap_usd(), dec(run_cap), "run cap at heat {heat}");
            assert_eq!(envelope.k(), 3);
        }
    }

    #[test]
    fn a_missing_share_is_weight_one_and_a_set_share_is_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        SwarmWriter::open(&path).unwrap().set_share(share("weksa", "3")).unwrap();
        let envelope = released(dir.path());
        assert_eq!(envelope.share(&slug("weksa")), dec("3"));
        assert_eq!(envelope.share(&slug("eureka")), Decimal::ONE);
    }

    /// An instance's own released reading, with a run cap of `run_usd`.
    fn instance_reading(instance: &str, run_usd: &str) -> Effective {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(instance).join("control.cc");
        ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
        release_swarm(dir.path(), dial("1", 0, run_usd));
        with_root(dir.path(), || read_effective(&slug(instance)))
    }

    #[test]
    fn grant_for_never_exceeds_cap() {
        let dir = tempfile::tempdir().unwrap();
        released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        let envelope = released(dir.path());
        let eureka = slug("eureka");
        let own = instance_reading("eureka", "10");
        for (budget, expected) in [("2.01", None), ("100", None), ("0", None), ("-1", None), ("2", Some("2")), ("0.5", Some("0.5")), ("0.01", Some("0.01"))] {
            let grant = envelope.grant_for(&eureka, &own, dec(budget));
            assert_eq!(grant.map(|g| g.run_cap_usd()), expected.map(dec), "budget {budget}");
        }
        let grant = envelope.grant_for(&eureka, &own, dec("1")).unwrap();
        assert_eq!(grant.instance(), &eureka);
    }

    #[test]
    fn grant_for_refuses_held_instance() {
        let dir = tempfile::tempdir().unwrap();
        released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        let envelope = released(dir.path());
        let eureka = slug("eureka");
        assert!(envelope.grant_for(&eureka, &Effective::Held { reason: HeldReason::BrakeHeld }, dec("1")).is_none());
        // Another instance released reading is no proof for this one.
        let weksa_reading = instance_reading("weksa", "10");
        assert!(envelope.grant_for(&eureka, &weksa_reading, dec("1")).is_none());
        assert!(envelope.grant_for(&slug("weksa"), &weksa_reading, dec("1")).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn the_swarm_store_is_written_by_root_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = released_swarm(dir.path(), swarm_dial("1", "5", "2", 2));
        let before = std::fs::read(&path).unwrap();
        for mode in [0o666, 0o664] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            let error = SwarmWriter::open(&path).unwrap().set_dial(swarm_dial("0", "5", "2", 2)).unwrap_err();
            assert!(error.downcast_ref::<UnsafeStore>().is_some(), "mode {mode:o}: {error:#}");
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before, "a refused write changed the store");
    }

    #[test]
    fn the_swarm_writer_refuses_out_of_bounds_values_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = swarm_file(dir.path());
        let mut writer = SwarmWriter::open(&path).unwrap();
        assert!(writer.set_dial(swarm_dial("2.5", "5", "2", 2)).is_err());
        assert!(writer.set_dial(swarm_dial("1", "5", "2", 0)).is_err());
        assert!(writer.set_share(share("weksa", "0")).is_err());
        assert!(writer.set_share(share("Not A Slug", "1")).is_err());
        assert!(!path.exists(), "a refused write created the store");
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "a refused write created a sibling");
    }
}
