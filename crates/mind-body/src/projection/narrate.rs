//! The Projector's narration, and the judge that measures it: one tool-less connector call each.
//!
//! `KitModel` is the stream organ's `KitProjector`, copied here when the stream was parked
//! (`persona-only-divergence-parked`), so the Projector has one caller on the connector.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use ghostlight_persona_projection::{ProjectorPrompt, build_projector_prompt};
use model_connector::{ConnectorClient, InputItem, ProviderRequest, ReasoningEffort, StopReason, TransportDisposition, TransportEventPayload, TransportInvocation, TransportOutcome};
use sha2::{Digest, Sha256};

use super::select::{Packet, TurnKind};
use super::tuning::Tuning;

/// One tool-less model turn. `KitModel` is the one live owner; a test answers with recorded text.
pub trait Model {
    /// One turn of `model`: `prompt` is its one user text and `request_id` its identity on the connector.
    fn ask(&self, request_id: &str, model: &str, prompt: &str) -> Result<String>;
}

/// What the connector's request asks of the model beside the user text. It names no membrane text: that is Ghostlight's.
pub(crate) const INSTRUCTIONS: &str = "Write only the text the request asks for.";
/// Room for the model's adaptive thinking; the narration itself is bounded by its word budget.
pub(crate) const MAX_OUTPUT_TOKENS: u32 = 1024;
const CONNECTOR_FRAME_BYTES: usize = 256 * 1024;
const INVOCATION_LIFE_MS: u64 = 60_000;

/// The connector key: the credential file `connector-key` in systemd's credentials directory, read once and trimmed. Its
/// error names the credential and never a byte of the file.
pub fn connector_key(credentials: &Path) -> Result<String> {
    let key = std::fs::read_to_string(credentials.join("connector-key")).map_err(|_| anyhow!("the connector-key credential could not be read"))?;
    Ok(key.trim().to_owned())
}

/// One tool-less connector turn by `caller` (her instance). Loopback only, as the kit is. Each call is its own
/// conversation: nothing is carried from one to the next. The model is the tuning's, named per call.
pub struct KitModel {
    client: ConnectorClient,
    caller: String,
}

impl KitModel {
    /// `endpoint` is the kit's loopback address; `connection_key` is the caller's key, read by the binary and never printed.
    pub fn open(endpoint: SocketAddr, connection_key: String, caller: &str, timeout: Duration) -> Result<Self> {
        if !endpoint.ip().is_loopback() {
            bail!("the connector endpoint is not a loopback address");
        }
        let client = ConnectorClient::new(endpoint, connection_key, CONNECTOR_FRAME_BYTES, Some(timeout)).map_err(|_| anyhow!("the connector client could not be configured"))?;
        Ok(Self { client, caller: caller.to_owned() })
    }
}

