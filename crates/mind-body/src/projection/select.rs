//! The Select stage: what she can feel this turn. Code, deterministic for a seed, and the one owner of the
//! projection packet; the Projector model only renders what this chose.
//!
//! The Projector shows her self and her room: her persona state and what the mouth showed. Her Mind is not read
//! here; reaching into it is the Interpreter's, through its read tools, when wondering leads to reading.
//!
//! Every packet carries its fixed slots: identity, the hour, her mood as change, a bond line for each person
//! present, and the stimulus. The rest comes from her state's pools. Each candidate is ordered by
//! `log(score) / T` plus Gumbel noise and included with probability `1 - exp(-score / inclusion_score)`, where
//! `score = salience × recency × touch × relevance × unresolved × freshness`; then each pool's cap and the context
//! budget bound what is kept. A cap is a maximum, never a target: a pool with fewer candidates than its cap still
//! rotates. Recency decays only the short-term items: a crystallized memory, a value or a stance is as near as it ever
//! was, and comes forward by what the room is touching (`relevance`, a lexical overlap with the stimulus).
//!
//! Who a message is to is `address::is_for`'s, the rule the heartbeat woke her by, and nothing else here. People are
//! keyed by their full Matrix id; only the operator and the Minds of the swarm are keyed by the names their bonds use,
//! and an unbonded speaker is shown by the display name the mouth reports, quoted and escaped, with the Matrix id after
//! it, never under a bonded person's name. Every message body and display name is rendered by one escape (`spoken`, a
//! JSON string), so no text can forge a line; speaker and room come from the mouth's metadata alone.
//!
//! Never read: `agencyPressure`, `candidateActions`, `privateNotes` and `provenance`. A memory tagged with a direct
//! message room (`room:!…`) is selectable only in that room.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use super::tuning::{LensWeights, Tuning};
use crate::address::localpart;

/// Touch: what the room is touching.
const TOUCH_PRESENT: f64 = 1.0;
const TOUCH_ROOM: f64 = 0.6;
const TOUCH_ELSE: f64 = 0.3;
/// The salience of an item that carries none.
const SALIENCE_UNSTATED: f64 = 0.5;
/// A move of mood or need below this is no move.
const MOVE_MIN: f64 = 0.03;
/// Incubation's unresolved factor caps here.
const INCUBATION_UNRESOLVED_MAX: f64 = 2.0;
/// The most bytes a bond's name or kind takes in its line.
const NAME_BYTES: usize = 80;
/// The most bytes the line naming the unbonded people present takes.
const ALSO_HERE_BYTES: usize = 300;

/// What kind of turn the packet is for. The stimulus and nothing else differs between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    Chat,
    Beat,
    FollowUp,
    Reflection,
}

/// One line of a room: a message or a line of its tail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoomLine {
    pub room: String,
    pub sender: String,
    pub display: String,
    pub text: String,
    pub at_ms: u64,
    pub to_her: bool,
}

/// The room as the mouth showed it: new messages, and the recent tail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Room {
    pub messages: Vec<RoomLine>,
    pub tail: Vec<RoomLine>,
    /// The room the turn is in, when it is one room.
    pub here: Option<String>,
}

/// One thing she said, from the trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Said {
    pub id: String,
    pub room: String,
    pub text: String,
    pub at: DateTime<Utc>,
}

/// What earlier turns left: the ids each of the last projections selected, oldest first, what she said, and her state
/// at her last turn (movement is measured against it). The commit's trace v2 (C3c) is its source; until then it is
/// empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct History {
    pub recent: Vec<Vec<String>>,
    pub said: Vec<Said>,
    pub previous: Option<Value>,
}

/// Who the people of a room are to her.
#[derive(Clone, Copy)]
pub(crate) struct People<'a> {
    pub operators: &'a [String],
    /// The Matrix ids of the swarm's Minds: their bonds name them by localpart.
    pub minds: &'a [String],
    pub own_sender: &'a str,
}

impl People<'_> {
    /// The id a bond names a person by: `operator` for the operator, a Mind's localpart for a Mind of the swarm, and the
    /// full Matrix id for anyone else, so `@libby:evil.example` is never Libby.
    pub(crate) fn person_of(&self, sender: &str) -> String {
        if self.operators.iter().any(|operator| operator == sender) {
            "operator".into()
        } else if self.minds.iter().any(|mind| mind == sender) {
            localpart(sender).to_lowercase()
        } else {
            sender.to_lowercase()
        }
    }
}

pub(crate) struct Inputs<'a> {
    pub kind: TurnKind,
    pub state: &'a Value,
    pub room: &'a Room,
    pub history: &'a History,
    /// A follow-up's finding.
    pub observation: Option<&'a str>,
    pub people: People<'a>,
    pub now: DateTime<Utc>,
    pub seed: u64,
}

/// Where a turn begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Lens {
    BodyAndHour,
    PersonPresent(String),
    UnresolvedWant,
    MemorySurfacing,
    Wondering,
}

