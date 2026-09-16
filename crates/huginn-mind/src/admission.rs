//! Admission: the single door into a mind.
//!
//! `admit` prepares typed documents through the leaf and calls
//! `admit_prepared`, the one commit path for Cut 8, Cut 10's sink, Cut 12's
//! hand-off and import, and Cut 13's tools. The steps run in order and the
//! first failing step is the outcome:
//!
//! - A1 the batch names this mind; A2 one to sixty-four envelopes;
//! - A3 every envelope passes the leaf's bounds, formats and key
//!   recomputation; A4 identities are unique in the batch; A5 every document
//!   carrying an instance names this mind;
//! - A6 an empty mind's first batch carries exactly one `instance` document,
//!   and a batch carrying one derives the epoch record;
//! - A7 every reference names a document present in the image or the batch,
//!   looked up by `(kind, id)`; the ones resolved in the image become the
//!   receipt's strong reads;
//! - the derived writes (a ruling's `Answered` resolution, a hand-off's
//!   stewardship withdrawal or assignment) are appended and pass A3-A7;
//! - A8 the per-kind rules, below; A9 an exact replay answers with the stored
//!   receipt, after validation and never before it (ruling 20); A10 no write
//!   collides with the image; A11 the commit.
//!
//! Every cross-field and cross-document rule lives here and nowhere else:
//! the leaf never gains one, the daemon and the client never re-derive one.
//! "In force" is recursive: a document is in force when no resolution that is
//! itself in force names it, so a withdrawn resolution stops closing its
//! subject. It is computed at rule time over image and batch, never stored.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use cultcache_rs::CultCacheEnvelope;
use epiphany_pipeline::{
    ClaimOutcome, FindingConfidence, Line, OrgRepo, PipelineDocument, PipelineKind, PipelineRef, PipelineRefusal,
    PipelineResolution, PipelineStewardship, ResolutionOutcome, RulingAuthority, Short, Slug, pipeline_key,
    validate_pipeline_write_envelope,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mind::{HuginnMindEpoch, Mind, unavailable};
use crate::receipt::{self, CommitOutcome, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The most envelopes one batch may carry.
pub const BATCH_MAX: usize = 64;

/// One admission request: the mind it is for, who asks, and the documents.
/// The leaf's `PipelineDocument` derives neither `Serialize` nor
/// `JsonSchema` at the pinned rev, so this type cannot either; the wire
/// shape is Cut 10's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineAdmissionBatch {
    pub instance: Slug,
    pub provenance: PipelineProvenance,
    pub documents: Vec<PipelineDocument>,
}

/// How admission ended. `Committed.writes` names every document the batch
/// landed, derived writes included; Cut 11 indexes from it after `admit`
/// returns, and never inside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PipelineAdmissionOutcome {
    Committed { receipt_id: String, committed_at: String, writes: Vec<PipelineRef> },
    AlreadyAdmitted { receipt_id: String },
    Refused(MindRefusal),
    Conflict { identities: Vec<PipelineRef> },
}

impl<S: MindStore> Mind<S> {
    /// Validates and prepares every document through the leaf, then admits
    /// the envelopes.
    pub fn admit(&mut self, batch: PipelineAdmissionBatch, now: DateTime<Utc>) -> PipelineAdmissionOutcome {
        let mut envelopes = Vec::with_capacity(batch.documents.len());
        for document in &batch.documents {
            if let Err(refusal) = document.validate() {
                return PipelineAdmissionOutcome::Refused(MindRefusal::Document(refusal));
            }
            match document.prepare(self.cache()) {
                Ok(envelope) => envelopes.push(envelope),
                Err(error) => return PipelineAdmissionOutcome::Refused(unavailable(error)),
            }
        }
        self.admit_prepared(&batch.instance, batch.provenance, envelopes, now)
    }

    /// The single commit path: envelopes prepared by anyone, validated here
    /// whoever prepared them.
    pub fn admit_prepared(
        &mut self,
        instance: &Slug,
        provenance: PipelineProvenance,
        envelopes: Vec<CultCacheEnvelope>,
        now: DateTime<Utc>,
    ) -> PipelineAdmissionOutcome {
        match self.admit_steps(instance, provenance, envelopes, now) {
            Ok(outcome) => outcome,
            Err(refusal) => PipelineAdmissionOutcome::Refused(refusal),
        }
    }

    fn admit_steps(
        &mut self,
        instance: &Slug,
        provenance: PipelineProvenance,
        envelopes: Vec<CultCacheEnvelope>,
        now: DateTime<Utc>,
    ) -> Result<PipelineAdmissionOutcome, MindRefusal> {
        // A1
        if instance != self.instance() {
            return Err(MindRefusal::ForeignInstance { declared: instance.0.clone(), mind: self.instance().0.clone() });
        }
        // A2
        if envelopes.is_empty() || envelopes.len() > BATCH_MAX {
            return Err(MindRefusal::BatchSize { actual: envelopes.len() as u32 });
        }
        let mind = self.instance().clone();
        let mut docs = Docs::from_image(self.envelopes())?;
        // A3, A5, then A4 on push.
        for envelope in envelopes {
            docs.push(stage(envelope, &mind)?)?;
        }
        // A6
        let carries_instance = docs.batch.iter().filter(|staged| staged.kind == PipelineKind::Instance).count() == 1;
        if self.is_empty() && !carries_instance {
            return Err(MindRefusal::MissingIdentity);
        }
        // A7
        let mut strong = BTreeSet::new();
        for staged in &docs.batch {
            resolve(&docs, staged, &mind, &mut strong)?;
        }
        // Derived writes pass A3-A7 themselves.
        let originals = docs.batch.len();
        for document in derive(&docs, &mind) {
            document.validate()?;
            let envelope = document.prepare(self.cache()).map_err(unavailable)?;
            docs.push(stage(envelope, &mind)?)?;
        }
        for staged in &docs.batch[originals..] {
            resolve(&docs, staged, &mind, &mut strong)?;
        }
        // The pins: cited image bytes, and every write.
        let strong_reads = strong
            .iter()
            .filter_map(|(type_id, key)| self.raw_envelope(type_id, key).cloned())
            .collect::<Vec<_>>();
        let mut writes = docs.batch.iter().map(|staged| staged.envelope.clone()).collect::<Vec<_>>();
        if carries_instance {
            writes.push(HuginnMindEpoch::envelope(self.cache())?);
        }
        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, now)?;
        // A8
        for staged in &docs.batch {
            check(&docs, staged, &mind)?;
        }
        // A9
        if let Some(existing) = receipt::replay(self, &candidate)? {
            return Ok(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: existing.receipt_id });
        }
        // A10
        refuse_collisions(&docs)?;
        // A11
        let landed = docs.batch.iter().map(|staged| PipelineRef { kind: staged.kind, id: Short(staged.key.clone()) }).collect();
        match receipt::commit(self, candidate, strong_reads, writes)? {
            CommitOutcome::Committed(receipt) => Ok(PipelineAdmissionOutcome::Committed {
                receipt_id: receipt.receipt_id,
                committed_at: receipt.committed_at,
                writes: landed,
            }),
            CommitOutcome::Conflict(identities) => Ok(PipelineAdmissionOutcome::Conflict { identities }),
        }
    }
}

/// A batch document after A3 and A5: decoded, keyed, and its envelope.
struct Staged {
    kind: PipelineKind,
    key: String,
    document: PipelineDocument,
    envelope: CultCacheEnvelope,
}

/// An image document, decoded.
struct Held {
    kind: PipelineKind,
    key: String,
    document: PipelineDocument,
}

/// The image and the batch, the two places a rule may look.
struct Docs {
    image: Vec<Held>,
    batch: Vec<Staged>,
}

fn kind_of_type(type_id: &str) -> Option<PipelineKind> {
    PipelineKind::ALL.iter().copied().find(|kind| kind.type_id() == type_id)
}

/// The kind segment of a full id, for the one field typed `Short` whose
/// referent may be of any kind.
fn kind_of_id(id: &str) -> Option<PipelineKind> {
    let name = id.split(':').nth(1)?;
    PipelineKind::ALL.iter().copied().find(|kind| kind.name() == name)
}

