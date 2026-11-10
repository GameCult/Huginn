//! The fork desk's answers: the Mind's counting door.
//!
//! `Mind::answer_desk` is the one owner of whether a desk answer counts, its
//! sequence, its stored outcome and the stamping of `answerer` and `authority`
//! (binding spec /opt/commons/handoff/fork-desk-answer-binding-20261110.md,
//! 1.3 steps 3-8 and R1, R2, R4). It builds one `desk_answer` and commits it
//! through `admit_steps` with `Door::Desk`, the same commit primitive every
//! other write uses; no other door may write the kind.
//!
//! The decider list is an input (`DeskStamp`, from the caller); the Mind never
//! reads one. The request types carry no `answerer` and no `authority`.
//! `answered_at` is the client's claim, displayed and never compared: order is
//! the per-question sequence, which is the receiver's order.

use chrono::{DateTime, Utc};
use eureka_pipeline::{
    DeskAuthority, DeskOutcome, DeskRefusal, Label, Para, PipelineDeskAnswer, PipelineDocument, PipelineKind, PipelineQuestion,
    PipelineRefusal, PipelineResolution, ResolutionOutcome, Sha256Hex, Short, Slug,
};

use crate::admission::{Door, PipelineAdmissionOutcome, prepare};
use crate::docs::{Docs, kind_of_id};
use crate::mind::Mind;
use crate::receipt::{Faculty, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::rows::root_and_local;
use crate::store::MindStore;

/// What the desk's receiver sends the Mind. `answered_at` is the person's
/// claim, RFC 3339, kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskAnswerRequest {
    pub fork: Short,
    pub options_hash: Sha256Hex,
    pub body: DeskBody,
    pub answered_at: Short,
}

/// An answer, or the retraction of one: a sum, so a retraction carrying an
/// answer, or naming no target, cannot be written down (R1 by construction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskBody {
    Answer { choice: Option<Label>, note: Option<Para> },
    Retraction { retracts: Short },
}

/// Who answered and in what standing, as the verified caller states it. The
/// Mind copies both into the record and decides nothing from the caller's
/// word except that `authority: None` is no decider (rule 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskStamp {
    pub answerer: Short,
    pub authority: Option<DeskAuthority>,
}

/// `Stored`: the answer id (the record key), its outcome and the commit's receipt.
/// `Refused`: nothing was stored (a shape fault, an unknown fork, a moved campaign,
/// or a commit fault).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskAnswerOutcome {
    Stored { answer: Short, outcome: DeskOutcome, receipt_id: String },
    Refused(MindRefusal),
}

/// The hash of a fork as the desk shows it, which an answer must quote (rule 4).
///
/// PENDING: the bytes are Libby's CultNet card contract (CultLib PR #72) and are
/// not implemented in this seam yet. Until they are, the seam fails closed: no
/// request can quote the value returned here, so every answer is stored
/// `Refused { StaleFork }`. Tests are served by `fixtures::fixture_options_hash`.
pub fn options_hash(question: &PipelineQuestion) -> Sha256Hex {
    #[cfg(test)]
    return fixtures::fixture_options_hash(question);
    #[cfg(not(test))]
    {
        let _ = question;
        Sha256Hex("0".repeat(64))
    }
}

/// Every desk answer of one fork, with its key.
fn answers_of<'a>(docs: &'a Docs<'_>, fork: &'a str) -> impl Iterator<Item = (&'a str, &'a PipelineDeskAnswer)> + 'a {
    docs.of_kind(PipelineKind::DeskAnswer).filter_map(move |(key, document)| match document {
        PipelineDocument::DeskAnswer(answer) if answer.fork.0 == fork => Some((key, answer)),
        _ => None,
    })
}

fn is_retracted(docs: &Docs<'_>, fork: &str, key: &str) -> bool {
    answers_of(docs, fork).any(|(_, other)| matches!(&other.outcome, DeskOutcome::Retraction { retracts } if retracts.0 == key))
}