impl Model for KitModel {
    fn ask(&self, request_id: &str, model: &str, prompt: &str) -> Result<String> {
        let mut request = ProviderRequest::new(request_id, request_id, model, INSTRUCTIONS, MAX_OUTPUT_TOKENS);
        request.input = vec![InputItem::UserText { text: prompt.to_owned() }];
        request.reasoning_effort = Some(ReasoningEffort::Low);
        let expires = u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default() + INVOCATION_LIFE_MS;
        let invocation = TransportInvocation::new(&self.caller, expires, Sha256::digest(prompt.as_bytes()).into(), request).map_err(|_| anyhow!("the request is not a valid connector request"))?;
        let result = self.client.execute(&invocation).map_err(|fault| anyhow!("the connector did not answer: {fault}"))?;
        match result.disposition {
            TransportDisposition::Refused(reason) => bail!("the connector refused the call: {reason:?}"),
            TransportDisposition::Transported { events, receipt } => match receipt.outcome {
                TransportOutcome::Completed { stop_reason: StopReason::EndTurn, .. } => Ok(events
                    .into_iter()
                    .filter_map(|event| match event.payload {
                        TransportEventPayload::TextDelta { text } => Some(text),
                        _ => None,
                    })
                    .collect()),
                TransportOutcome::Completed { stop_reason, .. } => bail!("the model's turn ended early: {stop_reason:?}"),
                TransportOutcome::Failed { kind, .. } => bail!("the provider failed the call: {kind:?}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Narration.

pub(crate) fn word_budget(kind: TurnKind, tuning: &Tuning) -> usize {
    if kind == TurnKind::Beat { tuning.projector.beat_words } else { tuning.projector.words }
}

/// The Projector's prompt: Ghostlight's membrane over the packet, with the lens as where to begin.
pub(crate) fn projector_prompt(packet: &Packet, tuning: &Tuning) -> String {
    let guidance = format!("{}\n\nBegin from {}.", tuning.projector.guidance.trim(), packet.lens.words());
    build_projector_prompt(&ProjectorPrompt { identity: &packet.identity, typed_context: &packet.context, visible_stimulus: &packet.stimulus, domain_guidance: &guidance, word_budget: word_budget(packet.kind, tuning) })
}

/// `text` cut after its `max`-th word; the model's count is not trusted.
pub(crate) fn cap_words(text: &str, max: usize) -> String {
    let text = text.trim();
    let mut words = 0;
    let mut in_word = false;
    for (at, c) in text.char_indices() {
        if c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words += 1;
            if words > max {
                return text[..at].trim_end().to_owned();
            }
        }
    }
    text.to_owned()
}

pub(crate) fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Her persona state's schema: its property names are the identifiers a narration must never carry.
const PERSONA_SCHEMA: &str = include_str!("../../../../schemas/cultnet/gamecult.persona_state.v0.schema.json");

/// An identifier as compared: lowercased with its underscores gone, so `socialBonds`, `SocialBonds` and `social_bonds`
/// are one.
fn folded(identifier: &str) -> String {
    identifier.chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect()
}

/// Whether a word is shaped like an identifier rather than prose: an underscore, or a capital after its first letter.
fn identifier_shaped(word: &str) -> bool {
    word.contains('_') || word.chars().skip(1).any(char::is_uppercase)
}

fn keys_of(value: &serde_json::Value, schema: bool, into: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                if schema && key == "properties"
                    && let Some(properties) = inner.as_object()
                {
                    into.extend(properties.keys().cloned());
                }
                if !schema {
                    into.insert(key.clone());
                }
                keys_of(inner, schema, into);
            }
        }
        serde_json::Value::Array(values) => values.iter().for_each(|inner| keys_of(inner, schema, into)),
        _ => {}
    }
}

/// The folded compound identifiers of the persona schema and of the tuning: a plain word such as `trust` or
/// `tension` is prose and is not among them.
fn identifiers() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Ok(schema) = serde_json::from_str::<serde_json::Value>(PERSONA_SCHEMA) {
        keys_of(&schema, true, &mut keys);
    }
    if let Ok(tuning) = serde_json::to_value(Tuning::default()) {
        keys_of(&tuning, false, &mut keys);
    }
    keys.into_iter().filter(|key| identifier_shaped(key)).map(|key| folded(&key)).collect()
}

/// The identifiers of typed state a narration carries: a word shaped like an identifier that names a property of her
/// persona schema or a tuning key, in any case convention, and any id the packet named (`ids`), as an exact token when
/// it cannot be a plain word, or inside brackets as the packet writes it. A small deterministic guard on identifiers,
/// never on meaning: "need-to-know", "trust:" and "Tension:" are prose.
pub(crate) fn field_names(narration: &str, ids: &[String]) -> Vec<String> {
    let known = identifiers();
    let mut found = BTreeSet::new();
    for word in narration.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|word| !word.is_empty()) {
        if identifier_shaped(word) && known.contains(&folded(word)) {
            found.insert(word.to_owned());
        }
    }
    let tokens: BTreeSet<&str> = narration.split_whitespace().map(|token| token.trim_matches(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':'))).collect();
    for id in ids {
        let compound = id.contains(['-', '_', ':']) || id.chars().any(|c| c.is_ascii_digit());
        if (compound && tokens.contains(id.as_str())) || narration.contains(&format!("[{id}]")) {
            found.insert(id.clone());
        }
    }
    found.into_iter().collect()
}

// ---------------------------------------------------------------------------------------------
// The eval: distinct-2 and the judge.

/// Distinct bigrams over all bigrams of the narrations, lowercased words: 1 is no repetition.
pub(crate) fn distinct_2(narrations: &[String]) -> f64 {
    let mut all = 0usize;
    let mut distinct = BTreeSet::new();
    for narration in narrations {
        let words: Vec<String> = narration.split_whitespace().map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).filter(|word| !word.is_empty()).collect();
        for pair in words.windows(2) {
            all += 1;
            distinct.insert((pair[0].clone(), pair[1].clone()));
        }
    }
    if all == 0 { 0.0 } else { distinct.len() as f64 / all as f64 }
}

/// The judge's question: which of two packets a narration was written from.
pub(crate) fn judge_prompt(first: &Packet, second: &Packet, narration: &str) -> String {
    format!(
        "Two descriptions, A and B, of one person's present moment differ in what she carries. Below them is a first-person passage written from exactly one of them. Answer with the single letter of the description the passage was written from: A or B.\n\nA:\n{}\n{}\n\nB:\n{}\n{}\n\nPassage:\n{narration}\n\nAnswer A or B.",
        first.context, first.stimulus, second.context, second.stimulus
    )
}

/// The judge's letter, or None when its answer does not start with one.
pub(crate) fn judged(answer: &str) -> Option<char> {
    answer.trim().trim_start_matches(['*', '"', '\'', '(']).chars().next().map(|c| c.to_ascii_uppercase()).filter(|c| *c == 'A' || *c == 'B')
}
