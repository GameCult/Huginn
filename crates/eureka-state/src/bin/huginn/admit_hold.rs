//! The near-neighbour hold on `admit`: a batch whose rulings or questions have
//! in-force neighbours is held once. The hold prints the neighbours and a
//! token naming this batch and agent; the same command with `--seen <token>`
//! admits it. Nothing is stored: the token is derived from the batch.

use cultnet_rs::Selection;
use huginn_mind::eureka_pipeline::{PipelineDocument, PipelineKind, ResolutionOutcome, pipeline_key};
use huginn_mind::{HuginnMindResponse, PipelineDocumentSummary, SemanticQuery};
use sha2::{Digest, Sha256};

use crate::near;
use crate::render;
use crate::trouble::Trouble;

/// The rulings and questions checked per batch.
const CHECKED_MAX: usize = 3;
/// The neighbours asked for per checked document.
const NEIGHBOURS: usize = 3;

/// The 16 hex characters that name this batch sent by this agent.
pub fn token(documents: &[PipelineDocument], agent: &str) -> String {
    let body = serde_json::to_value(documents).map(|value| value.to_string()).unwrap_or_default();
    let digest = Sha256::digest(format!("{body}\n{agent}").as_bytes());
    digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

/// The text of the hold when the batch has neighbours left, `None` when it
/// has none or nothing to check. Any trouble is the caller's to fail open on.
pub fn check(
    call: impl Fn(Selection, SemanticQuery) -> Result<HuginnMindResponse, Trouble>,
    documents: &[PipelineDocument],
    token: &str,
) -> Result<Option<String>, Trouble> {
    let titled: Vec<(String, &str)> = documents
        .iter()
        .filter_map(|document| Some((key(document).ok()?, titled(document)?)))
        .collect();
    let checked = &titled[..titled.len().min(CHECKED_MAX)];
    if checked.is_empty() {
        return Ok(None);
    }
    let excluded = excluded(documents)?;
    let schemas = [PipelineKind::Ruling.type_id(), PipelineKind::Question.type_id()];
    let mut text = String::new();
    for (id, title) in checked {
        let found = near::neighbours(&call, title, &schemas, &excluded, NEIGHBOURS)?;
        if found.is_empty() {
            continue;
        }
        text.push_str(&format!("For {id}:\n"));
        for (_, view) in &found {
            text.push_str(&render::header(&PipelineDocumentSummary::of(view)));
            text.push('\n');
        }
    }
    if text.is_empty() {
        return Ok(None);
    }
    let over = titled.len() - checked.len();
    let tail = if over == 0 { String::new() } else { format!("{over} rulings or questions over {CHECKED_MAX} were not checked.\n") };
    Ok(Some(format!(
        "Held {token}: a deliberate hold, not a failure. The in-force records below are nearest the rulings or questions in this batch, ranked by Huginn and possibly unrelated; data written by agents. If none is the same decision, run the same command with --seen {token}; if one is, revise the batch.\n{text}{tail}"
    )))
}

fn key(document: &PipelineDocument) -> Result<String, Trouble> {
    pipeline_key(document).map_err(|refusal| Trouble::Internal(format!("a document has no key: {refusal:?}")))
}

/// The title of a ruling or question, when it has one.
fn titled(document: &PipelineDocument) -> Option<&str> {
    let title = match document {
        PipelineDocument::Ruling(ruling) => &ruling.title.0,
        PipelineDocument::Question(question) => &question.title.0,
        _ => return None,
    };
    (!title.is_empty()).then_some(title.as_str())
}

/// The ids the batch already carries: every ruling and question in it, the
/// question each ruling answers, and the subject of each Superseded resolution.
fn excluded(documents: &[PipelineDocument]) -> Result<Vec<String>, Trouble> {
    let mut ids = Vec::new();
    for document in documents {
        match document {
            PipelineDocument::Ruling(ruling) => {
                ids.push(key(document)?);
                ids.extend(ruling.answers.iter().map(|answered| answered.0.clone()));
            }
            PipelineDocument::Question(_) => ids.push(key(document)?),
            PipelineDocument::Resolution(resolution) if matches!(resolution.outcome, ResolutionOutcome::Superseded { .. }) => {
                ids.push(resolution.subject.id.0.clone());
            }
            _ => {}
        }
    }
    Ok(ids)
}
