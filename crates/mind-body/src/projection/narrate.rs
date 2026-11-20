//! The Projector's narration, the Persona stage's continuation, and the judge that measures them: one tool-less
//! connector call each.
//!
//! `KitModel` is the stream organ's `KitProjector`, copied here when the stream was parked
//! (`persona-only-divergence-parked`), so the Projector has one caller on the connector.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use ghostlight_persona_projection::{PersonaPrompt, ProjectorPrompt, build_persona_prompt, build_projector_prompt};
use model_connector::{ConnectorClient, InputItem, ProviderRequest, ReasoningEffort, StopReason, TransportDisposition, TransportEventPayload, TransportInvocation, TransportOutcome};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::select::{Packet, TurnKind};
use super::tuning::Tuning;

/// One tool-less model turn. `KitModel` is the one live owner; a test answers with recorded text.
pub trait Model {
    /// One turn of `model`: `prompt` is its one user text and `request_id` its identity on the connector. A failed turn
    /// keeps whatever usage the connector reported for it.
    fn ask(&self, request_id: &str, model: &str, prompt: &str) -> std::result::Result<Answer, Failed>;
}

/// A turn that produced no answer: why, and what it cost when the connector's receipt says (a refusal, a fault before
/// the provider, or a failure with no usage block costs none). A receipt whose usage block is missing or malformed does
/// not decode at all: that is model-connector's error, reported as the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    pub reason: String,
    pub usage: Option<Spent>,
}

impl std::fmt::Display for Failed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl Failed {
    fn without_usage(reason: impl Into<String>) -> Self {
        Self { reason: reason.into(), usage: None }
    }
}

fn spent(model: &str, usage: &model_connector::Usage) -> Spent {
    Spent { model: model.to_owned(), input_tokens: usage.input_tokens, output_tokens: usage.output_tokens, cache_read_tokens: usage.cache_read_tokens, cache_write_tokens: usage.cache_write_tokens, reasoning_tokens: usage.reasoning_tokens }
}

/// A model's answer, and what the turn cost as the connector counted it: none when the answerer counts nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub usage: Option<Spent>,
}

