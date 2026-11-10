//! CultNet's operation envelope around the mind's vocabulary. Pure functions:
//! a request goes out as `cultnet.operation_request.v0` and an answer comes
//! back as `cultnet.operation_response.v0`, both carrying base64 of named
//! MessagePack, which is what the C# reference and Eve's plugin ABI speak.
//!
//! A message this module cannot decode is a typed failure with a code, never
//! a transport error and never a call into a mind. `payload_encoding` is
//! CultNet's own check in `validate_message` and is not re-checked here.
//! `source_runtime_id` and `target_runtime_id` on a request are read by
//! nothing: ruling 18 says an identity is declared in the payload, not
//! granted by the envelope.

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cultnet_rs::CultNetMessage;
use crate::desk_wire::{DESK_REQUEST_SCHEMA, DESK_RESPONSE_SCHEMA, DESK_SERVICE_ID, DeskRequest, DeskResponse};
use crate::wire::{HuginnMindRequest, HuginnMindResponse, MIND_REQUEST_SCHEMA, MIND_RESPONSE_SCHEMA, MIND_SERVICE_ID};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The C# reference's `CultNetOperationServer.FailureSchemaId`. The Rust
/// `cultnet-rs` publishes no type for it, so this two-field mirror carries the
/// reference's keys; parity belongs in CultLib.
pub const FAILURE_SCHEMA: &str = "gamecult.cultnet.operation_failure.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OperationFailure {
    pub code: String,
    pub message: String,
}

impl OperationFailure {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

fn encode_payload<T: Serialize>(value: &T) -> Result<String> {
    Ok(STANDARD.encode(rmp_serde::to_vec_named(value)?))
}

/// One request to `service_id`, naming its own operation, its payload under `payload_schema`.
fn encode_operation_request<T: Serialize>(
    message_id: &str,
    service_id: &str,
    payload_schema: &str,
    operation: &str,
    request: &T,
    source_runtime_id: Option<String>,
) -> Result<CultNetMessage> {
    Ok(CultNetMessage::OperationRequest {
        message_id: message_id.to_string(),
        service_id: service_id.to_string(),
        operation: operation.to_string(),
        payload_schema: payload_schema.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: encode_payload(request)?,
        source_runtime_id,
        target_runtime_id: None,
    })
}

/// The message, or the failure that says why it is not a request of this
/// service. The envelope's `operation` string is checked against the decoded
/// request (`operation_of`) rather than trusted, so a payload cannot arrive
/// under another operation's name.
fn decode_operation_request<T: serde::de::DeserializeOwned>(
    message: &CultNetMessage,
    service_id: &str,
    payload_schema: &str,
    operation_of: impl Fn(&T) -> &'static str,
) -> Result<(String, T), OperationFailure> {
    let CultNetMessage::OperationRequest { message_id, service_id: sent_service, operation, payload_schema: sent_schema, payload, .. } = message
    else {
        return Err(OperationFailure::new(
            "not-an-operation-request",
            format!("{service_id} reads cultnet.operation_request.v0"),
        ));
    };
    if sent_service != service_id {
        return Err(OperationFailure::new("wrong-service", format!("the service is not {service_id}")));
    }
    if sent_schema != payload_schema {
        return Err(OperationFailure::new(
            "wrong-payload-schema",
            format!("the payload schema is not {payload_schema}"),
        ));
    }
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| OperationFailure::new("payload-not-base64", "the payload is not standard base64"))?;
    let request = read_payload::<T>(&bytes)?;
    let expected = operation_of(&request);
    if operation != expected {
        return Err(OperationFailure::new(
            "operation-mismatch",
            format!("the envelope's operation is not the payload's {expected}"),
        ));
    }
    Ok((message_id.clone(), request))
}

/// The payload as a `T`, or a failure naming the field path and the class
/// of error and nothing the caller sent: a decoder's own text quotes the
/// offending value (an unknown variant, an integer), so no decoder text
/// leaves this function.
fn read_payload<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, OperationFailure> {
    use rmp_serde::decode::Error as Decode;
    let mut decoder = rmp_serde::Deserializer::new(bytes);
    serde_path_to_error::deserialize(&mut decoder).map_err(|error| {
        let class = match error.inner() {
            Decode::InvalidMarkerRead(_) | Decode::InvalidDataRead(_) => "truncated",
            Decode::TypeMismatch(_) => "wrong-type",
            Decode::OutOfRange => "out-of-range",
            Decode::LengthMismatch(_) => "wrong-length",
            Decode::Uncategorized(_) | Decode::Syntax(_) => "invalid-value",
            Decode::Utf8Error(_) => "invalid-text",
            Decode::DepthLimitExceeded => "too-deep",
        };
        let path = error.path().to_string();
        let at = if path == "." { "the payload".to_string() } else { format!("field {path}") };
        OperationFailure::new("payload-not-a-request", format!("{at}: {class}"))
    })
}

