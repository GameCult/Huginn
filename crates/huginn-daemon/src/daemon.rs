//! The dispatch: one mind, one request, one response. No socket, no clock, no
//! rule. `now` is the caller's, so a test pins it and the loop is the only
//! thing that reads a wall clock.

use std::path::Path;

use chrono::{DateTime, Utc};
use huginn_mind::epiphany_pipeline::{PipelineRef, Slug};
use huginn_mind::wire::{HuginnMindRequest, HuginnMindResponse};
use huginn_mind::{Mind, MindRefusal, MindStore, OwnedRedbMessagePackBackingStore, PipelineAdmissionOutcome};

/// Where Cut 11's index attaches. Called after a batch commits, with the refs
/// the batch landed, derived writes included; the sink may read each through
/// `mind.view`. Its error is logged and never reaches the caller: the index
/// does not decide whether a batch was admitted.
pub trait IndexSink<S: MindStore> {
    fn committed(&mut self, mind: &Mind<S>, writes: &[PipelineRef]) -> anyhow::Result<()>;
}

/// This cut's index: there is none.
pub struct NoIndex;

impl<S: MindStore> IndexSink<S> for NoIndex {
    fn committed(&mut self, _mind: &Mind<S>, _writes: &[PipelineRef]) -> anyhow::Result<()> {
        Ok(())
    }
}

/// One mind and whatever indexes what lands in it.
pub struct Daemon<S: MindStore, I: IndexSink<S>> {
    mind: Mind<S>,
    index: I,
}

impl Daemon<OwnedRedbMessagePackBackingStore, NoIndex> {
    /// Opens the instance's mind, taking the store's exclusive lock for the
    /// daemon's lifetime. Ruling 15: a mind that will not open is a refusal
    /// here, before any socket exists; this function takes no address and
    /// cannot bind one.
    pub fn open(state_root: &Path, instance: &Slug) -> Result<Self, MindRefusal> {
        Ok(Self { mind: Mind::open(state_root, instance)?, index: NoIndex })
    }
}

impl<S: MindStore, I: IndexSink<S>> Daemon<S, I> {
    pub fn with_index<J: IndexSink<S>>(self, index: J) -> Daemon<S, J> {
        Daemon { mind: self.mind, index }
    }

    pub fn mind(&self) -> &Mind<S> {
        &self.mind
    }

    /// The envelope's `source_runtime_id` on every response this daemon sends.
    pub fn runtime_id(&self) -> String {
        format!("huginn-{}", self.mind.instance().0)
    }

