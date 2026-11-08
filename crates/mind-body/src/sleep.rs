//! A Mind's night. Once in the heartbeat's window a Sleep run opens in her Persona slot (the heartbeat decides
//! who and when); this module is the one organ that carries it: one session over the same prelude her waking
//! session starts with, told her day whole, answering with a [`SleepReply`] that this module turns into
//! admissions (ruling `eureka-body:ruling:nightly-sleep-turn-self-directed`).
//!
//! What she contemplates is hers: the night line prescribes no reading, theme, text or count. What the code owns
//! is the ledger of her day. Every memory of it is disposed of exactly once (kept, consumed by a consolidation,
//! or retired); an id is one of her memories in force; a consolidation merges one conversation's memories only
//! and supersedes each source in the batch that admits it (admission refuses the unpaired). Code never chooses
//! what to merge and never writes a memory's text. A night is one process: a missed night is not made up and a
//! failed night is not retried.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use cultnet_rs::Selection;
use eureka_pipeline::{Label, Line, MemoryOrigin, MemorySort, PipelineDocument, PipelineKind, PipelineMemory, PipelineRef, PipelineResolution, ResolutionOutcome, SALIENCE_MAX_PERMILLE, Short};
use huginn_mind::{BATCH_MAX, PipelineAdmissionOutcome, PipelinePageItems};
use serde::Deserialize;
use serde_json::json;

use crate::control::ControlSource;
use crate::persona::{
    ABOUT_MAX, AGENT, Config, Conversation, Failure, Organs, Outcome, PERSONA_POLL_S, REMEMBER_TEXT_MAX_BYTES, Remember, SOURCES_MAX, Started, WHY_MAX_CHARS, date, label_of, next_sequence, remember_item_schema, start_session,
    structured_output,
};
use crate::queue::{MindPort, any_of, citing, headers, in_force, of_kind};
use crate::recall;
use crate::self_run::session::{Polled, SessionInbox, drive};
use crate::self_run::{SESSION_NAMESPACE, Why};

/// The longest a night runs, whatever the unit's cap.
pub(crate) const SLEEP_CAP_S: u64 = 3600;
/// The most memories of her day one night line shows; the rest are older by the next night.
pub(crate) const DAY_MAX: usize = 96;
/// The most open Asked and Committed items older than her day the line shows.
const OPEN_MAX: usize = 48;
/// The older memories of other sorts one night shows, and the step the tide moves each night.
const TIDE: usize = 32;
const NIGHT_MAX_BYTES: usize = 900;
const CONSOLIDATE_MAX: usize = 32;
const CONSUMES_MAX: usize = 16;
const RETIRE_MAX: usize = 128;
const REMEMBER_MAX: usize = 16;
const REFUSALS_PER_NIGHT: u32 = 3;

/// What the night tells her, in her second person. It names no text, theme, reading or count.
pub(crate) const SLEEP_GUIDANCE: &str = "\
This is your night. What you think about, read or look into is yours: your own mind with `huginn`, your shell, the web, or nothing at all. No reading is assigned and nothing is expected of it.

