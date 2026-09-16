//! The read side of a mind: a document with the facts of its admission and
//! its status, and the four queries over them.
//!
//! Nothing here is stored. Status is derived per call through the same
//! `docs::Docs` the admission rules use, so a withdrawn resolution reopens its
//! subject for a reader exactly when it reopens it for a rule. The facts come
//! from the commit receipts, from their `writes` and from nothing else: a
//! document's `admitted_at` is the `now` its batch was admitted at
//! (`committed_at`), never the store's `stored_at` stamp. A pipeline document
//! in the image that no receipt wrote is an integrity fault and refuses; the
//! store has one writer and the receipt is its proof.
//!
//! `semantic` is accepted by the type and refused typed until Cut 11 wires the
//! index. No clock, no store handle, no network.

use std::collections::BTreeMap;

use epiphany_pipeline::{
    Label, Line, OrgRepo, PipelineDocument, PipelineKind, PipelineRef, PipelineResolution, Short, Slug,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::docs::{Docs, Held};
use crate::mind::Mind;
use crate::receipt::{Faculty, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The most views one query answers with. A query that matched more says so in
/// `matched`; the cap never truncates silently.
pub const QUERY_LIMIT_MAX: usize = 200;

/// What admission recorded about a document when it landed: the receipt that
/// wrote it, the `now` that receipt was taken at, and who asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdmissionFacts {
    pub receipt_id: String,
    pub admitted_at: String,
    pub provenance: PipelineProvenance,
}

/// Whether a document still stands, and if not, what closed it. Derived at
/// read time; no document carries a status field and no admission writes one.
/// The whole closing record travels, not only its outcome, because a reader
/// rehydrating wants why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelineStatus {
    InForce,
    Resolved { resolution: PipelineRef, record: PipelineResolution },
}

/// One document as a reader gets it: its id, the document, the facts of its
/// admission and its status. The id travels with the document because a
/// `PipelineDocument` does not carry its key and a client must never derive
/// one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineDocumentView {
    pub id: PipelineRef,
    pub document: PipelineDocument,
    pub admission: AdmissionFacts,
    pub status: PipelineStatus,
}

/// A semantic query, accepted by the type and refused until Cut 11 owns the
/// index. Cut 11 also owns which fields of each kind are text, so no substring
/// filter lives here to become a second answer to that question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticQuery {
    pub text: Line,
    pub top_k: u32,
}

/// The filters over one mind. Each is one rule, and an unset filter matches
/// everything. The mind is the instance, so there is no `instance` field: the
/// daemon picks the mind before it calls here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineQuery {
    /// The key's root segment: a campaign slug, or an instance for
    /// stewardships, hand-offs and the identity document.
    pub campaign: Option<Slug>,
    pub repo: Option<OrgRepo>,
    pub cut: Option<Label>,
    /// Empty matches every kind.
    pub kinds: Vec<PipelineKind>,
    pub in_force: Option<bool>,
    /// Attribution, ruling 18: it filters, it grants nothing.
    pub faculty: Option<Faculty>,
    /// Exclusive, compared as a string against `admitted_at`.
    pub admitted_after: Option<Short>,
    /// Exclusive, compared as a string against `admitted_at`.
    pub admitted_before: Option<Short>,
    /// `None` is `QUERY_LIMIT_MAX`; anything else is clamped into
    /// `1..=QUERY_LIMIT_MAX`.
    pub limit: Option<u32>,
    pub semantic: Option<SemanticQuery>,
}

/// One page of a query, with the number of documents that matched before the
/// cap, so a short page is never mistaken for the whole answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineQueryPage {
    pub items: Vec<PipelineDocumentView>,
    pub matched: u32,
}

/// One campaign's open work, derived from what is in force and what cites
/// what. Uncapped: the open set of a campaign is bounded by the campaign, and
/// truncating it would hide work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineOpenItems {
    pub questions: Vec<PipelineDocumentView>,
    pub findings: Vec<PipelineDocumentView>,
    pub follow_ups: Vec<PipelineDocumentView>,
    pub specs_without_report: Vec<PipelineDocumentView>,
    pub reports_without_verdict: Vec<PipelineDocumentView>,
}

/// What a history is a history of: every resolution one subject ever had, or
/// every assignment of one repo to this mind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HistoryScope {
    Subject(PipelineRef),
    Repo(OrgRepo),
}

/// The admission facts of every stored document, built once per read call from
/// the receipts. `writes` only: a document a later batch cited as a strong read
/// keeps the receipt that wrote it. A10 admits one write per identity, so two
/// receipts naming one document is an integrity fault, not a choice to make.
pub(crate) struct AdmissionIndex(BTreeMap<(String, String), AdmissionFacts>);

impl AdmissionIndex {
    pub(crate) fn build<S: MindStore>(mind: &Mind<S>) -> Result<Self, MindRefusal> {
        let mut facts = BTreeMap::new();
        for receipt in mind.receipts()? {
            for write in receipt.writes.iter() {
                let identity = (write.document_type.clone(), write.document_key.clone());
                let landed = AdmissionFacts {
                    receipt_id: receipt.receipt_id.clone(),
                    admitted_at: receipt.committed_at.clone(),
                    provenance: receipt.provenance.clone(),
                };
                if facts.insert(identity, landed).is_some() {
                    return Err(integrity(&write.document_type, &write.document_key, "is written by two receipts"));
                }
            }
        }
        Ok(Self(facts))
    }

    fn of(&self, held: &Held) -> Result<AdmissionFacts, MindRefusal> {
        let type_id = held.kind.type_id();
        self.0
            .get(&(type_id.to_string(), held.key.clone()))
            .cloned()
            .ok_or_else(|| integrity(type_id, &held.key, "has no commit receipt"))
    }
}

fn integrity(type_id: &str, key: &str, fault: &str) -> MindRefusal {
    MindRefusal::Unavailable { detail: format!("document {type_id}/{key} {fault}") }
}