impl Lens {
    /// The lens as the domain guidance's "begin from …".
    pub(crate) fn words(&self) -> String {
        match self {
            Self::BodyAndHour => "how she is, here, at this hour".into(),
            Self::PersonPresent(name) => format!("{name}, who is here"),
            Self::UnresolvedWant => "something she still wants and has not got".into(),
            Self::MemorySurfacing => "a memory coming back to her".into(),
            Self::Wondering => "something she has been wondering about".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Pool {
    MemoriesAboutPresent,
    OtherMemories,
    Needs,
    Incubation,
    Stance,
    Value,
    Said,
}

/// One selected item: its id goes to the trace, never to the narration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Pick {
    pub id: String,
    pub pool: Pool,
    /// `ln(score)`.
    pub log_score: f64,
}

/// The projection packet: what the Projector is shown, and what chose it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Packet {
    pub kind: TurnKind,
    pub seed: u64,
    pub lens: Lens,
    pub identity: String,
    pub context: String,
    pub stimulus: String,
    /// What was said to her, verbatim, rendered by code for the Persona stage.
    pub heard: String,
    /// Bytes the messages to her took past the stimulus and heard caps: verbatim beats the cap.
    pub overflow: Overflow,
    pub present: Vec<String>,
    /// Every typed id the packet's lines name: the selected items' and the bonds' of the people present.
    pub ids: Vec<String>,
    pub selected: Vec<Pick>,
}

/// What the messages to her cost past the caps: bytes over the stimulus and heard caps, the older messages to her the
/// `addressed_bytes` ceiling left out, and the bytes clipped from a single message larger than the ceiling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Overflow {
    pub stimulus: usize,
    pub heard: usize,
    pub said_to_her_not_shown: usize,
    pub clipped_bytes: usize,
}

struct Candidate {
    id: String,
    pool: Pool,
    line: String,
    log_score: f64,
}

// ---------------------------------------------------------------------------------------------
// Reading her state.

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    path.iter().try_fold(value, |at, key| at.get(*key)).and_then(Value::as_str).unwrap_or_default().trim()
}

fn number(value: &Value, path: &[&str]) -> Option<f64> {
    path.iter().try_fold(value, |at, key| at.get(*key)).and_then(Value::as_f64)
}

fn items<'a>(value: &'a Value, path: &[&str]) -> &'a [Value] {
    path.iter().try_fold(value, |at, key| at.get(*key)).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn live(item: &Value) -> bool {
    !matches!(text(item, &["status"]), "retired" | "cooled" | "resolved")
}

fn time(item: &Value) -> Option<DateTime<Utc>> {
    ["updatedAt", "createdAt"].iter().find_map(|key| DateTime::parse_from_rfc3339(text(item, &[key])).ok()).map(|at| at.with_timezone(&Utc))
}

/// `text` cut to at most `max` bytes on a grapheme boundary, with an ellipsis where it was cut.
pub(crate) fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_owned();
    }
    let room = max.saturating_sub('…'.len_utf8());
    let end = text.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).take_while(|end| *end <= room).last().unwrap_or(0);
    format!("{}…", &text[..end])
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words of a text that can carry a topic: four letters or more, lowercased.
fn content_words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|word| word.chars().count() >= 4).map(str::to_lowercase).collect()
}

/// `ln(1 + min(1, shared / 3))`: how much of an item the room is touching, by the content words it shares with the
/// stimulus. Lexical and cheap; no embedder.
fn relevance_log(item_text: &str, touching: &BTreeSet<String>) -> f64 {
    let shared = content_words(item_text).intersection(touching).count();
    (1.0 + (shared as f64 / 3.0).min(1.0)).ln()
}

/// A message body or a display name as every line renders it: a JSON string, quotes, backslashes and every control
/// character escaped (`\n`, `\t`, `\r`, `\u…`, and the Unicode line separators too), so no text can end its line or
/// start another, and the escape is reversible: nothing is dropped.
pub(crate) fn spoken(text: &str) -> String {
    crate::persona::quoted(text)
}

// ---------------------------------------------------------------------------------------------
// Numbers as words.

fn trust_words(trust: f64) -> &'static str {
    match trust {
        t if t < 0.2 => "little trust",
        t if t < 0.45 => "guarded trust",
        t if t < 0.7 => "fair trust",
        t if t < 0.85 => "solid trust",
        _ => "deep trust",
    }
}

fn friction_words(tension: f64) -> &'static str {
    match tension {
        t if t < 0.05 => "easy between them",
        t if t < 0.25 => "a little friction",
        t if t < 0.5 => "real friction",
        _ => "heavy friction",
    }
}

fn strength_words(intensity: f64) -> &'static str {
    match intensity {
        i if i < 0.25 => "faint",
        i if i < 0.5 => "present",
        i if i < 0.75 => "strong",
        _ => "pressing",
    }
}