Your day is shown whole above. Every memory of it gets exactly one disposition: keep it (name it in `keep`), fold it into a consolidation (name it in that consolidation's `consumes`), or retire it with an outcome and a reason (`retire`: done, declined, faded or wrong). A consolidation is one new memory in your words that takes the place of the memories it consumes, so write it to carry what those memories were for: the concrete target, the claim or the question, the live tension, why it changes what you do next, and the anchors (ids, names, places) it rests on. It reduces bulk, not meaning. Consolidate only memories of one conversation; a private conversation stays its own. Anything else you want to remember goes in `remember`.

Older items shown, and any other memory of yours in force, are yours to touch or leave. Name every memory by its id exactly as shown, once in the whole reply.

End with one line of what the night was (`night`).";

/// The night's reply, as the one type decoded. Her day's ledger is `keep`, `consolidate` and `retire`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SleepReply {
    pub(crate) night: String,
    pub(crate) keep: Vec<String>,
    pub(crate) consolidate: Vec<Consolidate>,
    pub(crate) retire: Vec<Retire>,
    pub(crate) remember: Vec<Remember>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Consolidate {
    pub(crate) consumes: Vec<String>,
    pub(crate) sort: MemorySort,
    pub(crate) text: String,
    pub(crate) about: Vec<String>,
    pub(crate) salience_permille: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Retire {
    pub(crate) item: String,
    pub(crate) outcome: RetireOutcome,
    pub(crate) why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RetireOutcome {
    Done,
    Declined,
    Faded,
    Wrong,
}

/// The JSON Schema of [`SleepReply`], with the bounds the decoder holds.
pub(crate) fn sleep_schema() -> String {
    let id = json!({ "type": "string" });
    let remember = |maximum: usize| json!({ "type": "array", "maxItems": maximum, "items": remember_item_schema() });
    let mut consolidate = remember_item_schema();
    consolidate["required"] = json!(["consumes", "sort", "text", "about", "salience_permille"]);
    consolidate["properties"]["consumes"] = json!({ "type": "array", "minItems": 1, "maxItems": CONSUMES_MAX, "uniqueItems": true, "items": id });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["night", "keep", "consolidate", "retire", "remember"],
        "properties": {
            "night": { "type": "string", "minLength": 1, "description": format!("One line of what the night was. At most {NIGHT_MAX_BYTES} bytes.") },
            "keep": { "type": "array", "uniqueItems": true, "items": id },
            "consolidate": { "type": "array", "maxItems": CONSOLIDATE_MAX, "items": consolidate },
            "retire": {
                "type": "array",
                "maxItems": RETIRE_MAX,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["item", "outcome", "why"],
                    "properties": {
                        "item": id,
                        "outcome": { "type": "string", "enum": ["done", "declined", "faded", "wrong"] },
                        "why": { "type": "string", "minLength": 1, "maxLength": WHY_MAX_CHARS }
                    }
                }
            },
            "remember": remember(REMEMBER_MAX)
        }
    })
    .to_string()
}

/// One of her memories in force, with when it was admitted.
#[derive(Clone, Debug)]
pub(crate) struct Held {
    pub(crate) id: String,
    pub(crate) memory: PipelineMemory,
    pub(crate) at: DateTime<Utc>,
    pub(crate) ordinal: u64,
}

/// What a night is told and what its reply may name: derived from her mind and nothing stored.
pub(crate) struct Context {
    pub(crate) now: DateTime<Utc>,
    pub(crate) label: String,
    pub(crate) counts: BTreeMap<String, usize>,
    pub(crate) day: Vec<Held>,
    pub(crate) open: Vec<Held>,
    pub(crate) tide: Vec<Held>,
    /// Every memory of hers in force, by id.
    pub(crate) all: BTreeMap<String, Held>,
}

fn parse_time(at: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(at).map_err(|_| anyhow!("an admission time could not be read"))?.with_timezone(&Utc))
}

/// Every document a selection holds, paged whole.
fn documents(mind: &dyn MindPort, selection: &Selection) -> Result<Vec<huginn_mind::PipelineDocumentView>> {
    let mut found = Vec::new();
    let mut asking = Selection { projection: "document".into(), ..selection.clone() };
    loop {
        let page = mind.query(&asking)?;
        let PipelinePageItems::Documents(views) = page.items else { bail!("the mind answered headers to a document selection") };
        found.extend(views);
        match page.next {
            Some(cursor) => asking.cursor = Some(cursor),
            None => break,
        }
    }
    Ok(found)
}