/// The root and the local of a key, by position in `<root>:<kind>:<local>`:
/// the leaf derives every key that way and its own test pins the three
/// segments. The only place the read side reads a key apart, and it reads no
/// sequence, no label and no kind out of one.
fn root_and_local(key: &str) -> (&str, &str) {
    let mut segments = key.split(':');
    let root = segments.next().unwrap_or_default();
    let local = segments.nth(1).unwrap_or_default();
    (root, local)
}

/// Which documents carry a repo, and where: a campaign lists them, five kinds
/// name one, and the rest never match.
fn repo_matches(document: &PipelineDocument, repo: &OrgRepo) -> bool {
    use PipelineDocument as D;
    match document {
        D::Campaign(campaign) => campaign.repos.contains(repo),
        D::CutSpec(spec) => spec.repo == *repo,
        D::CutReport(report) => report.repo == *repo,
        D::FollowUp(follow_up) => follow_up.repo == *repo,
        D::Stewardship(stewardship) => stewardship.repo == *repo,
        D::HandOff(hand_off) => hand_off.repo == *repo,
        D::Target(_)
        | D::Question(_)
        | D::Ruling(_)
        | D::Verdict(_)
        | D::Finding(_)
        | D::Instance(_)
        | D::Resolution(_) => false,
    }
}

/// One read call's working set: the decoded image, the facts, and the mind the
/// two came from. Built once per call and thrown away with it; nothing here
/// outlives the answer.
struct Reader<'a, S: MindStore> {
    docs: Docs,
    facts: AdmissionIndex,
    mind: &'a Mind<S>,
}

impl<'a, S: MindStore> Reader<'a, S> {
    fn new(mind: &'a Mind<S>) -> Result<Self, MindRefusal> {
        Ok(Self { docs: Docs::from_image(mind.envelopes())?, facts: AdmissionIndex::build(mind)?, mind })
    }

    fn held(&self, kind: PipelineKind, key: &str) -> Option<&Held> {
        self.docs.image.iter().find(|held| held.kind == kind && held.key == key)
    }

    fn view_of(&self, held: &Held) -> Result<PipelineDocumentView, MindRefusal> {
        let admission = self.facts.of(held)?;
        let status = match self.docs.closing_resolution(held.kind, &held.key) {
            Some((key, record)) => PipelineStatus::Resolved {
                resolution: PipelineRef { kind: PipelineKind::Resolution, id: Short(key.to_string()) },
                record: record.clone(),
            },
            None => PipelineStatus::InForce,
        };
        Ok(PipelineDocumentView {
            id: PipelineRef { kind: held.kind, id: Short(held.key.clone()) },
            document: held.document.clone(),
            admission,
            status,
        })
    }

    /// The document a filter reads a resolution through: its subject, and its
    /// subject's subject when that is a resolution too (the Q19 cap stops the
    /// chain at two). A7 puts every subject in the image, and a resolution key
    /// is strictly longer than the id it resolves, so this walks down and
    /// stops. Anything that is not a resolution is its own base.
    fn base<'b>(&'b self, held: &'b Held) -> &'b Held {
        let mut current = held;
        while let PipelineDocument::Resolution(resolution) = &current.document {
            let Some(subject) = self.held(resolution.subject.kind, &resolution.subject.id.0) else {
                return current;
            };
            current = subject;
        }
        current
    }

    /// Every filter, each one rule, in one pass over one document.
    fn matches(&self, query: &PipelineQuery, held: &Held, view: &PipelineDocumentView) -> bool {
        if let Some(campaign) = &query.campaign
            && root_and_local(&held.key).0 != campaign.0
        {
            return false;
        }
        let base = self.base(held);
        if let Some(repo) = &query.repo
            && !repo_matches(&base.document, repo)
        {
            return false;
        }
        if let Some(cut) = &query.cut
            && !root_and_local(&base.key).1.starts_with(&format!("cut-{}.", cut.0))
        {
            return false;
        }
        if !query.kinds.is_empty() && !query.kinds.contains(&held.kind) {
            return false;
        }
        if let Some(in_force) = query.in_force
            && in_force != (view.status == PipelineStatus::InForce)
        {
            return false;
        }
        if let Some(faculty) = query.faculty
            && view.admission.provenance.faculty != faculty
        {
            return false;
        }
        if let Some(after) = &query.admitted_after
            && view.admission.admitted_at <= after.0
        {
            return false;
        }
        if let Some(before) = &query.admitted_before
            && view.admission.admitted_at >= before.0
        {
            return false;
        }
        true
    }

    /// Every view of the image, in order, refusing rather than skipping a
    /// document the receipts do not account for.
    fn views(&self) -> Result<Vec<PipelineDocumentView>, MindRefusal> {
        let mut views = Vec::with_capacity(self.docs.image.len());
        for held in &self.docs.image {
            views.push(self.view_of(held)?);
        }
        ordered(&mut views);
        Ok(views)
    }
}

/// `(admitted_at, id)` ascending. The batch decides the order and the key
/// breaks a tie inside one, which is root-first and so is not the image's
/// `(type, key)` order.
fn ordered(views: &mut [PipelineDocumentView]) {
    views.sort_by(|left, right| {
        (&left.admission.admitted_at, &left.id.id.0).cmp(&(&right.admission.admitted_at, &right.id.id.0))
    });
}

impl<S: MindStore> Mind<S> {
    /// One document with its admission facts and its derived status, or
    /// `None` when the mind does not hold it.
    ///
    /// The ref is validated first, through the leaf's own door: a kind and an
    /// id that disagree name no document any writer could have composed, and
    /// answering `None` over one would report a malformed reference as an
    /// absent document. The refusal is the leaf's, wrapped: this crate owns no
    /// grammar, and a second name for `InvalidFormat` would be a second
    /// vocabulary for one rule.
    pub fn view(&self, id: &PipelineRef) -> Result<Option<PipelineDocumentView>, MindRefusal> {
        id.validate_ref()?;
        let reader = Reader::new(self)?;
        reader.held(id.kind, &id.id.0).map(|held| reader.view_of(held)).transpose()
    }

