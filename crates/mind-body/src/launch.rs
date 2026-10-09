//! The one way her organs open a run and start its unit.
//!
//! `open_and_launch` is the whole path: Busy, queue membership and the
//! repetition breaker are checked in that order, the run is admitted into the
//! mind, and only after it commits is the unit started. The run is the grant
//! (target invariant run-is-the-grant), so there is no pressure file, lease or
//! breaker state beside it, and a unit never starts without a committed run.
//! The primitive owns no file and stops no unit: stopping her units is the
//! operator's brake hold, which is another cut's.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use eureka_pipeline::{
    Date, Label, PipelineDocument, PipelineKind, PipelineQuestion, PipelineRef, PipelineResolution, PipelineRun, QuestionOption,
    ResolutionOutcome, RunOperator, RunTurn, Slug,
};
use huginn_mind::{MindRefusal, PipelineAdmissionOutcome};
use rust_decimal::Decimal;

use crate::queue::{Breaker, MindPort, any_of, breaker, headers, in_force, of_kind, queue};

/// Starts the unit that carries a run, and says whether it is still running.
/// Nothing here stops a unit.
pub trait Launcher {
    fn start(&self, turn: RunTurn, label: &Label) -> Result<()>;
    fn alive(&self, turn: RunTurn, label: &Label) -> bool;
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
/// `mind-persona@<instance>:<label>.service` and
/// `mind-self@<instance>:<label>.service`.
pub struct SystemdLauncher {
    instance: Slug,
    systemctl: Box<dyn Systemctl>,
}

impl SystemdLauncher {
    pub fn new(instance: Slug) -> Self {
        Self::over(instance, Box::new(RealSystemctl))
    }

    pub(crate) fn over(instance: Slug, systemctl: Box<dyn Systemctl>) -> Self {
        Self { instance, systemctl }
    }

    /// The unit of one run: the template by turn, then the instance and the
    /// label. Start and alive both name it here.
    pub(crate) fn unit(&self, turn: RunTurn, label: &Label) -> String {
        let template = match turn {
            RunTurn::PersonaTurn => "mind-persona",
            RunTurn::SelfRun => "mind-self",
        };
        format!("{template}@{}:{}.service", self.instance.0, label.0)
    }
}

impl Launcher for SystemdLauncher {
    fn start(&self, turn: RunTurn, label: &Label) -> Result<()> {
        let reply = self.systemctl.run(&["start", "--no-block", &self.unit(turn, label)])?;
        if reply.success { Ok(()) } else { Err(anyhow!("systemctl did not start the unit")) }
    }

    fn alive(&self, turn: RunTurn, label: &Label) -> bool {
        self.systemctl
            .run(&["is-active", &self.unit(turn, label)])
            .is_ok_and(|reply| matches!(reply.stdout.trim(), "active" | "activating" | "reloading"))
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
    /// The most one run may spend, from the dial's `read_effective`.
    pub run_cap_usd: Decimal,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchOutcome {
    Launched { run: PipelineRef },
    NotQueued { item: PipelineRef },
    Tripped { item: PipelineRef, question: PipelineRef },
    Busy { run: PipelineRef },
    Refused(Declined),
}

fn opened_run(instance: &Slug, label: &Label) -> PipelineRef {
    PipelineRef { kind: PipelineKind::Run, id: format!("{}:run:{}", instance.0, label.0).as_str().into() }
}

fn date(now: DateTime<Utc>) -> Date {
    Date(now.format("%Y-%m-%d").to_string())
}

/// The root of a full pipeline id: the campaign slug of a campaign's document.
fn root_of(item: &PipelineRef) -> &str {
    item.id.0.split(':').next().unwrap_or_default()
}

pub fn open_and_launch(ports: &Ports, request: LaunchRequest) -> Result<LaunchOutcome> {
    let LaunchRequest { turn, claims, agent, run_cap_usd } = request;
    let instance = ports.mind.instance().clone();
    let now = ports.clock.now();
    let label = run_label(now);

    if turn == RunTurn::SelfRun {
        let live = in_force(any_of(any_of(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]), "turn", &["SelfRun"]), "operated_by", &["Mind"]));
        if let Some(run) = headers(ports.mind, &live)?.into_iter().next() {
            return Ok(LaunchOutcome::Busy { run: run.id });
        }
    }
    let open = queue(ports.mind, &instance)?;
    if let Some(item) = claims.iter().find(|claim| claim.kind == PipelineKind::CutSpec && !open.contains(claim)) {
        return Ok(LaunchOutcome::NotQueued { item: item.clone() });
    }
    for item in &claims {
        if breaker(ports.mind, item)? == Breaker::Trip {
            let question = breaker_question(item, now);
            let id = PipelineRef { kind: PipelineKind::Question, id: format!("{}:question:{}", root_of(item), breaker_label(item).0).as_str().into() };
            return Ok(match committed(ports.mind.admit(&agent, &label.0, vec![PipelineDocument::Question(question)])?) {
                Ok(()) => LaunchOutcome::Tripped { item: item.clone(), question: id },
                Err(declined) => LaunchOutcome::Refused(declined),
            });
        }
    }

