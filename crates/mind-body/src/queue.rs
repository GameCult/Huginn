//! The Persona's view of open work, and the guard on it: what may be worked
//! (`queue`) and whether an item has stopped paying for its runs (`breaker`).
//!
//! Nothing here keeps state. Both answers are derived from the mind on every
//! call, through `MindPort`: the mind is the only store, so a breaker that
//! needed a file of its own would be a second owner of what the runs already
//! record (target invariant run-is-the-grant).

use anyhow::{Result, anyhow, bail};
use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef, Selection};
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, RunOperator, RunTurn, Slug};
use eureka_state::{ClientError, HuginnClient};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, PipelineAdmissionBatch, PipelineAdmissionOutcome, PipelineDocumentSummary, PipelineFacts,
    PipelinePageItems, PipelineProvenance, PipelineSelectionPage, PipelineStatusSummary,
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
}

impl MindPort for HuginnMind<'_> {
    fn instance(&self) -> &Slug {
        &self.instance
    }

    fn query(&self, selection: &Selection) -> Result<PipelineSelectionPage> {
        let request = HuginnMindRequest::Query { instance: self.instance().clone(), selection: selection.clone(), semantic: None };
        match (self.call)(request).map_err(|error| anyhow!("the mind could not be asked: {}", client_error_code(&error)))? {
            HuginnMindResponse::Query(page) => Ok(page),
            HuginnMindResponse::Refused(_) => bail!("the mind refused the query"),
            _ => bail!("the mind answered a query with something else"),
        }
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

/// The documents that cite `target`, under `role` or any.
pub(crate) fn citing(target: &PipelineRef, role: Option<&str>) -> Citation {
    Citation { target: RecordRef::new(target.kind.type_id(), target.id.0.clone()), role: role.map(Into::into) }
}

pub(crate) fn of_kind(kind: PipelineKind) -> Selection {
    Selection { schemas: Some(vec![kind.type_id().to_string()]), ..Selection::default() }
}

pub(crate) fn any_of(mut selection: Selection, index: &str, values: &[&str]) -> Selection {
    selection.fields.get_or_insert_with(Vec::new).push(FieldPredicate {
        index: index.into(),
        op: "any_of".into(),
        values: Some(values.iter().map(|value| value.to_string()).collect()),
        number: None,
    });
    selection
}

pub(crate) fn in_force(selection: Selection) -> Selection {
    any_of(selection, "in_force", &["true"])
}

/// Every page of a selection, in the mind's order. The one paging loop.
pub(crate) fn headers(mind: &dyn MindPort, selection: &Selection) -> Result<Vec<PipelineDocumentSummary>> {
    let mut all = Vec::new();
    let mut asking = selection.clone();
    loop {
        let page = mind.query(&asking)?;
        let PipelinePageItems::Headers(items) = page.items else { bail!("the mind answered documents to a header selection") };
        all.extend(items);
        match page.next {
            Some(cursor) => asking.cursor = Some(cursor),
            None => return Ok(all),
        }
    }
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

/// What may be worked, oldest first: the cut specs in force that no report
/// answers, that no open question blocks, and that no live run claims. A run in
/// `ending` is one the caller closes in the same admission that opens the next,
/// so it claims nothing here.
pub fn queue(mind: &dyn MindPort, instance: &Slug, ending: &[PipelineRef]) -> Result<Vec<PipelineRef>> {
    let unreported = Selection { cited: Some(Incoming { role: "cut_spec".into(), exists: false }), ..in_force(of_kind(PipelineKind::CutSpec)) };
    let mut blocked: Vec<PipelineRef> = Vec::new();
    for header in headers(mind, &in_force(of_kind(PipelineKind::Question)))? {
        if let PipelineFacts::Question { raised_in: Some(item), .. } = header.facts {
            blocked.push(item);
        }
    }
    let claimed = live_claims(mind, instance, ending)?;
    Ok(headers(mind, &unreported)?.into_iter().map(|header| header.id).filter(|spec| !blocked.contains(spec) && !claimed.contains(spec)).collect())
}

/// Whether an item may still be launched on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breaker {
    Clear,
    Trip,
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
/// run's (or to now). Trip when the last `CONSECUTIVE` ended runs are empty, or
/// `EMPTY_SHARE` of the last `WINDOW`. What else the mind admitted, such as a
/// Persona turn's run or an unrelated ruling, cites nothing here and counts for
/// nothing.
pub fn breaker(mind: &dyn MindPort, item: &PipelineRef, ending: &[PipelineRef]) -> Result<Breaker> {
    let mut set = headers(mind, &Selection { cites: Some(citing(item, None)), ..Selection::default() })?;
    set.sort_by_key(|header| header.admission.ordinal);
    let boundary = set
        .iter()
        .filter(|header| matches!(&header.facts, PipelineFacts::Question { raised_in: Some(raised), .. } if raised == item))
        .map(|header| header.admission.ordinal)
        .max();
    let after = |header: &&PipelineDocumentSummary| boundary.is_none_or(|boundary| header.admission.ordinal > boundary);
    let withdrawn =
        |run: &PipelineDocumentSummary| matches!(run.status, PipelineStatusSummary::Resolved { outcome: ResolutionOutcome::Withdrawn { .. }, .. });
    // (admission ordinal, ended) of each counted run, oldest first.
    let ran: Vec<(u64, bool)> = set
        .iter()
        .filter(after)
        .filter(|header| {
            matches!(header.facts, PipelineFacts::Run { turn: RunTurn::SelfRun, operated_by: RunOperator::Mind, .. }) && !withdrawn(header)
        })
        .map(|run| (run.admission.ordinal, matches!(run.status, PipelineStatusSummary::Resolved { .. }) || ending.contains(&run.id)))
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
    Ok(if consecutive || share { Breaker::Trip } else { Breaker::Clear })
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use eureka_pipeline::{RunOperator, RunTurn};

    use super::*;
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
        breaker(&mind, &item, &[]).unwrap() == Breaker::Trip
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
        assert_eq!(after_a_run_by(RunOperator::Mind), Breaker::Trip, "hers, the third empty one");
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
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip);
    }

    #[test]
    fn a_run_in_ending_is_an_ended_empty_run() {
        let (mind, item) = worked(&[E, E]);
        mind.committed(vec![run("dying", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Clear, "open, it is not yet ended");
        assert_eq!(breaker(&mind, &item, &[run_ref("dying")]).unwrap(), Breaker::Trip);
        let instance = Slug(INSTANCE.into());
        assert!(queue(&mind, &instance, &[]).unwrap().iter().all(|spec| *spec != item), "it claims the item");
        assert!(queue(&mind, &instance, &[run_ref("dying")]).unwrap().contains(&item), "an ending run claims nothing");
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
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip, "three empty runs after it");
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
        assert_eq!(withdrawn_between(false), Breaker::Trip, "the withdrawn run is neither counted nor does it break the streak");
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
        assert_eq!(breaker(&mind, &item, &[]).unwrap(), Breaker::Trip, "the next run's batch is not the previous run's window");
    }

    /// The adapter over the wire vocabulary, answered by a real mind.
    #[test]
    fn the_daemon_port_asks_and_admits_in_the_wire_vocabulary() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x")]);
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| Ok(mind.answer(request))));
        assert_eq!(port.instance().0, INSTANCE);
        assert_eq!(queue(&port, port.instance(), &[]).unwrap(), vec![spec_ref("x")]);
        let outcome = port.admit("agent-x", "session-y", vec![run("p", RunTurn::PersonaTurn, RunOperator::Mind, &[])]).unwrap();
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
        let facts = mind.view(PipelineKind::Run, &run_ref("p").id.0).unwrap().admission.provenance;
        assert_eq!(
            (facts.faculty, facts.agent.0.as_str(), facts.session.0.as_str(), facts.tool.0.as_str()),
            (Faculty::SelfFaculty, "agent-x", "session-y", "mind-body")
        );
        let refused = port.query(&any_of(of_kind(PipelineKind::Ruling), "turn", &["canary-turn-5d2e"])).unwrap_err();
        assert!(format!("{refused:#}").contains("refused the query"));
        assert!(!format!("{refused:#}").contains("canary"), "{refused:#}");
    }

    #[test]
    fn runs_of_another_item_are_not_this_items_runs() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x"), spec("y")]);
        history_as(&mind, &spec_ref("x"), "x", &[E, E]);
        history_as(&mind, &spec_ref("y"), "y", &[E, E, E]);
        assert_eq!(breaker(&mind, &spec_ref("x"), &[]).unwrap(), Breaker::Clear);
        assert_eq!(breaker(&mind, &spec_ref("y"), &[]).unwrap(), Breaker::Trip);
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
        let all: Vec<PipelineRef> = ["a", "b", "c", "d", "e"].iter().map(|cut| spec_ref(cut)).collect();
        assert_eq!(queue(&mind, &instance, &[]).unwrap(), all);

        mind.committed(vec![report("b")]);
        mind.committed(vec![question_in("fork", &spec_ref("c"))]);
        mind.committed(vec![run("held", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("d")])]);
        assert_eq!(queue(&mind, &instance, &[]).unwrap(), vec![spec_ref("a"), spec_ref("e")]);

        mind.committed(vec![close(run_ref("held"), recorded())]);
        assert_eq!(queue(&mind, &instance, &[]).unwrap(), vec![spec_ref("a"), spec_ref("d"), spec_ref("e")], "a closed run claims nothing");
        let other = Slug("another-instance".into());
        mind.committed(vec![run("again", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("a")])]);
        assert_eq!(queue(&mind, &other, &[]).unwrap().len(), 3, "claims are read for the instance asked");
    }

    /// A daemon that is not there: the failure names no input, endpoint or
    /// instance.
    #[test]
    fn an_unreachable_mind_names_no_input() {
        let canary = "canary-instance-7f3a91";
        let endpoint: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let port = HuginnMind::new(HuginnClient::new(endpoint, Slug(canary.into()), Duration::from_millis(300)));
        let error = queue(&port, &Slug(canary.into()), &[]).unwrap_err();
        let shown = format!("{error:#}");
        assert!(shown.contains("unavailable"), "{shown}");
        assert!(!shown.contains(canary) && !shown.contains("127.0.0.1"), "{shown}");
    }
}
