//! The daemon half of the fixtures: an in-process Huginn daemon over a
//! temporary mind, on a loopback port, shared by the client and the server
//! tests.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::Utc;
use cultnet_rs::{CultNetMessage, CultNetRudpServerEvent, CultNetWireContract, decode_cultnet_message_from_slice};
use huginn_daemon::serve::{bind, run, schema_registry};
use huginn_daemon::{Daemon, Handled, Hits, IndexSink, SearchTicket, ServeOptions};
use huginn_mind::epiphany_pipeline::{
    Date, DocRef, OrgRepo, PipelineCampaign, PipelineDocument, PipelineInstance, PipelineRef, PipelineStewardship, Sha, Short, Slug,
    Title,
};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, IndexStatus, Mind, MindStore, OwnedRedbMessagePackBackingStore,
    PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelineProvenance,
};
use tempfile::TempDir;

pub const INSTANCE: &str = "eureka";

pub fn slug(value: &str) -> Slug {
    Slug(value.into())
}

pub fn identity(name: &str) -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: slug(name),
        display_name: Short(format!("{name} mind")),
        created_at: Date("2026-09-29".into()),
        host: Short(name.into()),
    })
}

pub fn batch(instance: &str, documents: Vec<PipelineDocument>) -> PipelineAdmissionBatch {
    PipelineAdmissionBatch {
        instance: slug(instance),
        provenance: PipelineProvenance {
            faculty: Faculty::Hands,
            agent: Short("claude".into()),
            session: Short("session-1".into()),
            tool: Short("admit".into()),
        },
        documents,
    }
}

/// No index: the client's subject is the transport, not the projection.
pub struct Inert;

impl<S: MindStore> IndexSink<S> for Inert {
    fn committed(&mut self, _mind: &Mind<S>, _writes: &[PipelineRef]) -> anyhow::Result<()> {
        Ok(())
    }
    fn status(&self) -> IndexStatus {
        IndexStatus::Current
    }
    fn search(&mut self, _text: &str, _top_k: u32) -> Result<SearchTicket, String> {
        Err("this index cannot search".into())
    }
    fn search_deadline(&self) -> Duration {
        Duration::ZERO
    }
    fn searched(&mut self) -> Vec<(SearchTicket, Result<Hits, String>)> {
        Vec::new()
    }
    fn abandon(&mut self, _ticket: SearchTicket) {}
}

pub type TestDaemon = Daemon<OwnedRedbMessagePackBackingStore, Inert>;

pub fn answered(handled: Handled) -> HuginnMindResponse {
    match handled {
        Handled::Answered(response) => response,
        Handled::Searching(_) => panic!("the inert index cannot search"),
    }
}

/// A daemon over a temporary mind whose identity is admitted, and whatever
/// else `seed` admits.
pub fn mind(seed: Vec<Vec<PipelineDocument>>) -> (TempDir, TestDaemon) {
    let root = tempfile::tempdir().unwrap();
    let mut daemon = Daemon::new(Mind::open(root.path(), &slug(INSTANCE)).unwrap(), Inert);
    for documents in std::iter::once(vec![identity(INSTANCE)]).chain(seed) {
        let outcome = answered(daemon.handle(HuginnMindRequest::Admit(batch(INSTANCE, documents)), Utc::now()));
        assert!(matches!(outcome, HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. })), "{outcome:?}");
    }
    (root, daemon)
}

/// A server on a loopback port that stops when dropped.
pub struct Server {
    pub addr: SocketAddr,
    pub stopping: Arc<AtomicBool>,
    pub thread: Option<JoinHandle<()>>,
    /// The sessions a scripted server has seen end: what the client's
    /// disconnect looks like from the daemon's side.
    pub ended: Arc<AtomicUsize>,
    pub _root: Option<TempDir>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn serve(root: TempDir, mut daemon: TestDaemon) -> Server {
    let mut hub = bind("127.0.0.1:0".parse().unwrap(), &daemon.runtime_id()).unwrap();
    let addr = hub.local_addr().unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let loop_stopping = Arc::clone(&stopping);
    let registry = schema_registry().unwrap();
    let thread = std::thread::spawn(move || {
        run(&mut daemon, &mut hub, &registry, &loop_stopping, &ServeOptions::default()).unwrap();
    });
    Server { addr, stopping, thread: Some(thread), ended: Arc::default(), _root: Some(root) }
}

/// A server that answers each schema message with whatever `script` says.
pub fn scripted(mut script: impl FnMut(&CultNetMessage) -> CultNetMessage + Send + 'static) -> Server {
    let mut hub = bind("127.0.0.1:0".parse().unwrap(), "scripted").unwrap();
    let addr = hub.local_addr().unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let loop_stopping = Arc::clone(&stopping);
    let ended = Arc::new(AtomicUsize::new(0));
    let loop_ended = Arc::clone(&ended);
    let thread = std::thread::spawn(move || {
        while !loop_stopping.load(Ordering::Relaxed) {
            hub.poll_resends().unwrap();
            while let Some(event) = hub.receive_event_once().unwrap() {
                if matches!(event, CultNetRudpServerEvent::Disconnected { .. }) {
                    loop_ended.fetch_add(1, Ordering::Relaxed);
                }
                let CultNetRudpServerEvent::Frame { session, frame } = event else { continue };
                let message =
                    decode_cultnet_message_from_slice(&frame.payload, CultNetWireContract::CultNetSchemaV0).unwrap();
                hub.send_schema_message(&session, &script(&message)).unwrap();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    Server { addr, stopping, thread: Some(thread), ended, _root: None }
}

/// A scripted server whose every answer is `reply`.
pub fn answering(reply: CultNetMessage) -> Server {
    scripted(move |_| reply.clone())
}

pub const CAMPAIGN: &str = "eureka-state";
pub const REPO: &str = "GameCult/Epiphany";

pub fn sha() -> Sha {
    Sha("5f98228d".into())
}

pub fn campaign() -> PipelineDocument {
    PipelineDocument::Campaign(PipelineCampaign {
        slug: slug(CAMPAIGN),
        title: Title("Eureka pipeline state".into()),
        repos: vec![OrgRepo(REPO.into())],
        working_branch: Short("hands/cut13a".into()),
        target_doc: DocRef { path: Short("notes/target.md".into()), start_line: 1, end_line: 9, commit: sha() },
    })
}

pub fn stewardship() -> PipelineDocument {
    PipelineDocument::Stewardship(PipelineStewardship {
        instance: slug(INSTANCE),
        repo: OrgRepo(REPO.into()),
        sequence: 1,
        assigned_on: Date("2026-09-29".into()),
        note: "assigned".into(),
    })
}