    /// The documents matching every set filter, ordered and capped, with the
    /// count that matched before the cap.
    pub fn query(&self, query: &PipelineQuery) -> Result<PipelineQueryPage, MindRefusal> {
        if query.semantic.is_some() {
            return Err(MindRefusal::Unavailable {
                detail: "semantic query: the index is not wired (Cut 11)".into(),
            });
        }
        let reader = Reader::new(self)?;
        let mut items = Vec::new();
        for held in &reader.docs.image {
            let view = reader.view_of(held)?;
            if reader.matches(query, held, &view) {
                items.push(view);
            }
        }
        let matched = items.len() as u32;
        ordered(&mut items);
        let limit = query.limit.map_or(QUERY_LIMIT_MAX, |limit| (limit as usize).clamp(1, QUERY_LIMIT_MAX));
        items.truncate(limit);
        Ok(PipelineQueryPage { items, matched })
    }

    /// One campaign's open work: the questions, findings and follow-ups still
    /// in force, the in-force cut specs no report names, and the cut reports no
    /// verdict names.
    pub fn open_items(&self, campaign: &Slug) -> Result<PipelineOpenItems, MindRefusal> {
        let reader = Reader::new(self)?;
        let views = reader
            .views()?
            .into_iter()
            .filter(|view| root_and_local(&view.id.id.0).0 == campaign.0)
            .collect::<Vec<_>>();
        let of_kind = |kind: PipelineKind| views.iter().filter(move |view| view.id.kind == kind);
        let in_force = |kind: PipelineKind| {
            of_kind(kind).filter(|view| view.status == PipelineStatus::InForce).cloned().collect::<Vec<_>>()
        };
        let cited = |referent: fn(&PipelineDocument) -> Option<&str>| {
            views.iter().filter_map(|view| referent(&view.document)).collect::<Vec<_>>()
        };
        let reported = cited(|document| match document {
            PipelineDocument::CutReport(report) => Some(report.cut_spec.0.as_str()),
            _ => None,
        });
        let judged = cited(|document| match document {
            PipelineDocument::Verdict(verdict) => Some(verdict.cut_report.0.as_str()),
            _ => None,
        });
        Ok(PipelineOpenItems {
            questions: in_force(PipelineKind::Question),
            findings: in_force(PipelineKind::Finding),
            follow_ups: in_force(PipelineKind::FollowUp),
            specs_without_report: in_force(PipelineKind::CutSpec)
                .into_iter()
                .filter(|view| !reported.contains(&view.id.id.0.as_str()))
                .collect(),
            // A report is not resolvable, so standing is not a condition here.
            reports_without_verdict: of_kind(PipelineKind::CutReport)
                .filter(|view| !judged.contains(&view.id.id.0.as_str()))
                .cloned()
                .collect(),
        })
    }

