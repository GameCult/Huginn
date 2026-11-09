//! Test support: a real mind over a temporary directory behind `MindPort`, a
//! recording launcher and a settable clock. Every test document is built here,
//! so a field the leaf adds to a kind is one edit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use anyhow::{Result, anyhow};
use chrono::Duration as ChronoDuration;
use model_connector::PlanQuota;
use chrono::{DateTime, Duration, TimeZone, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{
    CommitRange, CutVerification, Date, DocRef, MemoryOrigin, MemorySort, PipelineCampaign, PipelineCutReport, PipelineCutSpec, PipelineDocument, PipelineFollowUp,
    PipelineInstance, PipelineKind, PipelineMemory, PipelineQuestion, PipelineRef, PipelineResolution, PipelineRuling, PipelineRun, PipelineStewardship,
    PipelineTarget, QuestionOption, ResolutionOutcome, RulingAuthority, RunOperator, RunTurn, Sha, Slug, StructuralDelta, TargetInvariant,
};
use huginn_mind::{
    Faculty, HuginnMindRequest, ImportSource, ImportSource, HuginnMindResponse, Mind, OwnedRedbMessagePackBackingStore, PipelineAdmissionBatch, PipelineAdmissionOutcome,
    PipelineDocumentView, PipelineProvenance, PipelineSelectionPage,
};
use tempfile::TempDir;

use crate::launch::{Clock, Launcher};
use crate::lease_client::{Lease, LeaseRefusalKind, LeaseRefused, LeaseRequest, Lender, Lent, TokenFd, token_holding};
use crate::queue::MindPort;
use crate::self_run::{Child, Kept, Read, Spawner, Stop};
use uuid::Uuid;

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

/// A launcher that records every start as (instance, turn), fails them on
/// command, and answers `alive` per unit, which is what instance and turn name:
/// `start` fills it, `kill` empties a turn's unit and `revive` fills it for a
/// seeded run.
#[derive(Default)]
pub(crate) struct Recording {
    pub(crate) started: RefCell<Vec<(Slug, RunTurn)>>,
    pub(crate) fail: Cell<bool>,
    /// One named instance whose unit never starts.
    pub(crate) fail_for: RefCell<Option<Slug>>,
    running: RefCell<Vec<(Slug, eureka_pipeline::RunSlot)>>,
}

impl Recording {
    pub(crate) fn kill(&self, turn: RunTurn) {
        self.running.borrow_mut().retain(|unit| *unit != (Slug(INSTANCE.into()), turn.slot()));
    }

    /// `kill` for a named instance.
    pub(crate) fn kill_as(&self, instance: &Slug, turn: RunTurn) {
        self.running.borrow_mut().retain(|unit| *unit != (instance.clone(), turn.slot()));
    }

    pub(crate) fn revive(&self, turn: RunTurn) {
        self.revive_as(&Slug(INSTANCE.into()), turn);
    }

    /// `revive` for a named instance.
    pub(crate) fn revive_as(&self, instance: &Slug, turn: RunTurn) {
        self.running.borrow_mut().push((instance.clone(), turn.slot()));
    }
}

impl Launcher for Recording {
    fn start(&self, instance: &Slug, turn: RunTurn) -> Result<()> {
        if self.fail.get() || self.fail_for.borrow().as_ref() == Some(instance) {
            return Err(anyhow!("refused"));
        }
        self.started.borrow_mut().push((instance.clone(), turn));
        self.running.borrow_mut().push((instance.clone(), turn.slot()));
        Ok(())
    }

    fn alive(&self, instance: &Slug, turn: RunTurn) -> bool {
        self.running.borrow().contains(&(instance.clone(), turn.slot()))
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
        Self::seeded_as(INSTANCE)
    }

    /// A mind holding only its identity and the repo's stewardship: no campaign.
    pub(crate) fn without_campaigns() -> Self {
        let dir = TempDir::new().unwrap();
        let mind = Mind::open(dir.path(), &Slug(INSTANCE.into())).unwrap();
        let this = Self { _dir: dir, instance: Slug(INSTANCE.into()), mind: RefCell::new(mind), now: Cell::new(at(0)) };
        this.put(vec![instance(INSTANCE), stewardship(INSTANCE)]);
        this
    }

    /// A mind holding only its identity: no stewardship, no campaign.
    pub(crate) fn bare() -> Self {
        let dir = TempDir::new().unwrap();
        let mind = Mind::open(dir.path(), &Slug(INSTANCE.into())).unwrap();
        let this = Self { _dir: dir, instance: Slug(INSTANCE.into()), mind: RefCell::new(mind), now: Cell::new(at(0)) };
        this.put(vec![instance(INSTANCE)]);
        this
    }

    /// The same, for another instance name.
    pub(crate) fn seeded_as(name: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let mind = Mind::open(dir.path(), &Slug(name.into())).unwrap();
        let this = Self { _dir: dir, instance: Slug(name.into()), mind: RefCell::new(mind), now: Cell::new(at(0)) };
        this.put(vec![instance(name), stewardship(name), campaign(), target()]);
        this
    }

    /// The next admission lands exactly at `when`.
    pub(crate) fn admit_at(&self, when: DateTime<Utc>) {
        self.now.set(when - Duration::seconds(1));
    }

    /// Admits as Hands, one second after the last admission.
    pub(crate) fn put(&self, documents: Vec<PipelineDocument>) -> PipelineAdmissionOutcome {
        self.put_as(Faculty::Hands, documents)
    }

    /// Admits as `faculty`, one second after the last admission.
    pub(crate) fn put_as(&self, faculty: Faculty, documents: Vec<PipelineDocument>) -> PipelineAdmissionOutcome {
        self.now.set(self.now.get() + Duration::seconds(1));
        let batch = PipelineAdmissionBatch {
            instance: self.instance.clone(),
            provenance: PipelineProvenance { faculty, agent: "test".into(), session: "test".into(), tool: "test".into(), imported_from: None },
            documents,
        };
        self.mind.borrow_mut().admit(batch, self.now.get())
    }

    /// Admits as an import of a giver's receipt committed at `committed_at`, one second after the last admission here.
    pub(crate) fn committed_imported(&self, committed_at: &str, documents: Vec<PipelineDocument>) {
        self.now.set(self.now.get() + Duration::seconds(1));
        let source = ImportSource {
            instance: Slug("giver".into()),
            receipt_id: format!("giver-{committed_at}").as_str().into(),
            ordinal: 1,
            committed_at: committed_at.into(),
            faculty: Faculty::Hands,
            agent: "test".into(),
            session: "test".into(),
            tool: "test".into(),
            external: vec![],
        };
        let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: "test".into(), session: "test".into(), tool: "test".into(), imported_from: Some(source) };
        let batch = PipelineAdmissionBatch { instance: self.instance.clone(), provenance, documents };
        let outcome = self.mind.borrow_mut().admit(batch, self.now.get());
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
    }

    /// Admits as an import of a giver's receipt committed at `committed_at`, one second after the last admission here.
    pub(crate) fn committed_imported(&self, committed_at: &str, documents: Vec<PipelineDocument>) {
        self.now.set(self.now.get() + Duration::seconds(1));
        let source = ImportSource {
            instance: Slug("giver".into()),
            receipt_id: format!("giver-{committed_at}").as_str().into(),
            ordinal: 1,
            committed_at: committed_at.into(),
            faculty: Faculty::Hands,
            agent: "test".into(),
            session: "test".into(),
            tool: "test".into(),
            external: vec![],
        };
        let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: "test".into(), session: "test".into(), tool: "test".into(), imported_from: Some(source) };
        let batch = PipelineAdmissionBatch { instance: self.instance.clone(), provenance, documents };
        let outcome = self.mind.borrow_mut().admit(batch, self.now.get());
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
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
            HuginnMindRequest::RootsTouched { .. } => match self.mind.borrow().roots_touched() {
                Ok(roots) => HuginnMindResponse::RootsTouched(roots),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
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

    /// A stand-in for the daemon's index, which this crate does not own: the selection's members
    /// ranked by the words they share with `text`, ties in the mind's order. The tests pin what is
    /// asked (`Seen`-style ports record it); what the index would rank is the daemon's.
    fn query_near(&self, selection: &Selection, text: &str, top_k: u32) -> Result<PipelineSelectionPage> {
        let mut page = self.query(selection)?;
        let words: std::collections::BTreeSet<String> = text.split_whitespace().map(str::to_lowercase).collect();
        if let huginn_mind::PipelinePageItems::Documents(views) = &mut page.items {
            let shared = |view: &PipelineDocumentView| {
                let own = serde_json::to_string(&view.document).unwrap_or_default().to_lowercase();
                words.iter().filter(|word| own.contains(word.as_str())).count()
            };
            views.sort_by_key(|view| std::cmp::Reverse(shared(view)));
            views.truncate(top_k as usize);
        }
        Ok(page)
    }

    /// A forge with nothing new in it: the daemon with a healthy forge and a quiet org.
    fn stewarded_activity(&self, _repos: &[huginn_mind::eureka_pipeline::OrgRepo], after: Option<u64>) -> Result<huginn_mind::RepoActivity> {
        Ok(huginn_mind::RepoActivity { head: after.unwrap_or(0), events: vec![], capped: vec![], open_prs: vec![], unseen: vec![], moved: vec![] })
    }

    /// The roots of this mind as it holds them: the daemon's answer from the same mind, never a stub.
    fn roots_touched(&self) -> Result<Vec<huginn_mind::RootTouched>> {
        self.mind.borrow().roots_touched().map_err(|_| anyhow!("refused"))
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        self.now.set(self.now.get() + Duration::seconds(1));
        let batch = PipelineAdmissionBatch {
            instance: self.instance.clone(),
            provenance: PipelineProvenance { faculty: Faculty::SelfFaculty, agent: agent.into(), session: session.into(), tool: "mind-body".into(), imported_from: None },
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

/// One of her memories: `sort` and `text`, about `about`, learned in #swarm, salience 500.
pub(crate) fn memory(label: &str, sort: MemorySort, text: &str, about: &[&str]) -> PipelineDocument {
    memory_from(label, sort, text, about, MemoryOrigin::Room { rooms: "swarm".into(), heard_from: vec![] }, 500)
}

/// One of her memories with the origin and salience the ranking tests need.
pub(crate) fn memory_from(label: &str, sort: MemorySort, text: &str, about: &[&str], origin: MemoryOrigin, salience_permille: u16) -> PipelineDocument {
    PipelineDocument::Memory(PipelineMemory {
        instance: Slug(INSTANCE.into()),
        label: eureka_pipeline::Label(label.into()),
        sort,
        text: text.into(),
        about: about.iter().map(|who| (*who).into()).collect(),
        origin,
        sources: vec![],
        salience_permille,
        consolidates: vec![],
    })
}

fn doc_ref() -> DocRef {
    DocRef { path: "docs/eureka-body-target.md".into(), start_line: 1, end_line: 9, commit: Sha("df7507b".into()) }
}

fn instance(name: &str) -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: Slug(name.into()),
        display_name: "test mind".into(),
        created_at: Date("2026-10-09".into()),
        host: "test".into(),
    })
}

fn stewardship(name: &str) -> PipelineDocument {
    PipelineDocument::Stewardship(PipelineStewardship {
        instance: Slug(name.into()),
        repo: REPO.into(),
        sequence: 1,
        assigned_on: Date("2026-10-09".into()),
        note: "assigned".into(),
        names: vec![],
    })
}

fn campaign() -> PipelineDocument {
    campaign_named(CAMPAIGN, "Eureka body")
}

pub(crate) fn campaign_named(slug: &str, title: &str) -> PipelineDocument {
    PipelineDocument::Campaign(PipelineCampaign {
        slug: Slug(slug.into()),
        title: title.into(),
        repos: vec![REPO.into()],
        working_branch: "main".into(),
        target_doc: doc_ref(),
    })
}

/// A campaign opened on `repos`, with its first target and the one invariant.
pub(crate) fn campaign_on(slug: &str, repos: &[&str]) -> Vec<PipelineDocument> {
    let PipelineDocument::Campaign(mut opened) = campaign_named(slug, slug) else { unreachable!("a campaign") };
    opened.repos = repos.iter().map(|repo| (*repo).into()).collect();
    let PipelineDocument::Target(mut first) = target() else { unreachable!("a target") };
    first.campaign = Slug(slug.into());
    vec![PipelineDocument::Campaign(opened), PipelineDocument::Target(first)]
}

fn target() -> PipelineDocument {
    PipelineDocument::Target(PipelineTarget {
        campaign: Slug(CAMPAIGN.into()),
        revision: 1,
        invariants: vec![TargetInvariant { label: "run-is-the-grant".into(), statement: "A run is the grant.".into() }],
        not_in_scope: vec![],
        canonical_implementations: vec![],
        repos: vec![],
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
        reads: vec![],
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
        options: ["A", "B", "reopen", "retire"].iter().map(|name| QuestionOption { label: (*name).into(), text: "an option".into() }).collect(),
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
    run_in(INSTANCE, label, turn, operated_by, claims)
}

pub(crate) fn run_in(instance: &str, label: &str, turn: RunTurn, operated_by: RunOperator, claims: &[PipelineRef]) -> PipelineDocument {
    PipelineDocument::Run(PipelineRun {
        instance: Slug(instance.into()),
        label: label.into(),
        operated_by,
        turn,
        host: "test".into(),
        started_on: Date("2026-10-09".into()),
        budget_usd: "0".into(),
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
        origin: None,
    })
}

// The scripted child, the fake spawner and the fake lease: the stream and the lease the Self and the Persona organs are tested against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Wrote(String),
    InputClosed,
    /// A signal was attempted, delivered or not.
    Stopped(Stop),
    Asked(StdDuration),
    /// The connector was asked for a lease and answered one.
    Lent,
    /// A child was started.
    Spawned,
    /// The spawner was told to keep this conversation until the instant, or to clear it.
    Kept(Option<(Uuid, DateTime<Utc>)>),
    /// The lease was dropped, which ends it: nothing is settled.
    Abandoned,
}

#[derive(Clone)]
pub(crate) enum Step {
    Line(String),
    /// Time passes with no output: the clock moves and the read times out.
    Idle(i64),
    /// The clock moves, then this line arrives.
    After(i64, String),
    /// The child is reaped with this code; its stream stays open.
    Exit(Option<i32>),
    /// Its stdout ends; the child is not reaped yet.
    Eof,
    /// The clock moves, this runs (the mind changes, a tool call's hook fires), and the read times out.
    Do(i64, Rc<dyn Fn()>),
}

pub(crate) struct Scripted {
    pub(crate) steps: VecDeque<Step>,
    pub(crate) log: Rc<RefCell<Vec<Event>>>,
    pub(crate) clock: Rc<Fixed>,
    pub(crate) code: Option<i32>,
    /// The weakest signal that ends it; weaker ones are ignored.
    pub(crate) dies_on: Option<Stop>,
    /// Signals that cannot be delivered (no `kill` program, say).
    pub(crate) refuses: Vec<Stop>,
    /// When the steps run out it neither exits nor closes its stream: time just passes.
    pub(crate) hangs: bool,
    pub(crate) exited: bool,
    pub(crate) eof: bool,
    /// How many times it was asked: a loop that never ends is a failure, not a hang.
    pub(crate) asked: u32,
}

impl Child for Scripted {
    fn write_line(&mut self, line: &str) {
        self.log.borrow_mut().push(Event::Wrote(line.to_string()));
    }

    fn close_input(&mut self) {
        self.log.borrow_mut().push(Event::InputClosed);
    }

    fn next(&mut self, timeout: StdDuration) -> Read {
        self.log.borrow_mut().push(Event::Asked(timeout));
        self.asked += 1;
        assert!(self.asked < 500, "the loop never ended");
        match self.steps.pop_front() {
            Some(Step::Line(line)) => Read::Line(line),
            Some(Step::Idle(seconds)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                Read::Timeout
            }
            Some(Step::After(seconds, line)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                Read::Line(line)
            }
            Some(Step::Exit(code)) => {
                self.exited = true;
                Read::Exited(code)
            }
            Some(Step::Do(seconds, act)) => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(seconds));
                act();
                Read::Timeout
            }
            Some(Step::Eof) => {
                self.eof = true;
                Read::Eof
            }
            // The stream is over: the child is reaped, then its stdout ends.
            None if self.hangs => {
                self.clock.0.set(self.clock.0.get() + ChronoDuration::seconds(timeout.as_secs() as i64));
                Read::Timeout
            }
            None if !self.exited => {
                self.exited = true;
                Read::Exited(self.code)
            }
            None if !self.eof => {
                self.eof = true;
                Read::Eof
            }
            None => panic!("the loop kept waiting on a child that ended"),
        }
    }

    fn stop(&mut self, how: Stop) -> bool {
        self.log.borrow_mut().push(Event::Stopped(how));
        if self.refuses.contains(&how) {
            return false;
        }
        if self.dies_on.is_some_and(|weakest| how >= weakest) {
            self.steps.clear();
            self.hangs = false;
        }
        true
    }
}