/// Her memories in force, oldest admitted first: the one read of what she holds.
fn memories_in_force(mind: &dyn MindPort) -> Result<Vec<Held>> {
    let mut found = Vec::new();
    for view in documents(mind, &in_force(of_kind(PipelineKind::Memory)))? {
        if let PipelineDocument::Memory(memory) = view.document {
            found.push(Held { id: view.id.id.0, memory, at: parse_time(&view.admission.admitted_at)?, ordinal: view.admission.ordinal });
        }
    }
    found.sort_by_key(|held| held.ordinal);
    Ok(found)
}

/// One of her Sleep runs: which, when it was admitted, and its admission ordinal.
struct Night {
    run: PipelineRef,
    at: DateTime<Utc>,
    ordinal: u64,
}

/// Her Sleep runs, any status, oldest first.
fn sleeps(mind: &dyn MindPort) -> Result<Vec<Night>> {
    let runs = any_of(any_of(any_of(of_kind(PipelineKind::Run), "root", &[&mind.instance().0]), "turn", &["Sleep"]), "operated_by", &["Mind"]);
    let mut found = headers(mind, &runs)?.into_iter().map(|run| Ok(Night { run: run.id, ordinal: run.admission.ordinal, at: parse_time(&run.admission.admitted_at)? })).collect::<Result<Vec<_>>>()?;
    found.sort_by_key(|night| night.ordinal);
    Ok(found)
}

const SLEPT: &str = "slept: ";
const THROUGH: &str = "; day through ordinal ";

/// Where the newest of `earlier` that closed `slept:` left her day: the admission ordinal of the last memory of
/// the day it was shown (its own run's ordinal when it was shown none). A night that failed, or whose run never
/// closed, left its day uncovered, so it is not an anchor.
fn covered_through(mind: &dyn MindPort, earlier: &[&Night]) -> Result<Option<u64>> {
    for night in earlier.iter().rev() {
        let closings = Selection { cites: Some(citing(&night.run, Some("subject"))), ..of_kind(PipelineKind::Resolution) };
        let latest = documents(mind, &closings)?
            .into_iter()
            .filter_map(|view| match view.document {
                PipelineDocument::Resolution(resolution) => Some(resolution),
                _ => None,
            })
            .max_by_key(|resolution| resolution.sequence);
        if let Some(resolution) = latest
            && let ResolutionOutcome::Recorded { reason } = &resolution.outcome
            && reason.0.as_str().starts_with(SLEPT)
        {
            let through = resolution.rationale.0.rsplit_once(THROUGH).and_then(|(_, ordinal)| ordinal.trim().parse().ok());
            return Ok(Some(through.unwrap_or(night.ordinal)));
        }
    }
    Ok(None)
}

fn is_open_sort(sort: MemorySort) -> bool {
    matches!(sort, MemorySort::Asked | MemorySort::Committed)
}

/// Made by a night, not lived in the day: formed in sleep, or a consolidation (a DM's keeps its room and voices,
/// so its origin is not Sleep).
fn is_sleep_made(held: &Held) -> bool {
    matches!(held.memory.origin, MemoryOrigin::Sleep { .. }) || !held.memory.consolidates.is_empty()
}