/// The three mood axes: name, the word for higher, the word for lower.
const MOOD_AXES: [(&str, &str, &str); 3] = [("valence", "lighter", "heavier"), ("arousal", "more keyed up", "more settled"), ("dominance", "more in command of things", "more at their mercy")];

fn activation(state: &Value, axis: &str) -> Option<f64> {
    number(state, &["activationProfile", "situationalState", axis, "currentActivation"])
}

/// Mood as change since her last turn; as a level only when there is no last turn to measure from.
fn mood_line(state: &Value, previous: Option<&Value>) -> String {
    let now: Vec<(usize, f64)> = MOOD_AXES.iter().enumerate().filter_map(|(at, (axis, ..))| activation(state, axis).map(|value| (at, value))).collect();
    if now.is_empty() {
        return "- mood: none recorded".into();
    }
    let Some(previous) = previous else {
        let words: Vec<&str> = now
            .iter()
            .map(|(at, value)| {
                let (_, up, down) = MOOD_AXES[*at];
                if *value >= 0.5 { up } else { down }
            })
            .collect();
        return format!("- mood, with no earlier turn to measure from: {} than middling", words.join(", "));
    };
    let moves: Vec<&str> = now
        .iter()
        .filter_map(|(at, value)| {
            let (axis, up, down) = MOOD_AXES[*at];
            let before = activation(previous, axis)?;
            match value - before {
                change if change >= MOVE_MIN => Some(up),
                change if change <= -MOVE_MIN => Some(down),
                _ => None,
            }
        })
        .collect();
    if moves.is_empty() { "- mood: where it was at her last turn".into() } else { format!("- mood: {} than at her last turn", moves.join(", ")) }
}

fn need_movement(need: &Value, previous: Option<&Value>) -> String {
    let intensity = number(need, &["intensity"]).unwrap_or(SALIENCE_UNSTATED);
    let before = previous.and_then(|previous| items(previous, &["affect", "needs"]).iter().find(|old| text(old, &["id"]) == text(need, &["id"])).and_then(|old| number(old, &["intensity"])));
    match before.map(|before| intensity - before) {
        Some(change) if change >= MOVE_MIN => "growing since her last turn".into(),
        Some(change) if change <= -MOVE_MIN => "easing since her last turn".into(),
        Some(_) => "where it was".into(),
        None => strength_words(intensity).into(),
    }
}

// ---------------------------------------------------------------------------------------------
// The seeded draw: splitmix64, so a turn id replays.

struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1).
    fn unit(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn gumbel(&mut self) -> f64 {
        -(-self.unit().ln()).ln()
    }
}

fn draw_lens(weights: &LensWeights, present: &[String], candidates: &[Candidate], draw: &mut Draw) -> Lens {
    let has = |pools: &[Pool]| candidates.iter().any(|candidate| pools.contains(&candidate.pool));
    let options = [
        (weights.body_and_hour, Lens::BodyAndHour),
        (if present.is_empty() { 0.0 } else { weights.person_present }, Lens::PersonPresent(present.first().cloned().unwrap_or_default())),
        (if has(&[Pool::Needs, Pool::Incubation]) { weights.unresolved_want } else { 0.0 }, Lens::UnresolvedWant),
        (if has(&[Pool::MemoriesAboutPresent, Pool::OtherMemories]) { weights.memory_surfacing } else { 0.0 }, Lens::MemorySurfacing),
        (if has(&[Pool::Incubation]) { weights.wondering } else { 0.0 }, Lens::Wondering),
    ];
    let total: f64 = options.iter().map(|(weight, _)| weight.max(0.0)).sum();
    let mut at = draw.unit() * total;
    for (weight, lens) in &options {
        let weight = weight.max(0.0);
        if weight > 0.0 && at < weight {
            return lens.clone();
        }
        at -= weight;
    }
    Lens::BodyAndHour
}

// ---------------------------------------------------------------------------------------------
// Candidates.

/// `ln(base ^ k)`, k the times `id` was shown in the last `freshness_window` projections: 0 when never shown, minus
/// infinity when the base is 0 and it was shown at all.
fn freshness_log(id: &str, history: &History, tuning: &Tuning) -> f64 {
    let window = &history.recent[history.recent.len().saturating_sub(tuning.select.freshness_window)..];
    let shown = window.iter().filter(|ids| ids.iter().any(|seen| seen == id)).count();
    if shown == 0 { 0.0 } else { tuning.select.freshness_base.powi(i32::try_from(shown).unwrap_or(i32::MAX)).ln() }
}

fn recency_log(at: Option<DateTime<Utc>>, now: DateTime<Utc>, tau_h: f64) -> f64 {
    at.map_or(0.0, |at| -((now - at).num_seconds().max(0) as f64 / 3600.0) / tau_h)
}

/// The memory's tags name a direct-message room other than the turn's: private matter stays in its room.
fn private_elsewhere(item: &Value, here: Option<&str>) -> bool {
    items(item, &["tags"]).iter().filter_map(Value::as_str).filter_map(|tag| tag.strip_prefix("room:")).any(|room| room.starts_with('!') && Some(room) != here)
}

