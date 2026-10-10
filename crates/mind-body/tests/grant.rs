//! A Grant is the proof that this instance's own control store released. The
//! tests are an outside crate's view: the only reader takes a root and an
//! instance and derives the store path itself, and a launch refuses a Grant
//! read for another instance before it asks the mind anything.

use std::path::Path;

use anyhow::{Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, RunTurn, Slug};
use huginn_mind::{PipelineAdmissionOutcome, PipelineSelectionPage};
use mind_body::control::{BurnRate, ControlWriter, Effective, HeldReason, control_path, read_effective};
use mind_body::launch::{Clock, Declined, LaunchOutcome, LaunchRequest, Launcher, Ports, open_and_launch};
use mind_body::queue::MindPort;
use rust_decimal::Decimal;

fn slug(name: &str) -> Slug {
    Slug(name.into())
}

fn release(root: &Path, instance: &Slug) {
    let mut writer = ControlWriter::open(control_path(root, instance).unwrap()).unwrap();
    let at = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
    writer.set_brake(true, at, "op").unwrap();
    writer.set_dial(BurnRate { heat: Decimal::ONE, base_cooldown_s: 3600, base_run_usd: Decimal::new(5, 0), set_at: at, set_by: "op".into() }).unwrap();
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
        Utc.timestamp_opt(1_800_000_100, 0).unwrap()
    }
}

fn launch_as(mind_instance: &str, grant: mind_body::control::Grant, turn: RunTurn) -> Result<LaunchOutcome> {
    let mind = SilentMind(slug(mind_instance));
    let ports = Ports { mind: &mind, launcher: &NoUnits, clock: &Now, host: "host" };
    open_and_launch(&ports, LaunchRequest { turn, claims: vec![], agent: "agent".into(), grant })
}

#[test]
fn the_reader_makes_a_grant_for_the_instance_whose_store_it_read() {
    let root = tempfile::tempdir().unwrap();
    release(root.path(), &slug("void"));
    let Effective::Released(grant) = read_effective(root.path(), &slug("void")) else { panic!("void's store is released") };
    assert_eq!(grant.instance(), &slug("void"));
    assert_eq!(grant.run_cap_usd(), Decimal::new(5, 0));
}

#[test]
fn another_instances_released_store_yields_no_grant_for_this_one() {
    let root = tempfile::tempdir().unwrap();
    release(root.path(), &slug("void"));
    // The operator holds eureka's brake: eureka has no released store under this root.
    assert_eq!(read_effective(root.path(), &slug("eureka")), Effective::Held { reason: HeldReason::BrakeAbsent });
    // A path-shaped instance cannot reach void's store by walking.
    for walk in ["../void", "void/..", "eureka/../void", "/void"] {
        assert_eq!(read_effective(root.path(), &slug(walk)), Effective::Held { reason: HeldReason::Undecodable }, "{walk}");
    }
}

#[test]
fn a_grant_read_for_one_instance_cannot_launch_another() {
    let root = tempfile::tempdir().unwrap();
    release(root.path(), &slug("void"));
    for turn in [RunTurn::SelfRun, RunTurn::PersonaTurn] {
        let Effective::Released(grant) = read_effective(root.path(), &slug("void")) else { panic!("void's store is released") };
        // The mind is eureka's; the silent mind and the unit-less launcher fail loudly if the launch goes on.
        let outcome = launch_as("eureka", grant, turn).unwrap();
        assert_eq!(outcome, LaunchOutcome::Refused(Declined::GrantForOtherInstance), "{turn:?}");
    }
}

#[test]
fn the_matching_grant_is_not_stopped_by_the_instance_check() {
    let root = tempfile::tempdir().unwrap();
    release(root.path(), &slug("void"));
    let Effective::Released(grant) = read_effective(root.path(), &slug("void")) else { panic!("void's store is released") };
    // Past the check the silent mind is asked, so the launch errors there.
    let error = launch_as("void", grant, RunTurn::SelfRun).unwrap_err();
    assert!(format!("{error:#}").contains("asked the mind"), "{error:#}");
}
