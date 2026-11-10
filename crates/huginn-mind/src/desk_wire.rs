//! The fork desk's answer wire, Huginn's half: Libby's answer-side documents
//! (CultLib `contracts/cultnet/governance/fork-card.md` and `fork-card-vectors.json`)
//! as Rust types, the one conversion between her `n<seq>` answer ids and the
//! record keys, the projection of a stored answer, the refusal mapping and the
//! showability predicate, and `Mind::desk_answer`, the one call that runs them in order.
//!
//! `Mind::answer_desk` stays the only owner of an answer's outcome, sequence,
//! request id lookup, replay and conflict; this module converts, projects and
//! maps, and never decides, recomputes or rewrites any of it. `counted_answer`
//! stays the only owner of "counts": the response builder calls it at the head
//! the response is read at, and nothing here stores it. Huginn writes
//! `gamecult.desk_answer.v0` and its own refusals; the stale-fork display
//! and the answer result are Bifrost's to attach and write. The envelope, the published
//! schemas, the reads and the socket are other cuts.

use chrono::{DateTime, Utc};
use eureka_pipeline::{
    DeskAuthority, DeskOutcome, DeskRefusal, PipelineDeskAnswer, PipelineDocument, PipelineKind, PipelineQuestion, RequestId, Sha256Hex, Short, Words,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::desk::{DeskAnswerOutcome, DeskAnswerRequest, DeskBody, DeskStamp, counted_answer};
use crate::docs::Docs;
use crate::mind::Mind;
use crate::refusal::MindRefusal;
use crate::rows::root_and_local;
use crate::store::MindStore;

/// The request document's schema id; any other value fails to decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RequestSchema {
    #[serde(rename = "gamecult.fork.answer_request.v0")]
    V0,
}

/// The desk answer document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AnswerSchema {
    #[serde(rename = "gamecult.desk_answer.v0")]
    V0,
}

/// The refusal document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RefusalSchema {
    #[serde(rename = "gamecult.fork.refusal.v0")]
    V0,
}

/// `gamecult.fork.answer_request.v0`: closed, her field order. The wire types hold text; the
/// shape step of `Mind::answer_desk` judges it. The subject and the attribution are the
/// door's context and are no field here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkAnswerRequest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: RequestSchema,
    pub fork_id: String,
    pub options_hash: String,
    pub answer: Option<ForkAnswer>,
    pub retracted: bool,
    pub retracts: Option<String>,
    pub answered_at: String,
    pub request_id: String,
}

/// The person's words: a choice, a note, or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkAnswer {
    pub choice: Option<String>,
    pub note: Option<String>,
}

/// The six codes a refused answer is stored with, as her schema spells them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForkAnswerCode {
    StaleFork,
    NotADecider,
    ForkClosed,
    ChoiceInvalid,
    BadRetraction,
    AlreadyRetracted,
}

impl From<DeskRefusal> for ForkAnswerCode {
    fn from(refusal: DeskRefusal) -> Self {
        match refusal {
            DeskRefusal::StaleFork => Self::StaleFork,
            DeskRefusal::NotADecider => Self::NotADecider,
            DeskRefusal::ForkClosed => Self::ForkClosed,
            DeskRefusal::ChoiceInvalid => Self::ChoiceInvalid,
            DeskRefusal::BadRetraction => Self::BadRetraction,
            DeskRefusal::AlreadyRetracted => Self::AlreadyRetracted,
        }
    }
}

/// `gamecult.desk_answer.v0`: one stored answer as the desk shows it, her field order,
/// `request_id` last. Every value is a stored fact; `answer_id`, `of` and `retracts` are the
/// record keys in her `n<seq>` spelling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkDeskAnswer {
    #[serde(rename = "schemaVersion")]
    pub schema_version: AnswerSchema,
    pub answer_id: String,
    pub fork_id: String,
    pub outcome: String,
    pub of: Option<String>,
    pub retracts: Option<String>,
    pub code: Option<ForkAnswerCode>,
    pub options_hash: String,
    pub choice: Option<String>,
    pub note: Option<String>,
    pub answered_at: String,
    pub received_at: String,
    pub claim_in_future: bool,
    pub answerer: String,
    pub authority: DeskAuthority,
    pub request_id: String,
}

/// The three requests Huginn refuses without storing, by code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForkRefusalCode {
    ForkUnknown,
    BodyInvalid,
    RequestIdConflict,
}

/// `gamecult.fork.refusal.v0`. `error` is a fixed sentence, never an echo of what was sent;
/// for `body_invalid` it names the field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkRefusal {
    #[serde(rename = "schemaVersion")]
    pub schema_version: RefusalSchema,
    pub code: ForkRefusalCode,
    pub error: String,
}