/// The answer that counts for `fork` now: the lowest-sequence answer whose stored
/// outcome is Counted or Revision and that no Retraction names (R4). Derived on
/// every call and never stored; retracting the counted answer promotes the next,
/// retracting another changes nothing. The one place "counts" is decided.
pub(crate) fn counted_answer(docs: &Docs<'_>, fork: &str) -> Option<String> {
    answers_of(docs, fork)
        .filter(|(key, answer)| matches!(answer.outcome, DeskOutcome::Counted | DeskOutcome::Revision { .. }) && !is_retracted(docs, fork, key))
        .min_by_key(|(_, answer)| answer.sequence)
        .map(|(key, _)| key.to_string())
}

/// Rules 4-7 and R1 in the binding spec's order; the first failure is the code.
fn refusal_of(docs: &Docs<'_>, question: &PipelineQuestion, stamp: &DeskStamp, request: &DeskAnswerRequest) -> Option<DeskRefusal> {
    let fork = request.fork.0.as_str();
    if request.options_hash != options_hash(question) {
        return Some(DeskRefusal::StaleFork);
    }
    if stamp.authority.is_none() {
        return Some(DeskRefusal::NotADecider);
    }
    if !docs.in_force(PipelineKind::Question, fork) {
        return Some(DeskRefusal::ForkClosed);
    }
    match &request.body {
        DeskBody::Answer { choice: Some(choice), .. } if !question.options.iter().any(|option| option.label == *choice) => {
            Some(DeskRefusal::ChoiceInvalid)
        }
        DeskBody::Answer { .. } => None,
        DeskBody::Retraction { retracts } => {
            let target = answers_of(docs, fork).find(|(key, _)| *key == retracts.0);
            if !target.is_some_and(|(_, answer)| matches!(answer.outcome, DeskOutcome::Counted | DeskOutcome::Revision { .. })) {
                return Some(DeskRefusal::BadRetraction);
            }
            is_retracted(docs, fork, &retracts.0).then_some(DeskRefusal::AlreadyRetracted)
        }
    }
}

fn shape_fault(refusal: PipelineRefusal) -> MindRefusal {
    let field = match &refusal {
        PipelineRefusal::FieldBound { field, .. } | PipelineRefusal::InvalidFormat { field, .. } | PipelineRefusal::DuplicateRepo { field, .. } => field.clone(),
        PipelineRefusal::InvalidIdentity { .. } | PipelineRefusal::ForeignStore { .. } => "request".into(),
    };
    MindRefusal::DeskRequestInvalid { field }
}

impl<S: MindStore> Mind<S> {
    /// Decides one desk answer and, when the request is well formed and names a
    /// held fork, stores it: counted, a revision, a retraction, or refused with its
    /// code. See the module header for the order and the stored/not-stored line.
    pub fn answer_desk(&mut self, stamp: DeskStamp, request: DeskAnswerRequest, now: DateTime<Utc>) -> DeskAnswerOutcome {
        match self.desk_steps(stamp, request, now) {
            Ok(outcome) => outcome,
            Err(refusal) => DeskAnswerOutcome::Refused(refusal),
        }
    }

