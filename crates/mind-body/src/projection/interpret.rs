//! The Interpreter's speech (map section 4, cut C3b): what she says aloud, decided from her prose by one tool-less
//! structured model call, and held by code to her prose.
//!
//! The Persona writes her thought; she does not mark speech. The Interpreter reads her prose against the stimulus and
//! proposes zero or more lines, each with its room, its explicit addressees (Matrix mentions and a reply target, as
//! structured fields, never `@` text) and the verbatim span of her prose the line rests on. Code then keeps a line only
//! when:
//! - its span is in her prose, located by Ghostlight's `SourceSpan::locate` (map 4.3: a quote not in the prose word for
//!   word binds nothing);
//! - its room is one she may speak in this turn;
//! - its words are not someone else's: a line whose text is what she heard is a recollection, never her speech;
//! - its mentions and reply target name people the packet showed her; any other is dropped, and the drop recorded.
//!
//! Everything refused is reported with its reason. Nothing here posts: the commit (C3c) will.

use ghostlight_persona_projection::{InterpreterPrompt, SourceSpan, build_interpreter_prompt};
use serde::Deserialize;
use serde_json::{Value, json};

use super::narrate::{Model, lived_stream};
use super::select::Packet;
use super::tuning::Tuning;

/// The contract the model answers in: one JSON object, nothing around it.
pub(crate) const SPEECH_CONTRACT: &str = r#"{"say": [{"room": "<a room from the rooms listed>", "text": "<the words she types, in her voice, no @handles>", "mentions": ["<Matrix id of someone listed whom she addresses directly>"], "reply_to": "<Matrix id of the person whose message she answers, or null>", "source_quote": "<the exact words of her prose this line rests on, copied character for character>"}], "gaps": [{"kind": "ambiguity | missing_reference | unresolved", "source_quote": "<exact words of her prose>", "detail": "<why>"}]}"#;

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    say: Vec<Proposed>,
    #[serde(default)]
    gaps: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct Proposed {
    #[serde(default)]
    room: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    mentions: Vec<String>,
    #[serde(default)]
    reply_to: Option<String>,
    #[serde(default)]
    source_quote: String,
}

/// What the Interpreter is told beside Ghostlight's membrane: this step's scope, its rules, and that it has no tools.
fn guidance(tuning: &Tuning) -> String {
    tuning.interpreter.guidance.trim().to_owned()
}

/// The typed context the Interpreter may act on: the rooms she may speak in and the people she may address, by id.
fn typed_context(packet: &Packet) -> String {
    let mut out = format!("Rooms she may speak in: {}\n", packet.rooms.join("; "));
    out.push_str("People she may address, as label \u{2014} Matrix id:\n");
    for (label, id) in &packet.people {
        out.push_str(&format!("- {label} \u{2014} {id}\n"));
    }
    out.push_str(&format!("\nWhat reached her this turn:\n{}", packet.stimulus));
    out
}

pub(crate) fn speech_prompt(packet: &Packet, narration: &str, prose: &str, tuning: &Tuning) -> String {
    build_interpreter_prompt(&InterpreterPrompt {
        identity: &packet.identity,
        typed_context: &typed_context(packet),
        lived_stream: &lived_stream(narration, &packet.heard),
        persona_output: prose,
        output_schema: Some(SPEECH_CONTRACT),
        domain_guidance: &guidance(tuning),
    })
}

/// The model's answer as JSON: the object itself, or inside a fenced block.
fn json_of(text: &str) -> Option<Answer> {
    let trimmed = text.trim();
    let body = trimmed.strip_prefix("```json").or_else(|| trimmed.strip_prefix("```")).map_or(trimmed, |inner| inner.trim_end_matches("```")).trim();
    let start = body.find('{')?;
    let end = body.rfind('}')?;
    serde_json::from_str(&body[start..=end]).ok()
}

/// Whitespace-folded, lowercased, outer quotation marks off: how two texts are compared for being the same words.
fn folded(text: &str) -> String {
    text.trim().trim_matches(|c: char| matches!(c, '"' | '\'' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}')).split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Whether a line is someone else's words: its text, folded, is inside what she heard. A narrow guard on a recollection
/// posted as speech, not a reading of meaning: the model decides what she means to say.
fn recollection(text: &str, heard: &str) -> bool {
    let line = folded(text);
    !line.is_empty() && folded(heard).contains(&line)
}

/// One speech decision: the Interpreter's call, then code's hold on each line. The report carries the said lines, the
/// refused ones with their reasons, the model's gaps and the call's usage; a failed call or an answer that is not the
/// contract is a failure, never a silence.
pub(crate) fn decide(model: &dyn Model, request_id: &str, packet: &Packet, narration: &str, prose: &str, tuning: &Tuning) -> Value {
    let answer = match model.ask(request_id, &tuning.interpreter.model, &speech_prompt(packet, narration, prose, tuning)) {
        Ok(answer) => answer,
        Err(failed) => return json!({ "failed": failed.reason, "usage": failed.usage }),
    };
    let Some(parsed) = json_of(&answer.text) else {
        return json!({ "failed": "the answer is not the speech contract's JSON", "raw": answer.text, "usage": answer.usage });
    };
    let known = |id: &str| packet.people.iter().any(|(_, person)| person == id);
    let mut said = Vec::new();
    let mut refused = Vec::new();
    for line in parsed.say {
        let text = line.text.trim().to_owned();
        let why = if text.is_empty() {
            Some("no words")
        } else if !packet.rooms.contains(&line.room) {
            Some("not a room she may speak in this turn")
        } else if recollection(&text, &packet.heard) {
            Some("someone else's words: a recollection, not her speech")
        } else {
            None
        };
        let span = SourceSpan::locate(prose, line.source_quote.trim());
        let why = why.or_else(|| span.is_none().then_some("its source quote is not in her prose word for word"));
        if let Some(why) = why {
            refused.push(json!({ "text": text, "room": line.room, "source_quote": line.source_quote, "why": why }));
            continue;
        }
        let span = span.unwrap_or_else(|| SourceSpan::whole(prose));
        let (mentions, unknown): (Vec<String>, Vec<String>) = line.mentions.into_iter().partition(|id| known(id));
        let (reply_to, unknown_reply) = match line.reply_to.filter(|id| !id.trim().is_empty()) {
            Some(id) if known(&id) => (Some(id), None),
            Some(id) => (None, Some(id)),
            None => (None, None),
        };
        said.push(json!({
            "room": line.room,
            "text": text,
            "mentions": mentions,
            "reply_to": reply_to,
            "rests_on": { "quote": span.verbatim(), "start_byte": span.start_byte(), "end_byte": span.end_byte() },
            "dropped": { "mentions": unknown, "reply_to": unknown_reply },
        }));
    }
    json!({ "said": said, "refused": refused, "gaps": parsed.gaps, "usage": answer.usage })
}

#[cfg(test)]
mod tests;
