//! The desk feed's golden fixtures: the bytes Bifrost writes to a Mind's desk socket for a list or
//! a read, and the bytes Huginn writes back, built here with Huginn's own encoders so the files
//! are what the door speaks. `docs/desk-feed-door.md` is the contract they pin; the README in the
//! fixtures directory lists them.
//!
//! The entry-bearing fixtures carry the reader-count shapes (`raised_at`, `decider_count`,
//! `answerer_count`, a nullable `answerer`), a decider's view and a non-decider's. CultLib's own bytes
//! for those shapes are vendored at one commit in `fixtures/desk-feed/cultlib-d3025d3c/`; Huginn's
//! real types decode and re-encode every one of them byte for byte (checked below).
//!
//! `cargo test -p huginn-mind --test desk_feed_fixtures` compares every file with what the
//! encoders produce now. `HUGINN_WRITE_DESK_FIXTURES=1` rewrites them instead.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use cultnet_rs::{CultNetMessage, CultNetWireContract, encode_cultnet_message_to_vec, encode_frame};
use huginn_mind::MindRefusal;
use huginn_mind::desk_wire::{
    fork_too_large_read, AnswerSchema, CardSchema, CardStatus, DeskRequest, DeskResponse, EntrySchema, ForkCard, ForkCursor, ForkDeskAnswer, ForkEntry, ForkLimit,
    ForkList, ForkOption, ForkRecord, ForkRefusal, ForkRefusalCode, ForkRef, ListSchema, ListStatus, RecordSchema,
};
use huginn_mind::envelope::{OperationFailure, encode_desk_failure, encode_desk_request, encode_desk_response};
use huginn_mind::eureka_pipeline::{DeskAuthority, Short, Slug};

const RUNTIME: &str = "huginn-eureka";
/// Account ids as Bifrost sends them: `UserAccount.Id` rendered lowercase D-format
/// (`DeskEndpoints.cs` `accountId.Value.ToString("D")`; `StartupConfigurationValidator` refuses any
/// other decider). Synthetic values.
const READER: &str = "5f0c2b1e-8a4d-4c3b-9e7f-1a2b3c4d5e6f";
const OTHER_DECIDER: &str = "0b9d7e3a-6c21-4f58-a0d4-9e8f7a6b5c4d";
/// A member who answered but is not a decider: their answer is stored `Refused(not_a_decider)`.
const OUTSIDER: &str = "c3a1f2e4-7b6d-4e8f-9a0b-1c2d3e4f5a6b";
/// A member the call's deciders do not name, who reads as a non-decider.
const NON_DECIDER: &str = "9e8d7c6b-5a49-4382-b1a0-f9e8d7c6b5a4";
/// A question Eureka's mind holds (asked 2026-11-11). The answers, notes and hash in `record()` are
/// synthetic: the hash is CultLib's sample digest, not this question's.
const FORK: &str = "eureka-substrate:question:desk-listing-authority";

fn s(text: &str) -> Short {
    Short(text.into())
}

fn forks(status: Option<ListStatus>, cursor: Option<u64>, limit: u8) -> DeskRequest {
    DeskRequest::Forks {
        mind: Slug("eureka".into()),
        subject: s(READER),
        deciders: vec![s(READER), s(OTHER_DECIDER)],
        status,
        cursor: cursor.and_then(ForkCursor::new),
        limit: ForkLimit::new(limit).unwrap(),
    }
}

fn read() -> DeskRequest {
    DeskRequest::Fork { mind: Slug("eureka".into()), subject: s(READER), deciders: vec![s(READER), s(OTHER_DECIDER)], fork_id: FORK.into() }
}

/// The same reads from a subject the call's deciders do not name: a non-decider.
fn forks_as_non_decider() -> DeskRequest {
    DeskRequest::Forks {
        mind: Slug("eureka".into()),
        subject: s(NON_DECIDER),
        deciders: vec![s(READER), s(OTHER_DECIDER)],
        status: Some(ListStatus::Open),
        cursor: None,
        limit: ForkLimit::new(50).unwrap(),
    }
}

fn read_as_non_decider() -> DeskRequest {
    DeskRequest::Fork { mind: Slug("eureka".into()), subject: s(NON_DECIDER), deciders: vec![s(READER), s(OTHER_DECIDER)], fork_id: FORK.into() }
}

