//! The Persona's view of open work, and the guard on it: what may be worked
//! (`queue`) and whether an item has stopped paying for its runs (`breaker`).
//!
//! Nothing here keeps state. Both answers are derived from the mind on every
//! call, through `MindPort`: the mind is the only store, so a breaker that
//! needed a file of its own would be a second owner of what the runs already
//! record (target invariant run-is-the-grant).

use anyhow::{Result, anyhow, bail};
use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef, Selection};
use eureka_pipeline::{PipelineDocument, PipelineKind, PipelineRef, ResolutionOutcome, Slug};
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

/// Every page of a selection, in the mind's order, handed to `visit` until it
/// answers `false`. The one paging loop.
fn scan(mind: &dyn MindPort, selection: &Selection, mut visit: impl FnMut(PipelineDocumentSummary) -> bool) -> Result<()> {
    let mut asking = selection.clone();
    loop {
        let page = mind.query(&asking)?;
        let PipelinePageItems::Headers(items) = page.items else { bail!("the mind answered documents to a header selection") };
        for item in items {
            if !visit(item) {
                return Ok(());
            }
        }
        match page.next {
            Some(cursor) => asking.cursor = Some(cursor),
            None => return Ok(()),
        }
    }
}

pub(crate) fn headers(mind: &dyn MindPort, selection: &Selection) -> Result<Vec<PipelineDocumentSummary>> {
    let mut all = Vec::new();
    scan(mind, selection, |header| {
        all.push(header);
        true
    })?;
    Ok(all)
}

/// The claims of the runs of `instance` that are still in force: the work
/// someone holds.
pub(crate) fn live_claims(mind: &dyn MindPort, instance: &Slug) -> Result<Vec<PipelineRef>> {
    let runs = in_force(any_of(of_kind(PipelineKind::Run), "root", &[&instance.0]));
    let mut claims = Vec::new();
    for header in headers(mind, &runs)? {
        if let PipelineFacts::Run { claims: held, .. } = header.facts {
            claims.extend(held);
        }
    }
    Ok(claims)
}

/// What may be worked, oldest first: the cut specs in force that no report
/// answers, that no open question blocks, and that no live run claims.
pub fn queue(mind: &dyn MindPort, instance: &Slug) -> Result<Vec<PipelineRef>> {
    let unreported = Selection { cited: Some(Incoming { role: "cut_spec".into(), exists: false }), ..in_force(of_kind(PipelineKind::CutSpec)) };
    let mut blocked: Vec<PipelineRef> = Vec::new();
    for header in headers(mind, &in_force(of_kind(PipelineKind::Question)))? {
        if let PipelineFacts::Question { raised_in: Some(item), .. } = header.facts {
            blocked.push(item);
        }
    }
    let claimed = live_claims(mind, instance)?;
    Ok(headers(mind, &unreported)?
        .into_iter()
        .map(|header| header.id)
        .filter(|spec| !blocked.contains(spec) && !claimed.contains(spec))
        .collect())
}

/// Whether an item may still be launched on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breaker {
    Clear,
    Trip,
}

