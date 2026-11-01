//! The Persona's view of open work, and the guard on it: what may be worked
//! (`queue`) and whether an item has stopped paying for its runs (`breaker`).
//!
//! Nothing here keeps state. Both answers are derived from the mind on every
//! call, through `MindPort`: the mind is the only store, so a breaker that
//! needed a file of its own would be a second owner of what the runs already
//! record (target invariant run-is-the-grant).

use anyhow::{Result, anyhow, bail};
use cultnet_rs::{Incoming, Selection};
use eureka_pipeline::{
    Line, PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, RunOperator, RunTurn,
    Slug,
};
use eureka_state::hold::{self, Hold};
use eureka_state::{ClientError, HuginnClient};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, PipelineAdmissionBatch,
    PipelineAdmissionOutcome, PipelineDocumentSummary, PipelineFacts, PipelineProvenance,
    PipelineSelectionPage, PipelineStatusSummary, SemanticQuery,
};

/// The last `CONSECUTIVE` ended runs over an item, all empty, trip its breaker.
pub const CONSECUTIVE: usize = 3;
/// So do `EMPTY_SHARE` empty ones among the last `WINDOW` (Codex auto-review's
/// per-turn breaker, `docs/research/autonomous-harness-prior-art.md` section 0).
pub const WINDOW: usize = 50;
pub const EMPTY_SHARE: usize = 10;

/// The two things her launch organs ask of the mind: a typed selection, and
/// an admission as herself. The impl for a daemon is `HuginnMind`; a test
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

    fn admit(&self, agent: &str, session: &str, documents: Vec<PipelineDocument>) -> Result<PipelineAdmissionOutcome> {
        let batch = PipelineAdmissionBatch {
            instance: self.instance().clone(),
            provenance: PipelineProvenance { faculty: Faculty::SelfFaculty, agent: agent.into(), session: session.into(), tool: "mind-body".into() },
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

/// The claims of the runs of `instance` that are still in force and not in
/// `ending`: the work someone holds.
pub(crate) fn live_claims(mind: &dyn MindPort, instance: &Slug, ending: &[PipelineRef]) -> Result<Vec<PipelineRef>> {
    let runs = in_force(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]));
    let mut claims = Vec::new();
    for header in headers(mind, &runs)? {
        if let PipelineFacts::Run { claims: held, .. } = header.facts
            && !ending.contains(&header.id)
        {
            claims.extend(held);
        }
    }
    Ok(claims)
}

/// The claims of the run `run`, resolved or not (a citation outlives its
/// resolution). Empty when the mind has no such run.
pub(crate) fn claims_of(mind: &dyn MindPort, run: &PipelineRef) -> Result<Vec<PipelineRef>> {
    hold::claims_of(&|asking| mind.query(asking), run)
}

/// Every question holding anything and what it holds: `eureka_state::hold`,
/// the one decider, which `huginn blocked` reads too.
pub(crate) fn holds(mind: &dyn MindPort) -> Result<Vec<Hold>> {
    hold::holds(&|asking| mind.query(asking))
}

/// A snapshot of the items an open question holds, read once by whoever
/// launches and passed to everything that launch consults, so the gate, the
/// queue and the claims they chose cannot answer differently within one launch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// What may be worked, oldest first: the cut specs in force that no report
/// answers, that no open question holds, and that no live run claims. A run in
/// `ending` is one the caller closes in the same admission that opens the next,
/// so it claims nothing here.
pub fn queue(
    mind: &dyn MindPort,
    instance: &Slug,
    ending: &[PipelineRef],
    held: &Held,
) -> Result<Vec<PipelineRef>> {
    let unreported = Selection {
        cited: Some(Incoming {
            role: "cut_spec".into(),
            exists: false,
        }),
        ..in_force(of_kind(PipelineKind::CutSpec))
    };
    let claimed = live_claims(mind, instance, ending)?;
    Ok(headers(mind, &unreported)?
        .into_iter()
        .map(|header| header.id)
        .filter(|spec| !held.contains(spec) && !claimed.contains(spec))
        .collect())
}

/// Whether an item may still be launched on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Breaker {
    Clear,
    /// Tripped, and the trip belongs to `run`: the newest ended counted run on
    /// the item, the run whose end completed the streak or the share.
    Trip { run: PipelineRef },
}