fn list(open_count: u64, recorded_count: u64) -> DeskResponse {
    DeskResponse::Forks {
        list: ForkList {
            schema_version: ListSchema::V0,
            mind: "eureka".into(),
            stamp: "s10190".into(),
            open_count,
            recorded_count,
            forks: Vec::new(),
            next_cursor: None,
        },
        as_of: 10190,
    }
}

fn refusal(code: ForkRefusalCode, error: &str) -> DeskResponse {
    DeskResponse::NotStored(ForkRefusal { schema_version: huginn_mind::desk_wire::RefusalSchema::V0, code, error: error.into() })
}

fn answer(id: &str, outcome: &str, of: Option<&str>, answerer: Option<&str>, note: &str, received_at: &str) -> ForkDeskAnswer {
    ForkDeskAnswer {
        schema_version: AnswerSchema::V0,
        answer_id: id.into(),
        fork_id: FORK.into(),
        outcome: outcome.into(),
        of: of.map(str::to_string),
        retracts: None,
        code: None,
        options_hash: "7d95fc87d532420611fe415c1def27a3447617bb68235f46e3af130aea361c48".into(),
        choice: Some("asking-mind".into()),
        note: Some(note.into()),
        answered_at: received_at.into(),
        received_at: received_at.into(),
        claim_in_future: false,
        answerer: answerer.map(str::to_string),
        authority: DeskAuthority::Member,
        request_id: format!("00000000-0000-4000-8000-00000000000{}", &id[1..]),
    }
}

/// A fork's record with a counted answer, a decider's revision, a refusal and a retraction: four
/// answers by three accounts. A decider's view names every answerer; a non-decider's names none and
/// keeps the count.
fn record(decider: bool) -> ForkRecord {
    let who = |id: &'static str| decider.then_some(id);
    ForkRecord {
        schema_version: RecordSchema::V0,
        fork_id: FORK.into(),
        options_hash: HASH.into(),
        counted_answer_id: Some("n1".into()),
        answerer_count: 3,
        answers: vec![
            answer("n1", "Counted", None, who(READER), "The asker lists her own forks.", "2026-11-11T17:00:00Z"),
            answer("n2", "Revision", Some("n1"), who(OTHER_DECIDER), "Agreed.", "2026-11-11T17:05:00Z"),
            ForkDeskAnswer { code: Some(huginn_mind::desk_wire::ForkAnswerCode::NotADecider), ..answer("n3", "Refused", None, who(OUTSIDER), "Me too.", "2026-11-11T17:06:00Z") },
            // A second answer by an account already counted: four answers, three answerers.
            ForkDeskAnswer {
                retracts: Some("n2".into()),
                choice: None,
                note: None,
                ..answer("n4", "Retraction", None, who(OTHER_DECIDER), "", "2026-11-11T17:07:00Z")
            },
        ],
    }
}

const HASH: &str = "7d95fc87d532420611fe415c1def27a3447617bb68235f46e3af130aea361c48";

/// The fork's card. A decider's view lists the call's ids; a non-decider's lists none and keeps the count.
fn card(decider: bool, raised_at: Option<&str>) -> ForkCard {
    ForkCard {
        schema_version: CardSchema::V0,
        fork_id: FORK.into(),
        title: "Who may list a fork?".into(),
        context: "A fork is read by members only once it is listed.".into(),
        options: vec![ForkOption { id: "asking-mind".into(), text: "The asking Mind lists her own forks.".into() }, ForkOption { id: "operator".into(), text: "An operator lists them.".into() }],
        recommendation: "asking-mind".into(),
        depends: Vec::new(),
        leans: None,
        flag: None,
        raised_in: Some(ForkRef { kind: "Idea".into(), id: "eureka-substrate:idea:desk-door-map".into() }),
        raised_at: raised_at.map(str::to_string),
        deciders: if decider { vec![READER.into(), OTHER_DECIDER.into()] } else { Vec::new() },
        decider_count: 2,
        status: CardStatus::Open,
        ruling: None,
        ruled_from: Vec::new(),
        order: 41,
        options_hash: HASH.into(),
    }
}

