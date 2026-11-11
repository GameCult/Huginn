//! The desk feed's golden fixtures: the bytes Bifrost writes to a Mind's desk socket for a list or
//! a read, and the bytes Huginn writes back, built here with Huginn's own encoders so the files
//! are what the door speaks. `docs/desk-feed-door.md` is the contract they pin; the README in the
//! fixtures directory lists them.
//!
//! No fixture written here carries a card: Huginn's types do not carry `raised_at`, `decider_count`,
//! `answerer_count` or a nullable `answerer` yet. Those shapes are CultLib's own bytes, vendored at
//! one commit in `fixtures/desk-feed/cultlib-3a51bbab/` and checked below.
//!
//! `cargo test -p huginn-mind --test desk_feed_fixtures` compares every file with what the
//! encoders produce now. `HUGINN_WRITE_DESK_FIXTURES=1` rewrites them instead.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use cultnet_rs::{CultNetMessage, CultNetWireContract, encode_cultnet_message_to_vec, encode_frame};
use huginn_mind::MindRefusal;
use huginn_mind::desk_wire::{
    fork_too_large_read, AnswerSchema, DeskRequest, DeskResponse, ForkCursor, ForkDeskAnswer, ForkLimit, ForkList, ForkRecord, ForkRefusal, ForkRefusalCode,
    ListSchema, ListStatus, RecordSchema,
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

fn answer(id: &str, outcome: &str, of: Option<&str>, answerer: &str, note: &str, received_at: &str) -> ForkDeskAnswer {
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
        answerer: answerer.into(),
        authority: DeskAuthority::Member,
        request_id: format!("00000000-0000-4000-8000-00000000000{}", &id[1..]),
    }
}

