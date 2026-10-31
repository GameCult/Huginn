//! What of her own mind enters a Persona turn, and in what words.
//!
//! Ruling `eureka-body:ruling:persona-memory-written-not-transcript`: what she
//! must still know when the conversation is gone is written into her mind as
//! typed memory, and a later turn reads it back from there. This module owns
//! the reading and nothing else. It reads only her own mind through `MindPort`;
//! it reads no chat (the lines shown beside a message are conversation, never memory),
//! keeps no cache between turns, and holds no index: the daemon's index answers
//! `MindPort::query_near`, and a rank only orders.
//!
//! Four lanes, merged in this order and de-duplicated by id:
//! - `asks`: memory in force with sort `Asked`, newest first, on every turn
//!   including a wake. An ask stays in front of her until a resolution closes
//!   it, so nothing stores an open flag.
//! - `about`: memory in force about any other sender of this batch (rapport,
//!   ideas and commitments with them come here).
//! - `relevant`: memory, follow-up and ruling in force nearest the batch's
//!   words. A wake has no words and skips it.
//! - `newest`: the latest memory in force.
//!
//! The merge is capped at `MAX_ITEMS` items and `MAX_BYTES` of rendered text,
//! the asks lane first, so a long tail never pushes an ask out.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use cultnet_rs::Selection;
use eureka_pipeline::{PipelineDocument, PipelineKind};
use huginn_mind::{PipelineDocumentView, PipelinePageItems};

use crate::persona::quoted;
use crate::queue::{MindPort, any_of, in_force, of_kind};

/// Open asks shown per turn.
const ASKS_LIMIT: u32 = 8;
/// Memories about the batch's other senders.
const ABOUT_LIMIT: u32 = 6;
/// Nearest documents asked of the index.
const RELEVANT_TOP_K: u32 = 8;
/// The latest memories.
const NEWEST_LIMIT: u32 = 4;
/// The most items one turn recalls.
pub(crate) const MAX_ITEMS: usize = 20;
/// The most bytes of rendered lines one turn recalls.
pub(crate) const MAX_BYTES: usize = 16 * 1024;
/// The longest id the index of `about` takes; a longer sender id is not asked about.
const ABOUT_ID_MAX_BYTES: usize = 200;
/// The longest semantic text, in bytes, cut on a character boundary.
pub(crate) const NEAR_TEXT_MAX: usize = 1000;

/// Which lane brought an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Lane {
    Asks,
    About,
    Relevant,
    Newest,
}

/// One recalled document as a line of the turn's input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub id: String,
    pub lane: Lane,
    pub line: String,
}

/// Whether the semantic lane ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Search {
    Ran,
    /// A wake has no words to look near.
    Skipped,
    /// The index could not be asked; the turn goes on without that lane.
    Unavailable,
}

/// What a turn recalls: the open asks she may settle or take on, then the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recalled {
    pub asks: Vec<Entry>,
    pub others: Vec<Entry>,
    pub search: Search,
}

impl Recalled {
    /// The ids of the open asks shown: the only ones a reply may settle or take on.
    pub(crate) fn ask_ids(&self) -> Vec<String> {
        self.asks.iter().map(|entry| entry.id.clone()).collect()
    }
}

/// What the batch lets recall look for.
pub(crate) struct Cue<'a> {
    /// The batch's senders other than her own, in arrival order.
    pub senders: Vec<&'a str>,
    /// The batch's message texts, newest first; empty on a wake.
    pub texts: Vec<&'a str>,
    /// Display names by sender id, to read a line with.
    pub names: BTreeMap<&'a str, &'a str>,
    /// Follow-ups and rulings the responsibilities section already lists.
    pub listed: &'a [String],
}

/// The semantic text: message texts newest first, then the senders' names,
/// cut to `NEAR_TEXT_MAX` bytes on a character boundary. None when it says nothing.
pub(crate) fn near_text(cue: &Cue) -> Option<String> {
    let mut parts: Vec<&str> = cue.texts.iter().copied().filter(|text| !text.trim().is_empty()).collect();
    let mut names: Vec<&str> = cue.senders.iter().filter_map(|id| cue.names.get(id).copied()).filter(|name| !name.is_empty()).collect();
    names.dedup();
    parts.extend(names);
    let mut joined = parts.join("\n");
    if joined.len() > NEAR_TEXT_MAX {
        let mut end = NEAR_TEXT_MAX;
        while !joined.is_char_boundary(end) {
            end -= 1;
        }
        joined.truncate(end);
    }
    if joined.trim().is_empty() { None } else { Some(joined) }
}

fn documents(page: huginn_mind::PipelineSelectionPage) -> Result<Vec<PipelineDocumentView>> {
    match page.items {
        PipelinePageItems::Documents(views) => Ok(views),
        PipelinePageItems::Headers(_) => bail!("the mind answered headers to a document selection"),
    }
}

fn newest(selection: Selection, limit: u32) -> Selection {
    Selection { descending: true, limit: Some(limit), projection: "document".into(), ..selection }
}

fn memories_in_force() -> Selection {
    in_force(of_kind(PipelineKind::Memory))
}

