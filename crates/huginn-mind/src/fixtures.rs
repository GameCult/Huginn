//! Test fixtures shared by the modules' tests: one valid document of each
//! kind, prepared through the leaf against the organ's cache, and a seeded
//! mind to admit into.

use chrono::{DateTime, TimeZone, Utc};
use cultcache_rs::CultCacheEnvelope;
use epiphany_pipeline::{
    AuthorityMap, ClaimOutcome, CodeLocation, CommitRange, CutVerification, Date, DocRef, Evidence, EvidenceKind,
    FindingConfidence, FindingOrigin, FindingSeverity, Label, Line, MutationRecord, OrgRepo, PipelineCampaign,
    PipelineCutReport, PipelineCutSpec, PipelineDocument, PipelineFinding, PipelineFollowUp, PipelineHandOff,
    PipelineInstance, PipelineKind, PipelineQuestion, PipelineRef, PipelineResolution, PipelineRuling,
    PipelineStewardship, PipelineTarget, PipelineVerdict, Promise, QuestionOption, ReportCommit, ResolutionOutcome,
    RulingAuthority, Sha, Short, Slug, StructuralDelta, TargetInvariant, VerdictClaim,
};

use crate::admission::{PipelineAdmissionBatch, PipelineAdmissionOutcome};
use crate::mind::{HuginnMindEpoch, Mind, schema_cache};
use crate::receipt::{Faculty, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::store::MindStore;
use crate::store::test_stores::MemoryStore;

pub(crate) const INSTANCE: &str = "yggdrasil";
pub(crate) const OTHER_INSTANCE: &str = "thought-cage";
pub(crate) const CAMPAIGN: &str = "eureka-state";
pub(crate) const REPO: &str = "GameCult/Epiphany";
pub(crate) const OTHER_REPO: &str = "GameCult/Huginn";
pub(crate) const INVARIANT: &str = "mind-admits";

pub(crate) fn slug(value: &str) -> Slug {
    value.into()
}

pub(crate) fn s(value: &str) -> Short {
    value.into()
}

pub(crate) fn l(value: &str) -> Label {
    value.into()
}

pub(crate) fn repo(value: &str) -> OrgRepo {
    value.into()
}

pub(crate) fn date() -> Date {
    Date("2026-09-16".into())
}

pub(crate) fn sha() -> Sha {
    Sha("5f98228d".into())
}

pub(crate) fn id(kind: &str, local: &str) -> String {
    format!("{CAMPAIGN}:{kind}:{local}")
}

pub(crate) fn r(kind: PipelineKind, id: &str) -> PipelineRef {
    PipelineRef { kind, id: s(id) }
}

fn range() -> CommitRange {
    CommitRange { base: sha(), head: sha() }
}

fn location() -> CodeLocation {
    CodeLocation { path: s("crates/huginn-mind/src/admission.rs"), line: 1, end_line: None }
}

pub(crate) fn evidence() -> Evidence {
    Evidence { kind: EvidenceKind::Test, locator: "cargo test".into(), result: "ok".into() }
}

fn doc_ref() -> DocRef {
    DocRef { path: s("notes/eureka-pipeline-state-target.md"), start_line: 1, end_line: 9, commit: sha() }
}

fn delta() -> StructuralDelta {
    StructuralDelta {
        lines_added: 0,
        lines_removed: 0,
        dependencies_added: vec![],
        dependencies_removed: vec![],
        formats_added: vec![],
        formats_removed: vec![],
        targets_added: vec![],
        targets_removed: vec![],
    }
}

pub(crate) fn instance(name: &str) -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: slug(name),
        display_name: s(&format!("{name} mind")),
        created_at: date(),
        host: s(name),
    })
}

pub(crate) fn stewardship(instance: &str, repo_name: &str) -> PipelineDocument {
    PipelineDocument::Stewardship(PipelineStewardship {
        instance: slug(instance),
        repo: repo(repo_name),
        assigned_on: date(),
        note: "assigned".into(),
    })
}

pub(crate) fn campaign(repos: &[&str]) -> PipelineDocument {
    PipelineDocument::Campaign(PipelineCampaign {
        slug: slug(CAMPAIGN),
        title: s("Eureka pipeline state"),
        repos: repos.iter().map(|name| repo(name)).collect(),
        working_branch: s("codex/eureka-pipeline-state"),
        target_doc: doc_ref(),
    })
}

