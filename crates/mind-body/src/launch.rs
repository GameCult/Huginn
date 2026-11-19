//! Her units under systemd, and the reads of the runs a Self run holds.
//!
//! `Launcher` starts a turn's unit and says whether it lives; `SystemdLauncher`
//! is it over `systemctl`, and its `stop` is the one stop a brake hold calls.
//! `holders` reads the in-force runs of hers of one slot, and `closure` builds
//! the closing resolution of a run that ended without its own flush; `self_run`
//! uses both. Nothing here opens a run: Self-run launching is parked at tag
//! `parked/self-run-launch` (ruling `eureka-body:ruling:persona-only-heartbeat`).

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{Date, PipelineDocument, PipelineKind, PipelineRef, PipelineResolution, ResolutionOutcome, RunOperator, RunSlot, RunTurn, Slug};
use huginn_mind::{PipelineDocumentSummary, PipelineFacts};

use crate::queue::{MindPort, any_of, citing, headers, in_force, of_kind};

/// Starts the unit that carries a run, and says whether it is still running.
/// Nothing here stops a unit.
pub trait Launcher {
    /// Units are named by instance and turn alone (ruling mind-unit-names), so
    /// liveness is keyed the same way.
    fn start(&self, instance: &Slug, turn: RunTurn) -> Result<()>;
    fn alive(&self, instance: &Slug, turn: RunTurn) -> bool;
}

/// The time her organs read: dates and receipts.
pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// What `systemctl` answered: whether it succeeded, its exit code (none when a
/// signal ended it), and what it printed.
pub struct SystemctlReply {
    pub success: bool,
    pub code: Option<i32>,
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
        Ok(SystemctlReply {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }
}

/// Her units under systemd, templated by instance:
/// `mind-persona@<instance>.service` and
/// `mind-self@<instance>.service` (ruling mind-unit-names: one live run per
/// instance and turn, so the run label names no unit). The launcher holds no instance: the
/// caller names it on every call.
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

/// The unit template of one slot, named only through `unit()`, which start,
/// alive and stop all call.
fn template(turn: RunTurn) -> &'static str {
    match turn.slot() {
        RunSlot::Persona => "mind-persona",
        RunSlot::SelfRun => "mind-self",
    }
}

/// The stream's unit template: not a run slot, so only a brake hold names it. The organ is parked
/// (tag `parked/divergence-stream`); a hold still stops its unit wherever a host has it installed.
const STREAM_TEMPLATE: &str = "mind-stream";

/// A template's unit for one instance, the instance held to the leaf's grammar
/// first so it cannot escape the name. The error names nothing of the value.
fn named(template: &str, instance: &Slug) -> Result<String> {
    instance.validate_slug().map_err(|_| anyhow!("the instance is not a unit name"))?;
    Ok(format!("{template}@{}.service", instance.0))
}

/// The unit of one turn: the template by turn, then the instance. Start and
/// alive both name the unit here.
pub(crate) fn unit(instance: &Slug, turn: RunTurn) -> Result<String> {
    named(template(turn), instance)
}

/// What one stop reaches: the three units of one Mind, or every Mind's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopScope<'a> {
    Mind(&'a Slug),
    Swarm,
}

/// The arguments of the one synchronous stop. A Mind's: the exact unit `unit()`
/// names for each turn of this instance, the same names start and alive use,
/// and its stream's, with no glob (a prefix glob also matches another instance, `eureka*` matches
/// `eureka-two`). The swarm's: each template's glob over the loaded units,
/// which is exactly every running Mind and exits 0 when none is loaded. No
/// `--no-block`: `systemctl stop` returns when the units are down. The
/// instance is validated by `named()`.
pub(crate) fn stop_args(scope: StopScope<'_>) -> Result<Vec<String>> {
    let mut args = vec!["stop".to_string(), "--".to_string()];
    args.extend(held_templates().into_iter().map(|template| match scope {
        StopScope::Mind(instance) => named(template, instance),
        StopScope::Swarm => Ok(format!("{template}@*.service")),
    }).collect::<Result<Vec<_>>>()?);
    Ok(args)
}

/// The templates a hold stops: both run slots and the stream.
fn held_templates() -> [&'static str; 3] {
    [template(RunTurn::SelfRun), template(RunTurn::PersonaTurn), STREAM_TEMPLATE]
}

