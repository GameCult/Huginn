//! The fork desk's read side: the card, the record, the entry and the list as Libby's closed
//! map-mode documents (CultLib `fork-card.schema.json` at e57fbaee), their projection from the
//! stored question and desk answers, and the two Mind calls the door dispatches to,
//! `Mind::desk_forks` and `Mind::desk_fork`. These docs, the doc of `DeskRequest` and the doc of
//! `DeskResponse` are the reads spec.
//!
//! Everything in one response is read at ONE snapshot, `image.head`, returned as `as_of`; the
//! list's `stamp` is `s<as_of>`. The calls take the call's `deciders` and nothing about who is
//! asking: `deciders` fills each card's `deciders` and is the showability gate's input, and no
//! call is handed the subject, so no rule here can read who the reader is. Huginn returns the
//! whole record, every note included; Bifrost applies visibility (ruling
//! `desk-read-visibility-whole-record`). Nothing is stored.

use super::*;
use crate::desk::answers_of;
use crate::desk::options_hash;

/// The card document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CardSchema {
    #[serde(rename = "gamecult.fork.card.v0")]
    V0,
}

/// The record document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RecordSchema {
    #[serde(rename = "gamecult.fork.record.v0")]
    V0,
}

/// The entry document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EntrySchema {
    #[serde(rename = "gamecult.fork.entry.v0")]
    V0,
}

/// The list document's schema id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ListSchema {
    #[serde(rename = "gamecult.fork.list.v0")]
    V0,
}

/// One shown option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkOption {
    pub id: String,
    pub text: String,
}

/// A typed reference, as the question holds it (`raised_in`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkRef {
    pub kind: String,
    pub id: String,
}

/// Whether a ruling closed the fork.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Open,
    Closed,
}

/// `gamecult.fork.card.v0`: closed, her field order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkCard {
    #[serde(rename = "schemaVersion")]
    pub schema_version: CardSchema,
    pub fork_id: String,
    pub title: String,
    pub context: String,
    pub options: Vec<ForkOption>,
    pub recommendation: String,
    pub depends: Vec<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub leans: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub flag: Option<String>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub raised_in: Option<ForkRef>,
    pub deciders: Vec<String>,
    pub status: CardStatus,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub ruling: Option<String>,
    pub ruled_from: Vec<String>,
    pub order: u64,
    pub options_hash: String,
}

/// `gamecult.fork.record.v0`: the answers in admission order and the derived counted answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkRecord {
    #[serde(rename = "schemaVersion")]
    pub schema_version: RecordSchema,
    pub fork_id: String,
    pub options_hash: String,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub counted_answer_id: Option<String>,
    pub answers: Vec<ForkDeskAnswer>,
}

/// `gamecult.fork.entry.v0`: a card and its record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkEntry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: EntrySchema,
    pub card: ForkCard,
    pub record: ForkRecord,
}

/// `gamecult.fork.list.v0`: a page of entries, the counts over the showable set and the stamp.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkList {
    #[serde(rename = "schemaVersion")]
    pub schema_version: ListSchema,
    pub mind: String,
    pub stamp: String,
    pub open_count: u64,
    pub recorded_count: u64,
    pub forks: Vec<ForkEntry>,
    #[serde(deserialize_with = "required")]
    #[schemars(required)]
    pub next_cursor: Option<ForkCursor>,
}

/// Which forks a list asks for; absent is both. `recorded` is a closed card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListStatus {
    Open,
    Recorded,
}

/// `c<N>`, N from 1: the cards whose `order` is below N. Built only through `new` or decode, so
/// a held cursor is always a well-formed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkCursor(u64);

impl ForkCursor {
    /// `None` for 0: `c0` is no cursor (her pattern is `^c[1-9][0-9]*$`).
    pub fn new(order: u64) -> Option<Self> {
        (order > 0).then_some(Self(order))
    }

    pub fn order(self) -> u64 {
        self.0
    }
}

impl Serialize for ForkCursor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("c{}", self.0))
    }
}

impl<'de> Deserialize<'de> for ForkCursor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let digits = text.strip_prefix('c').filter(|digits| !digits.starts_with('0') && digits.bytes().all(|byte| byte.is_ascii_digit()));
        digits.and_then(|digits| digits.parse().ok()).and_then(Self::new).ok_or_else(|| serde::de::Error::custom("a cursor is c<N>, N from 1"))
    }
}

impl JsonSchema for ForkCursor {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ForkCursor".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "pattern": "^c[1-9][0-9]*$" })
    }
}

/// Most entries one page holds (CultLib `list.forks` `maxItems`).
pub const DESK_PAGE_MAX: u8 = 50;

/// A page size, 0 to `DESK_PAGE_MAX`; 0 asks for the counts and the stamp only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkLimit(u8);

impl ForkLimit {
    pub fn new(limit: u8) -> Option<Self> {
        (limit <= DESK_PAGE_MAX).then_some(Self(limit))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Serialize for ForkLimit {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for ForkLimit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u8::deserialize(deserializer)?).ok_or_else(|| serde::de::Error::custom("a limit is 0 to 50"))
    }
}

impl JsonSchema for ForkLimit {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ForkLimit".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "integer", "minimum": 0, "maximum": DESK_PAGE_MAX })
    }
}

/// The card of a question held at the snapshot `docs` reads, shown to the call's `deciders`.
fn card_of(docs: &Docs<'_>, key: &str, question: &PipelineQuestion, deciders: &[Short]) -> Result<ForkCard, MindRefusal> {
    let open = docs.in_force(PipelineKind::Question, key);
    // The ruling that closed the fork: the in-force one that answers it (the key order picks if a second ever does).
    let ruling = (!open)
        .then(|| {
            docs.of_kind(PipelineKind::Ruling)
                .filter(|(ruling_key, document)| {
                    matches!(document, PipelineDocument::Ruling(ruling) if ruling.answers.as_ref().is_some_and(|answers| answers.0 == key))
                        && docs.in_force(PipelineKind::Ruling, ruling_key)
                })
                .map(|(ruling_key, _)| ruling_key.to_string())
                .min()
        })
        .flatten();
    Ok(ForkCard {
        schema_version: CardSchema::V0,
        fork_id: key.into(),
        title: question.title.0.clone(),
        context: question.question.0.clone(),
        options: question.options.iter().map(|option| ForkOption { id: option.label.0.clone(), text: option.text.0.clone() }).collect(),
        recommendation: question.recommended.0.clone(),
        depends: question.depends.iter().map(|line| line.0.clone()).collect(),
        leans: None,
        flag: None,
        raised_in: question.raised_in.as_ref().map(|raised| ForkRef { kind: format!("{:?}", raised.kind), id: raised.id.0.clone() }),
        deciders: deciders.iter().map(|decider| decider.0.clone()).collect(),
        status: if open { CardStatus::Open } else { CardStatus::Closed },
        ruling,
        ruled_from: Vec::new(),
        order: docs.ordinal_of(PipelineKind::Question, key).ok_or_else(unreadable_key)?,
        options_hash: options_hash(question).0,
    })
}

/// The record of a fork: its answers in admission order and the answer that counts at this snapshot.
fn record_of(docs: &Docs<'_>, key: &str, options_hash: &str) -> Result<ForkRecord, MindRefusal> {
    let mut stored: Vec<_> = answers_of(docs, key).collect();
    stored.sort_by_key(|(_, answer)| answer.sequence);
    let answers = stored.into_iter().map(|(answer_key, answer)| project(answer_key, answer)).collect::<Result<Vec<_>, _>>()?;
    let counted_answer_id = counted_answer(docs, key).map(|counted| answer_seq(key, &counted).map(n_text).ok_or_else(unreadable_key)).transpose()?;
    Ok(ForkRecord { schema_version: RecordSchema::V0, fork_id: key.into(), options_hash: options_hash.into(), counted_answer_id, answers })
}

/// What `response` encodes to as a CultNet message with one-byte ids; `DESK_ENVELOPE_MARGIN` covers
/// the rest of the envelope (the door's reply with the longest id it echoes is under the bound).
fn page_bytes(response: &DeskResponse) -> Result<usize, MindRefusal> {
    let unavailable = |error: &dyn std::fmt::Display| MindRefusal::Unavailable { detail: format!("a desk page cannot be measured: {error}") };
    let message = crate::envelope::encode_desk_response("m", "forks", response, "r").map_err(|error| unavailable(&error))?;
    cultnet_rs::encode_cultnet_message_to_vec(&message, cultnet_rs::CultNetWireContract::CultNetSchemaV0).map(|bytes| bytes.len()).map_err(|error| unavailable(&error))
}

