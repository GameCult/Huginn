//! Her Self's inbox: the briefs her Persona has given her Self, derived from the
//! mind, spooled for the session, claimed exactly once, and closed by the answer.
//!
//! The mind owns what her Self is asked: an in-force follow-up with owner
//! `self-run` is a brief not yet answered. `derive` lists those that no closed
//! run claims and that the session does not hold yet; `spool` writes each as
//! `<spool>/<ordinal>-<label>.json`. A claim renames entries into `<spool>/told/`
//! and returns them: the rename is the exactly-once point, so the PostToolBatch
//! hook (`inject`) and the actuator's idle tell race safely, and `told/` is the
//! record of what this session has heard. A brief is closed by the result that
//! answers it (`close`), and the same rule closes, at session start, the briefs
//! a closed run claimed (`close_past`). Everything here is derived or a cache:
//! the spool is lost with the unit and rebuilt from the mind.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome};
use huginn_mind::{PipelineAdmissionOutcome, PipelineFacts, PipelineStatusSummary};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::AGENT;
use crate::launch::closure;
use crate::queue::{MindPort, any_of, headers, in_force, of_kind};

/// The owner a follow-up names when it is a brief for her Self.
pub const OWNER: &str = "self-run";
/// The entries this session has claimed.
const TOLD: &str = "told";
/// The plan line as of the lease grant, which a tell ends with.
const PLAN: &str = "plan.txt";
/// The PreCompact hook's marker: the ceiling is reached by the CLI's own measure.
const PRE_COMPACT: &str = "pre-compact";
/// What the hook event is called when its input does not say.
const DEFAULT_EVENT: &str = "PostToolBatch";

/// One brief from her Persona: a follow-up of the mind, as the session hears it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    fn file_name(&self) -> String {
        let label = self.id.rsplit(':').next().unwrap_or(&self.id);
        format!("{:020}-{label}.json", self.ordinal)
    }
}

/// Where the hooks find the spool and the program they run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hooks {
    pub spool: PathBuf,
    /// The absolute path of `mind-self-run`.
    pub injector: PathBuf,
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

impl Hooks {
    /// The `--settings` JSON: the tell between tool calls, and the block on auto-compaction.
    pub fn settings(&self) -> String {
        let command = format!("{} inject --spool {}", quoted(&self.injector), quoted(&self.spool));
        json!({"hooks": {
            "PostToolBatch": [{"hooks": [{"type": "command", "command": command}]}],
            "PreCompact": [{"matcher": "auto", "hooks": [{"type": "command", "command": format!("{command} --pre-compact")}]}],
        }})
        .to_string()
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

/// Every in-force follow-up with owner `self-run`, oldest first.
fn open_briefs(mind: &dyn MindPort) -> Result<Vec<Brief>> {
    let mut briefs: Vec<Brief> = headers(mind, &in_force(of_kind(PipelineKind::FollowUp)))?
        .into_iter()
        .filter_map(|summary| match summary.facts {
            PipelineFacts::FollowUp { repo, owner, item, .. } if owner.0 == OWNER => {
                let campaign = summary.id.id.0.split(':').next().unwrap_or("").to_string();
                Some(Brief { id: summary.id.id.0, campaign, repo: repo.0, item: item.0, ordinal: summary.admission.ordinal })
            }
            _ => None,
        })
        .collect();
    briefs.sort_by_key(|brief| brief.ordinal);
    Ok(briefs)
}

/// Her open briefs that no closed run claims and the session does not hold yet.
pub fn derive(mind: &dyn MindPort, spool: &Path) -> Result<Vec<Brief>> {
    let held: HashSet<String> = entries(spool)?.into_iter().chain(entries(&spool.join(TOLD))?).map(|brief| brief.id).collect();
    let mut open: Vec<Brief> = open_briefs(mind)?.into_iter().filter(|brief| !held.contains(&brief.id)).collect();
    if !open.is_empty() {
        let claimed = claimed_by_closed(mind)?;
        open.retain(|brief| !claimed.contains_key(&brief.id));
    }
    Ok(open)
}

fn entries(dir: &Path) -> Result<Vec<Brief>> {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("the inbox could not be read"),
    };
    let mut names: Vec<PathBuf> = read.filter_map(|entry| entry.ok()).filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file())).map(|entry| entry.path()).collect();
    names.retain(|path| path.extension().is_some_and(|extension| extension == "json") && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.')));
    names.sort();
    names.iter().filter_map(|path| read_entry(path).transpose()).collect()
}

/// None where another claimer renamed the entry away since it was listed.
fn read_entry(path: &Path) -> Result<Option<Brief>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("an inbox entry could not be read"),
    };
    serde_json::from_str(&text).map(Some).context("an inbox entry is not a brief")
}

fn write_entry(dir: &Path, brief: &Brief) -> Result<()> {
    std::fs::create_dir_all(dir).context("the inbox could not be made")?;
    let name = brief.file_name();
    // Written under a dot name, then renamed: a claimer never sees half an entry.
    let partial = dir.join(format!(".{name}"));
    std::fs::write(&partial, serde_json::to_vec(brief)?).context("an inbox entry could not be written")?;
    std::fs::rename(&partial, dir.join(name)).context("an inbox entry could not be placed")
}