impl Docs {
    fn from_image(envelopes: &[CultCacheEnvelope]) -> Result<Self, MindRefusal> {
        let mut image = Vec::new();
        for envelope in envelopes {
            let Some(kind) = kind_of_type(&envelope.r#type) else { continue };
            let document = PipelineDocument::decode(envelope).map_err(|refusal| MindRefusal::Unavailable {
                detail: format!("stored document {} does not decode: {refusal}", envelope.key),
            })?;
            image.push(Held { kind, key: envelope.key.clone(), document });
        }
        Ok(Self { image, batch: Vec::new() })
    }

    /// A4: one identity, once.
    fn push(&mut self, staged: Staged) -> Result<(), MindRefusal> {
        if self.batch.iter().any(|other| other.kind == staged.kind && other.key == staged.key) {
            return Err(MindRefusal::IdentityCollision { kind: staged.kind, id: staged.key });
        }
        self.batch.push(staged);
        Ok(())
    }

    fn in_batch(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.batch.iter().find(|staged| staged.kind == kind && staged.key == id).map(|staged| &staged.document)
    }

    fn in_image(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.image.iter().find(|held| held.kind == kind && held.key == id).map(|held| &held.document)
    }

    fn find(&self, kind: PipelineKind, id: &str) -> Option<&PipelineDocument> {
        self.in_batch(kind, id).or_else(|| self.in_image(kind, id))
    }

    fn of_kind(&self, kind: PipelineKind) -> impl Iterator<Item = (&str, &PipelineDocument)> {
        let batch = self.batch.iter().filter(move |staged| staged.kind == kind).map(|staged| (staged.key.as_str(), &staged.document));
        let image = self.image.iter().filter(move |held| held.kind == kind).map(|held| (held.key.as_str(), &held.document));
        batch.chain(image)
    }

    fn resolutions(&self) -> impl Iterator<Item = (&str, &PipelineResolution)> {
        self.of_kind(PipelineKind::Resolution).filter_map(|(key, document)| match document {
            PipelineDocument::Resolution(resolution) => Some((key, resolution)),
            _ => None,
        })
    }

    fn stewardships(&self) -> impl Iterator<Item = (&str, &PipelineStewardship)> {
        self.of_kind(PipelineKind::Stewardship).filter_map(|(key, document)| match document {
            PipelineDocument::Stewardship(stewardship) => Some((key, stewardship)),
            _ => None,
        })
    }

    /// The greatest sequence among a subject's resolutions in image or batch,
    /// `0` when it has none, so the next is `latest + 1`. `own` is the
    /// document being checked, excluded by content identity so that an exact
    /// replay measures the same `latest` the first admission did.
    fn latest_resolution(&self, subject: &PipelineRef, own: Option<&PipelineResolution>) -> u32 {
        self.resolutions()
            .filter(|(_, resolution)| resolution.subject == *subject && own != Some(*resolution))
            .map(|(_, resolution)| resolution.sequence)
            .max()
            .unwrap_or(0)
    }

    /// The same for a repo's assignments to one mind.
    fn latest_stewardship(&self, mind: &Slug, repo: &OrgRepo, own: Option<&PipelineStewardship>) -> u32 {
        self.stewardships()
            .filter(|(_, stewardship)| stewardship.instance == *mind && stewardship.repo == *repo && own != Some(*stewardship))
            .map(|(_, stewardship)| stewardship.sequence)
            .max()
            .unwrap_or(0)
    }

    /// No resolution that is itself in force names the document. The
    /// recursion is the whole rule: a withdrawn resolution stops counting, so
    /// its subject is open again. `key(R)` is strictly longer than the id it
    /// resolves, so the chain is bounded and this terminates.
    fn in_force(&self, kind: PipelineKind, id: &str) -> bool {
        self.in_force_unless(kind, id, |_| false)
    }

    /// In force, ignoring a resolution the caller's own document is or
    /// derives: the ruling that answers a question is what resolves it, the
    /// hand-off that withdraws a stewardship is what resolves that, and a
    /// resolution does not resolve its own subject out from under itself.
    fn in_force_unless(&self, kind: PipelineKind, id: &str, own: impl Fn(&PipelineResolution) -> bool) -> bool {
        !self.resolutions().any(|(key, resolution)| {
            resolution.subject.kind == kind
                && resolution.subject.id.0 == id
                && !own(resolution)
                && self.in_force(PipelineKind::Resolution, key)
        })
    }

    /// Every in-force stewardship of `(mind, repo)`, ignoring the withdrawal
    /// `hand_off_key` derives. At most one is in force once the stewardship
    /// row holds, which is why `stewardship_of` needs no tie-break.
    fn stewardships_of(&self, mind: &Slug, repo: &OrgRepo, hand_off_key: Option<&str>) -> Vec<(&str, &PipelineStewardship)> {
        self.stewardships()
            .filter(|(key, stewardship)| {
                stewardship.instance == *mind
                    && stewardship.repo == *repo
                    && self.in_force_unless(PipelineKind::Stewardship, key, |resolution| {
                        matches!(&resolution.outcome, ResolutionOutcome::Withdrawn { reason } if Some(reason.0.as_str()) == hand_off_key)
                    })
            })
            .collect()
    }

    /// The in-force stewardship of `(mind, repo)`, if any.
    fn stewardship_of(&self, mind: &Slug, repo: &OrgRepo, hand_off_key: Option<&str>) -> Option<(&str, &PipelineStewardship)> {
        self.stewardships_of(mind, repo, hand_off_key).first().copied()
    }
}

/// A3 through the leaf, then A5.
fn stage(envelope: CultCacheEnvelope, mind: &Slug) -> Result<Staged, MindRefusal> {
    validate_pipeline_write_envelope(&envelope)?;
    let document = PipelineDocument::decode(&envelope)?;
    refuse_foreign_instance(&document, mind)?;
    Ok(Staged { kind: document.kind(), key: envelope.key.clone(), document, envelope })
}

/// A5: every document carrying an instance field names this mind; a hand-off
/// names it on one side.
fn refuse_foreign_instance(document: &PipelineDocument, mind: &Slug) -> Result<(), MindRefusal> {
    let foreign = |declared: &Slug| MindRefusal::ForeignInstance { declared: declared.0.clone(), mind: mind.0.clone() };
    match document {
        PipelineDocument::Instance(value) if value.instance != *mind => Err(foreign(&value.instance)),
        PipelineDocument::Stewardship(value) if value.instance != *mind => Err(foreign(&value.instance)),
        PipelineDocument::HandOff(value) if value.from_instance != *mind && value.to_instance != *mind => {
            Err(foreign(&value.from_instance))
        }
        _ => Ok(()),
    }
}

/// What a missing referent is called, by the field that cited it.
enum Missing {
    Reference,
    SpecOfReport(String),
    Supersessor,
}

struct Reference {
    kind: PipelineKind,
    id: String,
    missing: Missing,
}

fn reference(kind: PipelineKind, id: &str) -> Reference {
    Reference { kind, id: id.to_string(), missing: Missing::Reference }
}

fn referenced(pipeline_ref: &PipelineRef) -> Reference {
    reference(pipeline_ref.kind, &pipeline_ref.id.0)
}

fn outcome_references(outcome: &ResolutionOutcome) -> Vec<Reference> {
    match outcome {
        ResolutionOutcome::Superseded { by } => by
            .iter()
            .map(|supersessor| Reference { kind: supersessor.kind, id: supersessor.id.0.clone(), missing: Missing::Supersessor })
            .collect(),
        ResolutionOutcome::Answered { by } => vec![referenced(by)],
        ResolutionOutcome::Fixed { by: Some(by), .. } => vec![referenced(by)],
        ResolutionOutcome::Deferred { to } => vec![referenced(to)],
        ResolutionOutcome::Fixed { by: None, .. } | ResolutionOutcome::Recorded { .. } | ResolutionOutcome::Withdrawn { .. } => {
            vec![]
        }
    }
}

/// A7's field list: every `PipelineRef` and every full-id `Short` field.
/// `Fixed.commit` and `ForeignRef` are syntax only. A hand-off's documents
/// are cited on the source side only; the receiving side imports them
/// afterwards (Cut 12).
fn references(staged: &Staged, mind: &Slug) -> Result<Vec<Reference>, MindRefusal> {
    use PipelineDocument as D;
    use PipelineKind as K;
    let mut refs = Vec::new();
    match &staged.document {
        D::Question(question) => refs.extend(question.raised_in.as_ref().map(referenced)),
        D::Ruling(ruling) => refs.extend(ruling.answers.as_ref().map(|question| reference(K::Question, &question.0))),
        D::CutSpec(spec) => {
            refs.extend(spec.rulings.iter().map(|ruling| reference(K::Ruling, &ruling.0)));
            refs.extend(spec.questions.iter().map(|question| reference(K::Question, &question.0)));
        }
        D::CutReport(report) => {
            refs.push(Reference {
                kind: K::CutSpec,
                id: report.cut_spec.0.clone(),
                missing: Missing::SpecOfReport(staged.key.clone()),
            });
            refs.extend(report.forks.iter().map(|question| reference(K::Question, &question.0)));
        }
        D::Verdict(verdict) => {
            refs.push(reference(K::CutReport, &verdict.cut_report.0));
            refs.extend(verdict.claims.iter().flat_map(|claim| claim.findings.iter()).map(|finding| reference(K::Finding, &finding.0)));
        }
        D::Finding(finding) => refs.push(reference(K::Verdict, &finding.verdict.0)),
        D::FollowUp(follow_up) => refs.push(referenced(&follow_up.source)),
        D::Resolution(resolution) => {
            refs.push(referenced(&resolution.subject));
            refs.extend(outcome_references(&resolution.outcome));
        }
        D::HandOff(hand_off) if hand_off.from_instance == *mind => {
            for document in &hand_off.documents {
                let kind = kind_of_id(&document.0).ok_or_else(|| PipelineRefusal::InvalidFormat {
                    field: "hand_off.documents".into(),
                    value: document.0.clone(),
                })?;
                refs.push(reference(kind, &document.0));
            }
        }
        D::Campaign(_) | D::Target(_) | D::Instance(_) | D::Stewardship(_) | D::HandOff(_) => {}
    }
    Ok(refs)
}

/// A7: every referent is in the batch or the image; the image ones are the
/// receipt's strong reads.
fn resolve(docs: &Docs, staged: &Staged, mind: &Slug, strong: &mut BTreeSet<(String, String)>) -> Result<(), MindRefusal> {
    for reference in references(staged, mind)? {
        if docs.in_batch(reference.kind, &reference.id).is_some() {
            continue;
        }
        if docs.in_image(reference.kind, &reference.id).is_some() {
            strong.insert((reference.kind.type_id().to_string(), reference.id));
            continue;
        }
        return Err(match reference.missing {
            Missing::Reference => MindRefusal::MissingReference { kind: reference.kind, id: reference.id },
            Missing::SpecOfReport(report) => MindRefusal::CutReportWithoutSpec { report },
            Missing::Supersessor => MindRefusal::UnknownSupersessor { id: reference.id },
        });
    }
    Ok(())
}

/// The writes admission derives from the batch: a ruling that answers
/// derives the `Answered` resolution of its question; a hand-off derives
/// the stewardship withdrawal on its source side and the stewardship
/// assignment on its receiving side. One the batch already carries is not
/// derived again.
fn derive(docs: &Docs, mind: &Slug) -> Vec<PipelineDocument> {
    use PipelineDocument as D;
    use PipelineKind as K;
    let mut derived = Vec::new();
    for staged in &docs.batch {
        match &staged.document {
            D::Ruling(ruling) => {
                if let Some(question) = &ruling.answers {
                    let subject = PipelineRef { kind: K::Question, id: question.clone() };
                    derived.push(D::Resolution(PipelineResolution {
                        sequence: docs.latest_resolution(&subject, None) + 1,
                        subject,
                        outcome: ResolutionOutcome::Answered { by: PipelineRef { kind: K::Ruling, id: Short(staged.key.clone()) } },
                        rationale: ruling.ruling.clone(),
                        resolved_on: ruling.ruled_on.clone(),
                    }));
                }
            }
            D::HandOff(hand_off) => {
                if hand_off.from_instance == *mind
                    && let Some((stewardship_key, _)) = docs.stewardship_of(mind, &hand_off.repo, Some(&staged.key))
                {
                    let subject = PipelineRef { kind: K::Stewardship, id: Short(stewardship_key.to_string()) };
                    derived.push(D::Resolution(PipelineResolution {
                        sequence: docs.latest_resolution(&subject, None) + 1,
                        subject,
                        outcome: ResolutionOutcome::Withdrawn { reason: Line(staged.key.clone()) },
                        rationale: hand_off.reason.clone(),
                        resolved_on: hand_off.handed_on.clone(),
                    }));
                }
                if hand_off.to_instance == *mind {
                    derived.push(D::Stewardship(PipelineStewardship {
                        instance: mind.clone(),
                        sequence: docs.latest_stewardship(mind, &hand_off.repo, None) + 1,
                        repo: hand_off.repo.clone(),
                        assigned_on: hand_off.handed_on.clone(),
                        note: Line(staged.key.clone()),
                    }));
                }
            }
            _ => {}
        }
    }
    derived.retain(|document| !docs.batch.iter().any(|staged| staged.document == *document));
    derived
}

/// A8: the per-kind rules.
fn check(docs: &Docs, staged: &Staged, mind: &Slug) -> Result<(), MindRefusal> {
    use PipelineDocument as D;
    use PipelineKind as K;
    let key = staged.key.as_str();
    match &staged.document {
        D::Campaign(campaign) => {
            if campaign.repos.is_empty() {
                return Err(MindRefusal::EmptyRepos { campaign: key.into() });
            }
            for repo in &campaign.repos {
                if docs.stewardship_of(mind, repo, None).is_none() {
                    return Err(MindRefusal::RepoNotStewarded { repo: repo.0.clone() });
                }
            }
            Ok(())
        }
        D::Target(target) => {
            let predecessor = D::Target(epiphany_pipeline::PipelineTarget { revision: target.revision.wrapping_sub(1), ..target.clone() });
            revision_rule(docs, K::Target, key, target.revision, &predecessor)?;
            unique_labels("target.invariants", target.invariants.iter().map(|invariant| invariant.label.0.as_str()))
        }
        D::Question(question) => {
            if question.options.len() < 2 || !question.options.iter().any(|option| option.label == question.recommended) {
                return Err(MindRefusal::InvalidOptions { question: key.into() });
            }
            unique_labels("question.options", question.options.iter().map(|option| option.label.0.as_str()))
        }
        D::Ruling(ruling) => {
            if ruling.operator_quote.is_some() && ruling.authority != RulingAuthority::Operator {
                return Err(MindRefusal::QuoteWithoutOperator { ruling: key.into() });
            }
            let Some(question_id) = &ruling.answers else { return Ok(()) };
            let Some(D::Question(question)) = docs.find(K::Question, &question_id.0) else {
                return Err(MindRefusal::MissingReference { kind: K::Question, id: question_id.0.clone() });
            };
            let own = |resolution: &PipelineResolution| {
                matches!(&resolution.outcome, ResolutionOutcome::Answered { by } if by.kind == K::Ruling && by.id.0 == key)
            };
            if !docs.in_force_unless(K::Question, &question_id.0, own) {
                return Err(MindRefusal::AlreadyResolved { subject: question_id.0.clone() });
            }
            let choice = ruling.choice.as_ref().map_or("", |choice| choice.0.as_str());
            if !question.options.iter().any(|option| option.label.0 == choice) {
                return Err(MindRefusal::InvalidChoice { ruling: key.into(), choice: choice.into() });
            }
            Ok(())
        }
        D::CutSpec(spec) => {
            let campaign = docs.of_kind(K::Campaign).find_map(|(_, document)| match document {
                D::Campaign(campaign) if campaign.slug == spec.campaign => Some(campaign),
                _ => None,
            });
            let Some(campaign) = campaign else {
                return Err(MindRefusal::MissingReference { kind: K::Campaign, id: spec.campaign.0.clone() });
            };
            if !campaign.repos.contains(&spec.repo) {
                return Err(MindRefusal::RepoNotInCampaign { repo: spec.repo.0.clone() });
            }
            for ruling in &spec.rulings {
                if !docs.in_force(K::Ruling, &ruling.0) {
                    return Err(MindRefusal::CitesResolvedDocument { kind: K::Ruling, id: ruling.0.clone() });
                }
            }
            let predecessor = D::CutSpec(epiphany_pipeline::PipelineCutSpec { revision: spec.revision.wrapping_sub(1), ..spec.clone() });
            revision_rule(docs, K::CutSpec, key, spec.revision, &predecessor)
        }
        D::CutReport(report) => {
            let Some(D::CutSpec(spec)) = docs.find(K::CutSpec, &report.cut_spec.0) else {
                return Err(MindRefusal::CutReportWithoutSpec { report: key.into() });
            };
            if !docs.in_force(K::CutSpec, &report.cut_spec.0) {
                return Err(MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: report.cut_spec.0.clone() });
            }
            if report.repo != spec.repo {
                return Err(MindRefusal::SpecMismatch { field: "repo".into() });
            }
            if report.branch != spec.branch {
                return Err(MindRefusal::SpecMismatch { field: "branch".into() });
            }
            if !report.commits.iter().any(|commit| commit.sha == report.range.head) {
                return Err(MindRefusal::RangeOutsideCommits { head: report.range.head.0.clone() });
            }
            Ok(())
        }
        D::Verdict(verdict) => {
            let Some(D::CutReport(report)) = docs.find(K::CutReport, &verdict.cut_report.0) else {
                return Err(MindRefusal::MissingReference { kind: K::CutReport, id: verdict.cut_report.0.clone() });
            };
            for claim in &verdict.claims {
                let confirmed = claim.findings.iter().filter_map(|finding| docs.find(K::Finding, &finding.0)).any(|document| {
                    matches!(document, D::Finding(finding) if finding.confidence == FindingConfidence::Confirmed)
                });
                match claim.outcome {
                    ClaimOutcome::Falsified if !confirmed => {
                        return Err(MindRefusal::FalsifiedClaimWithoutConfirmedFinding { claim: claim.claim.0.clone() });
                    }
                    ClaimOutcome::Unproven if confirmed => {
                        return Err(MindRefusal::UnprovenClaimWithConfirmedFinding { claim: claim.claim.0.clone() });
                    }
                    _ => {}
                }
                for label in &claim.mutations {
                    if !report.mutations.iter().any(|mutation| mutation.label == *label) {
                        return Err(MindRefusal::UnknownMutationLabel { label: label.0.clone() });
                    }
                }
            }
            for promise in &report.promises {
                let count = verdict.claims.iter().filter(|claim| claim.promise.as_ref() == Some(&promise.label)).count();
                if count != 1 {
                    return Err(MindRefusal::PromiseWithoutVerdict { promise: promise.label.0.clone() });
                }
            }
            Ok(())
        }
        D::Finding(finding) => {
            if finding.evidence.is_empty() || finding.locations.is_empty() {
                return Err(MindRefusal::FindingWithoutEvidence);
            }
            let target = docs.of_kind(K::Target).find_map(|(target_key, document)| match document {
                D::Target(target) if target.campaign == finding.campaign && docs.in_force(K::Target, target_key) => Some(target),
                _ => None,
            });
            for label in &finding.invariants {
                if !target.is_some_and(|target| target.invariants.iter().any(|invariant| invariant.label == *label)) {
                    return Err(MindRefusal::UnknownInvariant { label: label.0.clone() });
                }
            }
            Ok(())
        }
        D::Resolution(resolution) => resolution_rule(docs, resolution),
        D::Stewardship(stewardship) => {
            let expected = docs.latest_stewardship(&stewardship.instance, &stewardship.repo, Some(stewardship)) + 1;
            if stewardship.sequence != expected {
                return Err(MindRefusal::StewardshipOutOfSequence {
                    repo: stewardship.repo.0.clone(),
                    expected,
                    actual: stewardship.sequence,
                });
            }
            if docs.stewardships_of(mind, &stewardship.repo, None).iter().any(|(other, _)| *other != key) {
                return Err(MindRefusal::AlreadyStewarded { repo: stewardship.repo.0.clone() });
            }
            Ok(())
        }
        D::HandOff(hand_off) => {
            if hand_off.from_instance == *mind && docs.stewardship_of(mind, &hand_off.repo, Some(key)).is_none() {
                return Err(MindRefusal::NotStewarded { repo: hand_off.repo.0.clone() });
            }
            Ok(())
        }
        D::FollowUp(_) | D::Instance(_) => Ok(()),
    }
}

