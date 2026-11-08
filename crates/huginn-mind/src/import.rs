//! The planner of a campaign import: what the receiver of a moved campaign replays from the
//! giver, and in what batches.
//!
//! `plan` is a function of the giver's views of the campaign's root and nothing else, so the
//! `huginn import` verb and the move tests share one definition of a replay batch. It reads no
//! mind and writes none: the verb sends what it returns, and the receiver's admission decides
//! every write.

use std::collections::{BTreeMap, BTreeSet};

use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, Short, Slug};

use crate::admission::BATCH_MAX;
use crate::docs::citations;
use crate::query::{PipelineDocumentView, PipelineStatus};
use crate::receipt::{IMPORT_EXTERNAL_MAX, ImportSource};
use crate::rows::root_and_local;

/// A moved campaign's history as the receiver replays it: the Moved resolution that moved it, and
/// one batch per source receipt in ascending source ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportPlan {
    pub moved: PipelineRef,
    pub batches: Vec<ImportBatch>,
}

/// One source receipt's documents under the root, minus the Moved resolution. `source` quotes the
/// receipt with no externals: the caller declares the `foreign` referents the receiver does not hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBatch {
    pub source: ImportSource,
    pub documents: Vec<(PipelineRef, PipelineDocument)>,
    /// Referents the batch cites under another root, in id order.
    pub foreign: Vec<PipelineRef>,
}

/// Why a campaign cannot be replayed from these views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportFault {
    /// No in-force Moved resolution closes the campaign.
    NotMoved,
    /// The campaign moved to another mind.
    MovedElsewhere { to: Slug },
    /// The source is the receiver.
    SourceIsReceiver,
    /// A source receipt wrote more documents under the root than one batch holds.
    BatchTooLarge { ordinal: u64, documents: usize },
    /// A batch cites more foreign referents than one import may declare.
    ExternalsTooMany { ordinal: u64, count: usize },
    /// A batch cites a document of its own root that no earlier receipt wrote.
    Unordered { ordinal: u64, id: Short },
}

/// The plan for `receiver` to import `campaign` from `source`, given every document the source
/// holds under the campaign's root.
pub fn plan(campaign: &Slug, source: &Slug, receiver: &Slug, views: Vec<PipelineDocumentView>) -> Result<ImportPlan, ImportFault> {
    let under_root = |view: &&PipelineDocumentView| root_and_local(&view.id.id.0).0 == campaign.0;
    let (moved, to) = views
        .iter()
        .filter(under_root)
        .find_map(|view| match &view.document {
            PipelineDocument::Resolution(resolution) if view.status == PipelineStatus::InForce && resolution.subject.kind == PipelineKind::Campaign => {
                match &resolution.outcome {
                    ResolutionOutcome::Moved { to } => Some((view.id.clone(), to.clone())),
                    _ => None,
                }
            }
            _ => None,
        })
        .ok_or(ImportFault::NotMoved)?;
    if to != *receiver {
        return Err(ImportFault::MovedElsewhere { to });
    }
    if source == receiver {
        return Err(ImportFault::SourceIsReceiver);
    }

    let mut receipts: BTreeMap<u64, Vec<&PipelineDocumentView>> = BTreeMap::new();
    for view in views.iter().filter(under_root).filter(|view| view.id != moved) {
        receipts.entry(view.admission.ordinal).or_default().push(view);
    }
    let mut written: BTreeSet<&str> = BTreeSet::new();
    let mut batches = Vec::new();
    for (ordinal, mut group) in receipts {
        group.sort_by(|a, b| a.id.id.0.cmp(&b.id.id.0));
        if group.len() > BATCH_MAX {
            return Err(ImportFault::BatchTooLarge { ordinal, documents: group.len() });
        }
        let in_batch: BTreeSet<&str> = group.iter().map(|view| view.id.id.0.as_str()).collect();
        let mut foreign: BTreeMap<&str, PipelineRef> = BTreeMap::new();
        for view in &group {
            for (_, target) in cited(&view.document, receiver) {
                let key = target.id.0.as_str();
                if in_batch.contains(key) || written.contains(key) {
                    continue;
                }
                if root_and_local(key).0 == campaign.0 {
                    return Err(ImportFault::Unordered { ordinal, id: Short(key.to_string()) });
                }
                foreign.entry(key).or_insert_with(|| target.clone());
            }
        }
        if foreign.len() > IMPORT_EXTERNAL_MAX {
            return Err(ImportFault::ExternalsTooMany { ordinal, count: foreign.len() });
        }
        let facts = &group[0].admission;
        batches.push(ImportBatch {
            source: ImportSource {
                instance: source.clone(),
                receipt_id: Short(facts.receipt_id.clone()),
                ordinal,
                committed_at: Short(facts.admitted_at.clone()),
                faculty: facts.provenance.faculty,
                agent: facts.provenance.agent.clone(),
                session: facts.provenance.session.clone(),
                tool: facts.provenance.tool.clone(),
                external: Vec::new(),
            },
            documents: group.iter().map(|view| (view.id.clone(), view.document.clone())).collect(),
            foreign: foreign.into_values().collect(),
        });
        written.extend(in_batch);
    }
    Ok(ImportPlan { moved, batches })
}