fn in_room(item: &Value, here: Option<&str>) -> bool {
    here.is_some_and(|here| items(item, &["tags"]).iter().filter_map(Value::as_str).any(|tag| tag.strip_prefix("room:") == Some(here)))
}

fn about_present(item: &Value, present: &BTreeSet<String>) -> bool {
    let target = text(item, &["target", "id"]).to_lowercase();
    present.contains(&target) || items(item, &["tags"]).iter().filter_map(Value::as_str).filter_map(|tag| tag.strip_prefix("about:")).any(|id| present.contains(&id.to_lowercase()))
}

fn candidates(inputs: &Inputs, tuning: &Tuning, present: &BTreeSet<String>, touching: &BTreeSet<String>) -> Vec<Candidate> {
    let state = inputs.state;
    let here = inputs.room.here.as_deref();
    let item_bytes = tuning.select.item_bytes;
    let mut out = Vec::new();
    let mut push = |id: &str, pool: Pool, line: String, salience: f64, recency: f64, touch: f64, unresolved: f64| {
        if id.is_empty() || salience <= 0.0 {
            return;
        }
        let log_score = salience.ln() + recency + touch.ln() + unresolved.ln() + freshness_log(id, inputs.history, tuning);
        out.push(Candidate { id: id.to_owned(), pool, line, log_score });
    };

    let crystallized = items(state, &["thoughtMemory", "memories"]).iter().map(|memory| (memory, false));
    let memories = crystallized.chain(items(state, &["thoughtMemory", "shortTerm"]).iter().map(|memory| (memory, true)));
    for (memory, short_term) in memories.filter(|(memory, _)| live(memory) && !private_elsewhere(memory, here)) {
        let about = about_present(memory, present);
        let pool = if about { Pool::MemoriesAboutPresent } else { Pool::OtherMemories };
        let touch = if about { TOUCH_PRESENT } else if in_room(memory, here) { TOUCH_ROOM } else { TOUCH_ELSE };
        let mut body = text(memory, &["summary"]).to_owned();
        let tension = text(memory, &["tension"]);
        if !tension.is_empty() {
            body = format!("{body} {tension}");
        }
        let line = format!("- memory [{}]: {}", text(memory, &["id"]), clip(&one_line(&body), item_bytes));
        let salience = number(memory, &["intensity"]).unwrap_or(SALIENCE_UNSTATED);
        // Only the day's short-term items fade; a crystallized memory comes forward by what the room touches.
        let nearness = if short_term { recency_log(time(memory), inputs.now, tuning.select.memory_tau_h) } else { relevance_log(&body, touching) };
        push(text(memory, &["id"]), pool, line, salience, nearness, touch, 1.0);
    }

    for need in items(state, &["affect", "needs"]).iter().filter(|need| live(need)) {
        let intensity = number(need, &["intensity"]).unwrap_or(SALIENCE_UNSTATED);
        let name = [text(need, &["label"]), text(need, &["summary"]), text(need, &["id"])].into_iter().find(|name| !name.is_empty()).unwrap_or_default();
        let line = format!("- need [{}]: {}, {}", text(need, &["id"]), clip(&one_line(name), item_bytes), need_movement(need, inputs.history.previous.as_ref()));
        push(text(need, &["id"]), Pool::Needs, line, intensity, 0.0, TOUCH_ELSE, 1.0 + intensity);
    }

    for wondering in items(state, &["thoughtMemory", "incubation"]).iter().filter(|item| live(item) && !private_elsewhere(item, here)) {
        let days = time(wondering).map_or(0.0, |at| (inputs.now - at).num_seconds().max(0) as f64 / 86_400.0);
        let touch = if about_present(wondering, present) { TOUCH_PRESENT } else { TOUCH_ELSE };
        let line = format!("- wondering [{}]: {}", text(wondering, &["id"]), clip(&one_line(text(wondering, &["summary"])), item_bytes));
        push(text(wondering, &["id"]), Pool::Incubation, line, number(wondering, &["intensity"]).unwrap_or(SALIENCE_UNSTATED), 0.0, touch, (1.0 + days / 7.0).min(INCUBATION_UNRESOLVED_MAX));
    }

    for stance in items(state, &["affect", "doctrineStances"]).iter().filter(|stance| live(stance)) {
        let touch = if about_present(stance, present) { TOUCH_PRESENT } else { TOUCH_ELSE };
        let line = format!("- stance [{}]: {}", text(stance, &["id"]), clip(&one_line(text(stance, &["summary"])), item_bytes));
        push(text(stance, &["id"]), Pool::Stance, line, number(stance, &["intensity"]).unwrap_or(SALIENCE_UNSTATED), relevance_log(text(stance, &["summary"]), touching), touch, 1.0);
    }

    for value in items(state, &["values"]) {
        let line = format!("- value [{}] {}: {}", text(value, &["id"]), clip(text(value, &["label"]), NAME_BYTES), clip(&one_line(text(value, &["summary"])), item_bytes));
        let relevance = relevance_log(&format!("{} {}", text(value, &["label"]), text(value, &["summary"])), touching);
        push(text(value, &["id"]), Pool::Value, line, number(value, &["priority"]).unwrap_or(SALIENCE_UNSTATED), relevance, TOUCH_ELSE, 1.0);
    }

    for said in &inputs.history.said {
        let touch = if Some(said.room.as_str()) == here { TOUCH_ROOM } else { TOUCH_ELSE };
        let line = format!("- she said [{}] in {}: {}", said.id, said.room, spoken(&clip(&said.text, item_bytes)));
        push(&said.id, Pool::Said, line, SALIENCE_UNSTATED, recency_log(Some(said.at), inputs.now, tuning.select.memory_tau_h), touch, 1.0);
    }
    out
}

