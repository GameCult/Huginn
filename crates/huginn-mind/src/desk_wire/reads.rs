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

impl<S: MindStore> Mind<S> {
    /// STUB
    pub fn desk_forks(&self, _deciders: &[Short], _status: Option<ListStatus>, _cursor: Option<ForkCursor>, _limit: ForkLimit) -> DeskResponse {
        DeskResponse::NotStored(fork_unknown())
    }

    /// STUB
    pub fn desk_fork(&self, _deciders: &[Short], _fork_id: &str) -> DeskResponse {
        DeskResponse::NotStored(fork_unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::options_hash;
    use crate::envelope::{decode_desk_request, encode_desk_request, encode_desk_response};
    use crate::fixtures::*;
    use crate::store::test_stores::MemoryStore;
    use crate::PipelineAdmissionOutcome;
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
        mind.desk_answer(&list(deciders), s(who), DeskAuthority::Member, request, now()).0
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
        for (at, expected) in [(0, 3), (1, 2), (2, 1), (3, 0)] {
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
        assert_eq!(card.order, mind.image().unwrap().row(mind.image().unwrap().id_of(PipelineKind::Question, &fork(1)).unwrap()).ordinal);
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
        let stored = mind.desk_answer(&list(&LIST), s("x"), DeskAuthority::Member, retract, now()).0;
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

    #[test]
    fn a_page_is_measured_against_the_response_bound_and_the_bound_is_the_door_s() {
        let mut mind = seeded();
        for n in 1..=50 {
            committed(admit(&mut mind, vec![wordy(n)]));
        }
        let encoded = |size: u8| {
            let response = mind.desk_forks(&list(&LIST), None, None, limit(size));
            let message = encode_desk_response("m-1", "forks", &response, "huginn-test").unwrap();
            cultnet_rs::encode_cultnet_message_to_vec(&message, cultnet_rs::CultNetWireContract::CultNetSchemaV0).unwrap().len()
        };
        let (zero, ten, fifty) = (encoded(0), encoded(10), encoded(50));
        // The counts always fit; a page fits while its cards do. Fifty cards with a 800-byte context each
        // and no answers at all do not: the door refuses that page by name, and the page size is the client's to lower.
        assert!(zero < 1000, "{zero}");
        assert!(ten < DESK_MAX_RESPONSE_BYTES, "{ten}");
        assert!(fifty > DESK_MAX_RESPONSE_BYTES, "{fifty} (a limit-50 page of realistic cards is over the bound)");
    }
}