/// Where a memory was learned, as she should read it.
fn where_learned(direct: bool, learned_in: &str, who: &str) -> String {
    if direct { format!("privately, with {who}") } else { format!("in #{learned_in}") }
}

/// The people a memory names, each quoted, by display name where this turn's senders showed one.
fn who<'a>(ids: impl Iterator<Item = &'a str>, names: &BTreeMap<&str, &str>) -> String {
    let people: Vec<String> = ids.map(|id| quoted(names.get(id).copied().filter(|name| !name.is_empty()).unwrap_or(id))).collect();
    if people.is_empty() { "someone".into() } else { people.join(", ") }
}

/// One recalled document as its line, or None for a kind recall does not show.
fn line_of(view: &PipelineDocumentView, lane: Lane, names: &BTreeMap<&str, &str>) -> Option<String> {
    let date = view.admission.admitted_at.get(..10).unwrap_or(&view.admission.admitted_at);
    match &view.document {
        PipelineDocument::Memory(memory) => {
            let people = who(memory.about.iter().map(|id| id.0.as_str()), names);
            let place = where_learned(memory.direct, &memory.learned_in.0, &people);
            if lane == Lane::Asks {
                Some(format!("- {} ({date}, {place}, asked by {people}): {}\n", view.id.id.0, quoted(&memory.text.0)))
            } else if memory.about.is_empty() {
                Some(format!("- {date}, {place}, {:?}: {}\n", memory.sort, quoted(&memory.text.0)))
            } else {
                Some(format!("- {date}, {place}, {:?} about {people}: {}\n", memory.sort, quoted(&memory.text.0)))
            }
        }
        PipelineDocument::FollowUp(follow_up) => Some(format!("- You set yourself to: {}\n", quoted(follow_up.item.0.lines().next().unwrap_or("")))),
        PipelineDocument::Ruling(ruling) => Some(format!("- Ruled: {}\n", quoted(&ruling.title.0))),
        _ => None,
    }
}

/// Selects what her mind gives this turn. A mind that cannot be read fails the
/// turn; an index that cannot be asked only loses the relevant lane.
pub(crate) fn select(mind: &dyn MindPort, cue: &Cue) -> Result<Recalled> {
    let asks = documents(mind.query(&newest(any_of(memories_in_force(), "sort", &["Asked"]), ASKS_LIMIT))?)?;
    let askable: Vec<&str> = cue.senders.iter().copied().filter(|sender| sender.len() <= ABOUT_ID_MAX_BYTES).collect();
    let about = if askable.is_empty() {
        Vec::new()
    } else {
        documents(mind.query(&newest(any_of(memories_in_force(), "about", &askable), ABOUT_LIMIT))?)?
    };
    let (relevant, search) = match near_text(cue) {
        None => (Vec::new(), Search::Skipped),
        Some(text) => {
            let kinds = [PipelineKind::Memory, PipelineKind::FollowUp, PipelineKind::Ruling];
            let members = in_force(Selection { schemas: Some(kinds.iter().map(|kind| kind.type_id().to_string()).collect()), projection: "document".into(), ..Selection::default() });
            match mind.query_near(&members, &text, RELEVANT_TOP_K).and_then(documents) {
                Ok(found) => (found, Search::Ran),
                Err(_) => (Vec::new(), Search::Unavailable),
            }
        }
    };
    let latest = documents(mind.query(&newest(memories_in_force(), NEWEST_LIMIT))?)?;

    let mut recalled = Recalled { asks: Vec::new(), others: Vec::new(), search };
    let (mut seen, mut count, mut bytes) = (Vec::<String>::new(), 0usize, 0usize);
    for (lane, views) in [(Lane::Asks, asks), (Lane::About, about), (Lane::Relevant, relevant), (Lane::Newest, latest)] {
        for view in views {
            let id = view.id.id.0.clone();
            if seen.contains(&id) || cue.listed.contains(&id) {
                continue;
            }
            let Some(line) = line_of(&view, lane, &cue.names) else { continue };
            if count == MAX_ITEMS || bytes + line.len() > MAX_BYTES {
                continue;
            }
            seen.push(id.clone());
            count += 1;
            bytes += line.len();
            let entry = Entry { id, lane, line };
            if lane == Lane::Asks { recalled.asks.push(entry) } else { recalled.others.push(entry) }
        }
    }
    Ok(recalled)
}

/// The section of a turn's input: the open asks first, then the rest, one line each.
pub(crate) fn render(recalled: &Recalled) -> String {
    let mut out = String::from("What you remember (from your own mind; `huginn view <id>` reads a record):\n");
    if !recalled.asks.is_empty() {
        out.push_str("Asked of you, not yet taken on:\n");
        recalled.asks.iter().for_each(|entry| out.push_str(&entry.line));
    }
    if !recalled.others.is_empty() {
        out.push_str("Also remembered:\n");
        recalled.others.iter().for_each(|entry| out.push_str(&entry.line));
    }
    if recalled.asks.is_empty() && recalled.others.is_empty() {
        out.push_str("- nothing is remembered yet.\n");
    }
    if recalled.search == Search::Unavailable {
        out.push_str("(The search of your memory is unavailable this turn; only the newest and what concerns the senders are shown.)\n");
    }
    out
}

#[cfg(test)]
mod tests;
