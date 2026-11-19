//! Her organs' port to her mind (`MindPort`, and `HuginnMind` over a daemon),
//! the paged reads built on it, and what an open question holds (`on_hold`),
//! which her Self's inbox reads before it tells a brief.
//!
//! Nothing here keeps state: every answer is derived from the mind on every
//! call. The work queue and the repetition breaker that guarded Self-run
//! launches are parked with them at tag `parked/self-run-launch`.

use anyhow::{Result, anyhow, bail};
use cultnet_rs::Selection;
use eureka_pipeline::{Line, OrgRepo, PipelineDocument, PipelineRef, Slug};
use eureka_state::hold::{self, Hold};
use eureka_state::{ClientError, HuginnClient};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, PipelineAdmissionBatch,
    PipelineAdmissionOutcome, PipelineDocumentSummary, PipelineProvenance,
    PipelineSelectionPage, RepoActivity, RootTouched, SemanticQuery,
};

/// What her organs ask of the mind: a typed selection, and an admission as
/// herself. The impl for a daemon is `HuginnMind`; a test
/// hands in a mind opened over a temporary directory.
pub trait MindPort {
    /// The instance this port acts for.
    fn instance(&self) -> &Slug;
    /// One page of the selection. A refusal is an error here and names no
    /// value.
    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage>;
    /// The `top_k` documents of the selection nearest `text` by meaning, ranked: the same read as
    /// `query` with a semantic ask, so one call path carries both. A port with no index refuses.
    fn query_near(&self, _selection: &Selection, _text: &str, _top_k: u32) -> Result<PipelineSelectionPage> {
        bail!("this mind port has no semantic index")
    }
    /// What others did in `repos` on the forge since the cursor `after`, from the daemon's own forge read (no document of the mind
    /// is read). A port with no forge refuses; so does a daemon without one, and a forge that does not answer in time.
    fn stewarded_activity(&self, _repos: &[OrgRepo], _after: Option<u64>) -> Result<RepoActivity> {
        bail!("this mind port has no forge to read")
    }
    /// Every root her mind holds a document under, with its newest admission and a campaign's repos in force. A port that does not
    /// read a mind refuses.
    fn roots_touched(&self) -> Result<Vec<RootTouched>> {
        bail!("this mind port has no roots to read")
    }
    /// Admits `documents` as one batch, attributed to `agent` in `session`.
    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome>;
}

/// One call to a mind: the `eureka-state` client's `call`, or whatever a test
/// answers with.
type Call<'a> = Box<dyn Fn(HuginnMindRequest) -> Result<HuginnMindResponse, ClientError> + 'a>;

/// The daemon's mind, reached over the `eureka-state` client.
pub struct HuginnMind<'a> {
    instance: Slug,
    call: Call<'a>,
}

impl HuginnMind<'static> {
    pub fn new(client: HuginnClient) -> Self {
        Self::over(client.instance().clone(), Box::new(move |request| client.call(request)))
    }
}

impl<'a> HuginnMind<'a> {
    pub(crate) fn over(instance: Slug, call: Call<'a>) -> Self {
        Self { instance, call }
    }

    /// The one query path: `semantic` None is a selection, Some is a ranked ask.
    fn ask(&self, selection: &Selection, semantic: Option<SemanticQuery>) -> Result<PipelineSelectionPage> {
        let request = HuginnMindRequest::Query { instance: self.instance().clone(), selection: selection.clone(), semantic };
        match (self.call)(request).map_err(|error| anyhow!("the mind could not be asked: {}", client_error_code(&error)))? {
            HuginnMindResponse::Query(page) => Ok(page),
            HuginnMindResponse::Refused(_) => bail!("the mind refused the query"),
            _ => bail!("the mind answered a query with something else"),
        }
    }
}