fn entry(decider: bool, raised_at: Option<&str>) -> ForkEntry {
    ForkEntry { schema_version: EntrySchema::V0, card: card(decider, raised_at), record: record(decider) }
}

/// One open fork on the page; the counts are the page's visible set.
fn list_of(entry: ForkEntry) -> DeskResponse {
    DeskResponse::Forks {
        list: ForkList { schema_version: ListSchema::V0, mind: "eureka".into(), stamp: "s10190".into(), open_count: 1, recorded_count: 0, forks: vec![entry], next_cursor: None },
        as_of: 10190,
    }
}

/// The named MessagePack payload a CultNet operation message carries (base64 in the envelope).
fn payload(message: &CultNetMessage) -> Vec<u8> {
    use base64::Engine;
    let (CultNetMessage::OperationRequest { payload, .. } | CultNetMessage::OperationResponse { payload, .. }) = message else {
        panic!("an operation message")
    };
    base64::engine::general_purpose::STANDARD.decode(payload).unwrap()
}

fn frame(message: &CultNetMessage) -> Vec<u8> {
    encode_frame(&encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap()).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    let mut text: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    text.push('\n');
    text
}

/// Every fixture: its file stem and its bytes. A message gives `.frame.bin`, `.frame.hex` and,
/// for an operation message, `.payload.msgpack` and `.payload.hex`; a bare payload gives the last two.
fn fixtures() -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut message = |name: &str, message: CultNetMessage| {
        let framed = frame(&message);
        files.push((format!("{name}.frame.bin"), framed.clone()));
        files.push((format!("{name}.frame.hex"), hex(&framed).into_bytes()));
        if matches!(message, CultNetMessage::OperationRequest { .. } | CultNetMessage::OperationResponse { .. }) {
            let inner = payload(&message);
            files.push((format!("{name}.payload.msgpack"), inner.clone()));
            files.push((format!("{name}.payload.hex"), hex(&inner).into_bytes()));
        }
    };
    message("list-open.request", encode_desk_request("m-1", &forks(Some(ListStatus::Open), None, 50), None).unwrap());
    message("list-open-cursor.request", encode_desk_request("m-2", &forks(Some(ListStatus::Open), Some(42), 50), None).unwrap());
    message("list-counts.request", encode_desk_request("m-3", &forks(Some(ListStatus::Open), None, 0), None).unwrap());
    message("read.request", encode_desk_request("m-4", &read(), None).unwrap());
    message("list-non-decider.request", encode_desk_request("m-5", &forks_as_non_decider(), None).unwrap());
    message("read-non-decider.request", encode_desk_request("m-6", &read_as_non_decider(), None).unwrap());
    message("list-open.response", encode_desk_response("m-1", "forks", &list_of(entry(true, Some("2026-11-11T16:40:12Z"))), RUNTIME).unwrap());
    message("list-open-non-decider.response", encode_desk_response("m-5", "forks", &list_of(entry(false, Some("2026-11-11T16:40:12Z"))), RUNTIME).unwrap());
    // raised_at nil: the question's first admission is unknown.
    message("read.response", encode_desk_response("m-4", "fork", &DeskResponse::Fork { entry: entry(true, None), as_of: 10190 }, RUNTIME).unwrap());
    message("read-non-decider.response", encode_desk_response("m-6", "fork", &DeskResponse::Fork { entry: entry(false, None), as_of: 10190 }, RUNTIME).unwrap());
    message("list-empty.response", encode_desk_response("m-1", "forks", &list(0, 0), RUNTIME).unwrap());
    message("list-counts.response", encode_desk_response("m-3", "forks", &list(3, 5), RUNTIME).unwrap());
    message(
        "read-unknown.response",
        encode_desk_response("m-4", "fork", &refusal(ForkRefusalCode::ForkUnknown, "fork_unknown: no such fork"), RUNTIME).unwrap(),
    );
    message(
        "list-too-large.response",
        encode_desk_response(
            "m-1",
            "forks",
            &refusal(ForkRefusalCode::ForkTooLarge, &format!("fork_too_large: fork {FORK} at order 41 is past the page budget by itself; continue with cursor c41")),
            RUNTIME,
        )
        .unwrap(),
    );
    message("read-too-large.response", encode_desk_response("m-4", "fork", &DeskResponse::NotStored(fork_too_large_read(FORK)), RUNTIME).unwrap());
    message(
        "list-response-too-large.response",
        encode_desk_response("m-1", "forks", &DeskResponse::Refused(MindRefusal::ResponseTooLarge { bytes: 70000, limit: 65536 }), RUNTIME).unwrap(),
    );
    message("refused-unit.response", encode_desk_response("m-1", "forks", &DeskResponse::Refused(MindRefusal::MissingIdentity), RUNTIME).unwrap());
    message(
        "unavailable.response",
        encode_desk_response("m-1", "forks", &DeskResponse::Refused(MindRefusal::Unavailable { detail: "the mind store is not readable".into() }), RUNTIME).unwrap(),
    );
    message(
        "failure.response",
        encode_desk_failure("m-1", "forks", &OperationFailure { code: "payload-not-a-request".into(), message: "field Forks.limit: out-of-range".into() }, RUNTIME),
    );
    message("error.response", CultNetMessage::Error { error: "huginn.desk refuses a message_id over 256 bytes".into(), code: None, details: None });
    for (name, decider) in [("record-whole", true), ("record-withheld", false)] {
        let bytes = rmp_serde::to_vec_named(&record(decider)).unwrap();
        files.push((format!("{name}.payload.msgpack"), bytes.clone()));
        files.push((format!("{name}.payload.hex"), hex(&bytes).into_bytes()));
    }
    files
}