/// Revision 1 stands alone; revision N needs a resolution in the batch that
/// supersedes revision N-1 by this document. The predecessor's key is the
/// leaf's, from the same document one revision back.
fn revision_rule(docs: &Docs, kind: PipelineKind, key: &str, revision: u32, predecessor: &PipelineDocument) -> Result<(), MindRefusal> {
    if revision == 1 {
        return Ok(());
    }
    let refusal = || MindRefusal::RevisionWithoutSupersession { kind, revision };
    if revision == 0 {
        return Err(refusal());
    }
    let predecessor_key = pipeline_key(predecessor)?;
    let superseded = docs.batch.iter().any(|staged| {
        matches!(&staged.document, PipelineDocument::Resolution(resolution)
            if resolution.subject.kind == kind
                && resolution.subject.id.0 == predecessor_key
                && matches!(&resolution.outcome, ResolutionOutcome::Superseded { by }
                    if by.iter().any(|supersessor| supersessor.kind == kind && supersessor.id.0 == key)))
    });
    if superseded { Ok(()) } else { Err(refusal()) }
}

fn unique_labels<'a>(field: &str, labels: impl Iterator<Item = &'a str>) -> Result<(), MindRefusal> {
    let mut seen = BTreeSet::new();
    for label in labels {
        if !seen.insert(label) {
            return Err(MindRefusal::DuplicateLabel { field: field.into(), label: label.into() });
        }
    }
    Ok(())
}

fn outcome_name(outcome: &ResolutionOutcome) -> &'static str {
    match outcome {
        ResolutionOutcome::Superseded { .. } => "Superseded",
        ResolutionOutcome::Answered { .. } => "Answered",
        ResolutionOutcome::Fixed { .. } => "Fixed",
        ResolutionOutcome::Deferred { .. } => "Deferred",
        ResolutionOutcome::Recorded { .. } => "Recorded",
        ResolutionOutcome::Withdrawn { .. } => "Withdrawn",
    }
}

