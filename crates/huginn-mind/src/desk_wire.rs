//! The fork desk's answer wire, Huginn's half: Libby's answer-side documents
//! (CultLib `contracts/cultnet/governance/`, the contract file and its vectors at d3025d3c)
//! as Rust types, the one conversion between her `n<seq>` answer ids and the
//! record keys, the projection of a stored answer, the refusal mapping and the
//! showability predicate, and `Mind::desk_answer`, the one call that runs them in order.
//! The read side (card, record, entry and list, `Mind::desk_forks` and `Mind::desk_fork`)
//! is the child module `reads`.
//!
//! `Mind::answer_desk` stays the only owner of an answer's outcome, sequence,
//! request id lookup, replay and conflict; this module converts, projects and
//! maps, and never decides, recomputes or rewrites any of it. `counted_answer`
//! stays the only owner of "counts": the response builder calls it at the head
//! the response is read at, and nothing here stores it. Huginn writes
//! `gamecult.desk_answer.v0` and its own refusals; the stale-fork display
//! and the answer result are Bifrost's to attach and write. The request type, its
//! constants and the published schemas are here; the socket is the daemon's.

mod reads;
pub use reads::{CardSchema, CardStatus, EntrySchema, ForkCard, ForkCursor, ForkEntry, ForkLimit, ForkList, ForkOption, ForkRecord, ForkRef, ListSchema, ListStatus, RecordSchema};

use chrono::{DateTime, Utc};
use eureka_pipeline::{
    DeskAuthority, DeskOutcome, DeskRefusal, PipelineDeskAnswer, PipelineDocument, PipelineKind, PipelineQuestion, RequestId, Sha256Hex, Short, Slug, Words,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::desk::{DeskAnswerOutcome, DeskAnswerRequest, DeskBody, DeskStamp, bound_answer, counted_answer};
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

/// A nullable key her schema lists as required: it must be sent, as null if it has no value.
/// (serde would otherwise read a missing `Option` key as `None`.)
fn required<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

/// `gamecult.fork.answer_request.v0`: closed, her field order. The wire types hold text; the
/// shape step of `Mind::answer_desk` judges it. The subject and the authority are the
/// door's context and are no field here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkAnswerRequest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: RequestSchema,
    pub fork_id: String,
    pub options_hash: String,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub answer: Option<ForkAnswer>,
    pub retracted: bool,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub retracts: Option<String>,
    pub answered_at: String,
    pub request_id: String,
}

/// The person's words: a choice, a note, or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkAnswer {
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub choice: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
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
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub of: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub retracts: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub code: Option<ForkAnswerCode>,
    pub options_hash: String,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub choice: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub note: Option<String>,
    pub answered_at: String,
    pub received_at: String,
    pub claim_in_future: bool,
    /// The answering account, or nil in a non-decider's view (the key is kept). A stored answer always has one.
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub answerer: Option<String>,
    pub authority: DeskAuthority,
    pub request_id: String,
}

/// The refusals Huginn sends without storing, by code. `ForkTooLarge` is the list's alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForkRefusalCode {
    ForkUnknown,
    BodyInvalid,
    RequestIdConflict,
    ForkTooLarge,
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
/// `Forks` and `Fork` are the reads: one page of entries, or one entry, with the head they were
/// all read at. `NotStored` is a request that stored nothing (an unknown fork included).
/// `Refused` is any other Mind refusal; an `Unavailable` is Bifrost's to turn into its 503, and
/// Huginn writes no document for it.
///
/// The trust boundary of the reads (ruling `desk-read-visibility-withheld-ids`): `Forks` and `Fork`
/// carry every answer's note, to whatever the peer-checked socket carries; Bifrost strips notes
/// at its door (a stripped read serves `note: null`, CultLib `fork-card.md`). Huginn itself
/// withholds the account ids from a reader who is not a decider (`deciders` empty, `answerer` nil,
/// counts in their place). Huginn must be reachable only by Bifrost's peer-checked socket: the
/// peer-uid check IS the whole authentication, and a second reader of that socket is a note leak.
// One value per call, built and encoded once; a box would only add a deref to every reader.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DeskResponse {
    Answered { answer: ForkDeskAnswer, counted_answer_id: Option<String>, as_of: u64 },
    Forks { list: ForkList, as_of: u64 },
    Fork { entry: ForkEntry, as_of: u64 },
    NotStored(ForkRefusal),
    Refused(MindRefusal),
}

impl DeskResponse {
    /// The envelope's status: `rejected` exactly when nothing was stored or the Mind refused.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Answered { .. } | Self::Forks { .. } | Self::Fork { .. } => "accepted",
            Self::NotStored(_) | Self::Refused(_) => "rejected",
        }
    }
}

/// The envelope's `service_id` for the desk door.
pub const DESK_SERVICE_ID: &str = "huginn.desk";
pub const DESK_REQUEST_SCHEMA: &str = "huginn.desk_request.v1";
pub const DESK_RESPONSE_SCHEMA: &str = "huginn.desk_response.v1";

/// The published schemas, derived from `DeskRequest` and `DeskResponse` and pinned byte for byte
/// by `published_desk_schemas_match_derivation`.
pub const DESK_REQUEST_SCHEMA_JSON: &str = include_str!("../../../schemas/cultnet/huginn.desk_request.v1.schema.json");
pub const DESK_RESPONSE_SCHEMA_JSON: &str = include_str!("../../../schemas/cultnet/huginn.desk_response.v1.schema.json");

/// The frame bounds the door enforces, on the encoded CultNet message.
pub const DESK_MAX_REQUEST_BYTES: usize = 65536;
pub const DESK_MAX_RESPONSE_BYTES: usize = 65536;
/// What a page of forks leaves for the envelope around it: the echoed message id (at most 256
/// bytes), the operation, the runtime id and the frame's own keys. `Mind::desk_forks` fills a page
/// to `DESK_PAGE_BUDGET`, the response bound less this, measured on the bare message.
pub const DESK_ENVELOPE_MARGIN: usize = 1024;
pub const DESK_PAGE_BUDGET: usize = DESK_MAX_RESPONSE_BYTES - DESK_ENVELOPE_MARGIN;

/// Most deciders one call carries (CultLib `card.deciders` `maxItems`).
pub const DESK_DECIDERS_MAX: usize = 64;

/// A decider list, read whole or not at all: one to `DESK_DECIDERS_MAX` subjects, none twice. A
/// bad list is the decoder's refusal, which names the field and never quotes a subject.
fn deciders<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Short>, D::Error> {
    struct List;
    impl<'de> serde::de::Visitor<'de> for List {
        type Value = Vec<Short>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a list of distinct deciders")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<Short>, A::Error> {
            let mut list = Vec::new();
            while let Some(subject) = seq.next_element::<Short>()? {
                if list.len() == DESK_DECIDERS_MAX {
                    return Err(serde::de::Error::custom("too many deciders"));
                }
                if list.contains(&subject) {
                    return Err(serde::de::Error::custom("a decider is listed twice"));
                }
                list.push(subject);
            }
            if list.is_empty() {
                return Err(serde::de::Error::custom("no deciders"));
            }
            Ok(list)
        }
    }
    deserializer.deserialize_seq(List)
}

fn deciders_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "array",
        "items": generator.subschema_for::<Short>(),
        "minItems": 1,
        "maxItems": DESK_DECIDERS_MAX,
        "uniqueItems": true
    })
}

/// Every request the desk door answers. The fields are the wire names: the `mind` addressed
/// (the persona and pipeline requests say `instance`; the sets are separate), the verified
/// `subject` and `authority` Bifrost vouches for (door context, never body fields), and the
/// `deciders` of this call (1 to 64, distinct; the Mind never stores them). Closed: a key outside
/// the set refuses the whole payload.
///
/// The reads, `Forks` and `Fork`, carry NO `authority` (CultLib `list_request` and
/// `read_request` are closed and have none; one that names it is refused whole). Their `deciders`
/// feed the showability gate, and `subject` decides the view: a subject in `deciders` is a decider
/// and is told the ids; any other is told counts in their place. `Forks` takes the contract's `list_request` fields flat
/// beside them (`status`, `cursor`, `limit`), each key written, `null` where absent. Bifrost
/// strips notes; see `DeskResponse` for the trust boundary. These docs and the doc of `Mind::desk_forks`/`Mind::desk_fork` are the
/// reads spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum DeskRequest {
    Answer {
        mind: Slug,
        subject: Short,
        authority: DeskAuthority,
        #[serde(deserialize_with = "deciders")]
        #[schemars(schema_with = "deciders_schema")]
        deciders: Vec<Short>,
        request: ForkAnswerRequest,
    },
    Forks {
        mind: Slug,
        subject: Short,
        #[serde(deserialize_with = "deciders")]
        #[schemars(schema_with = "deciders_schema")]
        deciders: Vec<Short>,
        #[serde(deserialize_with = "required")]
        #[schemars(required)]
        status: Option<ListStatus>,
        #[serde(deserialize_with = "required")]
        #[schemars(required)]
        cursor: Option<ForkCursor>,
        limit: ForkLimit,
    },
    Fork {
        mind: Slug,
        subject: Short,
        #[serde(deserialize_with = "deciders")]
        #[schemars(schema_with = "deciders_schema")]
        deciders: Vec<Short>,
        fork_id: String,
    },
}

