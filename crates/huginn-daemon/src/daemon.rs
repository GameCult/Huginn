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
    /// other request names an instance, and the mind is asked whether that is
    /// its own before the method runs.
    pub fn handle(&mut self, request: HuginnMindRequest, now: DateTime<Utc>) -> HuginnMindResponse {
        if let Some(declared) = request.instance()
            && !matches!(request, HuginnMindRequest::Admit(_))
            && let Err(refusal) = self.mind.require_instance(declared)
        {
            return HuginnMindResponse::Refused(refusal);
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
        Date, OrgRepo, PipelineDocument, PipelineInstance, PipelineKind, PipelineQuestion, PipelineRuling,
        QuestionOption, RulingAuthority, Short,
    };
    use huginn_mind::wire::MindStatus;
    use huginn_mind::{
        Faculty, HistoryScope, PipelineAdmissionBatch, PipelineProvenance, PipelineQuery, PipelineStatus,
    };
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    pub(crate) const INSTANCE: &str = "yggdrasil";
    pub(crate) const OTHER: &str = "thought-cage";
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

        let view = HuginnMindRequest::View { instance: slug(OTHER), id: writes[0].clone() };
        assert_eq!(daemon.handle(view, now()), HuginnMindResponse::Refused(foreign.clone()));

        let items = HuginnMindRequest::OpenItems { instance: slug(OTHER), campaign: slug(CAMPAIGN) };
        assert_eq!(daemon.handle(items, now()), HuginnMindResponse::Refused(foreign.clone()));

        let scope = HistoryScope::Repo(OrgRepo("GameCult/Huginn".into()));
        let history = HuginnMindRequest::History { instance: slug(OTHER), scope };
        assert_eq!(daemon.handle(history, now()), HuginnMindResponse::Refused(foreign));

        assert_eq!(status(&mut daemon).documents, 1, "nothing landed");
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
