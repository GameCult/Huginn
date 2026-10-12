//! The waker: one cycle, no memory between cycles.
//!
//! A cycle is three steps in a fixed order. (1) The brake and the dial are read
//! once, through `control::derive`, the derivation behind `read_effective` and
//! the unit gate; held ends the cycle before the mind, the clock or a unit is
//! touched. (2) The newest Persona turn of hers is found, whatever became of
//! it, and her cadence (the Grant's, from the dial) is waited out since its
//! admission. (3) `open_and_launch` is called once for a Persona turn with no
//! claims and the Grant, and its answer is the cycle's.
//!
//! The waker owns no file, no timer state and no cursor: the run in the mind is
//! the grant (target invariant run-is-the-grant), and whether she is awake is
//! decided by admission and answered by the launch primitive. The timer that
//! calls a cycle is a fixed poll; it decides nothing. The waker starts no Self
//! run and ends no run or unit.

use std::fmt;

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineKind, PipelineRef, RunOperator, RunTurn, Slug};
use huginn_mind::PipelinePageItems;

use crate::control::{ControlSource, Effective, HeldReason, derive};
use crate::launch::{Declined, LaunchOutcome, LaunchRequest, Ports, open_and_launch};
use crate::queue::{MindPort, any_of, of_kind};

/// The admitting agent's name on the receipt of every run the waker opens.
pub const AGENT: &str = "mind-wake";

/// What one cycle did. Every outcome is an answer; only a mind that cannot be
/// reached is an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WakeOutcome {
    /// The brake or the dial did not allow a launch; nothing else was touched.
    Held { reason: HeldReason },
    /// Her cadence since the newest Persona turn has not passed; `until` is when it has.
    NotYet { until: DateTime<Utc> },
    /// A Persona turn of hers is live.
    Busy { run: PipelineRef },
    Launched { run: PipelineRef },
    Refused(Declined),
}

impl fmt::Display for WakeOutcome {
    /// One line, with ids and times the mind gave and never a value a refusal
    /// carries.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held { reason } => write!(f, "held: {reason:?}"),
            Self::NotYet { until } => write!(f, "not yet: until {}", until.to_rfc3339()),
            Self::Busy { run } => write!(f, "busy: {}", run.id.0),
            Self::Launched { run } => write!(f, "launched: {}", run.id.0),
            Self::Refused(declined) => write!(f, "refused: {}", declined_code(declined)),
        }
    }
}

fn declined_code(declined: &Declined) -> &'static str {
    match declined {
        Declined::Admission(_) => "admission",
        Declined::Conflict => "conflict",
        Declined::AlreadyAdmitted => "already_admitted",
        Declined::StartFailed => "start_failed",
        Declined::GrantForOtherInstance => "grant_for_other_instance",
        Declined::PersonaRunTakesNoClaims => "persona_run_takes_claims",
    }
}

/// When the newest Persona turn of hers, of any status, was admitted. The
/// spellings of the two filters are the types' own, and one page of one row is
/// asked for: an idle wake is a query.
fn newest_persona_turn(mind: &dyn MindPort, instance: &Slug) -> Result<Option<DateTime<Utc>>> {
    let turn = format!("{:?}", RunTurn::PersonaTurn);
    let operator = format!("{:?}", RunOperator::Mind);
    let runs = any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]);
    let selection = Selection { descending: true, limit: Some(1), ..any_of(any_of(runs, "turn", &[&turn]), "operated_by", &[&operator]) };
    let PipelinePageItems::Headers(headers) = mind.query(&selection)?.items else {
        bail!("the mind answered documents to a header selection");
    };
    headers
        .first()
        .map(|newest| {
            DateTime::parse_from_rfc3339(&newest.admission.admitted_at)
                .map(|at| at.with_timezone(&Utc))
                .map_err(|_| anyhow!("the newest Persona turn's admission time is not a timestamp"))
        })
        .transpose()
}

/// One cycle. `control` is the instance's control store; the reading is taken
/// once and its Grant is the one handed to the launch.
pub fn cycle(ports: &Ports, control: &impl ControlSource) -> Result<WakeOutcome> {
    let instance = ports.mind.instance().clone();
    let grant = match derive(control, &instance) {
        Effective::Held { reason } => return Ok(WakeOutcome::Held { reason }),
        Effective::Released(grant) => grant,
    };
    if let Some(newest) = newest_persona_turn(ports.mind, &instance)? {
        let cadence = Duration::from_std(grant.cadence()).map_err(|_| anyhow!("the dial's cadence is out of range"))?;
        let until = newest + cadence;
        if until > ports.clock.now() {
            return Ok(WakeOutcome::NotYet { until });
        }
    }
    let request = LaunchRequest { turn: RunTurn::PersonaTurn, claims: vec![], agent: AGENT.into(), grant };
    match open_and_launch(ports, request)? {
        LaunchOutcome::Launched { run } => Ok(WakeOutcome::Launched { run }),
        LaunchOutcome::Busy { run } => Ok(WakeOutcome::Busy { run }),
        LaunchOutcome::Refused(declined) => Ok(WakeOutcome::Refused(declined)),
        LaunchOutcome::NotQueued { .. } | LaunchOutcome::Tripped { .. } => bail!("the launch answered a Persona turn with an answer only a Self run has"),
    }
}

#[cfg(test)]
mod tests;