impl DeskRequest {
    /// The envelope's `operation`.
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Answer { .. } => "answer",
            Self::Forks { .. } => "forks",
            Self::Fork { .. } => "fork",
        }
    }

    /// The mind the request addresses, for `Mind::require_instance`.
    pub fn mind(&self) -> &Slug {
        match self {
            Self::Answer { mind, .. } | Self::Forks { mind, .. } | Self::Fork { mind, .. } => mind,
        }
    }

    pub fn deciders(&self) -> &[Short] {
        match self {
            Self::Answer { deciders, .. } | Self::Forks { deciders, .. } | Self::Fork { deciders, .. } => deciders,
        }
    }
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

fn fork_refusal(code: ForkRefusalCode, error: String) -> ForkRefusal {
    ForkRefusal { schema_version: RefusalSchema::V0, code, error }
}

fn fork_unknown() -> ForkRefusal {
    fork_refusal(ForkRefusalCode::ForkUnknown, "fork_unknown: no such fork".into())
}

/// A list page whose first entry alone is past the page budget: names that fork and its `order`. A
/// client continues with the cursor `c<order>`; at order 0 there is no such cursor and the list ends.
fn fork_too_large(fork: &str, order: u64) -> ForkRefusal {
    let onward = if order == 0 { "this is the end of the list".to_string() } else { format!("continue with cursor c{order}") };
    fork_refusal(ForkRefusalCode::ForkTooLarge, format!("fork_too_large: fork {fork} at order {order} is past the page budget by itself; {onward}"))
}

/// A READ of one fork whose entry is past the reply cap: names the fork, nothing else (a read has no
/// order to continue from and no cursor). The door raises it where it measures the reply.
pub fn fork_too_large_read(fork: &str) -> ForkRefusal {
    fork_refusal(ForkRefusalCode::ForkTooLarge, format!("fork_too_large: fork {fork} is past the response bound by itself"))
}

fn body_invalid(field: &str) -> ForkRefusal {
    fork_refusal(ForkRefusalCode::BodyInvalid, format!("body_invalid: the body does not satisfy gamecult.fork.answer_request.v0 ({field})"))
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
        answerer: Some(stored.answerer.0.clone()),
        authority: stored.authority,
        request_id: stored.request_id.0.clone(),
    })
}

/// The Mind's refusal as the door answers it: an unknown or moved fork (one that reads as no
/// question id, or whose id is over the 200-byte bound, names no fork), a body the shape step
/// refused and a reused request id store nothing and have codes of their own; anything else,
/// `Unavailable` included, passes through unchanged.
/// The Mind's `DeskRequestInvalid` fields that come from the fork id alone.
const FORK_FIELDS: [&str; 4] = ["fork", "desk_answer.fork", "desk_answer.key", "desk_answer.campaign"];

