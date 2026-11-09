//! What of her own mind enters a Persona turn besides what she is always shown, and in what words.
//!
//! Ruling `eureka-body:ruling:persona-memory-written-not-transcript`: what she
//! must still know when the conversation is gone is written into her mind as
//! typed memory, and a later turn reads it back from there. Open asks and open
//! commitments are not recalled: `persona::render_responsibilities` shows them
//! on every turn beside her stewardships, because they stay in front of her
//! until a resolution closes them. This module owns the cued, ranked remainder
//! and nothing else. It reads only her own mind through `MindPort`; it reads no
//! chat (the lines shown beside a message are conversation, never memory),
//! keeps no cache between turns, and holds no index: the daemon's index answers
//! `MindPort::query_near`, and a rank only orders.
//!
//! Three lanes, merged in this order and de-duplicated by id, none showing an
//! open item:
//! - `about`: memory in force about any other sender of this batch, ranked by
//!   `weight` from the newest `POOL` of them.
//! - `relevant`: memory, follow-up and ruling in force nearest the batch's
//!   words, in the daemon's order. A wake has no words and skips it.
//! - `salient`: memory in force ranked by `weight` from the newest `POOL`.
//!   On a wake, which has no senders and no words, this is all that runs.
//!
//! Each lane has an item limit and a byte budget, so no one signal crowds out
//! another; a lane that leaves something out says how many.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{MemoryOrigin, MemorySort, PipelineDocument, PipelineKind};
use huginn_mind::{AdmissionFacts, PipelineDocumentView, PipelinePageItems};

use crate::persona::quoted;
use crate::queue::{MindPort, any_of, in_force, of_kind};

/// What each lane shows, and the bytes of rendered lines it may spend.
pub(crate) const ABOUT_LIMIT: usize = 6;
pub(crate) const ABOUT_BYTES: usize = 5 * 1024;
pub(crate) const RELEVANT_LIMIT: usize = 8;
pub(crate) const RELEVANT_BYTES: usize = 7 * 1024;
pub(crate) const SALIENT_LIMIT: usize = 4;
pub(crate) const SALIENT_BYTES: usize = 4 * 1024;
/// The most bytes of rendered lines one turn recalls: the lanes' budgets.
#[cfg(test)]
pub(crate) const MAX_BYTES: usize = ABOUT_BYTES + RELEVANT_BYTES + SALIENT_BYTES;
/// The newest memories a ranked lane ranks, and the nearest documents the index is asked for: open
/// items among them are dropped, so asking past the lane's limit keeps its items in reach.
const POOL: u32 = 32;
/// Hours over which a memory's recency halves. Calibration, not a gate.
pub(crate) const RECENCY_HALF_LIFE_H: f64 = 72.0;
/// The longest semantic text, in bytes, cut on a character boundary.
pub(crate) const NEAR_TEXT_MAX: usize = 1000;
/// The sorts recall may show: the ones that are not open items.
const RECALLED_SORTS: [&str; 5] = ["Decided", "Learned", "Did", "Idea", "Rapport"];

/// Which lane brought an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Lane {
    About,
    Relevant,
    Salient,
}

impl Lane {
    fn heading(self) -> &'static str {
        match self {
            Self::About => "About the people here:",
            Self::Relevant => "Near what was said:",
            Self::Salient => "Weighed most:",
        }
    }
}

/// One recalled document as a line of the turn's input, and the people it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub id: String,
    pub line: String,
    /// The Matrix ids its `about` and origin name.
    pub people: Vec<String>,
}

/// One lane's result: what it shows and how many it left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub lane: Lane,
    pub entries: Vec<Entry>,
    pub left_out: usize,
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

/// What a turn recalls, lane by lane in merge order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recalled {
    pub lanes: Vec<Shown>,
    pub search: Search,
}

impl Recalled {
    /// Every item shown, in merge order.
    pub(crate) fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.lanes.iter().flat_map(|shown| shown.entries.iter())
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
    /// The time recency is measured against.
    pub now: DateTime<Utc>,
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

fn recalled_memories_in_force() -> Selection {
    any_of(in_force(of_kind(PipelineKind::Memory)), "sort", &RECALLED_SORTS)
}

/// How much a memory weighs: its salience in thousandths of one, plus a
/// recency that is 1 when admitted now and halves every `RECENCY_HALF_LIFE_H`
/// hours. A time that cannot be read has no recency. Relevance is its own
/// lane, not a summand, so no one signal can crowd out another.
pub(crate) fn weight(salience_permille: u16, admitted_at: &str, now: DateTime<Utc>) -> f64 {
    let recency = DateTime::parse_from_rfc3339(admitted_at).map_or(0.0, |at| {
        let age_hours = (now - at.with_timezone(&Utc)).num_seconds().max(0) as f64 / 3600.0;
        0.5_f64.powf(age_hours / RECENCY_HALF_LIFE_H)
    });
    f64::from(salience_permille) / 1000.0 + recency
}

/// The views ranked by weight, heaviest first; equal weights keep their order, which is newest first.
fn ranked(mut views: Vec<PipelineDocumentView>, now: DateTime<Utc>) -> Vec<PipelineDocumentView> {
    let weight_of = |view: &PipelineDocumentView| match &view.document {
        PipelineDocument::Memory(memory) => weight(memory.salience_permille, &view.admission.admitted_at, now),
        _ => 0.0,
    };
    views.sort_by(|a, b| weight_of(b).total_cmp(&weight_of(a)));
    views
}

/// The people a memory names, each quoted, by display name where this turn's senders showed one.
fn who<'a>(ids: impl Iterator<Item = &'a str>, names: &BTreeMap<&str, &str>) -> String {
    let people: Vec<String> = ids.map(|id| quoted(names.get(id).copied().filter(|name| !name.is_empty()).unwrap_or(id))).collect();
    if people.is_empty() { "someone".into() } else { people.join(", ") }
}