/// One turn's tokens, by the model that ran it, as the connector's receipt reports them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Spent {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: Option<u64>,
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
    fn ask(&self, request_id: &str, model: &str, prompt: &str) -> std::result::Result<Answer, Failed> {
        let mut request = ProviderRequest::new(request_id, request_id, model, INSTRUCTIONS, MAX_OUTPUT_TOKENS);
        request.input = vec![InputItem::UserText { text: prompt.to_owned() }];
        request.reasoning_effort = Some(ReasoningEffort::Low);
        let expires = u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default() + INVOCATION_LIFE_MS;
        let invocation = TransportInvocation::new(&self.caller, expires, Sha256::digest(prompt.as_bytes()).into(), request).map_err(|_| Failed::without_usage("the request is not a valid connector request"))?;
        let result = self.client.execute(&invocation).map_err(|fault| Failed::without_usage(format!("the connector did not answer: {fault}")))?;
        match result.disposition {
            TransportDisposition::Refused(reason) => Err(Failed::without_usage(format!("the connector refused the call: {reason:?}"))),
            TransportDisposition::Transported { events, receipt } => match receipt.outcome {
                TransportOutcome::Completed { stop_reason: StopReason::EndTurn, usage, .. } => Ok(Answer {
                    text: events
                        .into_iter()
                        .filter_map(|event| match event.payload {
                            TransportEventPayload::TextDelta { text } => Some(text),
                            _ => None,
                        })
                        .collect(),
                    usage: Some(spent(model, &usage)),
                }),
                TransportOutcome::Completed { stop_reason, usage, .. } => Err(Failed { reason: format!("the model's turn ended early: {stop_reason:?}"), usage: Some(spent(model, &usage)) }),
                TransportOutcome::Failed { kind, usage, .. } => Err(Failed { reason: format!("the provider failed the call: {kind:?}"), usage: usage.as_ref().map(|usage| spent(model, usage)) }),
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
    let guidance = format!("{}\n\nBegin from {}.", tuning.projector.guidance.trim(), packet.lens.words(&packet.forms));
    build_projector_prompt(&ProjectorPrompt { identity: &packet.identity, typed_context: &packet.context, visible_stimulus: &packet.stimulus, domain_guidance: &guidance, word_budget: word_budget(packet.kind, tuning) })
}

/// A character that is a word on its own: Han, kana and Hangul are written without spaces, so each counts as one.
fn cjk(c: char) -> bool {
    matches!(u32::from(c), 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0x20000..=0x2FFFF)
}

/// CJK and full-width punctuation (`。`, `，`, `！`, `？`, `「」`): it belongs to the word before it and starts none.
fn cjk_punctuation(c: char) -> bool {
    matches!(u32::from(c), 0x3000..=0x303F | 0xFF01..=0xFF0F | 0xFF1A..=0xFF20 | 0xFF3B..=0xFF40 | 0xFF5B..=0xFF65)
}

/// The byte offset where each word of `text` begins: a run of non-space characters is one word, and a CJK character is
/// a word of its own.
fn word_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut in_word = false;
    for (at, c) in text.char_indices() {
        if c.is_whitespace() || cjk_punctuation(c) {
            in_word = false;
        } else if cjk(c) {
            starts.push(at);
            in_word = false;
        } else if !in_word {
            starts.push(at);
            in_word = true;
        }
    }
    starts
}

/// The share of the budget a sentence cut must keep; below it the word cut keeps more of the stream.
const SENTENCE_CUT_FLOOR: f64 = 0.6;

// ---------------------------------------------------------------------------------------------
// The Persona stage.

/// What the Persona is shown as her moment: the Projector's narration, then what was said to her and in her room,
/// verbatim, as code rendered it (map section 3.2: the Projector never paraphrases what she heard).
pub(crate) fn lived_stream(narration: &str, heard: &str) -> String {
    if heard.trim().is_empty() { narration.to_owned() } else { format!("{narration}\n\n{heard}") }
}

/// The Persona stage's prompt: Ghostlight's prose-only membrane over her identity and her lived stream. No field name,
/// number, schema or instruction of hers reaches it (invariant 7); the guidance is the tuning's setting line.
pub(crate) fn persona_prompt(packet: &Packet, narration: &str, tuning: &Tuning) -> String {
    build_persona_prompt(&PersonaPrompt { identity: &packet.identity, lived_stream: &lived_stream(narration, &packet.heard), domain_guidance: tuning.persona.guidance.trim(), word_budget: tuning.persona.words })
}

/// The share of the narration's word trigrams that also appear in what she heard: a lexical overlap, cheap and blind to
/// paraphrase. A metric, never a filter: 0 is no shared word run.
pub(crate) fn heard_overlap(narration: &str, heard: &str) -> f64 {
    let trigrams = |text: &str| -> Vec<(String, String, String)> {
        let words: Vec<String> = text.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|word| !word.is_empty()).map(str::to_lowercase).collect();
        words.windows(3).map(|three| (three[0].clone(), three[1].clone(), three[2].clone())).collect()
    };
    let told = trigrams(narration);
    if told.is_empty() {
        return 0.0;
    }
    let heard: BTreeSet<(String, String, String)> = trigrams(heard).into_iter().collect();
    told.iter().filter(|three| heard.contains(three)).count() as f64 / told.len() as f64
}

/// Whether a narration lays out options for her: "I may…", "I could…", "she might…", "nothing asks me to…". The Projector
/// renders the moment and the Persona decides; a menu of choices is the Projector deciding. A lexical flag on a narrow
/// form, reported as a metric, never a filter.
pub(crate) fn offers_options(narration: &str) -> bool {
    let lower = format!(" {} ", narration.to_lowercase().replace('\u{2019}', "'"));
    ["i may ", "i could ", "i might ", "she may ", "she could ", "she might ", "nothing asks me", "nothing asks her", "or i could", "or she could"]
        .iter()
        .any(|form| lower.contains(&format!(" {form}")) || lower.contains(&format!(". {form}")) || lower.contains(&format!(", {form}")))
}

