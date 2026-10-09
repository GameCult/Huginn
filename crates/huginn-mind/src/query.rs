//! The read side of a mind: a document with the facts of its admission and
//! its status, and one typed selection over them.
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
//! `query` is CultNet's selection over rows this crate supplies (`rows.rs`).
//! The substrate owns the filter, the order, the hop, the cursor and the
//! paging; this module owns the snapshot (`Reader::at`: every derivation runs
//! over the documents admitted at or before the page's `asOf`), the
//! projection to a header or a whole view, and the typing of the edges.
//!
//! `rank` is the semantic read: the daemon's index supplies candidate ids and
//! scores, and this module joins them back through the mind and lets the
//! selection decide. No clock, no store handle, no network.

use std::collections::BTreeMap;

use cultnet_rs::{
    Cursor, EdgeAnchor, EdgeMatch, Evaluation, LIMIT_MAX, LIMIT_MIN, PROJECTION_DOCUMENT, Row, Selection, select, validate,
};
use eureka_pipeline::{
    ClaimOutcome, CommitRange, Date, FindingConfidence, FindingOrigin, FindingSeverity, Label, Line, OrgRepo,
    PipelineDocument, PipelineKind, PipelineRef, PipelineResolution, ResolutionOutcome, RulingAuthority, Sha, Short,
    Slug, Title,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::docs::{CitationRole, Docs, Held, citations};
use crate::mind::Mind;
use crate::receipt::{self, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::rows::{SelectionRow, Vocabulary, refuse_values};
use crate::store::MindStore;

/// What admission recorded about a document when it landed: the receipt that
/// wrote it, the `now` that receipt was taken at, who asked, and the
/// receipt's ordinal, which is the read side's order and its snapshot
/// position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdmissionFacts {
    pub receipt_id: String,
    pub admitted_at: String,
    pub provenance: PipelineProvenance,
    #[schemars(range(min = 1))]
    pub ordinal: u64,
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

/// A semantic query: the text to find documents near, and how many. The index
/// owns which fields of each kind are text (`index_text`), so no substring
/// filter lives here to become a second answer to that question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticQuery {
    pub text: Line,
    pub top_k: u32,
}

/// A header's most encoded bytes, whatever the document's lists hold: the
/// summary carries only the facts a caller decides what to work on from,
/// each bounded by the leaf's own bounds.
pub const SUMMARY_MAX_BYTES: usize = 8_192;

/// Whether a summarised document stands: the closing record's outcome, not its
/// rationale. A caller who wants why opens the resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelineStatusSummary {
    InForce,
    Resolved { resolution: PipelineRef, outcome: ResolutionOutcome },
}

/// The organ's meaning of `projection: header`: a document's id, the facts of
/// its admission, its status and the few fields of its kind a caller triages
/// by. Derived per read and bounded by `SUMMARY_MAX_BYTES`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineDocumentSummary {
    pub id: PipelineRef,
    pub admission: AdmissionFacts,
    pub status: PipelineStatusSummary,
    pub facts: PipelineFacts,
}

/// The triage fields of each kind, in the leaf's own envelope spelling. Prose
/// (`Para`) and unbounded lists are not carried; `finding.claim` and
/// `follow_up.item` are, because the leaf typed them as one-line statements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PipelineFacts {
    Campaign { title: Title, repos: Vec<OrgRepo> },
    Target { revision: u32, invariants: Vec<Label> },
    Question { label: Label, title: Title, options: Vec<Label>, recommended: Label, raised_in: Option<PipelineRef>, asked_on: Date },
    Ruling { label: Label, title: Title, answers: Option<Short>, choice: Option<Label>, authority: RulingAuthority, ruled_on: Date },
    CutSpec { cut: Label, revision: u32, title: Title, repo: OrgRepo, branch: Short, base: Sha, depends_on: Vec<Short> },
    CutReport { cut_spec: Short, attempt: u32, repo: OrgRepo, branch: Short, range: CommitRange },
    Verdict { cut_report: Short, pass: u32, range: CommitRange, outcomes: Vec<ClaimOutcome> },
    Finding {
        verdict: Short,
        label: Label,
        confidence: FindingConfidence,
        severity: FindingSeverity,
        origin: FindingOrigin,
        claim: Line,
        invariants: Vec<Label>,
        range: CommitRange,
    },
    FollowUp { label: Label, source: PipelineRef, repo: OrgRepo, owner: Short, item: Line },
    Resolution { subject: PipelineRef, sequence: u32, outcome: ResolutionOutcome, resolved_on: Date },
    Instance { instance: Slug, display_name: Short, host: Short, created_at: Date },
    Stewardship { instance: Slug, repo: OrgRepo, sequence: u32, assigned_on: Date, note: Line },
    HandOff { from_instance: Slug, to_instance: Slug, repo: OrgRepo, handed_on: Date },
}

impl PipelineFacts {
    fn of(document: &PipelineDocument) -> Self {
        use PipelineDocument as D;
        match document {
            D::Campaign(value) => Self::Campaign { title: value.title.clone(), repos: value.repos.clone() },
            D::Target(value) => Self::Target {
                revision: value.revision,
                invariants: value.invariants.iter().map(|invariant| invariant.label.clone()).collect(),
            },
            D::Question(value) => Self::Question {
                label: value.label.clone(),
                title: value.title.clone(),
                options: value.options.iter().map(|option| option.label.clone()).collect(),
                recommended: value.recommended.clone(),
                raised_in: value.raised_in.clone(),
                asked_on: value.asked_on.clone(),
            },
            D::Ruling(value) => Self::Ruling {
                label: value.label.clone(),
                title: value.title.clone(),
                answers: value.answers.clone(),
                choice: value.choice.clone(),
                authority: value.authority,
                ruled_on: value.ruled_on.clone(),
            },
            D::CutSpec(value) => Self::CutSpec {
                cut: value.cut.clone(),
                revision: value.revision,
                title: value.title.clone(),
                repo: value.repo.clone(),
                branch: value.branch.clone(),
                base: value.base.clone(),
                depends_on: value.depends_on.clone(),
            },
            D::CutReport(value) => Self::CutReport {
                cut_spec: value.cut_spec.clone(),
                attempt: value.attempt,
                repo: value.repo.clone(),
                branch: value.branch.clone(),
                range: value.range.clone(),
            },
            D::Verdict(value) => Self::Verdict {
                cut_report: value.cut_report.clone(),
                pass: value.pass,
                range: value.range.clone(),
                outcomes: value.claims.iter().map(|claim| claim.outcome).collect(),
            },
            D::Finding(value) => Self::Finding {
                verdict: value.verdict.clone(),
                label: value.label.clone(),
                confidence: value.confidence,
                severity: value.severity,
                origin: value.origin,
                claim: value.claim.clone(),
                invariants: value.invariants.clone(),
                range: value.range.clone(),
            },
            D::FollowUp(value) => Self::FollowUp {
                label: value.label.clone(),
                source: value.source.clone(),
                repo: value.repo.clone(),
                owner: value.owner.clone(),
                item: value.item.clone(),
            },
            D::Resolution(value) => Self::Resolution {
                subject: value.subject.clone(),
                sequence: value.sequence,
                outcome: value.outcome.clone(),
                resolved_on: value.resolved_on.clone(),
            },
            D::Instance(value) => Self::Instance {
                instance: value.instance.clone(),
                display_name: value.display_name.clone(),
                host: value.host.clone(),
                created_at: value.created_at.clone(),
            },
            D::Stewardship(value) => Self::Stewardship {
                instance: value.instance.clone(),
                repo: value.repo.clone(),
                sequence: value.sequence,
                assigned_on: value.assigned_on.clone(),
                note: value.note.clone(),
            },
            D::HandOff(value) => Self::HandOff {
                from_instance: value.from_instance.clone(),
                to_instance: value.to_instance.clone(),
                repo: value.repo.clone(),
                handed_on: value.handed_on.clone(),
            },
        }
    }
}

impl PipelineDocumentSummary {
    fn of(view: &PipelineDocumentView) -> Self {
        Self {
            id: view.id.clone(),
            admission: view.admission.clone(),
            status: match &view.status {
                PipelineStatus::InForce => PipelineStatusSummary::InForce,
                PipelineStatus::Resolved { resolution, record } => {
                    PipelineStatusSummary::Resolved { resolution: resolution.clone(), outcome: record.outcome.clone() }
                }
            },
            facts: PipelineFacts::of(&view.document),
        }
    }
}

/// The items of a page, chosen once by the selection's `projection`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelinePageItems {
    Headers(Vec<PipelineDocumentSummary>),
    Documents(Vec<PipelineDocumentView>),
}

/// One citation edge a hop traversed: the organ's typing of the substrate's
/// edge, whose ends are record keys and whose role is a string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineEdge {
    pub from: PipelineRef,
    pub role: CitationRole,
    pub to: PipelineRef,
}

/// One page of a selection. `matched` is the count before paging, `as_of` the
/// snapshot the page and every cursor of its walk are exact as of, and `edges`
/// is present exactly when the selection follows a hop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineSelectionPage {
    pub matched: u32,
    pub as_of: u64,
    pub next: Option<String>,
    pub items: PipelinePageItems,
    pub edges: Option<Vec<PipelineEdge>>,
}