impl SystemdLauncher {
    /// Stops the units of one Mind, or of every Mind, and waits for them. An
    /// inherent method, not on `Launcher`: only a brake hold calls it, through
    /// `mind-control`. A unit that is not installed is systemctl's exit 5, and
    /// the others still stop; when a Mind's stop fails the error names each
    /// unit's state, because systemd may have stopped one and not the other,
    /// and a failed swarm stop names systemctl's exit status only.
    pub fn stop(&self, scope: StopScope<'_>) -> Result<()> {
        let args = stop_args(scope)?;
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let reply = self.systemctl.run(&args)?;
        if reply.success {
            return Ok(());
        }
        let states = match scope {
            StopScope::Mind(instance) => {
                let states = held_templates()
                    .into_iter()
                    .map(|template| {
                        let unit = named(template, instance)?;
                        let state = self.unit_state(&unit);
                        Ok(format!("{unit} is {state}"))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                format!("; {states}")
            }
            StopScope::Swarm => String::new(),
        };
        match reply.code {
            Some(code) => Err(anyhow!("systemctl stop failed with exit status {code}{states}")),
            None => Err(anyhow!("systemctl stop was ended by a signal{states}")),
        }
    }

    /// One unit's state after a failed stop: systemd's own word when it is one
    /// of the three a failed stop can leave, else `unknown`. Nothing else the query
    /// prints is carried.
    fn unit_state(&self, unit: &str) -> &'static str {
        let Ok(reply) = self.systemctl.run(&["is-active", unit]) else {
            return "unknown";
        };
        match reply.stdout.trim() {
            "active" => "active",
            "inactive" => "inactive",
            "failed" => "failed",
            _ => "unknown",
        }
    }
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

fn date(now: DateTime<Utc>) -> Date {
    Date(now.format("%Y-%m-%d").to_string())
}

/// The closing resolution of `run` with `outcome`. Its sequence is the run's
/// next, counting a closure that was itself withdrawn. The one builder of a
/// closure for a run that ended without its own flush: a Self run whose
/// credential lease was refused (`self_run`, Recorded for a refusal, Withdrawn
/// for a misconfigured length: no run happened and none is counted).
pub(crate) fn closure(mind: &dyn MindPort, run: &PipelineRef, now: DateTime<Utc>, outcome: ResolutionOutcome, rationale: &str) -> Result<PipelineDocument> {
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
        outcome,
        rationale: rationale.into(),
        resolved_on: date(now),
    }))
}