impl MindPort for HuginnMind<'_> {
    fn instance(&self) -> &Slug {
        &self.instance
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        self.ask(selection, None)
    }

    fn query_near(&self, selection: &Selection, text: &str, top_k: u32) -> Result<PipelineSelectionPage> {
        self.ask(selection, Some(SemanticQuery { text: Line(text.to_string()), top_k }))
    }

    fn stewarded_activity(&self, repos: &[OrgRepo], after: Option<u64>) -> Result<RepoActivity> {
        let request = HuginnMindRequest::StewardedActivity { instance: self.instance().clone(), repos: repos.to_vec(), after };
        match (self.call)(request).map_err(|error| anyhow!("the mind could not be asked: {}", client_error_code(&error)))? {
            HuginnMindResponse::Activity(activity) => Ok(activity),
            HuginnMindResponse::Refused(_) => bail!("the mind refused the activity read"),
            _ => bail!("the mind answered an activity read with something else"),
        }
    }

    fn roots_touched(&self) -> Result<Vec<RootTouched>> {
        let request = HuginnMindRequest::RootsTouched { instance: self.instance().clone() };
        match (self.call)(request).map_err(|error| anyhow!("the mind could not be asked: {}", client_error_code(&error)))? {
            HuginnMindResponse::RootsTouched(roots) => Ok(roots),
            HuginnMindResponse::Refused(_) => bail!("the mind refused the roots read"),
            _ => bail!("the mind answered a roots read with something else"),
        }
    }

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        let batch = PipelineAdmissionBatch {
            instance: self.instance().clone(),
            provenance: PipelineProvenance { faculty: Faculty::SelfFaculty, agent: agent.into(), session: session.into(), tool: "mind-body".into(), imported_from: None },
            documents,
        };
        match (self.call)(HuginnMindRequest::Admit(batch)).map_err(|error| anyhow!("the mind could not be asked: {}", client_error_code(&error)))? {
            HuginnMindResponse::Admit(outcome) => Ok(outcome),
            _ => bail!("the mind answered an admission with something else"),
        }
    }
}

/// The variant of a client failure, without its detail (an endpoint and a
/// transport message are not ours to print).
fn client_error_code(error: &ClientError) -> &'static str {
    match error {
        ClientError::Unavailable { .. } => "unavailable",
        ClientError::Rejected { .. } => "rejected",
        ClientError::TooLarge { .. } => "too_large",
        ClientError::Unencodable { .. } => "unencodable",
    }
}

pub(crate) use eureka_state::hold::{any_of, citing, in_force, of_kind};

/// Every page of a selection, in the mind's order.
pub(crate) fn headers(
    mind: &dyn MindPort,
    selection: &Selection,
) -> Result<Vec<PipelineDocumentSummary>> {
    hold::headers(&|asking| mind.query(asking), selection)
}

/// Every question holding anything and what it holds: `eureka_state::hold`,
/// the one decider, which `huginn blocked` reads too.
pub(crate) fn holds(mind: &dyn MindPort) -> Result<Vec<Hold>> {
    hold::holds(&|asking| mind.query(asking))
}

/// A snapshot of the items an open question holds. Only `on_hold` makes one
/// (there is no empty default), and the launch gate reads its own, once, at
/// admission; a reader that chooses claims by one is advisory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held(Vec<PipelineRef>);

impl Held {
    pub fn contains(&self, item: &PipelineRef) -> bool {
        self.0.contains(item)
    }

    pub fn items(&self) -> &[PipelineRef] {
        &self.0
    }
}

