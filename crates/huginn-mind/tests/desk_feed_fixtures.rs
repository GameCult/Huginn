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

use serde::Serialize;

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

// ---- Reader views, as MIRROR TYPES (ruling desk-read-visibility-withheld-ids; shape ruling
// desk-reader-counts-shape, Libby's CultLib reader-counts cut). These types exist only here: Huginn
// does not project reader views yet (cut-desk-feed-listing), and CultLib's re-pinned vectors are not
// posted yet. They pin the rules, not CultLib's bytes; when Libby posts the sha, her vectors are
// decoded against Huginn's real types instead. ----

/// The card's `deciders` and `decider_count` (right after `deciders`), as a fragment. Not a card.
#[derive(Serialize)]
struct CardViewers {
    deciders: Vec<String>,
    decider_count: u64,
}

/// What the call's deciders look like to one reader: the ids for a decider, `[]` (withheld) for
/// anyone else; the count either way.
fn card_viewers(decider: bool) -> CardViewers {
    let ids = vec![READER.to_string(), OTHER_DECIDER.to_string()];
    CardViewers { decider_count: ids.len() as u64, deciders: if decider { ids } else { Vec::new() } }
}

/// The card rule: `decider_count` is 1..64, and `deciders` is either empty (withheld) or holds
/// exactly `decider_count` ids.
fn card_viewers_hold(view: &CardViewers) -> bool {
    (1..=64).contains(&view.decider_count) && (view.deciders.is_empty() || view.deciders.len() as u64 == view.decider_count)
}

/// `gamecult.desk_answer.v0` with `answerer` nullable (all four variants).
#[derive(Serialize)]
struct ViewAnswer {
    #[serde(rename = "schemaVersion")]
    schema_version: AnswerSchema,
    answer_id: String,
    fork_id: String,
    outcome: String,
    of: Option<String>,
    retracts: Option<String>,
    code: Option<huginn_mind::desk_wire::ForkAnswerCode>,
    options_hash: String,
    choice: Option<String>,
    note: Option<String>,
    answered_at: String,
    received_at: String,
    claim_in_future: bool,
    answerer: Option<String>,
    authority: DeskAuthority,
    request_id: String,
}

/// `gamecult.fork.record.v0` with `answerer_count`: six keys (header 0x86). The count's position
/// among the six is CultLib's; this mirror puts it before `answers` until the vectors say.
#[derive(Serialize)]
struct ViewRecord {
    #[serde(rename = "schemaVersion")]
    schema_version: RecordSchema,
    fork_id: String,
    options_hash: String,
    counted_answer_id: Option<String>,
    answerer_count: u64,
    answers: Vec<ViewAnswer>,
}

/// One record as one reader sees it. `answerer_count` is the distinct answerers over ALL stored
/// answers, refusals included, for both readers; only a non-decider's view nils `answerer`.
fn view(record: ForkRecord, decider: bool) -> ViewRecord {
    let mut answerers: Vec<&str> = record.answers.iter().map(|answer| answer.answerer.as_str()).collect();
    answerers.sort_unstable();
    answerers.dedup();
    let answerer_count = answerers.len() as u64;
    ViewRecord {
        schema_version: record.schema_version,
        fork_id: record.fork_id,
        options_hash: record.options_hash,
        counted_answer_id: record.counted_answer_id,
        answerer_count,
        answers: record
            .answers
            .into_iter()
            .map(|answer| ViewAnswer {
                schema_version: answer.schema_version,
                answer_id: answer.answer_id,
                fork_id: answer.fork_id,
                outcome: answer.outcome,
                of: answer.of,
                retracts: answer.retracts,
                code: answer.code,
                options_hash: answer.options_hash,
                choice: answer.choice,
                note: answer.note,
                answered_at: answer.answered_at,
                received_at: answer.received_at,
                claim_in_future: answer.claim_in_future,
                answerer: decider.then_some(answer.answerer),
                authority: answer.authority,
                request_id: answer.request_id,
            })
            .collect(),
    }
}

/// The record rule: `answerer` is null exactly in a non-decider's view.
fn record_view_holds(view: &ViewRecord, decider: bool) -> bool {
    view.answers.iter().all(|answer| answer.answerer.is_some() == decider)
}