/// One answer, with the status its caller derived.
fn encode_operation_response<T: Serialize>(
    message_id: &str,
    service_id: &str,
    payload_schema: &str,
    operation: &str,
    status: &str,
    response: &T,
    source_runtime_id: &str,
) -> Result<CultNetMessage> {
    Ok(CultNetMessage::OperationResponse {
        message_id: message_id.to_string(),
        service_id: service_id.to_string(),
        operation: operation.to_string(),
        status: status.to_string(),
        payload_schema: payload_schema.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: encode_payload(response)?,
        diagnostics: vec![],
        source_runtime_id: Some(source_runtime_id.to_string()),
    })
}

/// The answer to an envelope that never reached a mind. Infallible: the
/// failure is two strings and the daemon must always be able to say no.
fn encode_operation_failure(
    message_id: &str,
    service_id: &str,
    operation: &str,
    failure: &OperationFailure,
    source_runtime_id: &str,
) -> CultNetMessage {
    CultNetMessage::OperationResponse {
        message_id: message_id.to_string(),
        service_id: service_id.to_string(),
        operation: operation.to_string(),
        status: "rejected".to_string(),
        payload_schema: FAILURE_SCHEMA.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: STANDARD.encode(rmp_serde::to_vec_named(failure).expect("two strings encode")),
        diagnostics: vec![failure.code.clone()],
        source_runtime_id: Some(source_runtime_id.to_string()),
    }
}

/// The client's side: the correlation key and either the service's answer or
/// the failure that says the envelope never reached one.
fn decode_operation_response<T: serde::de::DeserializeOwned>(
    message: &CultNetMessage,
    service_id: &str,
    response_schema: &str,
) -> Result<(String, Result<T, OperationFailure>)> {
    let CultNetMessage::OperationResponse { message_id, payload_schema, payload, .. } = message else {
        anyhow::bail!("{service_id} answers with cultnet.operation_response.v0");
    };
    let bytes = STANDARD.decode(payload)?;
    let answer = if payload_schema == response_schema {
        Ok(rmp_serde::from_slice::<T>(&bytes)?)
    } else if payload_schema == FAILURE_SCHEMA {
        Err(rmp_serde::from_slice::<OperationFailure>(&bytes)?)
    } else {
        anyhow::bail!("{payload_schema} is neither {response_schema} nor {FAILURE_SCHEMA}")
    };
    Ok((message_id.clone(), answer))
}

/// One request, addressed to the mind service.
pub fn encode_request(
    message_id: &str,
    request: &HuginnMindRequest,
    source_runtime_id: Option<String>,
) -> Result<CultNetMessage> {
    encode_operation_request(message_id, MIND_SERVICE_ID, MIND_REQUEST_SCHEMA, request.operation(), request, source_runtime_id)
}

pub fn decode_request(message: &CultNetMessage) -> Result<(String, HuginnMindRequest), OperationFailure> {
    decode_operation_request(message, MIND_SERVICE_ID, MIND_REQUEST_SCHEMA, HuginnMindRequest::operation)
}

pub fn encode_response(
    message_id: &str,
    operation: &str,
    response: &HuginnMindResponse,
    source_runtime_id: &str,
) -> Result<CultNetMessage> {
    encode_operation_response(message_id, MIND_SERVICE_ID, MIND_RESPONSE_SCHEMA, operation, response.status(), response, source_runtime_id)
}

pub fn encode_failure(message_id: &str, operation: &str, failure: &OperationFailure, source_runtime_id: &str) -> CultNetMessage {
    encode_operation_failure(message_id, MIND_SERVICE_ID, operation, failure, source_runtime_id)
}

pub fn decode_response(message: &CultNetMessage) -> Result<(String, Result<HuginnMindResponse, OperationFailure>)> {
    decode_operation_response(message, MIND_SERVICE_ID, MIND_RESPONSE_SCHEMA)
}

