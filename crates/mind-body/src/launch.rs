//! The one way her organs open a run and start its unit.
//!
//! `open_and_launch` is the whole path, in this order: (0) the in-force runs of
//! hers of the request's turn are read, a live one (its unit active, or
//! admitted within `START_GRACE_S`) answers Busy and a dead one is to be closed
//! Recorded; (1) every cut-spec claim must be in the queue; (2) no claim may
//! trip the repetition breaker; (3) the run is admitted, in the one batch that
//! closes the dead holders, so liveness is decided by the in-force record alone
//! and the unit table is only the evidence for a close; (4) the unit is started,
//! only after that batch commits. The run is the grant (target invariant
//! run-is-the-grant), so there is no pressure file, lease or breaker state
//! beside it, and a unit never starts without a committed run. A run opens
//! only against a `Grant`, which `read_effective` alone makes and which names
//! the instance it was read for: a Grant of another instance is refused. A
//! Persona turn's run opens with no claims, so a request that carries any is
//! refused. The primitive
//! owns no file and stops no unit: stopping her units is the operator's brake
//! hold, which is another cut's.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{
    Date, Label, PipelineDocument, PipelineKind, PipelineQuestion, PipelineRef, PipelineResolution, PipelineRun, QuestionOption, ResolutionOutcome,
    RunOperator, RunTurn, Slug, pipeline_key,
};
use huginn_mind::{MindRefusal, PipelineAdmissionOutcome, PipelineDocumentSummary, PipelineFacts};

use crate::control::Grant;
use crate::queue::{Breaker, MindPort, any_of, breaker, citing, headers, in_force, of_kind, queue};

/// How long a run just admitted may have no active unit before its slot is
/// treated as dead: `systemctl start --no-block` returns before the unit runs.
pub const START_GRACE_S: i64 = 120;

/// Starts the unit that carries a run, and says whether it is still running.
/// Nothing here stops a unit.
pub trait Launcher {
    /// Units are named by instance and turn alone (ruling mind-unit-names), so
    /// liveness is keyed the same way.
    fn start(&self, instance: &Slug, turn: RunTurn) -> Result<()>;
    fn alive(&self, instance: &Slug, turn: RunTurn) -> bool;
}

/// The time the primitive reads: run labels, dates and receipts.
pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// What `systemctl` answered: whether it succeeded, and what it printed.
pub struct SystemctlReply {
    pub success: bool,
    pub stdout: String,
}

/// The one door to `systemctl`, so tests never reach a real unit.
pub trait Systemctl {
    fn run(&self, args: &[&str]) -> Result<SystemctlReply>;
}

struct RealSystemctl;

impl Systemctl for RealSystemctl {
    fn run(&self, args: &[&str]) -> Result<SystemctlReply> {
        let output = std::process::Command::new("systemctl").args(args).output().map_err(|_| anyhow!("systemctl could not be run"))?;
        Ok(SystemctlReply { success: output.status.success(), stdout: String::from_utf8_lossy(&output.stdout).into_owned() })
    }
}

/// Her units under systemd, templated by instance:
/// `mind-persona@<instance>.service` and
/// `mind-self@<instance>.service` (ruling mind-unit-names: one live run per
/// instance and turn, so the run label names no unit). The launcher holds no instance: the
/// caller names it on every call, and `open_and_launch` always names the
/// mind's own.
pub struct SystemdLauncher {
    systemctl: Box<dyn Systemctl>,
}

impl SystemdLauncher {
    pub fn new() -> Self {
        Self::over(Box::new(RealSystemctl))
    }

    pub(crate) fn over(systemctl: Box<dyn Systemctl>) -> Self {
        Self { systemctl }
    }
}

impl Default for SystemdLauncher {
    fn default() -> Self {
        Self::new()
    }
}

/// The unit of one turn: the template by turn, then the instance, held to the
/// leaf's grammar first so it cannot escape the name. The error names nothing
/// of the value. Start and alive both name the unit here.
pub(crate) fn unit(instance: &Slug, turn: RunTurn) -> Result<String> {
    instance.validate_slug().map_err(|_| anyhow!("the instance is not a unit name"))?;
    let template = match turn {
        RunTurn::PersonaTurn => "mind-persona",
        RunTurn::SelfRun => "mind-self",
    };
    Ok(format!("{template}@{}.service", instance.0))
}

impl Launcher for SystemdLauncher {
    fn start(&self, instance: &Slug, turn: RunTurn) -> Result<()> {
        let reply = self.systemctl.run(&["start", "--no-block", &unit(instance, turn)?])?;
        if reply.success { Ok(()) } else { Err(anyhow!("systemctl did not start the unit")) }
    }

    fn alive(&self, instance: &Slug, turn: RunTurn) -> bool {
        let Ok(unit) = unit(instance, turn) else {
            return false;
        };
        self.systemctl.run(&["is-active", &unit]).is_ok_and(|reply| matches!(reply.stdout.trim(), "active" | "activating" | "reloading"))
    }
}

/// A run's label from the instant it opens: UTC, to the millisecond. Two
/// launches in one millisecond collide on the run's key, admission answers
/// Conflict, and nothing launches.
pub fn run_label(now: DateTime<Utc>) -> Label {
    Label(now.format("mind-%Y%m%dT%H%M%S%3fZ").to_string())
}

pub struct Ports<'a> {
    pub mind: &'a dyn MindPort,
    pub launcher: &'a dyn Launcher,
    pub clock: &'a dyn Clock,
    /// The `host` the run records.
    pub host: &'a str,
}

pub struct LaunchRequest {
    pub turn: RunTurn,
    pub claims: Vec<PipelineRef>,
    /// The admitting agent's name on the receipt.
    pub agent: String,
    /// The reading of a released brake and an in-bounds dial; its cap is the
    /// run's budget. Taken by value, so one reading opens at most one run.
    pub grant: Grant,
}