/// The repetition breaker, derived from the item's Self runs and from what
/// else the mind admitted.
///
/// A run is a Self run of the item that claims it. A run that was withdrawn
/// never ran and is no run here. An ended run is empty when no document other
/// than a run of this item, or the resolution that closes one, was admitted
/// from its own admission up to the next run's (or to now). Trip when the last
/// `CONSECUTIVE` ended runs are empty, or `EMPTY_SHARE` of the last `WINDOW`.
/// A document admitted after the last run, such as a ruling, makes that run
/// non-empty, so reopening an item resets it.
pub fn breaker(mind: &dyn MindPort, item: &PipelineRef) -> Result<Breaker> {
    let claiming = Citation { target: RecordRef::new(item.kind.type_id(), item.id.0.clone()), role: Some("claims".into()) };
    let runs = headers(mind, &Selection { cites: Some(claiming), ..any_of(of_kind(PipelineKind::Run), "turn", &["SelfRun"]) })?;
    let withdrawn = |run: &PipelineDocumentSummary| {
        matches!(run.status, PipelineStatusSummary::Resolved { outcome: ResolutionOutcome::Withdrawn { .. }, .. })
    };
    // (admission ordinal, ended) of each run that ran, oldest first.
    let ran: Vec<(u64, bool)> = runs
        .iter()
        .filter(|run| !withdrawn(run))
        .map(|run| (run.admission.ordinal, matches!(run.status, PipelineStatusSummary::Resolved { .. })))
        .collect();
    let ended: Vec<usize> = (0..ran.len()).filter(|index| ran[*index].1).collect();
    let considered: Vec<usize> = ended.iter().rev().take(WINDOW).copied().collect();
    let Some(oldest) = considered.last().map(|index| ran[*index].0) else { return Ok(Breaker::Clear) };

    let run_ids: Vec<&PipelineRef> = runs.iter().map(|run| &run.id).collect();
    let mut admitted = Vec::new();
    scan(mind, &Selection { descending: true, ..Selection::default() }, |header| {
        if header.admission.ordinal < oldest {
            return false;
        }
        let closes_a_run = matches!(&header.facts, PipelineFacts::Resolution { subject, .. } if run_ids.contains(&subject));
        if !run_ids.contains(&&header.id) && !closes_a_run {
            admitted.push(header.admission.ordinal);
        }
        true
    })?;

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
        breaker(&mind, &item).unwrap() == Breaker::Trip
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
    fn a_withdrawn_run_never_ran_and_is_no_boundary() {
        let withdrawn_between = |ruling_after: bool| {
            let (mind, item) = worked(&[E]);
            mind.committed(vec![run("w", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item))]);
            mind.committed(vec![close(run_ref("w"), withdrawn())]);
            if ruling_after {
                mind.committed(vec![ruling("after", None)]);
            }
            ended_run(&mind, &item, "h", 1, false);
            ended_run(&mind, &item, "h", 2, false);
            breaker(&mind, &item).unwrap()
        };
        assert_eq!(withdrawn_between(false), Breaker::Trip, "the withdrawn run is neither counted nor does it break the streak");
        assert_eq!(withdrawn_between(true), Breaker::Clear, "a document after the withdrawn run still lands in the first run's window");
    }

    #[test]
    fn a_runs_window_includes_its_own_batch_and_stops_before_the_next_runs() {
        let (mind, item) = worked(&[E, E]);
        // The third run opens with a ruling in its batch; it is still open.
        mind.committed(vec![run("h2", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), ruling("with-run", None)]);
        mind.committed(vec![close(run_ref("h2"), recorded())]);
        assert_eq!(breaker(&mind, &item).unwrap(), Breaker::Clear, "the ruling in its own batch is in its window");

        let (mind, item) = worked(&[E, E, E]);
        mind.committed(vec![run("live", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), ruling("with-next", None)]);
        assert_eq!(breaker(&mind, &item).unwrap(), Breaker::Trip, "the next run's batch is not the previous run's window");
    }

    #[test]
    /// The adapter over the wire vocabulary, answered by a real mind.
    #[test]
    fn the_daemon_port_asks_and_admits_in_the_wire_vocabulary() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("x")]);
        let port = HuginnMind::over(Slug(INSTANCE.into()), Box::new(|request| Ok(mind.answer(request))));
        assert_eq!(port.instance().0, INSTANCE);
        assert_eq!(queue(&port, port.instance()).unwrap(), vec![spec_ref("x")]);
        let outcome = port.admit("agent-x", "session-y", vec![run("p", RunTurn::PersonaTurn, RunOperator::Mind, &[])]).unwrap();
        assert!(matches!(outcome, PipelineAdmissionOutcome::Committed { .. }), "{outcome:?}");
        let facts = mind.view(PipelineKind::Run, &run_ref("p").id.0).unwrap().admission.provenance;
        assert_eq!((facts.faculty, facts.agent.0.as_str(), facts.session.0.as_str(), facts.tool.0.as_str()), (Faculty::SelfFaculty, "agent-x", "session-y", "mind-body"));
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
        assert_eq!(breaker(&mind, &spec_ref("x")).unwrap(), Breaker::Clear);
        assert_eq!(breaker(&mind, &spec_ref("y")).unwrap(), Breaker::Trip);
    }

    #[test]
    fn the_oldest_counted_runs_own_batch_is_in_its_window() {
        let (mind, item) = worked(&[]);
        mind.committed(vec![run("h0", RunTurn::SelfRun, RunOperator::Mind, std::slice::from_ref(&item)), ruling("with-oldest", None)]);
        mind.committed(vec![close(run_ref("h0"), recorded())]);
        ended_run(&mind, &item, "h", 1, false);
        ended_run(&mind, &item, "h", 2, false);
        assert_eq!(breaker(&mind, &item).unwrap(), Breaker::Clear);
    }

    #[test]
    fn the_queue_is_the_unreported_unblocked_unclaimed_specs_oldest_first() {
        let mind = TestMind::seeded();
        mind.committed(vec![spec("a"), spec("b"), spec("c"), spec("d"), spec("e")]);
        let instance = Slug(INSTANCE.into());
        let all: Vec<PipelineRef> = ["a", "b", "c", "d", "e"].iter().map(|cut| spec_ref(cut)).collect();
        assert_eq!(queue(&mind, &instance).unwrap(), all);

        mind.committed(vec![report("b")]);
        mind.committed(vec![question_in("fork", &spec_ref("c"))]);
        mind.committed(vec![run("held", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("d")])]);
        assert_eq!(queue(&mind, &instance).unwrap(), vec![spec_ref("a"), spec_ref("e")]);

        mind.committed(vec![close(run_ref("held"), recorded())]);
        assert_eq!(queue(&mind, &instance).unwrap(), vec![spec_ref("a"), spec_ref("d"), spec_ref("e")], "a closed run claims nothing");
        let other = Slug("another-instance".into());
        mind.committed(vec![run("again", RunTurn::SelfRun, RunOperator::Operator, &[spec_ref("a")])]);
        assert_eq!(queue(&mind, &other).unwrap().len(), 3, "claims are read for the instance asked");
    }

    /// A daemon that is not there: the failure names no input, endpoint or
    /// instance.
    #[test]
    fn an_unreachable_mind_names_no_input() {
        let canary = "canary-instance-7f3a91";
        let endpoint: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let port = HuginnMind::new(HuginnClient::new(endpoint, Slug(canary.into()), Duration::from_millis(300)));
        let error = queue(&port, &Slug(canary.into())).unwrap_err();
        let shown = format!("{error:#}");
        assert!(shown.contains("unavailable"), "{shown}");
        assert!(!shown.contains(canary) && !shown.contains("127.0.0.1"), "{shown}");
    }
}
