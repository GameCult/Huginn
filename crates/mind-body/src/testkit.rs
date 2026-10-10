//! Test support: a real mind over a temporary directory behind `MindPort`, a
//! recording launcher and a settable clock. Every test document is built here,
//! so a field the leaf adds to a kind is one edit.

use std::cell::{Cell, RefCell};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, TimeZone, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{
    CommitRange, CutVerification, Date, DocRef, Label, PipelineCampaign, PipelineCutReport, PipelineCutSpec, PipelineDocument, PipelineFollowUp,
    PipelineInstance, PipelineKind, PipelineQuestion, PipelineRef, PipelineResolution, PipelineRuling, PipelineRun, PipelineStewardship,
    PipelineTarget, QuestionOption, ResolutionOutcome, RulingAuthority, RunOperator, RunTurn, Sha, Slug, StructuralDelta, TargetInvariant,
};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, Mind, OwnedRedbMessagePackBackingStore, PipelineAdmissionBatch, PipelineAdmissionOutcome,
    PipelineDocumentView, PipelineProvenance, PipelineSelectionPage,
};
use tempfile::TempDir;

use crate::launch::{Clock, Launcher};
use crate::queue::MindPort;

pub(crate) const INSTANCE: &str = "yggdrasil";
pub(crate) const CAMPAIGN: &str = "eureka-body";
pub(crate) const REPO: &str = "GameCult/Huginn";

pub(crate) fn at(milliseconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 9, 13, 15, 0).unwrap() + Duration::milliseconds(milliseconds)
}

pub(crate) struct Fixed(pub(crate) Cell<DateTime<Utc>>);

impl Clock for Fixed {
    fn now(&self) -> DateTime<Utc> {
        self.0.get()
    }
}

/// A launcher that records every start as (instance, turn, label), fails them
/// on command, and answers `alive` from a set of labels: `start` fills it,
/// `kill` empties it for a label and `revive` fills it for a seeded run.
#[derive(Default)]
pub(crate) struct Recording {
    pub(crate) started: RefCell<Vec<(Slug, RunTurn, Label)>>,
    pub(crate) fail: Cell<bool>,
    running: RefCell<Vec<Label>>,
}

impl Recording {
    pub(crate) fn kill(&self, run: &str) {
        self.running.borrow_mut().retain(|label| label.0 != run);
    }

    pub(crate) fn revive(&self, run: &str) {
        self.running.borrow_mut().push(label(run));
    }
}

impl Launcher for Recording {
    fn start(&self, instance: &Slug, turn: RunTurn, label: &Label) -> Result<()> {
        if self.fail.get() {
            return Err(anyhow!("refused"));
        }
        self.started.borrow_mut().push((instance.clone(), turn, label.clone()));
        self.running.borrow_mut().push(label.clone());
        Ok(())
    }

    fn alive(&self, _instance: &Slug, _turn: RunTurn, label: &Label) -> bool {
        self.running.borrow().contains(label)
    }
}

pub(crate) struct TestMind {
    _dir: TempDir,
    instance: Slug,
    mind: RefCell<Mind<OwnedRedbMessagePackBackingStore>>,
    now: Cell<DateTime<Utc>>,
}

impl TestMind {
    /// A mind holding its identity, the repo's stewardship, the campaign and a
    /// first target.
    pub(crate) fn seeded() -> Self {
        let dir = TempDir::new().unwrap();
        let mind = Mind::open(dir.path(), &Slug(INSTANCE.into())).unwrap();
        let this = Self { _dir: dir, instance: Slug(INSTANCE.into()), mind: RefCell::new(mind), now: Cell::new(at(0)) };
        this.put(vec![instance(), stewardship(), campaign(), target()]);
        this
    }

    /// Admits as Hands, one second after the last admission.
    pub(crate) fn put(&self, documents: Vec<PipelineDocument>) -> PipelineAdmissionOutcome {
        self.now.set(self.now.get() + Duration::seconds(1));
        let batch = PipelineAdmissionBatch {
            instance: Slug(INSTANCE.into()),
            provenance: PipelineProvenance { faculty: Faculty::Hands, agent: "test".into(), session: "test".into(), tool: "test".into() },
            documents,
        };
        self.mind.borrow_mut().admit(batch, self.now.get())
    }

    pub(crate) fn committed(&self, documents: Vec<PipelineDocument>) {
        let outcome = self.put(documents);
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }

    /// What a daemon over this mind answers a request with.
    pub(crate) fn answer(&self, request: HuginnMindRequest) -> HuginnMindResponse {
        match request {
            HuginnMindRequest::Query { selection, .. } => match self.mind.borrow().query(&selection) {
                Ok(page) => HuginnMindResponse::Query(page),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
            HuginnMindRequest::Admit(batch) => HuginnMindResponse::Admit(self.mind.borrow_mut().admit(batch, self.now.get())),
            other => panic!("the tests ask nothing else: {other:?}"),
        }
    }

    pub(crate) fn view(&self, kind: PipelineKind, id: &str) -> Option<PipelineDocumentView> {
        self.mind.borrow().view(&PipelineRef { kind, id: id.into() }).unwrap()
    }

    /// Every document of a kind, as ids.
    pub(crate) fn ids(&self, kind: PipelineKind) -> Vec<String> {
        let selection = Selection { schemas: Some(vec![kind.type_id().to_string()]), ..Selection::default() };
        let page = self.query(&selection).unwrap();
        match page.items {
            huginn_mind::PipelinePageItems::Headers(headers) => headers.into_iter().map(|header| header.id.id.0).collect(),
            huginn_mind::PipelinePageItems::Documents(views) => views.into_iter().map(|view| view.id.id.0).collect(),
        }
    }
}

impl MindPort for TestMind {
    fn instance(&self) -> &Slug {
        &self.instance
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.mind.borrow().query(selection).map_err(|_| anyhow!("refused"))
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.now.set(self.now.get() + Duration::seconds(1));
        let batch = PipelineAdmissionBatch {
            instance: Slug(INSTANCE.into()),
            provenance: PipelineProvenance { faculty: Faculty::SelfFaculty, agent: agent.into(), session: session.into(), tool: "mind-body".into() },
            documents,
        };
        Ok(self.mind.borrow_mut().admit(batch, self.now.get()))
    }
}

/// A mind whose admissions answer as told, over a real mind's reads.
pub(crate) struct Answering<'a> {
    pub(crate) inner: &'a TestMind,
    pub(crate) outcome: PipelineAdmissionOutcome,
}

impl MindPort for Answering<'_> {
    fn instance(&self) -> &Slug {
        self.inner.instance()
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.inner.query(selection)
    }

    fn admit(&self, _agent: &str, _session: &str, _documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        Ok(self.outcome.clone())
    }
}

fn doc_ref() -> DocRef {
    DocRef { path: "docs/eureka-body-target.md".into(), start_line: 1, end_line: 9, commit: Sha("df7507b".into()) }
}

fn instance() -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: Slug(INSTANCE.into()),
        display_name: "test mind".into(),
        created_at: Date("2026-10-09".into()),
        host: "test".into(),
    })
}

fn stewardship() -> PipelineDocument {
    PipelineDocument::Stewardship(PipelineStewardship {
        instance: Slug(INSTANCE.into()),
        repo: REPO.into(),
        sequence: 1,
        assigned_on: Date("2026-10-09".into()),
        note: "assigned".into(),
    })
}

fn campaign() -> PipelineDocument {
    PipelineDocument::Campaign(PipelineCampaign {
        slug: Slug(CAMPAIGN.into()),
        title: "Eureka body".into(),
        repos: vec![REPO.into()],
        working_branch: "main".into(),
        target_doc: doc_ref(),
    })
}

fn target() -> PipelineDocument {
    PipelineDocument::Target(PipelineTarget {
        campaign: Slug(CAMPAIGN.into()),
        revision: 1,
        invariants: vec![TargetInvariant { label: "run-is-the-grant".into(), statement: "A run is the grant.".into() }],
        not_in_scope: vec![],
        canonical_implementations: vec![],
        doc: doc_ref(),
    })
}

pub(crate) fn id(kind: &str, local: &str) -> String {
    format!("{CAMPAIGN}:{kind}:{local}")
}

pub(crate) fn reference(kind: PipelineKind, id: &str) -> PipelineRef {
    PipelineRef { kind, id: id.into() }
}

pub(crate) fn spec_ref(cut: &str) -> PipelineRef {
    reference(PipelineKind::CutSpec, &id("cut_spec", &format!("cut-{cut}.r1")))
}