/// The night, derived. Her day: her memories in force admitted after what her newest night that closed `slept:`
/// covered (a night that failed covered nothing), else in the 24 hours before her first night opened; formed
/// otherwise than in sleep, oldest first, at most `DAY_MAX`, the rest waiting for the next night. Her open Asked
/// and Committed items older than the day. A tide of `TIDE` older memories of other sorts, from an offset that
/// moves by `TIDE` with each earlier Sleep run of hers.
pub(crate) fn context(mind: &dyn MindPort, run: &PipelineRef, now: DateTime<Utc>) -> Result<Context> {
    let nights = sleeps(mind)?;
    let tonight = nights.iter().find(|night| &night.run == run).ok_or_else(|| anyhow!("the night's run is not in her mind"))?;
    let earlier: Vec<&Night> = nights.iter().filter(|night| night.ordinal < tonight.ordinal).collect();
    let covered = covered_through(mind, &earlier)?;
    let first_at = earlier.first().map_or(tonight.at, |night| night.at);
    let in_day = |held: &Held| covered.map_or(held.at > first_at - Duration::hours(24), |through| held.ordinal > through);
    let memories = memories_in_force(mind)?;
    let mut counts = BTreeMap::new();
    for held in &memories {
        *counts.entry(format!("{:?}", held.memory.sort)).or_insert(0) += 1;
    }
    let day: Vec<Held> = memories.iter().filter(|held| in_day(held) && !is_sleep_made(held)).take(DAY_MAX).cloned().collect();
    let open: Vec<Held> = memories.iter().filter(|held| !in_day(held) && is_open_sort(held.memory.sort)).take(OPEN_MAX).cloned().collect();
    let backlog: Vec<&Held> = memories.iter().filter(|held| !in_day(held) && !is_open_sort(held.memory.sort) && !is_sleep_made(held)).collect();
    let tide: Vec<Held> = if backlog.is_empty() {
        Vec::new()
    } else {
        let offset = earlier.len() * TIDE % backlog.len();
        (0..TIDE.min(backlog.len())).map(|step| backlog[(offset + step) % backlog.len()].clone()).collect()
    };
    let all = memories.into_iter().map(|held| (held.id.clone(), held)).collect();
    Ok(Context { now, label: label_of(run).to_string(), counts, day, open, tide, all })
}

fn age(now: DateTime<Utc>, at: DateTime<Utc>) -> String {
    let minutes = (now - at).num_minutes().max(0);
    match minutes {
        0..=59 => format!("{minutes}m"),
        60..=2879 => format!("{}h", minutes / 60),
        _ => format!("{}d", minutes / 1440),
    }
}

fn entry(now: DateTime<Utc>, held: &Held) -> String {
    let memory = &held.memory;
    let about = if memory.about.is_empty() { String::new() } else { format!("; about {}", memory.about.iter().map(|who| who.0.as_str()).collect::<Vec<_>>().join(", ")) };
    let sources = if memory.sources.is_empty() { String::new() } else { format!("; sources {}", memory.sources.iter().map(|source| source.0.as_str()).collect::<Vec<_>>().join(", ")) };
    format!(
        "- {} [{:?}, {} ago, salience {}] {}{about}{sources}\n  {}\n",
        held.id,
        memory.sort,
        age(now, held.at),
        memory.salience_permille,
        recall::origin_phrase(&memory.origin, &BTreeMap::new()),
        memory.text.0
    )
}

/// The night line: the date, her counts in force, her day whole, her open items, the tide, and the guidance.
pub(crate) fn night_line(context: &Context) -> String {
    let now = context.now;
    let counts = if context.counts.is_empty() { "none".to_string() } else { context.counts.iter().map(|(sort, count)| format!("{sort} {count}")).collect::<Vec<_>>().join(", ") };
    let mut line = format!("It is night, {} UTC.\n\nYour memories in force: {counts}.\n\n", now.format("%Y-%m-%d %H:%M"));
    if context.day.is_empty() {
        line.push_str("Your day: nothing new has been admitted since your last night.\n\n");
    } else {
        line.push_str(&format!("Your day, whole, oldest first ({} memories):\n", context.day.len()));
        context.day.iter().for_each(|held| line.push_str(&entry(now, held)));
        line.push('\n');
    }
    if !context.open.is_empty() {
        line.push_str(&format!("Your open asks and commitments from before your day, oldest first ({}):\n", context.open.len()));
        context.open.iter().for_each(|held| line.push_str(&entry(now, held)));
        line.push('\n');
    }
    if !context.tide.is_empty() {
        line.push_str(&format!("Older memories of yours, as they come round ({}):\n", context.tide.len()));
        context.tide.iter().for_each(|held| line.push_str(&entry(now, held)));
        line.push('\n');
    }
    line.push_str(SLEEP_GUIDANCE);
    line
}