    fn desk_steps(&mut self, stamp: DeskStamp, request: DeskAnswerRequest, now: DateTime<Utc>) -> Result<DeskAnswerOutcome, MindRefusal> {
        let invalid = |field: &str| MindRefusal::DeskRequestInvalid { field: field.into() };
        // Shape. The fork must read as a question id; the probe document carries every other bounded field.
        if kind_of_id(&request.fork.0) != Some(PipelineKind::Question) {
            return Err(invalid("fork"));
        }
        let root = root_and_local(&request.fork.0).0.to_string();
        if DateTime::parse_from_rfc3339(&request.answered_at.0).is_err() {
            return Err(invalid("answered_at"));
        }
        let (choice, note, shape_outcome) = match &request.body {
            DeskBody::Answer { choice, note } => {
                if choice.is_none() && note.is_none() {
                    return Err(invalid("body"));
                }
                (choice.clone(), note.clone(), DeskOutcome::Counted)
            }
            DeskBody::Retraction { retracts } => (None, None, DeskOutcome::Retraction { retracts: retracts.clone() }),
        };
        let answer = |sequence: u32, outcome: DeskOutcome| PipelineDeskAnswer {
            campaign: Slug(root.clone()),
            fork: request.fork.clone(),
            sequence,
            options_hash: request.options_hash.clone(),
            choice: choice.clone(),
            note: note.clone(),
            answered_at: request.answered_at.clone(),
            answerer: stamp.answerer.clone(),
            authority: stamp.authority,
            outcome,
        };
        let probe = PipelineDocument::DeskAnswer(answer(1, shape_outcome));
        probe.validate().map_err(shape_fault)?;
        eureka_pipeline::pipeline_key(&probe).map_err(shape_fault)?;

        // Rules 3 and the moved campaign: nothing is stored.
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let fork = request.fork.0.as_str();
        let Some(PipelineDocument::Question(question)) = docs.find(PipelineKind::Question, fork) else {
            return Err(MindRefusal::MissingReference { kind: PipelineKind::Question, id: fork.into() });
        };
        if let Some((moved, PipelineResolution { outcome: ResolutionOutcome::Moved { to }, .. })) = docs.moved_campaign(&root) {
            return Err(MindRefusal::CampaignMoved { campaign: root_and_local(moved).0.into(), to: to.0.clone() });
        }

        // Rules 4-7, R1: stored, refused, with the first failing code.
        let outcome = match refusal_of(&docs, question, &stamp, &request) {
            Some(code) => DeskOutcome::Refused { code },
            None => match &request.body {
                DeskBody::Retraction { retracts } => DeskOutcome::Retraction { retracts: retracts.clone() },
                DeskBody::Answer { .. } => match counted_answer(&docs, fork) {
                    None => DeskOutcome::Counted,
                    Some(counted) => DeskOutcome::Revision { of: Short(counted) },
                },
            },
        };
        let sequence = answers_of(&docs, fork).map(|(_, held)| held.sequence).max().unwrap_or(0).checked_add(1).ok_or_else(|| invalid("sequence"))?;
        let document = PipelineDocument::DeskAnswer(answer(sequence, outcome.clone()));
        let envelope = prepare(&document, self.cache())?;
        let provenance = PipelineProvenance {
            faculty: Faculty::Operator,
            agent: stamp.answerer.clone(),
            session: "desk".into(),
            tool: "huginn-desk".into(),
            imported_from: None,
        };
        let instance = self.instance().clone();
        match self.admit_steps(Door::Desk, &instance, provenance, vec![envelope], now)? {
            PipelineAdmissionOutcome::Committed { receipt_id, writes, .. } => {
                let answer = writes.into_iter().next().map(|written| written.id).ok_or_else(|| MindRefusal::Unavailable { detail: "a desk answer committed no write".into() })?;
                Ok(DeskAnswerOutcome::Stored { answer, outcome, receipt_id })
            }
            PipelineAdmissionOutcome::Refused(refusal) => Err(refusal),
            PipelineAdmissionOutcome::AlreadyAdmitted { .. } | PipelineAdmissionOutcome::Conflict { .. } => {
                Err(MindRefusal::Unavailable { detail: "a desk answer raced another write; ask again".into() })
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    /// A stand-in for `options_hash` served to tests only: a deterministic function of every
    /// member the binding spec names, order-sensitive, NOT SHA-256 and NOT Libby's bytes.
    /// It lets the counting rules be tested while the real bytes are pending.
    pub(crate) fn fixture_options_hash(question: &PipelineQuestion) -> Sha256Hex {
        let text = format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            question.title.0,
            question.question.0,
            question.options.iter().map(|option| (&option.label.0, &option.text.0)).collect::<Vec<_>>(),
            question.recommended.0,
            question.depends.iter().map(|line| &line.0).collect::<Vec<_>>()
        );
        // Four rounds of FNV-1a with distinct seeds make 64 hex digits.
        let digits: String = (0u64..4)
            .map(|round| {
                let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ round.wrapping_mul(0x9e37_79b9_7f4a_7c15);
                for byte in text.bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
                }
                format!("{hash:016x}")
            })
            .collect();
        Sha256Hex(digits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::store::test_stores::MemoryStore;
    use PipelineDocument as D;
    use PipelineKind as K;

    const FORK: &str = "eureka-state:question:Q1";
    const FORK2: &str = "eureka-state:question:Q2";

    fn question_with(label: &str, options: &[&str], recommended: &str) -> PipelineQuestion {
        let D::Question(question) = question(label, options, recommended) else { unreachable!() };
        question
    }

    fn desk() -> Mind<MemoryStore> {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(1), question_n(2)]));
        mind
    }