fn cap_of(pool: Pool, tuning: &Tuning) -> usize {
    let caps = &tuning.select.caps;
    match pool {
        Pool::MemoriesAboutPresent => caps.memories_about_present,
        Pool::OtherMemories => caps.other_memories,
        Pool::Needs => caps.needs,
        Pool::Incubation => caps.incubation,
        Pool::Stance => caps.stance,
        Pool::Value => caps.value,
        Pool::Said => caps.said,
    }
}

// ---------------------------------------------------------------------------------------------
// The stimulus, the people present and the fixed slots.

/// How a speaker reads: a bonded person by the bond's label; anyone else by their display name with the Matrix id beside
/// it, so a stranger called "Meta" never reads as the operator.
fn name_of(line: &RoomLine, inputs: &Inputs) -> String {
    let person = inputs.people.person_of(&line.sender);
    let bonded = items(inputs.state, &["affect", "socialBonds"]).iter().find(|bond| live(bond) && text(bond, &["object", "id"]).to_lowercase() == person).map(|bond| text(bond, &["object", "label"]).to_owned()).filter(|label| !label.is_empty());
    match bonded {
        Some(label) => clip(&one_line(&label), NAME_BYTES),
        // A display name is the speaker's own text: quoted and escaped, at most `NAME_BYTES`, the real id always after it.
        None if !line.display.trim().is_empty() => format!("{} ({})", spoken(&clip(line.display.trim(), NAME_BYTES)), line.sender),
        None => line.sender.clone(),
    }
}

/// Lines appended under a byte cap: a line that would pass it is refused whole.
struct Lines {
    out: String,
    cap: usize,
}

impl Lines {
    fn new(cap: usize) -> Self {
        Self { out: String::new(), cap }
    }

    fn fits(&self, line: &str) -> bool {
        self.out.len() + line.len() + usize::from(!self.out.is_empty()) <= self.cap
    }

    fn push(&mut self, line: &str) -> bool {
        if !self.fits(line) {
            return false;
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out.push_str(line);
        true
    }
}

/// A section of room lines under `cap` bytes: its heading, a note counting the older lines that did not fit, then the
/// newest whole lines that did. Empty when there are no lines, or not even the heading fits.
fn newest_section(heading: &str, lines: &[String], cap: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    // The note is at most this long: "(NNNNN earlier not shown)".
    let note_room = 32;
    let mut kept: Vec<&String> = Vec::new();
    let mut used = heading.len() + note_room;
    for line in lines.iter().rev() {
        if used + line.len() + 1 > cap {
            break;
        }
        used += line.len() + 1;
        kept.push(line);
    }
    kept.reverse();
    let dropped = lines.len() - kept.len();
    let mut out = Lines::new(cap);
    if !out.push(heading) {
        return String::new();
    }
    let mut shown = false;
    if dropped > 0 {
        shown |= out.push(&format!("({dropped} earlier not shown)"));
    }
    for line in kept {
        shown |= out.push(line);
    }
    // A heading over nothing says nothing.
    if shown { out.out } else { String::new() }
}

/// Which messages to her a turn shows: whole messages, newest first, until the `addressed_bytes` ceiling, and a single
/// message larger than the ceiling clipped on a grapheme boundary (`cut`, the bytes of its text kept). Decided once, on
/// the stimulus's lines, and rendered for the stimulus and the heard lines alike.
struct Addressed<'a> {
    /// Chronological: each message shown, and where its text is cut if it is.
    shown: Vec<(&'a RoomLine, Option<usize>)>,
    left_out: usize,
    clipped: usize,
}

impl Addressed<'_> {
    fn lines(&self, render: impl Fn(&RoomLine, &str) -> String) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        if self.left_out > 0 {
            lines.push(format!("({} more said to her, not shown)", self.left_out));
        }
        lines.extend(self.shown.iter().map(|(line, cut)| match cut {
            None => render(line, &spoken(&line.text)),
            Some(cut) => render(line, &format!("{} [… {} bytes more]", spoken(&line.text[..*cut]), line.text.len() - cut)),
        }));
        lines
    }
}