/// `text` held to `max` words (whitespace-separated, each CJK character one); the model's count is not trusted. An
/// overrun is cut at the last sentence end inside the budget, so the stream ends on a whole sentence, when that keeps
/// at least 60% of the budget; otherwise, or with no sentence end inside it, it is cut after the `max`-th word. No
/// abbreviation list: a cut after "e.g." is accepted.
pub(crate) fn cap_words(text: &str, max: usize) -> String {
    let text = text.trim();
    let starts = word_starts(text);
    let Some(&over) = starts.get(max) else { return text.to_owned() };
    let within = text[..over].trim_end();
    match last_sentence_end(within) {
        Some(end) if words(&within[..end]) as f64 >= SENTENCE_CUT_FLOOR * max as f64 => within[..end].to_owned(),
        _ => within.to_owned(),
    }
}

/// The byte just past the last sentence end in `text`: a `.`, `!`, `?` or `…` with any closing quotes or brackets after
/// it, followed by whitespace or the end of the text; or a `。`, `！` or `？`, which ends a sentence where it stands.
fn last_sentence_end(text: &str) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut found = None;
    for (index, (at, c)) in chars.iter().enumerate() {
        let full_width = matches!(c, '。' | '！' | '？');
        if !full_width && !matches!(c, '.' | '!' | '?' | '…') {
            continue;
        }
        let mut end = at + c.len_utf8();
        let mut next = index + 1;
        while let Some((close_at, close)) = chars.get(next).filter(|(_, close)| matches!(close, '"' | '\'' | '”' | '’' | ')' | ']' | '」' | '』')) {
            end = close_at + close.len_utf8();
            next += 1;
        }
        if full_width || chars.get(next).is_none_or(|(_, after)| after.is_whitespace()) {
            found = Some(end);
        }
    }
    found
}

/// Words as the cap counts them.
pub(crate) fn words(text: &str) -> usize {
    word_starts(text).len()
}

/// The one kebab form that is ordinary English: "short-term" folds to the schema key `shortTerm`, and prose must not fail
/// the judge for it. `shortTerm`, `short_term` and the run-together `shortterm` are still caught.
const KEBAB_ENGLISH: &str = "short-term";

/// Her persona state's schema: its property names are the identifiers a narration must never carry.
const PERSONA_SCHEMA: &str = include_str!("../../../../schemas/cultnet/gamecult.persona_state.v0.schema.json");

/// An identifier as compared: lowercased with its underscores and hyphens gone, so `socialBonds`, `SocialBonds`,
/// `social_bonds`, `social-bonds`, `SOCIAL_BONDS` and `socialbonds` are one.
fn folded(identifier: &str) -> String {
    identifier.chars().filter(|c| !matches!(c, '_' | '-')).flat_map(char::to_lowercase).collect()
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

/// The identifiers of typed state a narration carries: any token that folds to a compound property name of her persona
/// schema or a tuning key, in every case convention (camelCase, PascalCase, snake_case, SCREAMING_CASE, kebab-case and
/// run together), and any id the packet named (`ids`), as an exact token when it cannot be a plain word, or inside
/// brackets as the packet writes it. Only compound names are known, so a plain word such as `trust`, `tension` or
/// `values` is prose: "need-to-know", "trust:" and "Tension:" pass. A small deterministic guard on identifiers, never on
/// meaning.
pub(crate) fn field_names(narration: &str, ids: &[String]) -> Vec<String> {
    let known = identifiers();
    let mut found = BTreeSet::new();
    for token in narration.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).map(|token| token.trim_matches('-')).filter(|token| !token.is_empty()) {
        if known.contains(&folded(token)) && !token.eq_ignore_ascii_case(KEBAB_ENGLISH) {
            found.insert(token.to_owned());
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