/// The mirror fragments by reader kind: (file stem, bytes).
fn reader_views() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("card-viewers-decider.synthetic", rmp_serde::to_vec_named(&card_viewers(true)).unwrap()),
        ("card-viewers-non-decider.synthetic", rmp_serde::to_vec_named(&card_viewers(false)).unwrap()),
        ("record-decider.mirror", rmp_serde::to_vec_named(&view(record(), true)).unwrap()),
        ("record-non-decider.mirror", rmp_serde::to_vec_named(&view(record(), false)).unwrap()),
    ]
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
    for (name, bytes) in reader_views() {
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

fn holds(bytes: &[u8], text: &str) -> bool {
    bytes.windows(text.len()).any(|window| window == text.as_bytes())
}

/// MIRROR TYPES ONLY: these tests pin the reader-view rules on the types above, not Huginn's
/// projection (which does not withhold ids yet) and not CultLib's bytes (pending Libby's sha). The
/// tests on real list and read responses land with `cut-desk-feed-listing`.
///
/// A non-decider's fragments hold no account id anywhere in their bytes, only counts; a decider's
/// hold every id and the same counts.
#[test]
fn a_non_decider_gets_counts_and_no_ids_a_decider_gets_ids() {
    let views = reader_views();
    let bytes = |name: &str| views.iter().find(|(stem, _)| *stem == name).unwrap().1.clone();
    for name in ["card-viewers-non-decider.synthetic", "record-non-decider.mirror"] {
        let held = bytes(name);
        assert!([READER, OTHER_DECIDER, OUTSIDER].iter().all(|id| !holds(&held, id)), "{name} carries an account id");
    }
    let held = bytes("card-viewers-decider.synthetic");
    assert!(holds(&held, READER) && holds(&held, OTHER_DECIDER));
    let held = bytes("record-decider.mirror");
    assert!([READER, OTHER_DECIDER, OUTSIDER].iter().all(|id| holds(&held, id)), "a decider's record keeps every answerer id");
    assert_eq!(card_viewers(false).decider_count, card_viewers(true).decider_count);
}

/// `answerer_count` counts DISTINCT answerers over every stored answer, the refused one included
/// (four answers, one account answering twice, count 3),
/// and is the same for both readers; a record view has six keys (header 0x86).
#[test]
fn answerer_count_covers_refusals_and_the_record_has_six_keys() {
    let outcomes: Vec<String> = record().answers.iter().map(|answer| answer.outcome.clone()).collect();
    assert_eq!(outcomes, ["Counted", "Revision", "Refused", "Retraction"], "all four variants");
    assert_eq!(record().answers.len(), 4, "four answers by three accounts");
    assert_eq!((view(record(), false).answerer_count, view(record(), true).answerer_count), (3, 3));
    for name in ["record-decider.mirror", "record-non-decider.mirror"] {
        let held = reader_views().into_iter().find(|(stem, _)| *stem == name).unwrap().1;
        assert_eq!(held[0], 0x86, "{name}");
    }
}

/// `decider_count` is 1..64; `deciders` is empty (withheld) or holds exactly `decider_count` ids;
/// `answerer` is null exactly in a non-decider's view.
#[test]
fn a_non_empty_decider_list_matches_its_count_and_answerer_is_null_only_for_non_deciders() {
    assert!(card_viewers_hold(&card_viewers(true)) && card_viewers_hold(&card_viewers(false)));
    let ids = |n: usize| (0..n).map(|at| format!("00000000-0000-4000-8000-{at:012x}")).collect::<Vec<_>>();
    for (deciders, decider_count, holds) in [
        (ids(1), 2, false),
        (ids(3), 2, false),
        (Vec::new(), 0, false),
        (Vec::new(), 65, false),
        (ids(64), 64, true),
        (Vec::new(), 64, true),
        (ids(1), 1, true),
    ] {
        assert_eq!(card_viewers_hold(&CardViewers { deciders, decider_count }), holds);
    }
    assert!(record_view_holds(&view(record(), true), true) && record_view_holds(&view(record(), false), false));
    assert!(!record_view_holds(&view(record(), true), false) && !record_view_holds(&view(record(), false), true));
}