#[test]
fn desk_feed_fixtures_are_what_the_encoders_write() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desk-feed");
    let write = std::env::var_os("HUGINN_WRITE_DESK_FIXTURES").is_some();
    if write {
        std::fs::create_dir_all(&dir).unwrap();
    }
    for (name, bytes) in fixtures() {
        let path = dir.join(&name);
        if write {
            std::fs::write(&path, &bytes).unwrap();
        } else {
            let held = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(held == bytes, "{name} differs from what the encoders write now");
        }
    }
}

/// The one grammar for `subject`, every `deciders` entry and every `answerer`: a lowercase
/// D-format UUID (8-4-4-4-12 lowercase hex), the grammar Huginn already names for `RequestId`
/// (`eureka-pipeline` `uuid_text`). A Matrix id, an email or a handle never fits it.
fn account_id(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(at, byte)| match at {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// Every account id the fixtures carry is a Bifrost account id, never a Matrix id, email or
/// handle. The door does not refuse other shapes yet (`Short`, any 200 bytes); that refusal is owed
/// by `cut-desk-feed-listing.r6`, and this pins the grammar it must enforce.
#[test]
fn every_account_id_in_the_fixtures_is_a_lowercase_d_format_uuid() {
    for request in [forks(Some(ListStatus::Open), None, 50), read()] {
        let subject = match &request {
            DeskRequest::Forks { subject, .. } | DeskRequest::Fork { subject, .. } | DeskRequest::Answer { subject, .. } => subject,
        };
        assert!(account_id(&subject.0), "subject");
        assert!(request.deciders().iter().all(|decider| account_id(&decider.0)), "deciders");
    }
    assert!(record(true).answers.iter().all(|answer| account_id(answer.answerer.as_deref().unwrap())), "answerer");
    for shape in [
        "@heimdall:gamecult.org",
        "meta@gamecult.org",
        "heimdall",
        "7",
        "5F0C2B1E-8A4D-4C3B-9E7F-1A2B3C4D5E6F",
        "{5f0c2b1e-8a4d-4c3b-9e7f-1a2b3c4d5e6f}",
        "",
        // 36 hex digits and no dash: the length alone does not make an id.
        "5f0c2b1e08a4d04c3b09e7f01a2b3c4d5e6f",
        // 37 characters: a valid id with one more digit.
        "5f0c2b1e-8a4d-4c3b-9e7f-1a2b3c4d5e6f0",
        // A dash moved off its position.
        "5f0c2b1e8-a4d-4c3b-9e7f-1a2b3c4d5e6f",
    ] {
        assert!(!account_id(shape), "{shape} is not an account id");
    }
}


// ---- CultLib's bytes: the reader-count shapes, pinned at one commit. ----

/// The CultLib commit the vendored vectors come from (#118 on #117 on #115). Re-pin: see the README beside them.
const CULTLIB_SHA: &str = "d3025d3c19674803ad32eb2335461b8cbc22bd28";

/// Each vendored vector and the SHA-256 of its bytes.
const VENDORED: &[(&str, &str)] = &[
    ("card-operator-fork", "56bfdb65ded60b562001d09d0b8da6e092900f404bf5f68f7084dff29bcc34f5"),
    ("card-raised-at-nil", "b81bb825790ffe03fabe63c3f8a61c36bd50ca268aa6dff67b5bcfad3957e58c"),
    ("card-withheld-deciders", "d13b9b77461cfdf78f943ef56c6aeab852ab6b6f0b355e26c5b361f99c79c7f7"),
    ("answer-counted", "55d69ae3cc7ebbeeae7d501e8641640e115c3fb5aac399ea760588bca343b9e6"),
    ("answer-revision", "19d7289395ae68cbaf6de1c0a1ef6c843211b2caf9462e1d2268cd0883386315"),
    ("answer-retraction", "89156f41ec3ea54f5d47bd3318913ff013785f4727916875be97785935cb7735"),
    ("answer-withheld-answerer", "b66b4f921cdd374be04af3de34c59e254a5e542027ae8c57f196a3ea3406e427"),
    ("answer-withheld-answerer-revision", "93f7c875f963a7cfeab2a5980b6d5ce81e6753fc9f3f8bf0d7353f3f388144b7"),
    ("answer-withheld-answerer-retraction", "7d0cc555bf9b6f7c345b0da7c504bcd1ed7b46040ff51d1e57c84b2f56655213"),
    ("answer-refused-stale", "3183bc9c483f4a625332cda11ef22a9c8560868a7728b77daa63a1960a3cd3d6"),
    ("entry-open-counted", "b51f499e8dd0465cf5df9d68d8fccbe94dd1339fd3157969693dd0d4ecb2549b"),
    ("entry-withheld-open-counted", "62ccad7a9f41262d6e35d0247315d0bb75d1313b3c9dabe8a99d1aa1b844607f"),
    ("entry-closed-with-ruling-and-a-refusal", "f7859c7d04d338d6b1c634308b08ae58ae77b0ff459767859a5e40f0af2f16e5"),
    ("entry-withheld-closed-with-ruling-and-a-refusal", "daaa819bfa2612616826ef3217dfcf8d8d8cb71d86547d7106426c779134f827"),
];

fn vendored(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desk-feed/cultlib-d3025d3c").join(format!("{name}.msgpack"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A decoded MessagePack value that keeps map order. A small decoder of its own, so the key counts,
/// headers and positions below are read off the bytes and not off `rmp_serde`'s idea of the types.
#[derive(Debug, PartialEq)]
enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    fn get(&self, key: &str) -> &Value {
        let Value::Map(pairs) = self else { panic!("not a map") };
        &pairs.iter().find(|(name, _)| name == key).unwrap_or_else(|| panic!("no key {key}")).1
    }
    fn keys(&self) -> Vec<&str> {
        let Value::Map(pairs) = self else { panic!("not a map") };
        pairs.iter().map(|(name, _)| name.as_str()).collect()
    }
    fn list(&self) -> &[Value] {
        let Value::List(items) = self else { panic!("not a list") };
        items
    }
    fn str(&self) -> &str {
        let Value::Str(text) = self else { panic!("not a string: {self:?}") };
        text
    }
    fn int(&self) -> i64 {
        let Value::Int(number) = self else { panic!("not an int: {self:?}") };
        *number
    }
}

fn decode(bytes: &[u8]) -> Value {
    fn take<'a>(bytes: &'a [u8], at: &mut usize, n: usize) -> &'a [u8] {
        let slice = &bytes[*at..*at + n];
        *at += n;
        slice
    }
    fn be(slice: &[u8]) -> u64 {
        slice.iter().fold(0, |acc, byte| (acc << 8) | u64::from(*byte))
    }
    fn text(bytes: &[u8], at: &mut usize, n: usize) -> Value {
        Value::Str(String::from_utf8(take(bytes, at, n).to_vec()).unwrap())
    }
    fn items(bytes: &[u8], at: &mut usize, n: usize) -> Vec<Value> {
        (0..n).map(|_| value(bytes, at)).collect()
    }
    fn pairs(bytes: &[u8], at: &mut usize, n: usize) -> Value {
        Value::Map((0..n).map(|_| (value(bytes, at).str().to_string(), value(bytes, at))).collect())
    }
    fn value(bytes: &[u8], at: &mut usize) -> Value {
        let tag = take(bytes, at, 1)[0];
        match tag {
            0x00..=0x7f => Value::Int(i64::from(tag)),
            0x80..=0x8f => pairs(bytes, at, usize::from(tag & 0x0f)),
            0x90..=0x9f => Value::List(items(bytes, at, usize::from(tag & 0x0f))),
            0xa0..=0xbf => text(bytes, at, usize::from(tag & 0x1f)),
            0xc0 => Value::Nil,
            0xc2 => Value::Bool(false),
            0xc3 => Value::Bool(true),
            0xcc => Value::Int(be(take(bytes, at, 1)) as i64),
            0xcd => Value::Int(be(take(bytes, at, 2)) as i64),
            0xce => Value::Int(be(take(bytes, at, 4)) as i64),
            0xd9 => {
                let n = be(take(bytes, at, 1)) as usize;
                text(bytes, at, n)
            }
            0xda => {
                let n = be(take(bytes, at, 2)) as usize;
                text(bytes, at, n)
            }
            0xdc => {
                let n = be(take(bytes, at, 2)) as usize;
                Value::List(items(bytes, at, n))
            }
            0xde => {
                let n = be(take(bytes, at, 2)) as usize;
                pairs(bytes, at, n)
            }
            other => panic!("MessagePack tag {other:#04x} at {} is not in this decoder", *at - 1),
        }
    }
    let mut at = 0;
    let decoded = value(bytes, &mut at);
    assert_eq!(at, bytes.len(), "trailing bytes");
    decoded
}