    /// Every record of one scope, withdrawn ones included with their status
    /// and their reasons, in the scope's own sequence order. Uncapped: a
    /// scope's history is bounded by the scope.
    ///
    /// A subject scope is validated as `view`'s id is, and for the same
    /// reason: an empty history is the answer for a subject with no records,
    /// not for a ref that is no ref. A repo scope is an `OrgRepo`, which the
    /// type already holds to its own format.
    pub fn history(&self, scope: &HistoryScope) -> Result<Vec<PipelineDocumentView>, MindRefusal> {
        if let HistoryScope::Subject(subject) = scope {
            subject.validate_ref()?;
        }
        let reader = Reader::new(self)?;
        let (kind, mut records) = match scope {
            HistoryScope::Subject(subject) => (
                PipelineKind::Resolution,
                reader
                    .docs
                    .resolutions_of(subject)
                    .map(|(key, resolution)| (resolution.sequence, key.to_string()))
                    .collect::<Vec<_>>(),
            ),
            HistoryScope::Repo(repo) => (
                PipelineKind::Stewardship,
                reader
                    .docs
                    .assignments_of(reader.mind.instance(), repo)
                    .map(|(key, stewardship)| (stewardship.sequence, key.to_string()))
                    .collect::<Vec<_>>(),
            ),
        };
        // The sequence owns history order. `admitted_at` coincides in a mind
        // written in order, but it is not the rule.
        records.sort_by_key(|(sequence, _)| *sequence);
        let mut views = Vec::with_capacity(records.len());
        for (_, key) in records {
            let held = reader
                .held(kind, &key)
                .ok_or_else(|| integrity(kind.type_id(), &key, "is derived over the image but not held in it"))?;
            views.push(reader.view_of(held)?);
        }
        Ok(views)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::fixtures::*;
    use crate::mind::schema_cache;
    use crate::store::test_stores::MemoryStore;
    use chrono::{TimeZone, Utc};
    use epiphany_pipeline::{
        ClaimOutcome, Date, FindingConfidence, PipelineDocument as D, PipelineKind as K, PipelineRefusal,
        ResolutionOutcome,
    };

    fn view(mind: &Mind<MemoryStore>, kind: K, key: &str) -> PipelineDocumentView {
        mind.view(&r(kind, key)).unwrap().unwrap_or_else(|| panic!("{key} is not held"))
    }

    fn ids(views: &[PipelineDocumentView]) -> Vec<String> {
        views.iter().map(|view| view.id.id.0.clone()).collect()
    }

    /// The refusal a door answers for a ref whose kind is not its id's: the
    /// leaf's own `InvalidFormat`, at the field the leaf names it, wrapped.
    fn malformed(id: &str) -> MindRefusal {
        MindRefusal::Document(PipelineRefusal::InvalidFormat { field: "ref.id".into(), value: id.into() })
    }

    /// A ruling that answers a question, which is what derives its
    /// `Answered` resolution.
    fn answering(label: &str, question: &str) -> D {
        let mut ruling = ruling(label);
        ruling.answers = Some(s(question));
        ruling.choice = Some(l("A"));
        D::Ruling(ruling)
    }

    /// A question of another campaign, beside that campaign's own record, so
    /// no filter can pass by matching everything.
    fn elsewhere(label: &str) -> Vec<D> {
        let D::Campaign(mut campaign) = campaign(&[REPO]) else { panic!() };
        campaign.slug = slug("other-campaign");
        let mut question = question(label, &["A", "B"], "A");
        let D::Question(value) = &mut question else { panic!() };
        value.campaign = slug("other-campaign");
        vec![D::Campaign(campaign), question]
    }

    /// R-A: the status a reader sees is the recursive derivation admission
    /// rules with, reopen case included. A test pinning the non-recursive
    /// form would pin the defect Cut 6d closed.
    #[test]
    fn status_is_the_derivation_admission_uses() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        assert_eq!(view(&mind, K::Question, &q1).status, PipelineStatus::InForce);

        // The ruling answers it; what closes it is the resolution admission
        // derived, and the view names that resolution and carries it whole.
        let closed = id("resolution", "question.Q1.n1");
        committed(admit(&mut mind, vec![answering("R1", &q1)]));
        let PipelineStatus::Resolved { resolution: closing, record } = view(&mind, K::Question, &q1).status else {
            panic!("the answered question is resolved")
        };
        assert_eq!(closing, r(K::Resolution, &closed));
        assert!(matches!(&record.outcome, ResolutionOutcome::Answered { by } if by.id.0 == id("ruling", "R1")));

        // Withdrawing that resolution reopens the question, and the withdrawn
        // resolution is a document with a status of its own: this is how
        // R-B's "with their reasons" reaches a reader.
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &closed), withdrawn())]));
        assert_eq!(view(&mind, K::Question, &q1).status, PipelineStatus::InForce);
        let PipelineStatus::Resolved { resolution: closing, record } = view(&mind, K::Resolution, &closed).status else {
            panic!("the withdrawn resolution is resolved by its withdrawal")
        };
        assert_eq!(closing, r(K::Resolution, &id("resolution", "resolution.question.Q1.n1.n1")));
        assert!(matches!(&record.outcome, ResolutionOutcome::Withdrawn { reason } if reason.0 == "moot"));

        // The next ruling closes it again, at the next sequence.
        committed(admit(&mut mind, vec![answering("R2", &q1)]));
        let PipelineStatus::Resolved { resolution: closing, .. } = view(&mind, K::Question, &q1).status else { panic!() };
        assert_eq!(closing, r(K::Resolution, &id("resolution", "question.Q1.n2")));

        // A ruling is not resolved by the withdrawal of the resolution it
        // derived, and the kinds the matrix makes unresolvable are in force by
        // the same derivation, with no special case for them.
        assert_eq!(view(&mind, K::Ruling, &id("ruling", "R1")).status, PipelineStatus::InForce);
        assert_eq!(view(&mind, K::Campaign, &id("campaign", "self")).status, PipelineStatus::InForce);
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
        committed(admit(&mut mind, vec![D::CutReport(cut_report("1", 1))]));
        committed(admit(&mut mind, vec![
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&id("finding", "cut-1.s1.F1")], Some("P1"), &["M1"])]),
            D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed)),
        ]));
        assert_eq!(view(&mind, K::Verdict, &id("verdict", "cut-1.s1")).status, PipelineStatus::InForce);
    }

    /// Ruling 1's read half: the grammar lives in the leaf, and both doors
    /// that take a ref ask it before they look. A ref that is no ref is
    /// refused; a ref that is well formed and names nothing is still the
    /// empty answer, so the door refuses malformation and not absence.
    #[test]
    fn a_ref_whose_kind_and_id_disagree_is_refused_by_both_doors() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![answering("R1", &q1)]));

        // The id names a question this mind holds and a resolution it derived;
        // read as a ruling it names neither, and is not a reference at all.
        let disagreeing = r(K::Ruling, &q1);
        assert_eq!(mind.view(&disagreeing), Err(malformed(&q1)));
        assert_eq!(mind.history(&HistoryScope::Subject(disagreeing)), Err(malformed(&q1)));

        // A local no writer composes is the same refusal, so the door is the
        // whole grammar and not the kind segment alone. The refusal's `value`
        // is the failing part, here the empty part after the trailing dot,
        // and not the whole id.
        let dotted = r(K::Question, &format!("{q1}."));
        assert_eq!(mind.view(&dotted), Err(malformed("")));
        assert_eq!(mind.history(&HistoryScope::Subject(dotted)), Err(malformed("")));

        // Absence is not malformation: a well-formed id of a kind this mind
        // does not hold answers as it always did.
        let absent = id("question", "Q9");
        assert_eq!(mind.view(&r(K::Question, &absent)), Ok(None));
        assert_eq!(mind.history(&HistoryScope::Subject(r(K::Question, &absent))), Ok(Vec::new()));
    }

    /// R-B and D7: a subject's resolutions are a first-class view, withdrawn
    /// ones included with their status and their reasons, in sequence order.
    #[test]
    fn a_subjects_history_lists_every_resolution_with_its_status_and_receipt() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        // Ten reopen cycles: each ruling answers, each withdrawal but the last
        // reopens.
        for cycle in 1..=10u32 {
            committed(admit(&mut mind, vec![answering(&format!("R{cycle}"), &q1)]));
            if cycle < 10 {
                let closed = id("resolution", &format!("question.Q1.n{cycle}"));
                committed(admit(&mut mind, vec![resolution(r(K::Resolution, &closed), withdrawn())]));
            }
        }

        let history = mind.history(&HistoryScope::Subject(r(K::Question, &q1))).unwrap();
        assert_eq!(
            ids(&history),
            (1..=10).map(|n| id("resolution", &format!("question.Q1.n{n}"))).collect::<Vec<_>>(),
            "sequence order, so n10 lands after n9 and not after n1 as the key string would have it"
        );
        for (index, entry) in history.iter().enumerate() {
            let sequence = index as u32 + 1;
            if sequence == 10 {
                assert_eq!(entry.status, PipelineStatus::InForce, "the standing closure");
                continue;
            }
            let PipelineStatus::Resolved { resolution, record } = &entry.status else { panic!("n{sequence}") };
            let withdrawal = id("resolution", &format!("resolution.question.Q1.n{sequence}.n1"));
            assert_eq!(*resolution, r(K::Resolution, &withdrawal));
            assert!(matches!(&record.outcome, ResolutionOutcome::Withdrawn { reason } if reason.0 == "moot"));
        }

        // A withdrawal's subject is the resolution, not the question, so it is
        // in the resolution's history and not the question's.
        let n1 = id("resolution", "question.Q1.n1");
        assert_eq!(ids(&mind.history(&HistoryScope::Subject(r(K::Resolution, &n1))).unwrap()), vec![
            id("resolution", "resolution.question.Q1.n1.n1")
        ]);

        // A subject whose kind disagrees with its id is no subject at all, and
        // the door says so: the id names ten resolutions, and a mind that
        // answered an empty history over it would report a malformed reference
        // as a subject with no records.
        assert_eq!(
            mind.history(&HistoryScope::Subject(r(K::Ruling, &q1))),
            Err(malformed(&q1)),
            "a malformed scope is refused, not answered over"
        );

        // History lands one record per batch (Cut 12's import constraint), so
        // ten records carry ten receipts.
        let receipts = history.iter().map(|entry| entry.admission.receipt_id.clone()).collect::<BTreeSet<_>>();
        assert_eq!(receipts.len(), 10);
    }

    /// D7's other scope: every assignment of one repo to this mind, the
    /// withdrawn ones with the hand-off that withdrew them.
    #[test]
    fn a_repos_stewardship_history_on_a_mind_lists_every_assignment() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![stewardship(INSTANCE, OTHER_REPO)]));
        let away = format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.GameCult_-Epiphany.2026-09-16");
        committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let D::HandOff(mut back) = hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) else { panic!() };
        back.handed_on = Date("2026-09-17".into());
        committed(admit(&mut mind, vec![D::HandOff(back)]));

        let history = mind.history(&HistoryScope::Repo(repo(REPO))).unwrap();
        assert_eq!(ids(&history), vec![
            format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1"),
            format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n2"),
        ]);
        let PipelineStatus::Resolved { record, .. } = &history[0].status else { panic!("handed away") };
        assert!(
            matches!(&record.outcome, ResolutionOutcome::Withdrawn { reason } if reason.0 == away),
            "the withdrawal names the hand-off that caused it"
        );
        assert_eq!(history[1].status, PipelineStatus::InForce);
        let (D::Stewardship(first), D::Stewardship(second)) = (&history[0].document, &history[1].document) else {
            panic!()
        };
        assert_eq!(first.assigned_on, date());
        assert_eq!(second.assigned_on, Date("2026-09-17".into()), "the assignment carries the hand-off's day");

        // Another repo's assignments are another history; the scope is
        // `(this mind, this repo)` and nothing else.
        assert_eq!(ids(&mind.history(&HistoryScope::Repo(repo(OTHER_REPO))).unwrap()), vec![
            format!("{INSTANCE}:stewardship:GameCult_-Huginn.n1")
        ]);
    }

    /// D3: the facts come from the one receipt that wrote the document, from
    /// `writes` alone, and a row no receipt wrote refuses rather than showing.
    #[test]
    fn views_join_admission_facts_from_the_receipt_that_wrote_them() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let q1 = id("question", "Q1");
        let asked = Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap();
        let (question_receipt, _) = committed(admit_at(&mut mind, vec![question("Q1", &["A", "B"], "A")], asked));
        let facts = view(&mind, K::Question, &q1).admission;
        assert_eq!(facts.receipt_id, question_receipt);
        assert_eq!(facts.admitted_at, "2026-09-16T09:00:00Z", "the `now` admission was passed");
        assert_eq!(facts.provenance, provenance(Faculty::Hands));

        // The ruling's batch writes the ruling and derives the resolution, so
        // both carry its receipt. The question it cites is a strong read, and a
        // strong read is not a write: it keeps the receipt that wrote it.
        let ruled = Utc.with_ymd_and_hms(2026, 9, 16, 10, 0, 0).unwrap();
        let (ruling_receipt, _) = committed(admit_at(&mut mind, vec![answering("R1", &q1)], ruled));
        assert_eq!(view(&mind, K::Ruling, &id("ruling", "R1")).admission.receipt_id, ruling_receipt);
        let derived = view(&mind, K::Resolution, &id("resolution", "question.Q1.n1")).admission;
        assert_eq!(derived.receipt_id, ruling_receipt, "a derived write carries the batch's receipt");
        assert_eq!(derived.admitted_at, "2026-09-16T10:00:00Z");
        assert_eq!(view(&mind, K::Question, &q1).admission.receipt_id, question_receipt, "the cited document keeps its first");

        // A pipeline document the image holds and no receipt names is an
        // integrity fault: the store has one writer and the receipt is its
        // proof, so it refuses from `view` and `query` alike, never a skip.
        let planted = MemoryStore::new();
        for row in store.rows() {
            planted.plant(row);
        }
        planted.plant(prepare(&question("Q9", &["A", "B"], "A")));
        let orphaned = opened(planted, INSTANCE);
        let orphan = id("question", "Q9");
        let detail = format!("document {}/{orphan} has no commit receipt", K::Question.type_id());
        assert_eq!(
            orphaned.view(&r(K::Question, &orphan)).err(),
            Some(MindRefusal::Unavailable { detail: detail.clone() })
        );
        assert_eq!(orphaned.query(&PipelineQuery::default()).err(), Some(MindRefusal::Unavailable { detail }));

        // The other half of the same rule: A10 admits one write per identity,
        // so two receipts naming one document is a store this organ cannot
        // read, not a choice of which receipt to believe.
        let doubled = MemoryStore::new();
        for row in store.rows() {
            doubled.plant(row);
        }
        let forged = crate::receipt::candidate(
            &slug(INSTANCE),
            provenance(Faculty::Soul),
            &[],
            &[prepare(&question("Q1", &["A", "B"], "A")), prepare(&question("Q8", &["A", "B"], "A"))],
            now(),
        )
        .unwrap();
        doubled.plant(schema_cache().unwrap().prepare_entry_named(&forged.receipt_id, &forged).unwrap().0);
        assert_eq!(
            opened(doubled, INSTANCE).query(&PipelineQuery::default()).err(),
            Some(MindRefusal::Unavailable {
                detail: format!("document {}/{q1} is written by two receipts", K::Question.type_id()),
            })
        );
    }

    /// R-C: `stored_at` is the store's stamp and no derivation reads it.
    /// Scramble every stamp and the answers do not move.
    #[test]
    fn views_read_admitted_at_from_the_receipt_not_stored_at() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let q1 = id("question", "Q1");
        committed(admit_at(
            &mut mind,
            vec![question("Q1", &["A", "B"], "A")],
            Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap(),
        ));
        committed(admit_at(&mut mind, vec![answering("R1", &q1)], Utc.with_ymd_and_hms(2026, 9, 17, 9, 0, 0).unwrap()));

        let rows = store.rows();
        let reopened = |stamps: Vec<String>| {
            let copy = MemoryStore::new();
            for (row, stored_at) in rows.iter().zip(stamps) {
                let mut row = row.clone();
                row.stored_at = stored_at;
                copy.plant(row);
            }
            opened(copy, INSTANCE)
        };
        let scope = HistoryScope::Subject(r(K::Question, &q1));
        let expected = (mind.query(&PipelineQuery::default()).unwrap(), mind.history(&scope).unwrap());
        let constant = vec!["2000-01-01T00:00:00Z".to_string(); rows.len()];
        let reversed = rows.iter().rev().map(|row| row.stored_at.clone()).collect::<Vec<_>>();
        for other in [reopened(constant), reopened(reversed)] {
            assert_eq!(other.query(&PipelineQuery::default()).unwrap(), expected.0);
            assert_eq!(other.history(&scope).unwrap(), expected.1);
        }
    }

    /// D5: one assertion per filter, each selecting by its own field alone.
    #[test]
    fn query_filters_each_select_by_one_field() {
        let mut mind = seeded();
        committed(admit(&mut mind, elsewhere("Q1")));
        committed(admit(&mut mind, vec![
            question("Q1", &["A", "B"], "A"),
            D::Ruling(ruling("R1")),
            D::CutSpec(cut_spec("1", 1)),
            D::CutSpec(cut_spec("9", 1)),
            D::CutSpec(cut_spec("10", 1)),
            follow_up("FU-1", r(K::Question, &id("question", "Q1"))),
        ]));
        committed(admit(&mut mind, vec![D::CutReport(cut_report("9", 1)), D::CutReport(cut_report("10", 1))]));
        committed(admit(&mut mind, vec![
            verdict("9", 1, vec![claim(ClaimOutcome::Falsified, &[&id("finding", "cut-9.s1.F1")], Some("P1"), &["M1"])]),
            D::Finding(finding("9", 1, "F1", FindingConfidence::Confirmed)),
        ]));
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("9", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-9.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-9.r2"))])),
        ]));
        // A resolution of a resolution, so a filter reading through the base
        // has two steps to walk: the cut-10 spec is withdrawn and its
        // withdrawal is itself withdrawn, which puts the spec back in force.
        committed(admit(&mut mind, vec![resolution(r(K::CutSpec, &id("cut_spec", "cut-10.r1")), withdrawn())]));
        committed(admit(&mut mind, vec![resolution(
            r(K::Resolution, &id("resolution", "cut_spec.cut-10.r1.n1")),
            withdrawn(),
        )]));
        committed(admit_as(&mut mind, Faculty::Soul, vec![D::Ruling(ruling("R2"))]));
        // One batch at a later clock, so the window filters have a boundary.
        let later = Utc.with_ymd_and_hms(2026, 9, 17, 12, 0, 0).unwrap();
        committed(admit_at(&mut mind, vec![question_n(2)], later));

        // Sorted, because the order is the next test's rule and not this one's.
        let matching = |query: PipelineQuery| {
            let mut matched = ids(&mind.query(&query).unwrap().items);
            matched.sort();
            matched
        };
        let supersession = id("resolution", "cut_spec.cut-9.r1.n1");

        // `campaign`: the key's root, which is a campaign slug for most kinds
        // and an instance for the ones that hang off one.
        let eureka = matching(PipelineQuery { campaign: Some(slug(CAMPAIGN)), ..Default::default() });
        assert!(eureka.contains(&id("question", "Q1")));
        assert!(eureka.contains(&supersession), "a resolution is keyed inside its subject's root");
        assert!(!eureka.contains(&"other-campaign:question:Q1".to_string()));
        assert_eq!(matching(PipelineQuery { campaign: Some(slug(INSTANCE)), ..Default::default() }), vec![
            format!("{INSTANCE}:instance:self"),
            format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1"),
        ]);

        // `repo`: a campaign lists its repos, five kinds name one, and a
        // resolution matches when its base does.
        let epiphany = matching(PipelineQuery { repo: Some(repo(REPO)), ..Default::default() });
        for expected in [
            id("campaign", "self"),
            "other-campaign:campaign:self".to_string(),
            id("cut_spec", "cut-9.r1"),
            id("cut_report", "cut-9.h1"),
            id("follow_up", "FU-1"),
            format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1"),
            supersession.clone(),
        ] {
            assert!(epiphany.contains(&expected), "{expected}");
        }
        assert!(!epiphany.contains(&id("question", "Q1")), "a question carries no repo");

        // `cut`: the local's `cut-<label>.` prefix, read through the base.
        assert_eq!(matching(PipelineQuery { cut: Some(l("9")), ..Default::default() }), vec![
            id("cut_report", "cut-9.h1"),
            id("cut_spec", "cut-9.r1"),
            id("cut_spec", "cut-9.r2"),
            id("finding", "cut-9.s1.F1"),
            supersession.clone(),
            id("verdict", "cut-9.s1"),
        ]);
        assert_eq!(matching(PipelineQuery { cut: Some(l("10")), ..Default::default() }), vec![
            id("cut_report", "cut-10.h1"),
            id("cut_spec", "cut-10.r1"),
            id("resolution", "cut_spec.cut-10.r1.n1"),
            id("resolution", "resolution.cut_spec.cut-10.r1.n1.n1"),
        ]);
        // The prefix ends at the dot, so cut-1 is not every cut that starts
        // with its label.
        assert_eq!(matching(PipelineQuery { cut: Some(l("1")), ..Default::default() }), vec![id("cut_spec", "cut-1.r1")]);

        // `kinds`, empty for every kind.
        assert_eq!(matching(PipelineQuery { kinds: vec![K::Question], ..Default::default() }), vec![
            id("question", "Q1"),
            id("question", "Q2"),
            "other-campaign:question:Q1".to_string(),
        ]);

        // `in_force`, both ways: the superseded revision is the one thing this
        // mind has closed.
        assert_eq!(matching(PipelineQuery { in_force: Some(false), ..Default::default() }), vec![
            id("cut_spec", "cut-9.r1"),
            id("resolution", "cut_spec.cut-10.r1.n1"),
        ]);
        let standing = matching(PipelineQuery { in_force: Some(true), ..Default::default() });
        assert!(!standing.contains(&id("cut_spec", "cut-9.r1")));
        assert!(standing.contains(&id("cut_spec", "cut-9.r2")));

        // `faculty`: attribution filters, and grants nothing (ruling 18).
        assert_eq!(matching(PipelineQuery { faculty: Some(Faculty::Soul), ..Default::default() }), vec![id("ruling", "R2")]);
        assert!(!matching(PipelineQuery { faculty: Some(Faculty::Hands), ..Default::default() }).contains(&id("ruling", "R2")));

        // The window, exclusive at the exact boundary on both sides.
        let boundary = s("2026-09-17T12:00:00Z");
        let opened_at = s("2026-09-16T12:00:00Z");
        assert_eq!(matching(PipelineQuery { admitted_after: Some(opened_at.clone()), ..Default::default() }), vec![
            id("question", "Q2")
        ]);
        assert!(matching(PipelineQuery { admitted_after: Some(boundary.clone()), ..Default::default() }).is_empty());
        assert!(matching(PipelineQuery { admitted_before: Some(opened_at), ..Default::default() }).is_empty());
        assert!(!matching(PipelineQuery { admitted_before: Some(boundary), ..Default::default() }).contains(&id("question", "Q2")));
    }

    /// D5's order and cap: `(admitted_at, id)` ascending, at most 200, and the
    /// count that matched before the cap so a short page is visible as one.
    #[test]
    fn query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count() {
        // The seed is one batch at one `now`, so it falls to id order, which
        // is root-first and so is not the image's `(type, key)` order.
        let seeded = seeded();
        assert_eq!(ids(&seeded.query(&PipelineQuery::default()).unwrap().items), vec![
            id("campaign", "self"),
            id("target", "r1"),
            format!("{INSTANCE}:instance:self"),
            format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1"),
        ]);

        // 201 questions over four batches at rising `now`s, with Q1 -- the id
        // that sorts first -- in the last batch.
        let mut mind = crate::fixtures::seeded();
        let batches: [Vec<u32>; 4] = [
            (2..=65).collect(),
            (66..=129).collect(),
            (130..=193).collect(),
            std::iter::once(1).chain(194..=201).collect(),
        ];
        for (index, labels) in batches.iter().enumerate() {
            let at = Utc.with_ymd_and_hms(2026, 9, 17, 12 + index as u32, 0, 0).unwrap();
            committed(admit_at(&mut mind, labels.iter().map(|label| question_n(*label)).collect(), at));
        }

        let questions = PipelineQuery { kinds: vec![K::Question], ..Default::default() };
        let page = mind.query(&PipelineQuery { limit: Some(500), ..questions.clone() }).unwrap();
        assert_eq!(page.matched, 201);
        assert_eq!(page.items.len(), 200, "the cap, spelled out rather than read from the constant under test");
        assert_eq!(page.items[0].admission.admitted_at, "2026-09-17T12:00:00Z", "the earliest batch first");
        assert_eq!(page.items[0].id.id.0, id("question", "Q10"), "and inside it, key order");
        assert_ne!(page.items[0].id.id.0, id("question", "Q1"), "Q1 sorts first by id and is admitted last");
        let last = page
            .items
            .iter()
            .filter(|view| view.admission.admitted_at == "2026-09-17T15:00:00Z")
            .map(|view| view.id.id.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            last,
            std::iter::once(1).chain(194..=200).map(|label| id("question", &format!("Q{label}"))).collect::<Vec<_>>(),
            "the last batch in key order, with the one the cap drops missing from its end"
        );
        assert_eq!(page.matched as usize - page.items.len(), 1);

        for (limit, expected) in [(None, 200), (Some(500), 200), (Some(0), 1), (Some(5), 5)] {
            let page = mind.query(&PipelineQuery { limit, ..questions.clone() }).unwrap();
            assert_eq!(page.items.len(), expected, "limit {limit:?}");
            assert_eq!(page.matched, 201, "the count is taken before the cap");
        }
    }

    /// D6: open work is derived from what is in force and what cites what,
    /// never maintained.
    #[test]
    fn open_items_are_derived_from_in_force_and_citation() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        let q2 = id("question", "Q2");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), question("Q2", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![answering("R1", &q2)]));

        // cut-9: a superseded r1 with a report of its own, and an r2 with none.
        // The citation is the exact spec and not its cut, so r2 is open work
        // while the cut it belongs to has been reported.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("9", 1))]));
        committed(admit(&mut mind, vec![D::CutReport(cut_report("9", 1))]));
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("9", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-9.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-9.r2"))])),
        ]));
        // cut-12: a superseded r1 and an r2, neither reported, so standing is
        // still what decides which of the two is open work.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("12", 1))]));
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("12", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-12.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-12.r2"))])),
        ]));
        // cut-10: a spec with a report, and that report with no verdict.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("10", 1))]));
        committed(admit(&mut mind, vec![D::CutReport(cut_report("10", 1))]));
        // cut-11: a spec with a report, that report with a verdict, and two
        // findings, one of which is then fixed.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("11", 1))]));
        // Two reports of one cut, one of them judged, so "this cut has a
        // verdict" is not the same answer as "this report has one".
        committed(admit(&mut mind, vec![D::CutReport(cut_report("11", 1)), D::CutReport(cut_report("11", 2))]));
        let f1 = id("finding", "cut-11.s1.F1");
        let f2 = id("finding", "cut-11.s1.F2");
        committed(admit(&mut mind, vec![
            verdict("11", 1, vec![claim(ClaimOutcome::Unproven, &[&f1, &f2], Some("P1"), &["M1"])]),
            D::Finding(finding("11", 1, "F1", FindingConfidence::Plausible)),
            D::Finding(finding("11", 1, "F2", FindingConfidence::Plausible)),
        ]));
        committed(admit(&mut mind, vec![
            follow_up("FU-1", r(K::Finding, &f1)),
            resolution(r(K::Finding, &f2), ResolutionOutcome::Fixed { commit: sha(), by: None }),
        ]));
        committed(admit(&mut mind, elsewhere("Q3")));

        let open = mind.open_items(&slug(CAMPAIGN)).unwrap();
        assert_eq!(ids(&open.questions), vec![q1], "the answered one is closed, another campaign's is not ours");
        assert_eq!(ids(&open.findings), vec![f1], "the fixed one is closed");
        assert_eq!(ids(&open.follow_ups), vec![id("follow_up", "FU-1")]);
        assert_eq!(
            ids(&open.specs_without_report),
            vec![id("cut_spec", "cut-12.r2"), id("cut_spec", "cut-9.r2")],
            "a superseded revision is not open, a spec with a report is not either, and the report of the \
             revision it superseded is not this revision's"
        );
        assert_eq!(
            ids(&open.reports_without_verdict),
            vec![id("cut_report", "cut-10.h1"), id("cut_report", "cut-11.h2"), id("cut_report", "cut-9.h1")],
            "a report is not resolvable, so only the verdict that names it, exactly, closes it"
        );
    }

    /// D5: `semantic` is refused typed, before the image or the receipts are
    /// read, and never answered with an empty page that reads like "nothing
    /// matched". Where the check sits is the rule: below the reader, a store
    /// the reader refuses would answer the integrity fault instead, which is
    /// not what the caller asked about.
    #[test]
    fn semantic_query_refuses_typed_until_wired() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let query = PipelineQuery {
            campaign: Some(slug(CAMPAIGN)),
            repo: Some(repo(REPO)),
            cut: Some(l("9")),
            kinds: vec![K::Question],
            in_force: Some(true),
            faculty: Some(Faculty::Hands),
            admitted_after: Some(s("2026-01-01")),
            admitted_before: Some(s("2027-01-01")),
            limit: Some(10),
            semantic: Some(SemanticQuery { text: "what did we rule about keys".into(), top_k: 5 }),
        };
        let unwired = MindRefusal::Unavailable { detail: "semantic query: the index is not wired (Cut 11)".into() };
        assert_eq!(mind.query(&query).err(), Some(unwired.clone()));
        assert!(mind.query(&PipelineQuery { semantic: None, ..query.clone() }).is_ok());

        // Two receipts naming one write is a store the reader refuses to build
        // over at all. The semantic refusal still lands, because it is decided
        // before the image and the receipts are read.
        let doubled = MemoryStore::new();
        for row in store.rows() {
            doubled.plant(row);
        }
        let forged = crate::receipt::candidate(
            &slug(INSTANCE),
            provenance(Faculty::Soul),
            &[],
            &[prepare(&question("Q1", &["A", "B"], "A")), prepare(&question("Q8", &["A", "B"], "A"))],
            now(),
        )
        .unwrap();
        doubled.plant(schema_cache().unwrap().prepare_entry_named(&forged.receipt_id, &forged).unwrap().0);
        let unreadable = opened(doubled, INSTANCE);
        assert_eq!(
            unreadable.query(&PipelineQuery::default()).err(),
            Some(MindRefusal::Unavailable {
                detail: format!("document {}/{q1} is written by two receipts", K::Question.type_id()),
            }),
            "the reader refuses this store"
        );
        assert_eq!(unreadable.query(&query).err(), Some(unwired), "and the semantic refusal precedes reading it");
    }

    /// D4 and D8: `view` is the typed one-document read with its facts, and an
    /// id the mind does not hold is `None` rather than a refusal.
    #[test]
    fn view_of_an_absent_id_is_none_and_of_a_present_one_is_its_document() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        assert_eq!(mind.view(&r(K::Question, &id("question", "Q404"))).unwrap(), None);
        let found = view(&mind, K::Question, &q1);
        assert_eq!(found.id, r(K::Question, &q1), "the id travels with the document");
        assert_eq!(Some(found.document), mind.get(K::Question, &q1).unwrap());
    }
}