    fn hash(label: &str) -> Sha256Hex {
        options_hash(&question_with(label, &["A", "B"], "A"))
    }

    fn decider(who: &str) -> DeskStamp {
        DeskStamp { answerer: s(who), authority: Some(DeskAuthority::Decider) }
    }

    fn pick(fork: &str, label: &str, choice: &str, answered_at: &str) -> DeskAnswerRequest {
        DeskAnswerRequest {
            fork: s(fork),
            options_hash: hash(label),
            body: DeskBody::Answer { choice: Some(l(choice)), note: None },
            answered_at: s(answered_at),
        }
    }

    fn retract(fork: &str, label: &str, target: &str) -> DeskAnswerRequest {
        DeskAnswerRequest {
            fork: s(fork),
            options_hash: hash(label),
            body: DeskBody::Retraction { retracts: s(target) },
            answered_at: s("2026-11-10T12:00:00Z"),
        }
    }

    fn key(label: &str, n: u32) -> String {
        format!("eureka-state:desk_answer:{label}.n{n}")
    }

    fn stored(outcome: DeskAnswerOutcome) -> (String, DeskOutcome) {
        match outcome {
            DeskAnswerOutcome::Stored { answer, outcome, .. } => (answer.0, outcome),
            other => panic!("expected a stored answer, got {other:?}"),
        }
    }

    fn counted(mind: &Mind<MemoryStore>, fork: &str) -> Option<String> {
        let image = mind.image().unwrap();
        counted_answer(&Docs::at(image, image.head), fork)
    }

    fn held(mind: &Mind<MemoryStore>, answer: &str) -> PipelineDeskAnswer {
        let image = mind.image().unwrap();
        match Docs::at(image, image.head).find(K::DeskAnswer, answer) {
            Some(D::DeskAnswer(held)) => held.clone(),
            other => panic!("no desk answer {answer}: {other:?}"),
        }
    }

    fn head(mind: &Mind<MemoryStore>) -> u64 {
        mind.image().unwrap().head
    }

    fn ask(mind: &mut Mind<MemoryStore>, stamp: &DeskStamp, request: DeskAnswerRequest) -> DeskAnswerOutcome {
        mind.answer_desk(stamp.clone(), request, now())
    }

