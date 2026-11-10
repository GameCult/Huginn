//! The fork desk's answers: the Mind's counting door.
//!
//! `Mind::answer_desk` is the one owner of whether a desk answer counts, its
//! sequence, its stored outcome and the stamping of `answerer` and `authority`
//! (binding spec /opt/commons/handoff/fork-desk-answer-binding-20261110.md,
//! 1.3 steps 3-8 and R1, R2, R4, and CultLib PR #72 `fork-card.md` for the
//! request id). It builds one `desk_answer` and commits it through
//! `admit_steps` with `Door::Desk`, the same commit primitive every other write
//! uses; no other door may write the kind. `answer_desk` and `DeskStamp` are
//! crate-private: only code inside this crate can stamp, so the verified door
//! that will call them (follow-up desk-answer-door) widens them in its own cut.
//!
//! The decider list is an input (`DeskStamp.deciders`, from the door); the Mind
//! never reads one. Whether an answerer decides is membership in that list and
//! nothing else: the stamped `authority` (Operator, Steward, Member) is an
//! attribution copied into the record and never decides. The request types carry
//! no `answerer` and no `authority`. `answered_at` is the client's claim: it is
//! displayed and flagged (`claim_in_future`), never compared for order, which is
//! the per-question sequence, the receiver's order. `received_at` is the Mind's.

use chrono::{DateTime, Duration, SecondsFormat, SubsecRound, Utc};
use eureka_pipeline::{
    DeskAuthority, DeskOutcome, DeskRefusal, PipelineDeskAnswer, PipelineDocument, PipelineKind, PipelineQuestion, PipelineRefusal,
    PipelineResolution, RequestId, ResolutionOutcome, Sha256Hex, Short, Slug, Words,
};

use crate::admission::{Door, PipelineAdmissionOutcome, prepare};
use crate::docs::{Docs, kind_of_id};
use crate::mind::Mind;
use crate::receipt::{Faculty, PipelineProvenance};
use crate::refusal::MindRefusal;
use crate::rows::root_and_local;
use crate::store::MindStore;

/// What the desk's receiver sends the Mind. `answered_at` is the person's
/// claim, RFC 3339 in UTC, kept as written. `request_id` is the client's retry
/// token (a lowercase UUID), required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskAnswerRequest {
    pub fork: Short,
    pub options_hash: Sha256Hex,
    pub body: DeskBody,
    pub answered_at: Short,
    pub request_id: RequestId,
}

/// An answer, a retraction of one, or one of the wire's illegal retraction forms. `choice` is
/// the person's text, any non-empty string: one that is no option label is a stored
/// `choice_invalid`, grammar or not (binding spec step 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskBody {
    Answer { choice: Option<Short>, note: Option<Words> },
    Retraction { retracts: Short },
    /// A retraction carrying an answer, naming no target, or a plain answer carrying
    /// `retracts` (R1): what the door decodes those three forms to. Always stored as a
    /// refused `bad_retraction` after steps 4 to 6; the record keeps every field as sent.
    /// `retracted` is the wire flag as sent and reaches only the body digest: true for a
    /// retraction carrying an answer or naming no target, false for a plain answer carrying
    /// `retracts`. Any other combination is no illegal form (`body` invalid).
    IllegalRetraction { choice: Option<Short>, note: Option<Words>, retracts: Option<Short>, retracted: bool },
}

/// The door's verified context for one call: who answered, the role the account
/// holds (an attribution, stored on every answer) and the decider list the door
/// holds. Membership of `answerer` in `deciders` is rule 5's whole input; the
/// attribution never enters it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeskStamp {
    pub(crate) answerer: Short,
    pub(crate) authority: DeskAuthority,
    pub(crate) deciders: Vec<Short>,
}

/// `Stored`: the answer id (the record key), its outcome and the commit's receipt.
/// `Replayed`: the same `(answerer, request_id)` and body came before; the stored
/// outcome comes back unchanged and nothing was written. (A record holds no card:
/// the read side shows the current card beside a `stale_fork`, replayed or not.)
/// `Refused`: nothing was stored (a shape fault, an unknown fork, a moved campaign,
/// a request id reused for another body, or a commit fault).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskAnswerOutcome {
    Stored { answer: Short, outcome: DeskOutcome, receipt_id: String },
    Replayed { answer: Short, outcome: DeskOutcome },
    Refused(MindRefusal),
}

