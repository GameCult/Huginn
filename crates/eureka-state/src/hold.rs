//! What an open question holds: the one decider, read by the heartbeat's gate
//! and by `huginn blocked`, so the two cannot disagree.
//!
//! A question raised in a run holds every claim of that run, the set that
//! failed together; a question raised in anything else holds that item. A
//! question answered with the option `retire` keeps holding the claims of the
//! run it was raised in for as long as they are in force: the answer has said
//! they are not to be worked, and whoever resolves them is the one writer of
//! that. Answering or withdrawing a question with any other choice lifts its
//! hold. The question's text is display only.
//!
//! Nothing here keeps state. Every answer is read through `ask`, one typed
//! selection at a time, so a caller with a daemon and a caller with a
//! temporary mind read the same way.

use anyhow::{Result, bail};
use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef, Selection};
use huginn_mind::eureka_pipeline::{PipelineKind, PipelineRef};
use huginn_mind::{
    PipelineDocumentSummary, PipelineFacts, PipelinePageItems, PipelineSelectionPage,
    PipelineStatusSummary,
};

/// The option label of a breaker question whose answer retires the claims it holds.
pub const RETIRE: &str = "retire";

/// One page of a selection, from whoever reads the mind.
pub type Ask<'a> = dyn Fn(&Selection) -> Result<PipelineSelectionPage> + 'a;

/// A question and the items it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hold {
    pub question: PipelineDocumentSummary,
    pub items: Vec<PipelineRef>,
    /// The question is answered `retire`: the items are held until they are resolved.
    pub retired: bool,
}

/// The documents that cite `target`, under `role` or any.
pub fn citing(target: &PipelineRef, role: Option<&str>) -> Citation {
    Citation {
        target: RecordRef::new(target.kind.type_id(), target.id.0.clone()),
        role: role.map(Into::into),
    }
}

pub fn of_kind(kind: PipelineKind) -> Selection {
    Selection {
        schemas: Some(vec![kind.type_id().to_string()]),
        ..Selection::default()
    }
}

pub fn any_of(mut selection: Selection, index: &str, values: &[&str]) -> Selection {
    selection
        .fields
        .get_or_insert_with(Vec::new)
        .push(FieldPredicate {
            index: index.into(),
            op: "any_of".into(),
            values: Some(values.iter().map(|value| value.to_string()).collect()),
            number: None,
        });
    selection
}

pub fn in_force(selection: Selection) -> Selection {
    any_of(selection, "in_force", &["true"])
}

/// Every page of a selection, in the mind's order. The one paging loop.
pub fn headers(ask: &Ask, selection: &Selection) -> Result<Vec<PipelineDocumentSummary>> {
    let mut all = Vec::new();
    let mut asking = selection.clone();
    loop {
        let page = ask(&asking)?;
        let PipelinePageItems::Headers(items) = page.items else {
            bail!("the mind answered documents to a header selection")
        };
        all.extend(items);
        match page.next {
            Some(cursor) => asking.cursor = Some(cursor),
            None => return Ok(all),
        }
    }
}

/// The claims of the run `run`, resolved or not (a citation outlives its
/// resolution). Empty when the mind has no such run.
pub fn claims_of(ask: &Ask, run: &PipelineRef) -> Result<Vec<PipelineRef>> {
    let one = Selection {
        keys: Some(vec![run.id.0.clone()]),
        ..of_kind(PipelineKind::Run)
    };
    Ok(headers(ask, &one)?
        .into_iter()
        .flat_map(|header| match header.facts {
            PipelineFacts::Run { claims, .. } => claims,
            _ => Vec::new(),
        })
        .collect())
}

/// Every question holding anything: the open ones, each with what it holds,
/// then the answered-`retire` ones whose claims are still in force.
pub fn holds(ask: &Ask) -> Result<Vec<Hold>> {
    let mut found = Vec::new();
    for question in headers(ask, &in_force(of_kind(PipelineKind::Question)))? {
        if let PipelineFacts::Question {
            raised_in: Some(at),
            ..
        } = &question.facts
        {
            let items = if at.kind == PipelineKind::Run {
                claims_of(ask, at)?
            } else {
                vec![at.clone()]
            };
            found.push(Hold {
                question,
                items,
                retired: false,
            });
        }
    }
    let asked_in = Selection {
        cited: Some(Incoming {
            role: "raised_in".into(),
            exists: true,
        }),
        ..of_kind(PipelineKind::Run)
    };
    for run in headers(ask, &asked_in)? {
        let questions = Selection {
            cites: Some(citing(&run.id, Some("raised_in"))),
            ..of_kind(PipelineKind::Question)
        };
        for question in headers(ask, &questions)? {
            if matches!(question.status, PipelineStatusSummary::InForce)
                || !answered_retire(ask, &question.id)?
            {
                continue;
            }
            let mut items = Vec::new();
            for claim in claims_of(ask, &run.id)? {
                let live = Selection {
                    keys: Some(vec![claim.id.0.clone()]),
                    ..in_force(of_kind(claim.kind))
                };
                if !headers(ask, &live)?.is_empty() {
                    items.push(claim);
                }
            }
            if !items.is_empty() {
                found.push(Hold {
                    question,
                    items,
                    retired: true,
                });
            }
        }
    }
    Ok(found)
}

/// Whether a ruling answers `question` with the option `retire`.
fn answered_retire(ask: &Ask, question: &PipelineRef) -> Result<bool> {
    let answers = Selection {
        cites: Some(citing(question, Some("answers"))),
        ..of_kind(PipelineKind::Ruling)
    };
    Ok(headers(ask, &answers)?.iter().any(|ruling| matches!(&ruling.facts, PipelineFacts::Ruling { choice: Some(choice), .. } if choice.0 == RETIRE)))
}
