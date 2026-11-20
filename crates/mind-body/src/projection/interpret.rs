//! The Interpreter's speech (map section 4, cut C3b): what she says aloud this turn, decided from her prose by one
//! tool-less structured model call, and built by code from her own words.
//!
//! The Persona writes her thought, and when she means to say something she writes the words she would type. The
//! Interpreter decides whether she says them now: it answers with zero or more lines, each a list of spans of her prose
//! cited verbatim, with the people she addresses (Matrix mentions and a reply target, as structured fields). Code builds
//! each line, never the model: the line is its spans, located in her prose in order by Ghostlight's
//! `SourceSpan::locate`, joined with one space. So a line is always her own words verbatim. What this gives up: indirect
//! speech ("I'll tell her I'm fine" says nothing) and any paraphrase by the Interpreter.
//!
//! Code keeps a line only when every span locates in her prose, in order; drops a line that is mostly one long run of
//! the words she heard (an echo, measured on message bodies alone); keeps at most `MAX_LINES` distinct lines; and keeps
//! a mention or a reply target only for someone this packet showed her (`Packet::people`). She speaks only in the turn's
//! room. Every refusal and drop is reported. Nothing here posts: the commit (C3c) will.

use ghostlight_persona_projection::SourceSpan;
use serde::Deserialize;
use serde_json::{Value, json};

use super::narrate::{Model, heard_overlap, lived_stream};
use super::select::Packet;
use super::tuning::Tuning;

/// The most lines she says in one turn.
pub(crate) const MAX_LINES: usize = 2;
/// A line of at least this many words whose word trigrams are mostly what she heard is an echo, not her speech.
const ECHO_MIN_WORDS: usize = 4;
const ECHO_OVERLAP: f64 = 0.5;

/// What to say, where, to whom, and the gaps: Huginn's chat Interpreter, beside nothing of Ghostlight's but its span rule.
const SPEECH_PROMPT: &str = "You decide what one person says aloud in a chat room this turn, from her own prose. You never write her words: you cite them.

Below are who she is, the room and who is in it, what reached her, her lived moment, and her prose. Decide which words of her prose she chooses to say aloud now, in this room. A line is speech only when her prose shows her choosing to say it now. Wondering whether to speak, deciding to wait or stay quiet, or saying that she says nothing, is not speech. Words she recalls or quotes from someone else are never her speech.

She speaks only in this room. In a room that is not a direct message, she does not say what she knows privately about someone: what the moment marks as known privately stays out of what she says to the room. Whether a line fits who is listening is yours to judge.

Answer with one JSON object and nothing else:
{\"say\": [{\"spans\": [\"<exact words of her prose, copied character for character>\"], \"mentions\": [\"<Matrix id of someone listed whom she addresses directly>\"], \"reply_to\": \"<Matrix id of the person whose message she answers, or null>\"}], \"gaps\": [{\"kind\": \"conditional | addressee | elsewhere\", \"quote\": \"<exact words of her prose>\", \"detail\": \"<why>\"}]}

A line is its spans joined in order: cite exactly the words she would type, one span or a few in the order they appear, nothing she only describes. At most two lines. Record a gap instead of guessing: conditional when she might say something but has not chosen to, addressee when she means to say it but whom to is unclear, elsewhere when she means it for another room or another time. An empty say list is the usual answer in a busy room.";

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
    spans: Vec<String>,
    #[serde(default)]
    mentions: Vec<String>,
    #[serde(default)]
    reply_to: Option<String>,
}

pub(crate) fn speech_prompt(packet: &Packet, narration: &str, prose: &str) -> String {
    let mut people = String::new();
    for (label, id) in &packet.people {
        people.push_str(&format!("- {label} \u{2014} {id}\n"));
    }
    if people.is_empty() {
        people.push_str("- no one else has spoken here lately\n");
    }
    format!(
        "{SPEECH_PROMPT}\n\nWho she is:\n{}\n\nThe room: {} ({}).\nWho is in it, as label \u{2014} Matrix id:\n{people}\nWhat reached her:\n{}\n\nHer lived moment:\n{}\n\nHer prose:\n{prose}",
        packet.identity,
        packet.room.as_deref().unwrap_or("none"),
        packet.room_kind.words(),
        packet.stimulus,
        lived_stream(narration, &packet.heard),
    )
}