/// The in-force runs of hers (operated by the mind) of `turn` on `instance`,
/// read from typed facts: the holders of that turn's slot. `self_run` reads
/// her open Self run through it.
pub(crate) fn holders(mind: &dyn MindPort, instance: &Slug, turn: RunTurn) -> Result<Vec<PipelineDocumentSummary>> {
    let runs = headers(mind, &in_force(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0])))?;
    Ok(runs
        .into_iter()
        .filter(|run| matches!(run.facts, PipelineFacts::Run { turn: held, operated_by: RunOperator::Mind, .. } if held.slot() == turn.slot()))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    struct Spy {
        calls: RefCell<Vec<Vec<String>>>,
        success: Cell<bool>,
        code: Cell<Option<i32>>,
        stdout: RefCell<String>,
        /// Per-unit answers to `is-active`; a unit not listed gets `stdout`.
        states: RefCell<Vec<(&'static str, &'static str)>>,
    }

    impl Systemctl for &'static Spy {
        fn run(&self, args: &[&str]) -> Result<SystemctlReply> {
            self.calls.borrow_mut().push(args.iter().map(|arg| arg.to_string()).collect());
            let said = match args {
                ["is-active", unit] => self.states.borrow().iter().find(|(named, _)| named == unit).map(|(_, state)| state.to_string()),
                _ => None,
            };
            Ok(SystemctlReply { success: self.success.get(), code: self.code.get(), stdout: said.unwrap_or_else(|| self.stdout.borrow().clone()) })
        }
    }

    fn spied() -> (SystemdLauncher, &'static Spy) {
        let spy: &'static Spy =
            Box::leak(Box::new(Spy { calls: RefCell::default(), success: Cell::new(true), code: Cell::new(Some(0)), stdout: RefCell::default(), states: RefCell::default() }));
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
    fn stop_args_is_one_synchronous_stop_of_exactly_the_units_start_names() {
        for name in ["eureka", "eureka.test", "eureka-two"] {
            let instance = Slug(name.into());
            let args = stop_args(StopScope::Mind(&instance)).unwrap();
            assert_eq!(args[..2], ["stop", "--"], "one synchronous stop, no --no-block");
            let (launcher, spy) = spied();
            let mut started = Vec::new();
            for turn in [RunTurn::SelfRun, RunTurn::PersonaTurn] {
                launcher.start(&instance, turn).unwrap();
                started.push(spy.calls.borrow().last().unwrap().last().unwrap().clone());
            }
            assert_eq!(args[2..4], started[..], "stop names exactly the units start names, for both turns");
            assert_eq!(args[4], format!("mind-stream@{name}.service"), "and the stream of that instance");
            assert_eq!(args.len(), 5);
            assert!(args[2..].iter().all(|unit| !unit.contains(['*', '?', '['])), "a glob can match another instance or none: {args:?}");
        }
        assert_eq!(stop_args(StopScope::Mind(&eureka())).unwrap(), ["stop", "--", "mind-self@eureka.service", "mind-persona@eureka.service", "mind-stream@eureka.service"]);
    }

    #[test]
    fn stop_runs_the_one_stop_and_waits() {
        let (launcher, spy) = spied();
        launcher.stop(StopScope::Mind(&eureka())).unwrap();
        assert_eq!(*spy.calls.borrow(), vec![stop_args(StopScope::Mind(&eureka())).unwrap()]);
    }

    #[test]
    fn the_swarm_stop_is_one_stop_of_both_templates_globs() {
        let (launcher, spy) = spied();
        launcher.stop(StopScope::Swarm).unwrap();
        assert_eq!(*spy.calls.borrow(), vec![vec!["stop", "--", "mind-self@*.service", "mind-persona@*.service", "mind-stream@*.service"]]);
    }

    #[test]
    fn a_failed_swarm_stop_names_the_exit_status_and_asks_no_state() {
        let (launcher, spy) = spied();
        spy.success.set(false);
        spy.code.set(Some(5));
        *spy.stdout.borrow_mut() = "canary-output-7f3a91
".to_string();
        let shown = format!("{:#}", launcher.stop(StopScope::Swarm).unwrap_err());
        assert!(shown.contains("exit status 5") && !shown.contains("canary"), "{shown}");
        assert_eq!(spy.calls.borrow().len(), 1, "no per-unit query for the swarm");
        spy.code.set(None);
        assert!(format!("{:#}", launcher.stop(StopScope::Swarm).unwrap_err()).contains("signal"));
    }

    #[test]
    fn stop_with_an_invalid_instance_is_an_error_and_runs_no_systemctl() {
        let (launcher, spy) = spied();
        for bad in ["canary-../evil-7f3a91", "canary instance", "", "canary*"] {
            let error = launcher.stop(StopScope::Mind(&Slug(bad.into()))).unwrap_err();
            assert!(!format!("{error:#}").contains("canary"), "{error:#}");
        }
        assert!(spy.calls.borrow().is_empty());
    }

    #[test]
    fn a_failed_stop_names_the_exit_status_and_each_units_state_and_carries_no_other_output() {
        let (launcher, spy) = spied();
        spy.success.set(false);
        spy.code.set(Some(5));
        spy.states.borrow_mut().extend([("mind-self@eureka.service", "inactive\n"), ("mind-persona@eureka.service", "active\n"), ("mind-stream@eureka.service", "failed\n")]);
        let shown = format!("{:#}", launcher.stop(StopScope::Mind(&eureka())).unwrap_err());
        assert!(shown.contains("exit status 5"), "{shown}");
        assert!(shown.contains("mind-self@eureka.service is inactive") && shown.contains("mind-persona@eureka.service is active") && shown.contains("mind-stream@eureka.service is failed"), "{shown}");
        let asked: Vec<Vec<String>> = spy.calls.borrow().iter().skip(1).cloned().collect();
        assert_eq!(asked, [["is-active", "mind-self@eureka.service"], ["is-active", "mind-persona@eureka.service"], ["is-active", "mind-stream@eureka.service"]], "one query per unit, after the one stop");
        // A state word outside the three, and anything systemctl prints with it, is not carried.
        spy.states.borrow_mut().clear();
        *spy.stdout.borrow_mut() = "canary-output-7f3a91\n".to_string();
        spy.code.set(None);
        let shown = format!("{:#}", launcher.stop(StopScope::Mind(&eureka())).unwrap_err());
        assert!(shown.contains("signal") && shown.contains("is unknown") && !shown.contains("canary"), "{shown}");
        spy.states.borrow_mut().push(("mind-self@eureka.service", "failed\n"));
        let shown = format!("{:#}", launcher.stop(StopScope::Mind(&eureka())).unwrap_err());
        assert!(shown.contains("mind-self@eureka.service is failed"), "{shown}");
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