    /// One request, one answer. `Admit` goes to the mind whole, because the
    /// batch declares its own instance and admission's A1 is the check. Every
    /// other request names an instance: the read path asks the leaf whether
    /// that name is even grammatical before asking the mind whether it is its
    /// own, so a name outside the grammar (a fullwidth fold, say) is refused
    /// by name rather than read as merely a foreign mind.
    pub fn handle(&mut self, request: HuginnMindRequest, now: DateTime<Utc>) -> HuginnMindResponse {
        if let Some(declared) = request.instance()
            && !matches!(request, HuginnMindRequest::Admit(_))
        {
            if let Err(refusal) = huginn_mind::require_grammatical_instance(declared) {
                return HuginnMindResponse::Refused(refusal);
            }
            if let Err(refusal) = self.mind.require_instance(declared) {
                return HuginnMindResponse::Refused(refusal);
            }
        }
        match request {
            HuginnMindRequest::Whoami => HuginnMindResponse::Whoami(self.mind.status()),
            HuginnMindRequest::Admit(batch) => {
                let outcome = self.mind.admit(batch, now);
                if let PipelineAdmissionOutcome::Committed { writes, .. } = &outcome
                    && let Err(error) = self.index.committed(&self.mind, writes)
                {
                    let refs = writes.iter().map(|write| write.id.0.clone()).collect::<Vec<_>>().join(", ");
                    eprintln!("huginn: the index refused the landed writes [{refs}]: {error:#}");
                }
                HuginnMindResponse::Admit(outcome)
            }
            HuginnMindRequest::View { id, .. } => match self.mind.view(&id) {
                Ok(view) => HuginnMindResponse::View(view),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
            HuginnMindRequest::Query { query, .. } => match self.mind.query(&query) {
                Ok(page) => HuginnMindResponse::Query(page),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
            HuginnMindRequest::OpenItems { campaign, .. } => match self.mind.open_items(&campaign) {
                Ok(items) => HuginnMindResponse::OpenItems(items),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
            HuginnMindRequest::History { scope, .. } => match self.mind.history(&scope) {
                Ok(views) => HuginnMindResponse::History(views),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chrono::TimeZone;
    use huginn_mind::epiphany_pipeline::{
        AuthorityMap, CodeLocation, CutDelete, CutVerification, Date, DocRef, FileChange, Line, NegativeCheck,
        OrgRepo, PipelineCampaign, PipelineCutSpec, PipelineDocument, PipelineInstance, PipelineKind,
        PipelineQuestion, PipelineRuling, PipelineStewardship, QuestionOption, RulingAuthority, Sha, Short,
        StructuralDelta, VerificationTest,
    };
    use huginn_mind::wire::MindStatus;
    use huginn_mind::{
        Faculty, HistoryScope, PipelineAdmissionBatch, PipelineProvenance, PipelineQuery, PipelineStatus,
        SemanticQuery,
    };
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    pub(crate) const INSTANCE: &str = "yggdrasil";
    pub(crate) const OTHER: &str = "thought-cage";
    /// `INSTANCE`'s own length, differing in its last byte alone.
    pub(crate) const NEAR: &str = "yggdrasix";
    /// `INSTANCE` whole, with more after it.
    pub(crate) const PREFIXED: &str = "yggdrasil-two";
    /// `INSTANCE` in another case, which a slug label permits.
    pub(crate) const CASED: &str = "Yggdrasil";
    pub(crate) const CAMPAIGN: &str = "eureka-state";

    pub(crate) fn slug(value: &str) -> Slug {
        value.into()
    }

    pub(crate) fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 16, 12, 0, 0).unwrap()
    }

    pub(crate) fn provenance() -> PipelineProvenance {
        PipelineProvenance {
            faculty: Faculty::Hands,
            agent: Short("claude".into()),
            session: Short("session-1".into()),
            tool: Short("admit".into()),
        }
    }

    pub(crate) fn identity(name: &str) -> PipelineDocument {
        PipelineDocument::Instance(PipelineInstance {
            instance: slug(name),
            display_name: Short(format!("{name} mind")),
            created_at: Date("2026-09-16".into()),
            host: Short(name.into()),
        })
    }

    pub(crate) fn batch(instance: &str, documents: Vec<PipelineDocument>) -> PipelineAdmissionBatch {
        PipelineAdmissionBatch { instance: slug(instance), provenance: provenance(), documents }
    }

    /// A question and the ruling that answers it: admitting the ruling derives
    /// the question's resolution, so one batch lands two writes.
    pub(crate) fn question_and_ruling() -> (PipelineDocument, PipelineDocument) {
        let question = PipelineDocument::Question(PipelineQuestion {
            campaign: slug(CAMPAIGN),
            label: "Q1".into(),
            question: "Who owns the state?".into(),
            options: vec![
                QuestionOption { label: "A".into(), text: "the organ".into() },
                QuestionOption { label: "B".into(), text: "the client".into() },
            ],
            recommended: "A".into(),
            depends: vec![],
            raised_in: None,
            asked_on: Date("2026-09-16".into()),
        });
        let ruling = PipelineDocument::Ruling(PipelineRuling {
            campaign: slug(CAMPAIGN),
            label: "R1".into(),
            answers: Some(Short(format!("{CAMPAIGN}:question:Q1"))),
            choice: Some("A".into()),
            ruling: "An instance owns its mind.".into(),
            operator_quote: None,
            ruled_on: Date("2026-09-16".into()),
            precedents: vec![],
            authority: RulingAuthority::Operator,
        });
        (question, ruling)
    }

    pub(crate) const REPO: &str = "GameCult/Epiphany";

    /// A `Line` and a `Short` at the leaf's own bound, so a document built out
    /// of them is as wide as any writer may make it.
    pub(crate) fn line(n: usize) -> Line {
        Line(format!("{n:0>1000}"))
    }

    pub(crate) fn short(n: usize) -> Short {
        Short(format!("{n:0>200}"))
    }

    fn sha() -> Sha {
        Sha("5f98228d".into())
    }

    fn doc_ref() -> DocRef {
        DocRef { path: Short("notes/target.md".into()), start_line: 1, end_line: 9, commit: sha() }
    }

    /// The three documents a cut spec needs before it may be admitted: the
    /// mind's identity, its stewardship of the repo, and the campaign that
    /// names the repo.
    pub(crate) fn campaign_seed() -> Vec<PipelineDocument> {
        vec![
            identity(INSTANCE),
            PipelineDocument::Stewardship(PipelineStewardship {
                instance: slug(INSTANCE),
                repo: OrgRepo(REPO.into()),
                sequence: 1,
                assigned_on: Date("2026-09-16".into()),
                note: "assigned".into(),
            }),
            PipelineDocument::Campaign(PipelineCampaign {
                slug: slug(CAMPAIGN),
                title: Short("Eureka pipeline state".into()),
                repos: vec![OrgRepo(REPO.into())],
                working_branch: Short("codex/eureka-pipeline-state".into()),
                target_doc: doc_ref(),
            }),
        ]
    }

    /// One cut spec with every list the leaf bounds filled to its maximum and
    /// every text at its own: `file_changes` file changes, 64 deletes, keeps,
    /// adds, builds, tests, negative checks and operator checks, a full
    /// authority map and a full structural delta. At 256 file changes that is
    /// roughly a megabyte of field content, which is what a real-sized answer
    /// looks like at the leaf's bounds rather than at a fixture's convenience.
    /// `depends_on`, `rulings` and `questions` stay empty because each is a
    /// reference admission resolves.
    pub(crate) fn cut_spec(cut: &str, file_changes: usize) -> PipelineDocument {
        let lines = |count: usize| (0..count).map(line).collect::<Vec<_>>();
        let shorts = |count: usize| (0..count).map(short).collect::<Vec<_>>();
        let location = |n: usize| CodeLocation { path: short(n), line: 1, end_line: Some(9) };
        PipelineDocument::CutSpec(PipelineCutSpec {
            campaign: slug(CAMPAIGN),
            cut: cut.into(),
            revision: 1,
            title: short(0),
            repo: OrgRepo(REPO.into()),
            branch: short(1),
            base: sha(),
            depends_on: vec![],
            first: lines(16),
            deletes: (0..64).map(|n| CutDelete { path: short(n), lines: 9, note: line(n) }).collect(),
            keeps_moves: lines(64),
            adds: lines(64),
            file_changes: (0..file_changes).map(|n| FileChange { location: location(n), change: line(n) }).collect(),
            authority_map: Some(AuthorityMap {
                owner: line(0),
                inputs: lines(16),
                outputs: lines(16),
                derived_state: lines(16),
                forbidden_writers: lines(16),
                shared_paths: lines(16),
                deletion_line: line(1),
            }),
            verification: CutVerification {
                builds: lines(64),
                tests: (0..64).map(|n| VerificationTest { name: short(n), pins: line(n) }).collect(),
                negative: (0..64).map(|n| NegativeCheck { pattern: short(n), scope: line(n) }).collect(),
                operator: lines(64),
            },
            estimate: StructuralDelta {
                lines_added: 0,
                lines_removed: 0,
                dependencies_added: shorts(64),
                dependencies_removed: shorts(64),
                formats_added: shorts(64),
                formats_removed: shorts(64),
                targets_added: shorts(64),
                targets_removed: shorts(64),
            },
            rulings: vec![],
            questions: vec![],
        })
    }

    /// The cut spec at the leaf's own maximum, and one narrow enough that its
    /// answer still fits a single send. `WIDE_CHANGES` is the leaf's bound;
    /// `FITTING_CHANGES` is chosen so the view of that spec lands inside the
    /// window with less than a packet-count's slack, which is what makes the
    /// window's own value load-bearing rather than merely generous.
    pub(crate) const WIDE_CUT: &str = "wide";
    pub(crate) const WIDE_CHANGES: usize = 256;
    pub(crate) const FITTING_CUT: &str = "fits";
    pub(crate) const FITTING_CHANGES: usize = 180;

    /// A daemon whose mind holds the campaign seed, one cut spec as wide as the
    /// leaf permits, and one narrow enough to answer, so a read over it is a
    /// real-sized answer either way.
    pub(crate) fn seeded_wide() -> (TempDir, Daemon<OwnedRedbMessagePackBackingStore, NoIndex>) {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        for documents in [
            campaign_seed(),
            vec![cut_spec(WIDE_CUT, WIDE_CHANGES), cut_spec(FITTING_CUT, FITTING_CHANGES)],
        ] {
            let outcome = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), now());
            assert!(
                matches!(outcome, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })),
                "{outcome:?}"
            );
        }
        (root, daemon)
    }