const CARD_KEYS: [&str; 18] = [
    "schemaVersion", "fork_id", "title", "context", "options", "recommendation", "depends", "leans", "flag", "raised_in", "raised_at", "deciders",
    "decider_count", "status", "ruling", "ruled_from", "order", "options_hash",
];
const RECORD_KEYS: [&str; 6] = ["schemaVersion", "fork_id", "options_hash", "counted_answer_id", "answerer_count", "answers"];

#[test]
fn the_vendored_cultlib_vectors_are_the_pinned_bytes() {
    assert_eq!(CULTLIB_SHA.len(), 40);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desk-feed/cultlib-d3025d3c");
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/desk-feed-door.md");
    for path in [dir.join("README.md"), docs] {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(text.contains(CULTLIB_SHA), "{} does not name the pinned sha {CULTLIB_SHA}", path.display());
    }
    assert!(dir.to_str().unwrap().ends_with(&format!("cultlib-{}", &CULTLIB_SHA[..8])), "the directory is named for the pin");
    for (name, sha) in VENDORED {
        let digest: String = Sha256::digest(vendored(name)).iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(&digest, sha, "{name} is not CultLib {CULTLIB_SHA}'s bytes");
    }
}

/// The card: header `DE 00 12` (map16, 18 entries: `schemaVersion` and 17 fields), the keys in this
/// order, `raised_at` a string or nil, `decider_count` 1..64 right after `deciders`, and `deciders` empty
/// (withheld) or exactly `decider_count` account ids.
fn check_card(card: &Value, withheld: bool) {
    assert_eq!(card.keys(), CARD_KEYS);
    assert!(matches!(card.get("raised_at"), Value::Nil | Value::Str(_)));
    let count = card.get("decider_count").int();
    assert!((1..=64).contains(&count));
    let ids = card.get("deciders").list();
    assert_eq!(ids.is_empty(), withheld);
    assert!(ids.is_empty() || ids.len() as i64 == count);
    assert!(ids.iter().all(|id| account_id(id.str())), "deciders are lowercase D-format UUIDs");
    assert_eq!(card.get("options_hash").str().len(), 64);
}