/// One request, addressed to the desk service.
pub fn encode_desk_request(message_id: &str, request: &DeskRequest, source_runtime_id: Option<String>) -> Result<CultNetMessage> {
    encode_operation_request(message_id, DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, request.operation(), request, source_runtime_id)
}

pub fn decode_desk_request(message: &CultNetMessage) -> Result<(String, DeskRequest), OperationFailure> {
    decode_operation_request(message, DESK_SERVICE_ID, DESK_REQUEST_SCHEMA, DeskRequest::operation)
}

pub fn encode_desk_response(message_id: &str, operation: &str, response: &DeskResponse, source_runtime_id: &str) -> Result<CultNetMessage> {
    encode_operation_response(message_id, DESK_SERVICE_ID, DESK_RESPONSE_SCHEMA, operation, response.status(), response, source_runtime_id)
}

pub fn encode_desk_failure(message_id: &str, operation: &str, failure: &OperationFailure, source_runtime_id: &str) -> CultNetMessage {
    encode_operation_failure(message_id, DESK_SERVICE_ID, operation, failure, source_runtime_id)
}

pub fn decode_desk_response(message: &CultNetMessage) -> Result<(String, Result<DeskResponse, OperationFailure>)> {
    decode_operation_response(message, DESK_SERVICE_ID, DESK_RESPONSE_SCHEMA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{INSTANCE, OTHER_INSTANCE as OTHER, instance, provenance, slug};
    use crate::wire::MindStatus;
    use crate::{Faculty, MindRefusal, PipelineAdmissionBatch, PipelineAdmissionOutcome};

    fn refusal() -> MindRefusal {
        MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() }
    }

    /// A refusal is an answer: it rides the response schema with a `rejected`
    /// status and decodes back to the same value. A conflict and a replay are
    /// answers about the mind, not rejections of the request.
    #[test]
    fn a_refusal_is_a_typed_response_not_a_transport_failure() {
        let cases = [
            (HuginnMindResponse::Refused(refusal()), "rejected"),
            (HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal())), "rejected"),
            (
                HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed {
                    receipt_id: "r1".into(),
                    committed_at: "2026-09-16T12:00:00Z".into(),
                    writes: vec![],
                }),
                "accepted",
            ),
            (HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: "r1".into() }), "accepted"),
            (HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict { identities: vec![] }), "accepted"),
            (
                HuginnMindResponse::Whoami(MindStatus {
                    instance: slug(INSTANCE),
                    schema_epoch: "epiphany.pipeline.epoch.v2".into(),
                    documents: 0,
                    receipts: 0,
                    index: crate::wire::IndexStatus::Current,
                    persona: crate::wire::PersonaStatus::Absent,
                }),
                "accepted",
            ),
        ];
        for (response, expected) in cases {
            let message = encode_response("m-1", "admit", &response, "huginn-yggdrasil").unwrap();
            let CultNetMessage::OperationResponse { status, payload_schema, diagnostics, .. } = &message else {
                panic!("not an operation response");
            };
            assert_eq!(status, expected, "{response:?}");
            assert_eq!(payload_schema, MIND_RESPONSE_SCHEMA);
            assert!(diagnostics.is_empty());
            assert_eq!(decode_response(&message).unwrap(), ("m-1".into(), Ok(response)));
        }

        let failure = OperationFailure::new("wrong-service", "odin.catalog is not huginn.mind");
        let message = encode_failure("m-2", "query", &failure, "huginn-yggdrasil");
        let CultNetMessage::OperationResponse { status, payload_schema, diagnostics, .. } = &message else {
            panic!("not an operation response");
        };
        assert_eq!((status.as_str(), payload_schema.as_str()), ("rejected", FAILURE_SCHEMA));
        assert_eq!(diagnostics, &vec!["wrong-service".to_string()]);
        assert_eq!(decode_response(&message).unwrap(), ("m-2".into(), Err(failure)));
    }

    fn request_message(service_id: &str, operation: &str, schema: &str, payload: String) -> CultNetMessage {
        CultNetMessage::OperationRequest {
            message_id: "m-9".into(),
            service_id: service_id.into(),
            operation: operation.into(),
            payload_schema: schema.into(),
            payload_encoding: "messagepack-base64".into(),
            payload,
            source_runtime_id: None,
            target_runtime_id: None,
        }
    }

    /// The payload of `request` with the value at `path` replaced, encoded the
    /// way a client does.
    fn payload_with(request: &HuginnMindRequest, path: &[&str], bad: serde_json::Value) -> String {
        let mut value = serde_json::to_value(request).unwrap();
        let mut at = &mut value;
        for key in path {
            at = at.get_mut(*key).unwrap_or_else(|| panic!("no {key} in the request"));
        }
        *at = bad;
        STANDARD.encode(rmp_serde::to_vec_named(&value).unwrap())
    }

    /// A refusal of the decoder names the field and the class of error and
    /// nothing the caller sent: not an unknown variant, not an integer, not
    /// an envelope field, not a byte of bad base64. Over the persona wire and
    /// over the pipeline's, because the decoder is one.
    #[test]
    fn the_decoder_never_echoes_what_the_caller_sent() {
        const CANARY: &str = "CANARY-5c0de1-do-not-echo";
        const NUMBER: i64 = 90817263;
        let put = HuginnMindRequest::PersonaPut {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Hands),
            state: serde_json::json!({}),
            expected_updated_at: None,
        };
        let admit = HuginnMindRequest::Admit(PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Hands),
            documents: vec![instance(INSTANCE)],
        });
        let cases = [
            (&put, vec!["PersonaPut", "provenance", "faculty"], serde_json::json!(CANARY), "provenance.faculty"),
            (&put, vec!["PersonaPut", "provenance", "agent"], serde_json::json!(NUMBER), "provenance.agent"),
            (&put, vec!["PersonaPut", "expected_updated_at"], serde_json::json!(NUMBER), "expected_updated_at"),
            (&put, vec!["PersonaPut", "instance"], serde_json::json!(NUMBER), "instance"),
            (&admit, vec!["Admit", "provenance", "faculty"], serde_json::json!(CANARY), "provenance.faculty"),
            (&admit, vec!["Admit", "instance"], serde_json::json!(NUMBER), "instance"),
        ];
        for (request, path, bad, field) in cases {
            let message = request_message(MIND_SERVICE_ID, request.operation(), MIND_REQUEST_SCHEMA, payload_with(request, &path, bad));
            let failure = decode_request(&message).unwrap_err();
            assert_eq!(failure.code, "payload-not-a-request");
            assert!(failure.message.contains(field), "{field} is named: {}", failure.message);
            assert!(failure.message.ends_with(": invalid-value"), "the class is named: {}", failure.message);
            for echoed in [CANARY.to_string(), NUMBER.to_string()] {
                assert!(!failure.message.contains(&echoed), "{echoed} came back: {}", failure.message);
            }
        }

        let good = payload_with(&put, &["PersonaPut", "expected_updated_at"], serde_json::Value::Null);
        let envelope_cases = [
            request_message(CANARY, "persona_put", MIND_REQUEST_SCHEMA, good.clone()),
            request_message(MIND_SERVICE_ID, "persona_put", CANARY, good.clone()),
            request_message(MIND_SERVICE_ID, CANARY, MIND_REQUEST_SCHEMA, good),
            request_message(MIND_SERVICE_ID, "persona_put", MIND_REQUEST_SCHEMA, format!("{CANARY}!!")),
        ];
        for message in envelope_cases {
            let failure = decode_request(&message).unwrap_err();
            assert!(!failure.message.contains(CANARY), "{} came back: {}", failure.code, failure.message);
        }
    }

    /// The round trip the client and the daemon share, and the one code that
    /// `serve::answer` cannot reach because it answers a foreign message with
    /// `Error` instead.
    #[test]
    fn a_request_round_trips_and_a_foreign_message_is_not_a_request() {
        let request = HuginnMindRequest::Admit(PipelineAdmissionBatch {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Hands),
            documents: vec![instance(INSTANCE)],
        });
        let message = encode_request("m-3", &request, Some("eureka-state".into())).unwrap();
        assert_eq!(decode_request(&message).unwrap(), ("m-3".into(), request));

        let foreign = CultNetMessage::Error { error: "not for you".into(), code: None, details: None };
        assert_eq!(decode_request(&foreign).unwrap_err().code, "not-an-operation-request");
    }

    // ---- the desk's instantiation of the same cores ----

    use crate::desk_wire::{
        DESK_MAX_REQUEST_BYTES, DESK_MAX_RESPONSE_BYTES, DESK_REQUEST_SCHEMA, DESK_RESPONSE_SCHEMA, DESK_SERVICE_ID, DeskRequest, DeskResponse,
        ForkAnswer, ForkAnswerRequest, ForkDeskAnswer, ForkRefusal, ForkRefusalCode, RefusalSchema,
    };
    use cultnet_rs::{CultNetWireContract, encode_cultnet_message_to_vec};
    use eureka_pipeline::DeskAuthority;

    fn desk_answer() -> DeskRequest {
        DeskRequest::Answer {
            mind: slug(INSTANCE),
            subject: crate::fixtures::s("user-7"),
            authority: DeskAuthority::Member,
            deciders: vec![crate::fixtures::s("user-7")],
            request: ForkAnswerRequest {
                schema_version: crate::desk_wire::RequestSchema::V0,
                fork_id: "eureka-state:question:Q1".into(),
                options_hash: "0".repeat(64),
                answer: Some(ForkAnswer { choice: Some("A".into()), note: None }),
                retracted: false,
                retracts: None,
                answered_at: "2026-11-10T12:00:00Z".into(),
                request_id: "00000000-0000-4000-8000-000000000001".into(),
            },
        }
    }

    fn desk_requests() -> [(DeskRequest, &'static str); 3] {
        let (mind, subject, deciders) = (slug(INSTANCE), crate::fixtures::s("user-7"), vec![crate::fixtures::s("user-7")]);
        [
            (desk_answer(), "Answer"),
            (DeskRequest::Forks { mind: mind.clone(), subject: subject.clone(), deciders: deciders.clone(), status: Some(crate::desk_wire::ListStatus::Open), cursor: crate::desk_wire::ForkCursor::new(7), limit: crate::desk_wire::ForkLimit::new(50).unwrap() }, "Forks"),
            (DeskRequest::Fork { mind, subject, deciders, fork_id: "eureka-state:question:Q1".into() }, "Fork"),
        ]
    }

    /// The fields of a desk request ARE Libby's wire names, with no mapping layer between: `mind`,
    /// `subject`, `authority`, `deciders`, `request`. The persona and pipeline requests keep
    /// `instance`; the sets do not mix. A key outside the set, or one that would make the caller
    /// its own authority (`answerer`, `attribution`), refuses the whole payload.
    #[test]
    fn desk_request_fields_are_the_wire_names_and_the_set_is_closed() {
        let keys = |value: &serde_json::Value, variant: &str| {
            let mut keys: Vec<String> = value[variant].as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        };
        let expected = [
            ("Answer", vec!["authority", "deciders", "mind", "request", "subject"]),
            ("Forks", vec!["cursor", "deciders", "limit", "mind", "status", "subject"]),
            ("Fork", vec!["deciders", "fork_id", "mind", "subject"]),
        ];
        for ((request, variant), (named, names)) in desk_requests().iter().zip(expected) {
            assert_eq!(*variant, named);
            let value = serde_json::to_value(request).unwrap();
            assert_eq!(keys(&value, variant), names, "{variant}");
            let message = encode_desk_request("m-1", request, None).unwrap();
            assert_eq!(decode_desk_request(&message).unwrap(), ("m-1".into(), request.clone()));
            for extra in ["answerer", "attribution", "instance", "mood"] {
                let mut bad = value.clone();
                bad[*variant].as_object_mut().unwrap().insert(extra.into(), serde_json::json!(CANARY_KEY));
                let payload = STANDARD.encode(rmp_serde::to_vec_named(&bad).unwrap());
                let message = request_message(DESK_SERVICE_ID, request.operation(), DESK_REQUEST_SCHEMA, payload);
                let failure = decode_desk_request(&message).unwrap_err();
                assert_eq!(failure.code, "payload-not-a-request", "{variant} with {extra}");
                assert!(failure.message.contains(variant), "{variant} with {extra}: {}", failure.message);
                assert!(!failure.message.contains(CANARY_KEY), "{}", failure.message);
            }
            // `mind` is the desk's word: renamed to `instance` the payload is no desk request.
            let mut renamed = value.clone();
            let moved = renamed[*variant].as_object_mut().unwrap().remove("mind").unwrap();
            renamed[*variant].as_object_mut().unwrap().insert("instance".into(), moved);
            let payload = STANDARD.encode(rmp_serde::to_vec_named(&renamed).unwrap());
            let message = request_message(DESK_SERVICE_ID, request.operation(), DESK_REQUEST_SCHEMA, payload);
            assert_eq!(decode_desk_request(&message).unwrap_err().code, "payload-not-a-request");
        }
        // And the other way: the mind's `instance` named `mind` is no persona request.
        let put = HuginnMindRequest::PersonaPut {
            instance: slug(INSTANCE),
            provenance: provenance(Faculty::Hands),
            state: serde_json::json!({}),
            expected_updated_at: None,
        };
        let mut value = serde_json::to_value(&put).unwrap();
        let moved = value["PersonaPut"].as_object_mut().unwrap().remove("instance").unwrap();
        value["PersonaPut"].as_object_mut().unwrap().insert("mind".into(), moved);
        let message = request_message(MIND_SERVICE_ID, "persona_put", MIND_REQUEST_SCHEMA, STANDARD.encode(rmp_serde::to_vec_named(&value).unwrap()));
        assert_eq!(decode_request(&message).unwrap_err().code, "payload-not-a-request");
    }

    const CANARY_KEY: &str = "CANARY-9e41-do-not-echo";

    /// Two services share the cores and nothing else: a payload under the other service's schema,
    /// service id or operation name never reaches a decoder.
    #[test]
    fn desk_and_mind_payloads_do_not_cross() {
        let whoami = encode_request("m-5", &HuginnMindRequest::Whoami, None).unwrap();
        let CultNetMessage::OperationRequest { payload: mind_payload, .. } = &whoami else { panic!("not a request") };
        for (request, operation) in desk_requests().iter().map(|(request, _)| (request, request.operation())) {
            let CultNetMessage::OperationRequest { payload: desk_payload, .. } = encode_desk_request("m-6", request, None).unwrap() else { panic!("not a request") };
            // A desk payload under the mind's schema and a mind payload under the desk's.
            let under_mind = request_message(DESK_SERVICE_ID, operation, MIND_REQUEST_SCHEMA, desk_payload.clone());
            assert_eq!(decode_desk_request(&under_mind).unwrap_err().code, "wrong-payload-schema");
            let under_desk = request_message(MIND_SERVICE_ID, "whoami", DESK_REQUEST_SCHEMA, mind_payload.clone());
            assert_eq!(decode_request(&under_desk).unwrap_err().code, "wrong-payload-schema");
            // The mind's service id at the desk decoder, and the desk's at the mind's.
            let mind_service = request_message(MIND_SERVICE_ID, operation, DESK_REQUEST_SCHEMA, desk_payload.clone());
            assert_eq!(decode_desk_request(&mind_service).unwrap_err().code, "wrong-service");
            let desk_service = request_message(DESK_SERVICE_ID, "whoami", MIND_REQUEST_SCHEMA, mind_payload.clone());
            assert_eq!(decode_request(&desk_service).unwrap_err().code, "wrong-service");
            // The right labels around the other service's bytes: not a request.
            let wrong_bytes = request_message(DESK_SERVICE_ID, operation, DESK_REQUEST_SCHEMA, mind_payload.clone());
            assert_eq!(decode_desk_request(&wrong_bytes).unwrap_err().code, "payload-not-a-request");
            // An operation name that is not the payload's.
            let other = if operation == "forks" { "fork" } else { "forks" };
            let mismatch = request_message(DESK_SERVICE_ID, other, DESK_REQUEST_SCHEMA, desk_payload);
            assert_eq!(decode_desk_request(&mismatch).unwrap_err().code, "operation-mismatch");
        }
        // The three operations name themselves, on the envelope too.
        let named: Vec<String> = desk_requests()
            .iter()
            .map(|(request, _)| match encode_desk_request("m", request, None).unwrap() {
                CultNetMessage::OperationRequest { service_id, operation, payload_schema, .. } => {
                    assert_eq!((service_id.as_str(), payload_schema.as_str()), (DESK_SERVICE_ID, DESK_REQUEST_SCHEMA));
                    operation
                }
                _ => panic!("not a request"),
            })
            .collect();
        assert_eq!(named, ["answer", "forks", "fork"]);
    }

    /// A desk answer is an answer on the desk's response schema; a refusal that stored nothing and
    /// any other Mind refusal ride `rejected`; a failure rides the shared failure schema under the
    /// desk's service id.
    #[test]
    fn a_desk_response_round_trips_with_its_status() {
        let answered = DeskResponse::Answered {
            answer: ForkDeskAnswer {
                schema_version: crate::desk_wire::AnswerSchema::V0,
                answer_id: "n1".into(),
                fork_id: "eureka-state:question:Q1".into(),
                outcome: "Counted".into(),
                of: None,
                retracts: None,
                code: None,
                options_hash: "0".repeat(64),
                choice: Some("A".into()),
                note: None,
                answered_at: "2026-11-10T12:00:00Z".into(),
                received_at: "2026-11-10T12:00:01Z".into(),
                claim_in_future: false,
                answerer: "user-7".into(),
                authority: DeskAuthority::Member,
                request_id: "00000000-0000-4000-8000-000000000001".into(),
            },
            counted_answer_id: Some("n1".into()),
            as_of: 9,
        };
        let not_stored = DeskResponse::NotStored(ForkRefusal { schema_version: RefusalSchema::V0, code: ForkRefusalCode::ForkUnknown, error: "fork_unknown".into() });
        for (response, status) in [(answered, "accepted"), (not_stored, "rejected"), (DeskResponse::Refused(refusal()), "rejected")] {
            let message = encode_desk_response("m-7", "answer", &response, "huginn-yggdrasil").unwrap();
            let CultNetMessage::OperationResponse { service_id, status: sent, payload_schema, .. } = &message else { panic!("not a response") };
            assert_eq!((service_id.as_str(), sent.as_str(), payload_schema.as_str()), (DESK_SERVICE_ID, status, DESK_RESPONSE_SCHEMA));
            assert_eq!(decode_desk_response(&message).unwrap(), ("m-7".into(), Ok(response)));
        }
        let failure = OperationFailure::new("wrong-service", "the service is not huginn.desk");
        let message = encode_desk_failure("m-8", "forks", &failure, "huginn-yggdrasil");
        let CultNetMessage::OperationResponse { service_id, status, payload_schema, .. } = &message else { panic!("not a response") };
        assert_eq!((service_id.as_str(), status.as_str(), payload_schema.as_str()), (DESK_SERVICE_ID, "rejected", FAILURE_SCHEMA));
        assert_eq!(decode_desk_response(&message).unwrap(), ("m-8".into(), Err(failure)));
        // The mind's response schema is not the desk's.
        let mind = encode_response("m-9", "whoami", &HuginnMindResponse::Refused(refusal()), "huginn-yggdrasil").unwrap();
        assert!(decode_desk_response(&mind).is_err());
    }

    /// The largest message either side can build fits the frame the door enforces: 64 deciders of
    /// 200 bytes and a 4000-byte note in a request, the widest stored answer in a response.
    #[test]
    fn a_maximal_desk_message_fits_the_frame() {
        let long = |n: usize| "x".repeat(n);
        let wire = |message: &CultNetMessage| encode_cultnet_message_to_vec(message, CultNetWireContract::CultNetSchemaV0).unwrap().len();
        let deciders: Vec<_> = (0..64).map(|at| crate::fixtures::s(&format!("{at:0>200}"))).collect();
        let mut sized = vec![];
        for (request, _) in desk_requests() {
            let widest = match request {
                DeskRequest::Answer { mind, request, authority, .. } => DeskRequest::Answer {
                    mind,
                    subject: crate::fixtures::s(&long(200)),
                    authority,
                    deciders: deciders.clone(),
                    request: ForkAnswerRequest {
                        fork_id: long(200),
                        answer: Some(ForkAnswer { choice: Some(long(200)), note: Some(long(4000)) }),
                        retracts: Some("n4294967295".into()),
                        ..request
                    },
                },
                DeskRequest::Forks { mind, status, cursor, limit, .. } => {
                    DeskRequest::Forks { mind, subject: crate::fixtures::s(&long(200)), deciders: deciders.clone(), status, cursor, limit }
                }
                DeskRequest::Fork { mind, .. } => {
                    DeskRequest::Fork { mind, subject: crate::fixtures::s(&long(200)), deciders: deciders.clone(), fork_id: long(200) }
                }
            };
            let message = encode_desk_request("m-10", &widest, Some(long(200))).unwrap();
            assert_eq!(decode_desk_request(&message).unwrap().1, widest);
            sized.push(wire(&message));
        }
        assert!(sized.iter().all(|size| *size <= DESK_MAX_REQUEST_BYTES), "{sized:?} against {DESK_MAX_REQUEST_BYTES}");
        assert!(sized[0] > DESK_MAX_REQUEST_BYTES / 4, "the test builds a request that is actually large: {sized:?}");

        let answer = ForkDeskAnswer {
            schema_version: crate::desk_wire::AnswerSchema::V0,
            answer_id: "n4294967295".into(),
            fork_id: long(200),
            outcome: "Refused".into(),
            of: Some("n4294967295".into()),
            retracts: Some("n4294967295".into()),
            code: None,
            options_hash: "f".repeat(64),
            choice: Some(long(200)),
            note: Some(long(4000)),
            answered_at: "2026-11-10T12:00:00.123456789+00:00".into(),
            received_at: "2026-11-10T12:00:00.123456789+00:00".into(),
            claim_in_future: true,
            answerer: long(200),
            authority: DeskAuthority::Operator,
            request_id: "00000000-0000-4000-8000-000000000001".into(),
        };
        let response = DeskResponse::Answered { answer, counted_answer_id: Some("n4294967295".into()), as_of: u64::MAX };
        let message = encode_desk_response("m-11", "answer", &response, &long(200)).unwrap();
        assert!(wire(&message) <= DESK_MAX_RESPONSE_BYTES && wire(&message) > 4000, "{}", wire(&message));
        assert_eq!(decode_desk_response(&message).unwrap().1, Ok(response));
    }

    /// A desk call's refusal rides `huginn.desk_response.v1` whole: code and detail survive the
    /// wire, and the published schema declares the variant. One row per refusal a desk call can
    /// return, the four the door maps to `NotStored` included, because the response type can
    /// carry any of them.
    #[test]
    fn every_refusal_a_desk_call_can_return_round_trips_losslessly() {
        use crate::desk_wire::DESK_RESPONSE_SCHEMA_JSON;
        use eureka_pipeline::{PipelineKind, PipelineRefusal};
        let refusals = [
            MindRefusal::Unavailable { detail: "the store did not answer".into() },
            MindRefusal::ForeignInstance { declared: OTHER.into(), mind: INSTANCE.into() },
            MindRefusal::MissingIdentity,
            MindRefusal::DeskAnswerOutsideDesk { id: "eureka-state:desk_answer:Q1.n1".into() },
            MindRefusal::MissingReference { kind: PipelineKind::Question, id: "eureka-state:question:Q9".into() },
            MindRefusal::CampaignMoved { campaign: "eureka-state".into(), to: OTHER.into() },
            MindRefusal::DeskRequestInvalid { field: "desk_answer.note".into() },
            MindRefusal::DeskRequestIdConflict { request_id: "00000000-0000-4000-8000-000000000001".into() },
            MindRefusal::Document(PipelineRefusal::FieldBound { field: "note".into(), limit: 4000, actual: 4001 }),
            MindRefusal::Document(PipelineRefusal::InvalidFormat { field: "request_id".into(), value: "x".into() }),
            MindRefusal::Document(PipelineRefusal::InvalidIdentity { kind: PipelineKind::DeskAnswer, key: "k".into(), expected: "e".into() }),
            MindRefusal::Document(PipelineRefusal::DuplicateRepo { field: "repos".into(), repo: "GameCult/Huginn".into() }),
            MindRefusal::Document(PipelineRefusal::ForeignStore { r#type: "other".into() }),
        ];
        let schema: serde_json::Value = serde_json::from_str(DESK_RESPONSE_SCHEMA_JSON).unwrap();
        assert!(schema["$defs"]["MindRefusal"].is_object() && schema["$defs"]["PipelineRefusal"].is_object());
        let declared = serde_json::to_string(&schema).unwrap();
        for refusal in refusals {
            let response = DeskResponse::Refused(refusal.clone());
            let message = encode_desk_response("m-12", "answer", &response, "huginn-yggdrasil").unwrap();
            let (_, decoded) = decode_desk_response(&message).unwrap();
            assert_eq!(decoded, Ok(DeskResponse::Refused(refusal.clone())), "{refusal:?}");
            let value = serde_json::to_value(&refusal).unwrap();
            let variant = value.as_str().map(str::to_string).or_else(|| value.as_object().and_then(|object| object.keys().next().cloned())).unwrap();
            assert!(declared.contains(&format!("\"{variant}\"")), "the published schema declares {variant}");
            // Each detail field is declared too, so a validator can hold the payload to the schema.
            if let Some(object) = value.get(&variant).and_then(|inner| inner.as_object()) {
                for field in object.keys() {
                    assert!(declared.contains(&format!("\"{field}\"")), "{variant}.{field} is declared");
                }
            }
        }
    }
}
