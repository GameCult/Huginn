//! Admission: the single door into a mind.
//!
//! `admit` prepares typed documents through the leaf and calls
//! `admit_prepared`, the one commit path for Cut 8, Cut 10's sink, Cut 12's
//! hand-off and import, and Cut 13's tools. The steps run in order and the
//! first failing step is the outcome:
//!
//! - A1 the batch names this mind, and attributes itself to a named agent and
//!   session (`refuse_provenance`); A2 one to sixty-four envelopes;
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
//! In force, the sequences and the scopes are `docs.rs`'s, shared with the
//! views (Cut 9).

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use cultcache_rs::{CultCache, CultCacheEnvelope};
use eureka_pipeline::{
    ClaimOutcome, FindingConfidence, Line, PipelineDocument, PipelineKind, PipelineRef, PipelineRefusal,
    PipelineResolution, PipelineRun, PipelineStewardship, ResolutionOutcome, RulingAuthority, Short, Slug, pipeline_key,
    validate_pipeline_write_envelope,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::docs::{CitationRole, Docs, Staged, citations, kind_of_id, outcome_citations};
use crate::mind::{HuginnMindEpoch, Mind, unavailable};
use crate::receipt::{self, CommitOutcome, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::MindStore;

/// The most envelopes one batch may carry.
pub const BATCH_MAX: usize = 64;

/// One admission request: the mind it is for, who asks, and the documents.
/// Rides the wire whole (`wire::HuginnMindRequest::Admit`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

/// Keys a document through the leaf, then encodes it. A key the leaf cannot
/// compose is the document's fault and is refused as a document; only the
/// cache's encode fault is the organ's.
fn prepare(document: &PipelineDocument, cache: &CultCache) -> Result<CultCacheEnvelope, MindRefusal> {
    pipeline_key(document).map_err(MindRefusal::Document)?;
    document.prepare(cache).map_err(unavailable)
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
            match prepare(document, self.cache()) {
                Ok(envelope) => envelopes.push(envelope),
                Err(refusal) => return PipelineAdmissionOutcome::Refused(refusal),
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
        self.require_instance(instance)?;
        refuse_provenance(&provenance)?;
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
            let envelope = prepare(&document, self.cache())?;
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
        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, receipt::head(self)? + 1, now)?;
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

/// The bound `eureka_pipeline::Short` declares, in UTF-8 bytes. The leaf
/// exposes no validator for it, so `short_bound_is_the_leafs_own` pins this to
/// the `maxLength` its schema publishes.
const SHORT_MAX: usize = 200;

/// Who admitted a batch is the receipt's only record of it, so the mind checks
/// it here and every writer, whatever its transport, gets the check: the agent,
/// the session and the tool follow the leaf's `Title` rule (one alphanumeric
/// character, no control or line-break character, no bidi control) within
/// `Short`'s bound. The leaf keeps that rule private, so
/// `provenance_text_is_the_leafs_title_rule` pins this to it. Refused as the
/// leaf's own refusals, since the fields are the leaf's own type.
fn refuse_provenance(provenance: &PipelineProvenance) -> Result<(), MindRefusal> {
    let bidi = |c: char| {
        matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
    };
    let control = |c: char| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}');
    for (field, value) in [
        ("provenance.agent", &provenance.agent),
        ("provenance.session", &provenance.session),
        ("provenance.tool", &provenance.tool),
    ] {
        let text = value.0.as_str();
        if !text.chars().any(char::is_alphanumeric) || text.contains(control) || text.contains(bidi) {
            return Err(PipelineRefusal::InvalidFormat { field: field.into(), value: text.into() }.into());
        }
        if text.len() > SHORT_MAX {
            return Err(PipelineRefusal::FieldBound { field: field.into(), limit: SHORT_MAX as u32, actual: text.len() as u32 }.into());
        }
    }
    Ok(())
}

/// A3 through the leaf, then A5.
fn stage(envelope: CultCacheEnvelope, mind: &Slug) -> Result<Staged, MindRefusal> {
    validate_pipeline_write_envelope(&envelope)?;
    let document = PipelineDocument::decode(&envelope)?;
    refuse_foreign_instance(&document, mind)?;
    Ok(Staged { kind: document.kind(), key: envelope.key.clone(), document, envelope })
}

/// A5: every document carrying an instance field names this mind (a run
/// included); a hand-off names it on one side.
fn refuse_foreign_instance(document: &PipelineDocument, mind: &Slug) -> Result<(), MindRefusal> {
    let foreign = |declared: &Slug| MindRefusal::ForeignInstance { declared: declared.0.clone(), mind: mind.0.clone() };
    match document {
        PipelineDocument::Instance(value) if value.instance != *mind => Err(foreign(&value.instance)),
        PipelineDocument::Stewardship(value) if value.instance != *mind => Err(foreign(&value.instance)),
        PipelineDocument::Run(value) if value.instance != *mind => Err(foreign(&value.instance)),
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

/// A7's field list is `docs::citations`: every `PipelineRef` and every
/// full-id `Short` field, each carrying the refusal its absence earns. A
/// hand-off's documents are cited on the source side only, and each must read
/// as an id of a kind; the receiving side imports them afterwards (Cut 12).
fn references(staged: &Staged, mind: &Slug) -> Result<Vec<Reference>, MindRefusal> {
    if let PipelineDocument::HandOff(hand_off) = &staged.document {
        if hand_off.from_instance != *mind {
            return Ok(Vec::new());
        }
        if let Some(document) = hand_off.documents.iter().find(|document| kind_of_id(&document.0).is_none()) {
            return Err(PipelineRefusal::InvalidFormat {
                field: "hand_off.documents".into(),
                value: document.0.clone(),
            }
            .into());
        }
    }
    Ok(citations(&staged.document)
        .into_iter()
        .map(|(role, target)| Reference {
            kind: target.kind,
            id: target.id.0,
            missing: match role {
                CitationRole::CutSpec => Missing::SpecOfReport(staged.key.clone()),
                CitationRole::SupersededBy => Missing::Supersessor,
                _ => Missing::Reference,
            },
        })
        .collect())
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
                    let sequence = docs.derived_resolution_sequence(&subject, |outcome| {
                        matches!(outcome, ResolutionOutcome::Answered { by } if by.kind == K::Ruling && by.id.0 == staged.key)
                    });
                    derived.push(D::Resolution(PipelineResolution {
                        sequence,
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
                    let sequence = docs.derived_resolution_sequence(&subject, |outcome| {
                        matches!(outcome, ResolutionOutcome::Withdrawn { reason } if reason.0 == staged.key)
                    });
                    derived.push(D::Resolution(PipelineResolution {
                        sequence,
                        subject,
                        outcome: ResolutionOutcome::Withdrawn { reason: Line(staged.key.clone()) },
                        rationale: hand_off.reason.clone(),
                        resolved_on: hand_off.handed_on.clone(),
                    }));
                }
                if hand_off.to_instance == *mind {
                    derived.push(D::Stewardship(PipelineStewardship {
                        instance: mind.clone(),
                        sequence: docs.derived_stewardship_sequence(mind, &hand_off.repo, &staged.key),
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
            Ok(())
        }
        D::Target(target) => {
            let predecessor = D::Target(eureka_pipeline::PipelineTarget { revision: target.revision.wrapping_sub(1), ..target.clone() });
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
            for dependency in &spec.depends_on {
                // A cut naming itself is no ordering (ruling self-dep).
                if dependency.0 == spec.cut.0 {
                    return Err(MindRefusal::UnknownDependency { cut: dependency.0.clone() });
                }
                let names_a_cut = docs.of_kind(K::CutSpec).any(|(_, document)| {
                    matches!(document, D::CutSpec(other) if other.campaign == spec.campaign && other.cut.0 == dependency.0)
                });
                if !names_a_cut {
                    return Err(MindRefusal::UnknownDependency { cut: dependency.0.clone() });
                }
            }
            let predecessor = D::CutSpec(eureka_pipeline::PipelineCutSpec { revision: spec.revision.wrapping_sub(1), ..spec.clone() });
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
            if !report.commits.iter().any(|commit| commit.sha.names_same_commit(&report.range.head)) {
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
        // run-is-the-grant: the claim is the one consumption fact. A claim must
        // be in force, and no other run in force may hold it. A run already in
        // the image under this key, byte for byte, is a replay and owes its
        // claims no fresh freedom, so the replay still answers AlreadyAdmitted
        // after the work it claimed has moved on. One live run of hers
        // per instance and turn (ruling one-live-self-run) is decided by the same
        // rule.
        D::Run(run) => {
            let replay = docs.in_image(K::Run, key) == Some(&staged.document);
            run_rule(docs, key, run, replay)
        }
        D::FollowUp(_) | D::Instance(_) => Ok(()),
    }
}

/// The one rule for a run opening or reinstated by withdrawing its close: every
/// claim in force (a replay of an admitted run owes none) and held by no other
/// live run, then at most one live run of hers per instance and turn.
fn run_rule(docs: &Docs, run_key: &str, run: &PipelineRun, replay: bool) -> Result<(), MindRefusal> {
    for claim in &run.claims {
        if !replay && !docs.in_force(claim.kind, &claim.id.0) {
            return Err(MindRefusal::CitesResolvedDocument { kind: claim.kind, id: claim.id.0.clone() });
        }
        if let Some(holder) = docs.claim_holder(run_key, claim) {
            return Err(MindRefusal::AlreadyClaimed { item: claim.id.0.clone(), run: holder.into() });
        }
    }
    if !replay && let Some(holder) = docs.live_holder(run_key, run) {
        return Err(MindRefusal::AlreadyLive { run: holder.into() });
    }
    Ok(())
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
        (K::Finding, O::Deferred { to }) => matches!(to.kind, K::FollowUp | K::CutSpec),
        (K::Finding, O::Recorded { .. } | O::Withdrawn { .. }) => true,
        (K::FollowUp, O::Fixed { by, .. }) => by.as_ref().is_none_or(|report| report.kind == K::CutReport),
        (K::FollowUp, O::Superseded { by }) => all(by, K::FollowUp),
        (K::FollowUp, O::Withdrawn { .. }) => true,
        (K::Stewardship, O::Superseded { by }) => all(by, K::Stewardship),
        (K::Stewardship, O::Withdrawn { .. }) => true,
        (K::Run, O::Recorded { .. } | O::Withdrawn { .. }) => true,
        (K::Resolution, O::Withdrawn { .. }) => true,
        _ => false,
    };
    fits
}

/// The resolution row: the sequence, the in-force subject, the matrix, the
/// Q19 cap and the reinstatement rule it rests on, a non-empty supersession,
/// every referent in force, and the two
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
    // Q19 A's rationale one step out from the cap: a withdrawal never
    // re-raises an earlier record over a later one. Withdrawing a resolution
    // puts its subject back in force, so refuse while a later record of that
    // subject's own scope stands -- a repo's next assignment, a document's
    // next revision.
    if matches!(resolution.outcome, ResolutionOutcome::Withdrawn { .. })
        && subject_kind == K::Resolution
        && let Some(D::Resolution(reinstating)) = docs.find(K::Resolution, subject_id)
        && let Some(base) = docs.find(reinstating.subject.kind, &reinstating.subject.id.0)
        && let Some(later) = docs.later_in_force(base)
    {
        return Err(MindRefusal::WouldReinstateOverLater {
            subject: reinstating.subject.id.0.clone(),
            later: later.into(),
        });
    }
    // Withdrawing a run's closure puts the run back in force, and a run in
    // force holds its claims and its turn's slot: refuse while another run in force
    // holds one of the claims or another run of hers of its turn is live.
    if matches!(resolution.outcome, ResolutionOutcome::Withdrawn { .. })
        && subject_kind == K::Resolution
        && let Some(D::Resolution(reinstating)) = docs.find(K::Resolution, subject_id)
        && reinstating.subject.kind == K::Run
        && let Some(D::Run(run)) = docs.find(K::Run, &reinstating.subject.id.0)
    {
        run_rule(docs, &reinstating.subject.id.0, run, false)?;
    }
    for (_, referent) in outcome_citations(&resolution.outcome) {
        if !docs.in_force(referent.kind, &referent.id.0) {
            return Err(MindRefusal::CitesResolvedDocument { kind: referent.kind, id: referent.id.0 });
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
    use crate::fixtures::prepare;
    use crate::mind::schema_cache;
    use crate::receipt::{DocumentVersion, Faculty, HuginnCommitReceipt};
    use crate::store::test_stores::{MemoryStore, RefusingStore, SwapCommand};
    use cultcache_rs::DatabaseEntry;
    use eureka_pipeline::{PIPELINE_SCHEMA_EPOCH, PipelineDocument as D, PipelineKind as K};
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

    /// The identity the index labels its collection with: none before the
    /// first write, then the first receipt's id and never a later one.
    #[test]
    fn the_genesis_receipt_id_is_the_first_admissions_and_stays_put() {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        assert_eq!(mind.genesis_receipt_id().unwrap(), None);
        let (first, _) = committed(admit(&mut mind, vec![instance(INSTANCE), stewardship(INSTANCE, REPO)]));
        assert_eq!(mind.genesis_receipt_id().unwrap(), Some(first.clone()));
        let (second, _) = committed(admit(&mut mind, vec![campaign(&[REPO])]));
        assert_ne!(first, second);
        assert_eq!(mind.genesis_receipt_id().unwrap(), Some(first));
    }

    fn admit_attributed(agent: &str, session: &str) -> (Mind<MemoryStore>, PipelineAdmissionOutcome) {
        admit_signed(agent, session, "admit")
    }

    fn admit_signed(agent: &str, session: &str, tool: &str) -> (Mind<MemoryStore>, PipelineAdmissionOutcome) {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: s(agent), session: s(session), tool: s(tool) };
        let batch = PipelineAdmissionBatch { instance: slug(INSTANCE), provenance, documents: vec![instance(INSTANCE), stewardship(INSTANCE, REPO)] };
        let outcome = mind.admit(batch, now());
        (mind, outcome)
    }

    /// Who admitted a batch is validated by the mind, so no writer can commit
    /// a blank, multi-line or oversized attribution, and a refused batch leaves
    /// nothing behind.
    #[test]
    fn provenance_names_an_agent_a_session_and_a_tool_on_one_clean_line_within_short() {
        let over = "x".repeat(SHORT_MAX + 1);
        let exact = "y".repeat(SHORT_MAX);
        for bad in [
            "",
            " \t ",
            "\u{2003}",
            "\u{200B}",
            "line\nbreak",
            "line\rbreak",
            "a\u{0B}b",
            "a\u{0C}b",
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\0b",
            "a\u{1B}[31mb",
            "a\u{202E}b",
            "a\u{2066}b",
            "a\u{200F}b",
        ] {
            for field in ["provenance.agent", "provenance.session", "provenance.tool"] {
                let (mind, outcome) = match field {
                    "provenance.agent" => admit_signed(bad, "session-1", "admit"),
                    "provenance.session" => admit_signed("claude", bad, "admit"),
                    _ => admit_signed("claude", "session-1", bad),
                };
                let refused = refusal(outcome);
                assert!(
                    matches!(&refused, MindRefusal::Document(PipelineRefusal::InvalidFormat { field: named, .. }) if named == field),
                    "{field}={bad:?}: {refused:?}"
                );
                assert!(mind.is_empty(), "{field}={bad:?}: a refused batch wrote");
            }
        }
        for (field, outcome) in [
            ("provenance.agent", admit_signed(&over, "session-1", "admit")),
            ("provenance.session", admit_signed("claude", &over, "admit")),
            ("provenance.tool", admit_signed("claude", "session-1", &over)),
        ] {
            let (mind, outcome) = outcome;
            assert_eq!(
                refusal(outcome),
                MindRefusal::Document(PipelineRefusal::FieldBound { field: field.into(), limit: 200, actual: 201 })
            );
            assert!(mind.is_empty());
        }
        // The bound is in bytes, not characters: 101 two-byte characters are 202 bytes.
        let (_, outcome) = admit_attributed(&"é".repeat(101), "session-1");
        assert_eq!(
            refusal(outcome),
            MindRefusal::Document(PipelineRefusal::FieldBound { field: "provenance.agent".into(), limit: 200, actual: 202 })
        );
        let (_, outcome) = admit_signed("claude", "session-1", &"é".repeat(101));
        assert!(matches!(refusal(outcome), MindRefusal::Document(PipelineRefusal::FieldBound { actual: 202, .. })));
        let (_, outcome) = admit_attributed(&"é".repeat(100), "session-1");
        committed(outcome);
        // The bound is inclusive and a name with inner spaces is a name.
        let (mind, outcome) = admit_signed(&exact, "session 7", "admit tool");
        committed(outcome);
        let receipts = mind.receipts().unwrap();
        assert_eq!(receipts[0].provenance.agent, s(&exact));
        assert_eq!(receipts[0].provenance.session, s("session 7"));
        assert_eq!(receipts[0].provenance.tool, s("admit tool"));
    }

    /// The leaf's `Title` rule is private, so this holds the provenance rule to
    /// it: the two accept and refuse the same text.
    #[test]
    fn provenance_text_is_the_leafs_title_rule() {
        let leaf_accepts = |text: &str| {
            let D::Campaign(mut campaign) = campaign(&[REPO]) else { unreachable!() };
            campaign.title = eureka_pipeline::Title(text.into());
            D::Campaign(campaign).validate().is_ok()
        };
        let mine_accepts = |text: &str| {
            let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: s(text), session: s("s"), tool: s("t") };
            refuse_provenance(&provenance).is_ok()
        };
        for text in [
            "claude", "session 7", "", " ", "\t", "\u{200B}", "\u{2003}", "!!!", "a\nb", "a\u{0B}b", "a\u{85}b", "a\u{2028}b",
            "a\0b", "a\u{7F}b", "a\u{202E}b", "a\u{061C}b", "a\u{2069}b", "a\u{200D}b", "é", "日本", "a\u{FEFF}b",
            &"x".repeat(200), &"x".repeat(201), &"é".repeat(100), &"é".repeat(101),
        ] {
            assert_eq!(mine_accepts(text), leaf_accepts(text), "{text:?}");
        }
    }

    #[test]
    fn short_bound_is_the_leafs_own() {
        let schema = serde_json::to_value(schemars::schema_for!(Short)).unwrap();
        assert_eq!(schema["maxLength"], serde_json::json!(SHORT_MAX));
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
        assert_ne!(
            Mind::store_path_for(root.path(), &slug(INSTANCE)).unwrap(),
            Mind::store_path_for(root.path(), &slug(OTHER_INSTANCE)).unwrap()
        );
        committed(admit(&mut yggdrasil, vec![instance(INSTANCE), stewardship(INSTANCE, REPO)]));
        committed(admit(&mut thought_cage, vec![instance(OTHER_INSTANCE)]));
        let key = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1");
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

    /// RS-1: the ordinal is `head + 1` at admission, not a rank derived from
    /// the clock or the receipt id. Three batches land at one `now`, so a
    /// mutant that ranked by `(committed_at, receipt_id)` would fall back to
    /// id order among them; the ids are checked not to already sort in
    /// admission order, so that fallback provably disagrees with the truth.
    #[test]
    fn ordinals_are_admission_order_not_clock_or_id_order() {
        let mut mind = seeded();
        assert_eq!(receipt::head(&mind).unwrap(), 1, "the seed batch is ordinal 1");
        let same_time = now();
        let (id1, _) = committed(admit_at(&mut mind, vec![question_n(1)], same_time));
        let (id2, _) = committed(admit_at(&mut mind, vec![question_n(2)], same_time));
        let (id3, _) = committed(admit_at(&mut mind, vec![question_n(3)], same_time));

        let mut by_id = [id1.clone(), id2.clone(), id3.clone()];
        by_id.sort();
        assert_ne!(
            by_id,
            [id1.clone(), id2.clone(), id3.clone()],
            "the fixture needs receipt ids that do not already sort in admission order"
        );

        let receipts = mind.receipts().unwrap();
        let ordinal_of = |id: &str| receipts.iter().find(|receipt| receipt.receipt_id == id).unwrap().ordinal;
        assert_eq!(ordinal_of(&id1), 2);
        assert_eq!(ordinal_of(&id2), 3);
        assert_eq!(ordinal_of(&id3), 4);
        assert_eq!(receipt::head(&mind).unwrap(), 4);
    }

    /// RS-1, A9: the ordinal is not digested, so an exact replay finds the
    /// receipt it first landed with and keeps that receipt's ordinal, even
    /// when the head has moved since and a fresh candidate would be assigned
    /// a different one.
    #[test]
    fn an_exact_replay_keeps_its_ordinal() {
        let mut mind = seeded();
        let (first_id, _) = committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let first_ordinal =
            mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == first_id).unwrap().ordinal;

        // Move the head, so a replay's own candidate would carry a different
        // ordinal than the one already on the stored receipt.
        committed(admit(&mut mind, vec![question("Q2", &["A", "B"], "A")]));

        let outcome = admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]);
        assert_eq!(outcome, PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: first_id.clone() });

        let receipts = mind.receipts().unwrap();
        assert_eq!(receipts.iter().filter(|receipt| receipt.receipt_id == first_id).count(), 1, "no second receipt lands");
        let stored = receipts.iter().find(|receipt| receipt.receipt_id == first_id).unwrap();
        assert_eq!(stored.ordinal, first_ordinal, "the replay does not overwrite the ordinal");
    }

    /// A batch that derives a write replays like any other: the second
    /// admission re-derives the record the first one landed, sequence and
    /// all, so A9 recognises the digest and answers with the first receipt
    /// instead of the derived write colliding with itself.
    #[test]
    fn an_exact_replay_re_derives_the_rulings_resolution() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let mut answering = ruling("R1");
        answering.answers = Some(s(&id("question", "Q1")));
        answering.choice = Some(l("A"));
        let (receipt_id, writes) = committed(admit(&mut mind, vec![D::Ruling(answering.clone())]));
        assert!(writes.contains(&r(K::Resolution, &id("resolution", "question.Q1.n1"))));
        let receipts = receipt_count(&mind);
        assert_eq!(
            admit(&mut mind, vec![D::Ruling(answering)]),
            PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id }
        );
        assert_eq!(receipt_count(&mind), receipts, "a replay writes no second receipt");
    }

    /// The same on both sides of a hand-off: the source re-derives the
    /// withdrawal it landed, the receiving mind the assignment.
    #[test]
    fn an_exact_replay_re_derives_both_sides_of_a_hand_off() {
        let away = hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[]);
        let mut source = seeded();
        let (receipt_id, _) = committed(admit(&mut source, vec![away.clone()]));
        let receipts = receipt_count(&source);
        assert_eq!(admit(&mut source, vec![away.clone()]), PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id });
        assert_eq!(receipt_count(&source), receipts);

        let mut receiving = opened(MemoryStore::new(), OTHER_INSTANCE);
        committed(admit(&mut receiving, vec![instance(OTHER_INSTANCE)]));
        let (receipt_id, _) = committed(admit(&mut receiving, vec![away.clone()]));
        let receipts = receipt_count(&receiving);
        assert_eq!(admit(&mut receiving, vec![away]), PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id });
        assert_eq!(receipt_count(&receiving), receipts);
    }

    /// The sequence a ruling's answer takes is the one that ruling's own
    /// answer carries, not whatever answer the question already has. A
    /// question reopened by withdrawing its first answer is answered again at
    /// the next sequence, so the first answer stays readable under it.
    #[test]
    fn a_ruling_answering_a_reopened_question_takes_the_next_sequence() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let mut first = ruling("R1");
        first.answers = Some(s(&id("question", "Q1")));
        first.choice = Some(l("A"));
        committed(admit(&mut mind, vec![D::Ruling(first)]));
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &id("resolution", "question.Q1.n1")), withdrawn())]));
        let mut second = ruling("R2");
        second.answers = Some(s(&id("question", "Q1")));
        second.choice = Some(l("B"));
        let (_, writes) = committed(admit(&mut mind, vec![D::Ruling(second)]));
        assert_eq!(writes, vec![
            r(K::Ruling, &id("ruling", "R2")),
            r(K::Resolution, &id("resolution", "question.Q1.n2")),
        ]);
        assert!(mind.envelope(K::Resolution, &id("resolution", "question.Q1.n1")).is_some());
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

    /// The sequence is exactly the previous plus one: a gap refuses as loudly
    /// as a taken sequence, on both sequenced kinds, and before the question
    /// of what still stands arises.
    #[test]
    fn a_sequence_gap_refuses_on_both_sequenced_kinds() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let subject = r(K::Question, &id("question", "Q1"));
        committed(admit(&mut mind, vec![resolution(subject.clone(), withdrawn())]));
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &id("resolution", "question.Q1.n1")), withdrawn())]));
        committed(admit(&mut mind, vec![resolution_n(subject.clone(), 2, withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution_n(subject, 4, withdrawn())])),
            MindRefusal::ResolutionOutOfSequence { subject: id("question", "Q1"), expected: 3, actual: 4 }
        );

        let mut steward = seeded();
        committed(admit(&mut steward, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let D::HandOff(mut back) = hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) else { panic!() };
        back.handed_on = eureka_pipeline::Date("2026-09-17".into());
        committed(admit(&mut steward, vec![D::HandOff(back)]));
        assert_eq!(
            refusal(admit(&mut steward, vec![stewardship_n(INSTANCE, REPO, 4)])),
            MindRefusal::StewardshipOutOfSequence { repo: REPO.into(), expected: 3, actual: 4 }
        );
    }

    /// The sequence counts the batch as well as the image: two records of one
    /// subject landing together are the second and the third, never two
    /// firsts, and the first of them is refused for the sequence rather than
    /// for what the other closes.
    #[test]
    fn the_latest_resolution_counts_the_batch() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        let subject = r(K::Question, &id("question", "Q1"));
        assert_eq!(
            refusal(admit(&mut mind, vec![
                resolution_n(subject.clone(), 1, withdrawn()),
                resolution_n(subject, 2, ResolutionOutcome::Withdrawn { reason: "again".into() }),
            ])),
            MindRefusal::ResolutionOutOfSequence { subject: id("question", "Q1"), expected: 3, actual: 1 }
        );
    }

    /// The Q19 cap reads the batch too: a chain three deep landing in one
    /// batch is capped exactly as one landing over three admissions.
    #[test]
    fn the_cap_reads_a_chain_landing_in_one_batch() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question("Q1", &["A", "B"], "A")]));
        committed(admit(&mut mind, vec![resolution(r(K::Question, &id("question", "Q1")), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![
                resolution(r(K::Resolution, &id("resolution", "question.Q1.n1")), withdrawn()),
                resolution(r(K::Resolution, &id("resolution", "resolution.question.Q1.n1.n1")), withdrawn()),
            ])),
            MindRefusal::IncompatibleResolution { subject_kind: K::Resolution, outcome: "Withdrawn".into() }
        );
    }

    /// Q18 A and Q20 A at admission: a repo is stewarded by one record at a
    /// time on a mind, and a repo handed away and handed back is a second
    /// record rather than a collision or an overwrite.
    #[test]
    fn a_repo_is_stewarded_once_at_a_time_and_again_after_a_transfer() {
        let mut mind = seeded();
        let first = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1");
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
        let away = format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.gamecult_-epiphany.{}", date().0);
        assert_eq!(writes, vec![
            r(K::HandOff, &away),
            r(K::Resolution, &format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1")),
        ]);
        let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
        let stewardship_envelope = mind.envelope(K::Stewardship, &first).unwrap().clone();
        assert!(
            receipt.strong_reads.contains(&DocumentVersion::from_envelope(&stewardship_envelope)),
            "the derived withdrawal's own citation is pinned: {:?}",
            receipt.strong_reads
        );

        // Nothing stewards the repo now, and a campaign over it still admits:
        // campaign admission reads no stewardship (ruling stewardship-rule).
        let D::Campaign(mut second) = campaign(&[REPO]) else { panic!() };
        second.slug = slug("second");
        committed(admit(&mut mind, vec![D::Campaign(second)]));

        // Handed back, the mind stewards it again, as `n2`. No field names a
        // return: this is the same hand-off shape with the instances swapped.
        let D::HandOff(mut back) = hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) else { panic!() };
        back.handed_on = eureka_pipeline::Date("2026-09-17".into());
        let (_, writes) = committed(admit(&mut mind, vec![D::HandOff(back)]));
        let again = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n2");
        assert_eq!(writes, vec![
            r(K::HandOff, &format!("{OTHER_INSTANCE}:hand_off:{INSTANCE}.gamecult_-epiphany.2026-09-17")),
            r(K::Stewardship, &again),
        ]);
        let Some(D::Stewardship(taken)) = mind.get(K::Stewardship, &again).unwrap() else { panic!() };
        assert_eq!(taken.sequence, 2);
        assert_eq!(taken.assigned_on, eureka_pipeline::Date("2026-09-17".into()));
        assert!(mind.envelope(K::Stewardship, &first).is_some(), "the first assignment is still readable by key");
    }

    /// Q19 A's rationale, which the cap alone does not carry: a withdrawal
    /// never re-raises an earlier record over a later one. The repo went away
    /// and came back, so withdrawing the hand-off's withdrawal would put the
    /// first assignment back in force beside the second.
    #[test]
    fn a_withdrawal_does_not_reinstate_a_stewardship_over_a_later_one() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        let D::HandOff(mut back) = hand_off(OTHER_INSTANCE, INSTANCE, REPO, &[]) else { panic!() };
        back.handed_on = eureka_pipeline::Date("2026-09-17".into());
        committed(admit(&mut mind, vec![D::HandOff(back)]));
        let first = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1");
        let again = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n2");
        let withdrawal = format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1");
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawal), withdrawn())])),
            MindRefusal::WouldReinstateOverLater { subject: first, later: again.clone() }
        );
        let docs = Docs::from_image(mind.envelopes()).unwrap();
        let in_force = docs.stewardships_of(&slug(INSTANCE), &repo(REPO), None).iter().map(|(key, _)| *key).collect::<Vec<_>>();
        assert_eq!(in_force, vec![again.as_str()], "one assignment stands, and it is the later one");
    }

    /// The same rule one kind over: withdrawing a supersession while the
    /// revision that superseded stands would leave two revisions in force.
    #[test]
    fn a_withdrawal_does_not_reinstate_a_revision_over_a_later_one() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![
            target(2, &[INVARIANT]),
            resolution(r(K::Target, &id("target", "r1")), superseded(&[r(K::Target, &id("target", "r2"))])),
        ]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(K::Resolution, &id("resolution", "target.r1.n1")), withdrawn())])),
            MindRefusal::WouldReinstateOverLater { subject: id("target", "r1"), later: id("target", "r2") }
        );
        assert!(docs_of(&mind).in_force(K::Target, &id("target", "r2")));
        assert!(!docs_of(&mind).in_force(K::Target, &id("target", "r1")));
    }

    fn docs_of<S: MindStore>(mind: &Mind<S>) -> Docs {
        Docs::from_image(mind.envelopes()).unwrap()
    }

    /// The withdrawal a hand-off derives is a record of its subject like any
    /// other: the second time the repo leaves, it takes the next sequence.
    #[test]
    fn a_hand_offs_derived_withdrawal_takes_the_next_sequence() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[])]));
        // Nothing later stands over the first assignment, so withdrawing the
        // withdrawal puts it back in force and the repo is this mind's again.
        let withdrawal = format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1");
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawal), withdrawn())]));
        let D::HandOff(mut again) = hand_off(INSTANCE, OTHER_INSTANCE, REPO, &[]) else { panic!() };
        again.handed_on = eureka_pipeline::Date("2026-09-17".into());
        let (_, writes) = committed(admit(&mut mind, vec![D::HandOff(again)]));
        assert_eq!(writes, vec![
            r(K::HandOff, &format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.gamecult_-epiphany.2026-09-17")),
            r(K::Resolution, &format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n2")),
        ]);
    }

    fn hand_off_on(from: &str, to: &str, day: &str) -> D {
        let D::HandOff(mut hand_off) = hand_off(from, to, REPO, &[]) else { panic!() };
        hand_off.handed_on = eureka_pipeline::Date(day.into());
        D::HandOff(hand_off)
    }

    fn stewardship_key(sequence: u32) -> String {
        format!("{INSTANCE}:stewardship:gamecult_-epiphany.n{sequence}")
    }

    /// Which assignment a hand-off's derived withdrawal names is a question of
    /// content, not of order. While the withdrawal is being derived the
    /// hand-off's own is excluded from what stands, so two assignments are in
    /// force at once, and after ten transfers the one that stewards the repo
    /// now sorts first: `n11` before `n2`. The replay must still withdraw the
    /// assignment it withdrew the first time, or it derives a second
    /// withdrawal of a standing assignment and collides instead of answering
    /// with its receipt.
    #[test]
    fn a_source_side_replay_withdraws_the_assignment_it_withdrew() {
        let mut mind = seeded();
        let mut aways = Vec::new();
        for transfer in 1..=10u32 {
            let day = format!("2026-09-{transfer:02}");
            let away = hand_off_on(INSTANCE, OTHER_INSTANCE, &day);
            let (receipt_id, _) = committed(admit(&mut mind, vec![away.clone()]));
            aways.push((away, receipt_id));
            committed(admit(&mut mind, vec![hand_off_on(OTHER_INSTANCE, INSTANCE, &day)]));
        }
        let in_force = docs_of(&mind).stewardships_of(&slug(INSTANCE), &repo(REPO), None).iter().map(|(key, _)| key.to_string()).collect::<Vec<_>>();
        assert_eq!(in_force, vec![stewardship_key(11)]);

        for index in [1usize, 8] {
            let (away, receipt_id) = &aways[index];
            assert_eq!(
                admit(&mut mind, vec![away.clone()]),
                PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: receipt_id.clone() },
                "the replay of transfer {} re-derives its own withdrawal",
                index + 1
            );
        }

        // The standing assignment is handed away and reinstated, so it too
        // carries a withdrawal -- of another hand-off. Sorting first, it is
        // what a match on the outcome alone would pick.
        committed(admit(&mut mind, vec![hand_off_on(INSTANCE, OTHER_INSTANCE, "2026-09-11")]));
        let withdrawal = format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n11.n1");
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawal), withdrawn())]));
        let (away, receipt_id) = &aways[1];
        assert_eq!(
            admit(&mut mind, vec![away.clone()]),
            PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: receipt_id.clone() }
        );
    }

    /// `later_in_force` counts what stands, not what exists: the repo went
    /// away, came back and went away again, so the second assignment is later
    /// than the first but withdrawn, and reinstating the first leaves one
    /// assignment in force rather than two.
    #[test]
    fn a_withdrawn_later_assignment_does_not_block_a_reinstatement() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![hand_off_on(INSTANCE, OTHER_INSTANCE, "2026-09-16")]));
        committed(admit(&mut mind, vec![hand_off_on(OTHER_INSTANCE, INSTANCE, "2026-09-17")]));
        committed(admit(&mut mind, vec![hand_off_on(INSTANCE, OTHER_INSTANCE, "2026-09-18")]));
        let withdrawal = format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1");
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &withdrawal), withdrawn())]));
        let in_force = docs_of(&mind).stewardships_of(&slug(INSTANCE), &repo(REPO), None).iter().map(|(key, _)| key.to_string()).collect::<Vec<_>>();
        assert_eq!(in_force, vec![stewardship_key(1)]);
    }

    /// And it reads the batch: the assignment that stands later than the one
    /// a withdrawal would reinstate may be landing beside that withdrawal,
    /// derived by a hand-off in the same batch.
    #[test]
    fn a_reinstatement_is_blocked_by_an_assignment_in_its_own_batch() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![hand_off_on(INSTANCE, OTHER_INSTANCE, "2026-09-16")]));
        let withdrawal = format!("{INSTANCE}:resolution:stewardship.gamecult_-epiphany.n1.n1");
        assert_eq!(
            refusal(admit(&mut mind, vec![
                hand_off_on(OTHER_INSTANCE, INSTANCE, "2026-09-17"),
                resolution(r(K::Resolution, &withdrawal), withdrawn()),
            ])),
            MindRefusal::WouldReinstateOverLater { subject: stewardship_key(1), later: stewardship_key(2) }
        );
    }

    /// A cut spec's revisions are sequenced within its cut, so another cut's
    /// later revision is not what a withdrawal reads: once cut A's second
    /// revision is gone, reinstating its first commits while cut B's second
    /// stands.
    #[test]
    fn a_reinstatement_reads_the_revisions_of_its_own_cut() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("A", 1)), D::CutSpec(cut_spec("B", 1))]));
        for cut in ["A", "B"] {
            committed(admit(&mut mind, vec![
                D::CutSpec(cut_spec(cut, 2)),
                resolution(
                    r(K::CutSpec, &id("cut_spec", &format!("cut-{cut}.r1"))),
                    superseded(&[r(K::CutSpec, &id("cut_spec", &format!("cut-{cut}.r2")))]),
                ),
            ]));
        }
        let supersession = id("resolution", "cut_spec.cut-A.r1.n1");
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(r(K::Resolution, &supersession), withdrawn())])),
            MindRefusal::WouldReinstateOverLater {
                subject: id("cut_spec", "cut-A.r1"),
                later: id("cut_spec", "cut-A.r2"),
            }
        );
        committed(admit(&mut mind, vec![resolution(r(K::CutSpec, &id("cut_spec", "cut-A.r2")), withdrawn())]));
        committed(admit(&mut mind, vec![resolution(r(K::Resolution, &supersession), withdrawn())]));
        assert!(docs_of(&mind).in_force(K::CutSpec, &id("cut_spec", "cut-A.r1")));
        assert!(docs_of(&mind).in_force(K::CutSpec, &id("cut_spec", "cut-B.r2")));
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
        let stewardship_ref = r(K::Stewardship, &format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1"));
        let campaign_ref = r(K::Campaign, &id("campaign", "self"));
        let report_ref = r(K::CutReport, &id("cut_report", "cut-1.h1"));
        let verdict_ref = r(K::Verdict, &id("verdict", "cut-1.s1"));
        let instance_ref = r(K::Instance, &format!("{INSTANCE}:instance:self"));
        let hand_off_ref = r(K::HandOff, &format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.gamecult_-huginn.2026-09-16"));

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

    /// refusals-typed: a document the leaf cannot key is the client's fault.
    /// The finding's local is 64 bytes of label under `cut-1.s1.`; the
    /// resolution's subject is a resolution whose own local leaves no room for
    /// a third nesting. Both pass `validate` and fail only at the key.
    fn unkeyable() -> Vec<D> {
        let wide = D::Finding(finding("1", 1, &"w".repeat(64), FindingConfidence::Confirmed));
        let deep_subject = id("resolution", &format!("{}.{}", "x".repeat(60), "y".repeat(40)));
        vec![wide, resolution(r(K::Resolution, &deep_subject), withdrawn())]
    }

    #[test]
    fn an_unkeyable_document_is_refused_as_a_document() {
        let store = RefusingStore::new();
        let mut mind = opened(store.clone(), INSTANCE);
        seed(&mut mind);
        let before = store.rows();
        let receipts = receipt_count(&mind);
        for (document, field) in unkeyable().into_iter().zip(["finding.key", "resolution.key"]) {
            document.validate().unwrap();
            // The leaf owns what a key error says; admission must carry it
            // unchanged, field and offending value included.
            let leaf = pipeline_key(&document).expect_err("the fixture must not key");
            assert!(
                matches!(&leaf, PipelineRefusal::InvalidFormat { field: named, value } if named == field && !value.is_empty()),
                "{leaf:?}"
            );
            assert_eq!(refusal(admit(&mut mind, vec![document])), MindRefusal::Document(leaf), "{field}");
            assert_eq!(store.rows(), before);
            assert_eq!(receipt_count(&mind), receipts);
        }
    }

    /// The derived path (`admit_steps`) shares the helper. No derivable
    /// document can fail to key after the leaf's per-kind bound (a derived
    /// resolution is depth one or two over a subject of at most 64 bytes), so
    /// the helper is pinned directly.
    #[test]
    fn prepare_classifies_a_key_refusal_as_a_document_and_encodes_the_rest() {
        let cache = schema_cache().unwrap();
        let keyable = question("Q1", &["A", "B"], "A");
        assert_eq!(super::prepare(&keyable, &cache).unwrap(), prepare(&keyable));
        for document in unkeyable() {
            let leaf = pipeline_key(&document).expect_err("the fixture must not key");
            assert_eq!(super::prepare(&document, &cache).unwrap_err(), MindRefusal::Document(leaf));
        }
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
        quoted.authority = RulingAuthority::Mind;
        assert_eq!(refusal(admit(&mut mind, vec![D::Ruling(quoted.clone())])), MindRefusal::QuoteWithoutOperator { ruling: id("ruling", "R1") });
        assert!(mind.get(K::Ruling, &id("ruling", "R1")).unwrap().is_none(), "a refused batch leaves the store unchanged");
        quoted.authority = RulingAuthority::Operator;
        committed(admit(&mut mind, vec![D::Ruling(quoted)]));
    }

    #[test]
    fn a_mind_ruling_without_a_quote_is_admitted() {
        let mut mind = seeded();
        let mut own = ruling("R1");
        own.authority = RulingAuthority::Mind;
        committed(admit(&mut mind, vec![D::Ruling(own)]));
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
        report.range.head = eureka_pipeline::Sha("abcdef0".into());
        assert_eq!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::RangeOutsideCommits { head: "abcdef0".into() });
        committed(admit(&mut mind, vec![resolution(r(K::CutSpec, &id("cut_spec", "cut-1.r1")), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![D::CutReport(cut_report("1", 1))])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: id("cut_spec", "cut-1.r1") }
        );
    }

    fn spec_ref(cut: &str) -> PipelineRef {
        r(K::CutSpec, &id("cut_spec", &format!("cut-{cut}.r1")))
    }

    fn run_key(label: &str) -> String {
        format!("{INSTANCE}:run:{label}")
    }

    /// A seeded mind holding cut specs 1 and 2.
    fn with_specs() -> Mind<MemoryStore> {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1)), D::CutSpec(cut_spec("2", 1))]));
        mind
    }

    fn recorded(label: &str) -> PipelineDocument {
        resolution(r(K::Run, &run_key(label)), ResolutionOutcome::Recorded { reason: "finished".into() })
    }

    /// run-is-the-grant: the claim is the only consumption fact. A second run
    /// on a claim a live run holds is AlreadyClaimed, naming the claim and the
    /// holder; another claim is free; once the holder is Recorded the claim is
    /// free, and withdrawing that record makes the holder live again.
    #[test]
    fn a_second_run_on_a_live_claim_is_already_claimed() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("1")])]));
        let claimed = MindRefusal::AlreadyClaimed { item: spec_ref("1").id.0, run: run_key("a") };
        assert_eq!(refusal(admit(&mut mind, vec![run("b", &[spec_ref("1")])])), claimed);
        assert_eq!(refusal(admit(&mut mind, vec![run("b", &[spec_ref("2"), spec_ref("1")])])), claimed, "any one held claim refuses the run");
        committed(admit(&mut mind, vec![run("b", &[spec_ref("2")])]));
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![run("c", &[spec_ref("1")])]));

        let mut reopened = with_specs();
        committed(admit(&mut reopened, vec![run("a", &[spec_ref("1")])]));
        committed(admit(&mut reopened, vec![recorded("a")]));
        let closure = r(K::Resolution, &format!("{INSTANCE}:resolution:run.a.n1"));
        committed(admit(&mut reopened, vec![resolution(closure, withdrawn())]));
        assert_eq!(refusal(admit(&mut reopened, vec![run("d", &[spec_ref("1")])])), claimed);
    }

    /// one-live-self-run at admission: a second run of hers of one turn is
    /// AlreadyLive naming the holder, whether the holder is claimless or
    /// claiming, and a claim the holder holds is still AlreadyClaimed first.
    #[test]
    fn a_second_live_run_of_hers_of_one_turn_is_already_live() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[])]));
        let live_a = MindRefusal::AlreadyLive { run: run_key("a") };
        assert_eq!(refusal(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[])])), live_a, "a claimless holder");
        assert_eq!(refusal(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[spec_ref("1")])])), live_a);
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[spec_ref("1")])]));
        let live_b = MindRefusal::AlreadyLive { run: run_key("b") };
        assert_eq!(refusal(admit(&mut mind, vec![mind_run("c", RunTurn::SelfRun, &[spec_ref("2")])])), live_b, "a claiming holder");
        let claimed = MindRefusal::AlreadyClaimed { item: spec_ref("1").id.0, run: run_key("b") };
        assert_eq!(refusal(admit(&mut mind, vec![mind_run("c", RunTurn::SelfRun, &[spec_ref("1")])])), claimed, "claims answer first");
    }

    /// The rule counts only in-force runs of hers of the same turn: an
    /// operator's run, a run of the other turn and a run after the holder is
    /// Recorded are admitted, and one live PersonaTurn and one live SelfRun of
    /// hers coexist.
    #[test]
    fn an_operators_run_another_turn_and_a_run_after_the_holder_closed_are_admitted() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[])]));
        committed(admit(&mut mind, vec![run("op1", &[])]));
        committed(admit(&mut mind, vec![run("op2", &[])]));
        committed(admit(&mut mind, vec![mind_run("p", RunTurn::PersonaTurn, &[])]));
        let live_p = MindRefusal::AlreadyLive { run: run_key("p") };
        assert_eq!(refusal(admit(&mut mind, vec![mind_run("p2", RunTurn::PersonaTurn, &[])])), live_p);
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[])]));
        committed(admit(&mut mind, vec![recorded("b")]));
        committed(admit(&mut mind, vec![mind_run("c", RunTurn::SelfRun, &[])]));

        let mut operator_only = with_specs();
        committed(admit(&mut operator_only, vec![run("op", &[])]));
        committed(admit(&mut operator_only, vec![mind_run("a", RunTurn::SelfRun, &[])]));
    }

    /// Two runs of hers of one turn in one batch hold nothing: the batch is
    /// checked against itself, refused whole, and nothing is written.
    #[test]
    fn two_live_runs_of_hers_in_one_batch_are_already_live_and_nothing_commits() {
        let mut mind = with_specs();
        let refused = refusal(admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[]), mind_run("b", RunTurn::SelfRun, &[])]));
        assert!(matches!(refused, MindRefusal::AlreadyLive { .. }), "{refused:?}");
        assert!(mind.envelope(K::Run, &run_key("a")).is_none() && mind.envelope(K::Run, &run_key("b")).is_none());
    }

    /// Withdrawing the close of a run of hers reinstates it, and a reinstated
    /// run is a live one: refused while another run of its turn is live,
    /// allowed when none is.
    #[test]
    fn reinstating_a_run_of_hers_over_a_live_holder_is_already_live() {
        let closure = r(K::Resolution, &format!("{INSTANCE}:resolution:run.a.n1"));
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[])]));
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[])]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(closure.clone(), withdrawn())])),
            MindRefusal::AlreadyLive { run: run_key("b") }
        );
        committed(admit(&mut mind, vec![recorded("b")]));
        committed(admit(&mut mind, vec![resolution(closure, withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![mind_run("c", RunTurn::SelfRun, &[])])),
            MindRefusal::AlreadyLive { run: run_key("a") },
            "the reinstated run holds the slot"
        );
    }

    /// A retry of an admitted run answers AlreadyAdmitted even when another
    /// run of its turn has since become the live one.
    #[test]
    fn replaying_an_admitted_run_of_hers_after_another_took_the_turn_is_already_admitted() {
        let mut mind = with_specs();
        let (first, _) = committed(admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[])]));
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![mind_run("b", RunTurn::SelfRun, &[])]));
        assert_eq!(
            admit(&mut mind, vec![mind_run("a", RunTurn::SelfRun, &[])]),
            PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: first }
        );
    }

    /// Two openers on one claim in one batch: the claim is held by neither, so
    /// the batch is refused whole and nothing is written. One opener at a time
    /// is `a_second_run_on_a_live_claim_is_already_claimed`.
    #[test]
    fn two_openers_in_one_batch_hold_nothing() {
        let mut mind = with_specs();
        let refused = refusal(admit(&mut mind, vec![run("a", &[spec_ref("1")]), run("b", &[spec_ref("1")])]));
        assert!(matches!(refused, MindRefusal::AlreadyClaimed { .. }), "{refused:?}");
        assert!(mind.envelope(K::Run, &run_key("a")).is_none() && mind.envelope(K::Run, &run_key("b")).is_none());
    }

    /// Every claim names a document that exists and is in force.
    #[test]
    fn a_runs_claims_must_exist_and_be_in_force() {
        let mut mind = with_specs();
        assert_eq!(
            refusal(admit(&mut mind, vec![run("a", &[spec_ref("9")])])),
            MindRefusal::MissingReference { kind: K::CutSpec, id: spec_ref("9").id.0 }
        );
        committed(admit(&mut mind, vec![resolution(spec_ref("2"), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![run("a", &[spec_ref("2")])])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: spec_ref("2").id.0 }
        );
        assert!(mind.envelope(K::Run, &run_key("a")).is_none());
    }

    /// A replay of an admitted run is AlreadyAdmitted even when its claim has
    /// since been closed: a retry owes nothing to the work moving on. A
    /// different run on that closed claim is still refused.
    #[test]
    fn replaying_an_admitted_run_after_its_claim_closed_is_already_admitted() {
        let mut mind = with_specs();
        let (first, _) = committed(admit(&mut mind, vec![run("a", &[spec_ref("1")])]));
        committed(admit(&mut mind, vec![resolution(spec_ref("1"), withdrawn())]));
        assert_eq!(admit(&mut mind, vec![run("a", &[spec_ref("1")])]), PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: first });
        assert_eq!(
            refusal(admit(&mut mind, vec![run("b", &[spec_ref("1")])])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: spec_ref("1").id.0 }
        );
    }

    /// Withdrawing a run's Recorded resolution puts the run back in force, so
    /// it goes through the same exclusivity as an opening: refused while
    /// another live run holds one of its claims, allowed when none does.
    #[test]
    fn reinstating_a_run_over_a_live_holder_is_already_claimed() {
        let closure = r(K::Resolution, &format!("{INSTANCE}:resolution:run.a.n1"));
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("1")])]));
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![run("c", &[spec_ref("1")])]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(closure.clone(), withdrawn())])),
            MindRefusal::AlreadyClaimed { item: spec_ref("1").id.0, run: run_key("c") }
        );

        let mut free = with_specs();
        committed(admit(&mut free, vec![run("a", &[spec_ref("1")])]));
        committed(admit(&mut free, vec![recorded("a")]));
        committed(admit(&mut free, vec![resolution(closure, withdrawn())]));
    }

    /// Reinstatement finds a contested claim anywhere in the run's list: a
    /// reinstated run of two claims where only the second is held is refused,
    /// and the first alone being contested is refused too.
    #[test]
    fn reinstating_a_run_finds_a_contested_second_claim() {
        let closure = r(K::Resolution, &format!("{INSTANCE}:resolution:run.a.n1"));
        for (held, expected) in [("2", "2"), ("1", "1")] {
            let mut mind = with_specs();
            committed(admit(&mut mind, vec![run("a", &[spec_ref("1"), spec_ref("2")])]));
            committed(admit(&mut mind, vec![recorded("a")]));
            committed(admit(&mut mind, vec![run("c", &[spec_ref(held)])]));
            assert_eq!(
                refusal(admit(&mut mind, vec![resolution(closure.clone(), withdrawn())])),
                MindRefusal::AlreadyClaimed { item: spec_ref(expected).id.0, run: run_key("c") }
            );
        }
    }

    /// Reinstating a run needs every claim in force, as opening it does: once
    /// a claimed item has resolved the withdrawal is refused, and while all
    /// stay in force it is allowed.
    #[test]
    fn reinstating_a_run_needs_its_claims_in_force() {
        let closure = r(K::Resolution, &format!("{INSTANCE}:resolution:run.a.n1"));
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("1"), spec_ref("2")])]));
        committed(admit(&mut mind, vec![recorded("a")]));
        committed(admit(&mut mind, vec![resolution(spec_ref("2"), withdrawn())]));
        assert_eq!(
            refusal(admit(&mut mind, vec![resolution(closure.clone(), withdrawn())])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: spec_ref("2").id.0 }
        );

        let mut live = with_specs();
        committed(admit(&mut live, vec![run("a", &[spec_ref("1"), spec_ref("2")])]));
        committed(admit(&mut live, vec![recorded("a")]));
        committed(admit(&mut live, vec![resolution(closure, withdrawn())]));
    }

    /// A holder is found by any of its claims, not only the first.
    #[test]
    fn a_holder_is_found_by_its_second_claim() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("2"), spec_ref("1")])]));
        assert_eq!(
            refusal(admit(&mut mind, vec![run("b", &[spec_ref("1")])])),
            MindRefusal::AlreadyClaimed { item: spec_ref("1").id.0, run: run_key("a") }
        );
    }

    /// A key already holding a run is taken: the same key with different
    /// content is an IdentityCollision, never a replay; only byte-identical
    /// content answers AlreadyAdmitted.
    #[test]
    fn a_run_key_reused_with_different_content_is_an_identity_collision() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("1")])]));
        let D::Run(mut changed) = run("a", &[spec_ref("1")]) else { panic!() };
        changed.budget_usd = "6".into();
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Run(changed)])),
            MindRefusal::IdentityCollision { kind: K::Run, id: run_key("a") }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![run("a", &[spec_ref("2")])])),
            MindRefusal::IdentityCollision { kind: K::Run, id: run_key("a") }
        );
    }

    /// The replay clause is byte identity, not the key: a changed run under an
    /// admitted key whose claim has since closed is not a replay, so the closed
    /// claim refuses it.
    #[test]
    fn a_changed_run_under_an_admitted_key_is_not_a_replay() {
        let mut mind = with_specs();
        committed(admit(&mut mind, vec![run("a", &[spec_ref("1")])]));
        committed(admit(&mut mind, vec![resolution(spec_ref("1"), withdrawn())]));
        let D::Run(mut changed) = run("a", &[spec_ref("1")]) else { panic!() };
        changed.budget_usd = "6".into();
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Run(changed)])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: spec_ref("1").id.0 }
        );
    }

    /// A5 for runs: a run of another instance is a ForeignInstance.
    #[test]
    fn a_run_of_another_mind_is_a_foreign_instance() {
        let mut mind = with_specs();
        let D::Run(mut foreign) = run("a", &[]) else { panic!() };
        foreign.instance = slug(OTHER_INSTANCE);
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Run(foreign)])),
            MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() }
        );
    }

    /// A run ends: Recorded or Withdrawn, and no other outcome.
    #[test]
    fn a_run_resolves_recorded_or_withdrawn_and_nothing_else() {
        let incompatible = |outcome: &str| MindRefusal::IncompatibleResolution { subject_kind: K::Run, outcome: outcome.into() };
        let subject = || r(K::Run, &run_key("a"));
        let open = || {
            let mut mind = with_specs();
            committed(admit(&mut mind, vec![D::Ruling(ruling("R1")), run("a", &[spec_ref("1")]), run("z", &[])]));
            mind
        };
        committed(admit(&mut open(), vec![recorded("a")]));
        committed(admit(&mut open(), vec![resolution(subject(), withdrawn())]));
        let other_run = r(K::Run, &run_key("z"));
        let refused = [
            (superseded(&[other_run.clone()]), "Superseded"),
            (ResolutionOutcome::Answered { by: r(K::Ruling, &id("ruling", "R1")) }, "Answered"),
            (ResolutionOutcome::Fixed { commit: sha(), by: None }, "Fixed"),
            (ResolutionOutcome::Deferred { to: other_run }, "Deferred"),
        ];
        for (outcome, name) in refused {
            assert_eq!(refusal(admit(&mut open(), vec![resolution(subject(), outcome)])), incompatible(name), "{name}");
        }
    }

    #[test]
    fn depends_on_names_cuts() {
        let depending = |cut: &str, on: &[&str]| {
            let mut spec = cut_spec(cut, 1);
            spec.depends_on = on.iter().map(|label| s(label)).collect();
            D::CutSpec(spec)
        };
        let mut mind = seeded();
        // An unknown label, and a spec id (a revision, not a cut), are refused.
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("2", &["1"])])),
            MindRefusal::UnknownDependency { cut: "1".into() }
        );
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("2", &[&id("cut_spec", "cut-1.r1")])])),
            MindRefusal::UnknownDependency { cut: id("cut_spec", "cut-1.r1") }
        );
        // The same batch carries the cut it depends on.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1)), depending("2", &["1"])]));
        // The image carries it, and any revision of the cut names it.
        committed(admit(&mut mind, vec![depending("3", &["1", "2"])]));
        // A cut of another campaign is not this campaign's cut.
        let mut other = cut_spec("9", 1);
        other.campaign = slug("elsewhere");
        let mut elsewhere = campaign(&[REPO]);
        if let D::Campaign(campaign) = &mut elsewhere {
            campaign.slug = slug("elsewhere");
        }
        committed(admit(&mut mind, vec![elsewhere, D::CutSpec(other)]));
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("4", &["9"])])),
            MindRefusal::UnknownDependency { cut: "9".into() }
        );
    }

    #[test]
    fn depends_on_matches_cut_labels_whole() {
        let depending = |cut: &str, on: &[&str]| {
            let mut spec = cut_spec(cut, 1);
            spec.depends_on = on.iter().map(|label| s(label)).collect();
            D::CutSpec(spec)
        };
        // The image carries cut 10: neither its prefix nor an extension of it names it.
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("10", 1))]));
        for near in ["1", "100"] {
            assert_eq!(
                refusal(admit(&mut mind, vec![depending("11", &[near])])),
                MindRefusal::UnknownDependency { cut: near.into() }
            );
        }
        // The same holds when the cut arrives in the same batch.
        for near in ["2", "200"] {
            assert_eq!(
                refusal(admit(&mut mind, vec![D::CutSpec(cut_spec("20", 1)), depending("21", &[near])])),
                MindRefusal::UnknownDependency { cut: near.into() }
            );
        }
        committed(admit(&mut mind, vec![depending("12", &["10"])]));
    }

    #[test]
    fn a_cut_may_not_depend_on_itself() {
        let depending = |cut: &str, revision: u32, on: &[&str]| {
            let mut spec = cut_spec(cut, revision);
            spec.depends_on = on.iter().map(|label| s(label)).collect();
            D::CutSpec(spec)
        };
        let mut mind = seeded();
        // Alone in the batch, and beside another cut it legitimately names.
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("1", 1, &["1"])])),
            MindRefusal::UnknownDependency { cut: "1".into() }
        );
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("2", 1))]));
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("1", 1, &["2", "1"])])),
            MindRefusal::UnknownDependency { cut: "1".into() }
        );
        // An earlier revision of the same cut in the image does not make it a real dependency.
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
        assert_eq!(
            refusal(admit(&mut mind, vec![depending("1", 2, &["1"])])),
            MindRefusal::UnknownDependency { cut: "1".into() }
        );
    }

    #[test]
    fn a_report_head_may_spell_its_commit_short_or_full() {
        let full = "5f98228d0123456789abcdef0123456789abcdef";
        let mut mind = seeded();
        committed(admit(&mut mind, vec![D::CutSpec(cut_spec("1", 1))]));
        // The commit is spelled short and the head full, and the reverse.
        let mut report = cut_report("1", 1);
        report.range.head = eureka_pipeline::Sha(full.into());
        committed(admit(&mut mind, vec![D::CutReport(report)]));
        let mut report = cut_report("1", 2);
        report.commits[0].sha = eureka_pipeline::Sha(full.into());
        committed(admit(&mut mind, vec![D::CutReport(report)]));
        // A head that is no commit's prefix, long or short, is outside.
        let mut report = cut_report("1", 3);
        report.range.head = eureka_pipeline::Sha("5f98229d0123456789abcdef0123456789abcdef".into());
        assert!(matches!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::RangeOutsideCommits { .. }));
        let mut report = cut_report("1", 3);
        report.commits[0].sha = eureka_pipeline::Sha(full.into());
        report.range.head = eureka_pipeline::Sha("5f98229".into());
        assert!(matches!(refusal(admit(&mut mind, vec![D::CutReport(report)])), MindRefusal::RangeOutsideCommits { .. }));
    }

    #[test]
    fn a_finding_defers_to_a_cut_spec() {
        let finding_ref = r(K::Finding, &id("finding", "cut-1.s1.F1"));
        let defer_to = |to: PipelineRef| resolution(finding_ref.clone(), ResolutionOutcome::Deferred { to });
        committed(admit(&mut world(), vec![defer_to(r(K::CutSpec, &id("cut_spec", "cut-1.r1")))]));
        // A spec superseded since is no longer a place to defer to.
        let mut mind = world();
        committed(admit(&mut mind, vec![
            D::CutSpec(cut_spec("1", 2)),
            resolution(r(K::CutSpec, &id("cut_spec", "cut-1.r1")), superseded(&[r(K::CutSpec, &id("cut_spec", "cut-1.r2"))])),
        ]));
        assert_eq!(
            refusal(admit(&mut mind, vec![defer_to(r(K::CutSpec, &id("cut_spec", "cut-1.r1")))])),
            MindRefusal::CitesResolvedDocument { kind: K::CutSpec, id: id("cut_spec", "cut-1.r1") }
        );
        assert_eq!(
            refusal(admit(&mut world(), vec![defer_to(r(K::Ruling, &id("ruling", "R1")))])),
            MindRefusal::IncompatibleResolution { subject_kind: K::Finding, outcome: "Deferred".into() }
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
            label: eureka_pipeline::Label,
            confidence: FindingConfidence,
            severity: eureka_pipeline::FindingSeverity,
            claim: Line,
            invariants: Vec<eureka_pipeline::Label>,
            locations: Vec<eureka_pipeline::CodeLocation>,
            failure_scenario: eureka_pipeline::Para,
            evidence: Vec<eureka_pipeline::Evidence>,
            precedents: Vec<eureka_pipeline::ForeignRef>,
            origin: eureka_pipeline::FindingOrigin,
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
    fn a_campaign_needs_no_stewardship() {
        let mut mind = opened(MemoryStore::new(), INSTANCE);
        committed(admit(&mut mind, vec![instance(INSTANCE), campaign(&[REPO])]));
        let D::Campaign(mut empty) = campaign(&[]) else { panic!() };
        empty.slug = slug("other");
        assert_eq!(
            refusal(admit(&mut mind, vec![D::Campaign(empty)])),
            MindRefusal::EmptyRepos { campaign: "other:campaign:self".into() }
        );
        let mut spec = cut_spec("1", 1);
        spec.repo = repo(OTHER_REPO);
        assert_eq!(refusal(admit(&mut mind, vec![D::CutSpec(spec)])), MindRefusal::RepoNotInCampaign { repo: OTHER_REPO.into() });
    }

    #[test]
    fn a_hand_off_derives_this_minds_side_only() {
        let key = format!("{INSTANCE}:hand_off:{OTHER_INSTANCE}.gamecult_-epiphany.2026-09-16");
        let campaign_key = id("campaign", "self");
        let stewardship_key = format!("{INSTANCE}:stewardship:gamecult_-epiphany.n1");

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
        let assigned = format!("{OTHER_INSTANCE}:stewardship:gamecult_-epiphany.n1");
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
        let ordinal = receipt::head(&mind).unwrap() + 1;
        let candidate =
            receipt::candidate(&slug(INSTANCE), provenance(Faculty::Hands), &[], &envelopes, ordinal, now()).unwrap();
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