#[test]
fn the_cards_are_eighteen_keys_with_raised_at_and_decider_count() {
    for name in ["card-operator-fork", "card-raised-at-nil", "card-withheld-deciders"] {
        let bytes = vendored(name);
        assert_eq!(bytes[..3], [0xde, 0x00, 0x12], "{name}: header");
        check_card(&decode(&bytes), name == "card-withheld-deciders");
    }
    assert!(matches!(decode(&vendored("card-operator-fork")).get("raised_at"), Value::Str(_)));
    assert_eq!(decode(&vendored("card-raised-at-nil")).get("raised_at"), &Value::Nil);
    assert_eq!(decode(&vendored("card-withheld-deciders")).get("decider_count").int(), 2);
}

/// `answerer` is a string or nil on every answer; `Value::Nil` only when `named` is false. The bytes
/// null it on all four variants (`Counted`, `Revision`, `Retraction`, `Refused`), by ruling
/// `desk-reader-counts-shape` and now by vector.
fn check_answer(answer: &Value, named: bool) {
    assert_eq!(answer.keys().len(), 16);
    assert_eq!(answer.keys()[13], "answerer");
    match answer.get("answerer") {
        Value::Str(id) => assert!(named && account_id(id), "a named answerer is a lowercase D-format UUID"),
        Value::Nil => assert!(!named),
        other => panic!("answerer is {other:?}"),
    }
}