/// Why admission did not open the run: its own refusal, or one of the two
/// answers that mean the run's key is taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Declined {
    Admission(MindRefusal),
    Conflict,
    AlreadyAdmitted,
    /// The run committed, its unit did not start, and the run was withdrawn.
    StartFailed,
    /// The Grant was read for another instance than the mind's own.
    GrantForOtherInstance,
    /// A Persona turn's run opens with no claims (ruling wake-target).
    PersonaRunTakesNoClaims,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchOutcome {
    Launched {
        run: PipelineRef,
    },
    NotQueued {
        item: PipelineRef,
    },
    Tripped {
        item: PipelineRef,
        question: PipelineRef,
    },
    /// The live holder of the request's turn, from step 0 or from admission.
    Busy {
        run: PipelineRef,
    },
    Refused(Declined),
}

fn date(now: DateTime<Utc>) -> Date {
    Date(now.format("%Y-%m-%d").to_string())
}

/// The root of a full pipeline id: the campaign slug of a campaign's document.
fn root_of(item: &PipelineRef) -> &str {
    item.id.0.split(':').next().unwrap_or_default()
}

fn key_of(document: &PipelineDocument) -> Result<String> {
    pipeline_key(document).map_err(|_| anyhow!("the document could not be keyed"))
}

/// Whether a run admitted at `admitted_at` has had `START_GRACE_S` to start
/// its unit. A stamp that does not parse has not.
fn past_grace(admitted_at: &str, now: DateTime<Utc>) -> bool {
    DateTime::parse_from_rfc3339(admitted_at).is_ok_and(|at| now.signed_duration_since(at) >= Duration::seconds(START_GRACE_S))
}

/// The resolution that closes a dead run of hers: Recorded, not Withdrawn, so
/// a unit that keeps dying counts as an empty run and trips the breaker. Its
/// sequence is the run's next, counting a closure that was itself withdrawn.
fn close_dead(mind: &dyn MindPort, run: &PipelineRef, now: DateTime<Utc>) -> Result<PipelineDocument> {
    let closures = Selection { cites: Some(citing(run, Some("subject"))), ..of_kind(PipelineKind::Resolution) };
    let latest = headers(mind, &closures)?
        .iter()
        .filter_map(|header| match header.facts {
            PipelineFacts::Resolution { sequence, .. } => Some(sequence),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    Ok(PipelineDocument::Resolution(PipelineResolution {
        subject: run.clone(),
        sequence: latest + 1,
        outcome: ResolutionOutcome::Recorded { reason: "unit not active".into() },
        rationale: "The run's unit was not active when its turn's slot was next wanted.".into(),
        resolved_on: date(now),
    }))
}

/// The in-force runs of hers (operated by the mind) of `turn` on `instance`,
/// read from typed facts: the holders of that turn's slot. The one reader of
/// the slot, for step 0 and for checking a holder admission names.
fn holders(mind: &dyn MindPort, instance: &Slug, turn: RunTurn) -> Result<Vec<PipelineDocumentSummary>> {
    let runs = headers(mind, &in_force(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0])))?;
    Ok(runs
        .into_iter()
        .filter(|run| matches!(run.facts, PipelineFacts::Run { turn: held, operated_by: RunOperator::Mind, .. } if held == turn))
        .collect())
}

pub fn open_and_launch(ports: &Ports, request: LaunchRequest) -> Result<LaunchOutcome> {
    let LaunchRequest { turn, claims, agent, grant } = request;
    let instance = ports.mind.instance().clone();
    if grant.instance() != &instance {
        return Ok(LaunchOutcome::Refused(Declined::GrantForOtherInstance));
    }
    if turn == RunTurn::PersonaTurn && !claims.is_empty() {
        return Ok(LaunchOutcome::Refused(Declined::PersonaRunTakesNoClaims));
    }
    let now = ports.clock.now();
    let label = run_label(now);

    // Step 0: a live holder of this turn's slot answers Busy; a dead one is closed
    // in the batch that admits whatever comes next.
    let mut batch: Vec<PipelineDocument> = Vec::new();
    let mut ending: Vec<PipelineRef> = Vec::new();
    for holder in holders(ports.mind, &instance, turn)? {
        if ports.launcher.alive(&instance, turn) || !past_grace(&holder.admission.admitted_at, now) {
            return Ok(LaunchOutcome::Busy { run: holder.id });
        }
        batch.push(close_dead(ports.mind, &holder.id, now)?);
        ending.push(holder.id);
    }

    if turn == RunTurn::SelfRun {
        let open = queue(ports.mind, &instance, &ending)?;
        if let Some(item) = claims.iter().find(|claim| claim.kind == PipelineKind::CutSpec && !open.contains(claim)) {
            return Ok(LaunchOutcome::NotQueued { item: item.clone() });
        }
        for item in &claims {
            if breaker(ports.mind, item, &ending)? == Breaker::Trip {
                let question = PipelineDocument::Question(breaker_question(item, &label, now));
                let id = PipelineRef { kind: PipelineKind::Question, id: key_of(&question)?.as_str().into() };
                batch.push(question);
                return Ok(match committed(ports.mind.admit(&agent, &label.0, batch)?) {
                    Ok(()) => LaunchOutcome::Tripped { item: item.clone(), question: id },
                    Err(declined) => LaunchOutcome::Refused(declined),
                });
            }
        }
    }

    let mut campaigns: Vec<Slug> = Vec::new();
    for claim in &claims {
        let root = Slug(root_of(claim).to_string());
        if !campaigns.contains(&root) {
            campaigns.push(root);
        }
    }
    let run = PipelineDocument::Run(PipelineRun {
        instance: instance.clone(),
        label: label.clone(),
        operated_by: RunOperator::Mind,
        turn,
        host: ports.host.into(),
        started_on: date(now),
        budget_usd: grant.run_cap_usd().normalize().to_string().as_str().into(),
        claims,
        campaigns,
    });
    let opened = PipelineRef { kind: PipelineKind::Run, id: key_of(&run)?.as_str().into() };
    batch.push(run);
    match ports.mind.admit(&agent, &label.0, batch)? {
        // The holder is a claim until the mind shows it: a run of this instance's own, of this turn, in force.
        PipelineAdmissionOutcome::Refused(MindRefusal::AlreadyLive { run: named }) => {
            let held = named.to_ref();
            return Ok(if holders(ports.mind, &instance, turn)?.iter().any(|holder| holder.id == held) {
                LaunchOutcome::Busy { run: held }
            } else {
                LaunchOutcome::Refused(Declined::Admission(MindRefusal::AlreadyLive { run: named }))
            });
        }
        answer => {
            if let Err(declined) = committed(answer) {
                return Ok(LaunchOutcome::Refused(declined));
            }
        }
    }
    if ports.launcher.start(&instance, turn).is_err() {
        let withdrawal = PipelineResolution {
            subject: opened,
            sequence: 1,
            outcome: ResolutionOutcome::Withdrawn { reason: "the unit failed to start".into() },
            rationale: "The run committed and its unit did not start, so it holds no work.".into(),
            resolved_on: date(now),
        };
        return match committed(ports.mind.admit(&agent, &label.0, vec![PipelineDocument::Resolution(withdrawal)])?) {
            Ok(()) => Ok(LaunchOutcome::Refused(Declined::StartFailed)),
            Err(_) => Err(anyhow!("the run could not be withdrawn after its unit failed to start")),
        };
    }
    Ok(LaunchOutcome::Launched { run: opened })
}