/// The model's answer as JSON: the object itself, or inside a fenced block.
fn json_of(text: &str) -> Option<Answer> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(&text[start..=end]).ok()
}

/// The line her cited spans make: each located in her prose after the one before it, joined with one space; or why not.
fn built(prose: &str, spans: &[String]) -> Result<(String, Vec<Value>), &'static str> {
    let mut from = 0;
    let mut words = Vec::new();
    let mut located = Vec::new();
    for span in spans.iter().map(|span| span.trim()).filter(|span| !span.is_empty()) {
        let Some(found) = prose.get(from..).and_then(|rest| SourceSpan::locate(rest, span)) else {
            return Err("a span is not in her prose, word for word and in order");
        };
        let (start, end) = (from + found.start_byte(), from + found.end_byte());
        located.push(json!({ "quote": found.verbatim(), "start_byte": start, "end_byte": end }));
        words.push(found.verbatim().to_owned());
        from = end;
    }
    if words.is_empty() {
        return Err("no words");
    }
    Ok((words.join(" "), located))
}

/// Whether a line is someone else's words: long enough to judge, and mostly a run of what she heard, by word trigrams
/// against the message bodies alone (`heard_overlap`). Short answers such as "ok" or "Hi" are never echoes.
fn echo(line: &str, bodies: &[String]) -> bool {
    line.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|word| !word.is_empty()).count() >= ECHO_MIN_WORDS && heard_overlap(line, &bodies.join("\n")) >= ECHO_OVERLAP
}

/// One speech decision: the Interpreter's call, then code's hold on each line. The report carries the lines said, the
/// lines refused with why, the mentions dropped, the gaps and the call's usage. A failed call or an answer that is not
/// the contract is a failure, never a silence.
pub(crate) fn decide(model: &dyn Model, request_id: &str, packet: &Packet, narration: &str, prose: &str, tuning: &Tuning) -> Value {
    let answer = match model.ask(request_id, &tuning.interpreter.model, &speech_prompt(packet, narration, prose)) {
        Ok(answer) => answer,
        Err(failed) => return json!({ "failed": failed.reason, "usage": failed.usage }),
    };
    let Some(parsed) = json_of(&answer.text) else {
        return json!({ "failed": "the answer is not the speech contract's JSON", "raw": answer.text, "usage": answer.usage });
    };
    let Some(room) = packet.room.clone() else {
        return json!({ "said": [], "refused": [{ "why": "the turn has no room to speak in" }], "gaps": parsed.gaps, "usage": answer.usage });
    };
    let shown = |id: &str| packet.people.iter().any(|(_, person)| person == id);
    let mut said: Vec<Value> = Vec::new();
    let mut refused = Vec::new();
    for line in parsed.say {
        let (text, spans) = match built(prose, &line.spans) {
            Ok(built) => built,
            Err(why) => {
                refused.push(json!({ "spans": line.spans, "why": why }));
                continue;
            }
        };
        let why = if echo(&text, &packet.bodies) {
            Some("mostly words she heard: an echo, not her speech")
        } else if said.iter().any(|kept| kept["text"] == text.as_str()) {
            Some("the same line again")
        } else if said.len() >= MAX_LINES {
            Some("past the lines one turn may say")
        } else {
            None
        };
        if let Some(why) = why {
            refused.push(json!({ "text": text, "why": why }));
            continue;
        }
        let (mentions, unknown): (Vec<String>, Vec<String>) = line.mentions.into_iter().partition(|id| shown(id));
        let (reply_to, unknown_reply) = match line.reply_to.filter(|id| !id.trim().is_empty()) {
            Some(id) if shown(&id) => (Some(id), None),
            Some(id) => (None, Some(id)),
            None => (None, None),
        };
        said.push(json!({ "room": room, "text": text, "mentions": mentions, "reply_to": reply_to, "rests_on": spans, "dropped": { "mentions": unknown, "reply_to": unknown_reply } }));
    }
    json!({ "room": room, "room_kind": packet.room_kind, "said": said, "refused": refused, "gaps": parsed.gaps, "usage": answer.usage })
}

#[cfg(test)]
mod tests;