#[test]
fn the_answers_keep_sixteen_keys_and_answerer_is_a_string_or_nil() {
    for (name, named) in [("answer-counted", true), ("answer-revision", true), ("answer-retraction", true), ("answer-refused-stale", true), ("answer-withheld-answerer", false), ("answer-withheld-answerer-revision", false), ("answer-withheld-answerer-retraction", false)] {
        let bytes = vendored(name);
        assert_eq!(bytes[..3], [0xde, 0x00, 0x10], "{name}: header");
        check_answer(&decode(&bytes), named);
    }
    for (name, outcome) in [("answer-counted", "Counted"), ("answer-revision", "Revision"), ("answer-retraction", "Retraction"), ("answer-refused-stale", "Refused"), ("answer-withheld-answerer-revision", "Revision"), ("answer-withheld-answerer-retraction", "Retraction")] {
        assert_eq!(decode(&vendored(name)).get("outcome").str(), outcome);
    }
}

/// The entry is `83` and its record `86`, `answerer_count` the fifth key. In a decider's view the count
/// is the number of distinct answerer ids over every stored answer, refusals included; in a
/// non-decider's view no answerer is named and the count is between 1 and the number of answers (0 for
/// none). A non-decider entry holds none of its twin's account ids anywhere in its bytes.
#[test]
fn the_entries_are_83_over_a_record_of_86_with_answerer_count_before_answers() {
    for (decider, non_decider) in [("entry-open-counted", "entry-withheld-open-counted"), ("entry-closed-with-ruling-and-a-refusal", "entry-withheld-closed-with-ruling-and-a-refusal")] {
        let (decider_bytes, non_decider_bytes) = (vendored(decider), vendored(non_decider));
        let mut ids = Vec::new();
        for (bytes, named) in [(&decider_bytes, true), (&non_decider_bytes, false)] {
            assert_eq!(bytes[0], 0x83, "entry header");
            let entry = decode(bytes);
            assert_eq!(entry.keys(), ["schemaVersion", "card", "record"]);
            check_card(entry.get("card"), !named);
            let record = entry.get("record");
            assert_eq!(record.keys(), RECORD_KEYS);
            let answers = record.list_of_answers();
            let count = record.get("answerer_count").int();
            answers.iter().for_each(|answer| check_answer(answer, named));
            if named {
                let mut distinct: Vec<&str> = answers.iter().map(|answer| answer.get("answerer").str()).collect();
                distinct.sort_unstable();
                distinct.dedup();
                assert_eq!(count, distinct.len() as i64, "{decider}: distinct answerers");
                ids.extend(distinct.iter().map(|id| id.to_string()));
                ids.extend(entry.get("card").get("deciders").list().iter().map(|id| id.str().to_string()));
            } else if answers.is_empty() {
                assert_eq!(count, 0);
            } else {
                assert!((1..=answers.len() as i64).contains(&count));
            }
        }
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| !holds(&non_decider_bytes, id)), "{non_decider} carries an account id");
        assert!(ids.iter().all(|id| holds(&decider_bytes, id)), "{decider} names every id");
        // The twins agree on everything the view does not move.
        let (a, b) = (decode(&decider_bytes), decode(&non_decider_bytes));
        assert_eq!(a.get("card").get("options_hash"), b.get("card").get("options_hash"));
        assert_eq!(a.get("card").get("decider_count"), b.get("card").get("decider_count"));
        assert_eq!(a.get("record").get("answerer_count"), b.get("record").get("answerer_count"));
    }
    // A refusal is an answer and its answerer counts: the closed fork's two answers are two accounts.
    let closed = decode(&vendored("entry-closed-with-ruling-and-a-refusal"));
    assert!(closed.get("record").list_of_answers().iter().any(|answer| answer.get("outcome").str() == "Refused"));
    assert_eq!(closed.get("record").get("answerer_count").int(), 2);
}