pub(crate) fn target(revision: u32, invariants: &[&str]) -> PipelineDocument {
    PipelineDocument::Target(PipelineTarget {
        campaign: slug(CAMPAIGN),
        revision,
        invariants: invariants
            .iter()
            .map(|label| TargetInvariant { label: l(label), statement: "Only admission writes.".into() })
            .collect(),
        not_in_scope: vec![],
        canonical_implementations: vec![],
        doc: doc_ref(),
    })
}

pub(crate) fn question(label: &str, options: &[&str], recommended: &str) -> PipelineDocument {
    PipelineDocument::Question(PipelineQuestion {
        campaign: slug(CAMPAIGN),
        label: l(label),
        question: "Who owns the state?".into(),
        options: options.iter().map(|option| QuestionOption { label: l(option), text: "an option".into() }).collect(),
        recommended: l(recommended),
        depends: vec![],
        raised_in: None,
        asked_on: date(),
    })
}

pub(crate) fn ruling(label: &str) -> PipelineRuling {
    PipelineRuling {
        campaign: slug(CAMPAIGN),
        label: l(label),
        answers: None,
        choice: None,
        ruling: "An instance owns its mind.".into(),
        operator_quote: None,
        ruled_on: date(),
        precedents: vec![],
        authority: RulingAuthority::Standing,
    }
}

pub(crate) fn cut_spec(cut: &str, revision: u32) -> PipelineCutSpec {
    PipelineCutSpec {
        campaign: slug(CAMPAIGN),
        cut: l(cut),
        revision,
        title: s("A cut"),
        repo: repo(REPO),
        branch: s("codex/eureka-pipeline-state"),
        base: sha(),
        depends_on: vec![],
        first: vec![],
        deletes: vec![],
        keeps_moves: vec![],
        adds: vec![],
        file_changes: vec![],
        authority_map: None::<AuthorityMap>,
        verification: CutVerification { builds: vec![], tests: vec![], negative: vec![], operator: vec![] },
        estimate: delta(),
        rulings: vec![],
        questions: vec![],
    }
}

pub(crate) fn cut_report(cut: &str, attempt: u32) -> PipelineCutReport {
    PipelineCutReport {
        campaign: slug(CAMPAIGN),
        cut_spec: s(&id("cut_spec", &format!("cut-{cut}.r1"))),
        attempt,
        repo: repo(REPO),
        branch: s("codex/eureka-pipeline-state"),
        commits: vec![ReportCommit { sha: sha(), subject: "Land the cut".into(), builds: true }],
        range: range(),
        verification: vec![evidence()],
        mutations: vec![MutationRecord {
            label: l("M1"),
            rule: "the rule".into(),
            location: location(),
            before: "before".into(),
            after: "after".into(),
            commit: sha(),
            failed_as_expected: true,
        }],
        deviations: vec![],
        forks: vec![],
        structural_delta: delta(),
        landed_names: vec![],
        undone: vec![],
        promises: vec![Promise { label: l("P1"), text: "One derived key per document.".into() }],
    }
}

pub(crate) fn claim(outcome: ClaimOutcome, findings: &[&str], promise: Option<&str>, mutations: &[&str]) -> VerdictClaim {
    VerdictClaim {
        claim: Line(format!("{outcome:?} claim")),
        outcome,
        evidence: vec![evidence()],
        findings: findings.iter().map(|finding| s(finding)).collect(),
        promise: promise.map(l),
        mutations: mutations.iter().map(|label| l(label)).collect(),
    }
}

pub(crate) fn verdict(cut: &str, pass: u32, claims: Vec<VerdictClaim>) -> PipelineDocument {
    PipelineDocument::Verdict(PipelineVerdict {
        campaign: slug(CAMPAIGN),
        cut_report: s(&id("cut_report", &format!("cut-{cut}.h1"))),
        pass,
        range: range(),
        claims,
    })
}