    /// A daemon over a real redb mind in a temporary directory, with its
    /// identity admitted. The directory is returned because dropping it
    /// removes the store.
    pub(crate) fn seeded() -> (TempDir, Daemon<OwnedRedbMessagePackBackingStore, NoIndex>) {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let outcome = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
        assert!(
            matches!(outcome, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })),
            "{outcome:?}"
        );
        (root, daemon)
    }

    #[derive(Clone, Default)]
    struct RecordingIndex {
        seen: Arc<Mutex<Vec<(Vec<String>, Vec<bool>)>>>,
    }

    impl<S: MindStore> IndexSink<S> for RecordingIndex {
        fn committed(&mut self, mind: &Mind<S>, writes: &[PipelineRef]) -> anyhow::Result<()> {
            let ids = writes.iter().map(|write| write.id.0.clone()).collect();
            let readable = writes.iter().map(|write| mind.view(write).unwrap().is_some()).collect();
            self.seen.lock().unwrap().push((ids, readable));
            Ok(())
        }
    }

    struct FailingIndex;

    impl<S: MindStore> IndexSink<S> for FailingIndex {
        fn committed(&mut self, _mind: &Mind<S>, _writes: &[PipelineRef]) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("the index is not there"))
        }
    }

    fn status(daemon: &mut Daemon<OwnedRedbMessagePackBackingStore, impl IndexSink<OwnedRedbMessagePackBackingStore>>) -> MindStatus {
        match daemon.handle(HuginnMindRequest::Whoami, now()) {
            HuginnMindResponse::Whoami(status) => status,
            other => panic!("expected a status, got {other:?}"),
        }
    }

    /// Typed hand-off between the organs without a socket: a batch enters and
    /// every read answers over what it landed, at a clock the test pins.
    #[test]
    fn a_batch_round_trips_typed_through_handle_without_a_socket() {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let outcome = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
        let HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { writes, .. }) = outcome else {
            panic!("expected a commit");
        };
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].kind, PipelineKind::Instance);

        assert_eq!(
            status(&mut daemon),
            MindStatus {
                instance: slug(INSTANCE),
                schema_epoch: "epiphany.pipeline.epoch.v1".into(),
                documents: 1,
                receipts: 1,
            }
        );
        assert_eq!(daemon.runtime_id(), "huginn-yggdrasil");

        let query = PipelineQuery { kinds: vec![PipelineKind::Instance], ..PipelineQuery::default() };
        let HuginnMindResponse::Query(page) =
            daemon.handle(HuginnMindRequest::Query { instance: slug(INSTANCE), query }, now())
        else {
            panic!("expected a page");
        };
        assert_eq!(page.matched, 1);
        assert_eq!(page.items[0].admission.provenance, provenance());
        assert_eq!(page.items[0].status, PipelineStatus::InForce);

        let id = writes[0].clone();
        let view = daemon.handle(HuginnMindRequest::View { instance: slug(INSTANCE), id: id.clone() }, now());
        assert_eq!(view, HuginnMindResponse::View(Some(page.items[0].clone())));

        let HuginnMindResponse::OpenItems(items) =
            daemon.handle(HuginnMindRequest::OpenItems { instance: slug(INSTANCE), campaign: slug(CAMPAIGN) }, now())
        else {
            panic!("expected open items");
        };
        assert_eq!(
            (items.questions.len(), items.findings.len(), items.follow_ups.len()),
            (0, 0, 0),
            "a mind holding only its identity has nothing open"
        );
        assert!(items.specs_without_report.is_empty() && items.reports_without_verdict.is_empty());

        let scope = HistoryScope::Repo(OrgRepo("GameCult/Huginn".into()));
        assert_eq!(
            daemon.handle(HuginnMindRequest::History { instance: slug(INSTANCE), scope }, now()),
            HuginnMindResponse::History(vec![])
        );
    }

    /// Ruling 14 across the transport, on both sides: the mind refuses a read
    /// and a write that name another instance, and nothing lands.
    #[test]
    fn a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let landed = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
        let HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { writes, .. }) = landed else {
            panic!("expected a commit");
        };
        let foreign = MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() };

        let query = HuginnMindRequest::Query { instance: slug(OTHER), query: PipelineQuery::default() };
        assert_eq!(daemon.handle(query, now()), HuginnMindResponse::Refused(foreign.clone()));

        let admit = HuginnMindRequest::Admit(batch(OTHER, vec![identity(OTHER)]));
        assert_eq!(
            daemon.handle(admit, now()),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(foreign.clone()))
        );

        // The declared instance travels to the mind as the client sent it. A
        // batch whose documents name no instance at all is still refused on
        // the declaration alone, so nothing between the wire and A1 may
        // rewrite it into the mind's own.
        let (question, _) = question_and_ruling();
        let admit = HuginnMindRequest::Admit(batch(OTHER, vec![question]));
        assert_eq!(
            daemon.handle(admit, now()),
            HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(foreign.clone()))
        );

        let view = HuginnMindRequest::View { instance: slug(OTHER), id: writes[0].clone() };
        assert_eq!(daemon.handle(view, now()), HuginnMindResponse::Refused(foreign.clone()));

        let items = HuginnMindRequest::OpenItems { instance: slug(OTHER), campaign: slug(CAMPAIGN) };
        assert_eq!(daemon.handle(items, now()), HuginnMindResponse::Refused(foreign.clone()));

        let scope = HistoryScope::Repo(OrgRepo("GameCult/Huginn".into()));
        let history = HuginnMindRequest::History { instance: slug(OTHER), scope };
        assert_eq!(daemon.handle(history, now()), HuginnMindResponse::Refused(foreign));

        // Three names a comparison could mistake for this mind's. `NEAR` is
        // `INSTANCE`'s own length and differs in its last byte, so a check
        // reading lengths, first bytes, or running only when the declared name
        // is longer admits it; `PREFIXED` carries `INSTANCE` whole, so a prefix
        // test admits it; `CASED` is `INSTANCE` in another case, which a slug
        // label permits, so a check folding case admits it. Each is refused on
        // the read side and the write side.
        assert_eq!(NEAR.len(), INSTANCE.len());
        assert!(PREFIXED.starts_with(INSTANCE));
        assert_eq!(CASED.to_ascii_lowercase(), INSTANCE);
        for declared in [NEAR, PREFIXED, CASED] {
            let refusal = MindRefusal::ForeignInstance { declared: declared.into(), mind: INSTANCE.into() };
            let query = HuginnMindRequest::Query { instance: slug(declared), query: PipelineQuery::default() };
            assert_eq!(daemon.handle(query, now()), HuginnMindResponse::Refused(refusal.clone()), "{declared}");
            let view = HuginnMindRequest::View { instance: slug(declared), id: writes[0].clone() };
            assert_eq!(daemon.handle(view, now()), HuginnMindResponse::Refused(refusal.clone()), "{declared}");
            let admit = HuginnMindRequest::Admit(batch(declared, vec![identity(declared)]));
            assert_eq!(
                daemon.handle(admit, now()),
                HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal)),
                "{declared}"
            );
        }

        assert_eq!(status(&mut daemon).documents, 1, "nothing landed");
    }

    /// A declared instance outside `Slug`'s grammar is refused by its own
    /// name, `InvalidFormat`, on the read path, before the identity
    /// comparison ever runs: it is not merely a foreign mind, it names no
    /// mind a client could compose. A fullwidth Latin small letter y is used
    /// because it is the shape Soul named: a check that folds it to plain
    /// ASCII `y` before validating would let this probe pass as grammatical
    /// and reach `require_instance`, which would then answer `ForeignInstance`
    /// instead — the wrong refusal, reached the wrong way.
    #[test]
    fn a_declared_instance_outside_the_grammar_is_refused_by_name_before_the_identity_check() {
        let (_root, mut daemon) = seeded();
        let fullwidth = "\u{FF59}ggdrasil";
        assert_ne!(fullwidth, INSTANCE, "not the mind's own bytes");
        assert!(!fullwidth.is_ascii(), "outside the grammar: a Slug is dot-joined ASCII labels");

        let expected = MindRefusal::Document(huginn_mind::epiphany_pipeline::PipelineRefusal::InvalidFormat {
            field: "instance.instance".into(),
            value: fullwidth.into(),
        });

        let query = HuginnMindRequest::Query { instance: slug(fullwidth), query: PipelineQuery::default() };
        assert_eq!(daemon.handle(query, now()), HuginnMindResponse::Refused(expected.clone()));

        let view = HuginnMindRequest::View {
            instance: slug(fullwidth),
            id: PipelineRef { kind: PipelineKind::Question, id: Short("not-reached".into()) },
        };
        assert_eq!(daemon.handle(view, now()), HuginnMindResponse::Refused(expected.clone()));

        let items = HuginnMindRequest::OpenItems { instance: slug(fullwidth), campaign: slug(CAMPAIGN) };
        assert_eq!(daemon.handle(items, now()), HuginnMindResponse::Refused(expected.clone()));

        let scope = HistoryScope::Repo(OrgRepo("GameCult/Huginn".into()));
        let history = HuginnMindRequest::History { instance: slug(fullwidth), scope };
        assert_eq!(daemon.handle(history, now()), HuginnMindResponse::Refused(expected));
    }

    /// A refusal a read method raised is the answer the dispatch returns, whole.
    /// The instance check refuses before the method runs and is pinned above;
    /// these are the refusals the methods themselves raise, so nothing here is
    /// the daemon's own. Each must arrive as the mind spelled it: not rewrapped
    /// as `Unavailable`, not rewrapped as another read's refusal, and not
    /// swallowed into an empty answer, which would read as "no such document"
    /// rather than "that is not a reference".
    ///
    /// The fourth arm, `open_items`, is pinned by the test below rather than
    /// here, because the refusal it must carry needs a store to be broken
    /// first.
    #[test]
    fn a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole() {
        let (_root, mut daemon) = seeded();

        // A reference whose kind and id disagree names no document any writer
        // could have composed; the leaf's own refusal says so.
        let invalid = PipelineRef { kind: PipelineKind::Question, id: "not a reference".into() };
        let malformed = MindRefusal::Document(huginn_mind::epiphany_pipeline::PipelineRefusal::InvalidFormat {
            field: "ref.id".into(),
            value: "not a reference".into(),
        });
        let view = HuginnMindRequest::View { instance: slug(INSTANCE), id: invalid.clone() };
        assert_eq!(daemon.handle(view, now()), HuginnMindResponse::Refused(malformed.clone()));

        let history = HuginnMindRequest::History { instance: slug(INSTANCE), scope: HistoryScope::Subject(invalid) };
        assert_eq!(daemon.handle(history, now()), HuginnMindResponse::Refused(malformed));

        // A semantic query is unavailable until Cut 11 wires an index, and the
        // detail is the mind's own sentence.
        let query = PipelineQuery {
            semantic: Some(SemanticQuery { text: "who owns the state".into(), top_k: 4 }),
            ..PipelineQuery::default()
        };
        let unwired =
            MindRefusal::Unavailable { detail: "semantic query: the index is not wired (Cut 11)".into() };
        assert_eq!(
            daemon.handle(HuginnMindRequest::Query { instance: slug(INSTANCE), query }, now()),
            HuginnMindResponse::Refused(unwired)
        );

        // And the same reads answer rather than refuse when they are asked
        // something answerable, so the arms above are refusing on the mind's
        // word and not on their own shape.
        let absent =
            PipelineRef { kind: PipelineKind::Question, id: Short(format!("{CAMPAIGN}:question:Q1")) };
        let present = HuginnMindRequest::View { instance: slug(INSTANCE), id: absent };
        assert_eq!(daemon.handle(present, now()), HuginnMindResponse::View(None));
        let plain = HuginnMindRequest::Query { instance: slug(INSTANCE), query: PipelineQuery::default() };
        let HuginnMindResponse::Query(page) = daemon.handle(plain, now()) else { panic!("expected a page") };
        assert_eq!(page.matched, 1);
    }

    /// The fourth read arm, pinned the same way. `open_items` raises no refusal
    /// of its own, so the refusal it must carry comes from the reader beneath
    /// it: a mind whose commit receipt has been deleted still opens, because
    /// the opener reads the store's types, epoch and identity and not its
    /// receipts, and then a read that joins a document to its receipt refuses
    /// by name over the orphan. The arm must return that sentence whole, like
    /// its three siblings: rewrapped it says the mind is unavailable for some
    /// unstated reason, and swallowed it says the campaign has nothing open,
    /// which is what a healthy mind says.
    ///
    /// The store is broken through `huginn-mind`'s own re-export of CultCache's
    /// row traits, so this crate still names one revision through one
    /// dependency. Hands had recorded this arm as impossible to pin; the
    /// swallow and the rewrap both survived the suite until this landed.
    #[test]
    fn a_refusal_open_items_raised_is_the_answer_the_dispatch_returns_whole() {
        use huginn_mind::HuginnCommitReceipt;
        use huginn_mind::store::{CacheBackingStore, DatabaseEntry};

        let root = tempfile::tempdir().unwrap();
        let path = Mind::<OwnedRedbMessagePackBackingStore>::path_for(root.path(), &slug(INSTANCE));
        {
            let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
            let outcome = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
            assert!(matches!(outcome, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })));
        }
        {
            let mut store = OwnedRedbMessagePackBackingStore::new(&path).unwrap();
            let rows = store.pull_all().unwrap();
            let receipt =
                rows.iter().find(|row| row.r#type == HuginnCommitReceipt::TYPE).expect("one receipt").clone();
            store.delete(&receipt).unwrap();
        }

        let mut daemon = Daemon::open(root.path(), &slug(INSTANCE)).expect("an orphaned image still opens");
        let orphaned = MindRefusal::Unavailable {
            detail: format!("document epiphany.pipeline.instance.v1/{INSTANCE}:instance:self has no commit receipt"),
        };
        let items = HuginnMindRequest::OpenItems { instance: slug(INSTANCE), campaign: slug(CAMPAIGN) };
        assert_eq!(daemon.handle(items, now()), HuginnMindResponse::Refused(orphaned));
    }

    /// Cut 11's seam: the index is handed every landed write, derived ones
    /// included, and can read each; an index that fails changes nothing.
    #[test]
    fn the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome() {
        let (_root, daemon) = seeded();
        let recording = RecordingIndex::default();
        let mut daemon = daemon.with_index(recording.clone());
        let (question, ruling) = question_and_ruling();

        let outcome = daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![question, ruling])), now());
        let HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { writes, .. }) = outcome else {
            panic!("expected a commit");
        };
        let landed: Vec<String> = writes.iter().map(|write| write.id.0.clone()).collect();
        assert_eq!(landed.len(), 3, "the ruling answers the question, so a resolution is derived: {landed:?}");

        let seen = recording.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, landed, "the sink sees exactly the landed writes");
        assert_eq!(seen[0].1, vec![true, true, true], "and can read each one when it is called");

        let (question, ruling) = question_and_ruling();
        let mut daemon = daemon.with_index(FailingIndex);
        let root = tempfile::tempdir().unwrap();
        let mut fresh = Daemon::open(root.path(), &slug(INSTANCE)).unwrap().with_index(FailingIndex);
        fresh.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
        let outcome = fresh.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![question, ruling])), now());
        let HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { writes, .. }) = outcome else {
            panic!("a failing index does not refuse the batch");
        };
        for write in &writes {
            let view = fresh.handle(HuginnMindRequest::View { instance: slug(INSTANCE), id: write.clone() }, now());
            assert!(matches!(view, HuginnMindResponse::View(Some(_))), "{write:?} is readable");
        }
        assert_eq!(status(&mut daemon).documents, 4, "identity, question, ruling and the derived resolution");
    }

    /// Ruling 15: a mind that will not open is refused, and `Daemon::open`
    /// takes no address, so no socket can precede it. `startup`'s order is
    /// pinned in `serve::tests`.
    #[test]
    fn the_daemon_refuses_loudly_when_it_cannot_open_the_mind_and_binds_nothing() {
        let root = tempfile::tempdir().unwrap();
        let held = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        let path = Mind::<OwnedRedbMessagePackBackingStore>::path_for(root.path(), &slug(INSTANCE));
        assert_eq!(
            Daemon::open(root.path(), &slug(INSTANCE)).err(),
            Some(MindRefusal::MindAlreadyOwned { path: path.display().to_string() })
        );
        drop(held);

        let mut planted = Daemon::open(root.path(), &slug(INSTANCE)).unwrap();
        planted.handle(HuginnMindRequest::Admit(batch(INSTANCE, vec![identity(INSTANCE)])), now());
        drop(planted);
        let moved = Mind::<OwnedRedbMessagePackBackingStore>::path_for(root.path(), &slug(OTHER));
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::copy(&path, &moved).unwrap();
        assert_eq!(
            Daemon::open(root.path(), &slug(OTHER)).err(),
            Some(MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() })
        );
    }
}