fn holds(bytes: &[u8], text: &str) -> bool {
    bytes.windows(text.len()).any(|window| window == text.as_bytes())
}

impl Value {
    fn list_of_answers(&self) -> &[Value] {
        self.get("answers").list()
    }
}

/// Huginn's real types against CultLib's bytes: each of the 14 vendored vectors decodes into the type
/// that names it and re-encodes to the same bytes.
#[test]
fn huginn_types_decode_and_re_encode_every_vendored_vector_byte_for_byte() {
    fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(name: &str) {
        let bytes = vendored(name);
        let value = rmp_serde::from_slice::<T>(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(rmp_serde::to_vec_named(&value).unwrap() == bytes, "{name} does not re-encode byte for byte");
    }
    assert_eq!(VENDORED.len(), 14);
    for (name, _) in VENDORED {
        match name.split('-').next().unwrap() {
            "card" => round_trip::<ForkCard>(name),
            "answer" => round_trip::<ForkDeskAnswer>(name),
            "entry" => round_trip::<ForkEntry>(name),
            other => panic!("{name}: no type for {other}"),
        }
    }
}

/// Negative first: a non-decider's bytes hold no account id, in any fixture Huginn writes for that view.
#[test]
fn a_non_decider_fixture_holds_no_account_id() {
    let ids = [READER, OTHER_DECIDER, OUTSIDER, NON_DECIDER];
    let withheld = |bytes: &[u8]| ids.iter().filter(|id| holds(bytes, id)).count();
    let files = fixtures();
    let bytes_of = |name: &str| files.iter().find(|(file, _)| file == name).unwrap_or_else(|| panic!("no fixture {name}")).1.clone();
    for name in ["list-open-non-decider.response.payload.msgpack", "read-non-decider.response.payload.msgpack", "record-withheld.payload.msgpack"] {
        let bytes = bytes_of(name);
        assert_eq!(withheld(&bytes), 0, "{name} carries an account id");
    }
    // The decider's twins do name them, so the search above can fail.
    for name in ["list-open.response.payload.msgpack", "record-whole.payload.msgpack"] {
        assert!(withheld(&bytes_of(name)) >= 3, "{name}");
    }
}

/// The record header is `86` with `answerer_count` fifth, the card `DE 00 12` with `raised_at` and
/// `decider_count` where CultLib puts them, in Huginn's own bytes.
#[test]
fn huginns_own_card_and_record_bytes_have_the_pinned_headers_and_keys() {
    let card_bytes = rmp_serde::to_vec_named(&card(true, Some("2026-11-11T16:40:12Z"))).unwrap();
    assert_eq!(card_bytes[..3], [0xde, 0x00, 0x12]);
    assert_eq!(decode(&card_bytes).keys(), CARD_KEYS);
    check_card(&decode(&card_bytes), false);
    let withheld_bytes = rmp_serde::to_vec_named(&card(false, None)).unwrap();
    check_card(&decode(&withheld_bytes), true);
    assert_eq!(decode(&withheld_bytes).get("raised_at"), &Value::Nil);
    for decider in [true, false] {
        let record_bytes = rmp_serde::to_vec_named(&record(decider)).unwrap();
        assert_eq!(record_bytes[0], 0x86);
        let decoded = decode(&record_bytes);
        assert_eq!(decoded.keys(), RECORD_KEYS);
        assert_eq!(decoded.get("answerer_count").int(), 3);
        decoded.list_of_answers().iter().for_each(|answer| check_answer(answer, decider));
    }
}