/// The page a response under construction carries.
fn held(response: &mut DeskResponse) -> &mut ForkList {
    match response {
        DeskResponse::Forks { list, .. } => list,
        _ => unreachable!("a page is built as a page"),
    }
}

fn entry_of(docs: &Docs<'_>, key: &str, question: &PipelineQuestion, deciders: &[Short]) -> Result<ForkEntry, MindRefusal> {
    let card = card_of(docs, key, question, deciders)?;
    let record = record_of(docs, key, &card.options_hash)?;
    Ok(ForkEntry { schema_version: EntrySchema::V0, card, record })
}

impl<S: MindStore> Mind<S> {
    /// One page of the desk's forks, read at one snapshot (`as_of`, also the page's stamp `s<as_of>`).
    ///
    /// The showable forks only: a question `showable` refuses (fewer than two options, a blank option
    /// or depends line, no decider) is in no count and no page. `open_count` and `recorded_count` cover
    /// that whole set whatever `status`, `cursor` and `limit` ask. The page is the forks newest first by
    /// `order` (the question's admission ordinal), those with the asked `status` (`None` is both) and
    /// an order strictly below the cursor's; `limit` 0 returns no entries. `next_cursor` names the last
    /// order returned and is `None` when no further fork matches.
    ///
    /// A page is filled until `limit` entries are held or the next entry would push the encoded
    /// response past `DESK_PAGE_BUDGET` (the one `DESK_MAX_RESPONSE_BYTES` less the envelope margin), so
    /// it may hold fewer than `limit`; counts and stamp are unaffected. Questions admitted in one commit
    /// share an `order`, and the contract's cursor ("an order strictly below N") has no tie-break, so
    /// this is Huginn's rule pending a CultLib contract note: a page never splits such a group. When
    /// `limit` or the budget would end the page inside a group the whole group is taken (the page may
    /// pass `limit` by the group's size), or, when the budget forbids that, the whole group is left for
    /// the next page. A page that cannot hold its first group is refused as
    /// `MindRefusal::DeskForkTooLarge`, naming that group's first fork; nothing is dropped silently.
    ///
    /// The call is handed the call's `deciders` and no subject: they fill each card's `deciders` and
    /// feed the showability gate, and gate nothing else. The record is whole, notes included
    /// (ruling `desk-read-visibility-whole-record`: Bifrost applies visibility, Huginn strips nothing).
    pub fn desk_forks(&self, deciders: &[Short], status: Option<ListStatus>, cursor: Option<ForkCursor>, limit: ForkLimit) -> DeskResponse {
        self.forks_page(deciders, status, cursor, limit).unwrap_or_else(door_refusal)
    }

    /// One fork, as the entry a page carries, read at one snapshot. A fork that is not held, is not a
    /// question or is not showable is `fork_unknown`, the answer path's refusal. Whole record, as `desk_forks`.
    pub fn desk_fork(&self, deciders: &[Short], fork_id: &str) -> DeskResponse {
        self.fork_entry(deciders, fork_id).unwrap_or_else(door_refusal)
    }