fn said_to_her<'a>(to_her: &[&'a RoomLine], render: impl Fn(&RoomLine, &str) -> String, ceiling: usize) -> Addressed<'a> {
    let mut shown: Vec<(&RoomLine, Option<usize>)> = Vec::new();
    let mut used = 0;
    let mut clipped = 0;
    for line in to_her.iter().rev() {
        let whole = render(line, &spoken(&line.text));
        if used + whole.len() < ceiling {
            used += whole.len() + 1;
            shown.push((line, None));
            continue;
        }
        if shown.is_empty() {
            // One message larger than the ceiling: as much of it as fits, cut between graphemes.
            let ends: Vec<usize> = line.text.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).collect();
            let fitting = |end: usize| render(line, &format!("{} [… {} bytes more]", spoken(&line.text[..end]), line.text.len() - end));
            let fit = ends.partition_point(|end| fitting(*end).len() < ceiling);
            let cut = if fit == 0 { 0 } else { ends[fit - 1] };
            clipped = line.text.len() - cut;
            shown.push((line, Some(cut)));
        }
        break;
    }
    let left_out = to_her.len() - shown.len();
    shown.reverse();
    Addressed { shown, left_out, clipped }
}

fn chat_stimulus(inputs: &Inputs, tuning: &Tuning) -> (String, String, Overflow) {
    let messages: Vec<&RoomLine> = inputs.room.messages.iter().filter(|line| line.sender != inputs.people.own_sender).collect();
    if messages.is_empty() {
        return ("Nothing new was said.".into(), String::new(), Overflow::default());
    }
    let (to_her, others): (Vec<&RoomLine>, Vec<&RoomLine>) = messages.into_iter().partition(|line| line.to_her);
    let typed = |line: &RoomLine, body: &str| format!("- {} in {}: {body}", name_of(line, inputs), line.room);
    // Heard lines are prose for the Persona: the speaker and the room from the mouth, then the escaped words.
    let heard = |line: &RoomLine, body: &str| format!("{}, in {}: {body}", name_of(line, inputs), line.room);
    let ceiling = tuning.select.addressed_bytes;
    // One selection, rendered twice: the stimulus and the heard lines show the same messages to her and the same count.
    let addressed = said_to_her(&to_her, typed, ceiling);
    let (left_out, clipped) = (addressed.left_out, addressed.clipped);
    let typed_to_her = addressed.lines(typed);
    let heard_to_her = addressed.lines(heard);

    let mut stimulus = String::new();
    if !typed_to_her.is_empty() {
        stimulus.push_str("Said to her:");
        for line in &typed_to_her {
            stimulus.push('\n');
            stimulus.push_str(line);
        }
    }
    let others_typed: Vec<String> = others.iter().map(|line| typed(line, &spoken(&line.text))).collect();
    let room_left = tuning.select.stimulus_bytes.saturating_sub(stimulus.len() + 1);
    let mut also = newest_section("Also in the room:", &others_typed, room_left);
    if also.is_empty() && !others_typed.is_empty() {
        // The messages to her took the room: the others are counted, never silently gone.
        also = format!("({} more in the room not shown)", others_typed.len());
    }
    if !also.is_empty() {
        if !stimulus.is_empty() {
            stimulus.push('\n');
        }
        stimulus.push_str(&also);
    }

    let mut heard_text = heard_to_her.join("\n");
    let others_heard: Vec<String> = others.iter().map(|line| heard(line, &spoken(&line.text))).collect();
    let heard_left = tuning.select.heard_bytes.saturating_sub(heard_text.len() + 1);
    let (kept, _) = newest_that_fit(&others_heard, heard_left);
    for line in kept {
        if !heard_text.is_empty() {
            heard_text.push('\n');
        }
        heard_text.push_str(&line);
    }
    let overflow = Overflow { stimulus: stimulus.len().saturating_sub(tuning.select.stimulus_bytes), heard: heard_text.len().saturating_sub(tuning.select.heard_bytes), said_to_her_not_shown: left_out, clipped_bytes: clipped };
    (stimulus, heard_text, overflow)
}

/// Whole lines, newest kept, that fit `budget` bytes with their newlines, and how many older ones did not.
fn newest_that_fit(lines: &[String], budget: usize) -> (Vec<String>, usize) {
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0;
    for line in lines.iter().rev() {
        if used + line.len() + 1 > budget {
            break;
        }
        used += line.len() + 1;
        kept.push(line.clone());
    }
    kept.reverse();
    let dropped = lines.len() - kept.len();
    (kept, dropped)
}

fn hours(now: DateTime<Utc>, at_ms: u64) -> i64 {
    let at = i64::try_from(at_ms).ok().and_then(DateTime::<Utc>::from_timestamp_millis).unwrap_or(now);
    (now - at).num_hours().max(0)
}

