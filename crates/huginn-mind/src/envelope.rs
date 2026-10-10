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

/// One request, addressed to the mind service and naming its own operation.
pub fn encode_request(
    message_id: &str,
    request: &HuginnMindRequest,
    source_runtime_id: Option<String>,
) -> Result<CultNetMessage> {
    Ok(CultNetMessage::OperationRequest {
        message_id: message_id.to_string(),
        service_id: MIND_SERVICE_ID.to_string(),
        operation: request.operation().to_string(),
        payload_schema: MIND_REQUEST_SCHEMA.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: encode_payload(request)?,
        source_runtime_id,
        target_runtime_id: None,
    })
}

/// The message, or the failure that says why it is not a request of this
/// service. The envelope's `operation` string is checked against the decoded
/// request rather than trusted, so a payload cannot arrive under another
/// operation's name.
pub fn decode_request(message: &CultNetMessage) -> Result<(String, HuginnMindRequest), OperationFailure> {
    let CultNetMessage::OperationRequest { message_id, service_id, operation, payload_schema, payload, .. } = message
    else {
        return Err(OperationFailure::new(
            "not-an-operation-request",
            "huginn.mind reads cultnet.operation_request.v0",
        ));
    };
    if service_id != MIND_SERVICE_ID {
        return Err(OperationFailure::new("wrong-service", format!("the service is not {MIND_SERVICE_ID}")));
    }
    if payload_schema != MIND_REQUEST_SCHEMA {
        return Err(OperationFailure::new(
            "wrong-payload-schema",
            format!("the payload schema is not {MIND_REQUEST_SCHEMA}"),
        ));
    }
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| OperationFailure::new("payload-not-base64", "the payload is not standard base64"))?;
    let request = read_request(&bytes)?;
    if operation != request.operation() {
        return Err(OperationFailure::new(
            "operation-mismatch",
            format!("the envelope's operation is not the payload's {}", request.operation()),
        ));
    }
    Ok((message_id.clone(), request))
}

/// The payload as a request, or a failure naming the field path and the class
/// of error and nothing the caller sent: a decoder's own text quotes the
/// offending value (an unknown variant, an integer), so no decoder text
/// leaves this function.
fn read_request(bytes: &[u8]) -> Result<HuginnMindRequest, OperationFailure> {
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

/// One answer, whose status the response derives.
pub fn encode_response(
    message_id: &str,
    operation: &str,
    response: &HuginnMindResponse,
    source_runtime_id: &str,
) -> Result<CultNetMessage> {
    Ok(CultNetMessage::OperationResponse {
        message_id: message_id.to_string(),
        service_id: MIND_SERVICE_ID.to_string(),
        operation: operation.to_string(),
        status: response.status().to_string(),
        payload_schema: MIND_RESPONSE_SCHEMA.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: encode_payload(response)?,
        diagnostics: vec![],
        source_runtime_id: Some(source_runtime_id.to_string()),
    })
}

/// The answer to an envelope that never reached a mind. Infallible: the
/// failure is two strings and the daemon must always be able to say no.
pub fn encode_failure(
    message_id: &str,
    operation: &str,
    failure: &OperationFailure,
    source_runtime_id: &str,
) -> CultNetMessage {
    CultNetMessage::OperationResponse {
        message_id: message_id.to_string(),
        service_id: MIND_SERVICE_ID.to_string(),
        operation: operation.to_string(),
        status: "rejected".to_string(),
        payload_schema: FAILURE_SCHEMA.to_string(),
        payload_encoding: "messagepack-base64".to_string(),
        payload: STANDARD.encode(rmp_serde::to_vec_named(failure).expect("two strings encode")),
        diagnostics: vec![failure.code.clone()],
        source_runtime_id: Some(source_runtime_id.to_string()),
    }
}

/// The client's side: the correlation key and either the mind's answer or the
/// failure that says the envelope never reached one.
pub fn decode_response(message: &CultNetMessage) -> Result<(String, Result<HuginnMindResponse, OperationFailure>)> {
    let CultNetMessage::OperationResponse { message_id, payload_schema, payload, .. } = message else {
        anyhow::bail!("huginn.mind answers with cultnet.operation_response.v0");
    };
    let bytes = STANDARD.decode(payload)?;
    let answer = match payload_schema.as_str() {
        MIND_RESPONSE_SCHEMA => Ok(rmp_serde::from_slice::<HuginnMindResponse>(&bytes)?),
        FAILURE_SCHEMA => Err(rmp_serde::from_slice::<OperationFailure>(&bytes)?),
        other => anyhow::bail!("{other} is neither {MIND_RESPONSE_SCHEMA} nor {FAILURE_SCHEMA}"),
    };
    Ok((message_id.clone(), answer))
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
}