/// A ranked semantic answer: the page, and the hits the index named that the
/// mind holds no document for (a restored mind, a second store). Local to the
/// process; it is never on the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub page: PipelineSelectionPage,
    pub dropped: Vec<PipelineRef>,
}

/// The admission facts of every stored document, built once per read call from
/// the receipts. `writes` only: a document a later batch cited as a strong read
/// keeps the receipt that wrote it. A10 admits one write per identity, so two
/// receipts naming one document is an integrity fault, not a choice to make.
pub(crate) struct AdmissionIndex(BTreeMap<(String, String), AdmissionFacts>);

impl AdmissionIndex {
    pub(crate) fn build<S: MindStore>(mind: &Mind<S>) -> Result<Self, MindRefusal> {
        // Every reader of the ordinal asks the same density check admission
        // does, through `head`: a chain that is not exactly `{1..=N}` refuses
        // here before a single ordinal is copied out, not only when admission
        // happens to be the caller.
        receipt::head(mind)?;
        let mut facts = BTreeMap::new();
        for receipt in mind.receipts()? {
            for write in receipt.writes.iter() {
                let identity = (write.document_type.clone(), write.document_key.clone());
                let landed = AdmissionFacts {
                    receipt_id: receipt.receipt_id.clone(),
                    admitted_at: receipt.committed_at.clone(),
                    provenance: receipt.provenance.clone(),
                    ordinal: receipt.ordinal,
                };
                if facts.insert(identity, landed).is_some() {
                    return Err(integrity(&write.document_type, &write.document_key, "is written by two receipts"));
                }
            }
        }
        Ok(Self(facts))
    }

    fn of(&self, held: &Held) -> Result<AdmissionFacts, MindRefusal> {
        self.facts_of(held.kind, &held.key)
    }

    fn facts_of(&self, kind: PipelineKind, key: &str) -> Result<AdmissionFacts, MindRefusal> {
        let type_id = kind.type_id();
        self.0
            .get(&(type_id.to_string(), key.to_string()))
            .cloned()
            .ok_or_else(|| integrity(type_id, key, "has no commit receipt"))
    }

    /// The ordinal of the receipt that wrote a document: the projection's
    /// position for it.
    pub(crate) fn ordinal_of(&self, kind: PipelineKind, key: &str) -> Result<u64, MindRefusal> {
        self.facts_of(kind, key).map(|facts| facts.ordinal)
    }
}

fn integrity(type_id: &str, key: &str, fault: &str) -> MindRefusal {
    MindRefusal::Unavailable { detail: format!("document {type_id}/{key} {fault}") }
}

/// One read call's working set: the image as of one snapshot and the facts it
/// was built from. Built once per call and thrown away with it; nothing here
/// outlives the answer.
struct Reader {
    docs: Docs,
    facts: AdmissionIndex,
}

impl Reader {
    /// The mind as it stood when receipt `as_of` landed: the documents whose
    /// writing receipt has `ordinal <= as_of`, and every derivation (in
    /// force, the closing resolution, `base`, citations) over that image
    /// alone. The facts are built over all receipts, and every held document
    /// must have its receipt whatever `as_of` is, so an orphan or a double
    /// write refuses at any snapshot.
    fn at<S: MindStore>(mind: &Mind<S>, as_of: u64) -> Result<Self, MindRefusal> {
        let mut docs = Docs::from_image(mind.envelopes())?;
        let facts = AdmissionIndex::build(mind)?;
        let mut visible = Vec::with_capacity(docs.image.len());
        for held in docs.image {
            if facts.of(&held)?.ordinal <= as_of {
                visible.push(held);
            }
        }
        docs.image = visible;
        Ok(Self { docs, facts })
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

    /// The document `repo` and `cut` are read through: a resolution's
    /// subject, and its subject's subject when that is a resolution too (the
    /// Q19 cap stops the chain at two). A7 puts every subject in the image,
    /// and a resolution key is strictly longer than the id it resolves, so
    /// this walks down and stops. Anything that is not a resolution is its
    /// own base.
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

    fn rows(&self) -> Result<Vec<SelectionRow>, MindRefusal> {
        self.docs
            .image
            .iter()
            .map(|held| {
                Ok(SelectionRow::new(held, self.base(held), self.view_of(held)?, citations(&held.document)))
            })
            .collect()
    }
}

fn ref_of(row: &SelectionRow) -> PipelineRef {
    PipelineRef { kind: row.kind, id: Short(row.key.clone()) }
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
        let reader = Reader::at(self, receipt::head(self)?)?;
        reader.held(id.kind, &id.id.0).map(|held| reader.view_of(held)).transpose()
    }

    /// One typed selection over the mind, as of one snapshot.
    ///
    /// In order: the substrate checks the selection's names; the organ's door
    /// checks its values; the snapshot is the cursor's own `asOf` when there
    /// is a cursor and the head otherwise (a cursor from beyond the head is
    /// one this mind could not have minted); the substrate evaluates; the page
    /// is projected. The evaluator's answer is never re-filtered here.
    pub fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage, MindRefusal> {
        validate(selection, &Vocabulary).map_err(cultnet_rs::SelectionRefusal::Invalid)?;
        let selection = refuse_values(selection)?;
        let head = receipt::head(self)?;
        let as_of = match selection.cursor.as_deref().filter(|cursor| !cursor.is_empty()) {
            Some(cursor) => {
                let as_of = Cursor::parse(cursor)?.as_of;
                if as_of > head {
                    return Err(MindRefusal::CursorInvalid {
                        message: format!("the cursor names snapshot {as_of}, beyond this mind's head {head}"),
                    });
                }
                as_of
            }
            None => head,
        };
        let rows = Reader::at(self, as_of)?.rows()?;
        let evaluation = select(&Vocabulary, &rows, &selection, as_of, self.cursor_key())?;
        Self::page(&selection, evaluation, as_of)
    }

    /// What a semantic query must satisfy before anything is embedded: the
    /// selection passes the same two doors `query` does, carries no cursor and
    /// no `descending` (a ranked answer is neither pageable nor reversible),
    /// and asks for between one and
    /// `LIMIT_MAX` hits over text that says something. Returns the selection
    /// as the door leaves it.
    fn semantic_selection(selection: &Selection, semantic: &SemanticQuery) -> Result<Selection, MindRefusal> {
        let refused = |field: &str, value: Option<String>, message: &str| MindRefusal::SelectionInvalid {
            field: field.into(),
            value,
            message: message.into(),
        };
        if selection.cursor.as_deref().is_some_and(|cursor| !cursor.is_empty()) {
            return Err(refused("cursor", None, "a semantic answer is ranked, not paged: it carries no cursor"));
        }
        if selection.descending {
            return Err(refused("descending", Some("true".into()), "a semantic answer is ordered by score: it has no order to reverse"));
        }
        if semantic.text.0.trim().is_empty() {
            return Err(refused("semantic.text", None, "a semantic query needs text"));
        }
        if !(LIMIT_MIN..=LIMIT_MAX).contains(&semantic.top_k) {
            return Err(refused(
                "semantic.top_k",
                Some(semantic.top_k.to_string()),
                &format!("top_k is between {LIMIT_MIN} and {LIMIT_MAX}"),
            ));
        }
        validate(selection, &Vocabulary).map_err(cultnet_rs::SelectionRefusal::Invalid)?;
        refuse_values(selection)
    }

    /// Whether a semantic query may be started, decided before the index is
    /// asked anything. `rank` runs the same check again over the same inputs.
    pub fn check_semantic(&self, selection: &Selection, semantic: &SemanticQuery) -> Result<(), MindRefusal> {
        Self::semantic_selection(selection, semantic).map(drop)
    }