pub(crate) fn finding(cut: &str, pass: u32, label: &str, confidence: FindingConfidence) -> PipelineFinding {
    PipelineFinding {
        campaign: slug(CAMPAIGN),
        verdict: s(&id("verdict", &format!("cut-{cut}.s{pass}"))),
        label: l(label),
        range: range(),
        confidence,
        severity: FindingSeverity::High,
        claim: "A dotted label composes two keys.".into(),
        invariants: vec![l(INVARIANT)],
        locations: vec![location()],
        failure_scenario: "Two documents claim one key.".into(),
        evidence: vec![evidence()],
        precedents: vec![],
        origin: FindingOrigin::Introduced,
    }
}

pub(crate) fn follow_up(label: &str, source: PipelineRef) -> PipelineDocument {
    PipelineDocument::FollowUp(PipelineFollowUp {
        campaign: slug(CAMPAIGN),
        label: l(label),
        source,
        repo: repo(REPO),
        locations: vec![location()],
        item: "Per-kind admission rules.".into(),
        why_it_can_wait: "The organ owns admission.".into(),
        owner: s("Hands"),
    })
}

pub(crate) fn resolution(subject: PipelineRef, outcome: ResolutionOutcome) -> PipelineDocument {
    PipelineDocument::Resolution(PipelineResolution {
        subject,
        outcome,
        rationale: "Resolved.".into(),
        resolved_on: date(),
    })
}

pub(crate) fn withdrawn() -> ResolutionOutcome {
    ResolutionOutcome::Withdrawn { reason: "moot".into() }
}

pub(crate) fn superseded(by: &[PipelineRef]) -> ResolutionOutcome {
    ResolutionOutcome::Superseded { by: by.to_vec() }
}

pub(crate) fn hand_off(from: &str, to: &str, repo_name: &str, documents: &[&str]) -> PipelineDocument {
    PipelineDocument::HandOff(PipelineHandOff {
        from_instance: slug(from),
        to_instance: slug(to),
        repo: repo(repo_name),
        documents: documents.iter().map(|document| s(document)).collect(),
        reason: "The workstation mind takes the campaign.".into(),
        handed_on: date(),
    })
}

pub(crate) fn provenance(faculty: Faculty) -> PipelineProvenance {
    PipelineProvenance { faculty, agent: s("claude"), session: s("session-1"), tool: s("admit") }
}

pub(crate) fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 16, 12, 0, 0).unwrap()
}

/// The leaf's envelope for a document, keyed and encoded as the organ stores
/// it.
pub(crate) fn prepare(document: &PipelineDocument) -> CultCacheEnvelope {
    document.validate().unwrap();
    document.prepare(&schema_cache().unwrap()).unwrap()
}

/// The epoch record admission derives on the first write.
pub(crate) fn epoch() -> CultCacheEnvelope {
    HuginnMindEpoch::envelope(&schema_cache().unwrap()).unwrap()
}

pub(crate) fn opened<S: MindStore>(store: S, instance: &str) -> Mind<S> {
    Mind::open_with(store, &slug(instance)).unwrap()
}

/// Admits as Hands at the fixed clock.
pub(crate) fn admit<S: MindStore>(mind: &mut Mind<S>, documents: Vec<PipelineDocument>) -> PipelineAdmissionOutcome {
    let batch = PipelineAdmissionBatch { instance: mind.instance().clone(), provenance: provenance(Faculty::Hands), documents };
    mind.admit(batch, now())
}

/// The first write into a fresh yggdrasil mind: its identity, stewardship of
/// the campaign repo, the campaign and a first target with one invariant.
pub(crate) fn seed<S: MindStore>(mind: &mut Mind<S>) {
    let outcome = admit(mind, vec![instance(INSTANCE), stewardship(INSTANCE, REPO), campaign(&[REPO]), target(1, &[INVARIANT])]);
    assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
}

pub(crate) fn seeded() -> Mind<MemoryStore> {
    let mut mind = opened(MemoryStore::new(), INSTANCE);
    seed(&mut mind);
    mind
}

pub(crate) fn refusal(outcome: PipelineAdmissionOutcome) -> MindRefusal {
    match outcome {
        PipelineAdmissionOutcome::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

pub(crate) fn committed(outcome: PipelineAdmissionOutcome) -> (String, Vec<PipelineRef>) {
    match outcome {
        PipelineAdmissionOutcome::Committed { receipt_id, writes, .. } => (receipt_id, writes),
        other => panic!("expected a commit, got {other:?}"),
    }
}
