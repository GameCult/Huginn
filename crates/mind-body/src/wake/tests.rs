//! The waker's tests: a real mind over a temporary directory, the real control
//! store written by the operator's writer and read by the real derivation, a
//! recording launcher and a settable clock. A wait is observed where it is
//! decided, as the `until` the cycle returns, never after a sleep.

use std::cell::Cell;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineKind as K, ResolutionOutcome, RunOperator, RunTurn, Slug};
use huginn_mind::{MindRefusal, PipelineAdmissionOutcome, PipelineSelectionPage};
use rust_decimal::Decimal;
use tempfile::TempDir;

use super::*;
use crate::control::{BurnRate, ControlWriter, FileSource};
use crate::launch::Launcher;
use crate::testkit::*;

/// Every call the units port receives, counted over a recording launcher.
struct Units {
    rec: Recording,
    calls: Cell<usize>,
}

impl Launcher for Units {
    fn start(&self, instance: &Slug, turn: RunTurn) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        self.rec.start(instance, turn)
    }

    fn alive(&self, instance: &Slug, turn: RunTurn) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.rec.alive(instance, turn)
    }
}

/// A mind that counts the queries and admissions it is asked for.
struct Counting<'a> {
    inner: &'a TestMind,
    queries: Cell<usize>,
    admits: Cell<usize>,
}

impl MindPort for Counting<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.queries.set(self.queries.get() + 1);
        self.inner.query(selection)
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.admits.set(self.admits.get() + 1);
        self.inner.admit(agent, session, documents)
    }
}

/// A mind that cannot be reached.
struct Unreachable(Slug);

impl MindPort for Unreachable {
    fn instance(&self) -> &Slug {
        &self.0
    }

    fn query(&self, _selection: &Selection) -> Result<PipelineSelectionPage> {
        Err(anyhow!("the mind could not be asked"))
    }

    fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        Err(anyhow!("the mind could not be asked"))
    }
}

struct Rig {
    mind: TestMind,
    launcher: Units,
    clock: Fixed,
    dir: TempDir,
}

fn dec(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap()
}

fn seconds(whole: i64, nanos: i64) -> Duration {
    Duration::seconds(whole) + Duration::nanoseconds(nanos)
}

impl Rig {
    fn new() -> Self {
        Self {
            mind: TestMind::seeded(),
            launcher: Units { rec: Recording::default(), calls: Cell::new(0) },
            clock: Fixed(Cell::new(at(0))),
            dir: TempDir::new().unwrap(),
        }
    }

    fn store(&self) -> std::path::PathBuf {
        self.dir.path().join(INSTANCE).join("control.cc")
    }

    /// The brake released and the dial set, through the operator's writer.
    fn release(&self, heat: &str, cooldown_s: u32, run_usd: &str) {
        let mut writer = ControlWriter::open(self.store()).unwrap();
        writer.set_brake(true, at(0), "op").unwrap();
        writer.set_dial(BurnRate { heat: dec(heat), base_cooldown_s: cooldown_s, base_run_usd: dec(run_usd), set_at: at(0), set_by: "op".into() }).unwrap();
    }

    fn cycle(&self) -> WakeOutcome {
        self.cycle_through(&self.mind).unwrap()
    }

    fn cycle_through(&self, mind: &dyn MindPort) -> Result<WakeOutcome> {
        let ports = Ports { mind, launcher: &self.launcher, clock: &self.clock, host: "yggdrasil-host" };
        super::cycle(&ports, &FileSource::new(self.store()))
    }

