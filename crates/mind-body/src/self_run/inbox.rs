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
use cultnet_rs::Selection;
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
    pub(crate) conversation: persona::Conversation,
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

/// The conversation a follow-up was given in, read by its key whether or not it is still in force; none when the mind does not hold it.
pub(crate) fn conversation_by_key(mind: &dyn MindPort, id: &str) -> Result<Option<persona::Conversation>> {
    let one = Selection { keys: Some(vec![id.to_string()]), ..of_kind(PipelineKind::FollowUp) };
    Ok(headers(mind, &one)?.into_iter().find_map(|header| match header.facts {
        PipelineFacts::FollowUp { origin, .. } => Some(persona::Conversation::of_origin(origin.as_ref())),
        _ => None,
    }))
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

/// How a brief closes: the one owner of what done means for her Self. The start message tells it
/// once; every told brief and the flush point back to it.
pub(crate) const CLOSING_RULE: &str = "A brief is done when what it asks for exists. A brief that asks for an answer, a review or a read is done when you answer it: resolve it Recorded with the answer as rationale. A brief that asks for an act with no commit (admitting records, a forge setting, a post, an ops step) is done when the act is done: resolve it Recorded with what you did and where it can be seen. A brief that asks for a change is done when the change is on its repo's default branch, and live where the brief asks for a deploy: resolve it Fixed with the commit on the default branch that contains the change, the merge or squash commit and not the PR head. An open PR, a pushed branch or a local commit is not done, and the brief stays open. In a repo you steward the merge is yours and needs no one's leave: have your Soul check the work, then merge it, then resolve the brief (rulings steward-is-maintainer and merge-gate-own-soul). Land what you have open before you start more. When what is left waits on someone else (a review in a repo you do not steward, a ruling, a deploy you do not hold), admit a follow-up of your own whose item names the PR or branch and whom it waits on, resolve the brief Superseded by it, and say so in your reply. If you decline a brief, resolve it Withdrawn with why.";

/// The reminder under each told brief: the closing rule in short, for this brief's id.
pub fn resolving_sentence(id: &str) -> String {
    format!("Close follow-up {id} by the closing rule: Recorded only if it asked just for an answer or for an act with no commit, with what you did; Fixed by the merge or squash commit on the default branch if it asked for a change; Superseded by your own follow-up while it waits on someone else; Withdrawn with why. An open PR, a pushed branch or a local commit is not done.")
}

#[cfg(test)]
mod tests;