fn bytes_fault(field: &str, text: &str, most: usize) -> Option<String> {
    (text.trim().is_empty() || text.len() > most).then(|| format!("{field}: empty, or over {most} bytes"))
}

/// The reply decoded and held to the ledger. The fault is told back to her whole.
pub(crate) fn decode(text: &str, context: &Context) -> std::result::Result<SleepReply, String> {
    let reply: SleepReply = serde_json::from_str(text).map_err(|error| format!("the reply is not a SleepReply: {error}"))?;
    let mut faults: Vec<String> = Vec::new();
    faults.extend(bytes_fault("night", &reply.night, NIGHT_MAX_BYTES));
    if reply.consolidate.len() > CONSOLIDATE_MAX || reply.retire.len() > RETIRE_MAX || reply.remember.len() > REMEMBER_MAX {
        faults.push(format!("at most {CONSOLIDATE_MAX} consolidations, {RETIRE_MAX} retirements and {REMEMBER_MAX} remembered"));
    }
    let shown: BTreeSet<String> = context.day.iter().chain(&context.open).chain(&context.tide).flat_map(|held| recall::people_of(&held.memory)).collect();
    let about_fault = |about: &[String]| about.len() > ABOUT_MAX || about.iter().any(|who| !shown.contains(who));
    for consolidation in &reply.consolidate {
        if consolidation.consumes.is_empty() || consolidation.consumes.len() > CONSUMES_MAX {
            faults.push(format!("consumes: between 1 and {CONSUMES_MAX} ids"));
        }
        faults.extend(bytes_fault("consolidate.text", &consolidation.text, REMEMBER_TEXT_MAX_BYTES));
        if consolidation.salience_permille > SALIENCE_MAX_PERMILLE || about_fault(&consolidation.about) {
            faults.push("consolidate.about or salience_permille: out of bounds (about is an id of a person a memory shown to you names)".into());
        }
        let conversations: Vec<Conversation> = consolidation.consumes.iter().filter_map(|id| context.all.get(id)).map(|held| Conversation::of_origin(Some(&held.memory.origin))).collect();
        if conversations.windows(2).any(|pair| pair[0] != pair[1]) {
            faults.push(format!("a consolidation merges the memories of one conversation only: {} are of two", consolidation.consumes.join(", ")));
        }
    }
    for retirement in &reply.retire {
        if retirement.why.trim().is_empty() || retirement.why.chars().count() > WHY_MAX_CHARS {
            faults.push(format!("retire {}: why is empty or over {WHY_MAX_CHARS} characters", retirement.item));
        }
    }
    for remembered in &reply.remember {
        faults.extend(bytes_fault("remember.text", &remembered.text, REMEMBER_TEXT_MAX_BYTES));
        if remembered.salience_permille > SALIENCE_MAX_PERMILLE || about_fault(&remembered.about) {
            faults.push("remember.about or salience_permille: out of bounds (about is an id of a person a memory shown to you names)".into());
        }
    }
    let named: Vec<&String> = reply.keep.iter().chain(reply.consolidate.iter().flat_map(|consolidation| &consolidation.consumes)).chain(reply.retire.iter().map(|retirement| &retirement.item)).collect();
    let mut seen = BTreeSet::new();
    for id in &named {
        if !seen.insert(id.as_str()) {
            faults.push(format!("{id} is named more than once; every memory is named once in the whole reply"));
        }
    }
    for id in &named {
        if !context.all.contains_key(id.as_str()) {
            faults.push(format!("{id} is not a memory of yours in force"));
        }
    }
    let missing: Vec<&str> = context.day.iter().map(|held| held.id.as_str()).filter(|id| !seen.contains(id)).collect();
    if !missing.is_empty() {
        faults.push(format!("your day is not covered; dispose of each of these in keep, consumes or retire: {}", missing.join(", ")));
    }
    if faults.is_empty() { Ok(reply) } else { Err(faults.join("\n")) }
}