/// Writes each brief into the spool.
pub fn spool(spool: &Path, briefs: &[Brief]) -> Result<()> {
    briefs.iter().try_for_each(|brief| write_entry(spool, brief))
}

/// Records briefs as already heard (the start message carries them).
pub fn mark_told(spool: &Path, briefs: &[Brief]) -> Result<()> {
    briefs.iter().try_for_each(|brief| write_entry(&spool.join(TOLD), brief))
}

/// Whether a brief waits in the spool.
pub fn pending(spool: &Path) -> Result<bool> {
    Ok(!entries(spool)?.is_empty())
}

/// Takes every waiting entry. A rename into `told/` is the claim: an entry some
/// other claimer renamed first is theirs, and is not returned here.
pub fn claim(spool: &Path) -> Result<Vec<Brief>> {
    let told = spool.join(TOLD);
    std::fs::create_dir_all(&told).context("the inbox could not be made")?;
    let mut taken = Vec::new();
    for brief in entries(spool)? {
        let name = brief.file_name();
        match std::fs::rename(spool.join(&name), told.join(&name)) {
            Ok(()) => taken.push(brief),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("an inbox entry could not be claimed"),
        }
    }
    Ok(taken)
}

/// The briefs this session has heard and not yet closed.
pub fn told(spool: &Path) -> Result<Vec<Brief>> {
    entries(&spool.join(TOLD))
}

/// Forgets heard briefs once they are closed.
pub fn forget(spool: &Path, briefs: &[Brief]) {
    for brief in briefs {
        let _ = std::fs::remove_file(spool.join(TOLD).join(brief.file_name()));
    }
}

/// Starts the spool empty: a session hears only what it is told after it starts.
pub fn reset(spool: &Path) -> Result<()> {
    match std::fs::remove_dir_all(spool) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("a stale inbox could not be removed"),
    }
    std::fs::create_dir_all(spool).context("the inbox could not be made")
}

/// The plan line a tell ends with: as of the lease grant.
pub fn write_plan(spool: &Path, line: &str) -> Result<()> {
    std::fs::create_dir_all(spool).context("the inbox could not be made")?;
    std::fs::write(spool.join(PLAN), line).context("the plan line could not be written")
}

/// One factual block per brief, then the plan line.
pub fn render(briefs: &[Brief], plan: &str) -> String {
    let mut blocks: Vec<String> = briefs
        .iter()
        .map(|brief| {
            format!(
                "A brief from your Persona: follow-up {} (campaign {}, repo {}): {}. Your answer closes it; work you take on beyond this answer is a follow-up of your own.",
                brief.id, brief.campaign, brief.repo, brief.item
            )
        })
        .collect();
    if !plan.is_empty() {
        blocks.push(plan.to_string());
    }
    blocks.join("\n\n")
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

/// The rule applied to the past: every open brief that a closed run claims is
/// resolved Recorded, naming that run, whose closure holds the result.
pub fn close_past(mind: &dyn MindPort, now: DateTime<Utc>, session: &str) -> Result<usize> {
    let claimed = claimed_by_closed(mind)?;
    let answered: Vec<(Brief, String)> = open_briefs(mind)?
        .into_iter()
        .filter_map(|brief| {
            let run = claimed.get(&brief.id)?;
            let rationale = format!("Claimed by run {run}, which has closed; its closure holds the result.");
            Some((brief, rationale))
        })
        .collect();
    close(mind, now, session, &answered, "answered by a closed run")
}

/// What a hook prints, and the status it exits with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Injection {
    pub stdout: String,
    pub exit: i32,
}

/// The hook: data only. It claims the spool and prints what it claimed; with an
/// agent id (a subagent's call) it prints nothing; `pre_compact` writes the
/// marker and exits 2, which blocks the compaction. It reads neither the mind,
/// chat nor the lease, and opens no socket.
pub fn inject(spool: &Path, input: &str, pre_compact: bool) -> Result<Injection> {
    if pre_compact {
        std::fs::create_dir_all(spool).context("the inbox could not be made")?;
        std::fs::write(spool.join(PRE_COMPACT), b"").context("the marker could not be written")?;
        return Ok(Injection { stdout: String::new(), exit: 2 });
    }
    let event: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    if event.get("agent_id").is_some_and(|id| !id.is_null()) {
        return Ok(Injection { stdout: String::new(), exit: 0 });
    }
    let briefs = claim(spool)?;
    if briefs.is_empty() {
        return Ok(Injection { stdout: String::new(), exit: 0 });
    }
    let plan = std::fs::read_to_string(spool.join(PLAN)).unwrap_or_default();
    let name = event.get("hook_event_name").and_then(Value::as_str).unwrap_or(DEFAULT_EVENT);
    let stdout = json!({"hookSpecificOutput": {"hookEventName": name, "additionalContext": render(&briefs, &plan)}}).to_string();
    Ok(Injection { stdout, exit: 0 })
}

/// Whether the PreCompact hook fired since the last look; the marker is consumed.
pub fn take_pre_compact(spool: &Path) -> bool {
    std::fs::remove_file(spool.join(PRE_COMPACT)).is_ok()
}

#[cfg(test)]
mod tests;