/// The ids a memory's `about` and origin name.
pub(crate) fn people_of(memory: &eureka_pipeline::PipelineMemory) -> Vec<String> {
    let origin: &[eureka_pipeline::Short] = match &memory.origin {
        MemoryOrigin::Room { heard_from, .. } => heard_from,
        MemoryOrigin::Direct { with, .. } => with,
        MemoryOrigin::Wake | MemoryOrigin::SelfRun { .. } | MemoryOrigin::Sleep { .. } => &[],
    };
    memory.about.iter().chain(origin).map(|id| id.0.clone()).collect()
}

/// Where she learned something and from whom, as a phrase: the one render of an origin, for a memory and for a brief.
pub(crate) fn origin_phrase(origin: &MemoryOrigin, names: &BTreeMap<&str, &str>) -> String {
    match origin {
        MemoryOrigin::Room { rooms, heard_from } if heard_from.is_empty() => format!("in #{}", rooms.0),
        MemoryOrigin::Room { rooms, heard_from } => format!("in #{}, with {}", rooms.0, who(heard_from.iter().map(|id| id.0.as_str()), names)),
        MemoryOrigin::Direct { with, .. } => format!("privately, with {}", who(with.iter().map(|id| id.0.as_str()), names)),
        MemoryOrigin::Wake => "on a wake".to_string(),
        MemoryOrigin::SelfRun { run } => format!("in your Self run {}", run.0),
        MemoryOrigin::Sleep { run } => format!("in your sleep {}", run.0),
    }
}

/// One memory as its line: date, where and from whom she learned it, what it
/// is and whom it concerns, then her words. The one way a memory reads, for
/// recall and for the open items the responsibilities section shows, which pass
/// `with_id` to lead with the id she settles it by. `about` is never the place.
pub(crate) fn memory_line(view: &PipelineDocumentView, names: &BTreeMap<&str, &str>, with_id: bool) -> Option<String> {
    // An open item of the responsibilities (the one caller that leads with the id) shows a clipped text; recall shows it whole.
    let PipelineDocument::Memory(memory) = &view.document else { return None };
    let when = origin_time(&view.admission);
    let date = when.get(..10).unwrap_or(when);
    let origin = origin_phrase(&memory.origin, names);
    let about = who(memory.about.iter().map(|id| id.0.as_str()), names);
    let sort = match (memory.sort, memory.about.is_empty()) {
        (MemorySort::Asked, _) => format!("asked by {about}"),
        (MemorySort::Committed, true) => "committed".to_string(),
        (MemorySort::Committed, false) => format!("committed, with {about}"),
        (sort, true) => format!("{sort:?}"),
        (sort, false) => format!("{sort:?} about {about}"),
    };
    let lead = if with_id { format!("{} ", view.id.id.0) } else { String::new() };
    Some(format!("- {lead}({date}, {origin}, {sort}): {}\n", quoted(&if with_id { clipped(&memory.text.0) } else { memory.text.0.clone() })))
}

/// The most bytes of an item's text a listed line carries; the rest is one `huginn view` away.
pub(crate) const LINE_TEXT_MAX_BYTES: usize = 160;

/// `text` cut to about `LINE_TEXT_MAX_BYTES` on a character boundary, with an ellipsis where it was cut.
pub(crate) fn clipped(text: &str) -> String {
    if text.len() <= LINE_TEXT_MAX_BYTES {
        return text.to_owned();
    }
    let cut = (0..=LINE_TEXT_MAX_BYTES).rev().find(|at| text.is_char_boundary(*at)).unwrap_or(0);
    format!("{}…", &text[..cut])
}

/// When a document first stood, as the time it was admitted where it began: an imported document carries the source
/// receipt's time, any other its own admission.
pub(crate) fn origin_time(admission: &AdmissionFacts) -> &str {
    admission.provenance.imported_from.as_ref().map_or(admission.admitted_at.as_str(), |source| source.committed_at.0.as_str())
}

/// The sort key of oldest first: origin time, then the mind's own admission ordinal for equal times.
pub(crate) fn origin_order(admission: &AdmissionFacts) -> (&str, u64) {
    (origin_time(admission), admission.ordinal)
}