fn reference(id: &str) -> PipelineRef {
    PipelineRef { kind: PipelineKind::Memory, id: Short(id.to_string()) }
}

/// The reply as whole operations, each a unit of documents that stands or falls together: a consolidation (its
/// memory and a Superseded resolution of each memory it consumes), a retirement, a remembered memory.
pub(crate) fn operations(mind: &dyn MindPort, context: &Context, reply: &SleepReply) -> Result<Vec<Vec<PipelineDocument>>> {
    let instance = mind.instance().clone();
    let on = date(context.now);
    let run = Short(context.label.clone());
    let mut units = Vec::new();
    for (index, consolidation) in reply.consolidate.iter().enumerate() {
        let mut consumed: Vec<&Held> = consolidation.consumes.iter().filter_map(|id| context.all.get(id)).collect();
        consumed.sort_by_key(|held| held.ordinal);
        let origin = match Conversation::of_origin(consumed.first().map(|held| &held.memory.origin)) {
            Conversation::Direct(room) => {
                let mut with: Vec<Short> = Vec::new();
                for who in consumed.iter().flat_map(|held| match &held.memory.origin {
                    MemoryOrigin::Direct { with, .. } => with.clone(),
                    _ => Vec::new(),
                }) {
                    if !with.contains(&who) && with.len() < ABOUT_MAX {
                        with.push(who);
                    }
                }
                MemoryOrigin::Direct { room: Short(room), with }
            }
            Conversation::Public => MemoryOrigin::Sleep { run: run.clone() },
        };
        let mut sources: Vec<Short> = Vec::new();
        for source in consumed.iter().rev().flat_map(|held| held.memory.sources.iter().rev()) {
            if !sources.contains(source) && sources.len() < SOURCES_MAX {
                sources.push(source.clone());
            }
        }
        let label = Label(format!("{}-c{index}", context.label));
        let memory = PipelineMemory {
            instance: instance.clone(),
            label: label.clone(),
            sort: consolidation.sort,
            text: Line(consolidation.text.clone()),
            about: consolidation.about.iter().cloned().map(Short).collect(),
            origin,
            sources,
            salience_permille: consolidation.salience_permille,
            consolidates: consolidation.consumes.iter().map(|id| reference(id)).collect(),
        };
        let by = reference(&format!("{}:memory:{}", instance.0, label.0));
        let mut unit = vec![PipelineDocument::Memory(memory)];
        for id in &consolidation.consumes {
            let subject = reference(id);
            unit.push(PipelineDocument::Resolution(PipelineResolution {
                sequence: next_sequence(mind, &subject)?,
                subject,
                outcome: ResolutionOutcome::Superseded { by: vec![by.clone()] },
                rationale: "Consolidated in sleep.".into(),
                resolved_on: on.clone(),
            }));
        }
        units.push(unit);
    }
    for retirement in &reply.retire {
        let subject = reference(&retirement.item);
        let reason = Line(retirement.why.clone());
        units.push(vec![PipelineDocument::Resolution(PipelineResolution {
            sequence: next_sequence(mind, &subject)?,
            subject,
            outcome: match retirement.outcome {
                RetireOutcome::Done | RetireOutcome::Faded => ResolutionOutcome::Recorded { reason },
                RetireOutcome::Declined | RetireOutcome::Wrong => ResolutionOutcome::Withdrawn { reason },
            },
            rationale: retirement.why.as_str().into(),
            resolved_on: on.clone(),
        })]);
    }
    for (index, remembered) in reply.remember.iter().enumerate() {
        units.push(vec![PipelineDocument::Memory(PipelineMemory {
            instance: instance.clone(),
            label: Label(format!("{}-r{index}", context.label)),
            sort: remembered.sort,
            text: Line(remembered.text.clone()),
            about: remembered.about.iter().cloned().map(Short).collect(),
            origin: MemoryOrigin::Sleep { run: run.clone() },
            sources: Vec::new(),
            salience_permille: remembered.salience_permille,
            consolidates: Vec::new(),
        })]);
    }
    Ok(units)
}

