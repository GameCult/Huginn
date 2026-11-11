//! The desk feed's golden fixtures: the bytes Bifrost writes to a Mind's desk socket for a list or
//! a read, and the bytes Huginn writes back, built here with Huginn's own encoders so the files
//! are what the door speaks. `docs/desk-feed-door.md` is the contract they pin; the README in the
//! fixtures directory lists them.
//!
//! The card (`gamecult.fork.card.v0`) is in no fixture: it waits on CultLib's re-pinned card
//! vectors for `raised_at`. Every response pinned here carries no entry.
//!
//! `cargo test -p huginn-mind --test desk_feed_fixtures` compares every file with what the
//! encoders produce now. `HUGINN_WRITE_DESK_FIXTURES=1` rewrites them instead.

use std::path::PathBuf;

use cultnet_rs::{CultNetMessage, CultNetWireContract, encode_cultnet_message_to_vec, encode_frame};
use huginn_mind::MindRefusal;
use huginn_mind::desk_wire::{
    fork_too_large_read, AnswerSchema, DeskRequest, DeskResponse, ForkCursor, ForkDeskAnswer, ForkLimit, ForkList, ForkRecord, ForkRefusal, ForkRefusalCode,
    ListSchema, ListStatus, RecordSchema,
};
use huginn_mind::envelope::{OperationFailure, encode_desk_failure, encode_desk_request, encode_desk_response};
use huginn_mind::eureka_pipeline::{DeskAuthority, Short, Slug};

const RUNTIME: &str = "huginn-eureka";
/// A question Eureka's mind holds (asked 2026-11-11). The answers, notes and hash in `record()` are
/// synthetic: the hash is CultLib's sample digest, not this question's.
const FORK: &str = "eureka-substrate:question:desk-listing-authority";

fn s(text: &str) -> Short {
    Short(text.into())
}

fn forks(status: Option<ListStatus>, cursor: Option<u64>, limit: u8) -> DeskRequest {
    DeskRequest::Forks {
        mind: Slug("eureka".into()),
        subject: s("7"),
        deciders: vec![s("7"), s("12")],
        status,
        cursor: cursor.and_then(ForkCursor::new),
        limit: ForkLimit::new(limit).unwrap(),
    }
}

fn read() -> DeskRequest {
    DeskRequest::Fork { mind: Slug("eureka".into()), subject: s("7"), deciders: vec![s("7"), s("12")], fork_id: FORK.into() }
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
            answer("n1", "Counted", None, "7", "The asker lists her own forks.", "2026-11-11T17:00:00Z"),
            answer("n2", "Revision", Some("n1"), "12", "Agreed.", "2026-11-11T17:05:00Z"),
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