/// What the door answers one desk request with. `Answered` is a stored answer, refused ones
/// included (a stale fork, a non-decider and the rest are stored), fresh or replayed alike.
/// `NotStored` is a request that stored nothing. `Refused` is any other Mind refusal; an
/// `Unavailable` is Bifrost's to turn into its 503, and Huginn writes no document for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DeskResponse {
    Answered { answer: ForkDeskAnswer, counted_answer_id: Option<String>, as_of: u64 },
    NotStored(ForkRefusal),
    Refused(MindRefusal),
}

// ---- the one conversion between her answer ids and the record keys ----

/// The key prefix of one fork's answers. The only place a desk answer key is spelled.
fn prefix(fork: &str) -> String {
    let (root, local) = root_and_local(fork);
    format!("{root}:desk_answer:{local}.")
}

fn n_text(n: u32) -> String {
    format!("n{n}")
}

/// `n[1-9][0-9]*` within a `u32`.
fn parse_n(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('n')?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The key of `fork`'s `n`th answer.
fn answer_key(fork: &str, n: u32) -> String {
    format!("{}{}", prefix(fork), n_text(n))
}

/// The sequence a key of `fork`'s answers names; `None` for another fork's key or any other text.
fn answer_seq(fork: &str, key: &str) -> Option<u32> {
    parse_n(key.strip_prefix(prefix(fork).as_str())?)
}

// ---- request, refusals ----

fn refusal(code: ForkRefusalCode, error: String) -> ForkRefusal {
    ForkRefusal { schema_version: RefusalSchema::V0, code, error }
}

fn fork_unknown() -> ForkRefusal {
    refusal(ForkRefusalCode::ForkUnknown, "fork_unknown: no such fork".into())
}

fn body_invalid(field: &str) -> ForkRefusal {
    refusal(ForkRefusalCode::BodyInvalid, format!("body_invalid: the body does not satisfy gamecult.fork.answer_request.v0 ({field})"))
}

/// The Mind's request for a wire request, or the refusal that stores nothing. Which of the
/// four shapes the person sent is decided here from the three fields that carry it; whether
/// the words are within bounds is `answer_desk`'s shape step.
fn request_of(wire: &ForkAnswerRequest) -> Result<DeskAnswerRequest, ForkRefusal> {
    let retracts = match &wire.retracts {
        None => None,
        Some(text) => Some(Short(answer_key(&wire.fork_id, parse_n(text).ok_or_else(|| body_invalid("retracts"))?))),
    };
    let sent = wire.answer.as_ref();
    if sent.is_some_and(|words| words.choice.is_none() && words.note.is_none()) {
        return Err(body_invalid("answer"));
    }
    let choice = sent.and_then(|words| words.choice.clone()).map(Short);
    let note = sent.and_then(|words| words.note.clone()).map(Words);
    let body = match (sent.is_some(), wire.retracted, retracts) {
        (true, false, None) => DeskBody::Answer { choice, note },
        (false, true, Some(retracts)) => DeskBody::Retraction { retracts },
        (false, false, _) => return Err(body_invalid("answer")),
        (_, retracted, retracts) => DeskBody::IllegalRetraction { choice, note, retracts, retracted },
    };
    Ok(DeskAnswerRequest {
        fork: Short(wire.fork_id.clone()),
        options_hash: Sha256Hex(wire.options_hash.clone()),
        body,
        answered_at: Short(wire.answered_at.clone()),
        request_id: RequestId(wire.request_id.clone()),
    })
}

/// A key the door could not have stored. No door path writes one, so this is a fault of the
/// store, and it is `Unavailable`, never a wrong id.
fn unreadable_key() -> MindRefusal {
    MindRefusal::Unavailable { detail: "a stored desk answer names a key outside its fork's n<seq> form".into() }
}

/// The stored answer as Libby's document. Every value is stored; nothing is recomputed.
fn project(key: &str, stored: &PipelineDeskAnswer) -> Result<ForkDeskAnswer, MindRefusal> {
    let fork = stored.fork.0.as_str();
    let id = |key: &str| answer_seq(fork, key).map(n_text).ok_or_else(unreadable_key);
    let (of, retracts, code) = match &stored.outcome {
        DeskOutcome::Counted => (None, None, None),
        DeskOutcome::Revision { of } => (Some(id(&of.0)?), None, None),
        DeskOutcome::Retraction { retracts } => (None, Some(id(&retracts.0)?), None),
        DeskOutcome::Refused { code, retracts } => (None, retracts.as_ref().map(|target| id(&target.0)).transpose()?, Some((*code).into())),
    };
    Ok(ForkDeskAnswer {
        schema_version: AnswerSchema::V0,
        answer_id: id(key)?,
        fork_id: stored.fork.0.clone(),
        outcome: stored.outcome.name().into(),
        of,
        retracts,
        code,
        options_hash: stored.options_hash.0.clone(),
        choice: stored.choice.as_ref().map(|choice| choice.0.clone()),
        note: stored.note.as_ref().map(|note| note.0.clone()),
        answered_at: stored.answered_at.0.clone(),
        received_at: stored.received_at.0.clone(),
        claim_in_future: stored.claim_in_future,
        answerer: stored.answerer.0.clone(),
        authority: stored.authority,
        request_id: stored.request_id.0.clone(),
    })
}

/// The Mind's refusal as the door answers it: an unknown or moved fork, a body the shape step
/// refused and a reused request id store nothing and have codes of their own; anything else,
/// `Unavailable` included, passes through unchanged.
fn door_refusal(refused: MindRefusal) -> DeskResponse {
    match refused {
        MindRefusal::MissingReference { .. } | MindRefusal::CampaignMoved { .. } => DeskResponse::NotStored(fork_unknown()),
        MindRefusal::DeskRequestInvalid { field } if field == "fork" => DeskResponse::NotStored(fork_unknown()),
        MindRefusal::DeskRequestInvalid { field } => DeskResponse::NotStored(body_invalid(&field)),
        MindRefusal::DeskRequestIdConflict { .. } => {
            DeskResponse::NotStored(refusal(ForkRefusalCode::RequestIdConflict, "request_id_conflict: this request_id was used for a different body".into()))
        }
        other => DeskResponse::Refused(other),
    }
}

/// Whether a held question can be shown as a fork and so answered: a depends line and an option
/// text are never empty, there are at least two options and at least one decider.
fn showable(question: &PipelineQuestion, deciders: &[Short]) -> bool {
    !deciders.is_empty()
        && question.options.len() >= 2
        && question.options.iter().all(|option| !option.text.0.is_empty())
        && question.depends.iter().all(|line| !line.0.is_empty())
}

impl<S: MindStore> Mind<S> {
    /// One desk answer from the door: the wire request converted, the question gated, the answer
    /// decided and stored (or replayed) by `answer_desk`, and the stored answer projected with the
    /// derived count at the head it was read at. The flag is `true` when nothing was committed (a
    /// replay); it is for the door and is never encoded. `subject` and `attribution` are the verified
    /// caller's, `deciders` the door's list.
    pub fn desk_answer(
        &mut self,
        deciders: &[Short],
        subject: Short,
        attribution: DeskAuthority,
        request: ForkAnswerRequest,
        now: DateTime<Utc>,
    ) -> (DeskResponse, bool) {
        match self.desk_response(deciders, subject, attribution, &request, now) {
            Ok(done) => done,
            Err(refused) => (door_refusal(refused), false),
        }
    }

    fn desk_response(
        &mut self,
        deciders: &[Short],
        subject: Short,
        attribution: DeskAuthority,
        request: &ForkAnswerRequest,
        now: DateTime<Utc>,
    ) -> Result<(DeskResponse, bool), MindRefusal> {
        let sent = match request_of(request) {
            Ok(sent) => sent,
            Err(refused) => return Ok((DeskResponse::NotStored(refused), false)),
        };
        let image = self.image()?;
        let unshowable = matches!(
            Docs::at(image, image.head).find(PipelineKind::Question, &sent.fork.0),
            Some(PipelineDocument::Question(question)) if !showable(question, deciders)
        );
        if unshowable {
            return Ok((DeskResponse::NotStored(fork_unknown()), false));
        }
        let stamp = DeskStamp { answerer: subject, authority: attribution, deciders: deciders.to_vec() };
        let (key, replayed) = match self.answer_desk(stamp, sent, now) {
            DeskAnswerOutcome::Stored { answer, .. } => (answer.0, false),
            DeskAnswerOutcome::Replayed { answer, .. } => (answer.0, true),
            DeskAnswerOutcome::Refused(refused) => return Err(refused),
        };
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let Some(PipelineDocument::DeskAnswer(stored)) = docs.find(PipelineKind::DeskAnswer, &key) else { return Err(unreadable_key()) };
        let fork = stored.fork.0.as_str();
        let counted_answer_id = counted_answer(&docs, fork).map(|counted| answer_seq(fork, &counted).map(n_text).ok_or_else(unreadable_key)).transpose()?;
        Ok((DeskResponse::Answered { answer: project(&key, stored)?, counted_answer_id, as_of: image.head }, replayed))
    }
}