/// The repetition breaker, derived from the documents that cite the item.
///
/// One role-less citation selection over every schema gives the set: the runs
/// that claim the item, and whatever else points at it (a follow-up sourced
/// from it, a report, a resolution, a question raised in it). The newest
/// question raised in the item is the boundary: only runs admitted after it
/// count, so answering a trip starts a fresh count. A counted run is a Self run
/// of hers (typed `turn` and `operated_by`) that was not withdrawn; a
/// withdrawn run never ran and is no run here. A counted run is ended when
/// resolved or listed in `ending`, and is empty when no document of the set
/// other than a run was admitted from its own admission up to the next counted
/// run's (or to now). Trip, naming the newest ended counted run, when the last `CONSECUTIVE` ended runs are empty, or
/// `EMPTY_SHARE` of the last `WINDOW`. What else the mind admitted, such as a
/// Persona turn's run or an unrelated ruling, cites nothing here and counts for
/// nothing.
pub fn breaker(mind: &dyn MindPort, item: &PipelineRef, ending: &[PipelineRef]) -> Result<Breaker> {
    let mut set = headers(mind, &Selection { cites: Some(citing(item, None)), ..Selection::default() })?;
    set.sort_by_key(|header| header.admission.ordinal);
    let asked_in_item = set
        .iter()
        .filter(|header| matches!(&header.facts, PipelineFacts::Question { raised_in: Some(raised), .. } if raised == item))
        .map(|header| header.admission.ordinal);
    let asked_in_run = Selection {
        cites: Some(citing(item, Some("claims"))),
        cited: Some(Incoming { role: "raised_in".into(), exists: true }),
        ..of_kind(PipelineKind::Run)
    };
    let boundary = asked_in_item.chain(headers(mind, &asked_in_run)?.into_iter().map(|run| run.admission.ordinal)).max();
    let after = |header: &&PipelineDocumentSummary| boundary.is_none_or(|boundary| header.admission.ordinal > boundary);
    let withdrawn =
        |run: &PipelineDocumentSummary| matches!(run.status, PipelineStatusSummary::Resolved { outcome: ResolutionOutcome::Withdrawn { .. }, .. });
    // (admission ordinal, ended, ref) of each counted run, oldest first.
    let ran: Vec<(u64, bool, PipelineRef)> = set
        .iter()
        .filter(after)
        .filter(|header| {
            matches!(header.facts, PipelineFacts::Run { turn: RunTurn::SelfRun, operated_by: RunOperator::Mind, .. }) && !withdrawn(header)
        })
        .map(|run| (run.admission.ordinal, matches!(run.status, PipelineStatusSummary::Resolved { .. }) || ending.contains(&run.id), run.id.clone()))
        .collect();
    let ended: Vec<usize> = (0..ran.len()).filter(|index| ran[*index].1).collect();
    let considered: Vec<usize> = ended.iter().rev().take(WINDOW).copied().collect();
    let admitted: Vec<u64> =
        set.iter().filter(|header| !matches!(header.facts, PipelineFacts::Run { .. })).map(|header| header.admission.ordinal).collect();

    // Newest first, like `considered`.
    let empty: Vec<bool> = considered
        .iter()
        .map(|index| {
            let end = ran.get(index + 1).map_or(u64::MAX, |next| next.0);
            !admitted.iter().any(|ordinal| *ordinal >= ran[*index].0 && *ordinal < end)
        })
        .collect();
    let consecutive = empty.len() >= CONSECUTIVE && empty[..CONSECUTIVE].iter().all(|empty| *empty);
    let share = empty.iter().filter(|empty| **empty).count() >= EMPTY_SHARE;
    Ok(match ended.last() {
        Some(newest) if consecutive || share => Breaker::Trip { run: ran[*newest].2.clone() },
        _ => Breaker::Clear,
    })
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use eureka_pipeline::{RunOperator, RunTurn};

    use super::*;

    /// `queue` against a hold read now.
    fn queued(
        mind: &dyn MindPort,
        instance: &Slug,
        ending: &[PipelineRef],
    ) -> Result<Vec<PipelineRef>> {
        queue(mind, instance, ending, &on_hold(mind)?)
    }
    use crate::testkit::*;

    /// A mind holding the spec `x` and a history of ended runs on it.
    fn worked(runs: &[bool]) -> (TestMind, PipelineRef) {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x")]);
        let item = spec_ref("x");
        history(&mind, &item, runs);
        (mind, item)
    }

    const E: bool = false;
    const N: bool = true;

    fn tripped(runs: &[bool]) -> bool {
        let (mind, item) = worked(runs);
        matches!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { .. })
    }

    #[test]
    fn three_consecutive_empty_runs_trip_and_two_do_not() {
        assert!(!tripped(&[]));
        assert!(!tripped(&[E, E]));
        assert!(tripped(&[E, E, E]));
        assert!(tripped(&[N, E, E, E]));
    }

    #[test]
    fn a_document_in_a_runs_window_makes_that_run_not_empty() {
        assert!(!tripped(&[E, E, N, E, E]), "one run with a document in its window breaks the streak");
        assert!(tripped(&[E, E, N, E, E, E]));
    }

    #[test]
    fn ten_empty_of_the_last_fifty_trip_with_gaps_and_nine_do_not() {
        let ten: Vec<bool> = [E, E, N].repeat(5);
        assert!(tripped(&ten), "10 empty, never 3 in a row");
        let mut nine: Vec<bool> = [E, E, N].repeat(4);
        nine.push(E);
        assert!(!tripped(&nine), "9 empty, never 3 in a row");
    }

    #[test]
    fn only_the_last_fifty_ended_runs_are_counted() {
        let mut runs = vec![E];
        for _ in 0..9 {
            runs.extend([E, N]);
        }
        runs.extend([N].repeat(32));
        assert_eq!(runs.len(), 51);
        assert!(!tripped(&runs), "the tenth empty run is the 51st newest");
    }

    #[test]
    fn the_operators_self_runs_on_the_item_are_not_hers_to_count() {
        let after_a_run_by = |operated_by: RunOperator| {
            let (mind, item) = worked(&[E, E]);
            mind.committed(vec![run("third", RunTurn::SelfRun, operated_by, std::slice::from_ref(&item))]);
            mind.committed(vec![close(run_ref("third"), recorded())]);
            breaker(&mind, &item, &[]).unwrap()
        };
        assert!(matches!(after_a_run_by(RunOperator::Mind), Breaker::Trip { .. }), "hers, the third empty one");
        assert_eq!(after_a_run_by(RunOperator::Operator), Breaker::Clear, "his, counted for nothing");
    }

    #[test]
    fn the_fiftieth_newest_ended_run_is_counted() {
        // Ten empty of exactly fifty, the tenth the oldest, never three in a row.
        let mut runs = vec![E];
        for _ in 0..9 {
            runs.extend([E, N]);
        }
        runs.extend([N].repeat(31));
        assert_eq!(runs.len(), WINDOW);
        assert!(tripped(&runs));
    }

    #[test]
    fn runs_born_closed_in_the_questions_batch_are_before_the_count() {
        // One batch holds the question raised in x and two runs of hers on x, each closed at birth.
        let (mind, item) = worked(&[]);
        mind.committed(vec![
            question_in("fork", &item),
            run("pa", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)),
            close(run_ref("pa"), recorded()),
            run("pb", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)),
            close(run_ref("pb"), recorded()),
        ]);
        // Then thirteen ended runs, nine of them empty and never three in a row.
        let mut after: Vec<bool> = [E, E, N].repeat(4);
        after.push(E);
        history(&mind, &item, &after);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear, "the pair is not after the question, so the tenth empty run is not counted");
    }

    #[test]
    fn three_empty_runs_trip_after_any_history_of_non_empty_ones() {
        for before in [0usize, 1, 12, 20] {
            let mut runs = vec![N; before];
            runs.extend([E, E, E]);
            assert!(tripped(&runs), "{before} non-empty runs then three empty ones");
            assert!(!tripped(&[vec![N; before], vec![E, E]].concat()), "two empty ones never trip");
        }
    }

    #[test]
    fn only_documents_that_cite_the_item_make_a_run_non_empty() {
        let (mind, item) = worked(&[]);
        mind.committed(vec![spec("other")]);
        for n in 0..3 {
            // A Persona turn's run, a ruling and a follow-up of another item all land in the window.
            mind.committed(vec![run(&format!("p{n}"), RunTurn::PersonaTurn, RunOperator::Mind, &[])]);
            mind.committed(vec![ruling(&format!("r{n}"), None), follow_up(&format!("o{n}"), &spec_ref("other"))]);
            ended_run(&mind, &item, "h", n, false);
            mind.committed(vec![close(run_ref(&format!("p{n}")), recorded())]);
        }
        assert!(matches!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { .. }));
    }

    #[test]
    fn a_run_in_ending_is_an_ended_empty_run() {
        let (mind, item) = worked(&[E, E]);
        mind.committed(vec![run("dying", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear, "open, it is not yet ended");
        assert!(matches!(breaker(&mind, &item, &[run_ref("dying")]).unwrap(), Breaker::Trip { .. }));
        let instance = Slug(INSTANCE.into());
        assert!(
            queued(&mind, &instance, &[])
                .unwrap()
                .iter()
                .all(|spec| *spec != item),
            "it claims the item"
        );
        assert!(
            queued(&mind, &instance, &[run_ref("dying")])
                .unwrap()
                .contains(&item),
            "an ending run claims nothing"
        );
    }

    #[test]
    fn the_newest_question_in_the_item_starts_a_fresh_count() {
        // Nine empty of the last fifty, then a question about the item is raised and answered.
        let (mind, item) = worked(&[E, E, N, E, E, N, E, E, N, E, E, N, E]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear);
        mind.committed(vec![question_in("fork", &item)]);
        mind.committed(vec![ruling("answer", Some((&id("question", "fork"), "A")))]);
        ended_run(&mind, &item, "g", 0, false);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear, "the tenth empty run, but nine of them are before the question");
        ended_run(&mind, &item, "g", 1, false);
        ended_run(&mind, &item, "g", 2, false);
        assert!(matches!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { .. }), "three empty runs after it");
    }

    #[test]
    fn a_withdrawn_run_never_ran_and_is_no_boundary() {
        let withdrawn_between = |ruling_after: bool| {
            let (mind, item) = worked(&[E]);
            mind.committed(vec![run("w", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
            mind.committed(vec![close(run_ref("w"), withdrawn())]);
            if ruling_after {
                mind.committed(vec![follow_up("after", &item)]);
            }
            ended_run(&mind, &item, "h", 1, false);
            ended_run(&mind, &item, "h", 2, false);
            breaker(&mind, &item, &[]).unwrap()
        };
        assert!(matches!(withdrawn_between(false), Breaker::Trip { .. }), "the withdrawn run is neither counted nor does it break the streak");
        assert_eq!(withdrawn_between(true), Breaker::Clear, "a document after the withdrawn run still lands in the first run's window");
    }

    #[test]
    fn a_runs_window_includes_its_own_batch_and_stops_before_the_next_runs() {
        let (mind, item) = worked(&[E, E]);
        // The third run opens with a ruling in its batch; it is still open.
        mind.committed(vec![run("h2", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), follow_up("with-run", &item)]);
        mind.committed(vec![close(run_ref("h2"), recorded())]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear, "the ruling in its own batch is in its window");

        let (mind, item) = worked(&[E, E, E]);
        mind.committed(vec![run("live", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), follow_up("with-next", &item)]);
        assert!(matches!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { .. }), "the next run's batch is not the previous run's window");
    }

    /// The adapter over the wire vocabulary, answered by a real mind.
    #[test]
    fn the_daemon_port_asks_and_admits_in_the_wire_vocabulary() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x")]);
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| Ok(mind.answer(request))));
        assert_eq!(port.instance().0, INSTANCE);
        assert_eq!(
            queued(&port, port.instance(), &[]).unwrap(),
            vec![spec_ref("x")]
        );
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

    #[test]
    fn runs_of_another_item_are_not_this_items_runs() {

        let mind = TestMind::seeded();
        mind.committed(vec![spec("x"), spec("y")]);
        history_as(&mind, &spec_ref("x"), "x", &[E, E]);
        history_as(&mind, &spec_ref("y"), "y", &[E, E, E]);
        assert_eq!(breaker(&mind, &spec_ref("x"), &[]).unwrap(), Breaker::Clear);
        assert!(matches!(breaker(&mind, &spec_ref("y"), &[]).unwrap(), Breaker::Trip { .. }));
    }

    #[test]
    fn the_oldest_counted_runs_own_batch_is_in_its_window() {
        let (mind, item) = worked(&[]);
        mind.committed(vec![run("h0", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), follow_up("with-oldest", &item)]);
        mind.committed(vec![close(run_ref("h0"), recorded())]);
        ended_run(&mind, &item, "h", 1, false);
        ended_run(&mind, &item, "h", 2, false);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear);
    }

    #[test]
    fn the_queue_is_the_unreported_unblocked_unclaimed_specs_oldest_first() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("a"), spec("b"), spec("c"), spec("d"), spec("e")]);
        let instance = Slug(INSTANCE.into());
        let all: Vec<PipelineRef> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|cut| spec_ref(cut))
            .collect();
        assert_eq!(queued(&mind, &instance, &[]).unwrap(), all);

        mind.committed(vec![report("b")]);
        mind.committed(vec![question_in("fork", &spec_ref("c"))]);
        mind.committed(vec![run(
            "held",
            RunTurn::SelfRun,
            RunOperator::Operator,
            &[spec_ref("d")],
        )]);
        assert_eq!(
            queued(&mind, &instance, &[]).unwrap(),
            vec![spec_ref("a"), spec_ref("e")]
        );

        mind.committed(vec![close(run_ref("held"), recorded())]);
        assert_eq!(
            queued(&mind, &instance, &[]).unwrap(),
            vec![spec_ref("a"), spec_ref("d"), spec_ref("e")],
            "a closed run claims nothing"
        );
        let other = Slug("another-instance".into());
        mind.committed(vec![run(
            "again",
            RunTurn::SelfRun,
            RunOperator::Operator,
            &[spec_ref("a")],
        )]);
        assert_eq!(
            queued(&mind, &other, &[]).unwrap().len(),
            3,
            "claims are read for the instance asked"
        );
    }

    #[test]
    fn a_trip_names_the_newest_ended_counted_run() {
        let (mind, item) = worked(&[E, E, E, E]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { run: run_ref("h3") });
        mind.committed(vec![run("live", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { run: run_ref("h3") }, "a run still open is not ended");
        assert_eq!(breaker(&mind, &item, &[run_ref("live")]).unwrap(), Breaker::Trip { run: run_ref("live") }, "a run in ending is");
    }

    /// Two items claimed together by every run of a history; the last one ended empty.
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
            assert_eq!(
                queued(&mind, &Slug(INSTANCE.into()), &[]).unwrap(),
                Vec::<PipelineRef>::new(),
                "queue reads the same hold"
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
        let retired: Vec<bool> = holds(&mind)
            .unwrap()
            .iter()
            .map(|hold| hold.retired)
            .collect();
        assert_eq!(retired, [true]);
        mind.committed(vec![close(a.clone(), withdrawn())]);
        assert_eq!(
            on_hold(&mind).unwrap().items(),
            std::slice::from_ref(&b),
            "a resolved claim drops out"
        );
        mind.committed(vec![close(b, withdrawn())]);
        assert!(
            on_hold(&mind).unwrap().items().is_empty(),
            "all resolved, nothing is held"
        );
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
    fn answering_a_question_in_the_run_gives_every_claim_of_it_a_fresh_count() {
        let (mind, a, b) = bundle();
        assert_eq!(breaker(&mind, &a, &[]).unwrap(), Breaker::Trip { run: run_ref("f2") });
        assert_eq!(breaker(&mind, &b, &[]).unwrap(), Breaker::Trip { run: run_ref("f2") });
        mind.committed(vec![question_in("fork", &run_ref("f2"))]);
        mind.committed(vec![ruling("answer", Some((&id("question", "fork"), "A")))]);
        for item in [&a, &b] {
            assert_eq!(breaker(&mind, item, &[]).unwrap(), Breaker::Clear, "counts only runs after f2");
        }
        for n in 0..3 {
            mind.committed(vec![run(&format!("g{n}"), RunTurn::SelfRun, RunOperator::Mind, &[a.clone(), b.clone()])]);
            mind.committed(vec![close(run_ref(&format!("g{n}")), recorded())]);
            let expected = if n == 2 { Breaker::Trip { run: run_ref("g2") } } else { Breaker::Clear };
            assert_eq!(breaker(&mind, &b, &[]).unwrap(), expected, "b has no question of its own and trips after three fresh empty runs, never before");
        }
    }

    #[test]
    fn a_question_in_a_run_that_did_not_claim_the_item_is_no_boundary() {
        let (mind, item) = worked(&[E, E, E]);
        mind.committed(vec![spec("y")]);
        mind.committed(vec![run("other", RunTurn::SelfRun, RunOperator::Mind, &[spec_ref("y")])]);
        mind.committed(vec![question_in("fork", &run_ref("other"))]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip { run: run_ref("h2") }, "the question holds y's run, not x's count");
    }

    /// A daemon that is not there: the failure names no input, endpoint or
    /// instance.
    #[test]
    fn an_unreachable_mind_names_no_input() {
        let canary = "canary-instance-7f3a91";
        let endpoint: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let port = HuginnMind::new(HuginnClient::new(
            endpoint,
            Slug(canary.into()),
            Duration::from_millis(300),
        ));
        let error = queued(&port, &Slug(canary.into()), &[]).unwrap_err();
        let shown = format!("{error:#}");
        assert!(shown.contains("unavailable"), "{shown}");
        assert!(!shown.contains(canary) && !shown.contains("127.0.0.1"), "{shown}");
    }
}
