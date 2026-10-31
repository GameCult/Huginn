//! Her Self's inbox: the briefs her Persona has given her Self, derived from the
//! mind and told to the live session by the actuator's one stdin writer.
//!
//! The mind owns what her Self is asked: an in-force follow-up with owner
//! `self-run` whose source is a Persona turn's run is a brief. A follow-up her
//! Self files for herself, or one whose source the mind does not hold, is not.
//! `derive` lists the briefs the session has not been told (the told set is the
//! actuator's process memory, a cache; the mind is the truth). Her Self resolves
//! each brief she answers; `close` resolves the ones she left open at an idle
//! end. `backlog` lists, and never closes, the in-force briefs a closed run
//! claimed: the one-time sweep at deploy.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, RunTurn};
use huginn_mind::{PipelineAdmissionOutcome, PipelineFacts, PipelineStatusSummary};

use super::AGENT;
use crate::launch::closure;
use crate::queue::{MindPort, any_of, headers, in_force, of_kind};

/// The owner a follow-up names when it is a brief for her Self.
pub const OWNER: &str = "self-run";

/// One brief from her Persona: a follow-up of the mind, as the session hears it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brief {
    /// The follow-up's full id.
    pub id: String,
    pub campaign: String,
    pub repo: String,
    pub item: String,
    /// The admission ordinal: the order briefs were given in.
    pub ordinal: u64,
}

impl Brief {
    fn follow_up(&self) -> PipelineRef {
        PipelineRef { kind: PipelineKind::FollowUp, id: self.id.as_str().into() }
    }
}

/// The ids of the follow-ups a closed run claims, with the run each was claimed by.
fn claimed_by_closed(mind: &dyn MindPort) -> Result<HashMap<String, String>> {
    let runs = headers(mind, &any_of(of_kind(PipelineKind::Run), "root", &[&mind.instance().0]))?;
    let mut claimed = HashMap::new();
    for run in runs {
        let (PipelineStatusSummary::Resolved { .. }, PipelineFacts::Run { claims, .. }) = (&run.status, &run.facts) else { continue };
        for claim in claims.iter().filter(|claim| claim.kind == PipelineKind::FollowUp) {
            claimed.insert(claim.id.0.clone(), run.id.id.0.clone());
        }
    }
    Ok(claimed)
}

/// Every in-force brief, oldest first: owner `self-run`, sourced from a run of
/// the mind whose turn is a Persona turn.
fn open_briefs(mind: &dyn MindPort) -> Result<Vec<Brief>> {
    let mut candidates: Vec<(Brief, String)> = headers(mind, &in_force(of_kind(PipelineKind::FollowUp)))?
        .into_iter()
        .filter_map(|summary| match summary.facts {
            PipelineFacts::FollowUp { repo, owner, item, source, .. } if owner.0 == OWNER => {
                let campaign = summary.id.id.0.split(':').next().unwrap_or("").to_string();
                Some((Brief { id: summary.id.id.0, campaign, repo: repo.0, item: item.0, ordinal: summary.admission.ordinal }, source.id.0))
            }
            _ => None,
        })
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let persona: HashSet<String> = headers(mind, &any_of(of_kind(PipelineKind::Run), "root", &[&mind.instance().0]))?
        .into_iter()
        .filter(|run| matches!(run.facts, PipelineFacts::Run { turn: RunTurn::PersonaTurn, .. }))
        .map(|run| run.id.id.0)
        .collect();
    candidates.retain(|(_, source)| persona.contains(source));
    let mut briefs: Vec<Brief> = candidates.into_iter().map(|(brief, _)| brief).collect();
    briefs.sort_by_key(|brief| brief.ordinal);
    Ok(briefs)
}

/// Her briefs in force that the session has not been told.
pub fn derive(mind: &dyn MindPort, told: &HashSet<String>) -> Result<Vec<Brief>> {
    Ok(open_briefs(mind)?.into_iter().filter(|brief| !told.contains(&brief.id)).collect())
}

/// Her briefs in force that a closed run claimed, with that run's id: a list for
/// the one-time sweep at deploy. Nothing is admitted.
pub fn backlog(mind: &dyn MindPort) -> Result<Vec<(Brief, String)>> {
    let claimed = claimed_by_closed(mind)?;
    Ok(open_briefs(mind)?.into_iter().filter_map(|brief| claimed.get(&brief.id).cloned().map(|run| (brief, run))).collect())
}

/// One factual block per brief, then the plan line.
pub fn render(briefs: &[Brief], plan: &str) -> String {
    let mut blocks: Vec<String> = briefs
        .iter()
        .map(|brief| {
            format!(
                "A brief from your Persona: follow-up {} (campaign {}, repo {}): {}. {}",
                brief.id,
                brief.campaign,
                brief.repo,
                brief.item,
                resolving_sentence(&brief.id)
            )
        })
        .collect();
    if !plan.is_empty() {
        blocks.push(plan.to_string());
    }
    blocks.join("\n\n")
}

/// What her Self does when she answers or declines a brief.
pub fn resolving_sentence(id: &str) -> String {
    format!("When you answer this brief, resolve follow-up {id} Recorded with your answer as rationale; if you decline it, resolve it Withdrawn with why.")
}

/// Resolves Recorded each of `briefs` that is still in force, in one admission:
/// a brief her Self resolved herself is left as it is. Returns how many.
pub fn close(mind: &dyn MindPort, now: DateTime<Utc>, session: &str, briefs: &[(Brief, String)], reason: &str) -> Result<usize> {
    let standing: HashSet<String> = headers(mind, &in_force(of_kind(PipelineKind::FollowUp)))?.into_iter().map(|summary| summary.id.id.0).collect();
    let mut documents: Vec<PipelineDocument> = Vec::new();
    for (brief, rationale) in briefs.iter().filter(|(brief, _)| standing.contains(&brief.id)) {
        documents.push(closure(mind, &brief.follow_up(), now, ResolutionOutcome::Recorded { reason: reason.into() }, rationale)?);
    }
    let count = documents.len();
    if count == 0 {
        return Ok(0);
    }
    match mind.admit(AGENT, session, documents)? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(count),
        _ => Err(anyhow!("the briefs could not be closed")),
    }
}

#[cfg(test)]
mod tests;