    /// A Persona turn of hers, opened and then ended as given.
    fn persona(&self, label: &str, ending: ResolutionOutcome) {
        self.mind.committed(vec![run(label, RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
        self.mind.committed(vec![close(run_ref(label), ending)]);
    }

    fn admitted(&self, label: &str) -> DateTime<Utc> {
        let stamp = self.mind.view(K::Run, &run_ref(label).id.0).unwrap().admission.admitted_at;
        DateTime::parse_from_rfc3339(&stamp).unwrap().with_timezone(&Utc)
    }

    fn runs(&self) -> usize {
        self.mind.ids(K::Run).len()
    }
}

#[test]
fn with_the_brake_held_or_the_dial_absent_a_cycle_queries_nothing_admits_nothing_and_calls_no_unit() {
    let cases: [(&str, HeldReason, fn(&Rig)); 4] = [
        ("no store", HeldReason::BrakeAbsent, |_| {}),
        ("brake held", HeldReason::BrakeHeld, |rig| {
            rig.release("1", 600, "5");
            ControlWriter::open(rig.store()).unwrap().set_brake(false, at(1), "op").unwrap();
        }),
        ("dial absent", HeldReason::DialAbsent, |rig| {
            ControlWriter::open(rig.store()).unwrap().set_brake(true, at(0), "op").unwrap();
        }),
        ("store undecodable", HeldReason::Undecodable, |rig| {
            rig.release("1", 600, "5");
            std::fs::write(rig.store(), b"junk").unwrap();
        }),
    ];
    for (name, reason, prepare) in cases {
        // No Persona turn yet, so a released brake would launch one.
        let rig = Rig::new();
        prepare(&rig);
        let counting = Counting { inner: &rig.mind, queries: Cell::new(0), admits: Cell::new(0) };
        let outcome = rig.cycle_through(&counting).unwrap();
        assert_eq!(outcome, WakeOutcome::Held { reason }, "{name}");
        assert_eq!((counting.queries.get(), counting.admits.get(), rig.launcher.calls.get(), rig.runs()), (0, 0, 0, 0), "{name}");
    }
}

#[test]
fn the_wait_is_the_grants_cadence_after_the_newest_persona_turn_and_ends_exactly_there() {
    // (heat, base cooldown, the cadence base_cooldown / heat): whole, halved,
    // and one that carries nanoseconds, so a clamp, offset or scale shows.
    let heats = [("1", 600, seconds(600, 0)), ("2", 600, seconds(300, 0)), ("1.5", 1000, seconds(666, 666_666_667)), ("0.05", 3600, seconds(72_000, 0))];
    for (heat, cooldown, cadence) in heats {
        let rig = Rig::new();
        rig.release(heat, cooldown, "5");
        rig.persona("p1", recorded());
        let until = rig.admitted("p1") + cadence;

        rig.clock.0.set(until - Duration::milliseconds(1));
        assert_eq!(rig.cycle(), WakeOutcome::NotYet { until }, "heat {heat}, a millisecond early");
        assert_eq!((rig.launcher.calls.get(), rig.runs()), (0, 1), "heat {heat}: waiting touches no unit and admits nothing");

        rig.clock.0.set(until);
        assert!(matches!(rig.cycle(), WakeOutcome::Launched { .. }), "heat {heat}, at the boundary");
        assert_eq!(rig.runs(), 2, "heat {heat}");
    }
}

#[test]
fn the_same_age_waits_under_one_dial_and_launches_under_another() {
    let age = |heat: &str| {
        let rig = Rig::new();
        rig.release(heat, 600, "5");
        rig.persona("p1", recorded());
        rig.clock.0.set(rig.admitted("p1") + Duration::seconds(400));
        rig.cycle()
    };
    assert!(matches!(age("1"), WakeOutcome::NotYet { .. }), "600 s cadence at 400 s");
    assert!(matches!(age("2"), WakeOutcome::Launched { .. }), "300 s cadence at 400 s");
}

#[test]
fn a_newer_self_run_or_operator_turn_does_not_move_the_boundary() {
    let rig = Rig::new();
    rig.release("1", 600, "5");
    rig.persona("p1", recorded());
    let until = rig.admitted("p1") + Duration::seconds(600);
    rig.mind.committed(vec![run("s1", RunTurn::SelfRun, RunOperator::Mind, &[])]);
    rig.mind.committed(vec![run("op1", RunTurn::PersonaTurn, RunOperator::Operator, &[])]);
    assert!(rig.admitted("s1") > rig.admitted("p1") && rig.admitted("op1") > rig.admitted("p1"));

    rig.clock.0.set(until - Duration::milliseconds(1));
    assert_eq!(rig.cycle(), WakeOutcome::NotYet { until });
    rig.clock.0.set(until);
    assert!(matches!(rig.cycle(), WakeOutcome::Launched { .. }), "the boundary is still her Persona turn's");
}

#[test]
fn an_ended_persona_turn_sets_the_boundary_whatever_ended_it_and_the_newest_wins() {
    for (name, ending) in [("recorded", recorded()), ("withdrawn", withdrawn())] {
        let rig = Rig::new();
        rig.release("1", 600, "5");
        rig.persona("p1", recorded());
        rig.persona("p2", ending);
        assert!(rig.admitted("p2") > rig.admitted("p1"), "{name}");
        let until = rig.admitted("p2") + Duration::seconds(600);
        rig.clock.0.set(until - Duration::milliseconds(1));
        assert_eq!(rig.cycle(), WakeOutcome::NotYet { until }, "{name}");
    }
}

#[test]
fn a_live_persona_turn_past_the_cadence_answers_busy_and_the_cycle_admits_nothing_itself() {
    let rig = Rig::new();
    rig.release("1", 600, "5");
    rig.mind.committed(vec![run("p1", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
    rig.launcher.rec.revive(RunTurn::PersonaTurn);
    rig.clock.0.set(rig.admitted("p1") + Duration::seconds(601));
    let counting = Counting { inner: &rig.mind, queries: Cell::new(0), admits: Cell::new(0) };
    assert_eq!(rig.cycle_through(&counting).unwrap(), WakeOutcome::Busy { run: run_ref("p1") });
    assert_eq!(counting.admits.get(), 0);
    assert!(rig.launcher.rec.started.borrow().is_empty());
    assert_eq!(rig.runs(), 1);
}

#[test]
fn the_first_released_cycle_launches_a_persona_run_with_no_claims_and_the_grants_cap() {
    let rig = Rig::new();
    // Decoys: a Self run of hers and an operator's Persona turn are not her Persona turns.
    rig.mind.committed(vec![run("s1", RunTurn::SelfRun, RunOperator::Mind, &[])]);
    rig.mind.committed(vec![run("op1", RunTurn::PersonaTurn, RunOperator::Operator, &[])]);
    rig.release("2", 600, "3.25");
    let WakeOutcome::Launched { run: opened } = rig.cycle() else { panic!("not launched") };
    let view = rig.mind.view(K::Run, &opened.id.0).unwrap();
    let PipelineDocument::Run(held) = &view.document else { panic!("not a run") };
    assert_eq!((held.operated_by, held.turn), (RunOperator::Mind, RunTurn::PersonaTurn));
    assert!(held.claims.is_empty() && held.campaigns.is_empty());
    assert_eq!(held.budget_usd.0, "6.5", "base_run_usd x heat");
    assert_eq!(held.host.0, "yggdrasil-host");
    assert_eq!(view.admission.provenance.agent.0, AGENT);
    assert_eq!(*rig.launcher.rec.started.borrow(), vec![(Slug(INSTANCE.into()), RunTurn::PersonaTurn)]);
}

#[test]
fn two_consecutive_cycles_launch_once_and_the_second_waits_or_answers_busy() {
    let rig = Rig::new();
    rig.release("1", 600, "5");
    let WakeOutcome::Launched { run: first } = rig.cycle() else { panic!("not launched") };
    let stamp = rig.mind.view(K::Run, &first.id.0).unwrap().admission.admitted_at;
    let until = DateTime::parse_from_rfc3339(&stamp).unwrap().with_timezone(&Utc) + Duration::seconds(600);
    assert_eq!(rig.cycle(), WakeOutcome::NotYet { until }, "straight after: within the cadence");
    rig.clock.0.set(until);
    assert_eq!(rig.cycle(), WakeOutcome::Busy { run: first }, "past the cadence with her turn still live");
    assert_eq!((rig.runs(), rig.launcher.rec.started.borrow().len()), (1, 1));
}

#[test]
fn a_unit_that_does_not_start_and_an_admission_that_is_refused_are_refusals_not_errors() {
    let rig = Rig::new();
    rig.release("1", 600, "5");
    rig.launcher.rec.fail.set(true);
    assert_eq!(rig.cycle(), WakeOutcome::Refused(Declined::StartFailed));

    let rig = Rig::new();
    rig.release("1", 600, "5");
    let refusing = Answering { inner: &rig.mind, outcome: PipelineAdmissionOutcome::Conflict { identities: vec![] } };
    assert_eq!(rig.cycle_through(&refusing).unwrap(), WakeOutcome::Refused(Declined::Conflict));
    assert!(rig.launcher.rec.started.borrow().is_empty());
}

#[test]
fn a_mind_that_cannot_be_reached_is_an_error_and_starts_nothing() {
    let rig = Rig::new();
    rig.release("1", 600, "5");
    assert!(rig.cycle_through(&Unreachable(Slug(INSTANCE.into()))).is_err());
    assert_eq!(rig.launcher.calls.get(), 0);
}

#[test]
fn an_outcome_prints_ids_and_codes_and_never_a_refusals_values() {
    let canary = "canary-8c1f";
    let refused = Declined::Admission(MindRefusal::RepoNotInCampaign { repo: canary.into() });
    assert_eq!(WakeOutcome::Refused(refused).to_string(), "refused: admission");
    assert_eq!(WakeOutcome::Held { reason: HeldReason::BrakeHeld }.to_string(), "held: BrakeHeld");
    assert_eq!(WakeOutcome::Launched { run: run_ref("p1") }.to_string(), format!("launched: {INSTANCE}:run:p1"));
    assert_eq!(WakeOutcome::Busy { run: run_ref("p1") }.to_string(), format!("busy: {INSTANCE}:run:p1"));
    assert_eq!(WakeOutcome::NotYet { until: at(0) }.to_string(), "not yet: until 2026-10-09T13:15:00+00:00");
}