/// Operations packed in order into batches of at most `BATCH_MAX`, none split.
pub(crate) fn pack(units: Vec<Vec<PipelineDocument>>) -> Vec<Vec<PipelineDocument>> {
    let mut batches: Vec<Vec<PipelineDocument>> = Vec::new();
    for unit in units {
        match batches.last_mut() {
            Some(last) if last.len() + unit.len() <= BATCH_MAX => last.extend(unit),
            _ => batches.push(unit),
        }
    }
    batches
}

/// How a night ended.
enum Finish {
    /// `through` is the ordinal of the last memory of the day the night was shown, if it was shown any.
    Slept { reply: SleepReply, through: Option<u64> },
    Failed { refusals: u32, why: String },
}

struct SleepInbox<'a, C, S> {
    organs: &'a Organs<'a>,
    control: &'a C,
    swarm: &'a S,
    context: &'a Context,
    line: String,
    told: Option<String>,
    refusals: u32,
    finish: Option<Finish>,
}

impl<C: ControlSource, S: ControlSource> SleepInbox<'_, C, S> {
    /// Her reply admitted whole operations at a time, in batches; a refused batch ends the night, the earlier ones standing.
    fn admit(&self, reply: &SleepReply) -> Result<()> {
        let batches = pack(operations(self.organs.mind, self.context, reply)?);
        for batch in batches {
            match self.organs.mind.admit(AGENT, &self.context.label, batch)? {
                PipelineAdmissionOutcome::Committed { .. } | PipelineAdmissionOutcome::AlreadyAdmitted { .. } => {}
                refused => bail!("her mind refused a batch of the night: {refused:?}"),
            }
        }
        Ok(())
    }

    fn refuse(&mut self, fault: String) -> (bool, bool) {
        self.refusals += 1;
        if self.refusals >= REFUSALS_PER_NIGHT {
            self.finish = Some(Finish::Failed { refusals: self.refusals, why: fault });
            return (false, true);
        }
        self.told = Some(format!("Your reply was not taken:\n{fault}\n\nReply again, whole."));
        (true, false)
    }
}

impl<C: ControlSource, S: ControlSource> SessionInbox for SleepInbox<'_, C, S> {
    fn start_line(&mut self) -> Result<String> {
        Ok(self.line.clone())
    }

    fn poll_every_s(&self) -> i64 {
        PERSONA_POLL_S
    }

    /// No chat is read into the night or acked by it; the brakes still stop it.
    fn poll(&mut self) -> Result<Polled> {
        if let crate::control::Effective::Held { reason } = crate::persona::read(self.control, self.swarm, self.organs.mind.instance()) {
            self.finish.get_or_insert(Finish::Failed { refusals: self.refusals, why: format!("held: {reason:?}") });
        }
        Ok(Polled { pending: self.told.is_some(), other: false, other_first: false, yield_now: self.finish.is_some() })
    }

    fn tell(&mut self, _switch: bool) -> Result<String> {
        Ok(self.told.take().unwrap_or_default())
    }

    fn flush_text(&mut self, _why: Why) -> String {
        "Your time is nearly spent. Reply now with your whole SleepReply: every memory of your day disposed of, and the night in one line.".into()
    }

    fn result(&mut self, line: &str, ok: bool, blocked: bool) -> Result<(bool, bool)> {
        if self.finish.is_some() {
            return Ok((false, true));
        }
        if blocked || !ok {
            self.finish = Some(Finish::Failed { refusals: self.refusals, why: "the session failed".into() });
            return Ok((false, true));
        }
        let Some(text) = structured_output(line) else { return Ok(self.refuse("the result carried no reply".into())) };
        match decode(&text, self.context) {
            Ok(reply) => match self.admit(&reply) {
                Ok(()) => {
                    self.finish = Some(Finish::Slept { reply, through: self.context.day.last().map(|held| held.ordinal) });
                    Ok((true, false))
                }
                Err(error) => {
                    self.finish = Some(Finish::Failed { refusals: self.refusals, why: error.to_string() });
                    Ok((false, true))
                }
            },
            Err(fault) => Ok(self.refuse(fault)),
        }
    }
}