/// The hash of a fork as the desk shows it, which an answer must quote (rule 4): the
/// lowercase hex SHA-256 of the canonical bytes pinned by CultLib PR #72
/// (`contracts/cultnet/governance/fork-card.md`, "Options hash"). The five members
/// `context`, `depends`, `options[{id, text}]`, `recommendation`, `title`, every string
/// NFC-normalised, serialised as RFC 8785 JCS. The struct declares its fields in JCS key
/// order, and `serde_json::to_string` writes no whitespace, escapes only `"`, `\` and the
/// controls below U+0020 (lowercase `\u00xx`, `\b \t \n \f \r` short forms) and
/// writes everything else raw, which is JCS for strings; the five members hold no number.
/// `depends` are the stored lines verbatim (`PipelineQuestion.depends` is prose lines).
/// This is the one owner: the counting rule calls it and nothing else computes the hash.
pub fn options_hash(question: &PipelineQuestion) -> Sha256Hex {
    use sha2::{Digest, Sha256};
    use unicode_normalization::UnicodeNormalization;

    // Declared in JCS key order (UTF-16 code units: context < depends < options < recommendation < title).
    #[derive(serde::Serialize)]
    struct Shown {
        context: String,
        depends: Vec<String>,
        options: Vec<ShownOption>,
        recommendation: String,
        title: String,
    }
    #[derive(serde::Serialize)]
    struct ShownOption {
        id: String,
        text: String,
    }
    let nfc = |text: &str| text.nfc().collect::<String>();
    let shown = Shown {
        context: nfc(&question.question.0),
        depends: question.depends.iter().map(|line| nfc(&line.0)).collect(),
        options: question.options.iter().map(|option| ShownOption { id: nfc(&option.label.0), text: nfc(&option.text.0) }).collect(),
        recommendation: nfc(&question.recommended.0),
        title: nfc(&question.title.0),
    };
    let canonical = serde_json::to_string(&shown).expect("strings serialise");
    Sha256Hex(Sha256::digest(canonical.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect())
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

/// Whether a stored answer is a candidate to count under the fork's current card:
/// Counted or Revision, and written against the card's current options hash.
fn counts_under(answer: &PipelineDeskAnswer, current: &Sha256Hex) -> bool {
    matches!(answer.outcome, DeskOutcome::Counted | DeskOutcome::Revision { .. }) && answer.options_hash == *current
}

/// The answer that counts for `fork` now: the lowest-sequence answer whose stored
/// outcome is Counted or Revision under the card's current options hash and that no
/// Retraction names (R4; an answer under an older hash stops counting). Derived on
/// every call and never stored; retracting the counted answer promotes the next,
/// retracting another changes nothing. The one place "counts" is decided.
pub(crate) fn counted_answer(docs: &Docs<'_>, fork: &str) -> Option<String> {
    let Some(PipelineDocument::Question(question)) = docs.find(PipelineKind::Question, fork) else { return None };
    let current = options_hash(question);
    answers_of(docs, fork)
        .filter(|(key, answer)| counts_under(answer, &current) && !is_retracted(docs, fork, key))
        .min_by_key(|(_, answer)| answer.sequence)
        .map(|(key, _)| key.to_string())
}

/// Rules 4-7 and R1 in the binding spec's order; the first failure is the code.
fn refusal_of(docs: &Docs<'_>, question: &PipelineQuestion, stamp: &DeskStamp, request: &DeskAnswerRequest) -> Option<DeskRefusal> {
    let fork = request.fork.0.as_str();
    let current = options_hash(question);
    if request.options_hash != current {
        return Some(DeskRefusal::StaleFork);
    }
    if !stamp.deciders.contains(&stamp.answerer) {
        return Some(DeskRefusal::NotADecider);
    }
    if !docs.in_force(PipelineKind::Question, fork) {
        return Some(DeskRefusal::ForkClosed);
    }
    match &request.body {
        DeskBody::Answer { choice: Some(choice), .. } if !question.options.iter().any(|option| option.label.0 == choice.0) => {
            Some(DeskRefusal::ChoiceInvalid)
        }
        DeskBody::Answer { .. } => None,
        DeskBody::IllegalRetraction { .. } => Some(DeskRefusal::BadRetraction),
        DeskBody::Retraction { retracts } => {
            let target = answers_of(docs, fork).find(|(key, _)| *key == retracts.0);
            if !target.is_some_and(|(_, answer)| counts_under(answer, &current)) {
                return Some(DeskRefusal::BadRetraction);
            }
            is_retracted(docs, fork, &retracts.0).then_some(DeskRefusal::AlreadyRetracted)
        }
    }
}

/// The one definition of "the same body": Huginn's own SHA-256 over the canonical body of the
/// request, which is every part of it but the id itself and so includes `answered_at` and, for a
/// retraction, the target. It is never read from the client. It decides a replay (equal), a
/// `request_id_conflict` (unequal under a bound id) and nothing else; no other code compares
/// bodies, a refused retraction's target included, which the stored `outcome` already carries.
///
/// FROZEN: the digest form (field order, `serde_json` string of `Sent`, lowercase hex) is stored
/// in every answer and read back by retries years later. A change of form is a new version of the
/// stored field, never an edit here; `body_digest_is_pinned` fails on any edit. The form covers
/// the contract's fork_id, options_hash, choice, note, retracts, answered_at and, for the illegal
/// form only, `retracted` (the two legal variants fix the flag, so their bytes do not carry it);
/// the three illegal goldens and the two legal ones are all frozen.
fn body_digest(request: &DeskAnswerRequest) -> Sha256Hex {
    use sha2::{Digest, Sha256};
    #[derive(serde::Serialize)]
    struct Sent<'a> {
        fork: &'a str,
        options_hash: &'a str,
        answered_at: &'a str,
        choice: Option<&'a str>,
        note: Option<&'a str>,
        retracts: Option<&'a str>,
        // Written only for the illegal form, so every other body keeps its stored digest.
        #[serde(skip_serializing_if = "is_false")]
        illegal_retraction: bool,
        // The wire flag, written only for the illegal form (the legal variants fix it).
        #[serde(skip_serializing_if = "Option::is_none")]
        retracted: Option<bool>,
    }
    fn is_false(flag: &bool) -> bool {
        !*flag
    }
    let (choice, note, retracts, illegal_retraction, retracted) = match &request.body {
        DeskBody::Answer { choice, note } => (choice.as_ref().map(|c| c.0.as_str()), note.as_ref().map(|n| n.0.as_str()), None, false, None),
        DeskBody::Retraction { retracts } => (None, None, Some(retracts.0.as_str()), false, None),
        DeskBody::IllegalRetraction { choice, note, retracts, retracted } => {
            (choice.as_ref().map(|c| c.0.as_str()), note.as_ref().map(|n| n.0.as_str()), retracts.as_ref().map(|r| r.0.as_str()), true, Some(*retracted))
        }
    };
    let sent = Sent { fork: &request.fork.0, options_hash: &request.options_hash.0, answered_at: &request.answered_at.0, choice, note, retracts, illegal_retraction, retracted };
    let bytes = serde_json::to_string(&sent).expect("strings serialise");
    Sha256Hex(Sha256::digest(bytes.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect())
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
    /// code. A retry of a stored request is replayed, never stored again. See the
    /// module header for the order and the stored/not-stored line.
    #[cfg_attr(not(test), allow(dead_code))] // the verified door (follow-up desk-answer-door) is the live caller
    pub(crate) fn answer_desk(&mut self, stamp: DeskStamp, request: DeskAnswerRequest, now: DateTime<Utc>) -> DeskAnswerOutcome {
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
        // answered_at is the client's claim and must be RFC 3339 in UTC (offset zero).
        let claimed = match DateTime::parse_from_rfc3339(&request.answered_at.0) {
            Ok(at) if at.offset().local_minus_utc() == 0 => at.with_timezone(&Utc),
            _ => return Err(invalid("answered_at")),
        };
        // A choice is any non-empty text of at most a Short (200 bytes), kept as sent: the record's field
        // is a Short because a `choice_invalid` choice is no label.
        let choice_shape = |choice: &Short| if choice.0.is_empty() || choice.0.len() > 200 { Err(invalid("desk_answer.choice")) } else { Ok(()) };
        let (choice, note, shape_outcome) = match &request.body {
            DeskBody::Answer { choice, note } => {
                if choice.is_none() && note.is_none() {
                    return Err(invalid("body"));
                }
                choice.as_ref().map(choice_shape).transpose()?;
                (choice.clone(), note.clone(), DeskOutcome::Counted)
            }
            DeskBody::Retraction { retracts } => (None, None, DeskOutcome::Retraction { retracts: retracts.clone() }),
            DeskBody::IllegalRetraction { choice, note, retracts, retracted } => {
                // Only the three wire forms are illegal retractions; any other value would store
                // bad_retraction for a body the wire meant as legal.
                let answered = choice.is_some() || note.is_some();
                let is_form = if *retracted { answered || retracts.is_none() } else { answered && retracts.is_some() };
                if !is_form {
                    return Err(invalid("body"));
                }
                choice.as_ref().map(choice_shape).transpose()?;
                if retracts.as_ref().is_some_and(|target| target.0.len() > 200) {
                    return Err(invalid("desk_answer.outcome.retracts"));
                }
                (choice.clone(), note.clone(), DeskOutcome::Counted)
            }
        };
        let body_sha256 = body_digest(&request);
        // received_at is the instant as stored, whole seconds, and claim_in_future is decided
        // against that stored value, never the finer clock reading.
        let received = now.trunc_subsecs(0);
        let received_at = Short(received.to_rfc3339_opts(SecondsFormat::Secs, true));
        let claim_in_future = claimed > received + Duration::minutes(5);
        let answer = |sequence: u32, outcome: DeskOutcome| PipelineDeskAnswer {
            campaign: Slug(root.clone()),
            fork: request.fork.clone(),
            sequence,
            options_hash: request.options_hash.clone(),
            choice: choice.clone(),
            note: note.clone(),
            answered_at: request.answered_at.clone(),
            received_at: received_at.clone(),
            claim_in_future,
            request_id: request.request_id.clone(),
            body_sha256: body_sha256.clone(),
            answerer: stamp.answerer.clone(),
            authority: stamp.authority,
            outcome,
        };
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let fork = request.fork.0.as_str();

        // The retry lookup: after the cheap shape and the body digest, before the probe and before
        // the fork is read, so a retry gets its stored outcome without re-validating and even if the
        // card has since changed. A malformed request_id binds no record and meets the probe below.
        // The key is (answerer, request_id).
        let bound = docs.of_kind(PipelineKind::DeskAnswer).find_map(|(key, document)| match document {
            PipelineDocument::DeskAnswer(held) if held.answerer == stamp.answerer && held.request_id == request.request_id => Some((key, held)),
            _ => None,
        });
        if let Some((key, held)) = bound {
            if held.body_sha256 != body_sha256 {
                return Err(MindRefusal::DeskRequestIdConflict { request_id: request.request_id.0.clone() });
            }
            return Ok(DeskAnswerOutcome::Replayed { answer: Short(key.to_string()), outcome: held.outcome.clone() });
        }

        let probe = PipelineDocument::DeskAnswer(answer(1, shape_outcome));
        probe.validate().map_err(shape_fault)?;
        eureka_pipeline::pipeline_key(&probe).map_err(shape_fault)?;

        // Rules 3 and the moved campaign: nothing is stored.
        let Some(PipelineDocument::Question(question)) = docs.find(PipelineKind::Question, fork) else {
            return Err(MindRefusal::MissingReference { kind: PipelineKind::Question, id: fork.into() });
        };
        if let Some((moved, PipelineResolution { outcome: ResolutionOutcome::Moved { to }, .. })) = docs.moved_campaign(&root) {
            return Err(MindRefusal::CampaignMoved { campaign: root_and_local(moved).0.into(), to: to.0.clone() });
        }

        // Rules 4-7, R1: stored, refused, with the first failing code.
        let outcome = match refusal_of(&docs, question, &stamp, &request) {
            Some(code) => DeskOutcome::Refused {
                code,
                retracts: match &request.body {
                    DeskBody::Retraction { retracts } => Some(retracts.clone()),
                    DeskBody::IllegalRetraction { retracts, .. } => retracts.clone(),
                    DeskBody::Answer { .. } => None,
                },
            },
            None => match &request.body {
                DeskBody::Retraction { retracts } => DeskOutcome::Retraction { retracts: retracts.clone() },
                DeskBody::IllegalRetraction { .. } => unreachable!("refusal_of refuses every illegal retraction"),
                DeskBody::Answer { .. } => match counted_answer(&docs, fork) {
                    None => DeskOutcome::Counted,
                    Some(counted) => DeskOutcome::Revision { of: Short(counted) },
                },
            },
        };
        let sequence = answers_of(&docs, fork).map(|(_, held)| held.sequence).max().unwrap_or(0).checked_add(1).ok_or_else(|| invalid("sequence"))?;
        let document = PipelineDocument::DeskAnswer(answer(sequence, outcome.clone()));
        let envelope = prepare(&document, self.cache())?;
        // The receipt names the operator only when the verified account is the operator's; a
        // steward or member answerer is named as such. Attribution only: admission reads no faculty.
        let provenance = PipelineProvenance {
            faculty: match stamp.authority {
                DeskAuthority::Operator => Faculty::Operator,
                DeskAuthority::Steward => Faculty::Steward,
                DeskAuthority::Member => Faculty::Member,
            },
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
mod tests {
    use super::*;
    use eureka_pipeline::Label;
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

    /// The decider list the door holds in these tests.
    const LIST: [&str; 6] = ["x", "y", "z", "a", "op", "dec"];

    fn stamp_as(who: &str, authority: DeskAuthority, list: &[&str]) -> DeskStamp {
        DeskStamp { answerer: s(who), authority, deciders: list.iter().map(|id| s(id)).collect() }
    }

    fn decider(who: &str) -> DeskStamp {
        stamp_as(who, DeskAuthority::Member, &LIST)
    }

    /// An account on no decider list.
    fn off() -> DeskStamp {
        stamp_as("nobody", DeskAuthority::Member, &LIST)
    }

    /// A fresh lowercase UUID per call: every request is its own unless a test reuses one.
    fn fresh_id() -> RequestId {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        RequestId(format!("00000000-0000-4000-8000-{:012x}", NEXT.fetch_add(1, Ordering::Relaxed)))
    }

    fn pick(fork: &str, label: &str, choice: &str, answered_at: &str) -> DeskAnswerRequest {
        DeskAnswerRequest {
            fork: s(fork),
            options_hash: hash(label),
            body: DeskBody::Answer { choice: Some(s(choice)), note: None },
            answered_at: s(answered_at),
            request_id: fresh_id(),
        }
    }

    fn retract(fork: &str, label: &str, target: &str) -> DeskAnswerRequest {
        DeskAnswerRequest {
            fork: s(fork),
            options_hash: hash(label),
            body: DeskBody::Retraction { retracts: s(target) },
            answered_at: s("2026-11-10T12:00:00Z"),
            request_id: fresh_id(),
        }
    }

    fn key(label: &str, n: u32) -> String {
        format!("eureka-state:desk_answer:{label}.n{n}")
    }

    fn stored(outcome: DeskAnswerOutcome) -> (String, DeskOutcome) {
        match outcome {
            DeskAnswerOutcome::Stored { answer, outcome, .. } | DeskAnswerOutcome::Replayed { answer, outcome, .. } => (answer.0, outcome),
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
        assert_eq!(held(&mind, &key("Q1", 1)).choice, Some(s("A")));
        // Swap the arrival: the other answer counts.
        let mut mind = desk();
        stored(ask(&mut mind, &decider("y"), pick(FORK, "Q1", "B", "2026-11-10T11:00:00Z")));
        stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T13:00:00Z")));
        assert_eq!(held(&mind, &counted(&mind, FORK).unwrap()).choice, Some(s("B")));
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
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::StaleFork, retracts: None });
        assert_eq!(held(&mind, &answer).outcome, outcome);
        assert_eq!(counted(&mind, FORK), None);
        // A hash over the title alone is not the fork's hash.
        let mut title_only = question_with("Q1", &["A", "B"], "A");
        title_only.options.clear();
        title_only.question = "".into();
        let mut request = pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z");
        request.options_hash = options_hash(&title_only);
        assert_eq!(stored(ask(&mut mind, &decider("x"), request)).1, DeskOutcome::Refused { code: DeskRefusal::StaleFork, retracts: None });
    }

    #[test]
    fn an_answer_off_the_decider_list_is_refused() {
        let mut mind = desk();
        let off = off();
        let (_, outcome) = stored(ask(&mut mind, &off, pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: None });
        assert_eq!(counted(&mind, FORK), None);
    }

    #[test]
    fn an_answer_to_a_closed_fork_is_refused() {
        let at = "2026-11-10T12:00:00Z";
        let mut withdrawn_fork = desk();
        committed(admit(&mut withdrawn_fork, vec![resolution(r(K::Question, FORK), withdrawn())]));
        assert_eq!(
            stored(ask(&mut withdrawn_fork, &decider("x"), pick(FORK, "Q1", "A", at))).1,
            DeskOutcome::Refused { code: DeskRefusal::ForkClosed, retracts: None }
        );
        let mut ruled_fork = desk();
        let mut ruled = ruling("RQ1");
        ruled.answers = Some(s(FORK));
        ruled.choice = Some(l("A"));
        committed(admit(&mut ruled_fork, vec![D::Ruling(ruled)]));
        assert_eq!(
            stored(ask(&mut ruled_fork, &decider("x"), pick(FORK, "Q1", "A", at))).1,
            DeskOutcome::Refused { code: DeskRefusal::ForkClosed, retracts: None }
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
        by_text.body = DeskBody::Answer { choice: Some(s("an-option")), note: None };
        assert_eq!(stored(ask(&mut mind, &decider("x"), by_text)).1, DeskOutcome::Refused { code: DeskRefusal::ChoiceInvalid, retracts: None });
        // A label of another question.
        committed(admit(&mut mind, vec![question("Q3", &["Z", "Y"], "Z")]));
        assert_eq!(stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "Z", at))).1, DeskOutcome::Refused { code: DeskRefusal::ChoiceInvalid, retracts: None });
        // A note alone is an answer.
        let mut note_only = pick(FORK, "Q1", "A", at);
        note_only.body = DeskBody::Answer { choice: None, note: Some("Neither; ask again.".into()) };
        assert_eq!(stored(ask(&mut mind, &decider("x"), note_only)).1, DeskOutcome::Counted);
    }

    #[test]
    fn refusals_follow_the_binding_step_order() {
        let at = "2026-11-10T12:00:00Z";
        let off = off();
        let mut mind = desk();
        committed(admit(&mut mind, vec![resolution(r(K::Question, FORK), withdrawn())]));
        let stale = |mut request: DeskAnswerRequest| {
            request.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
            request
        };
        let bad_choice = |mut request: DeskAnswerRequest| {
            request.body = DeskBody::Answer { choice: Some(s("Q")), note: None };
            request
        };
        let code = |mind: &mut Mind<MemoryStore>, stamp: &DeskStamp, request| match stored(ask(mind, stamp, request)).1 {
            DeskOutcome::Refused { code, .. } => code,
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
            DeskOutcome::Refused { code, .. } => code,
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
        let off = off();
        assert_eq!(stored(ask(&mut mind, &off, retract(FORK, "Q1", &first))).1, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: Some(s(&first)) });
        assert_eq!(counted(&mind, FORK), Some(first.clone()));
        assert_eq!(stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first))).1, DeskOutcome::Retraction { retracts: s(&first) });
        assert_eq!(counted(&mind, FORK), None);
    }

    /// What a read at `as_of` shows of the fork's answers: (key, stored outcome name, counts now).
    fn read_counts(mind: &Mind<MemoryStore>, as_of: u64, keys: &[String]) -> Vec<(String, String, bool)> {
        let selection = cultnet_rs::Selection { keys: Some(keys.to_vec()), ..Default::default() };
        let page = mind.query_at(&selection, as_of).unwrap();
        let crate::query::PipelinePageItems::Headers(headers) = page.items else { panic!("headers") };
        headers
            .into_iter()
            .map(|header| match header.facts {
                crate::query::PipelineFacts::DeskAnswer { outcome, counts, .. } => (header.id.id.0, outcome.0, counts),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    /// Which answer counts is derived at the read's snapshot, never the stored write-time word: n1 is
    /// stored `Counted` forever, but once retracted the facts say it no longer counts and the promoted
    /// n2 does; a read at an earlier snapshot shows the earlier counting.
    #[test]
    fn a_retraction_changes_which_answer_the_facts_say_counts() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let (n1, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
        let (n2, second) = stored(ask(&mut mind, &decider("y"), pick(FORK, "Q1", "B", at)));
        assert_eq!(second, DeskOutcome::Revision { of: s(&n1) });
        let both = [n1.clone(), n2.clone()];
        let before = head(&mind);
        assert_eq!(read_counts(&mind, before, &both), [(n1.clone(), "Counted".into(), true), (n2.clone(), "Revision".into(), false)]);
        stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &n1)));
        let after = head(&mind);
        // The stored words are unchanged; the derived flag moved.
        assert_eq!(read_counts(&mind, after, &both), [(n1.clone(), "Counted".into(), false), (n2.clone(), "Revision".into(), true)]);
        assert_eq!(held(&mind, &n1).outcome, DeskOutcome::Counted);
        // A read at the earlier snapshot shows the earlier counting.
        assert_eq!(read_counts(&mind, before, &both), [(n1.clone(), "Counted".into(), true), (n2.clone(), "Revision".into(), false)]);
        assert_eq!(counted(&mind, FORK), Some(n2));
    }

    /// Every refusal keeps what the request was sent with: the choice, the note and (for a retraction
    /// form) the target, under every refusal code the desk stores.
    #[test]
    fn every_refusal_keeps_what_was_sent() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", at)));
        let kept = |mind: &mut Mind<MemoryStore>, stamp: &DeskStamp, request: DeskAnswerRequest| {
            let (answer, outcome) = stored(ask(mind, stamp, request));
            let record = held(mind, &answer);
            (outcome, record.choice, record.note)
        };
        let with = |fork: &str, label: &str, body: DeskBody| DeskAnswerRequest { body, ..pick(fork, label, "A", at) };
        let answer = |choice: &str| DeskBody::Answer { choice: Some(s(choice)), note: Some("because".into()) };
        let code = |code: DeskRefusal, retracts: Option<&str>| DeskOutcome::Refused { code, retracts: retracts.map(s) };
        // stale_fork: an old card.
        let mut stale = with(FORK, "Q1", answer("A"));
        stale.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        assert_eq!(kept(&mut mind, &decider("x"), stale), (code(DeskRefusal::StaleFork, None), Some(s("A")), Some("because".into())));
        // not_a_decider, answer and retraction.
        assert_eq!(kept(&mut mind, &off(), with(FORK, "Q1", answer("B"))), (code(DeskRefusal::NotADecider, None), Some(s("B")), Some("because".into())));
        assert_eq!(kept(&mut mind, &off(), retract(FORK, "Q1", &first)), (code(DeskRefusal::NotADecider, Some(&first)), None, None));
        // choice_invalid keeps the text that is no label.
        assert_eq!(kept(&mut mind, &decider("x"), with(FORK, "Q1", answer("not a label"))), (code(DeskRefusal::ChoiceInvalid, None), Some(s("not a label")), Some("because".into())));
        // bad_retraction (no such answer) keeps the target; already_retracted keeps it too.
        let missing = key("Q1", 99);
        assert_eq!(kept(&mut mind, &decider("y"), retract(FORK, "Q1", &missing)), (code(DeskRefusal::BadRetraction, Some(&missing)), None, None));
        stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first)));
        assert_eq!(kept(&mut mind, &decider("z"), retract(FORK, "Q1", &first)), (code(DeskRefusal::AlreadyRetracted, Some(&first)), None, None));
        // fork_closed: a ruling closes the fork, then an answer and a retraction.
        let mut ruled = ruling("RQ1");
        ruled.answers = Some(s(FORK));
        ruled.choice = Some(l("A"));
        committed(admit(&mut mind, vec![D::Ruling(ruled)]));
        assert_eq!(kept(&mut mind, &decider("x"), with(FORK, "Q1", answer("A"))), (code(DeskRefusal::ForkClosed, None), Some(s("A")), Some("because".into())));
        assert_eq!(kept(&mut mind, &decider("x"), retract(FORK, "Q1", &first)), (code(DeskRefusal::ForkClosed, Some(&first)), None, None));
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
        let off = stamp_as("guest", DeskAuthority::Steward, &LIST);
        let (answer, outcome) = stored(ask(&mut mind, &off, pick(FORK, "Q1", "A", at)));
        let record = held(&mind, &answer);
        assert_eq!(record.outcome, outcome);
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: None });
        assert_eq!((record.answerer, record.authority, record.answered_at), (s("guest"), DeskAuthority::Steward, s(at)));
    }

    #[test]
    fn the_stamp_alone_sets_answerer_and_authority() {
        let mut mind = desk();
        let at = "2026-11-10T12:00:00Z";
        for (who, authority) in [("op", DeskAuthority::Operator), ("dec", DeskAuthority::Steward), ("x", DeskAuthority::Member)] {
            let (answer, _) = stored(ask(&mut mind, &stamp_as(who, authority, &LIST), pick(FORK, "Q1", "A", at)));
            let record = held(&mind, &answer);
            assert_eq!((record.answerer, record.authority), (s(who), authority));
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
        let mut blank_note = pick(FORK, "Q1", "A", at);
        blank_note.body = DeskBody::Answer { choice: None, note: Some("  \n\t ".into()) };
        let mut empty_note = pick(FORK, "Q1", "A", at);
        empty_note.body = DeskBody::Answer { choice: Some(s("A")), note: Some("".into()) };
        let mut not_utc = pick(FORK, "Q1", "A", "2026-11-10T14:00:00+02:00");
        not_utc.answered_at = s("2026-11-10T14:00:00+02:00");
        let mut no_id = pick(FORK, "Q1", "A", at);
        no_id.request_id = RequestId("".into());
        let mut upper_id = pick(FORK, "Q1", "A", at);
        upper_id.request_id = RequestId("00000000-0000-4000-8000-00000000ABCD".into());
        let mut not_uuid = pick(FORK, "Q1", "A", at);
        not_uuid.request_id = RequestId("req-1".into());
        let mut braced = pick(FORK, "Q1", "A", at);
        braced.request_id = RequestId("{00000000-0000-4000-8000-00000000abcd}".into());
        let bad_time = pick(FORK, "Q1", "A", "yesterday");
        let not_a_question = pick("eureka-state:ruling:R1", "Q1", "A", at);
        let bad_target = retract(FORK, "Q1", "n1");
        for (request, field) in [
            (empty, "body"),
            (blank_note, "desk_answer.note"),
            (empty_note, "desk_answer.note"),
            (not_utc, "answered_at"),
            (no_id, "desk_answer.request_id"),
            (upper_id, "desk_answer.request_id"),
            (not_uuid, "desk_answer.request_id"),
            (braced, "desk_answer.request_id"),
            (bad_time, "answered_at"),
            (not_a_question, "fork"),
            (bad_target, "desk_answer.outcome.retracts"),
        ] {
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
        // The door is refused before references are read: a forged answer to a fork nobody holds is
        // still the door's refusal.
        let D::DeskAnswer(mut orphan) = doc.clone() else { unreachable!() };
        orphan.fork = s("eureka-state:question:nobody");
        orphan.sequence = 1;
        let orphan_id = "eureka-state:desk_answer:nobody.n1".to_string();
        assert_eq!(refusal(admit(&mut mind, vec![D::DeskAnswer(orphan)])), MindRefusal::DeskAnswerOutsideDesk { id: orphan_id });
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

    /// Libby's vectors (CultLib PR #72 head 7aec4961, `fork-card-vectors.json`, `optionsHash`): the seven
    /// of the binding file, A to G, and the eight she adds. Each is recomputed from its shown members.
    fn vectors() -> Vec<serde_json::Value> {
        serde_json::from_str(include_str!("../vectors/fork-card-options-hash.json")).unwrap()
    }

    fn shown(vector: &serde_json::Value) -> PipelineQuestion {
        let text = |value: &serde_json::Value| -> String { value.as_str().unwrap().to_string() };
        let D::Question(mut question) = question("Q1", &["A", "B"], "A") else { unreachable!() };
        question.title = text(&vector["title"]).as_str().into();
        question.question = text(&vector["context"]).as_str().into();
        question.recommended = Label(text(&vector["recommendation"]));
        question.options = vector["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| eureka_pipeline::QuestionOption { label: Label(text(&option["id"])), text: text(&option["text"]).as_str().into() })
            .collect();
        question.depends = vector["depends"].as_array().unwrap().iter().map(|line| text(line).as_str().into()).collect();
        question
    }

    #[test]
    fn options_hash_reproduces_binding_vectors() {
        let vectors = vectors();
        assert_eq!(vectors.len(), 15);
        for vector in &vectors {
            let name = vector["name"].as_str().unwrap();
            assert_eq!(options_hash(&shown(vector)).0, vector["sha256"].as_str().unwrap(), "{name}");
        }
        let by = |tag: &str| vectors.iter().find(|vector| vector["heimdall"] == tag).unwrap();
        let hash = |tag: &str| options_hash(&shown(by(tag))).0;
        // The pinned values of the binding file, 1.5.
        assert_eq!(hash("A"), "7d95fc87d532420611fe415c1def27a3447617bb68235f46e3af130aea361c48");
        assert_eq!(hash("B"), "14b8ffbdb61106dbc06f69f3fc8e2072af7a205d766e4cc040e89733013d54dc");
        assert_eq!(hash("C"), "ac67b41539f869f6203ac0908819df046e4328b27795743a919bd4ab390596b5");
        assert_eq!(hash("D"), "71d5c413478ad06aaa1cbdd9e91c53f0cb4cef1dbd4c51bf3d473bccc6a5a272");
        assert_eq!(hash("F"), "fb53aa3a9f7e74029f755d774064eedd240ab90d8fc64723f97c31b3d2846d40");
        assert_eq!(hash("G"), "f856edce121db4121b5f1af400b1c471ebbfa6579a042aca9a34ae31c15f5c86");
        // E spells "Cafe" decomposed; NFC makes it D.
        assert_eq!(hash("E"), hash("D"));
        // Each shown member moves the hash.
        let distinct = ["A", "B", "C", "D", "F", "G"].map(hash);
        assert_eq!(distinct.iter().collect::<std::collections::BTreeSet<_>>().len(), 6);
    }

    #[test]
    fn options_hash_needs_nfc_to_equal_across_spellings() {
        // Without NFC the decomposed spelling hashes differently: this is what E pins.
        use sha2::{Digest, Sha256};
        let vectors = vectors();
        let raw = |tag: &str| {
            let vector = vectors.iter().find(|vector| vector["heimdall"] == tag).unwrap();
            let json = serde_json::json!({
                "context": vector["context"], "depends": vector["depends"], "options": vector["options"],
                "recommendation": vector["recommendation"], "title": vector["title"],
            });
            Sha256::digest(serde_json::to_string(&json).unwrap().as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect::<String>()
        };
        assert_ne!(raw("E"), raw("D"));
        assert_eq!(raw("D"), options_hash(&shown(vectors.iter().find(|vector| vector["heimdall"] == "D").unwrap())).0);
    }

    // ---- the fix batch on PR #29 ----

    /// Writes `answer` the way only the desk door can, for a state the public path cannot reach
    /// (an answer written under a card that has since changed).
    fn plant(mind: &mut Mind<MemoryStore>, answer: PipelineDeskAnswer) {
        let envelope = crate::fixtures::prepare(&D::DeskAnswer(answer));
        let instance = mind.instance().clone();
        let outcome = mind.admit_steps(Door::Desk, &instance, provenance(Faculty::Operator), vec![envelope], now());
        committed(outcome.unwrap());
    }

    fn at_offset(seconds: i64) -> String {
        (now() + Duration::seconds(seconds)).to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    #[test]
    fn the_decider_list_decides_and_the_attribution_does_not() {
        let at = "2026-11-10T12:00:00Z";
        let mut mind = desk();
        // The operator's own account, off the list, is no decider.
        let operator_off = stamp_as("op", DeskAuthority::Operator, &["x"]);
        assert_eq!(stored(ask(&mut mind, &operator_off, pick(FORK, "Q1", "A", at))).1, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: None });
        // A plain member on the list decides.
        let member_on = stamp_as("m", DeskAuthority::Member, &["m"]);
        assert_eq!(stored(ask(&mut mind, &member_on, pick(FORK, "Q1", "A", at))).1, DeskOutcome::Counted);
        // An empty list admits no one.
        let empty = stamp_as("m", DeskAuthority::Operator, &[]);
        assert_eq!(stored(ask(&mut mind, &empty, pick(FORK2, "Q2", "A", at))).1, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: None });
    }

    #[test]
    fn the_receipt_names_the_operator_only_for_the_operator_account() {
        let at = "2026-11-10T12:00:00Z";
        let mut mind = desk();
        for (authority, faculty) in [(DeskAuthority::Operator, Faculty::Operator), (DeskAuthority::Steward, Faculty::Steward), (DeskAuthority::Member, Faculty::Member)] {
            let outcome = ask(&mut mind, &stamp_as("x", authority, &LIST), pick(FORK, "Q1", "A", at));
            let DeskAnswerOutcome::Stored { receipt_id, .. } = outcome else { panic!("{outcome:?}") };
            let receipt = mind.receipts().unwrap().into_iter().find(|receipt| receipt.receipt_id == receipt_id).unwrap();
            assert_eq!(receipt.provenance.faculty, faculty, "{authority:?}");
            assert_eq!(receipt.provenance.agent, s("x"));
        }
    }

    #[test]
    fn answers_under_an_older_card_neither_count_nor_can_be_retracted() {
        let mut mind = desk();
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        let mut old = held(&mind, &first);
        old.sequence = 2;
        old.options_hash = Sha256Hex("e".repeat(64));
        old.request_id = fresh_id();
        old.outcome = DeskOutcome::Counted;
        plant(&mut mind, old);
        // Retract the current answer, so only the older-card one remains.
        stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first)));
        assert_eq!(counted(&mind, FORK), None, "an answer under an older hash does not count");
        let older = key("Q1", 2);
        // R1: a retraction names an answer under the current card only.
        assert_eq!(stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &older))).1, DeskOutcome::Refused { code: DeskRefusal::BadRetraction, retracts: Some(s(&older)) });
        // The next answer is Counted, not a Revision of the older one.
        let outcome = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "B", "2026-11-10T12:00:00Z"))).1;
        assert_eq!(outcome, DeskOutcome::Counted);
    }

    #[test]
    fn received_at_is_the_minds_and_a_claim_over_five_minutes_ahead_is_flagged() {
        let mut mind = desk();
        for (offset, flagged) in [(-86_400, false), (0, false), (300, false), (301, true)] {
            let claim = at_offset(offset);
            let (answer, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", &claim)));
            let record = held(&mind, &answer);
            assert_eq!((record.claim_in_future, record.answered_at, record.received_at), (flagged, s(&claim), s("2026-09-16T12:00:00Z")), "{offset}");
        }
    }

    #[test]
    fn a_note_is_one_to_four_thousand_bytes_and_not_blank() {
        let at = "2026-11-10T12:00:00Z";
        let with_note = |mind: &mut Mind<MemoryStore>, choice: Option<&str>, note: String| {
            let mut request = pick(FORK, "Q1", "A", at);
            request.body = DeskBody::Answer { choice: choice.map(s), note: Some(Words(note)) };
            ask(mind, &decider("x"), request)
        };
        let invalid = DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.note".into() });
        let mut mind = desk();
        let before = head(&mind);
        for note in ["".to_string(), "   ".into(), "\u{3000}\n".into(), "x".repeat(4001), "\u{1F600}".repeat(1001), "e\u{301}".repeat(1334)] {
            // With a choice or without: the note alone sinks the body, and nothing is stored.
            assert_eq!(with_note(&mut mind, None, note.clone()), invalid, "{} bytes", note.len());
            assert_eq!(with_note(&mut mind, Some("A"), note.clone()), invalid, "{} bytes", note.len());
        }
        assert_eq!(head(&mind), before);
        for note in ["x".repeat(4000), "\u{1F600}".repeat(1000), format!("{}x", "e\u{301}".repeat(1333)), " a ".to_string()] {
            assert!(matches!(stored(with_note(&mut mind, None, note)).1, DeskOutcome::Counted | DeskOutcome::Revision { .. }));
        }
    }

    #[test]
    fn a_retraction_after_a_ruling_closed_the_fork_is_fork_closed() {
        // Binding V10a: a ruling that answers the fork closes it; the retraction meets step 6.
        let mut mind = desk();
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", "2026-11-10T12:00:00Z")));
        let mut ruled = ruling("RQ1");
        ruled.answers = Some(s(FORK));
        ruled.choice = Some(l("A"));
        committed(admit(&mut mind, vec![D::Ruling(ruled)]));
        let (retraction, outcome) = stored(ask(&mut mind, &decider("y"), retract(FORK, "Q1", &first)));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::ForkClosed, retracts: Some(s(&first)) });
        assert_eq!(held(&mind, &retraction).outcome, outcome);
        assert_eq!(counted(&mind, FORK), Some(first), "the answer still stands");
    }

    /// Two spellings of "café" hash alike in every member that may carry one.
    #[test]
    fn options_hash_normalises_context_option_text_and_depends() {
        let hash_with = |set: &dyn Fn(&mut PipelineQuestion, &str), spelling: &str| {
            let mut question = question_with("Q1", &["A", "B"], "A");
            set(&mut question, spelling);
            options_hash(&question)
        };
        type Setter = fn(&mut PipelineQuestion, &str);
        let members: [(&str, Setter); 4] = [
            ("title", |q, s| q.title = s.into()),
            ("context", |q, s| q.question = s.into()),
            ("option text", |q, s| q.options[1].text = s.into()),
            ("depends", |q, s| q.depends = vec![s.into()]),
        ];
        for (member, set) in members {
            let (composed, decomposed) = ("caf\u{e9} \u{2603}", "cafe\u{301} \u{2603}");
            assert_eq!(hash_with(&set, composed), hash_with(&set, decomposed), "{member}");
            assert_ne!(hash_with(&set, composed), hash_with(&set, "cafe \u{2603}"), "{member} still moves the hash");
        }
    }

    fn replayed(outcome: DeskAnswerOutcome) -> (String, DeskOutcome) {
        match outcome {
            DeskAnswerOutcome::Replayed { answer, outcome } => (answer.0, outcome),
            other => panic!("expected a replay, got {other:?}"),
        }
    }

    fn with_id(mut request: DeskAnswerRequest, id: &str) -> DeskAnswerRequest {
        request.request_id = RequestId(id.into());
        request
    }

    const R1: &str = "6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f13";
    const R2: &str = "6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f14";
    const R3: &str = "6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f15";
    const AT: &str = "2026-11-10T12:00:00Z";

    /// Binding V12 (Q1): a retry returns the stored outcome, Refused included, and writes nothing.
    #[test]
    fn q1_v12_a_retry_replays_the_stored_outcome_and_adds_nothing() {
        let mut mind = desk();
        let request = with_id(pick(FORK, "Q1", "A", AT), R1);
        assert_eq!(stored(ask(&mut mind, &decider("x"), request.clone())), (key("Q1", 1), DeskOutcome::Counted));
        let after_first = head(&mind);
        assert_eq!(replayed(ask(&mut mind, &decider("x"), request)), (key("Q1", 1), DeskOutcome::Counted));
        assert_eq!(head(&mind), after_first, "no n2 and no Revision");
        // A stale-hash request replays as the same Refused stale_fork.
        let mut stale = with_id(pick(FORK, "Q1", "A", AT), R2);
        stale.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        let first = stored(ask(&mut mind, &decider("x"), stale.clone()));
        assert_eq!(first.1, DeskOutcome::Refused { code: DeskRefusal::StaleFork, retracts: None });
        let after_stale = head(&mind);
        assert_eq!(replayed(ask(&mut mind, &decider("x"), stale)), first);
        // A Revision replays as the Revision it was, not as a new one.
        let revision = with_id(pick(FORK, "Q1", "B", AT), R3);
        let (n, outcome) = stored(ask(&mut mind, &decider("x"), revision.clone()));
        assert_eq!(outcome, DeskOutcome::Revision { of: s(&key("Q1", 1)) });
        let after_revision = head(&mind);
        assert!(after_revision > after_stale);
        assert_eq!(replayed(ask(&mut mind, &decider("x"), revision)), (n, outcome));
        assert_eq!(head(&mind), after_revision);
        // A retraction replays too.
        let retraction = with_id(retract(FORK, "Q1", &key("Q1", 1)), "6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f16");
        let (r, outcome) = stored(ask(&mut mind, &decider("y"), retraction.clone()));
        assert_eq!(replayed(ask(&mut mind, &decider("y"), retraction)), (r, outcome));
    }

    /// V12's lookup precedes the fork lookup: a retry gets its stored outcome after the campaign moved.
    #[test]
    fn q1_the_lookup_comes_before_the_fork_is_read() {
        let mut mind = desk();
        let request = with_id(pick(FORK, "Q1", "A", AT), R1);
        stored(ask(&mut mind, &decider("x"), request.clone()));
        committed(admit(&mut mind, vec![resolution(r(K::Campaign, "eureka-state:campaign:self"), ResolutionOutcome::Moved { to: slug("thought-cage") })]));
        assert_eq!(replayed(ask(&mut mind, &decider("x"), request)), (key("Q1", 1), DeskOutcome::Counted));
    }

    /// The lookup precedes the probe: a bound id replays its stored outcome without the probe's
    /// validation. The stored answerer is rewritten past a Short, which no door can store but a
    /// fresh probe refuses; a retry of it must still replay, and an unbound id must meet the probe.
    #[test]
    fn q1_the_lookup_comes_before_the_probe_validates() {
        let long = "w".repeat(201);
        let request = with_id(pick(FORK, "Q1", "A", AT), R1);
        let mut first = desk();
        stored(ask(&mut first, &decider("x"), request.clone()));
        let store = MemoryStore::new();
        first.store().rows().into_iter().for_each(|row| store.plant(row));
        let cache = crate::mind::schema_cache().unwrap();
        let image = first.image().unwrap();
        let found = Docs::at(image, image.head).find(K::DeskAnswer, &key("Q1", 1)).cloned();
        let Some(PipelineDocument::DeskAnswer(mut held)) = found else { panic!("stored") };
        held.answerer = s(&long);
        store.plant(PipelineDocument::DeskAnswer(held).prepare(&cache).unwrap());
        let mut mind = opened(store, INSTANCE);
        let stamp = stamp_as(&long, DeskAuthority::Member, &LIST);
        let before = head(&mind);
        assert_eq!(replayed(ask(&mut mind, &stamp, request)), (key("Q1", 1), DeskOutcome::Counted));
        assert_eq!(head(&mind), before);
        let unbound = with_id(pick(FORK, "Q1", "A", AT), R2);
        assert_eq!(
            ask(&mut mind, &stamp, unbound),
            DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.answerer".into() })
        );
    }

    /// V13 (Q2): the same id with any other part of the body is request_id_conflict and the store is unchanged.
    #[test]
    fn q2_v13_the_same_id_with_another_body_conflicts_and_stores_nothing() {
        let mut mind = desk();
        let sent = with_id(pick(FORK, "Q1", "A", AT), R1);
        stored(ask(&mut mind, &decider("x"), sent.clone()));
        let before = head(&mind);
        let conflict = DeskAnswerOutcome::Refused(MindRefusal::DeskRequestIdConflict { request_id: R1.into() });
        let mut other_choice = sent.clone();
        other_choice.body = DeskBody::Answer { choice: Some(s("B")), note: None };
        let mut only_the_time = sent.clone();
        only_the_time.answered_at = s("2026-11-10T12:00:01Z");
        let mut with_note = sent.clone();
        with_note.body = DeskBody::Answer { choice: Some(s("A")), note: Some("Also this.".into()) };
        let mut other_fork = with_id(pick(FORK2, "Q2", "A", AT), R1);
        other_fork.answered_at = s(AT);
        let mut other_hash = sent.clone();
        other_hash.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        let mut as_retraction = sent.clone();
        as_retraction.body = DeskBody::Retraction { retracts: s(&key("Q1", 1)) };
        for request in [other_choice, only_the_time, with_note, other_fork, other_hash, as_retraction] {
            assert_eq!(ask(&mut mind, &decider("x"), request), conflict);
            assert_eq!(head(&mind), before);
        }
        // A retraction's id conflicts with another target.
        let retraction = with_id(retract(FORK, "Q1", &key("Q1", 1)), R2);
        stored(ask(&mut mind, &decider("y"), retraction.clone()));
        let mut other_target = retraction;
        other_target.body = DeskBody::Retraction { retracts: s(&key("Q1", 2)) };
        let settled = head(&mind);
        assert_eq!(ask(&mut mind, &decider("y"), other_target), DeskAnswerOutcome::Refused(MindRefusal::DeskRequestIdConflict { request_id: R2.into() }));
        assert_eq!(head(&mind), settled);
    }

    /// V14 (Q3): the key is (answerer, request_id); an identical body from another account is its own request.
    #[test]
    fn q3_v14_another_account_with_the_same_id_and_body_is_its_own_request() {
        let mut mind = desk();
        let sent = with_id(pick(FORK, "Q1", "A", AT), R1);
        assert_eq!(stored(ask(&mut mind, &decider("x"), sent.clone())), (key("Q1", 1), DeskOutcome::Counted));
        let (y_answer, y_outcome) = stored(ask(&mut mind, &decider("y"), sent.clone()));
        assert_eq!((y_answer.as_str(), &y_outcome), (key("Q1", 2).as_str(), &DeskOutcome::Revision { of: s(&key("Q1", 1)) }));
        assert_eq!(held(&mind, &y_answer).answerer, s("y"));
        assert_eq!(held(&mind, &key("Q1", 1)).answerer, s("x"));
        // x's later retry still returns n1; y's returns n2.
        assert_eq!(replayed(ask(&mut mind, &decider("x"), sent.clone())).0, key("Q1", 1));
        assert_eq!(replayed(ask(&mut mind, &decider("y"), sent)).0, key("Q1", 2));
        // x's id never reveals x's refusal to y: x off the list is refused, y with the identical body is Counted.
        let mut fresh = desk();
        let sent = with_id(pick(FORK, "Q1", "A", AT), R2);
        assert_eq!(stored(ask(&mut fresh, &off(), sent.clone())).1, DeskOutcome::Refused { code: DeskRefusal::NotADecider, retracts: None });
        assert_eq!(stored(ask(&mut fresh, &decider("y"), sent)), (key("Q1", 2), DeskOutcome::Counted));
    }

    /// V15 (Q4), the Huginn half: a request Huginn never stored is a fresh answer on retry.
    #[test]
    fn q4_v15_a_request_never_stored_is_answered_fresh_on_retry() {
        let mut mind = desk();
        let request = with_id(pick(FORK, "Q1", "A", AT), R1);
        // The first attempt died before Huginn: nothing is bound, so the retry is Counted.
        assert_eq!(stored(ask(&mut mind, &decider("x"), request)), (key("Q1", 1), DeskOutcome::Counted));
    }

    /// V17 (Q6): a refusal that stores nothing binds no id.
    #[test]
    fn q6_v17_a_body_that_stored_nothing_leaves_its_id_free() {
        let mut mind = desk();
        let mut blank = with_id(pick(FORK, "Q1", "A", AT), R2);
        blank.body = DeskBody::Answer { choice: None, note: Some("  ".into()) };
        assert!(matches!(ask(&mut mind, &decider("x"), blank), DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { .. })));
        assert_eq!(stored(ask(&mut mind, &decider("x"), with_id(pick(FORK, "Q1", "A", AT), R2))).1, DeskOutcome::Counted);
        let unknown = with_id(pick("eureka-state:question:nope", "Q1", "A", AT), R3);
        assert!(matches!(ask(&mut mind, &decider("x"), unknown), DeskAnswerOutcome::Refused(MindRefusal::MissingReference { .. })));
        assert!(matches!(stored(ask(&mut mind, &decider("x"), with_id(pick(FORK2, "Q2", "A", AT), R3))).1, DeskOutcome::Counted));
    }

    /// V18: an id that is not a lowercase D-format UUID is refused (also in the shape table above).
    #[test]
    fn v18_request_ids_must_be_lowercase_d_format_uuids() {
        for bad in ["", "retry-1", "6F1C0A52-3B7E-4D1A-9C58-0E2D7A4B9F13", "6f1c0a523b7e4d1a9c580e2d7a4b9f13", "{6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f13}", "6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f1g", " 6f1c0a52-3b7e-4d1a-9c58-0e2d7a4b9f13"] {
            let mut mind = desk();
            let before = head(&mind);
            assert_eq!(
                ask(&mut mind, &decider("x"), with_id(pick(FORK, "Q1", "A", AT), bad)),
                DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.request_id".into() }),
                "{bad:?}"
            );
            assert_eq!(head(&mind), before);
        }
    }

    // ---- fix batch 3 ----

    /// Faculty is attribution: a batch admitted under any faculty, Member and Steward included,
    /// gets the outcome Hands gets, and no faculty opens the desk-answer door.
    #[test]
    fn member_and_steward_confer_no_admission_right() {
        let all = [Faculty::SelfFaculty, Faculty::Imagination, Faculty::Hands, Faculty::Soul, Faculty::Life, Faculty::Eyes, Faculty::Operator, Faculty::Member, Faculty::Steward];
        let outcome_of = |faculty: Faculty| {
            let mut mind = desk();
            let mut answer = held_after_one(&mut mind);
            answer.request_id = fresh_id();
            answer.sequence = 2;
            let refused = admit_as(&mut mind, faculty, vec![D::DeskAnswer(answer)]);
            let ordinary = admit_as(&mut mind, faculty, vec![question("Q9", &["A", "B"], "A")]);
            (format!("{refused:?}"), matches!(ordinary, PipelineAdmissionOutcome::Committed { .. }))
        };
        let baseline = outcome_of(Faculty::Hands);
        assert!(baseline.1 && baseline.0.contains("Refused"), "{baseline:?}");
        for faculty in all {
            assert_eq!(outcome_of(faculty), baseline, "{faculty:?}");
        }
    }

    fn held_after_one(mind: &mut Mind<MemoryStore>) -> PipelineDeskAnswer {
        let (answer, _) = stored(ask(mind, &decider("x"), pick(FORK, "Q1", "A", AT)));
        held(mind, &answer)
    }

    /// The digest form is frozen: a fixed request digests to a fixed hex, computed outside this code.
    #[test]
    fn body_digest_is_pinned() {
        let request = |body: DeskBody| DeskAnswerRequest {
            fork: s("eureka-state:question:Q1"),
            options_hash: Sha256Hex("a".repeat(64)),
            body,
            answered_at: s("2026-11-10T12:00:00Z"),
            request_id: RequestId(R1.into()),
        };
        let answer = request(DeskBody::Answer { choice: Some(s("A")), note: None });
        assert_eq!(body_digest(&answer).0, "8a4d5b2ed0b3452bfed2f3a276b2828974e6824a3edcc643cfe9cfcdda34173a");
        let retraction = request(DeskBody::Retraction { retracts: s("eureka-state:desk_answer:Q1.1") });
        assert_eq!(body_digest(&retraction).0, "9f8075382556898c0bea364ed7c6deb2a86d8c78288a1f34e06da6e9bf5f58a3");
        // The illegal form covers the wire flag: (i) a retraction carrying an answer, (iii) a plain
        // answer carrying retracts, (ii) a retraction naming no target.
        let target = Some(s("eureka-state:desk_answer:Q1.n1"));
        let illegal = |choice: Option<&str>, retracts: Option<Short>, retracted: bool| {
            request(DeskBody::IllegalRetraction { choice: choice.map(s), note: None, retracts, retracted })
        };
        let carried = body_digest(&illegal(Some("A"), target.clone(), true));
        let plain_with_target = body_digest(&illegal(Some("A"), target.clone(), false));
        assert_eq!(carried.0, "935cab1a0ae34606b809d8ebca7cffa2889d20b3585d2622d32153a4d6898af1");
        assert_eq!(plain_with_target.0, "82b24cccb4ed2535a549f95f844fcd935074c3712b5b832f0b2cbd89c71fd9a6");
        assert_eq!(body_digest(&illegal(None, None, true)).0, "34ef86ed63b9318459965d4b85e794d2636ffc15736da866036b087c9c4a30eb");
        assert_ne!(carried, plain_with_target);
        // The id is not part of the body; answered_at is.
        let mut other_id = answer.clone();
        other_id.request_id = RequestId(R2.into());
        assert_eq!(body_digest(&other_id), body_digest(&answer));
        let mut other_time = answer.clone();
        other_time.answered_at = s("2026-11-10T12:00:01Z");
        assert_ne!(body_digest(&other_time), body_digest(&answer));
    }

    /// The retry lookup follows the shape check: a malformed body reusing a bound id is
    /// desk_request_invalid, never a conflict, and stores nothing.
    #[test]
    fn the_lookup_comes_after_the_shape_check() {
        let mut mind = desk();
        stored(ask(&mut mind, &decider("x"), with_id(pick(FORK, "Q1", "A", AT), R1)));
        let before = head(&mind);
        let mut blank = with_id(pick(FORK, "Q1", "A", AT), R1);
        blank.body = DeskBody::Answer { choice: None, note: None };
        let mut bad_time = with_id(pick(FORK, "Q1", "A", AT), R1);
        bad_time.answered_at = s("yesterday");
        let mut bad_fork = with_id(pick(FORK, "Q1", "A", AT), R1);
        bad_fork.fork = s("not-a-question");
        for (request, field) in [(blank, "body"), (bad_time, "answered_at"), (bad_fork, "fork")] {
            assert_eq!(ask(&mut mind, &decider("x"), request), DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: field.into() }), "{field}");
        }
        assert_eq!(head(&mind), before);
    }

    /// claim_in_future compares at sub-second precision: 300.0 s ahead is not flagged, 300.5 s is.
    #[test]
    fn claim_in_future_is_decided_below_the_second() {
        let mut mind = desk();
        for (claim, flagged) in [("2026-09-16T12:05:00.000Z", false), ("2026-09-16T12:05:00.500Z", true), ("2026-09-16T12:04:59.900Z", false)] {
            let (answer, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", claim)));
            assert_eq!(held(&mind, &answer).claim_in_future, flagged, "{claim}");
        }
    }

    fn code_of(outcome: DeskAnswerOutcome) -> DeskRefusal {
        match stored(outcome).1 {
            DeskOutcome::Refused { code, .. } => code,
            other => panic!("{other:?}"),
        }
    }

    /// R1: the wire's three illegal retraction forms are a stored `bad_retraction`, after steps 4 to 6.
    #[test]
    fn an_illegal_retraction_form_is_a_stored_bad_retraction() {
        let mut mind = desk();
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", AT)));
        let form = |choice: Option<&str>, note: Option<&str>, retracts: Option<&str>, retracted: bool| {
            let mut request = pick(FORK, "Q1", "A", AT);
            request.body = DeskBody::IllegalRetraction { choice: choice.map(s), note: note.map(Into::into), retracts: retracts.map(s), retracted };
            request
        };
        for (what, request, sent) in [
            ("a retraction carrying an answer", form(Some("A"), Some("why"), Some(&first), true), (Some("A"), Some("why"), Some(first.as_str()))),
            ("a retraction lacking retracts", form(None, None, None, true), (None, None, None)),
            ("a plain answer carrying retracts", form(Some("A"), None, Some(&first), false), (Some("A"), None, Some(first.as_str()))),
        ] {
            let before = head(&mind);
            let id = request.request_id.clone();
            let (answer, outcome) = stored(ask(&mut mind, &decider("y"), request.clone()));
            assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::BadRetraction, retracts: sent.2.map(|target| s(target)) }, "{what}");
            assert_eq!(head(&mind), before + 1, "{what} is stored");
            // The record keeps what was sent: the choice, the note and the target it named.
            let record = held(&mind, &answer);
            assert_eq!((record.choice, record.note, record.request_id), (sent.0.map(s), sent.1.map(Into::into), id), "{what}");
            // The stored refusal binds its request id and replays unchanged.
            assert_eq!(replayed(ask(&mut mind, &decider("y"), request)), (answer, outcome), "{what}");
            assert_eq!(head(&mind), before + 1, "{what} replay writes nothing");
        }
        assert_eq!(counted(&mind, FORK), Some(first));
        // Steps 4 to 6 come first: an off-list account is not_a_decider, a stale card is stale_fork.
        assert_eq!(code_of(ask(&mut mind, &off(), form(None, None, None, true))), DeskRefusal::NotADecider);
        let mut stale = form(None, None, None, true);
        stale.options_hash = options_hash(&question_with("Q1", &["A", "B"], "B"));
        assert_eq!(code_of(ask(&mut mind, &decider("y"), stale)), DeskRefusal::StaleFork);
    }

    /// The illegal form is its own body: the same id with the same fields as a legal answer conflicts.
    #[test]
    fn an_illegal_retraction_is_not_the_same_body_as_a_plain_answer() {
        let mut mind = desk();
        let plain = with_id(pick(FORK, "Q1", "A", AT), R1);
        stored(ask(&mut mind, &decider("x"), plain.clone()));
        let mut illegal = plain;
        illegal.body = DeskBody::IllegalRetraction { choice: Some(s("A")), note: None, retracts: None, retracted: true };
        assert_eq!(
            ask(&mut mind, &decider("x"), illegal),
            DeskAnswerOutcome::Refused(MindRefusal::DeskRequestIdConflict { request_id: R1.into() })
        );
    }

    /// An illegal form still has to be a well-formed body, and an unknown fork still stores nothing.
    #[test]
    fn a_malformed_illegal_retraction_or_unknown_fork_stores_nothing() {
        let mut mind = desk();
        let before = head(&mind);
        let illegal = |fork: &str, note: Option<&str>| {
            let mut request = pick(fork, "Q1", "A", AT);
            request.body = DeskBody::IllegalRetraction { choice: None, note: note.map(Into::into), retracts: None, retracted: true };
            request
        };
        assert!(matches!(
            ask(&mut mind, &decider("x"), illegal("eureka-state:question:nope", None)),
            DeskAnswerOutcome::Refused(MindRefusal::MissingReference { .. })
        ));
        assert_eq!(
            ask(&mut mind, &decider("x"), illegal(FORK, Some("  "))),
            DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.note".into() })
        );
        assert_eq!(head(&mind), before);
    }

    /// Step 7: a choice that is no option label is a stored `choice_invalid`, whatever it looks like;
    /// an empty or oversize one is a malformed body and stores nothing.
    #[test]
    fn a_choice_that_is_no_label_is_a_stored_choice_invalid() {
        let mut mind = desk();
        let ask_choice = |mind: &mut Mind<MemoryStore>, choice: &str| {
            let mut request = pick(FORK, "Q1", "A", AT);
            request.body = DeskBody::Answer { choice: Some(s(choice)), note: None };
            let id = request.request_id.clone();
            (ask(mind, &decider("x"), request.clone()), request, id)
        };
        for choice in ["has space", "ä", "A/B", "a", &"x".repeat(65), &"x".repeat(200)] {
            let before = head(&mind);
            let (outcome, request, id) = ask_choice(&mut mind, choice);
            let (answer, outcome) = stored(outcome);
            assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::ChoiceInvalid, retracts: None }, "{choice}");
            assert_eq!(head(&mind), before + 1);
            let record = held(&mind, &answer);
            assert_eq!((record.choice, record.request_id), (Some(s(choice)), id), "{choice} is kept as sent");
            assert_eq!(replayed(ask(&mut mind, &decider("x"), request)), (answer, outcome));
            assert_eq!(head(&mind), before + 1);
        }
        // Rule order: a stale card or an off-list account outranks it.
        let mut off_list = pick(FORK, "Q1", "A", AT);
        off_list.body = DeskBody::Answer { choice: Some(s("has space")), note: None };
        assert_eq!(code_of(ask(&mut mind, &off(), off_list)), DeskRefusal::NotADecider);
        assert_eq!(counted(&mind, FORK), None);
        // Malformed: stores nothing and binds nothing.
        let before = head(&mind);
        let mut last = None;
        for choice in ["", &"x".repeat(201)] {
            let (outcome, request, _) = ask_choice(&mut mind, choice);
            assert_eq!(outcome, DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.choice".into() }));
            assert_eq!(head(&mind), before);
            last = Some(request);
        }
        let mut fixed = last.unwrap();
        fixed.body = DeskBody::Answer { choice: Some(s("A")), note: None };
        assert!(matches!(ask(&mut mind, &decider("x"), fixed), DeskAnswerOutcome::Stored { .. }), "its id stayed free");
        assert_eq!(head(&mind), before + 1);
    }

    /// claim_in_future is decided against the stored received_at (whole seconds), not the clock
    /// reading: at 12:00:00.6 the stored instant is 12:00:00, so a claim at 12:05:00.2 is 300.2 s
    /// ahead of what the record says and is flagged, though 299.6 s ahead of the clock.
    #[test]
    fn claim_in_future_follows_the_stored_received_at() {
        let mut mind = desk();
        let clock = now() + Duration::milliseconds(600);
        for (claim, flagged) in [("2026-09-16T12:05:00.000Z", false), ("2026-09-16T12:05:00.200Z", true), ("2026-09-16T12:05:00.600Z", true)] {
            let (answer, _) = stored(mind.answer_desk(decider("x"), pick(FORK, "Q1", "A", claim), clock));
            let record = held(&mind, &answer);
            assert_eq!((record.received_at, record.claim_in_future), (s("2026-09-16T12:00:00Z"), flagged), "{claim}");
        }
    }

    fn illegal_form(choice: Option<&str>, retracts: Option<&str>, retracted: bool) -> DeskAnswerRequest {
        let mut request = pick(FORK, "Q1", "A", AT);
        request.body = DeskBody::IllegalRetraction { choice: choice.map(s), note: None, retracts: retracts.map(s), retracted };
        request
    }

    /// The illegal digest covers the wire flag: forms (i) and (iii) with equal fields are different bodies.
    #[test]
    fn the_illegal_digest_covers_retracted() {
        let mut mind = desk();
        let target = key("Q1", 1);
        let (answer, outcome) = stored(ask(&mut mind, &decider("x"), with_id(illegal_form(Some("A"), Some(&target), false), R1)));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::BadRetraction, retracts: Some(s(&target)) });
        let before = head(&mind);
        assert_eq!(
            ask(&mut mind, &decider("x"), with_id(illegal_form(Some("A"), Some(&target), true), R1)),
            DeskAnswerOutcome::Refused(MindRefusal::DeskRequestIdConflict { request_id: R1.into() })
        );
        assert_eq!(head(&mind), before);
        assert_eq!(replayed(ask(&mut mind, &decider("x"), with_id(illegal_form(Some("A"), Some(&target), false), R1))), (answer, outcome));
        assert_eq!(head(&mind), before);
    }

    /// A value that is no wire form is refused before the digest and the lookup, and binds nothing.
    #[test]
    fn a_value_that_is_no_illegal_form_is_body_invalid() {
        let mut mind = desk();
        let before = head(&mind);
        let target = key("Q1", 1);
        for (what, request) in [
            ("a plain answer", illegal_form(Some("A"), None, false)),
            ("a target and no answer, flag false", illegal_form(None, Some(&target), false)),
            ("a legal retraction", illegal_form(None, Some(&target), true)),
        ] {
            let request = with_id(request, R1);
            assert_eq!(
                ask(&mut mind, &decider("x"), request),
                DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "body".into() }),
                "{what}"
            );
            assert_eq!(head(&mind), before, "{what}");
        }
        assert!(matches!(ask(&mut mind, &decider("x"), with_id(pick(FORK, "Q1", "A", AT), R1)), DeskAnswerOutcome::Stored { .. }), "the id stayed free");
    }

    /// S5: a refused target is kept as sent up to 200 bytes; past that the body is malformed.
    #[test]
    fn an_illegal_target_is_at_most_200_bytes() {
        let mut mind = desk();
        let long = "x".repeat(200);
        let (answer, outcome) = stored(ask(&mut mind, &decider("x"), illegal_form(Some("A"), Some(&long), false)));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::BadRetraction, retracts: Some(s(&long)) });
        assert_eq!(held(&mind, &answer).choice, Some(s("A")));
        let before = head(&mind);
        assert_eq!(
            ask(&mut mind, &decider("x"), illegal_form(Some("A"), Some(&"x".repeat(201)), false)),
            DeskAnswerOutcome::Refused(MindRefusal::DeskRequestInvalid { field: "desk_answer.outcome.retracts".into() })
        );
        assert_eq!(head(&mind), before);
    }

    /// M2: steps 4 to 6 precede R1, so an illegal form on a closed fork is fork_closed.
    #[test]
    fn an_illegal_form_on_a_closed_fork_is_fork_closed() {
        let mut mind = desk();
        let (first, _) = stored(ask(&mut mind, &decider("x"), pick(FORK, "Q1", "A", AT)));
        let mut ruled = ruling("RQ1");
        ruled.answers = Some(s(FORK));
        ruled.choice = Some(l("A"));
        committed(admit(&mut mind, vec![D::Ruling(ruled)]));
        let (_, outcome) = stored(ask(&mut mind, &decider("y"), illegal_form(Some("A"), Some(&first), false)));
        assert_eq!(outcome, DeskOutcome::Refused { code: DeskRefusal::ForkClosed, retracts: Some(s(&first)) });
    }
}