    /// The page a semantic query answers, from the candidates the index gave,
    /// with the hits the mind holds no document for.
    ///
    /// The index supplies ids and scores and nothing else. A hit is a
    /// candidate: it is dropped unless the mind holds a document of that kind
    /// and id at the head, and what remains is decided by the selection, run
    /// by the substrate over every row the mind has, its `keys` narrowed to
    /// the candidates. Narrowing `keys` rather than the rows keeps the
    /// evaluator's universe whole, so `cited` and `in_force` mean what they
    /// mean in `query`. Nothing about a document's standing is read from the
    /// index; `in_force` is the caller's to ask for and the mind's to derive.
    ///
    /// This is the one place the mind judges whether it holds a hit, in the
    /// one read of its rows that ranking needs: `Ranked::dropped` returns the
    /// hits it found no document for, for the caller to report.
    ///
    /// The order is the one thing the substrate cannot know: score, highest
    /// first, ties by ordinal. The page holds at most the selection's limit
    /// and at most `top_k` documents, `matched` counts every candidate the
    /// selection passed, and `next` is never set. The substrate answers at
    /// most `LIMIT_MAX` rows and cuts by ordinal, so the candidates are cut
    /// to the best `LIMIT_MAX` by score first, among the documents the mind
    /// holds: an id the mind does not hold never takes the place of one it
    /// does.
    pub fn rank(
        &self,
        selection: &Selection,
        semantic: &SemanticQuery,
        hits: &[(PipelineRef, f32)],
    ) -> Result<Ranked, MindRefusal> {
        let selection = Self::semantic_selection(selection, semantic)?;
        let head = receipt::head(self)?;
        let rows = Reader::at(self, head)?.rows()?;

        let held: BTreeMap<(PipelineKind, &str), i64> =
            rows.iter().map(|row| ((row.kind, row.key.as_str()), row.ordinal())).collect();
        let mut candidates: Vec<(&str, f32, i64)> = Vec::new();
        let mut dropped = Vec::new();
        for (hit, score) in hits {
            match held.get(&(hit.kind, hit.id.0.as_str())) {
                Some(&ordinal) => candidates.push((hit.id.0.as_str(), *score, ordinal)),
                None => dropped.push(hit.clone()),
            }
        }
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
        candidates.truncate(LIMIT_MAX as usize);
        let mut scores: BTreeMap<&str, f32> = BTreeMap::new();
        for (key, score, _) in &candidates {
            scores.entry(key).or_insert(*score);
        }
        let mut keys: Vec<String> = scores.keys().map(|key| key.to_string()).collect();
        if let Some(allowed) = &selection.keys {
            keys.retain(|key| allowed.contains(key));
        }
        let limit = selection.limit.unwrap_or(LIMIT_MAX).clamp(LIMIT_MIN, LIMIT_MAX).min(semantic.top_k) as usize;

        let mut evaluation = if keys.is_empty() {
            Evaluation { rows: Vec::new(), matched: 0, edges: Vec::new(), next_cursor: None }
        } else {
            let narrowed = Selection { keys: Some(keys), limit: Some(LIMIT_MAX), ..selection.clone() };
            select(&Vocabulary, &rows, &narrowed, head, self.cursor_key())?
        };
        let score = |row: &SelectionRow| scores.get(row.key.as_str()).copied().unwrap_or(f32::NEG_INFINITY);
        evaluation.rows.sort_by(|a, b| score(b).total_cmp(&score(a)).then(a.ordinal().cmp(&b.ordinal())));
        evaluation.rows.truncate(limit);
        evaluation.next_cursor = None;
        let position =
            |kind: PipelineKind, key: &str| evaluation.rows.iter().position(|row| row.kind == kind && row.key == key);
        let mut kept: Vec<(usize, EdgeMatch<SelectionRow>)> = std::mem::take(&mut evaluation.edges)
            .into_iter()
            .filter_map(|edge| {
                let owner = match edge.anchor {
                    EdgeAnchor::Citee => &edge.to,
                    EdgeAnchor::Citer => &edge.from,
                };
                position(owner.kind, &owner.key).map(|at| (at, edge))
            })
            .collect();
        kept.sort_by_key(|(at, _)| *at);
        evaluation.edges = kept.into_iter().map(|(_, edge)| edge).collect();
        Ok(Ranked { page: Self::page(&selection, evaluation, head)?, dropped })
    }

    /// The projection of an evaluation: a header or a whole view per row, and
    /// the typed edges a hop traversed.
    fn page(
        selection: &Selection,
        evaluation: Evaluation<SelectionRow>,
        as_of: u64,
    ) -> Result<PipelineSelectionPage, MindRefusal> {
        let items = if selection.projection == PROJECTION_DOCUMENT {
            PipelinePageItems::Documents(evaluation.rows.iter().map(|row| row.view.clone()).collect())
        } else {
            PipelinePageItems::Headers(evaluation.rows.iter().map(|row| PipelineDocumentSummary::of(&row.view)).collect())
        };
        let edges = if selection.has_hop() {
            let mut typed = Vec::with_capacity(evaluation.edges.len());
            for edge in &evaluation.edges {
                let role = CitationRole::from_name(&edge.role).ok_or_else(|| MindRefusal::Unavailable {
                    detail: format!("the evaluator returned an edge in role {:?}, which is not a citation role", edge.role),
                })?;
                typed.push(PipelineEdge { from: ref_of(&edge.from), role, to: ref_of(&edge.to) });
            }
            Some(typed)
        } else {
            None
        };
        Ok(PipelineSelectionPage { matched: evaluation.matched, as_of, next: evaluation.next_cursor, items, edges })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::receipt::Faculty;
    use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef};
    use crate::mind::schema_cache;
    use crate::store::test_stores::MemoryStore;
    use chrono::{TimeZone, Utc};
    use eureka_pipeline::{
        ClaimOutcome, FindingConfidence, PipelineDocument as D, PipelineKind as K, PipelineRefusal, ResolutionOutcome,
    };

    fn view(mind: &Mind<MemoryStore>, kind: K, key: &str) -> PipelineDocumentView {
        mind.view(&r(kind, key)).unwrap().unwrap_or_else(|| panic!("{key} is not held"))
    }

    fn ids(page: &PipelineSelectionPage) -> Vec<String> {
        match &page.items {
            PipelinePageItems::Headers(headers) => headers.iter().map(|header| header.id.id.0.clone()).collect(),
            PipelinePageItems::Documents(views) => views.iter().map(|view| view.id.id.0.clone()).collect(),
        }
    }

    fn headers(page: &PipelineSelectionPage) -> &[PipelineDocumentSummary] {
        match &page.items {
            PipelinePageItems::Headers(headers) => headers,
            PipelinePageItems::Documents(_) => panic!("expected headers"),
        }
    }

    fn of_kinds(kinds: &[K]) -> Selection {
        Selection { schemas: Some(kinds.iter().map(|kind| kind.type_id().to_string()).collect()), ..Selection::default() }
    }

    fn any_of(index: &str, values: &[&str]) -> FieldPredicate {
        FieldPredicate {
            index: index.into(),
            op: "any_of".into(),
            values: Some(values.iter().map(|value| value.to_string()).collect()),
            number: None,
        }
    }

    fn with(mut selection: Selection, predicate: FieldPredicate) -> Selection {
        selection.fields.get_or_insert_with(Vec::new).push(predicate);
        selection
    }

    fn cites(kind: K, key: &str, role: Option<&str>) -> Citation {
        Citation { target: RecordRef::new(kind.type_id(), key), role: role.map(str::to_string) }
    }

    /// The refusal a selection earns from a mind that would otherwise answer.
    fn refused(mind: &Mind<MemoryStore>, selection: &Selection) -> MindRefusal {
        mind.query(selection).expect_err("the selection is refused")
    }