/// The resolution matrix: which outcomes a subject kind admits, and of which
/// kinds its referents must be. Admission's, never the leaf's.
///
/// A resolution is itself resolvable, by withdrawal alone: the operator's
/// ruling on Q17 keeps a withdrawn resolution attached to its subject rather
/// than erasing it, and a subject whose resolution is withdrawn may be
/// resolved again at the next sequence, which the leaf's key now carries. The
/// chain stops there: Q19 A caps it at depth two, in `resolution_rule`, since
/// a withdrawal that could itself be withdrawn would leave two closures
/// standing over one subject.
fn matrix(subject: PipelineKind, outcome: &ResolutionOutcome) -> bool {
    use PipelineKind as K;
    use ResolutionOutcome as O;
    let all = |by: &[PipelineRef], kind: K| by.iter().all(|supersessor| supersessor.kind == kind);
    let fits = match (subject, outcome) {
        (K::Target, O::Superseded { by }) => all(by, K::Target),
        (K::Question, O::Answered { by }) => by.kind == K::Ruling,
        (K::Question, O::Withdrawn { .. }) => true,
        (K::Ruling, O::Superseded { by }) => all(by, K::Ruling),
        (K::CutSpec, O::Superseded { by }) => all(by, K::CutSpec),
        (K::CutSpec, O::Withdrawn { .. }) => true,
        (K::Finding, O::Fixed { by, .. }) => by.as_ref().is_none_or(|report| report.kind == K::CutReport),
        (K::Finding, O::Deferred { to }) => to.kind == K::FollowUp,
        (K::Finding, O::Recorded { .. } | O::Withdrawn { .. }) => true,
        (K::FollowUp, O::Fixed { by, .. }) => by.as_ref().is_none_or(|report| report.kind == K::CutReport),
        (K::FollowUp, O::Superseded { by }) => all(by, K::FollowUp),
        (K::FollowUp, O::Withdrawn { .. }) => true,
        (K::Stewardship, O::Superseded { by }) => all(by, K::Stewardship),
        (K::Stewardship, O::Withdrawn { .. }) => true,
        (K::Resolution, O::Withdrawn { .. }) => true,
        _ => false,
    };
    fits
}

/// The resolution row: the sequence, the in-force subject, the matrix, the
/// Q19 cap, a non-empty supersession, every referent in force, and the two
/// coherence rules (an `Answered` ruling answers this question; a cut spec is
/// superseded within its cut).
fn resolution_rule(docs: &Docs, resolution: &PipelineResolution) -> Result<(), MindRefusal> {
    use PipelineDocument as D;
    use PipelineKind as K;
    let subject_kind = resolution.subject.kind;
    let subject_id = resolution.subject.id.0.as_str();
    if let ResolutionOutcome::Superseded { by } = &resolution.outcome && by.is_empty() {
        return Err(MindRefusal::EmptySupersession);
    }
    // The sequence is the writer's and is checked against the image, as a
    // revision is: the previous plus one, counting every record of this
    // subject other than this document itself.
    let expected = docs.latest_resolution(&resolution.subject, Some(resolution)) + 1;
    if resolution.sequence != expected {
        return Err(MindRefusal::ResolutionOutOfSequence {
            subject: subject_id.into(),
            expected,
            actual: resolution.sequence,
        });
    }
    // A8: a subject with a resolution still in force is closed. A withdrawn
    // one is not in force, so the subject is open and this is its next record.
    if !docs.in_force_unless(subject_kind, subject_id, |other| other == resolution) {
        return Err(MindRefusal::AlreadyResolved { subject: subject_id.into() });
    }
    let incompatible = || MindRefusal::IncompatibleResolution { subject_kind, outcome: outcome_name(&resolution.outcome).into() };
    if !matrix(subject_kind, &resolution.outcome) {
        return Err(incompatible());
    }
    // Q19 A: the chain stops at depth two. A withdrawal cannot be withdrawn;
    // to reinstate a closure, resolve the subject again at the next sequence.
    if subject_kind == K::Resolution
        && let Some(D::Resolution(subject)) = docs.find(K::Resolution, subject_id)
        && subject.subject.kind == K::Resolution
    {
        return Err(incompatible());
    }
    for referent in outcome_references(&resolution.outcome) {
        if !docs.in_force(referent.kind, &referent.id) {
            return Err(MindRefusal::CitesResolvedDocument { kind: referent.kind, id: referent.id });
        }
    }
    match &resolution.outcome {
        ResolutionOutcome::Superseded { by } if subject_kind == K::CutSpec => {
            let Some(D::CutSpec(subject)) = docs.find(K::CutSpec, subject_id) else { return Ok(()) };
            for supersessor in by {
                if let Some(D::CutSpec(spec)) = docs.find(K::CutSpec, &supersessor.id.0)
                    && spec.cut != subject.cut
                {
                    return Err(incompatible());
                }
            }
        }
        ResolutionOutcome::Answered { by } => {
            let Some(D::Ruling(ruling)) = docs.find(K::Ruling, &by.id.0) else { return Ok(()) };
            if ruling.answers.as_ref().map(|answers| answers.0.as_str()) != Some(subject_id) {
                return Err(incompatible());
            }
        }
        _ => {}
    }
    Ok(())
}