    #[test]
    fn a_first_decider_answer_counts_and_a_later_one_is_a_revision() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        assert_eq!(stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at))), (key("Q1", 1), DeskOutcome::Counted));
        assert_eq!(
            stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "B", at))),
            (key("Q1", 2), DeskOutcome::Revision { of: s(&key("Q1", 1)) })
        );
        assert_eq!(
            stored(ask(&mut mind, &decider("y"), pick(FORK, "Q1", "B", at))),
            (key("Q1", 3), DeskOutcome::Revision { of: s(&key("Q1", 1)) })
        );
        assert_eq!(counted(&mind, FORK), Some(key("Q1", 1)));
    }

    #[test]
    fn receiver_order_not_answered_at_decides_first() {
        // The earlier arrival claims the later time; arrival still decides.
        let mut mind = desk();
        let first = pick(FORK, "Q1", "A", "2026-11-10T13:00:00Z");
        let second = pick(FORK, "Q1", "B", "2026-11-10T11:00:00Z");
        stored(ask(&mut mind, &decider("x"), first));
        stored(ask(&mut mind, &decider("y"), second));
        assert_eq!(counted(&mind, FORK), Some(key("Q1", 1)));
        assert_eq!(held(&mind, &key("Q1", 1)).choice, Some(l("A")));
        // Swap the arrival: the other answer counts.
        let mut mind = desk();
        stored(ask(&mut mind, &decider("y"), pick(FORK, "Q1", "B", "2026-11-10T11:00:00Z")));
        stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T13:00:00Z")));
        assert_eq!(held(&mind, &counted(&mind, FORK).unwrap()).choice, Some(l("B")));
        // And by answerer: the lexically later answerer arriving first still counts.
        let mut mind = desk();
        stored(ask(&mut mind, &decider("z"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        stored(ask(&mut mind, &decider("a"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        assert_eq!(held(&mind, &counted(&mind, FORK).unwrap()).answerer, s("z"));
    }

    #[test]
    fn a_stale_hash_is_stored_refused_and_never_counted() {
        let mut mind = desk();
        let mut request = pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z");
        // The fork as another recommendation would render it: vector B against stored A.
        request.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        let (answer, outcome) = stored(ask(&mut mind, &decider("x"), request));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::StaleFork });
        assert_eq!(held(&mind, &answer).outcome, outcome);
        assert_eq!(counted(&mind, FORK), None);
        // A hash over the title alone is not the fork's hash.
        let mut title_only = question_with("Q1", &["A", "B"], "A");
        title_only.options.clear();
        title_only.question = "".into();
        let mut request = pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z");
        request.options_hash = options_hash(&title_only);
        assert_eq!(stored(ask(&mut mind, &decider("x"), request)).1, DeskOutcome::Refused { code: DeskRefusal::StaleFork });
    }

    #[test]
    fn an_answer_off_the_decider_list_is_refused() {
        let mut mind = desk();
        let off = DeskStamp { answerer: s("nobody"), authority: None };
        let (_, outcome) = stored(ask(&mut mind, &off, pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::NotADecider });
        assert_eq!(counted(&mind, FORK), None);
    }

    #[test]
    fn an_answer_to_a_closed_fork_is_refused() {
        let at = "2026-11-10T12:00:00Z";
        let mut withdrawn_fork = desk();
        committed(admit(&mut withdrawn_fork, vec![resolution(r(K::Question, FORK), withdrawn())]));
        assert_eq!(
            stored(ask(&mut withdrawn_fork, &decider("x"), pick(FORK, "Q1", "A", at))).1,
            DeskOutcome::Refused { code: DeskRefusal::ForkClosed }
        );
        let mut ruled_fork = desk();
        let mut ruled = ruling("RQ1");
        ruled.answers = Some(s(FORK));
        ruled.choice = Some(l("A"));
        committed(admit(&mut ruled_fork, vec![D::Ruling(ruled)]));
        assert_eq!(
            stored(ask(&mut ruled_fork, &decider("x"), pick(FORK, "Q1", "A", at))).1,
            DeskOutcome::Refused { code: DeskRefusal::ForkClosed }
        );
        // The other fork stays open.
        assert_eq!(stored(ask(&mut ruled_fork, &decider("x"), pick(FORK2, "Q2", "A", at))).1, DeskOutcome::Counted);
    }

    #[test]
    fn a_choice_outside_the_option_labels_is_refused() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        // Equal to an option's text, not its label.
        let mut by_text = pick(FORK, "Q1", "A", at);
        by_text.body = DeskBody::Answer { choice: Some(l("an-option")), note: None };
        assert_eq!(stored(ask(&mut mind, &decider("x"), by_text)).1, DeskOutcome::Refused { code: DeskRefusal::ChoiceInvalid });
        // A label of another question.
        committed(admit(&mut mind, vec![question("Q3", &["Z", "Y"], "Z")]));
        assert_eq!(stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "Z", at))).1, DeskOutcome::Refused { code: DeskRefusal::ChoiceInvalid });
        // A note alone is an answer.
        let mut note_only = pick(FORK, "Q1", "A", at);
        note_only.body = DeskBody::Answer { choice: None, note: Some("Neither; ask again.".into()) };
        assert_eq!(stored(ask(&mut mind, &decider("x"), note_only)).1, DeskOutcome::Counted);
    }

    #[test]
    fn refusals_follow_the_binding_step_order() {
        let at = "2026-11-10T12:00:00Z";
        let off = DeskStamp { answerer: s("nobody"), authority: None };
        let mut mind = desk();
        committed(admit(&mut mind, vec![resolution(r(K::Question, FORK), withdrawn())]));
        let stale = |mut request: DeskAnswerRequest| {
            request.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
            request
        };
        let bad_choice = |mut request: DeskAnswerRequest| {
            request.body = DeskBody::Answer { choice: Some(l("Q")), note: None };
            request
        };
        let code = |mind: &mut Mind<MemoryStore>, stamp: &DeskStamp, request| match stored(ask(mind, stamp, request)).1 {
            DeskOutcome::Refused { code } => code,
            other => panic!("{other:?}"),
        };
        // stale + off-list + closed
        assert_eq!(code(&mut mind, &off, stale(pick(FORK, "Q1", "A", at))), DeskRefusal::StaleFork);
        // off-list + closed
        assert_eq!(code(&mut mind, &off, pick(FORK, "Q1", "A", at)), DeskRefusal::NotADecider);
        // closed + bad choice
        assert_eq!(code(&mut mind, &decider("x"), bad_choice(pick(FORK, "Q1", "A", at))), DeskRefusal::ForkClosed);
        // off-list + bad choice, open fork
        assert_eq!(code(&mut mind, &off, bad_choice(pick(FORK2, "Q2", "A", at))), DeskRefusal::NotADecider);
    }

    #[test]
    fn a_bad_retraction_is_stored_refused() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
        let (other_fork, _) = stored(ask(&mut mind, &decider("x"), pick(FORK2, "Q2", "A", at)));
        let mut stale = pick(FORK, "Q1", "A", at);
        stale.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        let (refused, _) = stored(ask(&mut mind, &decider("x"), stale));
        let code = |mind: &mut Mind<MemoryStore>, request| match stored(ask(mind, &decider("y"), request)).1 {
            DeskOutcome::Refused { code } => code,
            other => panic!("{other:?}"),
        };
        // The retraction's own record, once it exists, is not a target.
        assert_eq!(code(&mut mind, retract(FORK, "Q1", &key("Q1", 99))), DeskRefusal::BadRetraction);
        assert_eq!(code(&mut mind, retract(FORK, "Q1", &other_fork)), DeskRefusal::BadRetraction);
        assert_eq!(code(&mut mind, retract(FORK, "Q1", &refused)), DeskRefusal::BadRetraction);
        let mut stale_retraction = retract(FORK, "Q1", &first);
        stale_retraction.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        assert_eq!(code(&mut mind, stale_retraction), DeskRefusal::StaleFork);
        let (retraction, outcome) = stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first)));
        assert_eq!(outcome, DeskOutcome::Retraction { retracts: s(&first) });
        assert_eq!(code(&mut mind, retract(FORK, "Q1", &retraction)), DeskRefusal::BadRetraction);
        assert_eq!(code(&mut mind, retract(FORK, "Q1", &first)), DeskRefusal::AlreadyRetracted);
    }

    #[test]
    fn any_decider_may_retract_and_a_non_decider_may_not() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
        let off = DeskStamp { answerer: s("nobody"), authority: None };
        assert_eq!(stored(ask(&mut mind, &off, retract(FORK, "Q1", &first))).1, DeskOutcome::Refused { code: DeskRefusal::NotADecider });
        assert_eq!(counted(&mind, FORK), Some(first.clone()));
        assert_eq!(stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first))).1, DeskOutcome::Retraction { retracts: s(&first) });
        assert_eq!(counted(&mind, FORK), None);
    }

    #[test]
    fn retracting_the_counted_answer_promotes_the_next() {
        let at = "2026-11-10T12:00:00Z";
        let build = || {
            let mut mind = desk();
            let (x, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
            let (y, _) = stored(ask(&mut mind, &decider("y"), pick(FORK, "Q1", "B", at)));
            let (z, _) = stored(ask(&mut mind, &decider("z"), pick(FORK, "Q1", "A", at)));
            (mind, x, y, z)
        };
        let (mut mind, x, y, _z) = build();
        assert_eq!(counted(&mind, FORK), Some(x.clone()));
        stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &x)));
        assert_eq!(counted(&mind, FORK), Some(y.clone()), "the next, not the latest");
        // A new answer is a revision of the promoted one.
        assert_eq!(stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at))).1, DeskOutcome::Revision { of: s(&y) });
        // Retracting a non-counted answer changes nothing.
        let (mut mind, x, _y, z) = build();
        stored(ask(&mut mind, &decider("x"), retract(FORK, "Q1", &z)));
        assert_eq!(counted(&mind, FORK), Some(x));
        // Retracting everything leaves nothing counted, and the next answer counts again.
        let (mut mind, x, y, z) = build();
        for target in [&x, &y, &z] {
            stored(ask(&mut mind, &decider("x"), retract(FORK, "Q1", target)));
        }
        assert_eq!(counted(&mind, FORK), None);
        assert_eq!(stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at))).1, DeskOutcome::Counted);
    }

    #[test]
    fn refused_answers_are_stored_with_their_code() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let off = DeskStamp { answerer: s("guest"), authority: None };
        let (answer, outcome) = stored(ask(&mut mind, &off, pick(FORK, "Q1", "A", at)));
        let record = held(&mind, &answer);
        assert_eq!(record.outcome, outcome);
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::NotADecider });
        assert_eq!((record.answerer, record.authority, record.answered_at), (s("guest"), None, s(at)));
    }

    #[test]
    fn the_stamp_alone_sets_answerer_and_authority() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        for (who, authority) in [("op", DeskAuthority::Operator), ("dec", DeskAuthority::Decider)] {
            let stamp = DeskStamp { answerer: s(who), authority: Some(authority) };
            let (answer, _) = stored(ask(&mut mind, &stamp, pick(FORK, "Q1", "A", at)));
            let record = held(&mind, &answer);
            assert_eq!((record.answerer, record.authority), (s(who), Some(authority)));
        }
    }

    #[test]
    fn sequence_is_per_question() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let long = "q".repeat(64);
        committed(admit(&mut mind, vec![question(&long, &["A", "B"], "A")]));
        let long_fork = format!("eureka-state:question:{long}");
        let mut keys = Vec::new();
        for (fork, label) in [(FORK, "Q1"), (FORK2, "Q2"), (FORK, "Q1"), (long_fork.as_str(), long.as_str()), (FORK2, "Q2")] {
            keys.push(stored(ask(&mut mind, &decider("x"), pick(fork, label, "A", at))).0);
        }
        assert_eq!(
            keys,
            [key("Q1", 1), key("Q2", 1), key("Q1", 2), key(&long, 1), key("Q2", 2)]
        );
    }

    #[test]
    fn an_unknown_fork_or_shape_fault_stores_nothing() {
        let mut mind = desk();
        let before = head(&mind);
        let at = "2026-11-10T12:00:00Z";
        let unknown = ask(&mut mind, &decider("x"), pick("eureka-state:question:nope", "Q1", "A", at));
        assert!(matches!(unknown, DeskAnswerOutcome::Refused(MindRefusal::MissingReference { kind: K::Question, .. })), "{unknown:?}");
        let mut empty = pick(FORK, "Q1", "A", at);
        empty.body = DeskBody::Answer { choice: None, note: None };
        let bad_time = pick(FORK, "Q1", "A", "yesterday");
        let not_a_question = pick("eureka-state:ruling:R1", "Q1", "A", at);
        let bad_target = retract(FORK, "Q1", "n1");
        for (request, field) in [(empty, "body"), (bad_time, "answered_at"), (not_a_question, "fork"), (bad_target, "desk_answer.outcome.retracts")] {
            assert_eq!(
                ask(&mut mind, &decider("x"), request),
                DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: field.into() })
            );
        }
        assert_eq!(head(&mind), before);
    }

    #[test]
    fn a_moved_campaign_takes_no_answer() {
        let mut mind = desk();
        let before = head(&mind);
        committed(admit(&mut mind, vec![resolution(r(K::Campaign, "eureka-state:campaign:self"), ResolutionOutcome::Moved { to: slug("thought-cage") })]));
        let moved = head(&mind);
        assert!(moved > before);
        let outcome = ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z"));
        assert!(matches!(outcome, DeskAnswerOutcome::Refused(MindRefusal::CampaignMoved { .. })), "{outcome:?}");
        assert_eq!(head(&mind), moved);
    }

    #[test]
    fn a_desk_answer_cannot_enter_through_admit_or_import() {
        let mut mind = desk();
        let before = head(&mind);
        let at = "2026-11-10T12:00:00Z";
        let (answer, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
        let mut forged = held(&mind, &answer);
        forged.sequence = 2;
        forged.outcome = DeskOutcome::Counted;
        let id = key("Q1", 2);
        let doc = D::DeskAnswer(forged);
        let after = head(&mind);
        assert!(after > before);
        assert_eq!(refusal(admit(&mut mind, vec![doc.clone()])), MindRefusal::DeskAnswerOutsideDesk { id: id.clone() });
        // Beside a legitimate document, still refused whole.
        assert_eq!(refusal(admit(&mut mind, vec![question_n(9), doc.clone()])), MindRefusal::DeskAnswerOutsideDesk { id: id.clone() });
        // An import carrying one.
        let source = crate::receipt::ImportSource {
            instance: slug(OTHER_INSTANCE),
            receipt_id: s("mind-commit-0"),
            ordinal: 1,
            committed_at: s("2026-11-10T12:00:00Z"),
            faculty: Faculty::Operator,
            agent: s("claude"),
            session: s("session-1"),
            tool: s("admit"),
            external: vec![],
        };
        let mut provenance = provenance(Faculty::Hands);
        provenance.imported_from = Some(source);
        let envelope = crate::fixtures::prepare(&doc);
        let outcome = mind.admit_prepared(&mind.instance().clone(), provenance, vec![envelope], now());
        assert_eq!(refusal(outcome), MindRefusal::DeskAnswerOutsideDesk { id });
        assert_eq!(head(&mind), after);
    }

    #[test]
    fn a_desk_answer_takes_no_resolution() {
        let mut mind = desk();
        let (answer, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        let before = head(&mind);
        let by = r(K::Question, FORK2);
        for outcome in [
            ResolutionOutcome::Superseded { by: vec![r(K::DeskAnswer, &answer)] },
            ResolutionOutcome::Answered { by: r(K::Ruling, "eureka-state:ruling:R1") },
            ResolutionOutcome::Fixed { commit: sha(), by: None },
            ResolutionOutcome::Deferred { to: by.clone() },
            ResolutionOutcome::Recorded { reason: "x".into() },
            withdrawn(),
            ResolutionOutcome::Slept { line: "x".into(), day_through: None },
            ResolutionOutcome::Moved { to: slug("thought-cage") },
        ] {
            let outcome = admit(&mut mind, vec![resolution(r(K::DeskAnswer, &answer), outcome)]);
            assert!(matches!(outcome, PipelineAdmissionOutcome::Refused(_)), "{outcome:?}");
        }
        assert_eq!(head(&mind), before);
        assert_eq!(counted(&mind, FORK), Some(answer));
    }
}
