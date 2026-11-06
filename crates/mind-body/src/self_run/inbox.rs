//! Her Self's inbox: the briefs her Persona has given her Self, derived from the
//! mind and told to the live session by the actuator's one stdin writer.
//!
//! The mind owns what her Self is asked: an in-force follow-up with owner
//! `self-run` that her Persona turn admitted is a brief. Who admitted it is the
//! receipt's provenance, which the mind records and a writer cannot choose; the
//! follow-up's `source` is the writer's word and decides nothing. A follow-up her
//! Self files for herself is not a brief, whatever it cites. `derive` lists the
//! briefs the session has not been told (the told set is the actuator's process
//! memory, a cache; the mind is the truth). Her Self resolves each brief she
//! answers and the actuator closes none. `backlog` lists, and never closes, the
//! in-force briefs a closed run claimed: the one-time sweep at deploy.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use eureka_pipeline::PipelineKind;
use huginn_mind::{PipelineFacts, PipelineStatusSummary};

use crate::persona;
use crate::queue::{Held, MindPort, any_of, headers, in_force, of_kind, on_hold};

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
    /// The conversation her Persona was in when it gave the brief, from the origin the follow-up records
    /// (none recorded, a brief admitted before origins existed, is the public one).
    pub conversation: persona::Conversation,
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

/// Every in-force brief, oldest first: owner `self-run`, admitted by her Persona
/// turn (the receipt's agent), whatever source the writer named.
fn open_briefs(mind: &dyn MindPort) -> Result<Vec<Brief>> {
    let mut briefs: Vec<Brief> = headers(mind, &in_force(of_kind(PipelineKind::FollowUp)))?
        .into_iter()
        .filter_map(|summary| match summary.facts {
            PipelineFacts::FollowUp { repo, owner, item, origin, .. } if owner.0 == OWNER && summary.admission.provenance.agent.0 == persona::AGENT => {
                let campaign = summary.id.id.0.split(':').next().unwrap_or("").to_string();
                Some(Brief { id: summary.id.id.0, campaign, repo: repo.0, item: item.0, ordinal: summary.admission.ordinal, conversation: persona::Conversation::of_origin(origin.as_ref()) })
            }
            _ => None,
        })
        .collect();
    briefs.sort_by_key(|brief| brief.ordinal);
    Ok(briefs)
}

/// Her briefs in force that the session has not been told and no open question
/// holds: a held brief is neither claimed nor told until its question is
/// answered or withdrawn. Reads the hold once.
pub fn derive(mind: &dyn MindPort, told: &HashSet<String>) -> Result<Vec<Brief>> {
    Ok(derive_counting_held(mind, &on_hold(mind)?, told)?.0)
}

/// `derive` against a hold the caller already read, and how many of her briefs
/// in force that hold keeps back.
pub fn derive_counting_held(mind: &dyn MindPort, holds: &Held, told: &HashSet<String>) -> Result<(Vec<Brief>, usize)> {
    let (held, free): (Vec<Brief>, Vec<Brief>) =
        open_briefs(mind)?.into_iter().partition(|brief| holds.items().iter().any(|item| item.kind == PipelineKind::FollowUp && item.id.0 == brief.id));
    Ok((free.into_iter().filter(|brief| !told.contains(&brief.id)).collect(), held.len()))
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

#[cfg(test)]
mod tests;