/// One recalled document as its line, or None for a kind recall does not show.
fn line_of(view: &PipelineDocumentView, names: &BTreeMap<&str, &str>) -> Option<String> {
    match &view.document {
        PipelineDocument::Memory(_) => memory_line(view, names, false),
        PipelineDocument::FollowUp(follow_up) => Some(format!("- You set yourself to: {}\n", quoted(follow_up.item.0.lines().next().unwrap_or("")))),
        PipelineDocument::Ruling(ruling) => Some(format!("- Ruled: {}\n", quoted(&ruling.title.0))),
        _ => None,
    }
}

/// Fills one lane from its ranked candidates: each not yet shown or listed
/// elsewhere, within the lane's item limit and byte budget (a line that does
/// not fit is skipped, a later shorter one may). `matched` is how many the
/// lane's query matched, so what it left out is counted exactly.
fn fill(lane: Lane, candidates: Vec<PipelineDocumentView>, matched: usize, limit: usize, budget: usize, seen: &mut Vec<String>, cue: &Cue) -> Shown {
    let (mut entries, mut bytes, mut elsewhere) = (Vec::new(), 0usize, 0usize);
    for view in candidates {
        let id = view.id.id.0.clone();
        if seen.contains(&id) || cue.listed.contains(&id) {
            elsewhere += 1;
            continue;
        }
        let Some(line) = line_of(&view, &cue.names) else { continue };
        if entries.len() == limit || bytes + line.len() > budget {
            continue;
        }
        seen.push(id.clone());
        bytes += line.len();
        let people = match &view.document {
            PipelineDocument::Memory(memory) => people_of(memory),
            _ => Vec::new(),
        };
        entries.push(Entry { id, line, people });
    }
    let left_out = matched.saturating_sub(entries.len() + elsewhere);
    Shown { lane, entries, left_out }
}

/// Whether a view is an open ask or commitment: a memory of those sorts.
fn is_open_item(view: &PipelineDocumentView) -> bool {
    matches!(&view.document, PipelineDocument::Memory(memory) if matches!(memory.sort, MemorySort::Asked | MemorySort::Committed))
}

/// Selects what her mind gives this turn. A mind that cannot be read fails the
/// turn; an index that cannot be asked only loses the relevant lane.
pub(crate) fn select(mind: &dyn MindPort, cue: &Cue) -> Result<Recalled> {
    let (about, about_matched) = if cue.senders.is_empty() {
        (Vec::new(), 0)
    } else {
        let page = mind.query(&newest(any_of(recalled_memories_in_force(), "about", &cue.senders), POOL))?;
        let matched = page.matched as usize;
        (ranked(documents(page)?, cue.now), matched)
    };
    let (relevant, search) = match near_text(cue) {
        None => (Vec::new(), Search::Skipped),
        Some(text) => {
            let kinds = [PipelineKind::Memory, PipelineKind::FollowUp, PipelineKind::Ruling];
            let members = in_force(Selection { schemas: Some(kinds.iter().map(|kind| kind.type_id().to_string()).collect()), projection: "document".into(), ..Selection::default() });
            match mind.query_near(&members, &text, POOL).and_then(documents) {
                // An open ask or commitment is shown by the responsibilities section; it never comes here.
                Ok(found) => (found.into_iter().filter(|view| !is_open_item(view)).collect::<Vec<_>>(), Search::Ran),
                Err(_) => (Vec::new(), Search::Unavailable),
            }
        }
    };
    let page = mind.query(&newest(recalled_memories_in_force(), POOL))?;
    let salient_matched = page.matched as usize;
    let salient = ranked(documents(page)?, cue.now);

    let mut seen = Vec::<String>::new();
    let relevant_found = relevant.len();
    let lanes = vec![
        fill(Lane::About, about, about_matched, ABOUT_LIMIT, ABOUT_BYTES, &mut seen, cue),
        fill(Lane::Relevant, relevant, relevant_found, RELEVANT_LIMIT, RELEVANT_BYTES, &mut seen, cue),
        fill(Lane::Salient, salient, salient_matched, SALIENT_LIMIT, SALIENT_BYTES, &mut seen, cue),
    ];
    Ok(Recalled { lanes, search })
}

/// The section of a turn's input: one heading per lane that shows or left out anything, one line each,
/// and after a lane that left something out a line saying how many.
pub(crate) fn render(recalled: &Recalled) -> String {
    let mut out = String::from("What you remember (from your own mind; `huginn view <id>` reads a record):\n");
    for shown in &recalled.lanes {
        if shown.entries.is_empty() && shown.left_out == 0 {
            continue;
        }
        out.push_str(shown.lane.heading());
        out.push('\n');
        shown.entries.iter().for_each(|entry| out.push_str(&entry.line));
        if shown.left_out > 0 {
            out.push_str(&format!("({} more not shown.)\n", shown.left_out));
        }
    }
    if recalled.lanes.iter().all(|shown| shown.entries.is_empty() && shown.left_out == 0) {
        out.push_str("- nothing is remembered yet.\n");
    }
    if recalled.search == Search::Unavailable {
        out.push_str("(The search of your memory is unavailable this turn; only what concerns the senders and what weighs most are shown.)\n");
    }
    out
}

#[cfg(test)]
mod tests;