    let mut campaigns: Vec<Slug> = Vec::new();
    for claim in &claims {
        let root = Slug(root_of(claim).to_string());
        if !campaigns.contains(&root) {
            campaigns.push(root);
        }
    }
    let run = PipelineRun {
        instance: instance.clone(),
        label: label.clone(),
        operated_by: RunOperator::Mind,
        turn,
        host: ports.host.into(),
        started_on: date(now),
        budget_usd: run_cap_usd.normalize().to_string().as_str().into(),
        claims,
        campaigns,
    };
    if let Err(declined) = committed(ports.mind.admit(&agent, &label.0, vec![PipelineDocument::Run(run)])?) {
        return Ok(LaunchOutcome::Refused(declined));
    }
    let run = opened_run(&instance, &label);
    if ports.launcher.start(turn, &label).is_err() {
        let withdrawal = PipelineResolution {
            subject: run,
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
    Ok(LaunchOutcome::Launched { run })
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

/// `breaker-<item local>`, with the local's dots (a label has none) as dashes.
fn breaker_label(item: &PipelineRef) -> Label {
    let local = item.id.0.splitn(3, ':').nth(2).unwrap_or_default();
    Label(format!("breaker-{}", local.replace('.', "-")))
}

/// The question a tripped item raises, in the item's own campaign.
fn breaker_question(item: &PipelineRef, now: DateTime<Utc>) -> PipelineQuestion {
    PipelineQuestion {
        campaign: Slug(root_of(item).to_string()),
        label: breaker_label(item),
        title: "Repeated runs on one item admitted nothing".into(),
        question: "The last runs on this item admitted nothing but themselves, so it launches no more. A ruling or any other new document reopens it.".into(),
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

    use eureka_pipeline::PipelineKind as K;
    use huginn_mind::PipelineAdmissionOutcome as Outcome;

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
            let ports = Ports { mind, launcher: &self.launcher, clock: &self.clock, host: "yggdrasil-host" };
            open_and_launch(&ports, LaunchRequest { turn, claims: claims.to_vec(), agent: "agent-x".into(), run_cap_usd: cap }).unwrap()
        }

        fn at(&self, milliseconds: i64) {
            self.clock.0.set(at(milliseconds));
        }

        fn started(&self) -> usize {
            self.launcher.started.borrow().len()
        }

        fn runs(&self) -> usize {
            self.mind.ids(K::Run).len()
        }
    }

    fn five() -> Decimal {
        Decimal::new(5, 0)
    }

    fn the_run(view: &huginn_mind::PipelineDocumentView) -> &PipelineRun {
        let PipelineDocument::Run(run) = &view.document else { panic!("not a run") };
        run
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
        assert_eq!(*rig.launcher.started.borrow(), vec![(RunTurn::SelfRun, label("mind-20261009T131500000Z"))]);
    }

    #[test]
    fn the_runs_budget_is_the_cap_passed_in() {
        let rig = Rig::new();
        for (n, (cap, want)) in [(Decimal::new(5, 0), "5"), (Decimal::new(750, 2), "7.5"), (Decimal::new(125, 1), "12.5")].into_iter().enumerate() {
            rig.at(n as i64);
            let LaunchOutcome::Launched { run } = rig.launch(RunTurn::PersonaTurn, &[], cap) else { panic!("not launched") };
            let view = rig.mind.view(K::Run, &run.id.0).unwrap();
            assert_eq!(the_run(&view).budget_usd.0, want);
            assert_eq!(the_run(&view).turn, RunTurn::PersonaTurn);
            assert!(the_run(&view).claims.is_empty() && the_run(&view).campaigns.is_empty());
        }
        assert_eq!(rig.started(), 3, "a Persona turn is never Busy");
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
        let second = rig.launch(RunTurn::PersonaTurn, &[], Decimal::new(6, 0));
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
        assert!(
            matches!(status(&opened), huginn_mind::PipelineStatus::Resolved { record, .. } if matches!(record.outcome, ResolutionOutcome::Withdrawn { .. }))
        );
        assert_eq!(status(&run_ref("other").id.0), huginn_mind::PipelineStatus::InForce);
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
        rig.mind.committed(vec![run("persona", RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
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
    fn a_tripped_item_raises_one_question_launches_nothing_and_a_ruling_reopens_it() {
        let rig = Rig::new();
        rig.mind.committed(vec![spec("x")]);
        history(&rig.mind, &spec_ref("x"), &[false, false, false]);
        let before = rig.runs();
        let outcome = rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five());
        let question = id("question", "breaker-cut-x-r1");
        assert_eq!(outcome, LaunchOutcome::Tripped { item: spec_ref("x"), question: reference(K::Question, &question) });
        assert_eq!((rig.runs(), rig.started()), (before, 0));
        assert_eq!(rig.mind.ids(K::Question), vec![question.clone()]);
        let view = rig.mind.view(K::Question, &question).unwrap();
        let PipelineDocument::Question(asked) = &view.document else { panic!("not a question") };
        assert_eq!((asked.raised_in.clone(), asked.campaign.0.as_str()), (Some(spec_ref("x")), CAMPAIGN));
        assert_eq!(view.admission.provenance.session.0, "mind-20261009T131500000Z");
        // Open, the question blocks the item.
        assert_eq!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::NotQueued { item: spec_ref("x") });
        // The ruling that answers it is a document after the last run.
        rig.mind.committed(vec![ruling("reopen-x", Some((&question, "reopen")))]);
        rig.at(1);
        assert!(matches!(rig.launch(RunTurn::SelfRun, &[spec_ref("x")], five()), LaunchOutcome::Launched { .. }));
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

    fn spied(instance: &str) -> (SystemdLauncher, &'static Spy) {
        let spy: &'static Spy = Box::leak(Box::new(Spy { calls: RefCell::default(), success: Cell::new(true), stdout: RefCell::default() }));
        (SystemdLauncher::over(Slug(instance.into()), Box::new(spy)), spy)
    }

    #[test]
    fn units_are_templated_by_instance_and_turn() {
        let (launcher, spy) = spied("eureka");
        let l = label("mind-20261009T131500123Z");
        assert_eq!(launcher.unit(RunTurn::PersonaTurn, &l), "mind-persona@eureka:mind-20261009T131500123Z.service");
        assert_eq!(launcher.unit(RunTurn::SelfRun, &l), "mind-self@eureka:mind-20261009T131500123Z.service");
        launcher.start(RunTurn::SelfRun, &l).unwrap();
        launcher.start(RunTurn::PersonaTurn, &l).unwrap();
        assert_eq!(
            *spy.calls.borrow(),
            vec![
                vec!["start", "--no-block", "mind-self@eureka:mind-20261009T131500123Z.service"],
                vec!["start", "--no-block", "mind-persona@eureka:mind-20261009T131500123Z.service"],
            ]
        );
    }

    #[test]
    fn a_unit_is_alive_while_active_activating_or_reloading() {
        let (launcher, spy) = spied("eureka");
        let l = label("a");
        for (said, alive) in [("active\n", true), ("activating\n", true), ("reloading\n", true), ("inactive\n", false), ("failed\n", false), ("", false)] {
            *spy.stdout.borrow_mut() = said.to_string();
            assert_eq!(launcher.alive(RunTurn::SelfRun, &l), alive, "{said:?}");
        }
        assert_eq!(spy.calls.borrow()[0], vec!["is-active", "mind-self@eureka:a.service"]);
    }

    #[test]
    fn a_failed_start_names_neither_instance_nor_label() {
        let (launcher, spy) = spied("canary-instance-7f3a91");
        spy.success.set(false);
        let error = launcher.start(RunTurn::SelfRun, &label("canary-label-2c8d04")).unwrap_err();
        let shown = format!("{error:#}");
        assert!(!shown.contains("canary"), "{shown}");
    }
}