/// A beat: nothing addressed her; when she last heard from each person she is bound to (the relatedness cue, most
/// recently heard first, in at most two thirds of the cap, a note counting any left out); then the newest lines of her
/// rooms in what is left. Every line is refused whole that would pass the cap.
fn beat_stimulus(inputs: &Inputs, tuning: &Tuning) -> String {
    let cap = tuning.select.stimulus_bytes;
    let mut contacts: Vec<(Option<u64>, String)> = Vec::new();
    for bond in items(inputs.state, &["affect", "socialBonds"]).iter().filter(|bond| live(bond) && text(bond, &["object", "kind"]) == "person") {
        let person = text(bond, &["object", "id"]).to_lowercase();
        let label = clip(&one_line([text(bond, &["object", "label"]), text(bond, &["object", "id"])].into_iter().find(|name| !name.is_empty()).unwrap_or_default()), NAME_BYTES);
        let last = inputs.room.tail.iter().filter(|line| inputs.people.person_of(&line.sender) == person).map(|line| line.at_ms).max();
        contacts.push((last, match last {
            Some(at) => format!("- last heard from {label} {} h ago", hours(inputs.now, at)),
            None => format!("- nothing from {label} in her rooms lately"),
        }));
    }
    contacts.sort_by_key(|(last, _)| std::cmp::Reverse(*last));
    let mut out = Lines::new(cap);
    out.push("Nothing addressed her.");
    let note_room = 48;
    let contact_cap = cap * 2 / 3;
    let mut left_out = 0;
    for (_, line) in &contacts {
        if out.out.len() + line.len() + 1 + note_room > contact_cap || !out.push(line) {
            left_out += 1;
        }
    }
    if left_out > 0 {
        out.push(&format!("({left_out} more people she is bound to not shown)"));
    }
    let others: Vec<&RoomLine> = inputs.room.tail.iter().filter(|line| line.sender != inputs.people.own_sender).collect();
    let shown = &others[others.len().saturating_sub(tuning.select.beat_tail_lines)..];
    let lines: Vec<String> = shown.iter().map(|line| format!("- {} in {}, {} h ago: {}", name_of(line, inputs), line.room, hours(inputs.now, line.at_ms), spoken(&clip(&line.text, tuning.select.item_bytes / 2)))).collect();
    let section = newest_section("Lately in her rooms:", &lines, cap.saturating_sub(out.out.len() + 2));
    if !section.is_empty() && out.push("") {
        out.push(&section);
    }
    out.out
}

fn reflection_stimulus(inputs: &Inputs, tuning: &Tuning) -> String {
    let lines: Vec<String> = items(inputs.state, &["thoughtMemory", "shortTerm"]).iter().filter(|item| live(item)).map(|item| format!("- {}", clip(&one_line(text(item, &["summary"])), tuning.select.item_bytes))).collect();
    let section = newest_section("Since she last took stock:", &lines, tuning.select.stimulus_bytes);
    if section.is_empty() { clip("Since she last took stock: nothing new.", tuning.select.stimulus_bytes) } else { section }
}