/// Whether admission landed the batch; if not, why.
fn committed(outcome: PipelineAdmissionOutcome) -> Result<(), Declined> {
    match outcome {
        PipelineAdmissionOutcome::Committed { .. } => Ok(()),
        PipelineAdmissionOutcome::Refused(refusal) => Err(Declined::Admission(refusal)),
        PipelineAdmissionOutcome::Conflict { .. } => Err(Declined::Conflict),
        PipelineAdmissionOutcome::AlreadyAdmitted { .. } => Err(Declined::AlreadyAdmitted),
    }
}

/// `breaker-<run label>`: one question per trip, since each trip is a launch
/// with a label of its own.
fn breaker_label(label: &Label) -> Label {
    Label(format!("breaker-{}", label.0))
}

/// The question a tripped item raises, in the item's own campaign.
fn breaker_question(item: &PipelineRef, label: &Label, now: DateTime<Utc>) -> PipelineQuestion {
    PipelineQuestion {
        campaign: Slug(root_of(item).to_string()),
        label: breaker_label(label),
        title: "Repeated runs on one item admitted nothing".into(),
        question:
            "The last runs on this item left nothing that cites it, so it launches no more. Answering this reopens the item and starts a fresh count."
                .into(),
        options: vec![
            QuestionOption { label: "reopen".into(), text: "Rule on the item, which reopens it.".into() },
            QuestionOption { label: "retire".into(), text: "Resolve the item so it is no longer worked.".into() },
        ],
        recommended: "reopen".into(),
        depends: vec![],
        raised_in: Some(item.clone()),
        asked_on: date(now),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeSet;
    use std::time::Duration as StdDuration;

    use eureka_pipeline::PipelineKind as K;
    use huginn_mind::{PipelineAdmissionOutcome as Outcome, PipelineStatus};
    use rust_decimal::Decimal;

    use super::*;
    use crate::testkit::*;

    struct Rig {
        mind: TestMind,
        launcher: Recording,
        clock: Fixed,
    }

    impl Rig {
        fn new() -> Self {
            Self { mind: TestMind::seeded(), launcher: Recording::default(), clock: Fixed(Cell::new(at(0))) }
        }

        fn launch(&self, turn: RunTurn, claims: &[PipelineRef], cap: Decimal) -> LaunchOutcome {
            self.launch_through(&self.mind, turn, claims, cap)
        }

        fn launch_through(&self, mind: &dyn MindPort, turn: RunTurn, claims: &[PipelineRef], cap: Decimal) -> LaunchOutcome {
            self.try_launch_through(mind, turn, claims, cap).unwrap()
        }

        fn try_launch_through(&self, mind: &dyn MindPort, turn: RunTurn, claims: &[PipelineRef], cap: Decimal) -> Result<LaunchOutcome> {
            let ports = Ports { mind, launcher: &self.launcher, clock: &self.clock, host: "yggdrasil-host" };
            let grant = Grant::for_test(Slug(INSTANCE.into()), StdDuration::from_secs(60), cap);
            open_and_launch(&ports, LaunchRequest { turn, claims: claims.to_vec(), agent: "agent-x".into(), grant })
        }

        fn at(&self, milliseconds: i64) {
            self.clock.0.set(at(milliseconds));
        }

        /// The clock `seconds` after the run with this id was admitted.
        fn age(&self, run: &str, seconds: i64) {
            let admitted = self.mind.view(K::Run, run).unwrap().admission.admitted_at;
            let admitted = DateTime::parse_from_rfc3339(&admitted).unwrap().with_timezone(&Utc);
            self.clock.0.set(admitted + Duration::seconds(seconds));
        }

        fn started(&self) -> usize {
            self.launcher.started.borrow().len()
        }

        fn runs(&self) -> usize {
            self.mind.ids(K::Run).len()
        }

        /// A run of hers in force, with a unit that is running.
        fn seed(&self, label: &str, turn: RunTurn, claims: &[PipelineRef]) {
            self.mind.committed(vec![run(label, turn, RunOperator::Mind, claims)]);
            self.launcher.revive(turn);
        }

        fn in_force(&self, run: &str) -> bool {
            self.mind.view(K::Run, &run_ref(run).id.0).unwrap().status == PipelineStatus::InForce
        }
    }

    fn five() -> Decimal {
        Decimal::new(5, 0)
    }

    fn the_run(view: &huginn_mind::PipelineDocumentView) -> &PipelineRun {
        let PipelineDocument::Run(run) = &view.document else { panic!("not a run") };
        run
    }

    fn the_closure(rig: &Rig, run: &str, sequence: u32) -> (PipelineResolution, String) {
        let view = rig.mind.view(K::Resolution, &format!("{INSTANCE}:resolution:run.{run}.n{sequence}")).expect("the closure");
        let PipelineDocument::Resolution(closure) = view.document else { panic!("not a resolution") };
        (closure, view.admission.receipt_id)
    }

    #[test]
    fn a_queued_spec_opens_a_run_that_records_its_launch_and_then_its_unit_starts() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        let LaunchOutcome::Launched { run } = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()) else { panic!("not launched") };
        assert_eq!(run.id.0, "yggdrasil:run:mind-20261009T131500000Z");
        let view = rig.mind.view(K::Run, &run.id.0).unwrap();
        let held = the_run(&view);
        assert_eq!((held.operated_by, held.turn), (RunOperator::Mind, RunTurn::SelfRun));
        assert_eq!((held.host.0.as_str(), held.started_on.0.as_str(), held.budget_usd.0.as_str()), ("yggdrasil-host", "2026-10-09", "5"));
        assert_eq!(held.claims, vec![spec_ref("x")]);
        assert_eq!(held.campaigns, vec![Slug(CAMPAIGN.into())]);
        assert_eq!(view.admission.provenance.agent.0, "agent-x");
        assert_eq!(view.admission.provenance.session.0, "mind-20261009T131500000Z");
        assert_eq!(*rig.launcher.started.borrow(), vec![(Slug(INSTANCE.into()), RunTurn::SelfRun)]);
    }

    #[test]
    fn the_unit_started_names_the_minds_instance() {
        let rig = Rig::new();
        rig.launch(RunTurn::PersonaTurn, &[], five());
        let started = rig.launcher.started.borrow();
        assert_eq!(started[0].0, *rig.mind.instance());
    }

    #[test]
    fn the_runs_budget_is_the_grants_cap_at_two_caps() {
        let rig = Rig::new();
        for (n, (cap, want)) in [(Decimal::new(5, 0), "5"), (Decimal::new(750, 2), "7.5"), (Decimal::new(125, 1), "12.5")].into_iter().enumerate() {
            rig.at(n as i64);
            let LaunchOutcome::Launched { run } = rig.launch(RunTurn::PersonaTurn, &[], cap) else { panic!("not launched") };
            let view = rig.mind.view(K::Run, &run.id.0).unwrap();
            assert_eq!(the_run(&view).budget_usd.0, want);
            assert_eq!(the_run(&view).turn, RunTurn::PersonaTurn);
            assert!(the_run(&view).claims.is_empty() && the_run(&view).campaigns.is_empty());
            // One live Persona turn of hers at a time: end this one.
            rig.mind.committed(vec![close(run, recorded())]);
        }
        assert_eq!(rig.started(), 3, "each Persona turn ended before the next opened, so none was refused");
    }

    #[test]
    fn a_live_persona_turn_is_busy_to_a_persona_launch_and_does_not_block_a_self_run() {
        let rig = Rig::new();
        let LaunchOutcome::Launched { run } = rig.launch(RunTurn::PersonaTurn, &[], five()) else { panic!("not launched") };
        rig.at(1);
        assert_eq!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Busy { run });
        assert_eq!((rig.runs(), rig.started()), (1, 1), "Busy admits and starts nothing");
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn run_labels_are_utc_to_the_millisecond() {
        assert_eq!(run_label(at(123)).0, "mind-20261009T131500123Z");
        assert_ne!(run_label(at(123)), run_label(at(124)));
    }

    #[test]
    fn two_launches_in_one_millisecond_open_one_run() {
        let rig = Rig::new();
        assert!(matches!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Launched { .. }));
        // A Self run holds another slot, so only the key (instance and label) collides.
        let second = rig.launch(RunTurn::SelfRun, &[], Decimal::new(6, 0));
        assert!(matches!(second, LaunchOutcome::Refused(_)), "{second:?}");
        assert_eq!((rig.runs(), rig.started()), (1, 1));
    }

    #[test]
    fn a_refused_run_never_starts_a_unit() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x"), follow_up("later", &spec_ref("x"))]);
        let later = reference(K::FollowUp, &id("follow_up", "later"));
        rig.mind.committed(vec![run("op", RunTurn::SelfRun, RunOperator::Operator, std::slice::from_ref(&later))]);
        let outcome = rig.launch(RunTurn::SelfRun, std::slice::from_ref(&later), five());
        assert!(matches!(outcome, LaunchOutcome::Refused(Declined::Admission(MindRefusal::AlreadyClaimed { .. }))), "{outcome:?}");
        for (answer, declined) in [
            (Outcome::Conflict { identities: vec![] }, Declined::Conflict),
            (Outcome::AlreadyAdmitted { receipt_id: "r".into() }, Declined::AlreadyAdmitted),
        ] {
            let mind = Answering { inner: &rig.mind, outcome: answer };
            assert_eq!(rig.launch_through(&mind, RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Refused(declined));
        }
        assert_eq!((rig.runs(), rig.started()), (1, 0));
    }

    #[test]
    fn a_start_failure_withdraws_the_run_it_opened_and_only_that_run() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x"), spec("y")]);
        rig.mind.committed(vec![run("other", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("y")])]);
        rig.launcher.fail.set(true);
        let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five());
        assert_eq!(outcome, LaunchOutcome::Refused(Declined::StartFailed));
        let opened = format!("{INSTANCE}:run:mind-20261009T131500000Z");
        let status = |id: &str| rig.mind.view(K::Run, id).unwrap().status;
        assert!(matches!(status(&opened), PipelineStatus::Resolved { record, .. } if matches!(record.outcome, ResolutionOutcome::Withdrawn { .. })));
        assert_eq!(status(&run_ref("other").id.0), PipelineStatus::InForce);
        // The spec is free again.
        rig.launcher.fail.set(false);
        rig.at(1);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn a_spec_that_is_reported_blocked_or_claimed_is_not_queued_and_admits_nothing() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("done"), spec("blocked"), spec("held"), spec("free")]);
        rig.mind.committed(vec![report("done")]);
        rig.mind.committed(vec![question_in("fork", &spec_ref("blocked"))]);
        rig.mind.committed(vec![run("op", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("held")])]);
        for item in ["done", "blocked", "held"] {
            let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref(item)], five());
            assert_eq!(outcome, LaunchOutcome::NotQueued { item: spec_ref(item) });
        }
        let mixed = rig.launch(RunTurn::SelfRun, &[spec_ref("free"), spec_ref("done")], five());
        assert_eq!(mixed, LaunchOutcome::NotQueued { item: spec_ref("done") }, "one unqueued claim refuses the whole request");
        assert_eq!((rig.runs(), rig.started()), (1, 0));
    }

    #[test]
    fn a_second_live_self_run_of_hers_is_busy_and_other_runs_do_not_count() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x"), spec("y")]);
        // None of these is a live Self run of hers.
        rig.mind.committed(vec![run("op", RunTurn::SelfRun, RunOperator::Operator, &[])]);
        rig.mind.committed(vec![run("gone", RunTurn::SelfRun, RunOperator::Mind, &[])]);
        rig.mind.committed(vec![close(run_ref("gone"), withdrawn())]);
        rig.seed("persona", RunTurn::PersonaTurn, &[]);
        rig.at(1);
        let LaunchOutcome::Launched { run: first } = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()) else {
            panic!("blocked by a run that is not hers")
        };
        let (runs, started) = (rig.runs(), rig.started());
        rig.at(2);
        assert_eq!(rig.launch(RunTurn::SelfRun, &[spec_ref("y")], five()), LaunchOutcome::Busy { run: first.clone() });
        assert_eq!((rig.runs(), rig.started()), (runs, started), "Busy admits and starts nothing");
        // Ending her run frees the slot.
        rig.mind.committed(vec![close(first, recorded())]);
        rig.at(3);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("y")], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn an_alive_run_is_busy_however_old_and_is_not_closed() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        rig.seed("old", RunTurn::SelfRun, &[spec_ref("x")]);
        rig.age(&run_ref("old").id.0, 24 * 3600);
        assert_eq!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Busy { run: run_ref("old") });
        assert!(rig.in_force("old"));
        assert_eq!((rig.runs(), rig.started()), (1, 0));
    }

    #[test]
    fn a_claimless_alive_run_is_busy() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        rig.seed("bare", RunTurn::SelfRun, &[]);
        rig.age(&run_ref("bare").id.0, 3600);
        assert_eq!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Busy { run: run_ref("bare") });
        assert!(rig.in_force("bare"));
    }

    #[test]
    fn a_dead_run_is_closed_past_the_grace_in_the_batch_that_opens_the_next() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        // Her run on x, admitted, and its unit never ran.
        rig.mind.committed(vec![run("dead", RunTurn::SelfRun, RunOperator::Mind, &[spec_ref("x")])]);
        let dead = run_ref("dead").id.0;
        rig.age(&dead, START_GRACE_S - 1);
        assert_eq!(
            rig.launch(RunTurn::SelfRun, &[], five()),
            LaunchOutcome::Busy { run: run_ref("dead") },
            "inside the grace the unit may still be starting"
        );
        assert!(rig.in_force("dead"));
        assert_eq!((rig.runs(), rig.started()), (1, 0));

        rig.age(&dead, START_GRACE_S);
        let LaunchOutcome::Launched { run: next } = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()) else {
            panic!("the dead run's claim stayed held")
        };
        assert!(!rig.in_force("dead"));
        let (closure, closed_in) = the_closure(&rig, "dead", 1);
        assert_eq!(closure.outcome, ResolutionOutcome::Recorded { reason: "unit not active".into() });
        assert_eq!(closure.subject, run_ref("dead"));
        assert_eq!(
            closed_in,
            rig.mind.view(K::Run, &next.id.0).unwrap().admission.receipt_id,
            "one admission closes the dead run and opens the next"
        );
        assert_eq!(rig.started(), 1);
    }

    #[test]
    fn a_dead_runs_closure_takes_the_next_sequence_of_its_subject() {
        let rig = Rig::new();
        rig.mind.committed(vec![run("dead", RunTurn::SelfRun, RunOperator::Mind, &[])]);
        rig.mind.committed(vec![close(run_ref("dead"), recorded())]);
        // Withdrawing that closure puts the run back in force.
        let first = reference(K::Resolution, &format!("{INSTANCE}:resolution:run.dead.n1"));
        rig.mind.committed(vec![close(first, withdrawn())]);
        assert!(rig.in_force("dead"));
        rig.age(&run_ref("dead").id.0, 600);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(!rig.in_force("dead"));
        assert_eq!(the_closure(&rig, "dead", 2).0.sequence, 2);
    }

    #[test]
    fn only_a_dead_holder_of_the_requests_turn_is_closed() {
        let rig = Rig::new();
        rig.mind.committed(vec![run("op", RunTurn::SelfRun, RunOperator::Operator, &[])]);
        rig.mind.committed(vec![run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
        rig.mind.committed(vec![run("dead", RunTurn::SelfRun, RunOperator::Mind, &[])]);
        rig.age(&run_ref("dead").id.0, 3600);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(!rig.in_force("dead"));
        assert!(rig.in_force("op") && rig.in_force("persona"), "an Operator run and a run of the other turn are never closed by this launch");
    }

    #[test]
    fn a_run_whose_admit_answer_was_lost_is_closed_by_the_next_launch_of_its_turn() {
        struct LosesAnswer<'a>(&'a TestMind);
        impl MindPort for LosesAnswer<'_> {
            fn instance(&self) -> &Slug {
                self.0.instance()
            }

            fn query(&self, selection: &Selection) -> Result<huginn_mind::PipelineSelectionPage> {
                self.0.query(selection)
            }

            fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<Outcome> {
                self.0.admit(agent, session, documents)?;
                Err(anyhow!("the answer was lost"))
            }
        }
        let rig = Rig::new();
        assert!(rig.try_launch_through(&LosesAnswer(&rig.mind), RunTurn::SelfRun, &[], five()).is_err());
        assert_eq!((rig.runs(), rig.started()), (1, 0), "committed, and no unit was started");
        let lost = rig.mind.ids(K::Run).remove(0);
        rig.age(&lost, START_GRACE_S);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Launched { .. }));
        assert_eq!((rig.runs(), rig.started()), (2, 1));
        let lost_label = lost.rsplit(':').next().unwrap().to_string();
        assert!(!rig.in_force(&lost_label));
    }

    #[test]
    fn a_port_that_admits_a_rival_live_run_between_step_0_and_the_admission_is_busy_and_starts_nothing() {
        struct Racing<'a> {
            inner: &'a TestMind,
            raced: Cell<bool>,
        }
        impl MindPort for Racing<'_> {
            fn instance(&self) -> &Slug {
                self.inner.instance()
            }

            fn query(&self, selection: &Selection) -> Result<huginn_mind::PipelineSelectionPage> {
                self.inner.query(selection)
            }

            fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<Outcome> {
                if !self.raced.replace(true) {
                    self.inner.committed(vec![run("rival", RunTurn::SelfRun, RunOperator::Mind, &[])]);
                }
                self.inner.admit(agent, session, documents)
            }
        }
        let rig = Rig::new();
        let racing = Racing { inner: &rig.mind, raced: Cell::new(false) };
        assert_eq!(rig.launch_through(&racing, RunTurn::SelfRun, &[], five()), LaunchOutcome::Busy { run: run_ref("rival") });
        assert_eq!((rig.runs(), rig.started()), (1, 0));
    }

    #[test]
    fn an_already_live_holder_the_mind_does_not_show_as_hers_is_refused_not_trusted() {
        struct Forged<'a>(&'a TestMind, &'a str);
        impl MindPort for Forged<'_> {
            fn instance(&self) -> &Slug {
                self.0.instance()
            }

            fn query(&self, selection: &Selection) -> Result<huginn_mind::PipelineSelectionPage> {
                self.0.query(selection)
            }

            fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<Outcome> {
                let run = huginn_mind::RunId::try_from(self.1.to_string()).unwrap();
                Ok(Outcome::Refused(MindRefusal::AlreadyLive { run }))
            }
        }
        let rig = Rig::new();
        // A Persona turn of hers is live, and a Self run of hers is not: a real Self holder named by admission is the race test's Busy.
        rig.seed("persona", RunTurn::PersonaTurn, &[]);
        for foreign in
            ["other:run:mind-20261010T010203004Z", "eureka-body:run:mind-20261010T010203004Z", "yggdrasil:run:nobody", "yggdrasil:run:persona"]
        {
            let outcome = rig.launch_through(&Forged(&rig.mind, foreign), RunTurn::SelfRun, &[], five());
            assert!(matches!(outcome, LaunchOutcome::Refused(Declined::Admission(MindRefusal::AlreadyLive { .. }))), "{foreign}: {outcome:?}");
        }
        assert_eq!(rig.started(), 0);
    }

    #[test]
    fn a_run_whose_unit_died_is_closed_by_the_next_launch_of_its_turn() {
        let rig = Rig::new();
        let LaunchOutcome::Launched { run: first } = rig.launch(RunTurn::SelfRun, &[], five()) else { panic!("not launched") };
        let first_label = first.id.0.rsplit(':').next().unwrap().to_string();
        rig.age(&first.id.0, 3600);
        assert_eq!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Busy { run: first.clone() }, "its unit is still running");
        rig.launcher.kill(RunTurn::SelfRun);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(!rig.in_force(&first_label));
        assert_eq!(rig.started(), 2);
    }

    #[test]
    fn a_tripped_item_raises_one_question_launches_nothing_and_a_ruling_reopens_it() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        history(&rig.mind, &spec_ref("x"), &[false, false, false]);
        let before = rig.runs();
        let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five());
        let question = id("question", "breaker-mind-20261009T131500000Z");
        assert_eq!(outcome, LaunchOutcome::Tripped { item: spec_ref("x"), question: reference(K::Question, &question) });
        assert_eq!((rig.runs(), rig.started()), (before, 0));
        assert_eq!(rig.mind.ids(K::Question), vec![question.clone()]);
        let view = rig.mind.view(K::Question, &question).unwrap();
        let PipelineDocument::Question(asked) = &view.document else { panic!("not a question") };
        assert_eq!((asked.raised_in.clone(), asked.campaign.0.as_str()), (Some(spec_ref("x")), CAMPAIGN));
        assert_eq!(view.admission.provenance.session.0, "mind-20261009T131500000Z");
        // Open, the question blocks the item.
        assert_eq!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::NotQueued { item: spec_ref("x") });
        // The ruling that answers it opens the item, and the count starts afresh.
        rig.mind.committed(vec![ruling("reopen-x", Some((&question, "reopen")))]);
        rig.at(1);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn launching_a_tripped_item_with_another_queued_trips_and_launches_nothing() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x"), spec("y")]);
        history_as(&rig.mind, &spec_ref("x"), "x", &[false, false, false]);
        let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five());
        assert!(matches!(outcome, LaunchOutcome::Tripped { .. }), "{outcome:?}");
        assert_eq!(rig.started(), 0);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("y")], five()), LaunchOutcome::Launched { .. }), "the other item still launches");
    }

    #[test]
    fn a_second_trip_raises_a_second_question_with_its_own_key() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        history(&rig.mind, &spec_ref("x"), &[false, false, false]);
        let LaunchOutcome::Tripped { question: first, .. } = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()) else { panic!("no first trip") };
        rig.mind.committed(vec![ruling("reopen", Some((&first.id.0, "reopen")))]);
        history_as(&rig.mind, &spec_ref("x"), "g", &[false, false, false]);
        rig.at(1);
        let LaunchOutcome::Tripped { question: second, .. } = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()) else {
            panic!("no second trip")
        };
        assert_ne!(first, second);
        assert_eq!(rig.mind.ids(K::Question).len(), 2);
        assert_eq!(rig.started(), 0);
    }

    #[test]
    fn nine_empty_runs_of_fifty_before_the_answer_do_not_trip_after_it() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        let item = spec_ref("x");
        history(&rig.mind, &item, &[false, false, true, false, false, true, false, false, true, false, false, true, false]);
        rig.mind.committed(vec![question_in("fork", &item)]);
        rig.mind.committed(vec![ruling("answer", Some((&id("question", "fork"), "A")))]);
        history_as(&rig.mind, &item, "g", &[false]);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[item], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn a_dead_run_is_an_ended_empty_run_for_the_breaker_and_its_claim_is_free() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        history(&rig.mind, &spec_ref("x"), &[false, false]);
        rig.mind.committed(vec![run("dead", RunTurn::SelfRun, RunOperator::Mind, &[spec_ref("x")])]);
        rig.age(&run_ref("dead").id.0, 600);
        let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five());
        assert!(matches!(outcome, LaunchOutcome::Tripped { .. }), "{outcome:?}");
        assert!(!rig.in_force("dead"), "the trip's batch closes the dead run too");
        assert_eq!((rig.runs(), rig.started()), (3, 0));
    }

    /// Every name that differs from `name` and is shaped to pass a compare weaker
    /// than equality. The set is generated from the real name, not listed: every
    /// single-character substitution (over ASCII and some wide characters), deletion
    /// and insertion; every transposition; every permutation; rotations; every
    /// prefix and suffix; case flips; whitespace and NUL padding at either end;
    /// repetitions past any plausible length bound; and seeded random names of the
    /// same length and of other lengths. A compare that agrees with equality on all
    /// of them, and so passes the gate test, is equality for every name a caller
    /// can plausibly build.
    fn near_misses(name: &str) -> Vec<String> {
        let chars: Vec<char> = name.chars().collect();
        let wide = ['é', 'ÿ', '\u{ffff}', '😀'];
        let alphabet: Vec<char> = (0u8..128).map(char::from).chain(wide).collect();
        let text = |parts: &[char]| parts.iter().collect::<String>();
        let mut out: BTreeSet<String> = BTreeSet::new();
        for i in 0..chars.len() {
            for &c in &alphabet {
                let mut changed = chars.clone();
                changed[i] = c;
                out.insert(text(&changed));
            }
            let mut deleted = chars.clone();
            deleted.remove(i);
            out.insert(text(&deleted));
            for j in i + 1..chars.len() {
                let mut swapped = chars.clone();
                swapped.swap(i, j);
                out.insert(text(&swapped));
            }
            out.insert(text(&chars[..i]));
            out.insert(text(&chars[i + 1..]));
            out.insert(text(&[&chars[i..], &chars[..i]].concat()));
            let mut flipped = chars.clone();
            flipped[i] = if chars[i].is_uppercase() { chars[i].to_ascii_lowercase() } else { chars[i].to_ascii_uppercase() };
            out.insert(text(&flipped));
        }
        for i in 0..=chars.len() {
            for &c in &alphabet {
                let mut grown = chars.clone();
                grown.insert(i, c);
                out.insert(text(&grown));
            }
        }
        for pad in ['\0', ' ', '\n', '\t', '\u{a0}'] {
            for count in 1..=4 {
                out.insert(format!("{name}{}", pad.to_string().repeat(count)));
                out.insert(format!("{}{name}", pad.to_string().repeat(count)));
            }
            for width in [32, 64, 255] {
                out.insert(format!("{name}{}", pad.to_string().repeat(width - chars.len())));
            }
        }
        for copies in [2, 3, 8, 40] {
            out.insert(name.repeat(copies));
        }
        out.insert(name.to_uppercase());
        out.insert(name.to_lowercase());
        // Every permutation (Heap's algorithm).
        let mut order = chars.clone();
        let mut counters = vec![0usize; order.len()];
        out.insert(text(&order));
        let mut at = 0;
        while at < order.len() {
            if counters[at] < at {
                order.swap(if at % 2 == 0 { 0 } else { counters[at] }, at);
                out.insert(text(&order));
                counters[at] += 1;
                at = 0;
            } else {
                counters[at] = 0;
                at += 1;
            }
        }
        // Seeded random names: same length over the name's own alphabet, over all of
        // ASCII, and of any length up to 300 over ASCII.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..4096 {
            out.insert((0..chars.len()).map(|_| chars[(next() % chars.len() as u64) as usize]).collect());
            out.insert((0..chars.len()).map(|_| char::from((next() % 128) as u8)).collect());
        }
        for _ in 0..1024 {
            let length = (next() % 301) as usize;
            out.insert((0..length).map(|_| char::from((next() % 128) as u8)).collect());
        }
        out.remove(name);
        out.into_iter().collect()
    }

    #[test]
    fn a_grant_read_for_any_other_instance_opens_nothing_and_starts_nothing() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        let forged = near_misses(INSTANCE);
        // The generator itself: wide enough to carry each family, and the mind's own name is not in it.
        assert!(forged.len() > 100_000, "{}", forged.len());
        for member in [format!("{}k", &INSTANCE[..INSTANCE.len() - 1]), INSTANCE.to_uppercase(), format!("{INSTANCE}\0"), INSTANCE.chars().rev().collect::<String>()] {
            assert!(forged.contains(&member), "{member:?}");
        }
        assert!(!forged.iter().any(|name| name == INSTANCE));
        for name in &forged {
            for turn in [RunTurn::SelfRun, RunTurn::PersonaTurn] {
                let claims = if turn == RunTurn::SelfRun { vec![spec_ref("x")] } else { vec![] };
                let ports = Ports { mind: &rig.mind, launcher: &rig.launcher, clock: &rig.clock, host: "yggdrasil-host" };
                let grant = Grant::for_test(Slug(name.clone()), StdDuration::from_secs(60), five());
                let outcome = open_and_launch(&ports, LaunchRequest { turn, claims, agent: "agent-x".into(), grant }).unwrap();
                assert_eq!(outcome, LaunchOutcome::Refused(Declined::GrantForOtherInstance), "{name:?} {turn:?}");
            }
        }
        assert_eq!((rig.runs(), rig.started()), (0, 0));
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Launched { .. }), "the mind's own Grant launches");
    }

    #[test]
    fn a_persona_turn_with_any_claims_at_all_is_refused_and_opens_nothing() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x"), spec("blocked")]);
        rig.mind.committed(vec![question_in("fork", &spec_ref("blocked"))]);
        let kinds = PipelineKind::ALL;
        let refused = |claims: &[PipelineRef]| {
            let outcome = rig.launch(RunTurn::PersonaTurn, claims, five());
            assert_eq!(outcome, LaunchOutcome::Refused(Declined::PersonaRunTakesNoClaims), "{} claims", claims.len());
        };
        // Claim sets of every size from one to well past the number of kinds, built four ways:
        // one queued spec repeated (duplicates), queued and blocked specs alternating, every kind
        // in turn over one id, and every kind in turn over distinct ids.
        for size in 1..=300 {
            refused(&(0..size).map(|_| spec_ref("x")).collect::<Vec<_>>());
            refused(&(0..size).map(|i| spec_ref(if i % 2 == 0 { "x" } else { "blocked" })).collect::<Vec<_>>());
            refused(&(0..size).map(|i| reference(kinds[i % kinds.len()], "no-such-document")).collect::<Vec<_>>());
            refused(&(0..size).map(|i| reference(kinds[i % kinds.len()], &format!("no-such-document-{i}"))).collect::<Vec<_>>());
        }
        // And every kind alone, repeated, whether or not the document exists.
        for kind in kinds {
            for size in 1..=40 {
                refused(&(0..size).map(|_| reference(*kind, "no-such-document")).collect::<Vec<_>>());
            }
        }
        assert_eq!((rig.runs(), rig.started()), (0, 0));
        assert!(matches!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Launched { .. }));
    }

    #[test]
    fn a_persona_launch_closes_a_dead_persona_holder_past_the_grace() {
        let rig = Rig::new();
        rig.mind.committed(vec![run("pdead", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
        rig.age(&run_ref("pdead").id.0, START_GRACE_S - 1);
        assert_eq!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Busy { run: run_ref("pdead") }, "inside the grace");
        rig.age(&run_ref("pdead").id.0, START_GRACE_S);
        assert!(matches!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(!rig.in_force("pdead"));
        assert_eq!(the_closure(&rig, "pdead", 1).0.outcome, ResolutionOutcome::Recorded { reason: "unit not active".into() });
        assert_eq!((rig.runs(), rig.started()), (2, 1));
    }

    #[test]
    fn a_live_self_run_does_not_make_a_persona_launch_busy() {
        let rig = Rig::new();
        rig.seed("alive-self", RunTurn::SelfRun, &[]);
        assert!(matches!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(rig.in_force("alive-self"));
    }

    #[test]
    fn a_dead_self_run_is_not_closed_by_a_persona_launch() {
        let rig = Rig::new();
        rig.mind.committed(vec![run("dead-self", RunTurn::SelfRun, RunOperator::Mind, &[])]);
        rig.age(&run_ref("dead-self").id.0, 3600);
        assert!(matches!(rig.launch(RunTurn::PersonaTurn, &[], five()), LaunchOutcome::Launched { .. }));
        assert!(rig.in_force("dead-self"), "a Persona launch closes only Persona holders");
    }

    struct Spy {
        calls: RefCell<Vec<Vec<String>>>,
        success: Cell<bool>,
        stdout: RefCell<String>,
    }

    impl Systemctl for &'static Spy {
        fn run(&self, args: &[&str]) -> Result<SystemctlReply> {
            self.calls.borrow_mut().push(args.iter().map(|arg| arg.to_string()).collect());
            Ok(SystemctlReply { success: self.success.get(), stdout: self.stdout.borrow().clone() })
        }
    }

    fn spied() -> (SystemdLauncher, &'static Spy) {
        let spy: &'static Spy = Box::leak(Box::new(Spy { calls: RefCell::default(), success: Cell::new(true), stdout: RefCell::default() }));
        (SystemdLauncher::over(Box::new(spy)), spy)
    }

    fn eureka() -> Slug {
        Slug("eureka".into())
    }

    #[test]
    fn units_are_templated_by_instance_and_turn() {
        let (launcher, spy) = spied();
        assert_eq!(unit(&eureka(), RunTurn::PersonaTurn).unwrap(), "mind-persona@eureka.service");
        assert_eq!(unit(&eureka(), RunTurn::SelfRun).unwrap(), "mind-self@eureka.service");
        launcher.start(&eureka(), RunTurn::SelfRun).unwrap();
        launcher.start(&Slug("eureka.test".into()), RunTurn::PersonaTurn).unwrap();
        assert_eq!(
            *spy.calls.borrow(),
            vec![vec!["start", "--no-block", "mind-self@eureka.service"], vec!["start", "--no-block", "mind-persona@eureka.test.service"],]
        );
    }

    #[test]
    fn a_unit_is_alive_while_active_activating_or_reloading() {
        let (launcher, spy) = spied();
        for (said, alive) in
            [("active\n", true), ("activating\n", true), ("reloading\n", true), ("inactive\n", false), ("failed\n", false), ("", false)]
        {
            *spy.stdout.borrow_mut() = said.to_string();
            assert_eq!(launcher.alive(&eureka(), RunTurn::SelfRun), alive, "{said:?}");
        }
        assert_eq!(spy.calls.borrow()[0], vec!["is-active", "mind-self@eureka.service"]);
    }

    #[test]
    fn a_failed_start_names_no_instance() {
        let (launcher, spy) = spied();
        spy.success.set(false);
        let error = launcher.start(&Slug("canary-instance-7f3a91".into()), RunTurn::SelfRun).unwrap_err();
        let shown = format!("{error:#}");
        assert!(!shown.contains("canary"), "{shown}");
    }

    #[test]
    fn hostile_instances_make_start_fail_and_alive_false_with_no_systemctl_call() {
        let (launcher, spy) = spied();
        *spy.stdout.borrow_mut() = "active
"
        .into();
        for hostile in [
            "",
            ".",
            "..",
            "a/b",
            "a@b",
            "a:b",
            "a b",
            "a
b",
            "eureka/../x
",
            "x
--now",
            "canary/9",
        ] {
            let instance = Slug(hostile.into());
            let error = launcher.start(&instance, RunTurn::SelfRun).unwrap_err();
            assert!(!format!("{error:#}").contains("canary"), "{error:#}");
            assert!(!launcher.alive(&instance, RunTurn::PersonaTurn), "instance {hostile:?}");
        }
        assert!(spy.calls.borrow().is_empty(), "no hostile name reached systemctl: {:?}", spy.calls.borrow());
    }

    #[test]
    fn a_unit_is_named_by_instance_and_turn_alone() {
        let (launcher, spy) = spied();
        launcher.start(&eureka(), RunTurn::SelfRun).unwrap();
        launcher.start(&eureka(), RunTurn::SelfRun).unwrap();
        let calls = spy.calls.borrow();
        assert_eq!(calls[0], calls[1], "two runs of one turn share a unit name");
        assert_eq!(calls[0][2], "mind-self@eureka.service");
    }
}