/// What admission requires a document's referents to be. A hand-off cites its documents on its
/// source side only (`admission::references`), so one from another mind cites nothing here.
fn cited(document: &PipelineDocument, receiver: &Slug) -> Vec<(crate::CitationRole, PipelineRef)> {
    match document {
        PipelineDocument::HandOff(hand_off) if hand_off.from_instance != *receiver => Vec::new(),
        _ => citations(document),
    }
}

#[cfg(test)]
mod tests {
    use eureka_pipeline::{PipelineKind as K, pipeline_key};

    use super::*;
    use crate::fixtures::*;
    use crate::query::AdmissionFacts;
    use crate::receipt::{Faculty, PipelineProvenance};

    const RECEIVER: &str = "heimdall";

    fn view(document: PipelineDocument, ordinal: u64) -> PipelineDocumentView {
        let key = pipeline_key(&document).unwrap();
        let id = PipelineRef { kind: document.kind(), id: s(&key) };
        let faculty = if ordinal % 2 == 0 { Faculty::Hands } else { Faculty::Soul };
        let admission = AdmissionFacts {
            receipt_id: format!("receipt-{ordinal}"),
            admitted_at: format!("2026-11-08T16:{ordinal:02}:00Z"),
            provenance: PipelineProvenance { faculty, agent: s(&format!("agent-{ordinal}")), session: s(&format!("session-{ordinal}")), tool: s("huginn"), imported_from: None },
            ordinal,
        };
        PipelineDocumentView { id, document, admission, status: PipelineStatus::InForce }
    }

    fn moved(to: &str, ordinal: u64) -> PipelineDocumentView {
        view(resolution(r(K::Campaign, &id("campaign", "self")), ResolutionOutcome::Moved { to: slug(to) }), ordinal)
    }

    fn plan_of(views: Vec<PipelineDocumentView>) -> Result<ImportPlan, ImportFault> {
        plan(&slug(CAMPAIGN), &slug(INSTANCE), &slug(RECEIVER), views)
    }

    fn q(label: &str) -> PipelineDocument {
        question(label, &["A", "B"], "A")
    }

    #[test]
    fn plan_groups_by_source_ordinal_and_drops_only_the_moved() {
        let fu = follow_up("FU-1", r(K::Question, &id("question", "Q1")));
        // The Moved receipt also wrote a follow-up under the root; views arrive in no order.
        let views = vec![moved(RECEIVER, 9), view(q("Q2"), 3), view(fu.clone(), 9), view(q("Q1"), 3), view(q("Q3"), 5)];
        let plan = plan_of(views).unwrap();
        assert_eq!(plan.moved.id.0, pipeline_key(&resolution(r(K::Campaign, &id("campaign", "self")), ResolutionOutcome::Moved { to: slug(RECEIVER) })).unwrap());
        let shape: Vec<(u64, Vec<String>)> =
            plan.batches.iter().map(|batch| (batch.source.ordinal, batch.documents.iter().map(|(id, _)| id.id.0.clone()).collect())).collect();
        assert_eq!(
            shape,
            vec![
                (3, vec![id("question", "Q1"), id("question", "Q2")]),
                (5, vec![id("question", "Q3")]),
                (9, vec![pipeline_key(&fu).unwrap()]),
            ],
            "ascending ordinals, keys in order, the follow-up of the Moved receipt replays and the Moved does not"
        );
        let nine = &plan.batches[2].source;
        assert_eq!(
            nine,
            &ImportSource {
                instance: slug(INSTANCE),
                receipt_id: s("receipt-9"),
                ordinal: 9,
                committed_at: s("2026-11-08T16:09:00Z"),
                faculty: Faculty::Soul,
                agent: s("agent-9"),
                session: s("session-9"),
                tool: s("huginn"),
                external: vec![],
            }
        );
        assert!(plan.batches.iter().all(|batch| batch.foreign.is_empty()));
    }