/// The people present, newest first: who spoke to her or in her room this turn, or in the beat's lines.
fn present_people(inputs: &Inputs, tuning: &Tuning) -> Vec<(String, String)> {
    let lines: Vec<&RoomLine> = match inputs.kind {
        TurnKind::Chat => inputs.room.messages.iter().collect(),
        TurnKind::Beat => {
            let tail = &inputs.room.tail;
            tail[tail.len().saturating_sub(tuning.select.beat_tail_lines)..].iter().collect()
        }
        TurnKind::FollowUp | TurnKind::Reflection => Vec::new(),
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for line in lines.iter().rev().filter(|line| line.sender != inputs.people.own_sender) {
        let person = inputs.people.person_of(&line.sender);
        if !out.iter().any(|(id, _)| *id == person) {
            out.push((person, name_of(line, inputs)));
        }
    }
    out
}

fn bond_line(bond: &Value, name: &str, tuning: &Tuning) -> String {
    let mut feel = vec![friction_words(number(bond, &["tension"]).unwrap_or(0.0))];
    if let Some(trust) = number(bond, &["trust"]) {
        feel.insert(0, trust_words(trust));
    }
    let kind = clip(&one_line(text(bond, &["relationshipKind"])), NAME_BYTES);
    let kind = if kind.is_empty() { String::new() } else { format!(", {kind}") };
    format!("- bond [{}] {name}{kind}: {}. {}", text(bond, &["id"]), feel.join(", "), clip(&one_line(text(bond, &["summary"])), tuning.select.bond_summary_bytes))
}

fn identity_of(state: &Value, tuning: &Tuning) -> String {
    let name = text(state, &["publicName"]);
    let pronouns = text(state, &["presentation", "pronouns"]);
    let head = if pronouns.is_empty() { name.to_owned() } else { format!("{name} ({pronouns})") };
    let voice = text(state, &["presentation", "voiceSummary"]);
    let whole = if voice.is_empty() { head } else { format!("{head}. {}", one_line(voice)) };
    clip(&whole, tuning.select.identity_bytes)
}

/// The names of unbonded people present, as many as fit `ALSO_HERE_BYTES`, then how many more.
fn also_here(named: &[String]) -> Option<String> {
    if named.is_empty() {
        return None;
    }
    let mut line = String::from("- also here: ");
    let mut shown = 0;
    for name in named {
        let more_room = 24;
        if line.len() + name.len() + 2 + more_room > ALSO_HERE_BYTES {
            break;
        }
        if shown > 0 {
            line.push_str(", ");
        }
        line.push_str(name);
        shown += 1;
    }
    if shown < named.len() {
        let _ = write!(line, "{}{} more", if shown > 0 { ", and " } else { "" }, named.len() - shown);
    }
    Some(line)
}

// ---------------------------------------------------------------------------------------------
// The packet.

pub(crate) fn select(inputs: &Inputs, tuning: &Tuning) -> Packet {
    let mut draw = Draw(inputs.seed);
    let identity = identity_of(inputs.state, tuning);
    let (stimulus, heard, overflow) = match inputs.kind {
        TurnKind::Chat => chat_stimulus(inputs, tuning),
        TurnKind::Beat => (beat_stimulus(inputs, tuning), String::new(), Overflow::default()),
        TurnKind::FollowUp => (clip(&format!("She just found: {}", inputs.observation.unwrap_or("nothing")), tuning.select.stimulus_bytes), String::new(), Overflow::default()),
        TurnKind::Reflection => (reflection_stimulus(inputs, tuning), String::new(), Overflow::default()),
    };

    // The fixed slots, each bounded on its own and taken in priority order inside the context budget: the hour and the
    // mood always, then a bond line per person present while they fit, then the names of the rest.
    let budget = tuning.select.context_bytes.saturating_sub(identity.len());
    let present = present_people(inputs, tuning);
    let present_ids: BTreeSet<String> = present.iter().map(|(id, _)| id.clone()).collect();
    let bonds = items(inputs.state, &["affect", "socialBonds"]);
    let mut fixed = vec![format!("- now: {}", inputs.now.format("%A %-d %B, %H:%M UTC")), mood_line(inputs.state, inputs.history.previous.as_ref())];
    let mut used: usize = fixed.iter().map(|line| line.len() + 1).sum();
    let mut ids = Vec::new();
    let mut named = Vec::new();
    let mut full = 0;
    for (id, name) in &present {
        match bonds.iter().find(|bond| live(bond) && text(bond, &["object", "id"]).to_lowercase() == *id) {
            Some(bond) if full < tuning.select.people_full => {
                let line = bond_line(bond, name, tuning);
                if used + line.len() < budget {
                    full += 1;
                    used += line.len() + 1;
                    ids.push(text(bond, &["id"]).to_owned());
                    fixed.push(line);
                } else {
                    named.push(name.clone());
                }
            }
            _ => named.push(name.clone()),
        }
    }
    if let Some(line) = also_here(&named).filter(|line| used + line.len() < budget) {
        used += line.len() + 1;
        fixed.push(line);
    }

    // What the room is touching: the content words of what was said there, and of a follow-up's finding.
    let touching: BTreeSet<String> = inputs.room.messages.iter().chain(&inputs.room.tail).map(|line| line.text.as_str()).chain(inputs.observation).flat_map(content_words).collect();
    let pool = candidates(inputs, tuning, &present_ids, &touching);
    // Gumbel-top-k over log(score)/T orders the candidates as sampling without replacement from the softmax would; each
    // is then included with a probability its score sets, so a cap is a maximum, never a target.
    let temperature = tuning.select.temperature;
    let mut keyed: Vec<(f64, bool, Candidate)> = pool
        .into_iter()
        .map(|candidate| {
            let key = candidate.log_score / temperature + draw.gumbel();
            let include = draw.unit() < 1.0 - (-candidate.log_score.exp() / tuning.select.inclusion_score).exp();
            (key, include, candidate)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut taken: Vec<Candidate> = Vec::new();
    for (_, include, candidate) in keyed {
        if !include || taken.iter().filter(|kept| kept.pool == candidate.pool).count() >= cap_of(candidate.pool, tuning) {
            continue;
        }
        if used + candidate.line.len() + 1 > budget {
            continue;
        }
        used += candidate.line.len() + 1;
        taken.push(candidate);
    }
    taken.sort_by_key(|candidate| candidate.pool);
    // The lens is drawn from what the packet holds: never a want, a memory or a wondering it does not carry.
    let present_names: Vec<String> = present.iter().map(|(_, name)| name.clone()).collect();
    let lens = draw_lens(&tuning.lens, &present_names, &taken, &mut draw);
    let mut lines = fixed;
    lines.extend(taken.iter().map(|candidate| candidate.line.clone()));
    ids.extend(taken.iter().map(|candidate| candidate.id.clone()));
    let selected = taken.into_iter().map(|candidate| Pick { id: candidate.id, pool: candidate.pool, log_score: candidate.log_score }).collect();
    Packet { kind: inputs.kind, seed: inputs.seed, lens, identity, context: lines.join("\n"), stimulus, heard, overflow, present: present_names, ids, selected }
}