/// The items an in-force question holds, read now: the one owner of the stop.
pub fn on_hold(mind: &dyn MindPort) -> Result<Held> {
    Ok(Held(
        holds(mind)?
            .into_iter()
            .flat_map(|hold| hold.items)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use eureka_pipeline::{PipelineKind, RunOperator, RunTurn};

    use super::*;
    use crate::testkit::*;

    /// The adapter over the wire vocabulary, answered by a real mind.
    #[test]
    fn the_daemon_port_asks_and_admits_in_the_wire_vocabulary() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x")]);
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| Ok(mind.answer(request))));
        assert_eq!(port.instance().0, INSTANCE);
        let specs: Vec<PipelineRef> = headers(&port, &in_force(of_kind(PipelineKind::CutSpec))).unwrap().into_iter().map(|header| header.id).collect();
        assert_eq!(specs, vec![spec_ref("x")]);
        let outcome = port
            .admit(
                "agent-x",
                "session-y",
                vec![run("p", RunTurn::PersonaTurn, RunOperator::Mind, &[])],
            )
            .unwrap();
        assert!(
            matches!(outcome, PipelineAdmissionOutcome::Committed { .. }),
            "{outcome:?}"
        );
        let facts = mind
            .view(PipelineKind::Run, &run_ref("p").id.0)
            .unwrap()
            .admission
            .provenance;
        assert_eq!(
            (
                facts.faculty,
                facts.agent.0.as_str(),
                facts.session.0.as_str(),
                facts.tool.0.as_str()
            ),
            (Faculty::SelfFaculty, "agent-x", "session-y", "mind-body")
        );
        let refused = port.query(&any_of(of_kind(PipelineKind::Ruling), "turn", &["canary-turn-5d2e"])).unwrap_err();
        assert!(format!("{refused:#}").contains("refused the query"));
        assert!(!format!("{refused:#}").contains("canary"), "{refused:#}");
    }

    /// The ranked ask goes out as the one Query request with a semantic part; a plain query carries none.
    #[test]
    fn a_near_query_is_the_same_wire_request_with_a_semantic_part() {
        let seen = std::cell::RefCell::new(Vec::new());
        let mind = TestMind::seeded();
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| {
            if let HuginnMindRequest::Query { selection, semantic, .. } = &request {
                seen.borrow_mut().push((selection.clone(), semantic.clone()));
            }
            Ok(mind.answer(request))
        }));
        let selection = in_force(of_kind(PipelineKind::Ruling));
        port.query(&selection).unwrap();
        // The test mind answers the selection and ignores the ask, as a mind without an index would.
        port.query_near(&selection, "the fork", 8).unwrap();
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], (selection.clone(), None));
        assert_eq!(seen[1], (selection.clone(), Some(SemanticQuery { text: Line("the fork".into()), top_k: 8 })));
    }

    /// The activity ask goes out as the one `StewardedActivity` request naming her instance, repos and cursor, and the daemon's
    /// `Activity` answer comes back whole; a refusal, or any other answer, is an error that carries nothing of the refusal.
    #[test]
    fn an_activity_read_goes_through_the_port_as_one_request() {
        let seen = std::cell::RefCell::new(Vec::new());
        let read = RepoActivity { head: 77, events: vec![], capped: vec![OrgRepo::from("GameCult/Huginn")], open_prs: vec![], unseen: vec![OrgRepo::from("GameCult/Ghost")], moved: vec![] };
        let answer = std::cell::RefCell::new(HuginnMindResponse::Activity(read.clone()));
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| {
            seen.borrow_mut().push(request);
            Ok(answer.borrow().clone())
        }));
        let repos = [OrgRepo::from("GameCult/Huginn")];
        assert_eq!(port.stewarded_activity(&repos, Some(5)).unwrap(), read);
        assert_eq!(
            seen.borrow()[0],
            HuginnMindRequest::StewardedActivity { instance: Slug(INSTANCE.into()), repos: repos.to_vec(), after: Some(5) }
        );

        *answer.borrow_mut() = HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable { detail: "canary-forge-detail".into() });
        let refused = port.stewarded_activity(&repos, None).unwrap_err();
        assert!(format!("{refused:#}").contains("refused the activity read") && !format!("{refused:#}").contains("canary"), "{refused:#}");
        *answer.borrow_mut() = HuginnMindResponse::View(None);
        assert!(format!("{:#}", port.stewarded_activity(&repos, None).unwrap_err()).contains("something else"));
    }

    /// The roots ask goes out as the one `RootsTouched` request naming her instance, a real mind's answer comes back whole through
    /// `HuginnMind` itself, and a refusal, a client error or any other answer is an error that carries nothing of the refusal.
    #[test]
    fn a_roots_read_goes_through_the_port_as_one_request() {
        let mind = TestMind::seeded();
        let seen = std::cell::RefCell::new(Vec::new());
        let broken = std::cell::Cell::new(0u8);
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| {
            seen.borrow_mut().push(request.clone());
            match broken.get() {
                0 => Ok(mind.answer(request)),
                1 => Ok(HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable { detail: "canary-roots-detail".into() })),
                2 => Ok(HuginnMindResponse::View(None)),
                _ => Err(ClientError::Unencodable { detail: "canary-detail".into() }),
            }
        }));
        let roots = port.roots_touched().unwrap();
        assert_eq!(seen.borrow()[0], HuginnMindRequest::RootsTouched { instance: Slug(INSTANCE.into()) });
        let campaign = roots.iter().find(|root| root.root.0 == CAMPAIGN).expect("the seeded campaign is a root");
        assert_eq!(campaign.repos, [OrgRepo::from(REPO)], "the campaign's repos in force come back through the port");
        assert!(roots.iter().any(|root| root.root.0 == INSTANCE && root.repos.is_empty()), "{roots:?}");

        broken.set(1);
        let refused = port.roots_touched().unwrap_err();
        assert!(format!("{refused:#}").contains("refused the roots read") && !format!("{refused:#}").contains("canary"), "{refused:#}");
        broken.set(2);
        assert!(format!("{:#}", port.roots_touched().unwrap_err()).contains("something else"));
        broken.set(3);
        let down = port.roots_touched().unwrap_err();
        assert!(format!("{down:#}").contains("could not be asked") && !format!("{down:#}").contains("canary"), "{down:#}");
    }

    fn bundle() -> (TestMind, PipelineRef, PipelineRef) {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("a"), spec("b")]);
        let (a, b) = (spec_ref("a"), spec_ref("b"));
        for n in 0..3 {
            mind.committed(vec![run(&format!("f{n}"), RunTurn::SelfRun, RunOperator::Mind, &[a.clone(), b.clone()])]);
            mind.committed(vec![close(run_ref(&format!("f{n}")), recorded())]);
        }
        (mind, a, b)
    }

    #[test]
    fn a_question_raised_in_a_run_holds_every_claim_of_it_until_answered_or_withdrawn() {
        for withdraw in [false, true] {
            let (mind, a, b) = bundle();
            assert!(on_hold(&mind).unwrap().items().is_empty());
            mind.committed(vec![question_in("fork", &run_ref("f2"))]);
            let held = on_hold(&mind).unwrap();
            assert!(
                held.contains(&a) && held.contains(&b) && held.items().len() == 2,
                "{held:?}: the claims of the resolved run"
            );
            let question = id("question", "fork");
            if withdraw {
                mind.committed(vec![close(reference(PipelineKind::Question, &question), withdrawn())]);
            } else {
                mind.committed(vec![ruling("answer", Some((&question, "A")))]);
            }
            assert!(
                on_hold(&mind).unwrap().items().is_empty(),
                "answered or withdrawn, nothing is held (withdrawn: {withdraw})"
            );
        }
    }

    #[test]
    fn a_question_answered_retire_keeps_holding_the_claims_that_are_still_in_force() {
        let (mind, a, b) = bundle();
        mind.committed(vec![question_in("fork", &run_ref("f2"))]);
        let question = id("question", "fork");
        mind.committed(vec![ruling(
            "answer",
            Some((&question, eureka_state::hold::RETIRE)),
        )]);
        let held = on_hold(&mind).unwrap();
        assert!(
            held.contains(&a) && held.contains(&b) && held.items().len() == 2,
            "{held:?}: retire holds what the answer retired"
        );
        // Another run's question, still open, is not this answer's: only the claims of f2 are retired.
        let c = spec_ref("c");
        mind.committed(vec![spec("c")]);
        mind.committed(vec![run("g", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&c))]);
        mind.committed(vec![close(run_ref("g"), recorded())]);
        mind.committed(vec![question_in("other", &run_ref("g"))]);
        let retired: Vec<PipelineRef> = holds(&mind).unwrap().into_iter().filter(|hold| hold.retired).flat_map(|hold| hold.items).collect();
        assert_eq!(retired, [a.clone(), b.clone()]);
        mind.committed(vec![close(a, withdrawn())]);
        let held = on_hold(&mind).unwrap();
        assert!(held.contains(&b) && held.contains(&c) && held.items().len() == 2, "{held:?}: a resolved claim drops out");
        mind.committed(vec![close(b, withdrawn())]);
        assert_eq!(on_hold(&mind).unwrap().items(), [c], "all of f2's resolved, only the open question's claim is held");
    }

    #[test]
    fn a_question_answered_with_another_choice_holds_nothing() {
        let (mind, _, _) = bundle();
        mind.committed(vec![question_in("fork", &run_ref("f2"))]);
        mind.committed(vec![ruling(
            "answer",
            Some((&id("question", "fork"), "reopen")),
        )]);
        assert!(on_hold(&mind).unwrap().items().is_empty());
        assert!(holds(&mind).unwrap().is_empty());
    }

    #[test]
    fn a_question_raised_in_anything_else_holds_that_item_alone() {
        let (mind, a, b) = bundle();
        mind.committed(vec![question_in("fork", &a)]);
        assert_eq!(
            on_hold(&mind).unwrap().items(),
            [a],
            "the legacy shape: raised in the item, it holds the item and no claimant"
        );
        assert!(!on_hold(&mind).unwrap().contains(&b));
    }

    #[test]
    fn an_unreachable_mind_names_no_input() {
        let canary = "canary-instance-7f3a91";
        let endpoint: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let port = HuginnMind::new(HuginnClient::new(
            endpoint,
            Slug(canary.into()),
            Duration::from_millis(300),
        ));
        let error = on_hold(&port).unwrap_err();
        let shown = format!("{error:#}");
        assert!(shown.contains("unavailable"), "{shown}");
        assert!(!shown.contains(canary) && !shown.contains("127.0.0.1"), "{shown}");
    }
}
