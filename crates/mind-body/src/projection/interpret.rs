//! The Interpreter's speech (map section 4, cut C3b): what she says aloud this turn, decided from her prose by one
//! tool-less structured model call, and built by code from her own words.
//!
//! The Persona writes her thought, and when she means to say something she writes the words she would type. The
//! Interpreter decides whether she says them now: it answers with zero or more lines, each a list of spans of her prose
//! cited verbatim, with the people she addresses (Matrix mentions and a reply target, as structured fields). Code builds
//! each line, never the model: each span is located in her prose by Ghostlight's `SourceSpan::locate` and must be whole
//! sentences (UAX #29 sentence segmentation, the Unicode family `locate`'s word boundaries come from), and the spans of
//! one line must be consecutive sentences; the line is that one unbroken run of her prose. A quotation's inside is
//! segmented as a text of its own, so a quoted sentence is whole, but only a standalone quotation's: one that is itself
//! whole sentences of her prose. A line may not start or end inside a quotation her sentence frames ("I won't say
//! \"Nibu is lying.\"", "If she pushes, I'll say \"Fine.\""): that is what she would or would not say. An opening
//! mark a budget left unclosed is not part of the sentence it starts. So a line is always whole sentences of her own,
//! verbatim and in their order: no stitching, no dropped negation, no condition lifted off its clause. What this gives
//! up: indirect speech ("I'll tell her I'm fine" says nothing), any paraphrase by the Interpreter, half a sentence, and
//! a line quoted inside a sentence of narration ("\"Fine,\" I tell her."): she says a sentence whole or not at all.
//!
//! Code keeps a line only when it is built so; drops an echo (at least `ECHO_MIN_WORDS` words, at least `ECHO_OVERLAP`
//! of its word trigrams in the bodies the stimulus quoted to her); keeps at most `MAX_LINES` distinct lines; and keeps a
//! mention or a reply target only for someone this packet showed her (`Packet::people`). She speaks only in the turn's
//! room. Whether a line tells what she knows privately is a reading of its meaning, so it is the Interpreter's
//! judgement over the private items the packet lists to it (`Packet::private`), never a word match here. Every refusal
//! and drop is reported. Nothing here posts: the commit (C3c) will.

use ghostlight_persona_projection::SourceSpan;
use serde::Deserialize;
use serde_json::{Value, json};
use unicode_segmentation::UnicodeSegmentation;

use super::narrate::{Model, heard_overlap, lived_stream};
use super::select::Packet;
use super::tuning::Tuning;

/// The most lines she says in one turn.
pub(crate) const MAX_LINES: usize = 2;
/// A line of at least this many words whose word trigrams are at least this share of what she heard is an echo, not
/// her speech.
const ECHO_MIN_WORDS: usize = 8;
const ECHO_OVERLAP: f64 = 0.8;

/// What to say, where, to whom, and the gaps: Huginn's chat Interpreter, beside nothing of Ghostlight's but its span rule.
const SPEECH_PROMPT: &str = "You decide what one person says aloud in a chat room this turn, from her own prose. You never write her words: you cite them.

Below are who she is, the room and who is in it, what reached her, her lived moment, and her prose. Decide which words of her prose she chooses to say aloud now, in this room. A line is speech only when her prose shows her choosing to say it now. Wondering whether to speak, deciding to wait or stay quiet, or saying that she says nothing, is not speech. Words she recalls or quotes from someone else are never her speech.

She speaks only in this room. What she knows privately is listed below as private: she says none of it here, and nothing that gives it away. A private matter may be said plainly only to the person it concerns, in a direct message with exactly that person; nothing listed is that here. Whether a line fits who is listening is yours to judge.