pub(crate) fn spec(cut: &str) -> PipelineDocument {
    PipelineDocument::CutSpec(PipelineCutSpec {
        campaign: Slug(CAMPAIGN.into()),
        cut: cut.into(),
        revision: 1,
        title: "A cut".into(),
        repo: REPO.into(),
        branch: "eureka-body/x".into(),
        base: Sha("df7507b".into()),
        depends_on: vec![],
        first: vec![],
        deletes: vec![],
        keeps_moves: vec![],
        adds: vec![],
        file_changes: vec![],
        authority_map: None,
        verification: CutVerification { builds: vec![], tests: vec![], negative: vec![], operator: vec![] },
        estimate: delta(),
        rulings: vec![],
        questions: vec![],
    })
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

pub(crate) fn report(cut: &str) -> PipelineDocument {
    PipelineDocument::CutReport(PipelineCutReport {
        campaign: Slug(CAMPAIGN.into()),
        cut_spec: id("cut_spec", &format!("cut-{cut}.r1")).as_str().into(),
        attempt: 1,
        repo: REPO.into(),
        branch: "eureka-body/x".into(),
        commits: vec![eureka_pipeline::ReportCommit { sha: Sha("df7507b".into()), subject: "Land the cut".into(), builds: true }],
        range: CommitRange { base: Sha("df7507b".into()), head: Sha("df7507b".into()) },
        verification: vec![],
        mutations: vec![],
        deviations: vec![],
        forks: vec![],
        structural_delta: delta(),
        landed_names: vec![],
        undone: vec![],
        promises: vec![],
    })
}

/// A question raised in `item`, which blocks it.
pub(crate) fn question_in(label: &str, item: &PipelineRef) -> PipelineDocument {
    PipelineDocument::Question(PipelineQuestion {
        campaign: Slug(CAMPAIGN.into()),
        label: label.into(),
        title: "A fork".into(),
        question: "Which?".into(),
        options: ["A", "B"].iter().map(|name| QuestionOption { label: (*name).into(), text: "an option".into() }).collect(),
        recommended: "A".into(),
        depends: vec![],
        raised_in: Some(item.clone()),
        asked_on: Date("2026-10-09".into()),
    })
}

/// A standing ruling: the plain "something else was admitted" document. With
/// `answers` (a question's id and one of its option labels), the ruling that
/// answers it.
pub(crate) fn ruling(label: &str, answers: Option<(&str, &str)>) -> PipelineDocument {
    PipelineDocument::Ruling(PipelineRuling {
        campaign: Slug(CAMPAIGN.into()),
        label: label.into(),
        title: "A ruling".into(),
        answers: answers.map(|(question, _)| question.into()),
        choice: answers.map(|(_, choice)| choice.into()),
        ruling: "So.".into(),
        operator_quote: None,
        ruled_on: Date("2026-10-09".into()),
        precedents: vec![],
        authority: RulingAuthority::Standing,
    })
}

pub(crate) fn run_ref(label: &str) -> PipelineRef {
    reference(PipelineKind::Run, &format!("{INSTANCE}:run:{label}"))
}

pub(crate) fn run(label: &str, turn: RunTurn, operated_by: RunOperator, claims: &[PipelineRef]) -> PipelineDocument {
    PipelineDocument::Run(PipelineRun {
        instance: Slug(INSTANCE.into()),
        label: label.into(),
        operated_by,
        turn,
        host: "test".into(),
        started_on: Date("2026-10-09".into()),
        budget_usd: "5".into(),
        claims: claims.to_vec(),
        campaigns: vec![Slug(CAMPAIGN.into())],
    })
}

pub(crate) fn close(subject: PipelineRef, outcome: ResolutionOutcome) -> PipelineDocument {
    PipelineDocument::Resolution(PipelineResolution {
        subject,
        sequence: 1,
        outcome,
        rationale: "Closed.".into(),
        resolved_on: Date("2026-10-09".into()),
    })
}

pub(crate) fn recorded() -> ResolutionOutcome {
    ResolutionOutcome::Recorded { reason: "finished".into() }
}

pub(crate) fn withdrawn() -> ResolutionOutcome {
    ResolutionOutcome::Withdrawn { reason: "never ran".into() }
}

/// Her Self run on `item`, ended, as `n` in a history: opened, closed, and
/// followed by a follow-up sourced from `item` when `leaves_something` (the
/// document that cites the item, which makes the run non-empty).
pub(crate) fn ended_run(mind: &TestMind, item: &PipelineRef, prefix: &str, n: usize, leaves_something: bool) {
    let label = format!("{prefix}{n}");
    mind.committed(vec![run(&label, RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(item))]);
    let mut closing = vec![close(run_ref(&label), recorded())];
    if leaves_something {
        closing.push(follow_up(&format!("{prefix}left{n}"), item));
    }
    mind.committed(closing);
}

/// A history of ended runs on `item`, oldest first; `true` leaves a document.
pub(crate) fn history(mind: &TestMind, item: &PipelineRef, runs: &[bool]) {
    history_as(mind, item, "h", runs);
}

/// The same, with run labels `<prefix>0`, `<prefix>1`, and so on, for a mind
/// holding the histories of several items.
pub(crate) fn history_as(mind: &TestMind, item: &PipelineRef, prefix: &str, runs: &[bool]) {
    for (n, leaves_something) in runs.iter().enumerate() {
        ended_run(mind, item, prefix, n, *leaves_something);
    }
}

pub(crate) fn label(value: &str) -> Label {
    value.into()
}

/// A follow-up sourced from `source`: work a run may claim that the queue does
/// not list.
pub(crate) fn follow_up(label: &str, source: &PipelineRef) -> PipelineDocument {
    PipelineDocument::FollowUp(PipelineFollowUp {
        campaign: Slug(CAMPAIGN.into()),
        label: label.into(),
        source: source.clone(),
        repo: REPO.into(),
        locations: vec![],
        item: "Later.".into(),
        why_it_can_wait: "It can.".into(),
        owner: "Hands".into(),
    })
}
