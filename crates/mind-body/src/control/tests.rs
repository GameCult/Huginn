//! The reader's tests, in the crate because they move the reader's root with
//! the `#[cfg(test)]` seam: no caller outside the crate can. Every test works in
//! a temporary directory through the same file door the units use; nothing
//! touches `/etc`. They run as root in the verify container like the writer's.

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use cultcache_rs::{CultCache, DatabaseEntry, SingleFileMessagePackBackingStore};
use eureka_pipeline::Slug;

use super::*;

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0).unwrap()
}

/// The store of the instance `eureka` under the root `dir`.
fn store_path(dir: &Path) -> PathBuf {
    dir.join("eureka").join("control.cc")
}

fn held(reason: HeldReason) -> Effective {
    Effective::Held { reason }
}

/// The real reader's answer for the store at `<root>/<instance>/control.cc`,
/// with the reader's root moved to `<root>` on this thread.
fn effective(path: &Path) -> Effective {
    let directory = path.parent().unwrap();
    let instance = Slug(directory.file_name().unwrap().to_str().unwrap().into());
    with_root(directory.parent().unwrap(), || read_effective(&instance))
}

#[test]
fn an_absent_store_reads_as_held() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(effective(&store_path(dir.path())), held(HeldReason::BrakeAbsent));
    assert!(!store_path(dir.path()).exists(), "reading must not create the store");
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

    fn envelope_at(dir: &Path) -> SwarmEffective {
        with_root(dir, read_envelope)
    }

    fn slug(name: &str) -> Slug {
        Slug(name.into())
    }

}

/// The swarm store with its brake released, and nothing else in it.
fn release_swarm(root: &Path) {
    SwarmWriter::open(swarm_path(root)).unwrap().set_brake(true, at(0), "op").unwrap();
}

/// A written instance store with its brake released, and a released swarm.
fn released_store(dir: &Path) -> PathBuf {
    let path = store_path(dir);
    ControlWriter::open(&path).unwrap().set_brake(true, at(0), "op").unwrap();
    release_swarm(dir);
    path
}

#[test]
fn both_brakes_released_is_a_grant_for_the_instance_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path());
    let Effective::Released(grant) = effective(&path) else { panic!("held") };
    assert_eq!(grant.instance(), &Slug("eureka".into()));
}

#[test]
fn either_held_brake_or_an_absent_one_holds_with_its_own_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path());
    SwarmWriter::open(swarm_path(dir.path())).unwrap().set_brake(false, at(1), "op").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld), "a held swarm brake stops every instance");
    release_swarm(dir.path());
    ControlWriter::open(&path).unwrap().set_brake(false, at(2), "op").unwrap();
    assert_eq!(effective(&path), held(HeldReason::BrakeHeld), "a held instance brake stops her");
    let other = tempfile::tempdir().unwrap();
    ControlWriter::open(store_path(other.path())).unwrap().set_brake(true, at(0), "op").unwrap();
    assert_eq!(effective(&store_path(other.path())), held(HeldReason::BrakeAbsent), "no swarm store is no swarm brake");
}

/// A document of the dollar-metering days, standing in for what an old `swarm.cc` holds.
#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.swarm_dial.v1", schema = "OldSwarmDial")]
struct OldDial {
    #[cultcache(key = 0)]
    heat: String,
}

#[derive(Clone, Debug, PartialEq, Eq, DatabaseEntry)]
#[cultcache(type = "eureka.control.share.v1", schema = "OldShare")]
struct OldShare {
    #[cultcache(key = 0)]
    weight: String,
}

#[test]
fn an_old_swarm_store_holding_a_dial_or_a_share_reads_as_held_undecodable_and_is_never_migrated() {
    for old in ["dial", "share"] {
        let dir = tempfile::tempdir().unwrap();
        let path = released_store(dir.path());
        let mut cache = CultCache::new();
        cache.register_entry_type::<Brake>().unwrap();
        cache.register_entry_type::<OldDial>().unwrap();
        cache.register_entry_type::<OldShare>().unwrap();
        cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(swarm_path(dir.path()))).unwrap();
        cache.pull_all_backing_stores().unwrap();
        match old {
            "dial" => {
                cache.put("dial", &OldDial { heat: "1".into() }).unwrap();
            }
            _ => {
                cache.put("eureka", &OldShare { weight: "1".into() }).unwrap();
            }
        }
        assert_eq!(effective(&path), held(HeldReason::Undecodable), "{old}: the brake in the same file does not rescue it");
        assert_eq!(read_swarm_for_test(dir.path()), SwarmEffective::Held(HeldReason::Undecodable));
    }
}

fn read_swarm_for_test(root: &Path) -> SwarmEffective {
    with_root(root, read_envelope)
}

#[test]
fn the_stores_are_written_world_readable_and_root_only_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = released_store(dir.path());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in [path.as_path(), swarm_path(dir.path()).as_path()] {
            assert_eq!(std::fs::metadata(file).unwrap().permissions().mode() & 0o777, 0o644, "{}", file.display());
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// A named instance store with its brake released, and a released swarm.
fn release(root: &Path, name: &str) {
    ControlWriter::open(root.join(name).join("control.cc")).unwrap().set_brake(true, at(0), "op").unwrap();
    release_swarm(root);
}