    #[test]
    fn plan_refuses_unmoved_moved_elsewhere_and_self() {
        let held = vec![view(q("Q1"), 3)];
        assert_eq!(plan_of(held.clone()), Err(ImportFault::NotMoved));
        let mut withdrawn_move = moved(RECEIVER, 4);
        withdrawn_move.status = PipelineStatus::Resolved {
            resolution: r(K::Resolution, &id("resolution", "x")),
            record: match resolution(r(K::Campaign, &id("campaign", "self")), withdrawn()) {
                PipelineDocument::Resolution(record) => record,
                _ => unreachable!(),
            },
        };
        assert_eq!(plan_of([held.clone(), vec![withdrawn_move]].concat()), Err(ImportFault::NotMoved), "only an in-force Moved moves a campaign");
        let elsewhere = [held.clone(), vec![moved("kiko", 4)]].concat();
        assert_eq!(plan_of(elsewhere), Err(ImportFault::MovedElsewhere { to: slug("kiko") }));
        let to_source = [held.clone(), vec![moved(RECEIVER, 4)]].concat();
        assert_eq!(plan(&slug(CAMPAIGN), &slug(RECEIVER), &slug(RECEIVER), to_source), Err(ImportFault::SourceIsReceiver));
        let other_root = view(
            resolution(r(K::Campaign, &format!("elsewhere:campaign:self")), ResolutionOutcome::Moved { to: slug(RECEIVER) }),
            4,
        );
        assert_eq!(plan_of([held, vec![other_root]].concat()), Err(ImportFault::NotMoved), "another root's move does not move this one");
    }

    #[test]
    fn plan_refuses_an_oversized_receipt_before_anything() {
        let mut views = vec![moved(RECEIVER, 100), view(q("Q0"), 2)];
        views.extend((0..=BATCH_MAX).map(|n| view(q(&format!("B{n}")), 7)));
        assert_eq!(plan_of(views.clone()), Err(ImportFault::BatchTooLarge { ordinal: 7, documents: BATCH_MAX + 1 }));
        views.pop();
        let plan = plan_of(views).unwrap();
        assert_eq!(plan.batches[1].documents.len(), BATCH_MAX, "exactly one batch's worth replays");
    }

    #[test]
    fn plan_splits_foreign_from_in_root_referents() {
        let q1 = r(K::Question, &id("question", "Q1"));
        let abroad = r(K::Ruling, "elsewhere:ruling:R9");
        let answered = |question: &PipelineRef, by: &PipelineRef| resolution(question.clone(), ResolutionOutcome::Answered { by: by.clone() });
        let views = vec![
            moved(RECEIVER, 20),
            view(q("Q1"), 3),
            view(answered(&q1, &abroad), 4),
            view(follow_up("FU-1", q1.clone()), 5),
        ];
        let plan = plan_of(views.clone()).unwrap();
        assert_eq!(plan.batches[1].foreign, vec![abroad.clone()], "another root's ruling is foreign");
        assert!(plan.batches[0].foreign.is_empty() && plan.batches[2].foreign.is_empty(), "an in-root referent of an earlier receipt is not foreign");

        // The same follow-up before the question it cites: no earlier receipt wrote it.
        let backwards = vec![moved(RECEIVER, 20), view(follow_up("FU-1", q1.clone()), 2), view(q("Q1"), 3)];
        assert_eq!(plan_of(backwards), Err(ImportFault::Unordered { ordinal: 2, id: s(&q1.id.0) }));

        // One receipt carrying both sides is ordered by being whole.
        let together = vec![moved(RECEIVER, 20), view(follow_up("FU-1", q1.clone()), 3), view(q("Q1"), 3)];
        assert!(plan_of(together).is_ok());

        // More foreign referents than an import may declare.
        let many = (0..=IMPORT_EXTERNAL_MAX).map(|n| format!("elsewhere:ruling:R{n}")).collect::<Vec<_>>();
        let mut spec = cut_spec("1", 1);
        spec.rulings = many.iter().map(|key| s(key)).collect();
        let mut views = vec![moved(RECEIVER, 20), view(PipelineDocument::CutSpec(spec.clone()), 6)];
        assert_eq!(plan_of(views.clone()), Err(ImportFault::ExternalsTooMany { ordinal: 6, count: IMPORT_EXTERNAL_MAX + 1 }));
        spec.rulings.pop();
        views[1] = view(PipelineDocument::CutSpec(spec), 6);
        assert_eq!(plan_of(views).unwrap().batches[0].foreign.len(), IMPORT_EXTERNAL_MAX, "the limit itself is declarable");
    }

    #[test]
    fn plan_ignores_a_hand_off_cited_from_another_mind() {
        let q9 = id("question", "Q9");
        let from_libby = view(hand_off("libby", RECEIVER, REPO, &[&q9]), 3);
        assert!(plan_of(vec![moved(RECEIVER, 20), from_libby]).unwrap().batches[0].foreign.is_empty(), "the receiving side cites nothing");
        let from_receiver = view(hand_off(RECEIVER, "libby", REPO, &[&q9]), 3);
        assert_eq!(plan_of(vec![moved(RECEIVER, 20), from_receiver]), Err(ImportFault::Unordered { ordinal: 3, id: s(&q9) }));
    }
}