    fn forks_page(&self, deciders: &[Short], status: Option<ListStatus>, cursor: Option<ForkCursor>, limit: ForkLimit) -> Result<DeskResponse, MindRefusal> {
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let mut shown = Vec::new();
        for (key, document) in docs.of_kind(PipelineKind::Question) {
            let PipelineDocument::Question(question) = document else { continue };
            if showable(question, deciders) {
                let order = docs.ordinal_of(PipelineKind::Question, key).ok_or_else(unreadable_key)?;
                shown.push((order, key, question, docs.in_force(PipelineKind::Question, key)));
            }
        }
        // Newest first; the key breaks a tie so that a page is the same page on every read.
        shown.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(left.1)));
        let open_count = shown.iter().filter(|fork| fork.3).count() as u64;
        let recorded_count = shown.len() as u64 - open_count;
        let matching: Vec<_> = shown
            .iter()
            .filter(|(order, _, _, open)| {
                cursor.is_none_or(|cursor| *order < cursor.order())
                    && match status {
                        None => true,
                        Some(ListStatus::Open) => *open,
                        Some(ListStatus::Recorded) => !*open,
                    }
            })
            .collect();
        let mut response = DeskResponse::Forks {
            list: ForkList {
                schema_version: ListSchema::V0,
                mind: self.instance().0.clone(),
                stamp: format!("s{}", image.head),
                open_count,
                recorded_count,
                forks: Vec::new(),
                next_cursor: None,
            },
            as_of: image.head,
        };
        let (mut taken, mut last_order) = (0, None);
        while taken < matching.len() && held(&mut response).forks.len() < limit.get() as usize {
            // A group admitted in one commit shares an order and goes whole (see the doc comment).
            let order = matching[taken].0;
            let end = taken + matching[taken..].iter().take_while(|fork| fork.0 == order).count();
            let kept = held(&mut response).forks.len();
            for (_, key, question, _) in &matching[taken..end] {
                held(&mut response).forks.push(entry_of(&docs, key, question, deciders)?);
            }
            held(&mut response).next_cursor = if end < matching.len() { ForkCursor::new(order) } else { None };
            let bytes = page_bytes(&response)?;
            let list = held(&mut response);
            if bytes > DESK_PAGE_BUDGET {
                if kept == 0 {
                    return Ok(DeskResponse::Refused(MindRefusal::DeskForkTooLarge { fork: matching[taken].1.to_string(), order: matching[taken].0, bytes: bytes as u64, limit: DESK_PAGE_BUDGET as u64 }));
                }
                list.forks.truncate(kept);
                list.next_cursor = last_order.and_then(ForkCursor::new);
                break;
            }
            (taken, last_order) = (end, Some(order));
        }
        Ok(response)
    }

    fn fork_entry(&self, deciders: &[Short], fork_id: &str) -> Result<DeskResponse, MindRefusal> {
        let image = self.image()?;
        let docs = Docs::at(image, image.head);
        let Some(PipelineDocument::Question(question)) = docs.in_image(PipelineKind::Question, fork_id) else {
            return Ok(DeskResponse::NotStored(fork_unknown()));
        };
        if !showable(question, deciders) {
            return Ok(DeskResponse::NotStored(fork_unknown()));
        }
        Ok(DeskResponse::Fork { entry: entry_of(&docs, fork_id, question, deciders)?, as_of: image.head })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{decode_desk_request, encode_desk_request, encode_desk_response};
    use crate::fixtures::*;
    use crate::store::test_stores::MemoryStore;
    use crate::{Faculty, PipelineAdmissionBatch, PipelineAdmissionOutcome};
    use eureka_pipeline::{Line, PipelineRef, QuestionOption};
    use serde_json::{Value, json};

    use PipelineDocument as D;

    const CAMPAIGN_ROOT: &str = "eureka-state";
    const LIST: [&str; 3] = ["x", "y", "op"];

    fn fork(n: u32) -> String {
        format!("{CAMPAIGN_ROOT}:question:Q{n}")
    }

    /// A desk of `n` questions, Q1 oldest, each admitted in its own commit so that each has its own order.
    fn desk(n: u32) -> Mind<MemoryStore> {
        let mut mind = seeded();
        for at in 1..=n {
            committed(admit(&mut mind, vec![question_n(at)]));
        }
        mind
    }

    fn list(who: &[&str]) -> Vec<Short> {
        who.iter().map(|id| s(id)).collect()
    }

    fn head(mind: &Mind<MemoryStore>) -> u64 {
        mind.image().unwrap().head
    }

    fn hash_of(mind: &Mind<MemoryStore>, fork: &str) -> String {
        let image = mind.image().unwrap();
        let docs = Docs::at(image, image.head);
        let Some(D::Question(question)) = docs.find(PipelineKind::Question, fork) else { panic!("no question {fork}") };
        options_hash(question).0
    }

    fn request_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!("00000000-0000-4000-8000-{:012x}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// `who`'s answer to `fork` under `hash`, choice A and a note, listing `deciders`.
    fn answer(mind: &mut Mind<MemoryStore>, who: &str, deciders: &[&str], fork: &str, hash: &str, note: Option<&str>) -> DeskResponse {
        let request = ForkAnswerRequest {
            schema_version: RequestSchema::V0,
            fork_id: fork.into(),
            options_hash: hash.into(),
            answer: Some(ForkAnswer { choice: Some("A".into()), note: note.map(str::to_string) }),
            retracted: false,
            retracts: None,
            answered_at: "2026-09-16T11:00:00Z".into(),
            request_id: request_id(),
        };
        mind.desk_answer(&list(deciders), s(who), DeskAuthority::Member, request, now())
    }

    fn close(mind: &mut Mind<MemoryStore>, fork: &str, label: &str) {
        let mut ruled = ruling(label);
        ruled.answers = Some(s(fork));
        ruled.choice = Some(l("A"));
        committed(admit(mind, vec![D::Ruling(ruled)]));
    }

    fn page(response: DeskResponse) -> (ForkList, u64) {
        match response {
            DeskResponse::Forks { list, as_of } => (list, as_of),
            other => panic!("expected a page, got {other:?}"),
        }
    }

    fn entry(response: DeskResponse) -> (ForkEntry, u64) {
        match response {
            DeskResponse::Fork { entry, as_of } => (entry, as_of),
            other => panic!("expected an entry, got {other:?}"),
        }
    }

    fn unknown(response: DeskResponse) {
        assert_eq!(response, DeskResponse::NotStored(fork_unknown()));
    }

    fn ids(list: &ForkList) -> Vec<String> {
        list.forks.iter().map(|entry| entry.card.fork_id.clone()).collect()
    }

    fn limit(n: u8) -> ForkLimit {
        ForkLimit::new(n).unwrap()
    }

    fn all(mind: &Mind<MemoryStore>, n: u8) -> ForkList {
        page(mind.desk_forks(&list(&LIST), None, None, limit(n))).0
    }

    // ---- the reads, one snapshot ----

    #[test]
    fn a_stale_answer_then_the_read_cards_hash_counts() {
        let mut mind = desk(1);
        let stale = answer(&mut mind, "x", &LIST, &fork(1), &"0".repeat(64), None);
        let DeskResponse::Answered { answer: stale, .. } = stale else { panic!("a stale answer is stored") };
        assert_eq!(stale.code, Some(ForkAnswerCode::StaleFork));
        let (shown, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        assert_eq!(shown.card.options_hash, hash_of(&mind, &fork(1)));
        assert_ne!(shown.card.options_hash, "0".repeat(64));
        assert_eq!((shown.record.answers.len(), shown.record.counted_answer_id.clone()), (1, None));
        assert_eq!(shown.record.options_hash, shown.card.options_hash);
        // The hash the card showed counts.
        let DeskResponse::Answered { answer: counted, counted_answer_id, .. } = answer(&mut mind, "x", &LIST, &fork(1), &shown.card.options_hash, None) else {
            panic!("stored")
        };
        assert_eq!((counted.outcome.as_str(), counted.answer_id.as_str()), ("Counted", "n2"));
        assert_eq!(counted_answer_id.as_deref(), Some("n2"));
        let (after, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        assert_eq!(after.record.counted_answer_id.as_deref(), Some("n2"));
        let names: Vec<_> = after.record.answers.iter().map(|answer| answer.answer_id.as_str()).collect();
        assert_eq!(names, ["n1", "n2"], "answers in admission order, named n<seq>");
    }

    #[test]
    fn list_then_read_at_one_stamp() {
        let mut mind = desk(3);
        let (first, as_of) = page(mind.desk_forks(&list(&LIST), None, None, limit(2)));
        assert_eq!(as_of, head(&mind));
        assert_eq!(first.stamp, format!("s{as_of}"));
        assert_eq!(first.mind, INSTANCE);
        assert_eq!(ids(&first), [fork(3), fork(2)]);
        let next = first.next_cursor.expect("one more fork waits");
        assert_eq!(next.order(), first.forks[1].card.order);
        let (second, _) = page(mind.desk_forks(&list(&LIST), None, Some(next), limit(2)));
        assert_eq!((ids(&second), second.next_cursor), (vec![fork(1)], None));
        // The read at that stamp is the entry the list carried.
        let (read, read_at) = entry(mind.desk_fork(&list(&LIST), &fork(2)));
        assert_eq!(read_at, as_of);
        assert_eq!(read, first.forks[1]);
        // Writing moves the stamp, and only then.
        let hash = hash_of(&mind, &fork(2));
        answer(&mut mind, "x", &LIST, &fork(2), &hash, None);
        let (moved, moved_at) = page(mind.desk_forks(&list(&LIST), None, None, limit(2)));
        assert_eq!((moved.stamp.clone(), moved_at), (format!("s{}", head(&mind)), head(&mind)));
        assert_ne!(moved.stamp, first.stamp);
        assert_eq!(moved.forks[1].record.answers.len(), 1);
    }

    #[test]
    fn forks_come_newest_first_by_order_and_the_cursor_is_strictly_below() {
        let mind = desk(4);
        let everything = all(&mind, 50);
        assert_eq!(ids(&everything), [fork(4), fork(3), fork(2), fork(1)]);
        let orders: Vec<u64> = everything.forks.iter().map(|entry| entry.card.order).collect();
        assert!(orders.windows(2).all(|pair| pair[0] > pair[1]), "{orders:?}");
        assert_eq!(everything.next_cursor, None, "the last page has no cursor");
        // A cursor names an order and returns the cards strictly below it.
        assert_eq!(orders, [3, 2, 1, 0], "the ranks, from 0");
        assert_eq!(ForkCursor::new(0), None, "no cursor names rank 0: nothing is below it");
        for (at, expected) in [(0, 3), (1, 2), (2, 1)] {
            let below = ForkCursor::new(orders[at]).unwrap();
            let (rest, _) = page(mind.desk_forks(&list(&LIST), None, Some(below), limit(50)));
            assert_eq!(rest.forks.len(), expected, "below order {}", orders[at]);
            assert!(rest.forks.iter().all(|entry| entry.card.order < orders[at]));
        }
        // A page that stops short names the last order it returned.
        let (short, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(1)));
        assert_eq!((ids(&short), short.next_cursor.map(ForkCursor::order)), (vec![fork(4)], Some(orders[0])));
        // A page that ends exactly on the last fork has no cursor.
        let (exact, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(4)));
        assert_eq!((exact.forks.len(), exact.next_cursor), (4, None));
        let (over, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(50)));
        assert_eq!(over.forks.len(), 4);
    }

    #[test]
    fn status_picks_open_or_recorded_and_the_counts_ignore_it_and_the_page() {
        let mut mind = desk(5);
        close(&mut mind, &fork(1), "R1");
        close(&mut mind, &fork(4), "R4");
        let expected = (3, 2);
        for status in [None, Some(ListStatus::Open), Some(ListStatus::Recorded)] {
            for cursor in [None, ForkCursor::new(1), ForkCursor::new(3), ForkCursor::new(1000)] {
                for size in [0, 1, 50] {
                    let (shown, _) = page(mind.desk_forks(&list(&LIST), status, cursor, limit(size)));
                    assert_eq!((shown.open_count, shown.recorded_count), expected, "{status:?} {cursor:?} {size}");
                    assert!(shown.forks.len() <= size as usize);
                    for entry in &shown.forks {
                        let want = match status {
                            None => entry.card.status,
                            Some(ListStatus::Open) => CardStatus::Open,
                            Some(ListStatus::Recorded) => CardStatus::Closed,
                        };
                        assert_eq!(entry.card.status, want, "{status:?}");
                    }
                }
            }
        }
        let (open, _) = page(mind.desk_forks(&list(&LIST), Some(ListStatus::Open), None, limit(50)));
        assert_eq!(ids(&open), [fork(5), fork(3), fork(2)]);
        let (recorded, _) = page(mind.desk_forks(&list(&LIST), Some(ListStatus::Recorded), None, limit(50)));
        assert_eq!(ids(&recorded), [fork(4), fork(1)]);
        // A cursor and a status together: the cards below the cursor that have the status.
        let below_four = ForkCursor::new(recorded.forks[0].card.order);
        let (rest, _) = page(mind.desk_forks(&list(&LIST), Some(ListStatus::Open), below_four, limit(50)));
        assert_eq!(ids(&rest), [fork(3), fork(2)]);
    }

    #[test]
    fn limit_zero_is_the_counts_and_the_stamp_and_no_entries() {
        let mind = desk(3);
        let (counts, as_of) = page(mind.desk_forks(&list(&LIST), None, None, limit(0)));
        assert_eq!((counts.forks.len(), counts.next_cursor), (0, None));
        assert_eq!((counts.open_count, counts.recorded_count), (3, 0));
        assert_eq!((counts.stamp, as_of), (format!("s{}", head(&mind)), head(&mind)));
        let (one, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(1)));
        assert_eq!(one.forks.len(), 1, "limit 1 is one entry, not none");
    }

    #[test]
    fn a_desk_with_no_forks_is_an_empty_page() {
        let mind = seeded();
        let (empty, as_of) = page(mind.desk_forks(&list(&LIST), None, None, limit(10)));
        assert_eq!((empty.open_count, empty.recorded_count, empty.forks.len(), empty.next_cursor), (0, 0, 0, None));
        assert_eq!(empty.stamp, format!("s{as_of}"));
    }

    #[test]
    fn an_unshowable_fork_is_not_counted_not_listed_and_unknown_on_a_read() {
        let mut mind = desk(2);
        // A question one decider cannot show: a blank depends line (the gate is `showable`).
        let mut blank = question("Q3", &["A", "B"], "A");
        let D::Question(held) = &mut blank else { unreachable!() };
        held.depends = vec!["Cut 3a".into(), "".into()];
        let admitted = admit(&mut mind, vec![blank]);
        let unshowable = if matches!(admitted, PipelineAdmissionOutcome::Committed { .. }) {
            fork(3)
        } else {
            let mut blank = question("Q3", &["A", "B"], "A");
            let D::Question(held) = &mut blank else { unreachable!() };
            held.options[0] = QuestionOption { label: l("A"), text: "".into() };
            committed(admit(&mut mind, vec![blank]));
            fork(3)
        };
        let shown = all(&mind, 50);
        assert_eq!(ids(&shown), [fork(2), fork(1)]);
        assert_eq!((shown.open_count, shown.recorded_count), (2, 0), "the count is over the showable set");
        unknown(mind.desk_fork(&list(&LIST), &unshowable));
        // No decider, nothing is shown at all: the gate needs one.
        let (none, _) = page(mind.desk_forks(&[], None, None, limit(50)));
        assert_eq!((none.forks.len(), none.open_count, none.recorded_count), (0, 0, 0));
        unknown(mind.desk_fork(&[], &fork(1)));
    }

    #[test]
    fn an_unknown_fork_is_fork_unknown_whatever_it_is_called() {
        let mut mind = desk(1);
        close(&mut mind, &fork(1), "R1");
        let ruling_key = format!("{CAMPAIGN_ROOT}:ruling:R1");
        for id in [fork(9).as_str(), "", "Q1", "no-colons", "eureka-state:question:", ruling_key.as_str(), &"x".repeat(5000), "other-campaign:question:Q1"] {
            unknown(mind.desk_fork(&list(&LIST), id));
        }
        // And nothing was written by looking.
        let before = head(&mind);
        let _ = mind.desk_fork(&list(&LIST), &fork(1));
        let _ = mind.desk_forks(&list(&LIST), None, None, limit(50));
        assert_eq!(head(&mind), before);
    }

    // ---- the trust boundary: the whole record, whoever asks ----

    #[test]
    fn the_whole_record_comes_back_whoever_the_deciders_list_leaves_out() {
        let mut mind = desk(2);
        let hash = hash_of(&mind, &fork(1));
        let note = "Keep it local: a slot mark is the author's call.";
        // Y, a decider of that call, answers with a note; a stranger is refused with a note too.
        answer(&mut mind, "y", &LIST, &fork(1), &hash, Some(note));
        answer(&mut mind, "stranger", &LIST, &fork(1), &hash, Some("a refused person's words"));
        // The reads carry another list that names neither of them: no call is handed a subject.
        let narrow = list(&["x"]);
        let (read, _) = entry(mind.desk_fork(&narrow, &fork(1)));
        let notes: Vec<_> = read.record.answers.iter().map(|answer| (answer.answerer.as_str(), answer.note.as_deref())).collect();
        assert_eq!(notes, [("y", Some(note)), ("stranger", Some("a refused person's words"))]);
        let (listed, _) = page(mind.desk_forks(&narrow, None, None, limit(50)));
        assert_eq!(listed.forks[1], read, "the list carries the same whole record");
        // The list only fills the card.
        assert_eq!(read.card.deciders, ["x"]);
        let (wide, _) = entry(mind.desk_fork(&list(&["x", "y", "z"]), &fork(1)));
        assert_eq!(wide.card.deciders, ["x", "y", "z"]);
        assert_eq!(ForkEntry { card: ForkCard { deciders: read.card.deciders.clone(), ..wide.card.clone() }, ..wide }, read, "nothing else moved");
    }

    // ---- the projection ----

    #[test]
    fn the_card_is_the_question_as_shown() {
        let mut mind = seeded();
        let mut shown = question("Q1", &["A", "B"], "B");
        let D::Question(held) = &mut shown else { unreachable!() };
        held.title = "Share the mark?".into();
        held.question = "Should a slot mark be shared?".into();
        held.options = vec![QuestionOption { label: l("A"), text: "Share it.".into() }, QuestionOption { label: l("B"), text: "Keep it local.".into() }];
        held.depends = vec!["eureka-state:question:other".into(), "a prose line".into()];
        held.raised_in = Some(PipelineRef { kind: PipelineKind::Ruling, id: s("eureka-state:ruling:R0") });
        committed(admit(&mut mind, vec![D::Ruling(ruling("R0"))]));
        committed(admit(&mut mind, vec![shown]));
        let (read, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        let card = &read.card;
        assert_eq!((card.fork_id.as_str(), card.title.as_str(), card.context.as_str()), (fork(1).as_str(), "Share the mark?", "Should a slot mark be shared?"));
        let options: Vec<_> = card.options.iter().map(|option| (option.id.as_str(), option.text.as_str())).collect();
        assert_eq!(options, [("A", "Share it."), ("B", "Keep it local.")]);
        assert_eq!(card.recommendation, "B");
        assert_eq!(card.depends, ["eureka-state:question:other", "a prose line"]);
        assert_eq!(card.raised_in, Some(ForkRef { kind: "Ruling".into(), id: "eureka-state:ruling:R0".into() }));
        assert_eq!((card.leans.clone(), card.flag.clone(), card.status, card.ruling.clone(), card.ruled_from.clone()), (None, None, CardStatus::Open, None, vec![]));
        assert_eq!(card.deciders, LIST);
        assert_eq!(card.options_hash, hash_of(&mind, &fork(1)));
        assert_eq!(card.order, 0, "the only question is rank 0");
        assert_eq!(read.record.fork_id, fork(1));
        assert_eq!((read.record.counted_answer_id, read.record.answers.len()), (None, 0));
    }

    #[test]
    fn a_ruling_closes_the_card_and_names_itself() {
        let mut mind = desk(2);
        close(&mut mind, &fork(1), "R1");
        let (closed, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        assert_eq!((closed.card.status, closed.card.ruling.as_deref()), (CardStatus::Closed, Some("eureka-state:ruling:R1")));
        let (open, _) = entry(mind.desk_fork(&list(&LIST), &fork(2)));
        assert_eq!((open.card.status, open.card.ruling), (CardStatus::Open, None));
    }

    #[test]
    fn the_counted_answer_is_derived_at_the_head_the_read_was_made() {
        let mut mind = desk(1);
        let hash = hash_of(&mind, &fork(1));
        answer(&mut mind, "x", &LIST, &fork(1), &hash, None);
        answer(&mut mind, "y", &LIST, &fork(1), &hash, None);
        let (read, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        assert_eq!(read.record.counted_answer_id.as_deref(), Some("n1"));
        let outcomes: Vec<_> = read.record.answers.iter().map(|answer| (answer.outcome.as_str(), answer.of.as_deref())).collect();
        assert_eq!(outcomes, [("Counted", None), ("Revision", Some("n1"))]);
        // Retracting the counted answer moves the count to the next.
        let retract = ForkAnswerRequest {
            schema_version: RequestSchema::V0,
            fork_id: fork(1),
            options_hash: hash,
            answer: None,
            retracted: true,
            retracts: Some("n1".into()),
            answered_at: "2026-09-16T11:00:00Z".into(),
            request_id: request_id(),
        };
        let stored = mind.desk_answer(&list(&LIST), s("x"), DeskAuthority::Member, retract, now());
        assert!(matches!(stored, DeskResponse::Answered { .. }));
        let (after, _) = entry(mind.desk_fork(&list(&LIST), &fork(1)));
        assert_eq!(after.record.counted_answer_id.as_deref(), Some("n2"));
    }

    // ---- Libby's vectors: the entry and list bytes ----

    fn transport() -> Value {
        serde_json::from_str::<Value>(include_str!("../../vectors/fork-card-vectors.json")).unwrap()["transport"].clone()
    }

    fn bytes_of(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap()).collect()
    }

    fn encode<T: Serialize>(value: &T) -> Vec<u8> {
        rmp_serde::to_vec_named(value).unwrap()
    }

    #[test]
    fn entry_and_list_vectors_round_trip_byte_for_byte() {
        let file = transport();
        let entries = file["entry"]["valid"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        for row in entries {
            let bytes = bytes_of(row["messagePackHex"].as_str().unwrap());
            let decoded: ForkEntry = rmp_serde::from_slice(&bytes).unwrap();
            assert_eq!(encode(&decoded), bytes, "{}", row["name"]);
            assert_eq!(decoded, serde_json::from_value::<ForkEntry>(row["message"].clone()).unwrap(), "{}", row["name"]);
        }
        let scenarios = file["listScenarios"].as_array().unwrap();
        assert_eq!(scenarios.len(), 5);
        let mut steps = 0;
        for scenario in scenarios {
            for step in scenario["steps"].as_array().unwrap() {
                let bytes = bytes_of(step["responseHex"].as_str().unwrap());
                let decoded: ForkList = rmp_serde::from_slice(&bytes).unwrap();
                assert_eq!(encode(&decoded), bytes, "{}", scenario["name"]);
                assert_eq!(decoded, serde_json::from_value::<ForkList>(step["response"].clone()).unwrap());
                steps += 1;
            }
        }
        assert_eq!(steps, 7);
    }

    #[test]
    fn invalid_entries_and_lists_do_not_decode_as_documents() {
        let file = transport();
        // Closed documents: a key outside the set, a missing key and a bad cursor are not documents.
        let mut entry = file["entry"]["valid"][0]["message"].clone();
        entry["extra"] = json!(1);
        assert!(serde_json::from_value::<ForkEntry>(entry).is_err());
        let mut entry = file["entry"]["valid"][0]["message"].clone();
        entry.as_object_mut().unwrap().remove("record");
        assert!(serde_json::from_value::<ForkEntry>(entry).is_err());
        let mut card = file["entry"]["valid"][0]["message"]["card"].clone();
        card.as_object_mut().unwrap().remove("leans");
        assert!(serde_json::from_value::<ForkCard>(card).is_err(), "a nullable key is still written");
        for row in file["listInvalid"].as_array().unwrap() {
            if row["name"] == "next_cursor_malformed" {
                assert!(serde_json::from_value::<ForkList>(row["message"].clone()).is_err(), "{}", row["name"]);
            }
        }
    }

    /// A card-shaped projection of the vector's list step reproduces the vector's request fields.
    #[test]
    fn the_list_request_vectors_decode_into_the_flat_variant_fields() {
        let file = transport();
        let flat = |message: &Value| {
            json!({ "Forks": {
                "mind": message["mind"], "subject": "s", "deciders": ["s"],
                "status": message["status"], "cursor": message["cursor"], "limit": message["limit"]
            }})
        };
        let valid = file["listRequest"]["valid"].as_array().unwrap();
        assert_eq!(valid.len(), 3);
        for row in valid {
            let decoded: DeskRequest = serde_json::from_value(flat(&row["message"])).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
            let DeskRequest::Forks { status, cursor, limit, .. } = decoded else { panic!() };
            assert_eq!(serde_json::to_value(status).unwrap(), row["message"]["status"], "{}", row["name"]);
            assert_eq!(serde_json::to_value(cursor).unwrap(), row["message"]["cursor"], "{}", row["name"]);
            assert_eq!(serde_json::to_value(limit).unwrap(), row["message"]["limit"], "{}", row["name"]);
        }
        let mut refused = 0;
        for row in file["listRequest"]["invalid"].as_array().unwrap() {
            // The envelope carries the mind and the closed set; the fields that are the list's are these.
            if ["limit_51", "limit_negative", "status_all", "cursor_malformed", "cursor_zero"].contains(&row["name"].as_str().unwrap()) {
                assert!(serde_json::from_value::<DeskRequest>(flat(&row["message"])).is_err(), "{}", row["name"]);
                refused += 1;
            }
        }
        assert_eq!(refused, 5);
    }

    fn forks_json(status: Value, cursor: Value, limit: Value) -> Value {
        json!({ "Forks": { "mind": INSTANCE, "subject": "s", "deciders": ["s"], "status": status, "cursor": cursor, "limit": limit } })
    }

    #[test]
    fn page_limit_is_zero_to_fifty_and_nothing_else() {
        for ok in [0, 1, 49, 50] {
            assert!(serde_json::from_value::<DeskRequest>(forks_json(Value::Null, Value::Null, json!(ok))).is_ok(), "{ok}");
        }
        for bad in [json!(51), json!(255), json!(256), json!(-1), json!(1.5), json!("10"), Value::Null] {
            assert!(serde_json::from_value::<DeskRequest>(forks_json(Value::Null, Value::Null, bad.clone())).is_err(), "{bad}");
        }
        assert_eq!(ForkLimit::new(DESK_PAGE_MAX).map(ForkLimit::get), Some(50));
        assert_eq!(ForkLimit::new(DESK_PAGE_MAX + 1), None);
    }

    #[test]
    fn a_cursor_is_c_and_a_positive_decimal() {
        for ok in ["c1", "c9", "c10", "c412", "c18446744073709551615"] {
            let read: DeskRequest = serde_json::from_value(forks_json(Value::Null, json!(ok), json!(5))).unwrap_or_else(|error| panic!("{ok}: {error}"));
            let DeskRequest::Forks { cursor, .. } = read else { panic!() };
            assert_eq!(serde_json::to_value(cursor).unwrap(), json!(ok));
        }
        for bad in ["c0", "c01", "c", "", "4", "C4", "c-1", "c+1", "c 4", "c4 ", "cx", "c18446744073709551616", "next"] {
            assert!(serde_json::from_value::<DeskRequest>(forks_json(Value::Null, json!(bad), json!(5))).is_err(), "{bad:?}");
        }
        assert_eq!(ForkCursor::new(0), None);
    }

    #[test]
    fn status_is_open_recorded_or_null_and_every_key_is_written() {
        for ok in [json!("open"), json!("recorded"), Value::Null] {
            assert!(serde_json::from_value::<DeskRequest>(forks_json(ok.clone(), Value::Null, json!(5))).is_ok(), "{ok}");
        }
        for bad in [json!("all"), json!("closed"), json!(""), json!("Open"), json!(1)] {
            assert!(serde_json::from_value::<DeskRequest>(forks_json(bad.clone(), Value::Null, json!(5))).is_err(), "{bad}");
        }
        for missing in ["status", "cursor", "limit"] {
            let mut value = forks_json(Value::Null, Value::Null, json!(5));
            value["Forks"].as_object_mut().unwrap().remove(missing);
            assert!(serde_json::from_value::<DeskRequest>(value).is_err(), "{missing} may not be left out");
        }
    }

    /// The reads carry no `authority` (CultLib's `list_request` and `read_request` are closed and have none).
    #[test]
    fn a_read_that_names_an_authority_is_not_a_request() {
        let mut forks = forks_json(Value::Null, Value::Null, json!(5));
        let mut fork = json!({ "Fork": { "mind": INSTANCE, "subject": "s", "deciders": ["s"], "fork_id": fork(1) } });
        assert!(serde_json::from_value::<DeskRequest>(forks.clone()).is_ok() && serde_json::from_value::<DeskRequest>(fork.clone()).is_ok());
        for authority in [json!("Operator"), json!("Member"), Value::Null] {
            forks["Forks"]["authority"] = authority.clone();
            fork["Fork"]["authority"] = authority.clone();
            assert!(serde_json::from_value::<DeskRequest>(forks.clone()).is_err(), "Forks with {authority}");
            assert!(serde_json::from_value::<DeskRequest>(fork.clone()).is_err(), "Fork with {authority}");
        }
        // And through the envelope the door reads.
        let typed = DeskRequest::Forks { mind: slug(INSTANCE), subject: s("s"), deciders: list(&["s"]), status: None, cursor: None, limit: limit(5) };
        let message = encode_desk_request("m-1", &typed, None).unwrap();
        assert_eq!(decode_desk_request(&message).unwrap().1, typed);
    }

    // ---- the response bound ----

    /// A realistic question: the longest context the stored bound allows is far over this, so a page of
    /// these is the ordinary case, not the worst.
    fn wordy(n: u32) -> PipelineDocument {
        let mut shown = question_n(n);
        let D::Question(held) = &mut shown else { unreachable!() };
        held.title = format!("Fork {n}: who owns the shared slot mark when two authors disagree?").as_str().into();
        held.question = "Should a slot mark be shared between authors? ".repeat(18).as_str().into();
        held.options = ["A", "B", "C", "D"].iter().map(|label| QuestionOption { label: l(label), text: format!("Option {label}: {}", "keep it where it was made. ".repeat(3)).as_str().into() }).collect();
        held.depends = (0..3).map(|at| Line::from(format!("eureka-state:question:dependency-{at}").as_str())).collect();
        shown
    }

    fn encoded(message_id: &str, runtime: &str, response: &DeskResponse) -> usize {
        let message = encode_desk_response(message_id, "forks", response, runtime).unwrap();
        cultnet_rs::encode_cultnet_message_to_vec(&message, cultnet_rs::CultNetWireContract::CultNetSchemaV0).unwrap().len()
    }

    /// The reply as the door sends it: the longest message id it echoes, the daemon's runtime id.
    fn on_the_wire(response: &DeskResponse) -> usize {
        encoded(&"m".repeat(256), &format!("huginn-{INSTANCE}"), response)
    }

    /// The reply as the page fill measures it: the message with one-byte ids.
    fn bare(response: &DeskResponse) -> usize {
        encoded("m", "r", response)
    }

    /// Every page from the newest fork on, following the cursors, with the response each came in.
    fn pages(mind: &Mind<MemoryStore>, size: u8) -> Vec<(DeskResponse, ForkList)> {
        let (mut cursor, mut out) = (None, Vec::new());
        loop {
            let response = mind.desk_forks(&list(&LIST), None, cursor, limit(size));
            let (page, _) = page(response.clone());
            cursor = page.next_cursor;
            out.push((response, page));
            assert!(out.len() < 200, "the cursors do not advance");
            if cursor.is_none() {
                return out;
            }
        }
    }

    fn admitted_each(range: std::ops::RangeInclusive<u32>) -> Mind<MemoryStore> {
        let mut mind = seeded();
        for n in range {
            committed(admit(&mut mind, vec![wordy(n)]));
        }
        mind
    }

    #[test]
    fn a_page_stops_at_the_response_budget_and_the_pages_are_every_fork_once_in_order() {
        let mind = admitted_each(1..=50);
        let all = pages(&mind, 50);
        assert!(all.len() > 1, "fifty wordy cards do not fit one page");
        let seen: Vec<String> = all.iter().flat_map(|(_, page)| ids(page)).collect();
        assert_eq!(seen, (1..=50).rev().map(fork).collect::<Vec<_>>(), "every fork once, newest first");
        let first = &all[0].1;
        for (response, page) in &all {
            assert!(on_the_wire(response) <= DESK_MAX_RESPONSE_BYTES, "{} bytes", on_the_wire(response));
            assert!(bare(response) <= DESK_PAGE_BUDGET);
            assert!(!page.forks.is_empty() && page.forks.len() < 50, "short of the limit by bytes");
            assert_eq!((page.open_count, page.recorded_count, &page.stamp), (50, 0, &first.stamp), "counts and stamp are the whole desk's");
        }
        assert!(all[..all.len() - 1].iter().all(|(_, page)| page.next_cursor.is_some()) && all.last().unwrap().1.next_cursor.is_none());
    }

    /// The last entry of the page moves to the next page when one more byte of its text would take
    /// the page past the budget, and not a byte sooner.
    #[test]
    fn the_page_budget_is_exact() {
        let build = |forks: u32, text: usize| {
            let mut mind = seeded();
            let mut oldest = wordy(1);
            let D::Question(held) = &mut oldest else { unreachable!() };
            held.question = "z".repeat(text).as_str().into();
            committed(admit(&mut mind, vec![oldest]));
            for n in 2..=forks {
                committed(admit(&mut mind, vec![wordy(n)]));
            }
            mind
        };
        let first_page = |mind: &Mind<MemoryStore>| page(mind.desk_forks(&list(&LIST), None, None, limit(50))).0;
        // The most forks that fit one page with the oldest one's text short.
        let fits = (2..=50).take_while(|forks| first_page(&build(*forks, 300)).forks.len() == *forks as usize).last().expect("two fit");
        assert!(fits < 50, "the desk outgrows a page");
        // Grow the oldest fork's text one byte at a time; the page holds it until the budget is exact.
        let (mut low, mut high) = (300_usize, 6000_usize);
        while high - low > 1 {
            let mid = (low + high) / 2;
            if first_page(&build(fits, mid)).forks.len() == fits as usize { low = mid } else { high = mid }
        }
        let (in_page, over) = (build(fits, low), build(fits, low + 1));
        let held = page(in_page.desk_forks(&list(&LIST), None, None, limit(50)));
        assert_eq!((held.0.forks.len(), held.0.next_cursor), (fits as usize, None));
        let size = bare(&DeskResponse::Forks { list: held.0, as_of: held.1 });
        // The wire grows in steps of up to three bytes per text byte, so "exact" is within one step.
        assert!(size <= DESK_PAGE_BUDGET && DESK_PAGE_BUDGET - size < 4, "{size} against {DESK_PAGE_BUDGET}");
        let (moved, _) = page(over.desk_forks(&list(&LIST), None, None, limit(50)));
        assert_eq!(moved.forks.len(), fits as usize - 1, "one byte more and the oldest fork goes to the next page");
        // The page it would have been, composed from the single reads, is past the budget.
        let would_be = ForkList { forks: (1..=fits).rev().map(|n| entry(over.desk_fork(&list(&LIST), &fork(n))).0).collect(), next_cursor: None, ..moved.clone() };
        assert!(bare(&DeskResponse::Forks { list: would_be, as_of: head(&over) }) > DESK_PAGE_BUDGET);
        assert_eq!(moved.next_cursor.map(ForkCursor::order), moved.forks.last().map(|entry| entry.card.order));
        let (rest, _) = page(over.desk_forks(&list(&LIST), None, moved.next_cursor, limit(50)));
        assert_eq!(ids(&rest), [fork(1)]);
    }

    /// Grows fork `n`'s record with notes until its entry alone encodes to more than `bytes`.
    fn grown_past(mind: &mut Mind<MemoryStore>, n: u32, bytes: usize) {
        let hash = hash_of(mind, &fork(n));
        let note = "a long reason, ".repeat(60);
        let mut added = 0;
        while bare(&mind.desk_fork(&list(&LIST), &fork(n))) <= bytes {
            let DeskResponse::Answered { .. } = answer(mind, "x", &LIST, &fork(n), &hash, Some(&note)) else { panic!("the note was not stored") };
            added += 1;
            assert!(added < 200, "the record does not grow");
        }
    }

    fn docs_order(mind: &Mind<MemoryStore>, key: &str) -> u64 {
        let image = mind.image().unwrap();
        Docs::at(image, image.head).ordinal_of(PipelineKind::Question, key).unwrap()
    }

    fn orders(list: &ForkList) -> Vec<u64> {
        list.forks.iter().map(|entry| entry.card.order).collect()
    }

    /// A fork whose entry alone is past the budget cannot be paged: the page that starts at it is the
    /// refusal naming it and its order, the forks before it page, and the cursor the refusal names
    /// reaches the forks below it. The list is not bricked.
    #[test]
    fn a_lone_oversize_entry_is_refused_by_name_and_the_older_forks_are_still_reachable() {
        let mut mind = desk(3);
        grown_past(&mut mind, 2, DESK_PAGE_BUDGET);
        // The newest fork pages alone: the oversize one would bust the page beside it.
        let (first, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(50)));
        assert_eq!(ids(&first), [fork(3)]);
        assert_eq!(first.next_cursor.map(ForkCursor::order), Some(2));
        let response = mind.desk_forks(&list(&LIST), None, first.next_cursor, limit(50));
        let DeskResponse::Refused(MindRefusal::DeskForkTooLarge { fork: named, order, bytes, limit: bound }) = response else { panic!("{response:?}") };
        assert_eq!((named, order, bound), (fork(2), 1, DESK_PAGE_BUDGET as u64));
        assert!(bytes > bound);
        // The order the refusal names is the order of the fork it names; its cursor goes on past it.
        assert_eq!(entry(mind.desk_fork(&list(&LIST), &fork(2))).0.card.order, order);
        let (rest, _) = page(mind.desk_forks(&list(&LIST), None, ForkCursor::new(order), limit(50)));
        assert_eq!((ids(&rest), rest.next_cursor), (vec![fork(1)], None));
    }

    /// A batch of 64 questions admitted in ONE commit pages whole at every limit: no page passes its
    /// limit or 50, and the pages are every fork once, newest first, with unique orders.
    #[test]
    fn a_batch_admitted_in_one_commit_pages_every_fork_once_newest_first() {
        let mut mind = seeded();
        committed(admit(&mut mind, (1..=64).map(question_n).collect()));
        let mut expected: Vec<String> = (1..=64).map(fork).collect();
        expected.sort();
        expected.reverse();
        for size in [1, 2, 3, 50] {
            let all = pages(&mind, size);
            let seen: Vec<String> = all.iter().flat_map(|(_, page)| ids(page)).collect();
            assert_eq!(seen, expected, "limit {size}: every fork once, newest first");
            for (_, page) in &all {
                assert!(page.forks.len() <= size as usize && page.forks.len() <= 50, "limit {size}: a page of {}", page.forks.len());
            }
            let seen: Vec<u64> = all.iter().flat_map(|(_, page)| orders(page)).collect();
            assert_eq!(seen, (0..64).rev().collect::<Vec<u64>>(), "limit {size}: the orders are the ranks 63 down to 0");
        }
    }

    /// A rank is the position in (admission, key) order among all the questions, so a question admitted
    /// later is the newest whatever its key, and a cursor held from before still resumes correctly.
    #[test]
    fn a_rank_is_stable_when_more_questions_are_admitted() {
        let mut mind = seeded();
        committed(admit(&mut mind, (1..=5).map(question_n).collect()));
        let (first, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(2)));
        assert_eq!((orders(&first), first.next_cursor.map(ForkCursor::order)), (vec![4, 3], Some(3)));
        let held = first.next_cursor;
        let (before, _) = page(mind.desk_forks(&list(&LIST), None, held, limit(50)));
        committed(admit(&mut mind, vec![question_n(6)]));
        committed(admit(&mut mind, vec![question_n(0)]));
        let (after, _) = page(mind.desk_forks(&list(&LIST), None, held, limit(50)));
        assert_eq!(after, ForkList { stamp: after.stamp.clone(), ..before.clone() }, "the cursor from before resumes the same forks");
        let everything = all(&mind, 50);
        assert_eq!(ids(&everything), [fork(0), fork(6), fork(5), fork(4), fork(3), fork(2), fork(1)]);
        assert_eq!(orders(&everything), [6, 5, 4, 3, 2, 1, 0], "Q0 sorts first by key and is still the newest");
    }

    /// An unshowable question keeps its rank, so the others' ranks leave a gap and do not shift, and the
    /// call's deciders list changes nothing about a rank.
    #[test]
    fn an_unshowable_question_leaves_a_gap_and_never_shifts_a_rank() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(1)]));
        let mut blank = question("Q2", &["A", "B"], "A");
        let D::Question(held) = &mut blank else { unreachable!() };
        held.depends = vec!["Cut 3a".into(), "".into()];
        if !matches!(admit(&mut mind, vec![blank]), PipelineAdmissionOutcome::Committed { .. }) {
            let mut blank = question("Q2", &["A", "B"], "A");
            let D::Question(held) = &mut blank else { unreachable!() };
            held.options[0] = QuestionOption { label: l("A"), text: "".into() };
            committed(admit(&mut mind, vec![blank]));
        }
        committed(admit(&mut mind, vec![question_n(3)]));
        for who in [&LIST[..], &["x"][..], &["z"][..]] {
            let (shown, _) = page(mind.desk_forks(&list(who), None, None, limit(50)));
            assert_eq!((ids(&shown), orders(&shown)), (vec![fork(3), fork(1)], vec![2, 0]), "{who:?}: rank 1 is the gap");
            assert_eq!(entry(mind.desk_fork(&list(who), &fork(3))).0.card.order, 2, "{who:?}");
        }
        committed(admit(&mut mind, vec![question_n(4)]));
        assert_eq!(orders(&all(&mind, 50)), [3, 2, 0]);
    }

    /// A page where one entry fits and the next busts the budget is that one entry and a cursor; it is
    /// neither empty nor a refusal.
    #[test]
    fn a_page_of_one_when_the_next_entry_busts_the_budget() {
        let mut mind = desk(2);
        for n in [1, 2] {
            grown_past(&mut mind, n, DESK_PAGE_BUDGET * 55 / 100);
            assert!(bare(&mind.desk_fork(&list(&LIST), &fork(n))) <= DESK_PAGE_BUDGET, "each fork alone fits");
        }
        let (one, _) = page(mind.desk_forks(&list(&LIST), None, None, limit(50)));
        assert_eq!((ids(&one), one.next_cursor.map(ForkCursor::order)), (vec![fork(2)], Some(1)));
        let (rest, _) = page(mind.desk_forks(&list(&LIST), None, one.next_cursor, limit(50)));
        assert_eq!((ids(&rest), rest.next_cursor), (vec![fork(1)], None));
    }

    /// The invariants of `order`: a unique 0-based rank by (admission, key), stable under later
    /// admissions, order 0 legal and reached by the page boundary `c1`.
    #[test]
    fn order_is_a_unique_zero_based_rank_stable_under_appends() {
        let mut mind = seeded();
        committed(admit(&mut mind, (1..=64).map(question_n).collect()));
        let mut keys: Vec<String> = (1..=64).map(fork).collect();
        keys.sort();
        let ranks = |mind: &Mind<MemoryStore>| -> Vec<(String, u64)> {
            let mut out: Vec<(String, u64)> = pages(mind, 50).iter().flat_map(|(_, page)| page.forks.iter().map(|e| (e.card.fork_id.clone(), e.card.order))).collect();
            out.sort();
            out
        };
        let before = ranks(&mind);
        assert_eq!(before.iter().map(|(_, order)| *order).collect::<std::collections::BTreeSet<_>>(), (0..64).collect(), "distinct, 0 to 63");
        for (rank, key) in keys.iter().enumerate() {
            assert_eq!(before.iter().find(|(id, _)| id == key).unwrap().1, rank as u64, "ties inside one commit break on the key");
        }
        committed(admit(&mut mind, vec![question_n(65)]));
        committed(admit(&mut mind, (66..=70).map(question_n).collect()));
        let after = ranks(&mind);
        for row in &before {
            assert!(after.contains(row), "{row:?} kept its rank");
        }
        assert_eq!(after.len(), 70);
        assert_eq!(after.iter().map(|(_, order)| *order).collect::<std::collections::BTreeSet<_>>(), (0..70).collect());
        // The page boundary at c1: the page ending at order 1 names c1, and the next page is order 0 alone.
        let (upper, _) = page(mind.desk_forks(&list(&LIST), None, ForkCursor::new(3), limit(2)));
        assert_eq!((orders(&upper), upper.next_cursor.map(ForkCursor::order)), (vec![2, 1], Some(1)));
        let (last, _) = page(mind.desk_forks(&list(&LIST), None, upper.next_cursor, limit(50)));
        assert_eq!((orders(&last), last.next_cursor), (vec![0], None));
    }

    /// No admission path reorders the ranks: a re-admitted question (replayed batch, the same document
    /// in a new batch, a rewrite, a resolution of it, an import of it or of a new one) keeps every
    /// existing rank, and a question that is new is admitted last whatever its key.
    #[test]
    fn no_admission_path_moves_an_existing_rank() {
        let mut mind = seeded();
        committed(admit(&mut mind, vec![question_n(2), question_n(3)]));
        committed(admit(&mut mind, vec![question_n(5)]));
        let ranks = |mind: &Mind<MemoryStore>| -> Vec<(String, u64)> {
            pages(mind, 50).iter().flat_map(|(_, page)| page.forks.iter().map(|e| (e.card.fork_id.clone(), e.card.order))).collect()
        };
        let base = ranks(&mind);
        assert_eq!(base.iter().map(|(_, order)| *order).collect::<Vec<_>>(), [2, 1, 0]);
        let held = |mind: &Mind<MemoryStore>| {
            let now = ranks(mind);
            assert!(base.iter().all(|row| now.contains(row)), "every existing rank held: {now:?}");
            now.len()
        };
        // The same document again in a new batch, and a rewrite of it.
        let again = admit(&mut mind, vec![question_n(2)]);
        assert!(!matches!(again, PipelineAdmissionOutcome::Committed { .. }), "{again:?}");
        let mut rewritten = question_n(3);
        let D::Question(body) = &mut rewritten else { unreachable!() };
        body.question = "Another reading entirely?".into();
        assert!(!matches!(admit(&mut mind, vec![rewritten]), PipelineAdmissionOutcome::Committed { .. }));
        assert_eq!(held(&mind), 3);
        // A batch replayed whole.
        let batch = PipelineAdmissionBatch { instance: mind.instance().clone(), provenance: provenance(Faculty::Hands), documents: vec![question_n(7)] };
        committed(mind.admit(batch.clone(), now()));
        let replay = mind.admit(batch, now());
        assert!(matches!(replay, PipelineAdmissionOutcome::AlreadyAdmitted { .. }), "{replay:?}");
        assert_eq!(held(&mind), 4);
        // A resolution of a question changes its status and not its rank.
        let subject = PipelineRef { kind: PipelineKind::Question, id: s(&fork(2)) };
        let withdrawn = crate::fixtures::resolution(subject, eureka_pipeline::ResolutionOutcome::Withdrawn { reason: "no".into() });
        committed(admit(&mut mind, vec![withdrawn]));
        assert_eq!(held(&mind), 4);
        // A new question whose key sorts before them all is the newest.
        committed(admit(&mut mind, vec![question_n(0)]));
        assert_eq!(held(&mind), 5);
        assert_eq!(ranks(&mind)[0], (fork(0), 4));
        // An import carries the source's ordinal and the Mind's own receipt orders it: appended, last.
        let source = crate::receipt::ImportSource {
            instance: slug(INSTANCE),
            receipt_id: s("mind-commit-source"),
            ordinal: 1,
            committed_at: s("2026-11-08T16:20:05Z"),
            faculty: Faculty::Imagination,
            agent: s("imagination"),
            session: s("source-session"),
            tool: s("huginn"),
            external: vec![],
        };
        let mut abroad = question_n(1);
        let D::Question(body) = &mut abroad else { unreachable!() };
        body.campaign = slug("other-campaign");
        let import = |documents: Vec<PipelineDocument>| PipelineAdmissionBatch {
            instance: INSTANCE.into(),
            provenance: crate::receipt::PipelineProvenance { faculty: Faculty::SelfFaculty, agent: s("self"), session: s("import-1"), tool: s("huginn"), imported_from: Some(source.clone()) },
            documents,
        };
        let outcome = mind.admit(import(vec![abroad.clone()]), now());
        let imported = matches!(outcome, PipelineAdmissionOutcome::Committed { .. });
        assert_eq!(held(&mind), 5 + usize::from(imported), "{outcome:?}");
        let replay = mind.admit(import(vec![abroad]), now());
        assert!(!matches!(replay, PipelineAdmissionOutcome::Committed { .. }), "{replay:?}");
        assert_eq!(held(&mind), 5 + usize::from(imported));
        if imported {
            assert_eq!(ranks(&mind)[0].1, 5, "the import is the newest, whatever ordinal its source gave it");
        }
    }

    /// A closed card names the in-force ruling that answers THIS fork, never another fork's.
    #[test]
    fn a_closed_card_names_the_ruling_that_answers_it() {
        let mut mind = desk(3);
        close(&mut mind, &fork(1), "R9");
        close(&mut mind, &fork(2), "R2");
        let named = |n: u32| entry(mind.desk_fork(&list(&LIST), &fork(n))).0.card.ruling;
        assert_eq!(named(1).as_deref(), Some("eureka-state:ruling:R9"));
        assert_eq!(named(2).as_deref(), Some("eureka-state:ruling:R2"));
        assert_eq!(named(3), None);
    }

    /// The response a page of the budget makes, in the worst envelope the door can echo (a 256-byte
    /// message id, a 64-byte slug in the runtime id), is within the bound; the margin is real.
    #[test]
    fn a_page_at_the_budget_in_the_worst_envelope_is_within_the_response_bound() {
        let mind = admitted_each(1..=50);
        let worst = format!("huginn-{}", "s".repeat(64));
        let all = pages(&mind, 50);
        assert!(all.len() > 1);
        for (response, _) in &all {
            let framed = 4 + encoded(&"m".repeat(256), &worst, response);
            assert!(framed <= DESK_MAX_RESPONSE_BYTES, "{framed} bytes framed");
            let overhead = encoded(&"m".repeat(256), &worst, response) - bare(response);
            assert!(overhead > 0 && overhead < DESK_ENVELOPE_MARGIN, "the envelope is {overhead} bytes against a margin of {DESK_ENVELOPE_MARGIN}");
        }
        // And the fullest page is within the margin's reach of the bound.
        let fullest = all.iter().map(|(response, _)| bare(response)).max().unwrap();
        assert!(fullest > DESK_PAGE_BUDGET - 3000, "the pages fill to the budget, not far short of it: {fullest}");
    }

    /// Libby's list scenarios (CultLib e57fbaee `listScenarios`) replayed through `desk_forks` over the
    /// data the Mind stores: five forks in five commits, the first and third closed by a ruling, the
    /// fifth answered, the third answered before and after it closed. What the Mind does not store the
    /// same way is not compared: the ruling's id (the vectors name a `resolution`, the Mind a `ruling`),
    /// `ruled_from` (always empty here), the answerers and their authority, timestamps and texts.
    #[test]
    fn libbys_list_scenarios_replay_through_desk_forks() {
        let file = transport();
        let mut replayed = 0;
        for scenario in file["listScenarios"].as_array().unwrap() {
            let mut mind = seeded();
            if !scenario["entries"].as_array().unwrap().is_empty() {
                for n in 1..=5 {
                    committed(admit(&mut mind, vec![question_n(n)]));
                }
                for n in [3, 5] {
                    let hash = hash_of(&mind, &fork(n));
                    answer(&mut mind, "x", &LIST, &fork(n), &hash, None);
                }
                close(&mut mind, &fork(1), "R1");
                close(&mut mind, &fork(3), "R3");
                let hash = hash_of(&mind, &fork(3));
                answer(&mut mind, "y", &LIST, &fork(3), &hash, None);
            }
            for step in scenario["steps"].as_array().unwrap() {
                let (request, want) = (&step["request"], &step["response"]);
                let status: Option<ListStatus> = serde_json::from_value(request["status"].clone()).unwrap();
                // Her orders count from 1 (the admission ordinal) and the Mind's from 0 (a rank): every order
                // and cursor in her scenarios is one more than the Mind's.
                let cursor: Option<ForkCursor> = serde_json::from_value::<Option<ForkCursor>>(request["cursor"].clone()).unwrap().and_then(|her| ForkCursor::new(her.order() - 1));
                let (got, _) = page(mind.desk_forks(&list(&LIST), status, cursor, limit(request["limit"].as_u64().unwrap() as u8)));
                let name = &scenario["name"];
                assert_eq!((got.open_count, got.recorded_count), (want["open_count"].as_u64().unwrap(), want["recorded_count"].as_u64().unwrap()), "{name}");
                assert_eq!(serde_json::to_value(got.next_cursor.and_then(|ours| ForkCursor::new(ours.order() + 1))).unwrap(), want["next_cursor"], "{name}");
                let wanted = want["forks"].as_array().unwrap();
                assert_eq!(got.forks.len(), wanted.len(), "{name}");
                for (have, want) in got.forks.iter().zip(wanted) {
                    assert_eq!(have.card.order + 1, want["card"]["order"].as_u64().unwrap(), "{name}");
                    assert_eq!(serde_json::to_value(have.card.status).unwrap(), want["card"]["status"], "{name}");
                    assert_eq!(have.record.counted_answer_id.as_deref(), want["record"]["counted_answer_id"].as_str(), "{name}");
                    let kept: Vec<_> = have.record.answers.iter().map(|answer| (answer.answer_id.as_str(), answer.outcome.as_str())).collect();
                    let wanted: Vec<_> = want["record"]["answers"].as_array().unwrap().iter().map(|a| (a["answer_id"].as_str().unwrap(), a["outcome"].as_str().unwrap())).collect();
                    assert_eq!(kept, wanted, "{name}");
                }
                replayed += 1;
            }
        }
        assert_eq!(replayed, 7);
    }
}