pub(crate) struct Fake {
    pub(crate) child: RefCell<Option<Scripted>>,
    pub(crate) spawns: RefCell<Vec<Vec<String>>>,
    pub(crate) refuse: bool,
    /// The conversation the spawner reports as kept; a test sets it.
    pub(crate) kept: RefCell<Option<Kept>>,
    /// Where `keep` logs, in order with the child's own events: the first child's log.
    log: Option<Rc<RefCell<Vec<Event>>>>,
}

impl Fake {
    pub(crate) fn with(child: Option<Scripted>, refuse: bool) -> Self {
        let log = child.as_ref().map(|child| child.log.clone());
        Self { child: RefCell::new(child), spawns: RefCell::new(Vec::new()), refuse, kept: RefCell::new(None), log }
    }
}

impl Spawner for Fake {
    fn kept(&self) -> Option<Kept> {
        self.kept.borrow().clone()
    }

    fn keep(&self, keep: Option<(Uuid, DateTime<Utc>)>) -> Result<()> {
        if let Some(log) = &self.log {
            log.borrow_mut().push(Event::Kept(keep));
        }
        Ok(())
    }

    fn spawn(&self, args: &[String], _token: &TokenFd) -> Result<Box<dyn Child>> {
        self.spawns.borrow_mut().push(args.to_vec());
        if self.refuse {
            return Err(anyhow!("refused"));
        }
        let child = self.child.borrow_mut().take().expect("spawned twice");
        child.log.borrow_mut().push(Event::Spawned);
        Ok(Box::new(child))
    }
}