/// A fork's record with a counted answer and a decider's revision, answerer ids raw: the door sends
/// them raw and Bifrost masks them for a reader who is not a decider.
fn record() -> ForkRecord {
    ForkRecord {
        schema_version: RecordSchema::V0,
        fork_id: FORK.into(),
        options_hash: "7d95fc87d532420611fe415c1def27a3447617bb68235f46e3af130aea361c48".into(),
        counted_answer_id: Some("n1".into()),
        answers: vec![
            answer("n1", "Counted", None, READER, "The asker lists her own forks.", "2026-11-11T17:00:00Z"),
            answer("n2", "Revision", Some("n1"), OTHER_DECIDER, "Agreed.", "2026-11-11T17:05:00Z"),
            ForkDeskAnswer { code: Some(huginn_mind::desk_wire::ForkAnswerCode::NotADecider), ..answer("n3", "Refused", None, OUTSIDER, "Me too.", "2026-11-11T17:06:00Z") },
            // A second answer by an account already counted: four answers, three answerers.
            ForkDeskAnswer {
                retracts: Some("n2".into()),
                choice: None,
                note: None,
                ..answer("n4", "Retraction", None, OTHER_DECIDER, "", "2026-11-11T17:07:00Z")
            },
        ],
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
    let whole = rmp_serde::to_vec_named(&record()).unwrap();
    files.push(("record-whole.payload.msgpack".into(), whole.clone()));
    files.push(("record-whole.payload.hex".into(), hex(&whole).into_bytes()));
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
    assert!(record().answers.iter().all(|answer| account_id(&answer.answerer)), "answerer");
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

/// The CultLib commit the vendored vectors come from (#117 on #115). Re-pin: see the README beside them.
const CULTLIB_SHA: &str = "3a51bbabad244a56ec4521080530d88744001ce4";

/// Each vendored vector and the SHA-256 of its bytes.
const VENDORED: &[(&str, &str)] = &[
    ("card-operator-fork", "56bfdb65ded60b562001d09d0b8da6e092900f404bf5f68f7084dff29bcc34f5"),
    ("card-raised-at-nil", "b81bb825790ffe03fabe63c3f8a61c36bd50ca268aa6dff67b5bcfad3957e58c"),
    ("card-withheld-deciders", "d13b9b77461cfdf78f943ef56c6aeab852ab6b6f0b355e26c5b361f99c79c7f7"),
    ("answer-counted", "55d69ae3cc7ebbeeae7d501e8641640e115c3fb5aac399ea760588bca343b9e6"),
    ("answer-revision", "19d7289395ae68cbaf6de1c0a1ef6c843211b2caf9462e1d2268cd0883386315"),
    ("answer-retraction", "89156f41ec3ea54f5d47bd3318913ff013785f4727916875be97785935cb7735"),
    ("answer-withheld-answerer", "b66b4f921cdd374be04af3de34c59e254a5e542027ae8c57f196a3ea3406e427"),
    ("answer-refused-stale", "3183bc9c483f4a625332cda11ef22a9c8560868a7728b77daa63a1960a3cd3d6"),
    ("entry-open-counted", "b51f499e8dd0465cf5df9d68d8fccbe94dd1339fd3157969693dd0d4ecb2549b"),
    ("entry-withheld-open-counted", "62ccad7a9f41262d6e35d0247315d0bb75d1313b3c9dabe8a99d1aa1b844607f"),
    ("entry-closed-with-ruling-and-a-refusal", "f7859c7d04d338d6b1c634308b08ae58ae77b0ff459767859a5e40f0af2f16e5"),
    ("entry-withheld-closed-with-ruling-and-a-refusal", "daaa819bfa2612616826ef3217dfcf8d8d8cb71d86547d7106426c779134f827"),
];

fn vendored(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desk-feed/cultlib-3a51bbab").join(format!("{name}.msgpack"));
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
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desk-feed/cultlib-3a51bbab");
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
/// null it on `Counted` and `Refused` only; nullable on all four variants is the ruling
/// `desk-reader-counts-shape`, and CultLib has no vector with a `Revision` or `Retraction` nulled.
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
    for (name, named) in [("answer-counted", true), ("answer-revision", true), ("answer-retraction", true), ("answer-refused-stale", true), ("answer-withheld-answerer", false)] {
        let bytes = vendored(name);
        assert_eq!(bytes[..3], [0xde, 0x00, 0x10], "{name}: header");
        check_answer(&decode(&bytes), named);
    }
    for (name, outcome) in [("answer-counted", "Counted"), ("answer-revision", "Revision"), ("answer-retraction", "Retraction"), ("answer-refused-stale", "Refused")] {
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

/// Huginn's real types against CultLib's bytes. A decider's answers already decode (the answer keeps
/// its 16 keys). The card and the record do not: `ForkCard` has no `raised_at` or `decider_count`,
/// `ForkRecord` no `answerer_count`, and `ForkDeskAnswer.answerer` is a non-null string, and each type
/// refuses unknown keys. This pins the gap `cut-desk-feed-listing` closes; when it does, these
/// refusals become successful decodes of the same bytes and this test changes with them.
#[test]
fn huginn_types_do_not_yet_carry_the_pinned_shapes() {
    use huginn_mind::desk_wire::ForkCard;
    for name in ["answer-counted", "answer-refused-stale"] {
        rmp_serde::from_slice::<ForkDeskAnswer>(&vendored(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    fn refused<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> String {
        rmp_serde::from_slice::<T>(bytes).err().expect("the real type refuses these bytes").to_string()
    }
    assert!(refused::<ForkCard>(&vendored("card-operator-fork")).contains("raised_at"));
    assert!(refused::<ForkCard>(&vendored("card-withheld-deciders")).contains("raised_at"));
    assert!(refused::<ForkDeskAnswer>(&vendored("answer-withheld-answerer")).contains("string"));
    // The record is the entry's last key: its bytes run from the `86` after the key to the end.
    let entry = vendored("entry-open-counted");
    let key = [&[0xa6u8][..], b"record", &[0x86]].concat();
    let at = entry.windows(key.len()).position(|window| window == key).expect("the record key") + key.len() - 1;
    assert!(refused::<ForkRecord>(&entry[at..]).contains("answerer_count"));
}