Answer with one JSON object and nothing else:
{\"say\": [{\"spans\": [\"<whole sentences of her prose, copied character for character>\"], \"mentions\": [\"<Matrix id of someone listed whom she addresses directly>\"], \"reply_to\": \"<Matrix id of the person whose message she answers, or null>\"}], \"gaps\": [{\"kind\": \"conditional | addressee | elsewhere\", \"quote\": \"<exact words of her prose>\", \"detail\": \"<why>\"}]}

A line is whole sentences of her prose, the words she would type and nothing she only describes: each span is one sentence or several, from its first character to its last, and the spans of one line are consecutive sentences. A quotation that stands as sentences of its own may be cited sentence by sentence; words quoted inside a sentence of narration are cited only with that whole sentence, and then only if that sentence is what she says. She never says half a sentence; if only part of a sentence is speech, she says nothing of it. At most two lines. Record a gap instead of guessing: conditional when she might say something but has not chosen to, addressee when she means to say it but whom to is unclear, elsewhere when she means it for another room or another time. An empty say list is the usual answer in a busy room.";

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
    let mut private: String = packet.private.iter().map(|words| format!("- private: {words}\n")).collect();
    if private.is_empty() {
        private.push_str("- none\n");
    }
    format!(
        "{SPEECH_PROMPT}\n\nWho she is:\n{}\n\nThe room: {} ({}).\nWho is in it, as label \u{2014} Matrix id:\n{people}\nWhat she knows privately, none of it to be said here:\n{private}\nWhat reached her:\n{}\n\nHer lived moment:\n{}\n\nHer prose:\n{prose}",
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

/// A quotation in her prose: where its opening mark starts, its inside, and where its closing mark ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Quotation {
    open: usize,
    inside: (usize, usize),
    close: usize,
}

/// Her quotations, paired in order: a straight double quote with the next one, a curly opening mark with the next curly
/// closing one. And the opening mark left unclosed, if one is (a budget cut her prose inside a quotation), with its
/// length.
fn quotations(prose: &str) -> (Vec<Quotation>, Option<(usize, usize)>) {
    let mut out = Vec::new();
    let mut open: Option<(usize, usize, char)> = None;
    for (at, c) in prose.char_indices() {
        match open {
            None if c == '"' => open = Some((at, c.len_utf8(), '"')),
            None if c == '\u{201C}' => open = Some((at, c.len_utf8(), '\u{201D}')),
            Some((mark, len, close)) if c == close => {
                out.push(Quotation { open: mark, inside: (mark + len, at), close: at + c.len_utf8() });
                open = None;
            }
            _ => {}
        }
    }
    (out, open.map(|(mark, len, _)| (mark, len)))
}

/// Byte ranges of `text`'s sentences (UAX #29), offset by `offset`, without the space around them.
fn segment(offset: usize, text: &str) -> Vec<(usize, usize)> {
    text.split_sentence_bound_indices()
        .filter_map(|(at, sentence)| {
            let start = at + sentence.len() - sentence.trim_start().len();
            let end = at + sentence.trim_end().len();
            (start < end).then_some((offset + start, offset + end))
        })
        .collect()
}

/// Her prose read for speech: its sentences in lists, her prose's own first, then each quotation's inside segmented as
/// a text of its own; and the quotations her sentences frame. A quotation is standalone when it, marks included, is
/// exactly a run of whole sentences of her prose (closing punctuation after its mark allowed); any other is framed. "I
/// won't say "Nibu is lying."" frames what she would or would not say, never what she says. An opening mark left
/// unclosed at a sentence's start is not part of that sentence.
struct Reading {
    lists: Vec<Vec<(usize, usize)>>,
    framed: Vec<Quotation>,
}

fn read(prose: &str) -> Reading {
    let (quotes, unclosed) = quotations(prose);
    let mut top = segment(0, prose);
    if let Some((mark, len)) = unclosed
        && let Some(sentence) = top.iter_mut().find(|sentence| sentence.0 == mark)
    {
        let rest = &prose[mark + len..sentence.1];
        let start = sentence.1 - rest.trim_start().len();
        if start < sentence.1 {
            sentence.0 = start;
        }
    }
    let standalone = |quote: &Quotation| {
        let opens = top.iter().position(|sentence| sentence.0 == quote.open);
        let closes = top.iter().position(|sentence| sentence.0 < quote.close && sentence.1 >= quote.close && prose[quote.close..sentence.1].chars().all(|c| c.is_ascii_punctuation()));
        matches!((opens, closes), (Some(opens), Some(closes)) if opens <= closes)
    };
    let framed = quotes.iter().copied().filter(|quote| !standalone(quote)).collect();
    let mut lists = vec![top];
    lists.extend(quotes.iter().map(|quote| segment(quote.inside.0, &prose[quote.inside.0..quote.inside.1])));
    Reading { lists, framed }
}

/// The line her cited spans make, or why not. Each span is located in her prose after the one before it and must be
/// whole sentences of one list (`read`), each span's first sentence the one after the last span's last; the line is her
/// prose from the first sentence to the last, verbatim, and it may neither start nor end inside a framed quotation. A
/// line that holds a framed quotation whole ("Kiko's "plan" is a mess.") is hers to say.
fn built(prose: &str, spans: &[String]) -> Result<(String, Vec<Value>), &'static str> {
    let Reading { lists, framed } = read(prose);
    let mut from = 0;
    // The list the line is in, and its last sentence so far.
    let mut run: Option<(usize, usize)> = None;
    let mut first = None;
    let mut located = Vec::new();
    for span in spans.iter().map(|span| span.trim()).filter(|span| !span.is_empty()) {
        let Some(found) = prose.get(from..).and_then(|rest| SourceSpan::locate(rest, span)) else {
            return Err("a span is not in her prose, word for word and in order");
        };
        let (start, end) = (from + found.start_byte(), from + found.end_byte());
        let whole: Vec<(usize, usize, usize)> = lists
            .iter()
            .enumerate()
            .filter_map(|(list, sentences)| {
                let opens = sentences.iter().position(|sentence| sentence.0 == start)?;
                let closes = sentences.iter().position(|sentence| sentence.1 == end)?;
                (opens <= closes).then_some((list, opens, closes))
            })
            .collect();
        if whole.is_empty() {
            return Err("a span is not whole sentences of her prose: she says a sentence whole or not at all");
        }
        let Some(&(list, _, closes)) = whole.iter().find(|(list, opens, _)| run.is_none_or(|(in_list, last)| in_list == *list && last + 1 == *opens)) else {
            return Err("the spans are not consecutive sentences of her prose");
        };
        run = Some((list, closes));
        first.get_or_insert(start);
        located.push(json!({ "quote": found.verbatim(), "start_byte": start, "end_byte": end }));
        from = end;
    }
    let Some(first) = first else {
        return Err("no words");
    };
    let inside = |at: usize| framed.iter().any(|quote| quote.open < at && at < quote.close);
    if inside(first) || inside(from) {
        return Err("it starts or ends inside a quotation her sentence frames: words she would or would not say, not words she says");
    }
    Ok((prose[first..from].to_owned(), located))
}

/// Whether a line is someone else's words: long enough to judge, and nearly all a run of what she heard, by word
/// trigrams against the bodies the stimulus quoted (`heard_overlap`). Short answers such as "ok" or "Hi" are never
/// echoes, and a line that answers a claim by naming it is not one.
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