/// What the connector answers.
#[derive(Clone, Copy)]
pub(crate) enum Answer {
    Grant,
    Refuse(LeaseRefusalKind),
    Unreachable,
}

/// The connector, as the actuator asks it: it logs into the child's log, so the
/// order of lend, spawn, the child's end and settle is one sequence.
pub(crate) struct FakeLender {
    pub(crate) log: Rc<RefCell<Vec<Event>>>,
    pub(crate) asked: RefCell<Vec<LeaseRequest>>,
    pub(crate) answer: Answer,
}

impl FakeLender {
    pub(crate) fn new(answer: Answer) -> Self {
        Self::logging(Rc::new(RefCell::new(Vec::new())), answer)
    }

    pub(crate) fn logging(log: Rc<RefCell<Vec<Event>>>, answer: Answer) -> Self {
        Self { log, asked: RefCell::new(Vec::new()), answer }
    }
}

impl Lender for FakeLender {
    fn lend(&self, request: &LeaseRequest) -> Result<Lent> {
        self.asked.borrow_mut().push(request.clone());
        match self.answer {
            Answer::Grant => {
                self.log.borrow_mut().push(Event::Lent);
                Ok(Lent::Granted(Box::new(FakeLease { log: self.log.clone(), token: token_holding("tok-canary"), plan: vec![plan_window()] })))
            }
            Answer::Refuse(kind) => Ok(Lent::Refused(LeaseRefused { kind, detail: "detail-canary".into() })),
            Answer::Unreachable => Err(anyhow!("down")),
        }
    }
}

pub(crate) fn plan_window() -> PlanQuota {
    PlanQuota { window: "five_hour".into(), utilization_permille: Some(423), resets_at_unix_ms: 1_793_307_000_000, status: "allowed".into(), observed_at_unix_ms: 1_793_306_000_000 }
}

pub(crate) struct FakeLease {
    pub(crate) log: Rc<RefCell<Vec<Event>>>,
    pub(crate) token: TokenFd,
    pub(crate) plan: Vec<PlanQuota>,
}

impl Lease for FakeLease {
    fn token(&self) -> &TokenFd {
        &self.token
    }

    fn plan_quota(&self) -> &[PlanQuota] {
        &self.plan
    }
}

impl Drop for FakeLease {
    fn drop(&mut self) {
        self.log.borrow_mut().push(Event::Abandoned);
    }
}