/// A10: a write whose identity the image already holds. Every kind answers
/// the same way now that a resolution key carries a sequence: the identity is
/// taken. Whether a subject is closed is A8's question, not this one's.
fn refuse_collisions(docs: &Docs) -> Result<(), MindRefusal> {
    for staged in &docs.batch {
        if docs.in_image(staged.kind, &staged.key).is_some() {
            return Err(MindRefusal::IdentityCollision { kind: staged.kind, id: staged.key.clone() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::receipt::{DocumentVersion, Faculty, HuginnCommitReceipt};
    use crate::store::test_stores::{MemoryStore, RefusingStore, SwapCommand};
    use cultcache_rs::DatabaseEntry;
    use epiphany_pipeline::{PIPELINE_SCHEMA_EPOCH, PipelineDocument as D, PipelineKind as K};
    use sha2::{Digest, Sha256};

    fn receipt_count<S: MindStore>(mind: &Mind<S>) -> usize {
        mind.receipts().unwrap().len()
    }

    #[test]
    fn an_empty_store_opens_and_the_first_write_must_carry_the_instance() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        assert!(mind.is_empty());
        assert_eq!(refusal(admit(&mut mind, vec![stewardship(INSTANCE, REPO), campaign(&[REPO])])), MindRefusal::MissingIdentity);
        assert!(store.rows().is_empty());
        let (receipt_id, writes) = committed(admit(&mut mind, vec![instance(INSTANCE), stewardship(INSTANCE, REPO)]));
        assert!(writes.contains(&r(K::Instance, &format!("{INSTANCE}:instance:self"))));
        let receipts = mind.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].receipt_id, receipt_id);
        let identities = receipts[0].writes.iter().map(DocumentVersion::identity).collect::<Vec<_>>();
        assert!(identities.contains(&(K::Instance.type_id(), format!("{INSTANCE}:instance:self").as_str())));
        assert!(identities.contains(&(HuginnMindEpoch::TYPE, PIPELINE_SCHEMA_EPOCH)), "the epoch record is derived: {identities:?}");
        assert!(receipts[0].strong_reads.is_empty());
        // The store now opens again as this instance, and as nothing else.
        assert!(!Mind::open_with(store.clone(), &slug(INSTANCE)).unwrap().is_empty());
        assert!(Mind::open_with(store, &slug(OTHER_INSTANCE)).is_err());
    }

    #[test]
    fn admission_refuses_a_foreign_instance_whatever_the_transport() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let before = store.rows();
        let foreign = MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() };
        let envelopes = vec![prepare(&question("Q1", &["A", "B"], "A"))];
        assert_eq!(
            refusal(mind.admit_prepared(&slug(OTHER_INSTANCE), provenance(Faculty::Hands), envelopes, now())),
            foreign
        );
        assert_eq!(refusal(admit(&mut mind, vec![stewardship(OTHER_INSTANCE, OTHER_REPO)])), foreign);
        assert_eq!(
            refusal(admit(&mut mind, vec![hand_off("a", "b", REPO, &[])])),
            MindRefusal::ForeignInstance { declared: "a".into(), mind: INSTANCE.into() }
        );
        assert_eq!(refusal(admit(&mut mind, vec![instance(OTHER_INSTANCE)])), foreign);
        assert_eq!(store.rows(), before);
    }

    #[test]
    fn two_minds_in_one_state_root_stay_separate() {
        let root = tempfile::tempdir().unwrap();
        let mut yggdrasil = Mind::open(root.path(), &slug(INSTANCE)).unwrap();
        let mut thought_cage = Mind::open(root.path(), &slug(OTHER_INSTANCE)).unwrap();
        assert_ne!(Mind::path_for(root.path(), &slug(INSTANCE)), Mind::path_for(root.path(), &slug(OTHER_INSTANCE)));
        committed(admit(&mut yggdrasil, vec![instance(INSTANCE), stewardship(INSTANCE, REPO)]));
        committed(admit(&mut thought_cage, vec![instance(OTHER_INSTANCE)]));
        let key = format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1");
        assert!(yggdrasil.envelope(K::Stewardship, &key).is_some());
        assert!(thought_cage.envelope(K::Stewardship, &key).is_none());
        assert_eq!(yggdrasil.envelopes().len(), 4);
        assert_eq!(thought_cage.envelopes().len(), 3);
    }

    #[test]
    fn keys_are_recomputed_and_a_forged_key_refuses() {
        let mut mind = seeded();
        let mut envelope = prepare(&question("Q1", &["A", "B"], "A"));
        let expected = envelope.key.clone();
        envelope.key = format!("{expected}-forged");
        let outcome = mind.admit_prepared(&slug(INSTANCE), provenance(Faculty::Hands), vec![envelope.clone()], now());
        assert_eq!(
            refusal(outcome),
            MindRefusal::Document(PipelineRefusal::InvalidIdentity { kind: K::Question, key: envelope.key, expected })
        );
    }

    #[test]
    fn references_must_exist_in_image_or_batch() {
        let mut mind = seeded();
        let absent_question = id("question", "Q9");
        let mut answering = ruling("R1");
        answering.answers = Some(s(&absent_question));
        answering.choice = Some(l("A"));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Ruling(answering)])),
            MindRefusal::MissingReference { kind: K::Question, id: absent_question }
        );
        let absent_finding = id("finding", "cut-1.s1.F9");
        assert_eq!(
            refusal(admit(&mut mind, vec![follow_up("FU-1", r(K::Finding, &absent_finding))])),
            MindRefusal::MissingReference { kind: K::Finding, id: absent_finding }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![claim(ClaimOutcome::Holds, &[], Some("P1"), &[])])])),
            MindRefusal::MissingReference { kind: K::CutReport, id: id("cut_report", "cut-1.h1") }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(K::Question, &id("question", "Q1")), withdrawn())])),
            MindRefusal::MissingReference { kind: K::Question, id: id("question", "Q1") }
        );
        // A well-formed id of another kind is a missing reference of the
        // cited kind, not a wrong-kind refusal.
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let mut spec = cut_spec("1", 1);
        spec.rulings = vec![s(&id("question", "Q1"))];
        assert_eq!(
            refusal(admit(&mut mind, vec![D::CutSpec(spec)])),
            MindRefusal::MissingReference { kind: K::Ruling, id: id("question", "Q1") }
        );
    }

    #[test]
    fn batch_is_all_or_nothing() {
        let store = RefusingStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let before = store.rows();
        let receipts = receipt_count(&mind);
        let mut quoted = ruling("R1");
        quoted.operator_quote = Some("go ahead".into());
        let batch = vec![question("Q1", &["A", "B"], "A"), question("Q2", &["A", "B"], "B"), D::Ruling(quoted)];
        assert_eq!(refusal(admit(&mut mind, batch)), MindRefusal::QuoteWithoutOperator { ruling: id("ruling", "R1") });
        assert_eq!(store.rows(), before);
        assert_eq!(receipt_count(&mind), receipts);

        store.command(SwapCommand::Lose);
        let outcome = admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]);
        assert!(matches!(outcome, PipelineAdmissionOutcome::Conflict { .. }), "{outcome:?}");
        assert_eq!(store.rows(), before);
        assert_eq!(receipt_count(&mind), receipts);

        store.command(SwapCommand::Fail);
        assert!(matches!(refusal(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")])), MindRefusal::Unavailable { .. }));
        assert_eq!(store.rows(), before);

        store.command(SwapCommand::Delegate);
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        assert_eq!(receipt_count(&mind), receipts + 1);
    }

    #[test]
    fn exact_replay_returns_already_admitted_across_provenance() {
        let mut mind = seeded();
        let receipts = receipt_count(&mind);
        let (first, _) = committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let batch = PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Soul),
            documents: vec![question("Q1", &["A", "B"], "A")],
        };
        let later = now() + chrono::Duration::hours(1);
        assert_eq!(mind.admit(batch, later), PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: first.clone() });
        assert_eq!(receipt_count(&mind), receipts + 1);
        // The first write, epoch record and all, replays too.
        let seed_batch = PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Imagination),
            documents: vec![instance(INSTANCE), stewardship(INSTANCE, REPO), campaign(&[REPO]), target(1, &[INVARIANT])],
        };
        assert!(matches!(mind.admit(seed_batch, later), PipelineAdmissionOutcome::AlreadyAdmitted { .. }));
        assert_eq!(receipt_count(&mind), receipts + 1);
    }

    #[test]
    fn a_refused_batch_is_not_answered_from_a_stored_receipt() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::Ruling(ruling("R1"))]));
        let mut spec = cut_spec("1", 1);
        spec.rulings = vec![s(&id("ruling", "R1"))];
        let envelopes = vec![prepare(&D::CutSpec(spec))];
        committed(mind.admit_prepared(&slug(INSTANCE), provenance(Faculty::Imagination), envelopes.clone(), now()));
        committed(admit(&mut mind, vec![
            D::Ruling(ruling("R2")),
            resolution(r(K::Ruling, &id("ruling", "R1")), superseded(&[r(K::Ruling, &id("ruling", "R2"))])),
        ]));
        assert_eq!(
            refusal(mind.admit_prepared(&slug(INSTANCE), provenance(Faculty::Imagination), envelopes, now())),
            MindRefusal::CitesResolvedDocument { kind: K::Ruling, id: id("ruling", "R1") }
        );
    }

    #[test]
    fn a_receipt_names_the_exact_bytes_it_read_and_wrote() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        committed(admit(&mut mind, vec![D::Ruling(ruling("R1"))]));
        let cited = mind.envelope(K::Ruling, &id("ruling", "R1")).unwrap().clone();
        let mut spec = cut_spec("1", 1);
        spec.rulings = vec![s(&id("ruling", "R1"))];
        let (receipt_id, _) = committed(admit(&mut mind, vec![D::CutSpec(spec)]));
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        assert_eq!(receipt.strong_reads, vec![DocumentVersion::from_envelope(&cited)]);
        let written = mind.envelope(K::CutSpec, &id("cut_spec", "cut-1.r1")).unwrap();
        assert_eq!(receipt.writes, vec![DocumentVersion::from_envelope(written)]);
        for version in receipt.strong_reads.iter().chain(&receipt.writes) {
            assert_eq!(version.payload_sha256, format!("{:x}", Sha256::digest(&version.payload_msgpack)));
        }
        let digest = rmp_serde::to_vec_named(&(&receipt.instance, &receipt.strong_reads, &receipt.writes)).unwrap();
        assert_eq!(receipt_id, format!("mind-commit-{:x}", Sha256::digest(&digest)));

        // The same content admitted by another faculty in another mind has
        // the same id: provenance is not digested.
        let mut twin = opened(MemoryStore::new(), INSTANCE);
        seed(&mut twin);
        committed(admit(&mut twin, vec![D::Ruling(ruling("R1"))]));
        let mut spec = cut_spec("1", 1);
        spec.rulings = vec![s(&id("ruling", "R1"))];
        let batch = PipelineAdmissionBatch { instance: slug(INSTANCE), provenance: provenance(Faculty::Soul), documents: vec![D::CutSpec(spec)] };
        let (twin_id, _) = committed(twin.admit(batch, now() + chrono::Duration::days(1)));
        assert_eq!(twin_id, receipt_id);
        let twin_receipt = twin.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == twin_id).unwrap();
        assert_ne!(twin_receipt.provenance, receipt.provenance);
    }

    #[test]
    fn a_document_is_written_once_and_superseded_by_resolution() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::Ruling(ruling("R8"))]));
        let mut rewritten = ruling("R8");
        rewritten.ruling = "A repo owns its mind.".into();
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Ruling(rewritten)])),
            MindRefusal::IdentityCollision { kind: K::Ruling, id: id("ruling", "R8") }
        );
        committed(admit(&mut mind, vec![
            D::Ruling(ruling("R9")),
            resolution(r(K::Ruling, &id("ruling", "R8")), superseded(&[r(K::Ruling, &id("ruling", "R9"))])),
        ]));
        assert_eq!(mind.get(K::Ruling, &id("ruling", "R8")).unwrap(), Some(D::Ruling(ruling("R8"))));
    }

    /// A8: one resolution of a subject is in force at a time, and the next
    /// record of that subject is the next sequence. The two rules are
    /// separate refusals: a second closure at the right sequence is refused
    /// because the first still stands, and one at a taken sequence is refused
    /// before the question of standing arises.
    #[test]
    fn a_subject_with_a_resolution_in_force_refuses_another() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let subject = r(K::Question, &id("question", "Q1"));
        committed(admit(&mut mind, vec![resolution(subject.clone(), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution_n(subject.clone(), 2, ResolutionOutcome::Withdrawn { reason: "again".into() })])),
            MindRefusal::AlreadyResolved { subject: id("question", "Q1") }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(subject.clone(), ResolutionOutcome::Withdrawn { reason: "again".into() })])),
            MindRefusal::ResolutionOutOfSequence { subject: id("question", "Q1"), expected: 2, actual: 1 }
        );
        let mut answering = ruling("R1");
        answering.answers = Some(s(&id("question", "Q1")));
        answering.choice = Some(l("A"));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Ruling(answering)])),
            MindRefusal::AlreadyResolved { subject: id("question", "Q1") }
        );
    }

    /// Q17 B and Q19 A together: withdrawing a resolution reopens its subject
    /// without erasing the record. The withdrawn closure is still readable by
    /// key, the subject takes its next resolution at the next sequence, and
    /// the chain stops at depth two -- a withdrawal cannot be withdrawn.
    #[test]
    fn a_withdrawn_resolution_reopens_its_subject_and_stays_readable() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let subject = r(K::Question, &id("question", "Q1"));
        let first = id("resolution", "question.Q1.n1");
        committed(admit(&mut mind, vec![resolution(subject.clone(), withdrawn())]));

        // The withdrawal of that closure: one nesting out, under the
        // resolution it withdraws.
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &first), withdrawn())]));
        let withdrawal = id("resolution", "resolution.question.Q1.n1.n1");
        assert!(mind.get(K::Resolution, &withdrawal).unwrap().is_some());

        // The subject is open again, and its next record is `n2`. The first
        // one is still there, outcome and all: this is a log, not an erasure.
        committed(admit(&mut mind, vec![resolution_n(subject.clone(), 2, withdrawn())]));
        let Some(D::Resolution(kept)) = mind.get(K::Resolution, &first).unwrap() else { panic!() };
        assert_eq!(kept.outcome, withdrawn());
        assert_eq!(kept.sequence, 1);

        // `n2` stands, so a third is refused, and the withdrawal cannot
        // itself be withdrawn to re-raise `n1` over it (Q19 A).
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution_n(subject, 3, withdrawn())])),
            MindRefusal::AlreadyResolved { subject: id("question", "Q1") }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawal), withdrawn())])),
            MindRefusal::IncompatibleResolution { subject_kind: K::Resolution, outcome: "Withdrawn".into() }
        );

        // And a ruling may answer a question whose first closure was
        // withdrawn: the derived resolution takes the next sequence.
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![resolution(r(K::Question, &id("question", "Q1")), withdrawn())]));
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &first), withdrawn())]));
        let mut answering = ruling("R1");
        answering.answers = Some(s(&id("question", "Q1")));
        answering.choice = Some(l("A"));
        let (_, writes) = committed(admit(&mut mind, vec![D::Ruling(answering)]));
        assert_eq!(writes, vec![
            r(K::Ruling, &id("ruling", "R1")),
            r(K::Resolution, &id("resolution", "question.Q1.n2")),
        ]);
    }

    /// Q18 A and Q20 A at admission: a repo is stewarded by one record at a
    /// time on a mind, and a repo handed away and handed back is a second
    /// record rather than a collision or an overwrite.
    #[test]
    fn a_repo_is_stewarded_once_at_a_time_and_again_after_a_transfer() {
        let mut mind = seeded();
        let first = format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1");
        assert_eq!(
            refusal(admit(&mut mind, vec![stewardship_n(INSTANCE, REPO, 2)])),
            MindRefusal::AlreadyStewarded { repo: REPO.into() }
        );
        let D::Stewardship(mut retaken) = stewardship(INSTANCE, REPO) else { panic!() };
        retaken.note = "assigned again".into();
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Stewardship(retaken)])),
            MindRefusal::StewardshipOutOfSequence { repo: REPO.into(), expected: 2, actual: 1 }
        );

        // Handing the repo away withdraws the assignment, and the withdrawal
        // is a resolution of that record, sequence and all. The stewardship it
        // reads to derive that is an image document, so it is pinned.
        let (receipt_id, writes) = committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let away = format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.GameCult_-Epiphany.{}", date().0);
        assert_eq!(writes, vec![
            r(K::HandOff, &away),
            r(K::Resolution, &format!("{INSTANCE}:resolution:stewardship.GameCult_-Epiphany.n1.n1")),
        ]);
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        let stewardship_envelope = mind.envelope(K::Stewardship, &first).unwrap().clone();
        assert!(
            receipt.strong_reads.contains(&DocumentVersion::from_envelope(&stewardship_envelope)),
            "the derived withdrawal's own citation is pinned: {:?}",
            receipt.strong_reads
        );

        // Nothing stewards the repo now, and `stewardship_of` says so: a new
        // campaign over it is refused.
        let D::Campaign(mut second) = campaign(&[REPO]) else { panic!() };
        second.slug = slug("second");
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Campaign(second)])),
            MindRefusal::RepoNotStewarded { repo: REPO.into() }
        );

        // Handed back, the mind stewards it again, as `n2`. No field names a
        // return: this is the same hand-off shape with the instances swapped.
        let D::HandOff(mut back) = hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) else { panic!() };
        back.handed_on = epiphany_pipeline::Date("2026-09-17".into());
        let (_, writes) = committed(admit(&mut mind, vec![D::HandOff(back)]));
        let again = format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n2");
        assert_eq!(writes, vec![
            r(K::HandOff, &format!("{OTHER_INSTANCE}:hand_off:{INSTANCE}.GameCult_-Epiphany.2026-09-17")),
            r(K::Stewardship, &again),
        ]);
        let Some(D::Stewardship(taken)) = mind.get(K::Stewardship, &again).unwrap() else { panic!() };
        assert_eq!(taken.sequence, 2);
        assert_eq!(taken.assigned_on, epiphany_pipeline::Date("2026-09-17".into()));
        assert!(mind.envelope(K::Stewardship, &first).is_some(), "the first assignment is still readable by key");
    }

    /// A seeded mind with one document of every resolvable kind, and the
    /// non-resolvable ones beside them.
    fn world() -> Mind<MemoryStore> {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![
            stewardship(INSTANCE, OTHER_REPO),
            question("Q1", &["A", "B"], "A"),
            D::Ruling(ruling("R1")),
            D::CutSpec(cut_spec("1", 1)),
            D::CutReport(cut_report("1", 1)),
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&id("finding", "cut-1.s1.F1")], Some("P1"), &["M1"])]),
            D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed)),
            follow_up("FU-1", r(K::Finding, &id("finding", "cut-1.s1.F1"))),
            hand_off(INSTANCE, OTHER_INSTANCE, OTHER_REPO, &[]),
        ]));
        mind
    }

    #[test]
    fn the_resolution_matrix_is_admissions() {
        let incompatible = |kind: K, outcome: &str| MindRefusal::IncompatibleResolution { subject_kind: kind, outcome: outcome.into() };
        let fixed = || ResolutionOutcome::Fixed { commit: sha(), by: None };
        let target_ref = r(K::Target, &id("target", "r1"));
        let target_r2 = r(K::Target, &id("target", "r2"));
        let question_ref = r(K::Question, &id("question", "Q1"));
        let ruling_ref = r(K::Ruling, &id("ruling", "R1"));
        let spec_ref = r(K::CutSpec, &id("cut_spec", "cut-1.r1"));
        let finding_ref = r(K::Finding, &id("finding", "cut-1.s1.F1"));
        let follow_up_ref = r(K::FollowUp, &id("follow_up", "FU-1"));
        let stewardship_ref = r(K::Stewardship, &format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1"));
        let campaign_ref = r(K::Campaign, &id("campaign", "self"));
        let report_ref = r(K::CutReport, &id("cut_report", "cut-1.h1"));
        let verdict_ref = r(K::Verdict, &id("verdict", "cut-1.s1"));
        let instance_ref = r(K::Instance, &format!("{INSTANCE}:instance:self"));
        let hand_off_ref = r(K::HandOff, &format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.GameCult_-Huginn.2026-09-16"));

        // Accepted, one per resolvable kind.
        committed(admit(&mut world(), vec![target(2, &[INVARIANT]), resolution(target_ref.clone(), superseded(&[target_r2.clone()]))]));
        committed(admit(&mut world(), vec![resolution(question_ref.clone(), withdrawn())]));
        committed(admit(&mut world(), vec![D::Ruling(ruling("R2")), resolution(ruling_ref.clone(), superseded(&[r(K::Ruling, &id("ruling", "R2"))]))]));
        committed(admit(&mut world(), vec![resolution(spec_ref.clone(), withdrawn())]));
        committed(admit(&mut world(), vec![resolution(finding_ref.clone(), ResolutionOutcome::Recorded { reason: "noted".into() })]));
        committed(admit(&mut world(), vec![resolution(finding_ref.clone(), ResolutionOutcome::Deferred { to: follow_up_ref.clone() })]));
        committed(admit(&mut world(), vec![resolution(follow_up_ref.clone(), fixed())]));
        committed(admit(&mut world(), vec![resolution(stewardship_ref.clone(), withdrawn())]));
        let mut mind = world();
        committed(admit(&mut mind, vec![resolution(question_ref.clone(), withdrawn())]));
        let nested = r(K::Resolution, &id("resolution", "question.Q1.n1"));
        committed(admit(&mut mind, vec![resolution(nested.clone(), withdrawn())]));

        // Refused, one per kind, and every non-resolvable kind.
        assert_eq!(refusal(admit(&mut world(), vec![resolution(target_ref.clone(), withdrawn())])), incompatible(K::Target, "Withdrawn"));
        assert_eq!(refusal(admit(&mut world(), vec![resolution(question_ref.clone(), fixed())])), incompatible(K::Question, "Fixed"));
        assert_eq!(refusal(admit(&mut world(), vec![resolution(ruling_ref.clone(), withdrawn())])), incompatible(K::Ruling, "Withdrawn"));
        // A kind admits the outcomes of its own row and no others: a question
        // is answered or withdrawn, never superseded; a target is superseded,
        // never answered.
        assert_eq!(
            refusal(admit(&mut world(), vec![
                question("Q2", &["A", "B"], "A"),
                resolution(question_ref, superseded(&[r(K::Question, &id("question", "Q2"))])),
            ])),
            incompatible(K::Question, "Superseded")
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![resolution(target_ref, ResolutionOutcome::Answered { by: ruling_ref })])),
            incompatible(K::Target, "Answered")
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![resolution(spec_ref.clone(), ResolutionOutcome::Answered { by: r(K::Ruling, &id("ruling", "R1")) })])),
            incompatible(K::CutSpec, "Answered")
        );
        // A cut spec superseded across cuts is refused too.
        let mut other_cut = cut_spec("2", 1);
        other_cut.cut = l("2");
        assert_eq!(
            refusal(admit(&mut world(), vec![D::CutSpec(other_cut), resolution(spec_ref, superseded(&[r(K::CutSpec, &id("cut_spec", "cut-2.r1"))]))])),
            incompatible(K::CutSpec, "Superseded")
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![resolution(finding_ref.clone(), superseded(&[finding_ref.clone()]))])),
            incompatible(K::Finding, "Superseded")
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![resolution(follow_up_ref, ResolutionOutcome::Answered { by: r(K::Ruling, &id("ruling", "R1")) })])),
            incompatible(K::FollowUp, "Answered")
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![resolution(stewardship_ref, ResolutionOutcome::Recorded { reason: "no".into() })])),
            incompatible(K::Stewardship, "Recorded")
        );
        let mut mind = world();
        committed(admit(&mut mind, vec![resolution(r(K::Question, &id("question", "Q1")), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(nested.clone(), superseded(&[nested]))])),
            incompatible(K::Resolution, "Superseded")
        );
        for (subject, kind) in [(campaign_ref, K::Campaign), (report_ref, K::CutReport), (verdict_ref, K::Verdict), (instance_ref, K::Instance), (hand_off_ref, K::HandOff)] {
            assert_eq!(refusal(admit(&mut world(), vec![resolution(subject, withdrawn())])), incompatible(kind, "Withdrawn"));
        }
    }

    #[test]
    fn supersession_names_each_supersessor_and_none_is_empty() {
        let subject = r(K::Ruling, &id("ruling", "R1"));
        assert_eq!(refusal(admit(&mut world(), vec![resolution(subject.clone(), superseded(&[]))])), MindRefusal::EmptySupersession);
        let (_, writes) = committed(admit(&mut world(), vec![
            D::Ruling(ruling("R2")),
            D::Ruling(ruling("R3")),
            resolution(subject.clone(), superseded(&[r(K::Ruling, &id("ruling", "R2")), r(K::Ruling, &id("ruling", "R3"))])),
        ]));
        assert_eq!(writes.len(), 3);
        assert_eq!(
            refusal(admit(&mut world(), vec![D::Ruling(ruling("R2")), resolution(subject.clone(), superseded(&[r(K::Ruling, &id("ruling", "R2")), r(K::Ruling, &id("ruling", "R4"))]))])),
            MindRefusal::UnknownSupersessor { id: id("ruling", "R4") }
        );
        let mut mind = world();
        committed(admit(&mut mind, vec![D::Ruling(ruling("R2")), D::Ruling(ruling("R3")), resolution(r(K::Ruling, &id("ruling", "R2")), superseded(&[r(K::Ruling, &id("ruling", "R3"))]))]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(subject, superseded(&[r(K::Ruling, &id("ruling", "R2"))]))])),
            MindRefusal::CitesResolvedDocument { kind: K::Ruling, id: id("ruling", "R2") }
        );
    }

    #[test]
    fn a_ruling_answering_a_question_derives_the_answered_resolution_atomically() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let mut answering = ruling("R1");
        answering.answers = Some(s(&id("question", "Q1")));
        answering.choice = Some(l("C"));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Ruling(answering.clone())])),
            MindRefusal::InvalidChoice { ruling: id("ruling", "R1"), choice: "C".into() }
        );
        answering.choice = Some(l("A"));
        let (receipt_id, writes) = committed(admit(&mut mind, vec![D::Ruling(answering)]));
        assert_eq!(writes, vec![r(K::Ruling, &id("ruling", "R1")), r(K::Resolution, &id("resolution", "question.Q1.n1"))]);
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        assert_eq!(receipt.writes.len(), 2);
        let Some(D::Resolution(derived)) = mind.get(K::Resolution, &id("resolution", "question.Q1.n1")).unwrap() else { panic!() };
        assert_eq!(derived.outcome, ResolutionOutcome::Answered { by: r(K::Ruling, &id("ruling", "R1")) });
        let mut again = ruling("R2");
        again.answers = Some(s(&id("question", "Q1")));
        again.choice = Some(l("B"));
        assert_eq!(refusal(admit(&mut mind, vec![D::Ruling(again)])), MindRefusal::AlreadyResolved { subject: id("question", "Q1") });

        // An explicit `Answered` must name the ruling that actually answers
        // its subject: a ruling answering Q1 does not resolve Q2.
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A"), question("Q2", &["A", "B"], "A")]));
        let mut answering = ruling("R1");
        answering.answers = Some(s(&id("question", "Q1")));
        answering.choice = Some(l("A"));
        let wrong = resolution(r(K::Question, &id("question", "Q2")), ResolutionOutcome::Answered { by: r(K::Ruling, &id("ruling", "R1")) });
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Ruling(answering.clone()), wrong.clone()])),
            MindRefusal::IncompatibleResolution { subject_kind: K::Question, outcome: "Answered".into() }
        );
        // The coherence is checked whether the ruling is in the batch or in
        // the image: a later batch citing the stored R1 is refused the same
        // way, and the derived resolution of Q1 is not what saves it.
        committed(admit(&mut mind, vec![D::Ruling(answering)]));
        assert_eq!(
            refusal(admit(&mut mind, vec![wrong])),
            MindRefusal::IncompatibleResolution { subject_kind: K::Question, outcome: "Answered".into() }
        );
    }

    #[test]
    fn a_batch_is_one_to_sixty_four_documents_each_with_its_own_identity() {
        let mut mind = seeded();
        // A2 at both ends, and at the boundary the batch is admitted whole.
        assert_eq!(refusal(admit(&mut mind, vec![])), MindRefusal::BatchSize { actual: 0 });
        let over = (0..65).map(|index| question(&format!("Q{index}"), &["A", "B"], "A")).collect::<Vec<_>>();
        assert_eq!(refusal(admit(&mut mind, over)), MindRefusal::BatchSize { actual: 65 });
        let full = (0..BATCH_MAX).map(|index| question(&format!("Q{index}"), &["A", "B"], "A")).collect::<Vec<_>>();
        let (_, writes) = committed(admit(&mut mind, full));
        assert_eq!(writes.len(), BATCH_MAX);
        // A4: one identity, once, whatever the two documents say.
        assert_eq!(
            refusal(admit(&mut mind, vec![question("Z", &["A", "B"], "A"), question("Z", &["A", "B"], "B")])),
            MindRefusal::IdentityCollision { kind: K::Question, id: id("question", "Z") }
        );
    }

    #[test]
    fn a_question_offers_at_least_two_options_and_recommends_one_of_them() {
        let mut mind = seeded();
        assert_eq!(
            refusal(admit(&mut mind, vec![question("Q7", &["A"], "A")])),
            MindRefusal::InvalidOptions { question: id("question", "Q7") }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![question("Q8", &["A", "B"], "C")])),
            MindRefusal::InvalidOptions { question: id("question", "Q8") }
        );
        committed(admit(&mut mind, vec![question("Q9", &["A", "B"], "B")]));
    }

    #[test]
    fn a_label_names_one_option_and_one_invariant() {
        let mut mind = seeded();
        assert_eq!(
            refusal(admit(&mut mind, vec![question("Q1", &["A", "A"], "A")])),
            MindRefusal::DuplicateLabel { field: "question.options".into(), label: "A".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![
                target(2, &["x", "x"]),
                resolution(r(K::Target, &id("target", "r1")), superseded(&[r(K::Target, &id("target", "r2"))])),
            ])),
            MindRefusal::DuplicateLabel { field: "target.invariants".into(), label: "x".into() }
        );
    }

    /// Every A7 reference the image resolved is pinned, not the first of
    /// them, so a batch citing three documents cannot commit over a change to
    /// the other two.
    #[test]
    fn every_cited_image_document_is_a_strong_read() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::Ruling(ruling("R1")), D::Ruling(ruling("R2")), D::Ruling(ruling("R3"))]));
        let cited = ["R1", "R2", "R3"].map(|label| mind.envelope(K::Ruling, &id("ruling", label)).unwrap().clone());
        let mut spec = cut_spec("1", 1);
        spec.rulings = cited.iter().map(|envelope| s(&envelope.key)).collect();
        let (receipt_id, _) = committed(admit(&mut mind, vec![D::CutSpec(spec)]));
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        assert_eq!(receipt.strong_reads.len(), 3);
        for envelope in &cited {
            assert!(receipt.strong_reads.contains(&DocumentVersion::from_envelope(envelope)), "{}", envelope.key);
        }

        // Two batch documents citing one image document pin it once: the pins
        // are a set, so a second citation neither duplicates the pin nor
        // cancels it.
        let mut twice = cut_spec("2", 1);
        twice.rulings = vec![s(&id("ruling", "R1"))];
        let D::Question(mut raised) = question("Q1", &["A", "B"], "A") else { panic!() };
        raised.raised_in = Some(r(K::Ruling, &id("ruling", "R1")));
        let (receipt_id, _) = committed(admit(&mut mind, vec![D::CutSpec(twice), D::Question(raised)]));
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        assert_eq!(receipt.strong_reads, vec![DocumentVersion::from_envelope(&cited[0])], "cited twice, pinned once");
    }

    #[test]
    fn an_operator_quote_requires_operator_authority() {
        let mut mind = seeded();
        let mut quoted = ruling("R1");
        quoted.operator_quote = Some("all recommendations, go ahead".into());
        assert_eq!(refusal(admit(&mut mind, vec![D::Ruling(quoted.clone())])), MindRefusal::QuoteWithoutOperator { ruling: id("ruling", "R1") });
        quoted.authority = RulingAuthority::Operator;
        committed(admit(&mut mind, vec![D::Ruling(quoted)]));
    }

    #[test]
    fn a_revision_requires_its_predecessors_supersession_in_the_batch() {
        let mut mind = seeded();
        assert_eq!(
            refusal(admit(&mut mind, vec![target(2, &[INVARIANT])])),
            MindRefusal::RevisionWithoutSupersession { kind: K::Target, revision: 2 }
        );
        committed(admit(&mut mind, vec![
            target(2, &[INVARIANT]),
            resolution(r(K::Target, &id("target", "r1")), superseded(&[r(K::Target, &id("target", "r2"))])),
        ]));
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 2))])),
            MindRefusal::RevisionWithoutSupersession { kind: K::CutSpec, revision: 2 }
        );
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("1", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-1.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-1.r2"))])),
        ]));
    }

    #[test]
    fn a_cut_report_cites_an_in_force_spec_and_agrees_with_it() {
        let mut mind = seeded();
        assert_eq!(
            refusal(admit(&mut mind, vec![D::CutReport(cut_report("1", 1))])),
            MindRefusal::CutReportWithoutSpec { report: id("cut_report", "cut-1.h1") }
        );
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
        let mut report = cut_report("1", 1);
        report.branch = s("another-branch");
        assert_eq!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::SpecMismatch { field: "branch".into() });
        // A branch is compared as written. Git's refs are case-sensitive, and
        // a report of `Codex/...` is a report of another branch or of none.
        let mut report = cut_report("1", 1);
        report.branch = s("CODEX/eureka-pipeline-state");
        assert_eq!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::SpecMismatch { field: "branch".into() });
        let mut report = cut_report("1", 1);
        report.repo = repo(OTHER_REPO);
        assert_eq!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::SpecMismatch { field: "repo".into() });
        let mut report = cut_report("1", 1);
        report.range.head = epiphany_pipeline::Sha("abcdef0".into());
        assert_eq!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::RangeOutsideCommits { head: "abcdef0".into() });
        committed(admit(&mut mind, vec![resolution(r(K::CutSpec, &id("cut_spec", "cut-1.r1")), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::CutReport(cut_report("1", 1))])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: id("cut_spec", "cut-1.r1") }
        );
    }

    #[test]
    fn verdict_vocabulary_binds_claims_to_findings_promises_and_mutations() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1)), D::CutReport(cut_report("1", 1))]));
        let finding_id = id("finding", "cut-1.s1.F1");
        let confirmed = D::Finding(finding("1", 1, "F1", FindingConfidence::Confirmed));
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[], Some("P1"), &[])])])),
            MindRefusal::FalsifiedClaimWithoutConfirmedFinding { claim: "Falsified claim".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![
                verdict("1", 1, vec![claim(ClaimOutcome::Unproven, &[&finding_id], Some("P1"), &[])]),
                confirmed.clone(),
            ])),
            MindRefusal::UnprovenClaimWithConfirmedFinding { claim: "Unproven claim".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![claim(ClaimOutcome::Holds, &[], None, &[])])])),
            MindRefusal::PromiseWithoutVerdict { promise: "P1".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![
                claim(ClaimOutcome::Holds, &[], Some("P1"), &[]),
                claim(ClaimOutcome::Holds, &[], Some("P1"), &[]),
            ])])),
            MindRefusal::PromiseWithoutVerdict { promise: "P1".into() }
        );
        // A promise is measured by the claim that names it, not by any claim
        // that names a promise: the report promises P1 and nothing else.
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![claim(ClaimOutcome::Holds, &[], Some("P2"), &[])])])),
            MindRefusal::PromiseWithoutVerdict { promise: "P1".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![verdict("1", 1, vec![claim(ClaimOutcome::Holds, &[], Some("P1"), &["M9"])])])),
            MindRefusal::UnknownMutationLabel { label: "M9".into() }
        );
        committed(admit(&mut mind, vec![
            verdict("1", 1, vec![claim(ClaimOutcome::Falsified, &[&finding_id], Some("P1"), &["M1"])]),
            confirmed,
        ]));
    }

    #[test]
    fn a_finding_names_evidence_locations_and_known_invariants() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("1", 1)),
            D::CutReport(cut_report("1", 1)),
            verdict("1", 1, vec![claim(ClaimOutcome::Holds, &[], Some("P1"), &[])]),
        ]));
        let mut bare = finding("1", 1, "F1", FindingConfidence::Plausible);
        bare.evidence = vec![];
        assert_eq!(refusal(admit(&mut mind, vec![D::Finding(bare)])), MindRefusal::FindingWithoutEvidence);
        // Locations are evidence's other half: a finding that names no place
        // in the code is refused the same way.
        let mut placeless = finding("1", 1, "F1", FindingConfidence::Plausible);
        placeless.locations = vec![];
        assert_eq!(refusal(admit(&mut mind, vec![D::Finding(placeless)])), MindRefusal::FindingWithoutEvidence);
        let mut unknown = finding("1", 1, "F1", FindingConfidence::Plausible);
        unknown.invariants = vec![l("nope")];
        assert_eq!(refusal(admit(&mut mind, vec![D::Finding(unknown)])), MindRefusal::UnknownInvariant { label: "nope".into() });


        // A raw envelope without `range` cannot be built from the type, and
        // it is the leaf's decode that refuses it, before any organ rule.
        #[derive(serde::Serialize)]
        struct Rangeless {
            campaign: Slug,
            verdict: Short,
            label: epiphany_pipeline::Label,
            confidence: FindingConfidence,
            severity: epiphany_pipeline::FindingSeverity,
            claim: Line,
            invariants: Vec<epiphany_pipeline::Label>,
            locations: Vec<epiphany_pipeline::CodeLocation>,
            failure_scenario: epiphany_pipeline::Para,
            evidence: Vec<epiphany_pipeline::Evidence>,
            precedents: Vec<epiphany_pipeline::ForeignRef>,
            origin: epiphany_pipeline::FindingOrigin,
        }
        let whole = finding("1", 1, "F1", FindingConfidence::Plausible);
        let rangeless = Rangeless {
            campaign: whole.campaign.clone(),
            verdict: whole.verdict.clone(),
            label: whole.label.clone(),
            confidence: whole.confidence,
            severity: whole.severity,
            claim: whole.claim.clone(),
            invariants: whole.invariants.clone(),
            locations: whole.locations.clone(),
            failure_scenario: whole.failure_scenario.clone(),
            evidence: whole.evidence.clone(),
            precedents: vec![],
            origin: whole.origin,
        };
        let mut envelope = prepare(&D::Finding(whole));
        envelope.payload = rmp_serde::to_vec_named(&(rangeless,)).unwrap();
        let outcome = mind.admit_prepared(&slug(INSTANCE), provenance(Faculty::Soul), vec![envelope], now());
        assert!(
            matches!(refusal(outcome), MindRefusal::Document(PipelineRefusal::InvalidFormat { field, .. }) if field == "payload")
        );

        // The in-force target owns the invariant vocabulary. Supersede r1 with
        // an r2 that drops its label, and the label is unknown again; r2's own
        // label is what a finding may cite. The target is looked for in image
        // and batch, like every other citation, so a finding may cite the
        // vocabulary of the revision landing beside it: `other` is admitted in
        // r2's own batch, before r2 is anywhere but here.
        let mut fresh = finding("1", 1, "F4", FindingConfidence::Plausible);
        fresh.invariants = vec![l("other")];
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Finding(fresh.clone())])),
            MindRefusal::UnknownInvariant { label: "other".into() }
        );
        committed(admit(&mut mind, vec![
            target(2, &["other"]),
            resolution(r(K::Target, &id("target", "r1")), superseded(&[r(K::Target, &id("target", "r2"))])),
            D::Finding(fresh),
        ]));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Finding(finding("1", 1, "F2", FindingConfidence::Plausible))])),
            MindRefusal::UnknownInvariant { label: INVARIANT.into() }
        );
        let mut current = finding("1", 1, "F3", FindingConfidence::Plausible);
        current.invariants = vec![l("other")];
        committed(admit(&mut mind, vec![D::Finding(current)]));
    }

    #[test]
    fn a_campaign_names_only_repos_this_mind_stewards() {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        assert_eq!(
            refusal(admit(&mut mind, vec![instance(INSTANCE), campaign(&[REPO])])),
            MindRefusal::RepoNotStewarded { repo: REPO.into() }
        );
        committed(admit(&mut mind, vec![instance(INSTANCE), stewardship(INSTANCE, REPO), campaign(&[REPO])]));
        // A campaign of no repos is the organ's refusal, not a leaf bound:
        // "every repo is stewarded" is vacuous and the campaign steers
        // nothing.
        let D::Campaign(mut empty) = campaign(&[]) else { panic!() };
        empty.slug = slug("other");
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Campaign(empty)])),
            MindRefusal::EmptyRepos { campaign: "other:campaign:self".into() }
        );
        // The stewardship may be held in the image: a later campaign over the
        // same repo carries no stewardship of its own.
        let D::Campaign(mut second) = campaign(&[REPO]) else { panic!() };
        second.slug = slug("second");
        committed(admit(&mut mind, vec![D::Campaign(second)]));
        let mut spec = cut_spec("1", 1);
        spec.repo = repo(OTHER_REPO);
        assert_eq!(refusal(admit(&mut mind, vec![D::CutSpec(spec)])), MindRefusal::RepoNotInCampaign { repo: OTHER_REPO.into() });
    }

    #[test]
    fn a_hand_off_derives_this_minds_side_only() {
        let key = format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.GameCult_-Epiphany.2026-09-16");
        let campaign_key = id("campaign", "self");
        let stewardship_key = format!("{INSTANCE}:stewardship:GameCult_-Epiphany.n1");

        let mut source = seeded();
        assert_eq!(
            refusal(admit(&mut source, vec![hand_off(INSTANCE, OTHER_INSTANCE, OTHER_REPO, &[])])),
            MindRefusal::NotStewarded { repo: OTHER_REPO.into() }
        );
        assert_eq!(
            refusal(admit(&mut source, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[&id("ruling", "R1")])])),
            MindRefusal::MissingReference { kind: K::Ruling, id: id("ruling", "R1") }
        );
        let (_, writes) = committed(admit(&mut source, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[&campaign_key])]));
        let withdrawal = id_of(&stewardship_key);
        assert_eq!(writes, vec![r(K::HandOff, &key), r(K::Resolution, &withdrawal)]);
        let Some(D::Resolution(derived)) = source.get(K::Resolution, &withdrawal).unwrap() else { panic!() };
        assert_eq!(derived.outcome, ResolutionOutcome::Withdrawn { reason: Line(key.clone()) });
        assert_eq!(derived.subject, r(K::Stewardship, &stewardship_key));

        let mut receiving = opened(MemoryStore::new(), OTHER_INSTANCE);
        committed(admit(&mut receiving, vec![instance(OTHER_INSTANCE)]));
        let (_, writes) = committed(admit(&mut receiving, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[&campaign_key])]));
        let assigned = format!("{OTHER_INSTANCE}:stewardship:GameCult_-Epiphany.n1");
        assert_eq!(writes, vec![r(K::HandOff, &key), r(K::Stewardship, &assigned)]);
        let Some(D::Stewardship(derived)) = receiving.get(K::Stewardship, &assigned).unwrap() else { panic!() };
        assert_eq!(derived.note, Line(key.clone()));
        assert_eq!(derived.instance, slug(OTHER_INSTANCE));
        // The stewardship starts the day the repo was handed over, not on some
        // clock of admission's own.
        assert_eq!(derived.assigned_on, date());
        assert_eq!(source.envelope(K::HandOff, &key).map(|e| &e.payload), receiving.envelope(K::HandOff, &key).map(|e| &e.payload));

        // A derived write passes A3-A7 and A8 like any other, and it counts as
        // a record of its scope. A batch carrying its own assignment of the
        // repo beside the hand-off is two assignments: the derived one takes
        // the sequence after the batch's, and the batch's own is then out of
        // sequence -- a typed refusal, not a duplicate identity the store
        // complains about.
        let mut colliding = opened(MemoryStore::new(), OTHER_INSTANCE);
        committed(admit(&mut colliding, vec![instance(OTHER_INSTANCE)]));
        assert_eq!(
            refusal(admit(&mut colliding, vec![
                hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[]),
                stewardship(OTHER_INSTANCE, REPO),
            ])),
            MindRefusal::StewardshipOutOfSequence { repo: REPO.into(), expected: 3, actual: 1 }
        );
    }

    /// The key of the first resolution of a stewardship: the subject's root,
    /// then its kind and local -- the subject's own sequence included -- then
    /// this resolution's own sequence.
    fn id_of(stewardship_key: &str) -> String {
        let (root, rest) = stewardship_key.split_once(':').unwrap();
        format!("{root}:resolution:{}.n1", rest.replace(':', "."))
    }

    /// The second construction site of a receipt, deliberately: a receipt
    /// planted through the store must be found by replay and must match.
    #[test]
    fn a_planted_receipt_with_other_content_is_refused_not_replayed() {
        let store = MemoryStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let envelopes = vec![prepare(&question("Q1", &["A", "B"], "A"))];
        let candidate = receipt::candidate(&slug(INSTANCE), provenance(Faculty::Hands), &[], &envelopes, now()).unwrap();
        let mut planted = HuginnCommitReceipt { writes: vec![], ..candidate.clone() };
        planted.writes = candidate.strong_reads.clone();
        planted.receipt_id = candidate.receipt_id.clone();
        store.plant(mind.cache().prepare_entry_named(&planted.receipt_id, &planted).unwrap().0);
        let mut mind = opened(store, INSTANCE);
        assert!(matches!(
            refusal(mind.admit_prepared(&slug(INSTANCE), provenance(Faculty::Hands), envelopes, now())),
            MindRefusal::Unavailable { .. }
        ));
    }
}