fn door_refusal(refused: MindRefusal) -> DeskResponse {
    match refused {
        MindRefusal::MissingReference { .. } | MindRefusal::CampaignMoved { .. } => DeskResponse::NotStored(fork_unknown()),
        // Any bad fork id is `fork_unknown`. The Mind derives these four fields from the fork id alone
        // (the id's shape, its campaign root, the record key built from it), so each one is the fork.
        MindRefusal::DeskRequestInvalid { field } if FORK_FIELDS.contains(&field.as_str()) => DeskResponse::NotStored(fork_unknown()),
        MindRefusal::DeskRequestInvalid { field } => DeskResponse::NotStored(body_invalid(&field)),
        MindRefusal::DeskRequestIdConflict { .. } => {
            DeskResponse::NotStored(fork_refusal(ForkRefusalCode::RequestIdConflict, "request_id_conflict: this request_id was used for a different body".into()))
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
    /// decided and stored (or committed) by `answer_desk`, and the stored answer projected with the
    /// derived count at the head it was read at. `subject` and `authority` are the verified
    /// caller's, `deciders` the door's list.
    pub fn desk_answer(
        &mut self,
        deciders: &[Short],
        subject: Short,
        authority: DeskAuthority,
        request: ForkAnswerRequest,
        now: DateTime<Utc>,
    ) -> DeskResponse {
        match self.desk_response(deciders, subject, authority, &request, now) {
            Ok(done) => done,
            Err(refused) => door_refusal(refused),
        }
    }

    fn desk_response(
        &mut self,
        deciders: &[Short],
        subject: Short,
        authority: DeskAuthority,
        request: &ForkAnswerRequest,
        now: DateTime<Utc>,
    ) -> Result<DeskResponse, MindRefusal> {
        let sent = match request_of(request) {
            Ok(sent) => sent,
            Err(refused) => return Ok(DeskResponse::NotStored(refused)),
        };
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        // A request already stored replays whatever the list is now: the lookup is `answer_desk`'s,
        // asked here only so the gate does not shadow it.
        let unshowable = bound_answer(&docs, &subject, &sent.request_id).is_none()
            && matches!(docs.find(PipelineKind::Question, &sent.fork.0), Some(PipelineDocument::Question(question)) if !showable(question, deciders));
        if unshowable {
            return Ok(DeskResponse::NotStored(fork_unknown()));
        }
        let stamp = DeskStamp { answerer: subject, authority, deciders: deciders.to_vec() };
        let key = match self.answer_desk(stamp, sent, now) {
            DeskAnswerOutcome::Stored { answer, .. } | DeskAnswerOutcome::Replayed { answer, .. } => answer.0,
            DeskAnswerOutcome::Refused(refused) => return Err(refused),
        };
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let Some(PipelineDocument::DeskAnswer(stored)) = docs.find(PipelineKind::DeskAnswer, &key) else { return Err(unreadable_key()) };
        let fork = stored.fork.0.as_str();
        let counted_answer_id = counted_answer(&docs, fork).map(|counted| answer_seq(fork, &counted).map(n_text).ok_or_else(unreadable_key)).transpose()?;
        Ok((DeskResponse::Answered { answer: project(&key, stored)?, counted_answer_id, as_of: image.head }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::options_hash as shown_hash;
    use crate::fixtures::*;
    use crate::store::test_stores::MemoryStore;
    use eureka_pipeline::QuestionOption;
    use serde_json::Value;
    use std::collections::HashMap;

    use PipelineDocument as D;

    const FORK: &str = "eureka-state:question:Q1";
    const FORK2: &str = "eureka-state:question:Q2";
    const LIST: [&str; 4] = ["x", "y", "z", "op"];
    const AT: &str = "2026-11-10T12:00:00Z";

    // ---- Libby's vectors: CultLib d3025d3c, `contracts/cultnet/governance/fork-card-vectors.json` ----

    fn vectors() -> Value {
        serde_json::from_str(include_str!("../vectors/fork-card-vectors.json")).unwrap()
    }

    fn bytes_of(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect()
    }

    fn encode<T: Serialize>(value: &T) -> Vec<u8> {
        rmp_serde::to_vec_named(value).unwrap()
    }

    /// The `{name, message, messagePackHex}` rows of one vector array.
    fn rows(array: &Value) -> Vec<(String, Value, Vec<u8>)> {
        array
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row["name"].as_str().unwrap().to_string(), row["message"].clone(), bytes_of(row["messagePackHex"].as_str().unwrap())))
            .collect()
    }

    // ---- a desk with two questions, and the helpers every test shares ----

    fn desk() -> Mind<MemoryStore> {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(1), question_n(2)]));
        mind
    }

    fn hash_of(mind: &Mind<MemoryStore>, fork: &str) -> String {
        let image = mind.image().unwrap();
        let docs = Docs::at(image, image.head);
        let Some(D::Question(question)) = docs.find(PipelineKind::Question, fork) else { panic!("no question {fork}") };
        shown_hash(question).0
    }

    fn fresh_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!("00000000-0000-4000-8000-{:012x}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// A plain answer, choice `A`, under a fresh request id.
    fn wire(fork: &str, hash: &str) -> ForkAnswerRequest {
        ForkAnswerRequest {
            schema_version: RequestSchema::V0,
            fork_id: fork.into(),
            options_hash: hash.into(),
            answer: Some(ForkAnswer { choice: Some("A".into()), note: None }),
            retracted: false,
            retracts: None,
            answered_at: AT.into(),
            request_id: fresh_id(),
        }
    }

    fn retraction(fork: &str, hash: &str, target: &str) -> ForkAnswerRequest {
        ForkAnswerRequest { answer: None, retracted: true, retracts: Some(target.into()), ..wire(fork, hash) }
    }

    fn deciders(list: &[&str]) -> Vec<Short> {
        list.iter().map(|who| s(who)).collect()
    }

    /// The response and whether the call committed anything: a replay, a refused body and a gate
    /// refusal store nothing, so the head does not move.
    fn ask_as(mind: &mut Mind<MemoryStore>, who: &str, list: &[&str], request: ForkAnswerRequest) -> (DeskResponse, bool) {
        let before = head(mind);
        let response = mind.desk_answer(&deciders(list), s(who), DeskAuthority::Member, request, now());
        (response, head(mind) != before)
    }

    fn ask(mind: &mut Mind<MemoryStore>, who: &str, request: ForkAnswerRequest) -> (DeskResponse, bool) {
        ask_as(mind, who, &LIST, request)
    }

    /// The answer, its counted id and the head it was read at; panics on any other response.
    fn answered(response: DeskResponse) -> (ForkDeskAnswer, Option<String>, u64) {
        match response {
            DeskResponse::Answered { answer, counted_answer_id, as_of } => (answer, counted_answer_id, as_of),
            other => panic!("expected an answer, got {other:?}"),
        }
    }

    fn not_stored(response: DeskResponse) -> ForkRefusal {
        match response {
            DeskResponse::NotStored(refusal) => refusal,
            other => panic!("expected a refusal that stores nothing, got {other:?}"),
        }
    }

    fn head(mind: &Mind<MemoryStore>) -> u64 {
        mind.image().unwrap().head
    }

    fn stored_count(mind: &Mind<MemoryStore>, fork: &str) -> usize {
        let image = mind.image().unwrap();
        Docs::at(image, image.head).of_kind(PipelineKind::DeskAnswer).filter(|(_, document)| matches!(document, D::DeskAnswer(answer) if answer.fork.0 == fork)).count()
    }

    fn counted_wire(mind: &Mind<MemoryStore>, fork: &str) -> Option<String> {
        let image = mind.image().unwrap();
        counted_answer(&Docs::at(image, image.head), fork).map(|key| n_text(answer_seq(fork, &key).unwrap()))
    }

    /// A ruling that answers the fork closes it.
    fn close(mind: &mut Mind<MemoryStore>, fork: &str, label: &str, choice: &str) {
        let mut ruled = ruling(label);
        ruled.answers = Some(s(fork));
        ruled.choice = Some(l(choice));
        committed(admit(mind, vec![D::Ruling(ruled)]));
    }

    // ---- the vectors, byte for byte ----

    #[test]
    fn request_vectors_round_trip() {
        let file = vectors();
        let valid = rows(&file["request"]["valid"]);
        assert_eq!(valid.len(), 7);
        for (name, message, bytes) in &valid {
            let decoded: ForkAnswerRequest = rmp_serde::from_slice(bytes).unwrap_or_else(|_| panic!("{name} does not decode"));
            assert_eq!(&encode(&decoded), bytes, "{name} re-encodes to other bytes");
            assert_eq!(decoded, serde_json::from_value::<ForkAnswerRequest>(message.clone()).unwrap(), "{name}");
        }
        // Where serde can say it, the invalid rows do not decode; every other one decodes and the
        // Mind's shape step refuses it (the wire types hold text).
        let invalid = rows(&file["request"]["invalid"]);
        assert_eq!(invalid.len(), 16);
        let mut undecodable = 0;
        for (name, _, bytes) in &invalid {
            match rmp_serde::from_slice::<ForkAnswerRequest>(bytes) {
                Err(_) => undecodable += 1,
                Ok(request) => {
                    let mut mind = desk();
                    let before = head(&mind);
                    let refusal = not_stored(ask(&mut mind, "x", request).0);
                    let want = if name == "fork_id_empty" { ForkRefusalCode::ForkUnknown } else { ForkRefusalCode::BodyInvalid };
                    assert_eq!(refusal.code, want, "{name}");
                    assert_eq!(head(&mind), before, "{name}");
                }
            }
        }
        // answerer_key, authority_key, answer_object_with_a_key, request_id_missing, retracted_not_a_boolean.
        assert_eq!(undecodable, 5);
    }

    #[test]
    fn door_refusal_vectors_round_trip() {
        let file = vectors();
        let mut seen = 0;
        for (name, message, bytes) in rows(&file["doorRefusals"]) {
            if !["fork_unknown", "body_invalid", "request_id_conflict"].contains(&name.as_str()) {
                continue;
            }
            let decoded: ForkRefusal = rmp_serde::from_slice(&bytes).unwrap();
            assert_eq!(encode(&decoded), bytes, "{name}");
            assert_eq!(decoded, serde_json::from_value::<ForkRefusal>(message).unwrap(), "{name}");
            seen += 1;
        }
        assert_eq!(seen, 3);
        // Huginn's two fixed sentences are hers, byte for byte.
        let fixed = |name: &str| rows(&file["doorRefusals"]).into_iter().find(|row| row.0 == name).unwrap().2;
        assert_eq!(encode(&fork_unknown()), fixed("fork_unknown"));
        let conflict = not_stored(door_refusal(MindRefusal::DeskRequestIdConflict { request_id: "x".into() }));
        assert_eq!(encode(&conflict), fixed("request_id_conflict"));
    }

    /// The stored record that projects to one of her answer messages.
    fn stored_from(message: &Value) -> (String, PipelineDeskAnswer) {
        let text = |name: &str| message[name].as_str().map(str::to_string);
        let fork = text("fork_id").unwrap();
        let parts: Vec<&str> = fork.split(':').collect();
        let key = |id: &str| format!("{}:desk_answer:{}.{id}", parts[0], parts[2]);
        let id = text("answer_id").unwrap();
        let target = |name: &str| text(name).map(|id| Short(key(&id)));
        let outcome = match message["outcome"].as_str().unwrap() {
            "Counted" => DeskOutcome::Counted,
            "Revision" => DeskOutcome::Revision { of: target("of").unwrap() },
            "Retraction" => DeskOutcome::Retraction { retracts: target("retracts").unwrap() },
            "Refused" => {
                let code = match message["code"].as_str().unwrap() {
                    "stale_fork" => DeskRefusal::StaleFork,
                    "not_a_decider" => DeskRefusal::NotADecider,
                    "fork_closed" => DeskRefusal::ForkClosed,
                    "choice_invalid" => DeskRefusal::ChoiceInvalid,
                    "bad_retraction" => DeskRefusal::BadRetraction,
                    "already_retracted" => DeskRefusal::AlreadyRetracted,
                    other => panic!("{other}"),
                };
                DeskOutcome::Refused { code, retracts: target("retracts") }
            }
            other => panic!("{other}"),
        };
        let authority = serde_json::from_value(message["authority"].clone()).unwrap();
        let stored = PipelineDeskAnswer {
            campaign: slug(parts[0]),
            fork: Short(fork.clone()),
            sequence: id[1..].parse().unwrap(),
            options_hash: Sha256Hex(text("options_hash").unwrap()),
            choice: text("choice").map(Short),
            note: text("note").map(Words),
            answered_at: Short(text("answered_at").unwrap()),
            received_at: Short(text("received_at").unwrap()),
            claim_in_future: message["claim_in_future"].as_bool().unwrap(),
            request_id: RequestId(text("request_id").unwrap()),
            body_sha256: Sha256Hex("0".repeat(64)),
            // A withheld vector names no answerer: store a placeholder, the vector is the non-decider's view of it.
            answerer: Short(text("answerer").unwrap_or_else(|| "e0d1c2b3-a495-4687-8879-6a5b4c3d2e1f".into())),
            authority,
            outcome,
        };
        (key(&id), stored)
    }

    #[test]
    fn stored_answers_project_to_the_desk_answer_vectors() {
        let file = vectors();
        let valid = rows(&file["deskAnswer"]["valid"]);
        assert_eq!(valid.len(), 8);
        for (name, message, bytes) in &valid {
            let (key, stored) = stored_from(message);
            let mut projected = project(&key, &stored).unwrap();
            // A null answerer is a non-decider's view of the stored answer.
            if message["answerer"].is_null() {
                projected.answerer = None;
            }
            assert_eq!(&encode(&projected), bytes, "{name}");
        }
        // Each answer inside her results is the projection of an equivalent record too.
        let results = file["results"].as_array().unwrap();
        assert_eq!(results.len(), 6);
        for result in results {
            let answer = &result["message"]["answer"];
            let (key, stored) = stored_from(answer);
            assert_eq!(project(&key, &stored).unwrap(), serde_json::from_value::<ForkDeskAnswer>(answer.clone()).unwrap(), "{}", result["name"]);
        }
    }

    #[test]
    fn the_projection_reads_the_stored_times() {
        let file = vectors();
        let (key, mut stored) = stored_from(&file["deskAnswer"]["valid"][0]["message"]);
        // A claim a minute after the receipt is no claim in the future; the record says it was flagged.
        stored.answered_at = Short("2020-01-01T00:01:00Z".into());
        stored.received_at = Short("2020-01-01T00:00:00Z".into());
        stored.claim_in_future = true;
        let projected = project(&key, &stored).unwrap();
        assert!(projected.claim_in_future);
        assert_eq!((projected.answered_at.as_str(), projected.received_at.as_str()), ("2020-01-01T00:01:00Z", "2020-01-01T00:00:00Z"));
        // And the other way: a claim far ahead that the record never flagged stays unflagged.
        stored.answered_at = Short("2031-01-01T00:00:00Z".into());
        stored.claim_in_future = false;
        assert!(!project(&key, &stored).unwrap().claim_in_future);
    }

    // ---- what the projection says of refusals, counts and the conversion ----

    /// What a stored refusal projects: the code, the sent choice and note, the target as `n<seq>`.
    fn refusal_of(mind: &mut Mind<MemoryStore>, who: &str, request: ForkAnswerRequest) -> ForkDeskAnswer {
        let (answer, _, as_of) = answered(ask(mind, who, request).0);
        assert_eq!(answer.outcome, "Refused", "{answer:?}");
        assert_eq!(as_of, head(mind), "a refused answer is read at the head too");
        answer
    }

    #[test]
    fn refusals_project_what_was_sent() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let stale = "e".repeat(64);
        // A refused plain answer keeps its choice as sent, a label or not, and names no target.
        let spaced = ForkAnswerRequest { answer: Some(ForkAnswer { choice: Some("has space".into()), note: Some("why".into()) }), ..wire(FORK, &hash) };
        let answer = refusal_of(&mut mind, "x", spaced);
        assert_eq!((answer.code, answer.choice.as_deref(), answer.note.as_deref(), answer.retracts.as_deref()), (Some(ForkAnswerCode::ChoiceInvalid), Some("has space"), Some("why"), None));
        let answer = refusal_of(&mut mind, "x", wire(FORK, &stale));
        assert_eq!((answer.code, answer.retracts), (Some(ForkAnswerCode::StaleFork), None));
        // An illegal form keeps its choice and note; form (ii), no target at all, projects nil.
        let both = ForkAnswer { choice: Some("A".into()), note: Some("sent with it".into()) };
        let form_i = ForkAnswerRequest { answer: Some(both.clone()), retracted: true, retracts: Some("n1".into()), ..wire(FORK, &hash) };
        let answer = refusal_of(&mut mind, "x", form_i);
        assert_eq!((answer.code, answer.choice.as_deref(), answer.note.as_deref(), answer.retracts.as_deref()), (Some(ForkAnswerCode::BadRetraction), Some("A"), Some("sent with it"), Some("n1")));
        let form_ii = ForkAnswerRequest { answer: None, retracted: true, retracts: None, ..wire(FORK, &hash) };
        let answer = refusal_of(&mut mind, "x", form_ii);
        assert_eq!((answer.code, answer.choice, answer.note, answer.retracts), (Some(ForkAnswerCode::BadRetraction), None, None, None));
        // A refused retraction projects the target it was sent, whatever it named.
        let target = |answer: ForkDeskAnswer, code| assert_eq!((answer.code, answer.retracts.as_deref()), (Some(code), Some("n99")), "{answer:?}");
        target(refusal_of(&mut mind, "x", retraction(FORK, &hash, "n99")), ForkAnswerCode::BadRetraction);
        target(refusal_of(&mut mind, "nobody", retraction(FORK, &hash, "n99")), ForkAnswerCode::NotADecider);
        target(refusal_of(&mut mind, "x", retraction(FORK, &stale, "n99")), ForkAnswerCode::StaleFork);
        // already_retracted and fork_closed name a real answer.
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        answered(ask(&mut mind, "x", wire(FORK, &hash)).0);
        answered(ask(&mut mind, "x", retraction(FORK, &hash, "n1")).0);
        let again = refusal_of(&mut mind, "y", retraction(FORK, &hash, "n1"));
        assert_eq!((again.code, again.retracts.as_deref()), (Some(ForkAnswerCode::AlreadyRetracted), Some("n1")));
        close(&mut mind, FORK, "RQ1", "A");
        let closed = refusal_of(&mut mind, "y", retraction(FORK, &hash, "n1"));
        assert_eq!((closed.code, closed.retracts.as_deref()), (Some(ForkAnswerCode::ForkClosed), Some("n1")));
    }

    /// Words are stored and projected as sent: spaces, case and trailing text are the person's.
    #[test]
    fn choice_and_note_pass_through_as_sent() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let padded = ForkAnswerRequest { answer: Some(ForkAnswer { choice: Some(" a ".into()), note: Some("  Why \n".into()) }), ..wire(FORK, &hash) };
        let answer = refusal_of(&mut mind, "x", padded);
        assert_eq!((answer.code, answer.choice.as_deref(), answer.note.as_deref()), (Some(ForkAnswerCode::ChoiceInvalid), Some(" a "), Some("  Why \n")));
        let note_only = ForkAnswerRequest { answer: Some(ForkAnswer { choice: None, note: Some("\tkeep my edges ".into()) }), ..wire(FORK, &hash) };
        let (answer, _, _) = answered(ask(&mut mind, "x", note_only).0);
        assert_eq!((answer.outcome.as_str(), answer.choice, answer.note.as_deref()), ("Counted", None, Some("\tkeep my edges ")));
        // The stored record says the same: the projection copied it.
        let image = mind.image().unwrap();
        let docs = Docs::at(image, image.head);
        let Some(D::DeskAnswer(stored)) = docs.find(PipelineKind::DeskAnswer, &answer_key(FORK, 2)) else { panic!("no second answer") };
        assert_eq!(stored.note.as_ref().map(|note| note.0.as_str()), Some("\tkeep my edges "));
    }

    #[test]
    fn counts_are_derived_at_the_response_snapshot() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let first = wire(FORK, &hash);
        let (answer, counted, as_of) = answered(ask(&mut mind, "x", first.clone()).0);
        assert_eq!((answer.outcome.as_str(), counted.as_deref(), as_of), ("Counted", Some("n1"), head(&mind)));
        let second = wire(FORK, &hash);
        let (answer, counted, as_of) = answered(ask(&mut mind, "y", second.clone()).0);
        assert_eq!((answer.outcome.as_str(), answer.of.as_deref(), counted.as_deref(), as_of), ("Revision", Some("n1"), Some("n1"), head(&mind)));
        // Retracting the counted answer promotes the next; the response says so at the new head.
        let (answer, counted, as_of) = answered(ask(&mut mind, "y", retraction(FORK, &hash, "n1")).0);
        assert_eq!((answer.outcome.as_str(), answer.retracts.as_deref(), counted.as_deref(), as_of), ("Retraction", Some("n1"), Some("n2"), head(&mind)));
        // The first request replayed: its stored word is still Counted, the count is read now.
        let before = head(&mind);
        let (response, committed) = ask(&mut mind, "x", first);
        let (answer, counted, as_of) = answered(response);
        assert!(!committed);
        assert_eq!((answer.outcome.as_str(), answer.answer_id.as_str(), counted.as_deref(), as_of), ("Counted", "n1", Some("n2"), before));
        assert_eq!(head(&mind), before);
        // The second request replayed now counts, and its stored word is still Revision.
        let (answer, counted, _) = answered(ask(&mut mind, "y", second).0);
        assert_eq!((answer.outcome.as_str(), answer.answer_id.as_str(), counted.as_deref()), ("Revision", "n2", Some("n2")));
        // Retracting what counted leaves nothing counted.
        let (_, counted, as_of) = answered(ask(&mut mind, "x", retraction(FORK, &hash, "n2")).0);
        assert_eq!((counted, as_of), (None, head(&mind)));
    }

    #[test]
    fn one_conversion_names_only_this_forks_answers() {
        assert_eq!(answer_key(FORK, 1), "eureka-state:desk_answer:Q1.n1");
        let mut mind = desk();
        let (hash1, hash2) = (hash_of(&mind, FORK), hash_of(&mind, FORK2));
        answered(ask(&mut mind, "x", wire(FORK, &hash1)).0);
        answered(ask(&mut mind, "x", wire(FORK2, &hash2)).0);
        // n1 retracted on Q2 is Q2's n1; Q1's n1 still counts.
        let (answer, counted, as_of) = answered(ask(&mut mind, "y", retraction(FORK2, &hash2, "n1")).0);
        assert_eq!((answer.outcome.as_str(), answer.retracts.as_deref(), counted, as_of), ("Retraction", Some("n1"), None, head(&mind)));
        assert_eq!(counted_wire(&mind, FORK), Some("n1".into()));
        assert_eq!(counted_wire(&mind, FORK2), None);
        // A key of another fork, a longer label, a leading zero or no n at all name no sequence here.
        assert_eq!(answer_seq(FORK, &answer_key(FORK2, 1)), None);
        assert_eq!(answer_seq(FORK, "eureka-state:desk_answer:Q10.n1"), None);
        assert_eq!(answer_seq(FORK, "eureka-state:desk_answer:Q1.n01"), None);
        assert_eq!(answer_seq(FORK, "eureka-state:desk_answer:Q1.1"), None);
        assert_eq!(answer_seq(FORK, "eureka-state:question:Q1.n1"), None);
        // The ends of the range round-trip.
        for n in [1, 7, u32::MAX] {
            assert_eq!(answer_seq(FORK, &answer_key(FORK, n)), Some(n));
        }
        assert_eq!(n_text(u32::MAX), "n4294967295");
        assert_eq!(parse_n("n4294967296"), None);
    }

    #[test]
    fn answer_key_and_answer_seq_round_trip() {
        let mut seqs: Vec<u32> = (1..=2000).collect();
        for power in 0..=9 {
            let ten = 10u32.pow(power);
            seqs.extend([ten.saturating_sub(1).max(1), ten, ten + 1]);
        }
        seqs.extend([u32::MAX - 2, u32::MAX - 1, u32::MAX, i32::MAX as u32, i32::MAX as u32 + 1, 65_535, 65_536]);
        for fork in [FORK, FORK2, "eureka-state:question:Q10", "other-root:question:Q1"] {
            for &n in &seqs {
                assert_eq!(answer_seq(fork, &answer_key(fork, n)), Some(n), "{fork} {n}");
                assert_eq!(n_text(n), format!("n{n}"));
                assert_eq!(parse_n(&n_text(n)), Some(n));
            }
        }
    }

    /// A stored key outside its fork's `n<seq>` form is a fault of the store, never a wrong id:
    /// it projects `Unavailable`, whichever of the answer, its `of` or its target names it. The
    /// other fork's `n1` and Q10's `n1` agree with a split on the dot, and are refused all the same.
    #[test]
    fn a_stored_key_outside_the_forks_form_is_unavailable() {
        let file = vectors();
        let (key, counted) = stored_from(&file["deskAnswer"]["valid"][0]["message"]);
        assert!(project(&key, &counted).is_ok());
        let fork = counted.fork.0.clone();
        let (root, local) = (fork.split(':').next().unwrap(), fork.split(':').nth(2).unwrap());
        let foreign = [
            format!("{root}:desk_answer:{local}x.n1"),
            format!("{root}:desk_answer:{local}0.n1"),
            format!("{root}:desk_answer:Other.n1"),
            format!("{root}:question:{local}.n1"),
            format!("{root}:desk_answer:{local}.n01"),
            format!("{root}:desk_answer:{local}.1"),
            format!("{root}:desk_answer:{local}.n"),
            format!("{root}:desk_answer:{local}.n4294967296"),
            format!("{root}:desk_answer:{local}.n1.n1"),
            "n1".to_string(),
            String::new(),
        ];
        let unavailable = |refused: MindRefusal| matches!(door_refusal(refused.clone()), DeskResponse::Refused(MindRefusal::Unavailable { .. }));
        for bad in &foreign {
            let refused = project(bad, &counted).expect_err(&format!("the answer's own key {bad:?}"));
            assert!(unavailable(refused), "{bad:?}");
            for outcome in [
                DeskOutcome::Revision { of: Short(bad.clone()) },
                DeskOutcome::Retraction { retracts: Short(bad.clone()) },
                DeskOutcome::Refused { code: DeskRefusal::BadRetraction, retracts: Some(Short(bad.clone())) },
            ] {
                let stored = PipelineDeskAnswer { outcome, ..counted.clone() };
                let refused = project(&key, &stored).expect_err(&format!("a target {bad:?}"));
                assert!(unavailable(refused), "{bad:?}");
            }
        }
        // A refused answer that names no target is fine.
        let none = PipelineDeskAnswer { outcome: DeskOutcome::Refused { code: DeskRefusal::StaleFork, retracts: None }, ..counted };
        assert!(project(&key, &none).is_ok());
    }

    #[test]
    fn a_malformed_retracts_is_body_invalid() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        for bad in ["3", "n0", "n01", "n4294967296", " n1", "N1", "n", "n-1", "n+1", "", "n1x", "n1 ", "n1\n", "n1.", "n\u{661}", "n1n2"] {
            let on_a_retraction = retraction(FORK, &hash, bad);
            let on_an_answer = ForkAnswerRequest { retracts: Some(bad.into()), ..wire(FORK, &hash) };
            for request in [on_a_retraction, on_an_answer] {
                let refusal = request_of(&request).unwrap_err();
                assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid, "{bad:?}");
                assert!(refusal.error.contains("retracts"), "{refusal:?}");
                assert!(!refusal.error.contains(bad) || bad.is_empty() || bad == "n", "the error echoes the input: {refusal:?}");
            }
        }
        for good in ["n1", "n9", "n4294967295"] {
            assert!(request_of(&retraction(FORK, &hash, good)).is_ok(), "{good}");
        }
        // Through the Mind: nothing stored, and the request id is still free.
        let before = head(&mind);
        let mut bad = retraction(FORK, &hash, "n01");
        let refusal = not_stored(ask(&mut mind, "x", bad.clone()).0);
        assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid);
        assert_eq!(head(&mind), before);
        bad = ForkAnswerRequest { retracts: None, retracted: false, answer: Some(ForkAnswer { choice: Some("A".into()), note: None }), ..bad };
        assert_eq!(answered(ask(&mut mind, "x", bad).0).0.outcome, "Counted");
    }

    #[test]
    fn illegal_retraction_forms_are_stored_bad_retraction() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        answered(ask(&mut mind, "x", wire(FORK, &hash)).0);
        let said = ForkAnswer { choice: Some("A".into()), note: None };
        let forms = [
            ForkAnswerRequest { answer: Some(said.clone()), retracted: true, retracts: Some("n1".into()), ..wire(FORK, &hash) },
            ForkAnswerRequest { answer: None, retracted: true, retracts: None, ..wire(FORK, &hash) },
            ForkAnswerRequest { answer: Some(said.clone()), retracted: false, retracts: Some("n1".into()), ..wire(FORK, &hash) },
        ];
        for (form, request) in forms.into_iter().enumerate() {
            let stored = stored_count(&mind, FORK);
            let (response, committed) = ask(&mut mind, "x", request.clone());
            assert!(committed);
            let (answer, _, _) = answered(response.clone());
            assert_eq!((answer.outcome.as_str(), answer.code), ("Refused", Some(ForkAnswerCode::BadRetraction)), "form {form}");
            assert_eq!(stored_count(&mind, FORK), stored + 1, "form {form}");
            // The refusal is stored and bound: the same bytes replay, with no write.
            let before = head(&mind);
            assert_eq!(ask(&mut mind, "x", request), (response, false), "form {form}");
            assert_eq!(head(&mind), before);
        }
        // (i) and (iii) with equal answer and target differ only in `retracted`: one id, two bodies.
        let id = fresh_id();
        let form_i = ForkAnswerRequest { answer: Some(said.clone()), retracted: true, retracts: Some("n1".into()), request_id: id.clone(), ..wire(FORK, &hash) };
        let form_iii = ForkAnswerRequest { retracted: false, ..form_i.clone() };
        answered(ask(&mut mind, "y", form_i).0);
        let before = head(&mind);
        assert_eq!(not_stored(ask(&mut mind, "y", form_iii).0).code, ForkRefusalCode::RequestIdConflict);
        assert_eq!(head(&mind), before);
        // No answer and no retraction, or an answer object with neither choice nor note, is no form.
        let neither = ForkAnswerRequest { answer: None, retracted: false, ..wire(FORK, &hash) };
        let empty = ForkAnswerRequest { answer: Some(ForkAnswer { choice: None, note: None }), ..wire(FORK, &hash) };
        for request in [neither, empty] {
            assert_eq!(not_stored(ask(&mut mind, "x", request).0).code, ForkRefusalCode::BodyInvalid);
        }
        assert_eq!(head(&mind), before);
    }

    #[test]
    fn the_refusal_mapping_is_heimdalls() {
        let code = |refused: MindRefusal| not_stored(door_refusal(refused)).code;
        let missing = MindRefusal::MissingReference { kind: PipelineKind::Question, id: "eureka-state:question:nope".into() };
        assert_eq!(code(missing), ForkRefusalCode::ForkUnknown);
        assert_eq!(code(MindRefusal::DeskRequestInvalid { field: "fork".into() }), ForkRefusalCode::ForkUnknown);
        assert_eq!(code(MindRefusal::CampaignMoved { campaign: "eureka-state".into(), to: "elsewhere".into() }), ForkRefusalCode::ForkUnknown);
        assert_eq!(code(MindRefusal::DeskRequestIdConflict { request_id: "secret".into() }), ForkRefusalCode::RequestIdConflict);
        let refusal = not_stored(door_refusal(MindRefusal::DeskRequestInvalid { field: "desk_answer.note".into() }));
        assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid);
        assert!(refusal.error.contains("desk_answer.note"), "{refusal:?}");
        // Nothing else is the door's to rename: it passes through unchanged, Unavailable included.
        for other in [
            MindRefusal::Unavailable { detail: "the store is down".into() },
            MindRefusal::Unavailable { detail: "a stored desk answer names a key outside its fork's n<seq> form".into() },
            MindRefusal::Unavailable { detail: "a race on fork request_id body".into() },
            MindRefusal::Unavailable { detail: String::new() },
            MindRefusal::ForeignInstance { declared: "a".into(), mind: "b".into() },
            MindRefusal::MissingIdentity,
        ] {
            assert_eq!(door_refusal(other.clone()), DeskResponse::Refused(other));
        }
        // The refusal never echoes the id it refuses.
        let conflict = not_stored(door_refusal(MindRefusal::DeskRequestIdConflict { request_id: "secret".into() }));
        assert!(!conflict.error.contains("secret"));
    }

    /// Her schema takes any non-empty string as a fork id and her vectors hold no over-long one;
    /// no such id can name a fork, so it is `fork_unknown` like the empty one. Recorded deviation:
    /// the bound is the Mind's 200 bytes (`desk_answer.fork`), and the answer is not `body_invalid`.
    #[test]
    fn an_over_long_fork_id_is_unknown() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let before = head(&mind);
        for fork in [format!("{FORK}{}", "9".repeat(200)), format!("eureka-state:question:{}", "q".repeat(300))] {
            let refusal = not_stored(ask(&mut mind, "x", wire(&fork, &hash)).0);
            assert_eq!(refusal.code, ForkRefusalCode::ForkUnknown, "{} bytes", fork.len());
            assert_eq!(refusal, fork_unknown());
        }
        assert_eq!(head(&mind), before);
        // The mapping is by exact field: a field that merely ends in "fork" is a bad body.
        for field in ["desk_answer.fork", "fork", "desk_answer.key", "desk_answer.campaign"] {
            assert_eq!(not_stored(door_refusal(MindRefusal::DeskRequestInvalid { field: field.into() })).code, ForkRefusalCode::ForkUnknown, "{field}");
        }
        for field in ["notfork", "desk_answer.forks", "desk_answer.fork.x", "request"] {
            assert_eq!(not_stored(door_refusal(MindRefusal::DeskRequestInvalid { field: field.into() })).code, ForkRefusalCode::BodyInvalid, "{field}");
        }
    }

    /// Any bad fork id is `fork_unknown`, whichever field the Mind faults: the id's shape, its
    /// campaign root or the record key built from it. A bad field that is not the fork stays `body_invalid`.
    #[test]
    fn every_bad_fork_id_is_unknown_and_other_bad_fields_are_not() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let before = head(&mind);
        let forks = [
            "",                                  // fork
            "eureka-state:epic:Q1",              // fork: not a question id
            "eureka-state:question:Q 1",         // desk_answer.fork
            ":question:Q1",                      // desk_answer.campaign
            "Bad Root:question:Q1",              // desk_answer.campaign
            "a:question:Q1.2",                   // desk_answer.key
            "eureka-state:question:Q99",         // a well-formed id nothing holds
        ];
        for fork in forks {
            let refusal = not_stored(ask(&mut mind, "x", wire(fork, &hash)).0);
            assert_eq!(refusal, fork_unknown(), "{fork:?}");
            assert_eq!(head(&mind), before, "{fork:?}");
        }
        let bad_choice = ForkAnswerRequest { answer: Some(ForkAnswer { choice: Some(String::new()), note: None }), ..wire(FORK, &hash) };
        let refusal = not_stored(ask(&mut mind, "x", bad_choice).0);
        assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid);
        assert!(refusal.error.contains("desk_answer.choice"), "{refusal:?}");
        assert_eq!(head(&mind), before);
    }

    #[test]
    fn wire_codes_are_snake_case() {
        let table = [
            (DeskRefusal::StaleFork, "stale_fork"),
            (DeskRefusal::NotADecider, "not_a_decider"),
            (DeskRefusal::ForkClosed, "fork_closed"),
            (DeskRefusal::ChoiceInvalid, "choice_invalid"),
            (DeskRefusal::BadRetraction, "bad_retraction"),
            (DeskRefusal::AlreadyRetracted, "already_retracted"),
        ];
        for (refusal, spelled) in table {
            assert_eq!(serde_json::to_value(ForkAnswerCode::from(refusal)).unwrap(), spelled);
        }
        for (code, spelled) in [(ForkRefusalCode::ForkUnknown, "fork_unknown"), (ForkRefusalCode::BodyInvalid, "body_invalid"), (ForkRefusalCode::RequestIdConflict, "request_id_conflict")] {
            assert_eq!(serde_json::to_value(code).unwrap(), spelled);
        }
        // Outcome and authority are spelled as her schema spells them.
        for (authority, spelled) in [(DeskAuthority::Operator, "Operator"), (DeskAuthority::Steward, "Steward"), (DeskAuthority::Member, "Member")] {
            assert_eq!(serde_json::to_value(authority).unwrap(), spelled);
        }
        let outcomes = [DeskOutcome::Counted, DeskOutcome::Revision { of: s("k") }, DeskOutcome::Retraction { retracts: s("k") }, DeskOutcome::Refused { code: DeskRefusal::StaleFork, retracts: None }];
        assert_eq!(outcomes.iter().map(DeskOutcome::name).collect::<Vec<_>>(), ["Counted", "Revision", "Retraction", "Refused"]);
    }

    #[test]
    fn a_stale_answer_carries_no_card() {
        let mut mind = desk();
        let stale = wire(FORK, &"e".repeat(64));
        let (fresh, committed) = ask(&mut mind, "x", stale.clone());
        assert!(committed);
        let (answer, _, _) = answered(fresh.clone());
        assert_eq!((answer.outcome.as_str(), answer.code), ("Refused", Some(ForkAnswerCode::StaleFork)));
        // After the fork closed, the retry still answers the stored refusal, with nothing added.
        close(&mut mind, FORK, "RQ1", "A");
        let (again, committed) = ask(&mut mind, "x", stale);
        assert!(!committed);
        let (again_answer, _, _) = answered(again.clone());
        assert_eq!(again_answer, answer);
        for response in [&fresh, &again] {
            assert!(!serde_json::to_string(response).unwrap().to_lowercase().contains("card"), "{response:?}");
        }
    }

    #[test]
    fn a_replay_encodes_as_the_fresh_store() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let request = wire(FORK, &hash);
        let (fresh, first) = ask(&mut mind, "x", request.clone());
        let (replay, second) = ask(&mut mind, "x", request);
        assert_eq!((first, second), (true, false));
        assert_eq!(encode(&fresh), encode(&replay));
        assert_eq!(stored_count(&mind, FORK), 1);
    }

    // ---- the gate, the bounds and the request id through the wire ----

    fn question_with(label: &str, edit: impl Fn(&mut PipelineQuestion)) -> PipelineDocument {
        let D::Question(mut question) = question(label, &["A", "B"], "A") else { unreachable!() };
        edit(&mut question);
        D::Question(question)
    }

    #[test]
    fn unshowable_forks_are_unknown() {
        let mut mind = desk();
        committed(admit(&mut mind, vec![question_with("Q3", |q| q.depends = vec!["".into()]), question_with("Q4", |q| q.options[1].text = "".into())]));
        let id = fresh_id();
        let request = |mind: &Mind<MemoryStore>, fork: &str| ForkAnswerRequest { request_id: id.clone(), ..wire(fork, &hash_of(mind, fork)) };
        // A held question with an empty depends line or an empty option text stores nothing, whatever
        // form the request takes: an answer, a retraction or an illegal form.
        for fork in ["eureka-state:question:Q3", "eureka-state:question:Q4"] {
            let hash = hash_of(&mind, fork);
            let said = ForkAnswer { choice: Some("A".into()), note: None };
            let forms = [
                request(&mind, fork),
                ForkAnswerRequest { request_id: id.clone(), ..retraction(fork, &hash, "n1") },
                ForkAnswerRequest { request_id: id.clone(), answer: Some(said), retracted: true, retracts: Some("n1".into()), ..wire(fork, &hash) },
            ];
            for asked in forms {
                let before = head(&mind);
                assert_eq!(not_stored(ask(&mut mind, "x", asked).0).code, ForkRefusalCode::ForkUnknown, "{fork}");
                assert_eq!((head(&mind), stored_count(&mind, fork)), (before, 0), "{fork}");
            }
        }
        // So does a showable question with no decider to show it to.
        let (before, asked) = (head(&mind), request(&mind, FORK));
        assert_eq!(not_stored(ask_as(&mut mind, "x", &[], asked).0).code, ForkRefusalCode::ForkUnknown);
        assert_eq!((head(&mind), stored_count(&mind, FORK)), (before, 0));
        // None of that bound the request id.
        let asked = request(&mind, FORK);
        assert_eq!(answered(ask(&mut mind, "x", asked).0).0.outcome, "Counted");
    }

    #[test]
    fn a_stored_answer_replays_whatever_the_decider_list_is_now() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let request = wire(FORK, &hash);
        let (stored, committed) = ask(&mut mind, "x", request.clone());
        assert!(committed);
        let before = head(&mind);
        // The list Bifrost passes is empty now; the stored request still gets its stored answer.
        let (again, committed) = ask_as(&mut mind, "x", &[], request.clone());
        assert!(!committed);
        assert_eq!(encode(&again), encode(&stored));
        assert_eq!(head(&mind), before);
        // Another subject with the same bytes has stored nothing: the gate refuses it.
        let (other, committed) = ask_as(&mut mind, "y", &[], request);
        assert_eq!((not_stored(other).code, committed), (ForkRefusalCode::ForkUnknown, false));
        // And a request nobody stored is fork_unknown while the list is empty.
        let fresh = wire(FORK, &hash);
        assert_eq!(not_stored(ask_as(&mut mind, "x", &[], fresh).0).code, ForkRefusalCode::ForkUnknown);
        assert_eq!((head(&mind), stored_count(&mind, FORK)), (before, 1));
        // A stored refusal and a stored revision replay the same way: a stale hash is stored as a
        // refusal, a second answer by the same subject as a revision.
        let stale = wire(FORK, "0".repeat(hash.len()).as_str());
        let (refused, committed) = ask(&mut mind, "x", stale.clone());
        assert!(committed);
        assert_eq!(answered(refused.clone()).0.outcome, "Refused");
        let revision = wire(FORK, &hash);
        let (revised, committed) = ask(&mut mind, "x", revision.clone());
        assert!(committed);
        assert_eq!(answered(revised.clone()).0.outcome, "Revision");
        let before = head(&mind);
        for (request, stored) in [(stale, refused), (revision, revised)] {
            let (again, committed) = ask_as(&mut mind, "x", &[], request.clone());
            assert!(!committed);
            // `as_of` is the head the replay was read at, so the stored answer is what is compared.
            assert_eq!(encode(&answered(again).0), encode(&answered(stored).0));
            assert_eq!(head(&mind), before);
            let (other, committed) = ask_as(&mut mind, "w", &[], request);
            assert_eq!((not_stored(other).code, committed), (ForkRefusalCode::ForkUnknown, false));
            assert_eq!(head(&mind), before);
        }
    }

    #[test]
    fn showable_needs_two_options_text_depends_and_a_decider() {
        let D::Question(plain) = question("Q9", &["A", "B"], "A") else { unreachable!() };
        let list = deciders(&LIST);
        assert!(showable(&plain, &list));
        assert!(!showable(&plain, &[]));
        let mut one = plain.clone();
        one.options.truncate(1);
        assert!(!showable(&one, &list));
        let mut none = plain.clone();
        none.options.clear();
        assert!(!showable(&none, &list));
        let mut blank_text = plain.clone();
        blank_text.options[0] = QuestionOption { label: l("A"), text: "".into() };
        assert!(!showable(&blank_text, &list));
        let mut blank_line = plain.clone();
        blank_line.depends = vec!["Cut 3a".into(), "".into()];
        assert!(!showable(&blank_line, &list));
        let mut lines = plain;
        lines.depends = vec!["Cut 3a".into()];
        assert!(showable(&lines, &list));
    }

    #[test]
    fn the_note_and_choice_limits_are_bytes() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let note = |text: String| ForkAnswerRequest { answer: Some(ForkAnswer { choice: None, note: Some(text) }), ..wire(FORK, &hash) };
        let choice = |text: String| ForkAnswerRequest { answer: Some(ForkAnswer { choice: Some(text), note: None }), ..wire(FORK, &hash) };
        let (answer, _, _) = answered(ask(&mut mind, "x", note("x".repeat(4000))).0);
        assert_eq!(answer.outcome, "Counted");
        let before = head(&mind);
        for refused in [note("x".repeat(4001)), note("\u{1F600}".repeat(1001)), note("é".repeat(2001)), note("  \u{a0} ".into()), note(String::new())] {
            let refusal = not_stored(ask(&mut mind, "x", refused).0);
            assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid);
            assert!(refusal.error.contains("desk_answer.note"), "{refusal:?}");
        }
        assert_eq!(head(&mind), before);
        // A choice is any text of 1 to 200 bytes: one that is no label is stored `choice_invalid`.
        for text in ["has space".to_string(), "z".repeat(200)] {
            let (answer, _, _) = answered(ask(&mut mind, "x", choice(text.clone())).0);
            assert_eq!((answer.code, answer.choice), (Some(ForkAnswerCode::ChoiceInvalid), Some(text)));
        }
        let before = head(&mind);
        for refused in [choice(String::new()), choice("z".repeat(201))] {
            let refusal = not_stored(ask(&mut mind, "x", refused).0);
            assert!(refusal.error.contains("desk_answer.choice"), "{refusal:?}");
        }
        assert_eq!(head(&mind), before);
    }

    #[test]
    fn request_id_passes_through() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let request = wire(FORK, &hash);
        let (first, _) = ask(&mut mind, "x", request.clone());
        // V12: the same bytes replay.
        let before = head(&mind);
        assert_eq!(ask(&mut mind, "x", request.clone()), (first, false));
        assert_eq!(head(&mind), before);
        // V13: only answered_at changed is a conflict, and nothing is stored.
        let moved = ForkAnswerRequest { answered_at: "2026-11-10T12:00:01Z".into(), ..request.clone() };
        assert_eq!(not_stored(ask(&mut mind, "x", moved).0).code, ForkRefusalCode::RequestIdConflict);
        assert_eq!((head(&mind), stored_count(&mind, FORK)), (before, 1));
        // V14: another subject with identical bytes is its own request: its own answer id, stamped with it.
        let (other, committed) = ask(&mut mind, "y", request.clone());
        let (other, _, _) = answered(other);
        assert!(committed);
        assert_eq!((other.answer_id.as_str(), other.answerer.as_deref().unwrap(), other.outcome.as_str(), other.request_id.as_str()), ("n2", "y", "Revision", request.request_id.as_str()));
        let (original, _, _) = answered(ask(&mut mind, "x", request.clone()).0);
        assert_eq!((original.answer_id.as_str(), original.answerer.as_deref().unwrap()), ("n1", "x"));
        // V17 and V18: an id that is no lowercase D-format UUID is body_invalid naming request_id, and binds nothing.
        let before = head(&mind);
        for bad in ["retry-1", "0000000A-0000-4000-8000-00000000000B", "{00000000-0000-4000-8000-000000000001}", "00000000-0000-4000-8000-0000000000010", "", " 00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000001 ", "00000000-0000-4000-8000-000000000001\n", "00000000-0000-4000-8000-000000000001x"] {
            let refusal = not_stored(ask(&mut mind, "x", ForkAnswerRequest { request_id: bad.into(), ..request.clone() }).0);
            assert_eq!(refusal.code, ForkRefusalCode::BodyInvalid, "{bad:?}");
            assert!(refusal.error.contains("request_id"), "{refusal:?}");
        }
        assert_eq!(head(&mind), before);
    }

    #[test]
    fn forbidden_and_unknown_keys_do_not_decode() {
        let base = vectors()["request"]["valid"][0]["message"].clone();
        let decode = |value: &Value| rmp_serde::from_slice::<ForkAnswerRequest>(&rmp_serde::to_vec_named(value).unwrap());
        assert!(decode(&base).is_ok());
        for key in ["answerer", "authority", "attribution", "subject", "deciders", "mind", "mood"] {
            let mut top = base.clone();
            top[key] = Value::String("x".into());
            assert!(decode(&top).is_err(), "{key} at the top");
            let mut inside = base.clone();
            inside["answer"][key] = Value::String("x".into());
            assert!(decode(&inside).is_err(), "{key} inside answer");
        }
        let mut wrong = base.clone();
        wrong["schemaVersion"] = Value::String("gamecult.fork.answer_request.v1".into());
        assert!(decode(&wrong).is_err());
    }

    /// Her schema lists every nullable key as required: a key left out refuses, the same key sent
    /// as null decodes.
    #[test]
    fn nullable_keys_must_be_present() {
        fn without(message: &Value, path: &[&str], key: &str) -> Value {
            let mut copy = message.clone();
            let mut at = &mut copy;
            for step in path {
                at = &mut at[*step];
            }
            at.as_object_mut().unwrap().remove(key).unwrap_or_else(|| panic!("{key} is not in the message"));
            copy
        }
        fn nulled(message: &Value, path: &[&str], key: &str) -> Value {
            let mut copy = message.clone();
            let mut at = &mut copy;
            for step in path {
                at = &mut at[*step];
            }
            at[key] = Value::Null;
            copy
        }
        fn decodes<T: serde::de::DeserializeOwned>(value: &Value) -> bool {
            rmp_serde::from_slice::<T>(&rmp_serde::to_vec_named(value).unwrap()).is_ok()
        }
        let file = vectors();
        // A request that sends an answer object (both words) and no target.
        let request = file["request"]["valid"].as_array().unwrap().iter().map(|row| row["message"].clone()).find(|message| message["answer"]["choice"].is_string() && message["answer"]["note"].is_string()).unwrap();
        assert!(decodes::<ForkAnswerRequest>(&request));
        for key in ["answer", "retracts"] {
            assert!(!decodes::<ForkAnswerRequest>(&without(&request, &[], key)), "request without {key}");
        }
        assert!(decodes::<ForkAnswerRequest>(&nulled(&request, &[], "answer")));
        assert!(decodes::<ForkAnswerRequest>(&nulled(&request, &[], "retracts")));
        for key in ["choice", "note"] {
            assert!(!decodes::<ForkAnswerRequest>(&without(&request, &["answer"], key)), "answer without {key}");
        }
        assert!(decodes::<ForkAnswerRequest>(&nulled(&nulled(&request, &["answer"], "note"), &[], "retracts")));
        // Every key of every valid desk answer, null or not, is required.
        let valid = rows(&file["deskAnswer"]["valid"]);
        for (name, message, _) in &valid {
            assert!(decodes::<ForkDeskAnswer>(message), "{name}");
            for key in message.as_object().unwrap().keys() {
                assert!(!decodes::<ForkDeskAnswer>(&without(message, &[], key)), "{name} without {key}");
            }
        }
        assert!(decodes::<ForkDeskAnswer>(&nulled(&valid[0].1, &[], "of")));
    }

    // ---- Libby's scenarios through Mind::desk_answer ----

    /// The scenarios whose card changes under their answers. A question is immutable in the Mind,
    /// so there is no operation to replay a changed card with; the older-card rules are
    /// desk.rs's `answers_under_an_older_card_neither_count_nor_can_be_retracted`.
    const CHANGED_CARD: [&str; 2] = ["a_changed_card_stops_the_old_answers_counting_and_a_new_answer_is_counted", "a_changed_card_leaves_no_counted_answer_until_one_arrives"];

    /// The question a scenario's card shows, held as Q1.
    fn question_of(card: &Value) -> PipelineQuestion {
        let text = |value: &Value| value.as_str().unwrap().to_string();
        let D::Question(mut question) = question("Q1", &["A", "B"], "A") else { unreachable!() };
        question.title = text(&card["title"]).as_str().into();
        question.question = text(&card["context"]).as_str().into();
        question.recommended = l(&text(&card["recommendation"]));
        question.options = card["options"].as_array().unwrap().iter().map(|option| QuestionOption { label: l(&text(&option["id"])), text: text(&option["text"]).as_str().into() }).collect();
        question.depends = card["depends"].as_array().unwrap().iter().map(|line| text(line).as_str().into()).collect();
        question
    }

    #[test]
    fn libby_scenarios_replay_through_desk_answer() {
        let file = vectors();
        let (mut played, mut compared, mut refused_at_the_door, mut skipped) = (0, 0, 0, Vec::new());
        let (mut with_count, mut with_target) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
        for scenario in file["scenarios"].as_array().unwrap() {
            let name = scenario["name"].as_str().unwrap();
            if CHANGED_CARD.contains(&name) {
                skipped.push(name);
                continue;
            }
            played += 1;
            let card = &scenario["card"];
            let (her_fork, her_hash) = (card["fork_id"].as_str().unwrap(), card["options_hash"].as_str().unwrap());
            let question = question_of(card);
            assert_eq!(shown_hash(&question).0, her_hash, "{name}: the card's hash");
            let mut mind = seeded();
            committed(admit(&mut mind, vec![D::Question(question)]));
            let list: Vec<&str> = card["deciders"].as_array().unwrap().iter().map(|who| who.as_str().unwrap()).collect();
            if card["status"] == "closed" {
                close(&mut mind, FORK, "RQ1", "no");
            }
            // Her ids to the Mind's: her fixtures number answers as they please, the Mind's sequence is its own.
            let mut ids: HashMap<String, String> = HashMap::new();
            for step in scenario["steps"].as_array().unwrap() {
                let (response, server) = (&step["response"], &step["server"]);
                match step["kind"].as_str().unwrap() {
                    // A ruling closes the fork; a revised card changes nothing the Mind holds.
                    "card" => {
                        if step["card"]["status"] == "closed" {
                            close(&mut mind, FORK, "RQ1", "no");
                        }
                        continue;
                    }
                    // Chat events reach no Mind.
                    "matrix_event" => continue,
                    _ => {}
                }
                let code = response["code"].as_str();
                // Refused at the door before the Mind: a body outside the closed document does not decode here either.
                if matches!(code, Some("body_forbidden_field" | "body_unknown_field")) {
                    assert!(serde_json::from_value::<ForkAnswerRequest>(step["body"].clone()).is_err(), "{name}");
                    refused_at_the_door += 1;
                    continue;
                }
                if matches!(code, Some("forbidden" | "unauthenticated")) || (code == Some("unavailable") && server["stored"] != true) {
                    continue;
                }
                let mut request: ForkAnswerRequest = serde_json::from_value(step["body"].clone()).unwrap();
                if request.fork_id == her_fork {
                    request.fork_id = FORK.into();
                }
                if let Some(mapped) = request.retracts.as_ref().and_then(|target| ids.get(target)) {
                    request.retracts = Some(mapped.clone());
                }
                let authority = serde_json::from_value(server["authority"].clone()).unwrap();
                let at = DateTime::parse_from_rfc3339(server["received_at"].as_str().unwrap()).unwrap().to_utc();
                let answerer = s(server["answerer"].as_str().unwrap());
                let mine = mind.desk_answer(&deciders(&list), answerer, authority, request, at);
                if code == Some("unavailable") {
                    continue; // the door went down after the Mind stored: its answer was never seen
                }
                if response["schemaVersion"].as_str().unwrap().ends_with("refusal.v0") {
                    assert_eq!(serde_json::to_value(not_stored(mine).code).unwrap(), code.unwrap(), "{name}");
                    continue;
                }
                let (mut mine, mut counted, _) = answered(mine);
                let mut hers: ForkDeskAnswer = serde_json::from_value(response["answer"].clone()).unwrap();
                assert_eq!(ids.entry(hers.answer_id.clone()).or_insert_with(|| mine.answer_id.clone()), &mine.answer_id, "{name}: one answer, one id");
                mine.fork_id = her_fork.into();
                // Her ids to the Mind's, once; everything else is compared exactly: the refused target as
                // `n<seq>` and the count derived at the response's head.
                let through = |id: &mut Option<String>| *id = id.take().map(|id| ids.get(&id).cloned().unwrap_or(id));
                let mut hers_counted = response["counted_answer_id"].as_str().map(str::to_string);
                through(&mut hers.of);
                through(&mut hers.retracts);
                through(&mut hers_counted);
                hers.answer_id = mine.answer_id.clone();
                assert_eq!(counted.take(), hers_counted, "{name}: counted_answer_id");
                if hers_counted.is_some() {
                    with_count.insert(name);
                }
                if mine.outcome == "Refused" && mine.retracts.is_some() {
                    with_target.insert(name);
                }
                assert_eq!(mine, hers, "{name}");
                assert_eq!(encode(&mine), encode(&hers), "{name}");
                compared += 1;
            }
            let record = &scenario["record"];
            // Where her fixture numbers its answers as the Mind's sequence does, the map is the identity.
            let sequential = record["answers"].as_array().unwrap().iter().enumerate().all(|(at, answer)| answer["answer_id"] == format!("n{}", at + 1));
            assert!(!sequential || ids.iter().all(|(hers, mine)| hers == mine), "{name}: {ids:?}");
            // And the map is one-to-one in every scenario, sequential or not: two of her answers never share one of the Mind's.
            let mut mine: Vec<&String> = ids.values().collect();
            mine.sort();
            mine.dedup();
            assert_eq!(mine.len(), ids.len(), "{name}: two answers share an id: {ids:?}");
            assert_eq!(stored_count(&mind, FORK), record["answers"].as_array().unwrap().len(), "{name}: the stored answers");
            let hers = record["counted_answer_id"].as_str().map(|id| ids.get(id).cloned().unwrap_or(id.to_string()));
            assert_eq!(counted_wire(&mind, FORK), hers, "{name}: the answer that counts");
        }
        assert_eq!((played, skipped.len()), (61, 2));
        assert_eq!(refused_at_the_door, 7);
        assert_eq!(compared, 91);
        // 43 replayed scenarios pin a non-null counted_answer_id on an answer; 12 pin a refused retraction's target as n<seq>.
        assert_eq!((with_count.len(), with_target.len()), (43, 12));
    }

    #[test]
    fn the_authority_is_stamped_and_never_decides() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        // The operator's own account, off the list, is no decider; a member on the list is.
        let off = mind.desk_answer(&deciders(&["x"]), s("op"), DeskAuthority::Operator, wire(FORK, &hash), now());
        assert_eq!(answered(off).0.code, Some(ForkAnswerCode::NotADecider));
        let on = mind.desk_answer(&deciders(&["m"]), s("m"), DeskAuthority::Member, wire(FORK, &hash), now());
        let (answer, _, _) = answered(on);
        assert_eq!((answer.outcome.as_str(), answer.authority, answer.answerer.as_deref().unwrap()), ("Counted", DeskAuthority::Member, "m"));
    }

    // ---- the desk request on the wire: deciders ride each call, at most 64, never stored ----

    use crate::envelope::{OperationFailure, decode_desk_request, encode_desk_request};

    fn list_of(n: usize) -> Vec<Short> {
        (0..n).map(|at| s(&format!("d{at}"))).collect()
    }

    /// Each request kind of the desk, carrying `list`.
    fn desk_requests(mind: &str, list: &[Short], hash: &str) -> [DeskRequest; 3] {
        [
            DeskRequest::Answer { mind: slug(mind), subject: s("d0"), authority: DeskAuthority::Member, deciders: list.to_vec(), request: wire(FORK, hash) },
            DeskRequest::Forks { mind: slug(mind), subject: s("d0"), deciders: list.to_vec(), status: None, cursor: None, limit: ForkLimit::new(10).unwrap() },
            DeskRequest::Fork { mind: slug(mind), subject: s("d0"), deciders: list.to_vec(), fork_id: FORK.into() },
        ]
    }

    /// The request as a client sends it and the door reads it.
    fn through_the_door(request: &DeskRequest) -> Result<DeskRequest, OperationFailure> {
        let message = encode_desk_request("m-1", request, None).unwrap();
        decode_desk_request(&message).map(|(_, read)| read)
    }

    #[test]
    fn deciders_are_at_most_sixty_four_per_call_on_answer_forks_and_fork() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let names = ["answer", "forks", "fork"];
        for (at, sent) in desk_requests(INSTANCE, &list_of(64), &hash).into_iter().enumerate() {
            let read = through_the_door(&sent).unwrap_or_else(|failure| panic!("{}: 64 deciders refused: {failure:?}", names[at]));
            assert_eq!((read.operation(), read.deciders().len()), (names[at], 64));
            assert_eq!(read, sent);
        }
        // 64 deciders through to the Mind: the subject is listed and counts.
        let DeskRequest::Answer { subject, authority, deciders, request, .. } = through_the_door(&desk_requests(INSTANCE, &list_of(64), &hash)[0]).unwrap() else { panic!("not an answer") };
        let before = head(&mind);
        let response = mind.desk_answer(&deciders, subject, authority, request, now());
        assert!(head(&mind) > before);
        assert_eq!(answered(response).0.outcome, "Counted");

        let (head_before, stored_before) = (head(&mind), stored_count(&mind, FORK));
        for (at, sent) in desk_requests(INSTANCE, &list_of(65), &hash).into_iter().enumerate() {
            let failure = through_the_door(&sent).unwrap_err();
            assert_eq!(failure.code, "payload-not-a-request", "{}", names[at]);
            assert!(failure.message.contains("deciders"), "{} names the field: {}", names[at], failure.message);
            assert!(!failure.message.contains("d64"), "{} echoes a decider: {}", names[at], failure.message);
        }
        assert_eq!((head(&mind), stored_count(&mind, FORK)), (head_before, stored_before));
    }

    #[test]
    fn an_empty_or_duplicated_decider_list_is_refused_at_decode_and_one_is_accepted() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let before = (head(&mind), stored_count(&mind, FORK));
        let mut twice = list_of(3);
        twice.push(s("d1"));
        for (what, list) in [("empty", vec![]), ("duplicate", twice)] {
            for sent in desk_requests(INSTANCE, &list, &hash) {
                let failure = through_the_door(&sent).unwrap_err();
                assert_eq!(failure.code, "payload-not-a-request", "{what} {}", sent.operation());
                assert!(failure.message.contains("deciders") && !failure.message.contains("d1"), "{what} {}: {}", sent.operation(), failure.message);
            }
        }
        // One decider is a list.
        for sent in desk_requests(INSTANCE, &list_of(1), &hash) {
            assert_eq!(through_the_door(&sent).unwrap(), sent);
        }
        assert_eq!((head(&mind), stored_count(&mind, FORK)), before);
        // The Mind's showability gate is the second line: an empty list handed to it directly
        // still refuses the fork and stores nothing.
        let refused = not_stored(mind.desk_answer(&[], s("d0"), DeskAuthority::Member, wire(FORK, &hash), now()));
        assert_eq!(refused.code, ForkRefusalCode::ForkUnknown);
        assert_eq!((head(&mind), stored_count(&mind, FORK)), before);
    }

    #[test]
    fn a_request_for_another_mind_is_refused_and_stores_nothing() {
        let mut mind = desk();
        let hash = hash_of(&mind, FORK);
        let before = (head(&mind), stored_count(&mind, FORK));
        for sent in desk_requests(OTHER_INSTANCE, &list_of(4), &hash) {
            let read = through_the_door(&sent).unwrap();
            assert_eq!(
                mind.require_instance(read.mind()),
                Err(MindRefusal::ForeignInstance { declared: OTHER_INSTANCE.into(), mind: INSTANCE.into() })
            );
        }
        assert_eq!((head(&mind), stored_count(&mind, FORK)), before);
        // The same request to the mind it names is answered.
        let DeskRequest::Answer { mind: named, subject, authority, deciders, request } = through_the_door(&desk_requests(INSTANCE, &list_of(4), &hash)[0]).unwrap() else { panic!("not an answer") };
        mind.require_instance(&named).unwrap();
        assert_eq!(answered(mind.desk_answer(&deciders, subject, authority, request, now())).0.outcome, "Counted");
    }
}