    fn invalid(field: &str, value: &str) -> impl Fn(&MindRefusal) -> bool {
        let (field, value) = (field.to_string(), value.to_string());
        move |refusal| {
            matches!(refusal, MindRefusal::SelectionInvalid { field: f, value: Some(v), .. } if *f == field && *v == value)
        }
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

    /// Ruling 1's read half: the grammar lives in the leaf, and `view`, the
    /// one door that takes a ref, asks it before it looks. A ref that is no
    /// ref is refused; a ref that is well formed and names nothing is still
    /// the empty answer, so the door refuses malformation and not absence.
    #[test]
    fn a_ref_whose_kind_and_id_disagree_is_refused_by_view() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![answering("R1", &q1)]));

        // The id names a question this mind holds and a resolution it derived;
        // read as a ruling it names neither, and is not a reference at all.
        let disagreeing = r(K::Ruling, &q1);
        assert_eq!(mind.view(&disagreeing), Err(malformed(&q1)));

        // A local no writer composes is the same refusal, so the door is the
        // whole grammar and not the kind segment alone. The refusal's `value`
        // is the failing part, here the empty part after the trailing dot,
        // and not the whole id.
        let dotted = r(K::Question, &format!("{q1}."));
        assert_eq!(mind.view(&dotted), Err(malformed("")));

        // Absence is not malformation: a well-formed id of a kind this mind
        // does not hold answers as it always did.
        let absent = id("question", "Q9");
        assert_eq!(mind.view(&r(K::Question, &absent)), Ok(None));
    }

    /// F1 (RS fix batch): the read side never got to answer over an
    /// undense ordinal chain, because `AdmissionIndex::build` copied
    /// `receipt.ordinal` straight off the receipts without asking `head`.
    /// `query` and `view` now go through the same density check admission
    /// does, so a store planted directly (bypassing admission entirely) with
    /// a duplicate, a gap, a zero, or an ordinal above `N` refuses both doors
    /// alike, never answering as though the chain were dense.
    #[test]
    fn query_and_view_refuse_a_receipt_chain_that_is_not_dense() {
        let base = MemoryStore::new();
        let mut mind = opened(base.clone(), INSTANCE);
        seed(&mut mind);
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        // The seed and Q1's batch: two receipts, ordinals {1, 2}.

        let cache = schema_cache().unwrap();
        let plant_at = |ordinal: u64, label: &str| {
            let broken = MemoryStore::new();
            for row in base.rows() {
                broken.plant(row);
            }
            let extra = receipt::candidate(
                &slug(INSTANCE),
                provenance(Faculty::Hands),
                &[],
                &[prepare(&question(label, &["A", "B"], "A"))],
                ordinal,
                now(),
            )
            .unwrap();
            broken.plant(cache.prepare_entry_named(&extra.receipt_id, &extra).unwrap().0);
            broken
        };

        let q1 = id("question", "Q1");
        let refuses = |store: MemoryStore, label: &str| {
            let broken_mind = opened(store, INSTANCE);
            assert!(
                matches!(broken_mind.query(&Selection::default()), Err(MindRefusal::Unavailable { .. })),
                "{label}: query must refuse an undense chain"
            );
            assert!(
                matches!(broken_mind.view(&r(K::Question, &q1)), Err(MindRefusal::Unavailable { .. })),
                "{label}: view must refuse an undense chain"
            );
        };

        refuses(plant_at(2, "Q2"), "duplicate [1,2,2]");
        refuses(plant_at(4, "Q2"), "gap [1,2,4]");
        refuses(plant_at(0, "Q2"), "zero [0,1,2]");
        refuses(plant_at(99, "Q2"), "above N [1,2,99]");
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
        assert_eq!(orphaned.query(&Selection::default()).err(), Some(MindRefusal::Unavailable { detail }));

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
            4, // head + 1: dense, so this plants only the duplicate identity, not an F1 density fault too.
            now(),
        )
        .unwrap();
        doubled.plant(schema_cache().unwrap().prepare_entry_named(&forged.receipt_id, &forged).unwrap().0);
        assert_eq!(
            opened(doubled, INSTANCE).query(&Selection::default()).err(),
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
        let expected = mind.query(&Selection::default()).unwrap();
        let constant = vec!["2000-01-01T00:00:00Z".to_string(); rows.len()];
        let reversed = rows.iter().rev().map(|row| row.stored_at.clone()).collect::<Vec<_>>();
        for other in [reopened(constant), reopened(reversed)] {
            assert_eq!(other.query(&Selection::default()).unwrap(), expected);
        }
    }

    /// A mind with something for every alias to select: two campaigns, three
    /// cuts (one superseded, one with a resolution of a resolution), a
    /// verdict and its finding, a follow-up, and a second faculty.
    fn worked() -> Mind<MemoryStore> {
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
        let later = Utc.with_ymd_and_hms(2026, 9, 17, 12, 0, 0).unwrap();
        committed(admit_at(&mut mind, vec![question_n(2)], later));
        mind
    }

    fn matching(mind: &Mind<MemoryStore>, selection: Selection) -> Vec<String> {
        let mut matched = ids(&mind.query(&selection).unwrap());
        matched.sort();
        matched
    }

    /// One assertion per alias, each selecting by its own field alone.
    #[test]
    fn each_alias_selects_by_its_own_field() {
        let mind = worked();
        let by = |index: &str, values: &[&str]| matching(&mind, with(Selection::default(), any_of(index, values)));
        let supersession = id("resolution", "cut_spec.cut-9.r1.n1");

        // `root`: the key's root, a campaign slug for most kinds and an
        // instance for the ones that hang off one; a resolution's is its
        // subject's.
        let eureka = by("root", &[CAMPAIGN]);
        assert!(eureka.contains(&id("question", "Q1")));
        assert!(eureka.contains(&supersession), "a resolution is keyed inside its subject's root");
        assert!(!eureka.contains(&"other-campaign:question:Q1".to_string()));
        assert_eq!(by("root", &[INSTANCE]), vec![
            format!("{INSTANCE}:instance:self"),
            format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1"),
        ]);

        // `repo`: a campaign lists its repos, five kinds name one, and a
        // resolution matches when its base does.
        let epiphany = by("repo", &[REPO]);
        for expected in [
            id("campaign", "self"),
            "other-campaign:campaign:self".to_string(),
            id("cut_spec", "cut-9.r1"),
            id("cut_report", "cut-9.h1"),
            id("follow_up", "FU-1"),
            format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1"),
            supersession.clone(),
        ] {
            assert!(epiphany.contains(&expected), "{expected}");
        }
        assert!(!epiphany.contains(&id("question", "Q1")), "a question carries no repo");

        // `cut`: the label in the local's `cut-<label>.` prefix, which ends at
        // the dot, so cut 1 is not every cut that starts with its label.
        assert_eq!(by("cut", &["9"]), vec![
            id("cut_report", "cut-9.h1"),
            id("cut_spec", "cut-9.r1"),
            id("cut_spec", "cut-9.r2"),
            id("finding", "cut-9.s1.F1"),
            supersession.clone(),
            id("verdict", "cut-9.s1"),
        ]);
        assert_eq!(by("cut", &["1"]), vec![id("cut_spec", "cut-1.r1")]);

        // `schemas`, and only the ones asked for.
        assert_eq!(matching(&mind, of_kinds(&[K::Question])), vec![
            id("question", "Q1"),
            id("question", "Q2"),
            "other-campaign:question:Q1".to_string(),
        ]);

        // `in_force`, both ways: the superseded revision is the one thing this
        // mind has closed, and the withdrawn cut-10 closure.
        assert_eq!(by("in_force", &["false"]), vec![
            id("cut_spec", "cut-9.r1"),
            id("resolution", "cut_spec.cut-10.r1.n1"),
        ]);
        let standing = by("in_force", &["true"]);
        assert!(!standing.contains(&id("cut_spec", "cut-9.r1")));
        assert!(standing.contains(&id("cut_spec", "cut-9.r2")));

        // `faculty`: attribution selects, and grants nothing (ruling 18).
        assert_eq!(by("faculty", &["Soul"]), vec![id("ruling", "R2")]);
        assert!(!by("faculty", &["Hands"]).contains(&id("ruling", "R2")));
        // The renamed faculty is selectable by its new name only.
        assert!(by("faculty", &["Life"]).is_empty());
        assert!(invalid("fields[0].values", "MindSteward")(&refused(&mind, &with(Selection::default(), any_of("faculty", &["MindSteward"])))));

        // A repo is named by its identity, which is case-insensitive.
        assert_eq!(by("repo", &["GAMECULT/EPIPHANY"]), epiphany);
    }

    /// V17/V17L: `repo` and `cut` reach a resolution through its base, down to
    /// the first non-resolution, and not one step of the way.
    #[test]
    fn repo_and_cut_reach_a_resolution_through_its_base() {
        let mind = worked();
        let withdrawal_of_withdrawal = id("resolution", "resolution.cut_spec.cut-10.r1.n1.n1");
        let by = |index: &str, value: &str| matching(&mind, with(of_kinds(&[K::Resolution]), any_of(index, &[value])));
        assert!(by("cut", "10").contains(&withdrawal_of_withdrawal), "two steps of base reach the cut spec");
        assert!(by("repo", REPO).contains(&withdrawal_of_withdrawal));
        assert!(by("cut", "10").contains(&id("resolution", "cut_spec.cut-10.r1.n1")));
    }

    /// The order is the ordinal: two batches at one clock, ids opposite to
    /// admission, come back in admission order.
    #[test]
    fn the_page_order_is_admission_order() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(9)]));
        committed(admit(&mut mind, vec![question_n(1)]));
        let selection = of_kinds(&[K::Question]);
        assert_eq!(ids(&mind.query(&selection).unwrap()), vec![id("question", "Q9"), id("question", "Q1")]);
        let newest_first = Selection { descending: true, ..selection };
        assert_eq!(ids(&mind.query(&newest_first).unwrap()), vec![id("question", "Q1"), id("question", "Q9")]);
        // The facts on a header name the ordinal the order came from.
        let page = mind.query(&of_kinds(&[K::Question])).unwrap();
        let ordinals: Vec<u64> = headers(&page).iter().map(|header| header.admission.ordinal).collect();
        assert_eq!(ordinals, vec![2, 3]);
    }

    /// The cap and the count: 201 questions, a page never longer than 200, the
    /// count taken before the page, and a cursor that walks the rest.
    #[test]
    fn the_limit_clamps_and_matched_counts_before_the_page() {
        let mut mind = seeded();
        let batches: [Vec<u32>; 4] =
            [(2..=65).collect(), (66..=129).collect(), (130..=193).collect(), std::iter::once(1).chain(194..=201).collect()];
        for labels in &batches {
            committed(admit(&mut mind, labels.iter().map(|label| question_n(*label)).collect()));
        }
        let questions = of_kinds(&[K::Question]);
        for (limit, expected) in [(None, 200), (Some(500), 200), (Some(0), 1), (Some(5), 5)] {
            let page = mind.query(&Selection { limit, ..questions.clone() }).unwrap();
            assert_eq!(ids(&page).len(), expected, "limit {limit:?}");
            assert_eq!(page.matched, 201, "the count is taken before the cap");
        }
        let first = mind.query(&questions).unwrap();
        assert_eq!(ids(&first)[0], id("question", "Q10"), "the earliest batch first, and inside it key order");
        let second = mind.query(&Selection { cursor: first.next.clone(), ..questions }).unwrap();
        assert_eq!(ids(&second), vec![id("question", "Q201")], "the last batch's last key is the one the cap dropped");
        assert_eq!((second.matched, second.next), (201, None));
        assert_eq!(second.as_of, first.as_of);
    }

    /// A page is exact as of the `asOf` its cursor names: rows and every
    /// derivation. A batch admitted between two pages closes a page-2 item and
    /// adds a match; page 2 still shows the item in force and the new match is
    /// absent.
    #[test]
    fn a_walk_reads_the_snapshot_its_cursor_names() {
        let mut mind = seeded();
        for n in 1..=3 {
            committed(admit(&mut mind, vec![question_n(n)]));
        }
        // Newest first, so page 1's last row is the head row: the boundary
        // sits at exactly `asOf`.
        let walk = Selection { descending: true, limit: Some(2), ..of_kinds(&[K::Question]) };
        let first = mind.query(&walk).unwrap();
        assert_eq!(ids(&first), vec![id("question", "Q3"), id("question", "Q2")]);
        assert_eq!((first.matched, first.as_of), (3, 4));
        let cursor = first.next.clone().expect("Q1 is left");

        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![answering("R1", &q1), question_n(4)]));

        let second = mind.query(&Selection { cursor: Some(cursor.clone()), ..walk.clone() }).unwrap();
        assert_eq!(ids(&second), vec![q1.clone()]);
        assert_eq!(headers(&second)[0].status, PipelineStatusSummary::InForce, "the closing ruling landed after asOf");
        assert_eq!((second.matched, second.as_of, second.next.as_deref()), (3, 4, None));

        // A fresh walk starts at the new head and sees all of it.
        let fresh = mind.query(&walk).unwrap();
        assert_eq!((fresh.matched, fresh.as_of), (4, 5));
        assert_eq!(ids(&fresh), vec![id("question", "Q4"), id("question", "Q3")]);
        let closed = mind.query(&Selection { cursor: fresh.next.clone(), ..walk.clone() }).unwrap();
        assert!(matches!(headers(&closed)[1].status, PipelineStatusSummary::Resolved { .. }), "Q1 is closed at head");

        // A cursor is bound to its selection: change the order and it is not
        // this walk's.
        let other = Selection { descending: false, cursor: Some(cursor), ..walk };
        assert!(matches!(refused(&mind, &other), MindRefusal::CursorInvalid { .. }));
    }

    /// A cursor whose `asOf` is beyond the head names a snapshot this mind
    /// never reached; one at exactly the head answers.
    #[test]
    fn a_cursor_from_the_future_is_invalid() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(1), question_n(2)]));
        let selection = of_kinds(&[K::Question]);
        let head = 2;
        let rows = Reader::at(&mind, head).unwrap().rows().unwrap();
        let at = |as_of: u64| Selection {
            cursor: Some(Cursor::mint(as_of, &rows[0], &selection, mind.cursor_key())),
            ..selection.clone()
        };
        assert!(mind.query(&at(head)).is_ok(), "a cursor at exactly the head answers");
        assert!(matches!(refused(&mind, &at(head + 1)), MindRefusal::CursorInvalid { .. }));
        let stale = at(1);
        assert!(mind.query(&stale).is_ok(), "an older snapshot is still answerable: the mind is append-only");
    }

    /// The door refuses a value outside its alias's domain, naming the field
    /// and the value the client sent, never answering empty.
    #[test]
    fn a_root_outside_the_grammar_is_refused_not_empty() {
        let mind = worked();
        assert!(mind.query(&with(Selection::default(), any_of("root", &[CAMPAIGN]))).unwrap().matched > 0);
        // The bad value is the second in its list and the predicate is the
        // second in its selection: the door reads them all.
        let bad = with(with(Selection::default(), any_of("in_force", &["true"])), any_of("root", &[CAMPAIGN, "a/../b"]));
        assert!(invalid("fields[1].values", "a/../b")(&refused(&mind, &bad)));
    }

    #[test]
    fn a_repo_outside_org_repo_is_refused() {
        let mind = worked();
        for bad in ["GameCult/", "a/b/c"] {
            assert!(invalid("fields[0].values", bad)(&refused(&mind, &with(Selection::default(), any_of("repo", &[bad])))));
        }
    }

    #[test]
    fn a_cut_outside_label_is_refused() {
        let mind = worked();
        assert!(invalid("fields[0].values", "1.2")(&refused(&mind, &with(Selection::default(), any_of("cut", &["1.2"])))));
    }

    #[test]
    fn enum_values_are_the_variant_names() {
        let mind = worked();
        let severity = |value: &str| with(of_kinds(&[K::Finding]), any_of("severity", &[value]));
        assert!(invalid("fields[0].values", "high")(&refused(&mind, &severity("high"))));
        assert_eq!(ids(&mind.query(&severity("High")).unwrap()), vec![id("finding", "cut-9.s1.F1")]);
        assert!(mind.query(&severity("Low")).unwrap().matched == 0, "a value in the domain that matches nothing is empty");
        let in_force = with(Selection::default(), any_of("in_force", &["yes"]));
        assert!(invalid("fields[0].values", "yes")(&refused(&mind, &in_force)));
    }

    /// The values the rows emit are the values the door admits: every emitted
    /// value of every closed alias is in its domain, so a client can select
    /// what it reads back.
    #[test]
    fn every_emitted_closed_value_passes_the_door() {
        let mind = worked();
        let rows = Reader::at(&mind, 100).unwrap().rows().unwrap();
        for row in &rows {
            for alias in ["root", "in_force", "faculty", "repo", "cut", "severity", "confidence", "origin", "authority", "claim_outcome", "outcome"] {
                for value in cultnet_rs::Row::values(row, alias) {
                    let selection = with(Selection::default(), any_of(alias, &[&value]));
                    assert!(mind.query(&selection).unwrap().matched >= 1, "{alias}={value}");
                }
            }
        }
    }

    #[test]
    fn an_unknown_schema_or_malformed_key_is_refused() {
        let mind = worked();
        let question = K::Question.type_id();
        for bad in ["epiphany.pipeline.nonsense.v2".to_string(), format!("{question}0")] {
            let selection = Selection { schemas: Some(vec![bad.clone()]), ..Selection::default() };
            assert!(invalid("schemas", &bad)(&refused(&mind, &selection)), "{bad}");
        }
        for bad in ["nonsense", "eureka-state:question:", "eureka-state:nonsense:Q1"] {
            let selection = Selection { keys: Some(vec![bad.to_string()]), ..Selection::default() };
            assert!(invalid("keys", bad)(&refused(&mind, &selection)), "{bad}");
        }
        let q1 = id("question", "Q1");
        let held = Selection { keys: Some(vec![q1.clone()]), ..Selection::default() };
        assert_eq!(ids(&mind.query(&held).unwrap()), vec![q1]);
    }

    /// V24 at the selection's door: a `cites` target whose kind and key
    /// disagree names no document any writer could compose.
    #[test]
    fn a_cites_target_whose_kind_disagrees_is_refused() {
        let mind = worked();
        let q1 = id("question", "Q1");
        let wrong = Selection { cites: Some(cites(K::Ruling, &q1, None)), ..Selection::default() };
        assert!(invalid("cites.target", &q1)(&refused(&mind, &wrong)));
        let right = Selection { cites: Some(cites(K::Question, &q1, Some("source"))), ..Selection::default() };
        assert_eq!(ids(&mind.query(&right).unwrap()), vec![id("follow_up", "FU-1")]);
    }

    /// The substrate's own refusals cross as themselves: an undeclared alias
    /// is a `SelectionInvalid` from the substrate's door, before the organ's.
    #[test]
    fn a_selection_the_substrate_refuses_crosses_as_selection_invalid() {
        let mind = worked();
        let undeclared = with(Selection::default(), any_of("nonsense", &["x"]));
        assert!(matches!(refused(&mind, &undeclared), MindRefusal::SelectionInvalid { field, .. } if field == "fields[0].index"));
        let empty = Selection { schemas: Some(vec![]), ..Selection::default() };
        assert!(matches!(refused(&mind, &empty), MindRefusal::SelectionInvalid { field, .. } if field == "schemas"));
        let garbled = Selection { cursor: Some("not a cursor".into()), ..Selection::default() };
        assert!(matches!(refused(&mind, &garbled), MindRefusal::CursorInvalid { .. }));
    }

    /// The header is the default projection and the document the asked-for
    /// one; the document is `view`'s answer, the header its summary.
    #[test]
    fn the_projection_chooses_a_header_or_the_view() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(1)]));
        let q1 = id("question", "Q1");
        let one = Selection { keys: Some(vec![q1.clone()]), ..Selection::default() };
        let page = mind.query(&one).unwrap();
        let expected = view(&mind, K::Question, &q1);
        assert_eq!(page.edges, None, "no hop, no edges");
        assert_eq!(headers(&page), [PipelineDocumentSummary::of(&expected)]);
        assert_eq!(headers(&page)[0].facts, PipelineFacts::Question {
            label: l("Q1"),
            title: "Who owns the state?".into(),
            options: vec![l("A"), l("B")],
            recommended: l("A"),
            raised_in: None,
            asked_on: date(),
        });
        let whole = mind.query(&Selection { projection: "document".into(), ..one }).unwrap();
        assert_eq!(whole.items, PipelinePageItems::Documents(vec![expected]));
    }

    /// One edge list: every role's edge comes out of the hop, typed by the
    /// role and the ends' kinds.
    #[test]
    fn every_role_yields_its_edge() {
        let mut mind = seeded();
        let (q1, q2, r2) = (id("question", "Q1"), id("question", "Q2"), id("ruling", "R2"));
        let (campaign_id, spec, report) = (id("campaign", "self"), id("cut_spec", "cut-1.r1"), id("cut_report", "cut-1.h1"));
        let (verdict_id, f1, f2) = (id("verdict", "cut-1.s1"), id("finding", "cut-1.s1.F1"), id("finding", "cut-1.s1.F2"));
        let fu = id("follow_up", "FU-1");

        let D::Question(mut raised) = question("Q2", &["A", "B"], "A") else { panic!() };
        raised.raised_in = Some(r(K::Campaign, &campaign_id));
        let mut spec_doc = cut_spec("1", 1);
        spec_doc.rulings = vec![s(&r2)];
        spec_doc.questions = vec![s(&q1)];
        let mut report_doc = cut_report("1", 1);
        report_doc.forks = vec![s(&q2)];
        committed(admit(&mut mind, vec![question_n(1), D::Question(raised), D::Ruling(ruling("R2")), D::CutSpec(spec_doc)]));
        committed(admit(&mut mind, vec![D::CutReport(report_doc), follow_up("FU-1", r(K::Question, &q1))]));
        committed(admit(&mut mind, vec![
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&f1, &f2], Some("P1"), &["M1"])]),
            D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed)),
            D::Finding(finding("1", 1, "F2", FindingConfidence::Plausible)),
        ]));
        committed(admit(&mut mind, vec![answering("R1", &q1)]));
        committed(admit(&mut mind, vec![resolution(
            r(K::Finding, &f1),
            ResolutionOutcome::Fixed { commit: sha(), by: Some(r(K::CutReport, &report)) },
        )]));
        committed(admit(&mut mind, vec![resolution(r(K::Finding, &f2), ResolutionOutcome::Deferred { to: r(K::FollowUp, &fu) })]));
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("1", 2)),
            resolution(r(K::CutSpec, &spec), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-1.r2"))])),
        ]));
        let (_, landed) = committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[&q1])]));
        let handoff = landed.iter().find(|write| write.kind == K::HandOff).expect("the hand-off landed").id.0.clone();

        let (n1, f1_closed, f2_closed) = (
            id("resolution", "question.Q1.n1"),
            id("resolution", &format!("finding.{}.n1", f1.rsplit(':').next().unwrap())),
            id("resolution", &format!("finding.{}.n1", f2.rsplit(':').next().unwrap())),
        );
        let spec_closed = id("resolution", "cut_spec.cut-1.r1.n1");
        let spec_r2 = id("cut_spec", "cut-1.r2");
        let r1 = id("ruling", "R1");
        let expected: Vec<(CitationRole, &str, &str)> = vec![
            (CitationRole::RaisedIn, &q2, &campaign_id),
            (CitationRole::Answers, &r1, &q1),
            (CitationRole::Rulings, &spec, &r2),
            (CitationRole::Questions, &spec, &q1),
            (CitationRole::CutSpec, &report, &spec),
            (CitationRole::Forks, &report, &q2),
            (CitationRole::CutReport, &verdict_id, &report),
            (CitationRole::Findings, &verdict_id, &f1),
            (CitationRole::Findings, &verdict_id, &f2),
            (CitationRole::Verdict, &f1, &verdict_id),
            (CitationRole::Verdict, &f2, &verdict_id),
            (CitationRole::Source, &fu, &q1),
            (CitationRole::Subject, &n1, &q1),
            (CitationRole::Subject, &f1_closed, &f1),
            (CitationRole::Subject, &f2_closed, &f2),
            (CitationRole::Subject, &spec_closed, &spec),
            (CitationRole::SupersededBy, &spec_closed, &spec_r2),
            (CitationRole::ResolvedBy, &n1, &r1),
            (CitationRole::ResolvedBy, &f1_closed, &report),
            (CitationRole::DeferredTo, &f2_closed, &fu),
            (CitationRole::Documents, &handoff, &q1),
        ];
        assert_eq!(CitationRole::ALL.len(), 15);
        for role in CitationRole::ALL {
            let selection =
                Selection { cited: Some(Incoming { role: role.name().into(), exists: true }), ..Selection::default() };
            let page = mind.query(&selection).unwrap();
            let edges = page.edges.expect("a hop carries edges");
            let mut found: Vec<(CitationRole, String, String)> =
                edges.iter().map(|edge| (edge.role, edge.from.id.0.clone(), edge.to.id.0.clone())).collect();
            found.sort();
            let mut wanted: Vec<(CitationRole, String, String)> = expected
                .iter()
                .filter(|(expected_role, _, _)| *expected_role == role)
                .map(|(role, from, to)| (*role, from.to_string(), to.to_string()))
                .collect();
            wanted.sort();
            assert!(!wanted.is_empty(), "{role:?} has a fixture");
            if role == CitationRole::Subject {
                // The hand-off also derived a withdrawal of the stewardship.
                assert!(wanted.iter().all(|edge| found.contains(edge)), "{role:?}: {found:?}");
            } else {
                assert_eq!(found, wanted, "{role:?}");
            }
        }
    }

    /// R-B: a subject's history is one `cites` hop over its resolutions, in
    /// ordinal order, withdrawn ones included with their status. Ten cycles
    /// put n10 after n9, which a string order would not.
    #[test]
    fn a_subjects_history_is_one_cites_hop() {
        let mut mind = seeded();
        let q1 = id("question", "Q1");
        committed(admit(&mut mind, vec![question_n(1), follow_up("FU-1", r(K::Question, &q1))]));
        for cycle in 1..=10 {
            committed(admit(&mut mind, vec![answering(&format!("R{cycle}"), &q1)]));
            if cycle < 10 {
                let closure = id("resolution", &format!("question.Q1.n{cycle}"));
                committed(admit(&mut mind, vec![resolution(r(K::Resolution, &closure), withdrawn())]));
            }
        }
        let history = Selection { cites: Some(cites(K::Question, &q1, Some("subject"))), ..of_kinds(&[K::Resolution]) };
        let page = mind.query(&history).unwrap();
        let expected: Vec<String> = (1..=10).map(|cycle| id("resolution", &format!("question.Q1.n{cycle}"))).collect();
        assert_eq!(ids(&page), expected, "ordinal order, so n10 follows n9");
        let statuses: Vec<bool> =
            headers(&page).iter().map(|header| matches!(header.status, PipelineStatusSummary::Resolved { .. })).collect();
        assert_eq!(statuses, [vec![true; 9], vec![false]].concat(), "the nine withdrawn stay, closed; the last is standing");
        let edges = page.edges.unwrap();
        assert_eq!(edges.len(), 10);
        assert!(edges.iter().all(|edge| edge.role == CitationRole::Subject && edge.to.id.0 == q1));

        // Without the role every kind that cites Q1 answers: the ten
        // resolutions by `subject`, the ten rulings by `answers` and the
        // follow-up by `source`.
        let unroled = Selection { cites: Some(cites(K::Question, &q1, None)), ..Selection::default() };
        let page = mind.query(&unroled).unwrap();
        assert_eq!(page.matched, 21);
        assert!(page.edges.unwrap().iter().any(|edge| edge.role == CitationRole::Answers));
    }

    /// R-B's second relation: a repo's assignments to one mind are one
    /// selection over its stewardships.
    #[test]
    fn a_repos_assignments_are_one_selection() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let assignments = |root: &str, repo_name: &str| {
            let selection = with(with(of_kinds(&[K::Stewardship]), any_of("root", &[root])), any_of("repo", &[repo_name]));
            mind.query(&selection).unwrap()
        };
        let mine = assignments(INSTANCE, REPO);
        assert_eq!(ids(&mine), vec![format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1")]);
        assert!(
            matches!(headers(&mine)[0].status, PipelineStatusSummary::Resolved { .. }),
            "the assignment the hand-off withdrew stays, closed"
        );
        // The mind holds its own side of a hand-off only, so the receiving
        // instance's root selects nothing here.
        assert_eq!(assignments(OTHER_INSTANCE, REPO).matched, 0);
        assert_eq!(assignments(INSTANCE, OTHER_REPO).matched, 0, "another repo is a value in the domain that matches nothing");
    }

    /// The five lists of open work, each one selection (F1's relations), over
    /// a fixture with the exact-id citation (V13L/V14L: cut 1 is reported, cut
    /// 10 is not, and the ids share a prefix) and the superseded revision
    /// (V13: it is not open).
    #[test]
    fn open_work_is_five_selections() {
        let mut mind = seeded();
        committed(admit(&mut mind, elsewhere("Q1")));
        committed(admit(&mut mind, vec![
            question_n(1),
            question_n(2),
            D::CutSpec(cut_spec("1", 1)),
            D::CutSpec(cut_spec("2", 1)),
            D::CutSpec(cut_spec("3", 1)),
            D::CutSpec(cut_spec("10", 1)),
        ]));
        let (q1, q2) = (id("question", "Q1"), id("question", "Q2"));
        committed(admit(&mut mind, vec![
            D::CutReport(cut_report("1", 1)),
            D::CutReport(cut_report("3", 1)),
            follow_up("FU-1", r(K::Question, &q1)),
            follow_up("FU-2", r(K::Question, &q1)),
        ]));
        let (f1, f2) = (id("finding", "cut-1.s1.F1"), id("finding", "cut-1.s1.F2"));
        committed(admit(&mut mind, vec![
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&f1, &f2], Some("P1"), &["M1"])]),
            D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed)),
            D::Finding(finding("1", 1, "F2", FindingConfidence::Plausible)),
        ]));
        committed(admit(&mut mind, vec![
            answering("R1", &q2),
            resolution(r(K::Finding, &f2), ResolutionOutcome::Recorded { reason: "noted".into() }),
            resolution(r(K::FollowUp, &id("follow_up", "FU-2")), withdrawn()),
            D::CutSpec(cut_spec("3", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-3.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-3.r2"))])),
        ]));

        let mine = |kinds: &[K]| with(of_kinds(kinds), any_of("root", &[CAMPAIGN]));
        let standing = |selection: Selection| with(selection, any_of("in_force", &["true"]));
        let unnamed = |role: &str, selection: Selection| Selection {
            cited: Some(Incoming { role: role.into(), exists: false }),
            ..selection
        };
        let open = |selection: Selection| ids(&mind.query(&selection).unwrap());

        assert_eq!(open(standing(mine(&[K::Question]))), vec![q1.clone()]);
        assert_eq!(open(standing(mine(&[K::Finding]))), vec![f1]);
        assert_eq!(open(standing(mine(&[K::FollowUp]))), vec![id("follow_up", "FU-1")]);
        assert_eq!(
            open(unnamed("cut_spec", standing(mine(&[K::CutSpec])))),
            vec![id("cut_spec", "cut-10.r1"), id("cut_spec", "cut-2.r1"), id("cut_spec", "cut-3.r2")],
            "cut 1 is reported, cut 3's first revision is superseded, and cut 10 is not cut 1"
        );
        assert_eq!(open(unnamed("cut_report", mine(&[K::CutReport]))), vec![id("cut_report", "cut-3.h1")]);
        // The campaign root is what keeps the other campaign's question out.
        assert_eq!(open(standing(of_kinds(&[K::Question]))).len(), 2);
    }

    fn hit(kind: K, key: &str, score: f32) -> (PipelineRef, f32) {
        (r(kind, key), score)
    }

    fn asking(top_k: u32) -> SemanticQuery {
        SemanticQuery { text: "who owns the state".into(), top_k }
    }

    fn standing(selection: Selection) -> Selection {
        with(selection, any_of("in_force", &["true"]))
    }

    /// A mind with every way a document stops being in force: Q1 answered
    /// (resolved), Q2 answered and the answer withdrawn (its resolution is
    /// resolved, the question in force again), R3 superseded by R4.
    struct Shelf {
        mind: Mind<MemoryStore>,
        q1: String,
        q2: String,
        withdrawn_resolution: String,
        r3: String,
        r4: String,
    }

    fn shelf() -> Shelf {
        let mut mind = seeded();
        let (q1, q2) = (id("question", "Q1"), id("question", "Q2"));
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), question("Q2", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![answering("R1", &q1)]));
        committed(admit(&mut mind, vec![answering("R2", &q2)]));
        let withdrawn_resolution = id("resolution", "question.Q2.n1");
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawn_resolution), withdrawn())]));
        let (r3, r4) = (id("ruling", "R3"), id("ruling", "R4"));
        committed(admit(&mut mind, vec![D::Ruling(ruling("R3")), D::Ruling(ruling("R4"))]));
        committed(admit(&mut mind, vec![resolution(r(K::Ruling, &r3), superseded(&[r(K::Ruling, &r4)]))]));
        Shelf { mind, q1, q2, withdrawn_resolution, r3, r4 }
    }

    /// The index never deletes and carries no status, so a hit is a candidate
    /// and nothing more. Here it names a resolved question, a withdrawn
    /// resolution, a superseded ruling, a document the mind does not hold and
    /// a real id under the wrong kind, all scoring above the two documents
    /// that stand; the standing ones are the whole answer.
    #[test]
    fn a_hit_is_judged_by_the_mind_and_never_by_the_index() {
        let shelf = shelf();
        let hits = [
            hit(K::Question, &shelf.q1, 0.9),
            hit(K::Resolution, &shelf.withdrawn_resolution, 0.8),
            hit(K::Ruling, &shelf.r3, 0.7),
            hit(K::Question, &id("question", "Q99"), 0.6),
            hit(K::Question, &shelf.r4, 0.55),
            hit(K::Ruling, &shelf.r4, 0.5),
            hit(K::Question, &shelf.q2, 0.4),
        ];
        let ranked = shelf.mind.rank(&Selection::default(), &asking(10), &hits).unwrap();
        assert_eq!(
            ranked.dropped,
            vec![r(K::Question, &id("question", "Q99")), r(K::Question, &shelf.r4)],
            "the ids the mind holds nothing for come back, in the order the index gave them, and no held id does"
        );
        let everything = ranked.page;
        assert_eq!(ids(&everything), vec![shelf.q1.clone(), shelf.withdrawn_resolution.clone(), shelf.r3.clone(), shelf.r4.clone(), shelf.q2.clone()],
            "without in_force the selection has not asked about standing, and the unheld and mis-kinded hits are gone");
        assert_eq!(everything.matched, 5);

        let page = shelf.mind.rank(&standing(Selection::default()), &asking(10), &hits).unwrap().page;
        assert_eq!(ids(&page), vec![shelf.r4.clone(), shelf.q2.clone()]);
        assert_eq!((page.matched, page.next), (2, None));
        assert_eq!(page.as_of, receipt::head(&shelf.mind).unwrap());

        let none = shelf.mind.rank(&standing(Selection::default()), &asking(10), &[hit(K::Question, &id("question", "Q99"), 1.0)]).unwrap().page;
        assert_eq!((none.matched, ids(&none)), (0, Vec::<String>::new()), "an empty candidate set is an empty page, not a refusal");
        assert_eq!(shelf.mind.rank(&standing(Selection::default()), &asking(10), &[]).unwrap().page.matched, 0);
    }

    /// The substrate answers 200 rows and cuts by ordinal, so 250 candidates
    /// must be cut by score first: the 200 best are answered, and the 50 lowest
    /// scores are the ones that fall away, whichever of them was admitted first.
    /// An id the mind does not hold does not take a place among the 200.
    #[test]
    fn more_candidates_than_the_substrate_answers_are_cut_by_score_not_by_ordinal() {
        let mut mind = seeded();
        for start in (0..250).step_by(crate::admission::BATCH_MAX) {
            committed(admit(&mut mind, (start..(start + crate::admission::BATCH_MAX as u32).min(250)).map(question_n).collect()));
        }
        let key = |n: u32| id("question", &format!("Q{n}"));
        // Question n scores n, so the later a question was admitted the better it ranks.
        let mut hits: Vec<(PipelineRef, f32)> = (0..250).map(|n| hit(K::Question, &key(n), n as f32)).collect();
        // Twenty ids the mind holds nothing for, above every real score.
        hits.extend((0..20).map(|n| hit(K::Question, &id("question", &format!("Gone{n}")), 1000.0 + n as f32)));
        let ranked = mind.rank(&Selection::default(), &asking(200), &hits).unwrap();
        let expected: Vec<String> = (50..250).rev().map(key).collect();
        assert_eq!(ids(&ranked.page), expected, "the 200 best, best first");
        assert_eq!(ranked.page.matched, 200);
        assert_eq!(ranked.dropped.len(), 20);
    }

    /// The order is the score's, highest first, and ordinal only settles a
    /// tie; the page is cut to the selection's limit and to top_k, and says
    /// how many matched.
    #[test]
    fn ranked_results_are_ordered_by_score_then_ordinal_and_cut_to_the_limit() {
        let shelf = shelf();
        let by_ordinal = [hit(K::Question, &shelf.q2, 0.4), hit(K::Ruling, &shelf.r4, 0.5)];
        assert_eq!(ids(&shelf.mind.rank(&standing(Selection::default()), &asking(10), &by_ordinal).unwrap().page), vec![shelf.r4.clone(), shelf.q2.clone()],
            "Q2 was admitted first, and the better score still comes first");
        let tied = [hit(K::Ruling, &shelf.r4, 0.5), hit(K::Question, &shelf.q2, 0.5)];
        assert_eq!(ids(&shelf.mind.rank(&standing(Selection::default()), &asking(10), &tied).unwrap().page), vec![shelf.q2.clone(), shelf.r4.clone()],
            "a tie falls to the earlier ordinal");

        let limited = Selection { limit: Some(1), ..standing(Selection::default()) };
        let page = shelf.mind.rank(&limited, &asking(10), &by_ordinal).unwrap().page;
        assert_eq!((ids(&page), page.matched, page.next), (vec![shelf.r4.clone()], 2, None), "the limit keeps the best, not the earliest");
        let page = shelf.mind.rank(&standing(Selection::default()), &asking(1), &by_ordinal).unwrap().page;
        assert_eq!((ids(&page), page.matched), (vec![shelf.r4.clone()], 2), "top_k bounds the page too");
    }

    /// Membership is the selection's alone: a kind, a key allowlist and a
    /// citation predicate each cut the candidates exactly as they cut a plain
    /// query, and the citation predicate sees the whole mind, not only the
    /// candidates (Q2 is named by a resolution that is not a hit).
    #[test]
    fn the_selections_own_predicates_decide_which_candidates_stay() {
        let shelf = shelf();
        let hits = [hit(K::Question, &shelf.q2, 0.4), hit(K::Ruling, &shelf.r4, 0.5)];
        assert_eq!(ids(&shelf.mind.rank(&of_kinds(&[K::Ruling]), &asking(10), &hits).unwrap().page), vec![shelf.r4.clone()]);
        let keyed = Selection { keys: Some(vec![shelf.q2.clone(), shelf.q1.clone()]), ..Selection::default() };
        assert_eq!(ids(&shelf.mind.rank(&keyed, &asking(10), &hits).unwrap().page), vec![shelf.q2.clone()], "keys intersect the candidates");
        let disjoint = Selection { keys: Some(vec![shelf.q1.clone()]), ..Selection::default() };
        assert_eq!(shelf.mind.rank(&disjoint, &asking(10), &hits).unwrap().page.matched, 0);
        let unnamed = Selection { cited: Some(Incoming { role: "subject".into(), exists: false }), ..Selection::default() };
        let plain = ids(&shelf.mind.query(&Selection { keys: Some(vec![shelf.q2.clone(), shelf.r4.clone()]), ..unnamed.clone() }).unwrap());
        assert_eq!(plain, vec![shelf.r4.clone()], "no resolution names R4 as its subject, and one names Q2");
        assert_eq!(ids(&shelf.mind.rank(&unnamed, &asking(10), &hits).unwrap().page), plain, "the same answer as the plain query over the same rows");
        assert_eq!(ids(&shelf.mind.rank(&Selection { projection: "document".into(), ..of_kinds(&[K::Ruling]) }, &asking(10), &hits).unwrap().page), vec![shelf.r4.clone()]);
    }

    /// A citation hop is answered over the whole mind, and its edges follow the
    /// ranked page: after the cut to the limit only the edges anchored on a kept
    /// row remain, in the page's order and not the evaluator's. An empty cursor
    /// is no cursor.
    #[test]
    fn a_hop_keeps_only_the_edges_of_the_ranked_page_in_its_order() {
        let shelf = shelf();
        let named = Selection { cited: Some(Incoming { role: "subject".into(), exists: true }), ..of_kinds(&[K::Question]) };
        let hits = [hit(K::Question, &shelf.q2, 0.9), hit(K::Question, &shelf.q1, 0.4)];
        let both = shelf.mind.rank(&named, &asking(10), &hits).unwrap().page;
        assert_eq!(ids(&both), vec![shelf.q2.clone(), shelf.q1.clone()], "Q1 was admitted first and scored lower");
        let edges = both.edges.expect("a hop carries its edges");
        let targets: Vec<&str> = edges.iter().map(|edge| edge.to.id.0.as_str()).collect();
        assert_eq!(targets, [shelf.q2.as_str(), shelf.q1.as_str()], "the edges follow the page order");
        assert!(edges.iter().all(|edge| edge.from.kind == K::Resolution && edge.role == CitationRole::Subject));

        let limited = shelf.mind.rank(&Selection { limit: Some(1), ..named.clone() }, &asking(10), &hits).unwrap().page;
        assert_eq!(ids(&limited), vec![shelf.q2.clone()]);
        let kept = limited.edges.unwrap();
        assert_eq!(kept.len(), 1, "the dropped row's edge is dropped with it");
        assert_eq!(kept[0].to.id.0, shelf.q2);
        assert_eq!(kept[0].from.id.0, id("resolution", "question.Q2.n1"));

        let empty_cursor = Selection { cursor: Some(String::new()), ..standing(Selection::default()) };
        let with = shelf.mind.rank(&empty_cursor, &asking(10), &[hit(K::Ruling, &shelf.r4, 0.5)]).unwrap().page;
        assert_eq!(ids(&with), vec![shelf.r4.clone()]);
    }


    /// A ranked answer cannot be resumed, so a cursor is refused by name, and
    /// so is a query that cannot be asked, before any hit is looked at.
    #[test]
    fn a_semantic_query_that_cannot_be_ranked_is_refused_by_name() {
        let shelf = shelf();
        let hits = [hit(K::Ruling, &shelf.r4, 0.5)];
        let with_cursor = Selection { cursor: Some("anything".into()), ..Selection::default() };
        fn on(field: &str, refusal: &MindRefusal) -> bool {
            matches!(refusal, MindRefusal::SelectionInvalid { field: f, .. } if f == field)
        }
        for refusal in [
            shelf.mind.rank(&with_cursor, &asking(10), &hits).unwrap_err(),
            shelf.mind.check_semantic(&with_cursor, &asking(10)).unwrap_err(),
        ] {
            assert!(on("cursor", &refusal), "{refusal:?}");
        }
        let reversed = Selection { descending: true, ..Selection::default() };
        for refusal in [
            shelf.mind.rank(&reversed, &asking(10), &hits).unwrap_err(),
            shelf.mind.check_semantic(&reversed, &asking(10)).unwrap_err(),
        ] {
            assert!(on("descending", &refusal), "a ranked answer has no order to reverse: {refusal:?}");
        }
        assert!(on("semantic.top_k", &shelf.mind.rank(&Selection::default(), &asking(0), &hits).unwrap_err()));
        assert!(on("semantic.top_k", &shelf.mind.rank(&Selection::default(), &asking(201), &hits).unwrap_err()));
        let blank = SemanticQuery { text: "  ".into(), top_k: 3 };
        assert!(on("semantic.text", &shelf.mind.rank(&Selection::default(), &blank, &hits).unwrap_err()));
        let malformed = with(Selection::default(), any_of("root", &["a/../b"]));
        assert_eq!(shelf.mind.check_semantic(&malformed, &asking(3)), shelf.mind.query(&malformed).map(drop), "the same door as a plain query");
        assert!(shelf.mind.check_semantic(&standing(Selection::default()), &asking(3)).is_ok());
    }

    /// A summary is bounded by a named constant whatever the document's lists
    /// hold: a cut spec at every leaf bound, admitted by a batch whose
    /// provenance is at its bounds too, and closed by a supersession naming
    /// eight successors. The measure is the whole summary, admission facts and
    /// status included.
    #[test]
    fn a_summary_is_bounded_whatever_the_lists() {
        let mut mind = seeded();
        let long = |unit: &str, bytes: usize| unit.repeat(bytes);
        // depends_on holds cut labels, each a cut that exists (its spec key stays within 64 bytes).
        let dependencies: Vec<String> = (0..8).map(|n| format!("{n}{}", long("d", 55))).collect();
        let dependency_specs: Vec<D> = dependencies.iter().map(|label| D::CutSpec(cut_spec(label, 1))).collect();
        let batch = crate::admission::PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::SelfFaculty),
            documents: dependency_specs,
        };
        committed(mind.admit(batch, now()));
        let mut maximal = cut_spec("1", 1);
        maximal.title = long("t", 200).as_str().into();
        maximal.branch = s(&long("b", 200));
        maximal.base = Sha("a".repeat(40));
        maximal.depends_on = dependencies.iter().map(|label| s(label)).collect();
        let maximal_provenance = || PipelineProvenance {
            faculty: Faculty::SelfFaculty,
            agent: s(&long("a", 200)),
            session: s(&long("s", 200)),
            tool: s(&long("t", 200)),
        };
        let batch = crate::admission::PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: maximal_provenance(),
            documents: vec![D::CutSpec(maximal)],
        };
        committed(mind.admit(batch, now()));
        // A ruling closed by a supersession naming eight successors, with the
        // closing record's rationale at its bound: the widest status the
        // summary carries, and the prose it must leave behind.
        let mut rulings: Vec<D> = (0..=8).map(|n| D::Ruling(ruling(&format!("S{n}")))).collect();
        let successors: Vec<PipelineRef> = (1..=8).map(|n| r(K::Ruling, &id("ruling", &format!("S{n}")))).collect();
        rulings.push(D::Resolution(PipelineResolution {
            subject: r(K::Ruling, &id("ruling", "S0")),
            sequence: 1,
            outcome: superseded(&successors),
            rationale: long("r", 4000).as_str().into(),
            resolved_on: date(),
        }));
        let batch = crate::admission::PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: maximal_provenance(),
            documents: rulings,
        };
        committed(mind.admit(batch, now()));

        let page = mind.query(&Selection::default()).unwrap();
        let sizes: Vec<usize> =
            headers(&page).iter().map(|summary| rmp_serde::to_vec_named(summary).unwrap().len()).collect();
        let widest = *sizes.iter().max().unwrap();
        assert!(widest > 1_500, "the fixture is at its bounds, not a small one: {widest}");
        assert!(widest <= SUMMARY_MAX_BYTES, "{widest} bytes over {SUMMARY_MAX_BYTES}");
        let closed = headers(&page).iter().find(|summary| summary.id.id.0 == id("ruling", "S0")).unwrap();
        assert!(matches!(&closed.status, PipelineStatusSummary::Resolved { outcome: ResolutionOutcome::Superseded { by }, .. } if by.len() == 8));
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