/// What a night closes with: the reason (her line, or the failure) and the rationale (the counts, and where her
/// day was covered through).
fn closing(finish: &Finish) -> (String, String) {
    match finish {
        Finish::Slept { reply, through } => (
            format!("{SLEPT}{}", reply.night),
            format!(
                "kept {}, consolidated {} from {}, retired {}, formed {}{}",
                reply.keep.len(),
                reply.consolidate.len(),
                reply.consolidate.iter().map(|consolidation| consolidation.consumes.len()).sum::<usize>(),
                reply.retire.len(),
                reply.remember.len(),
                through.map_or(String::new(), |ordinal| format!("{THROUGH}{ordinal}"))
            ),
        ),
        Finish::Failed { refusals, why } => (format!("failed after {refusals} refusals: {why}"), "The night ended without a reply the ledger took.".to_string()),
    }
}

/// Closes the night's run Recorded with her line and the counts, or the failure.
fn close(organs: &Organs, run: &PipelineRef, finish: &Finish) -> Result<()> {
    let (reason, rationale) = closing(finish);
    let resolution = PipelineResolution {
        subject: run.clone(),
        sequence: next_sequence(organs.mind, run)?,
        outcome: ResolutionOutcome::Recorded { reason: reason.as_str().into() },
        rationale: rationale.as_str().into(),
        resolved_on: date(organs.clock.now()),
    };
    match organs.mind.admit(AGENT, label_of(run), vec![PipelineDocument::Resolution(resolution)])? {
        PipelineAdmissionOutcome::Committed { .. } => Ok(()),
        _ => bail!("the Sleep run could not be closed"),
    }
}

/// One night, in the Persona unit, under the Sleep run the heartbeat opened: one process, no retry.
pub(crate) fn carry_night(organs: &Organs, control: &impl ControlSource, swarm: &impl ControlSource, config: &Config, run: &PipelineRef) -> Result<Outcome> {
    let remaining_s = organs.run_cap_s.min(SLEEP_CAP_S);
    let finish = match context(organs.mind, run, organs.clock.now()) {
        Err(error) => Finish::Failed { refusals: 0, why: format!("her mind could not be read for the night: {error}") },
        Ok(context) => {
            let session_id = uuid::Uuid::new_v5(&SESSION_NAMESPACE, format!("{}:sleep", run.id.0).as_bytes());
            match start_session(organs, config, session_id, remaining_s, |_| sleep_schema())? {
                Err(why) => Finish::Failed { refusals: 0, why: format!("the night could not start: {why}") },
                Ok(Started { child, lease, .. }) => {
                    let mut inbox = SleepInbox { organs, control, swarm, context: &context, line: night_line(&context), told: None, refusals: 0, finish: None };
                    let driven = drive(organs.clock, child, &mut inbox, remaining_s, &mut std::io::stdout());
                    drop(lease);
                    match (inbox.finish.take(), driven) {
                        (Some(finish), _) => finish,
                        (None, Err(error)) => Finish::Failed { refusals: inbox.refusals, why: format!("the session broke: {error}") },
                        (None, Ok(_)) => Finish::Failed { refusals: inbox.refusals, why: "the night ended without an answer".into() },
                    }
                }
            }
        }
    };
    close(organs, run, &finish)?;
    let failure = matches!(finish, Finish::Failed { .. }).then_some(Failure::Session);
    Ok(Outcome::Served { run: run.clone(), answered: u32::from(failure.is_none()), failure, self_run: None })
}

#[cfg(test)]
mod tests;
