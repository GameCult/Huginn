//! The persona-seed harness: `project` run over fixtures instead of her live body, so a candidate state can be judged
//! before any Mind holds it.
//!
//! - A **room fixture** (`mind-persona project --room <file>`) is a TOML file standing in for the mouth and the room tail:
//!   her Matrix id, the rooms, the messages with who said them, when and where, and which were new to her, plus a mocked
//!   clock and a mocked "last heard from" table. The fixture is a `MouthReader`, so `project` reads it exactly as it reads
//!   the mouth, and whether a message is for her is still `address::is_for`, over the fixture's names.
//! - A **batch** (`--batch <plan>`) runs `project` once per state, scenario and sample and writes one Markdown report.
//! - A **simulation** (`--simulate <plan>`, `simulate.rs`) runs the cast of the swarm in turn in one simulated room.
//!
//! Every case is one call of `project`: experiments and the live turn share one code path. None of this can write: the
//! organs are `project`'s read-only ports, a state file is read and never put, and nothing reaches the mouth.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use eureka_pipeline::Slug;
use serde::Deserialize;
use serde_json::{Value, json};

use super::narrate::{Model, distinct_2};
use super::{MouthReader, Organs, PersonaSource, Request, Through, TurnKind, project};
use crate::address::{Addressee, localpart};
use crate::chat_client::{Basis, ChatDelivery, ChatPending, Form, TailLine};
use crate::heartbeat::{Addressing, HeartbeatView};

pub use super::simulate::simulate;

/// A narration whose word trigrams overlap what she heard past this share is marked as a high lexical overlap.
pub(crate) const OVERLAP_FLAG: f64 = 0.15;
/// The most room lines a view's tail carries, as a mouth's tail is bounded.
const TAIL_LINES: usize = 40;

/// A persona state file, checked as a put would check its content (huginn-mind's rules P1 to P3: the published schema,
/// `personaId` is the instance, canonical provenance). A file this accepts is one a put would not refuse for what it says.
/// P4 and P5 are not checked: they compare `updatedAt` with the stored document, and no store is read here.
pub fn state_file(path: &Path, instance: &str) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("{} could not be read", path.display()))?;
    let state: Value = serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    huginn_mind::persona::validate_document(&Slug(instance.to_owned()), &state).map_err(|refusal| anyhow!("{} would be refused by a put: {refusal}", path.display()))?;
    Ok(state)
}

/// "90s", "3m", "26h" or "2d": how long ago.
fn ago(text: &str) -> Result<Duration> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    let amount: i64 = text[..split].parse().map_err(|_| anyhow!("\"{text}\" is not a time like 90s, 3m, 26h or 2d"))?;
    Ok(match &text[split..] {
        "s" => Duration::seconds(amount),
        "m" => Duration::minutes(amount),
        "h" => Duration::hours(amount),
        "d" => Duration::days(amount),
        _ => bail!("\"{text}\" is not a time like 90s, 3m, 26h or 2d"),
    })
}

fn unix_ms(at: DateTime<Utc>) -> u64 {
    u64::try_from(at.timestamp_millis()).unwrap_or_default()
}

fn matrix_id(id: &str) -> bool {
    id.starts_with('@') && id.contains(':')
}

/// One message as a fixture or a simulation script writes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// The sender's Matrix id.
    pub from: String,
    /// The sender's display name.
    #[serde(default)]
    pub name: String,
    pub room: String,
    /// How long before the fixture's `now` it was said: "90s", "3m", "26h", "2d". Room fixtures only.
    #[serde(default)]
    pub ago: Option<String>,
    /// The round it is said at; 0 is before the first round. Simulations only.
    #[serde(default)]
    pub round: Option<usize>,
    pub says: String,
    /// New to her this turn: delivered by the mouth, not only in the room's tail. Room fixtures only.
    #[serde(default)]
    pub new: bool,
    /// Tags her (`m.mentions` holds her id).
    #[serde(default)]
    pub tag: bool,
    /// Other Matrix ids it tags.
    #[serde(default)]
    pub mentions: Vec<String>,
    /// A direct message: to her in a room fixture; to `to` in a simulation script.
    #[serde(default)]
    pub dm: bool,
    /// The recipient of a direct message, by Matrix id. A room fixture's defaults to her; a script must name one.
    #[serde(default)]
    pub to: Option<String>,
    /// A reply in a conversation she opened (the mouth's follow-up basis).
    #[serde(default)]
    pub follow_up: bool,
    /// The number (from 1, in file order) of the earlier message it replies to; Matrix calls it a reply to her when
    /// that message was hers.
    #[serde(default)]
    pub reply_to: Option<usize>,
}

/// A message placed in the room: when it was said, and whom it tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub from: String,
    pub name: String,
    pub room: String,
    pub at_ms: u64,
    pub says: String,
    pub mentions: Vec<String>,
    /// A direct message, to this Matrix id: only its sender and its recipient see it.
    pub dm_to: Option<String>,
    pub follow_up: bool,
    /// The index of the line it replies to.
    pub reply_to: Option<usize>,
    pub new: bool,
}

impl Line {
    /// A line from a script message; `her` is whom `tag` names.
    pub(crate) fn of(message: &Message, at: DateTime<Utc>, her: Option<&str>, index: usize) -> Result<Self> {
        if !matrix_id(&message.from) {
            bail!("message {} is from \"{}\", which is not a Matrix id", index + 1, message.from);
        }
        let mut mentions = message.mentions.clone();
        if message.tag {
            mentions.push(her.ok_or_else(|| anyhow!("message {} tags her, and a simulation has no one \"her\": name the ids in mentions", index + 1))?.to_owned());
        }
        let dm_to = match (message.dm, &message.to) {
            (true, Some(to)) if matrix_id(to) => Some(to.clone()),
            (true, Some(to)) => bail!("message {} is a direct message to \"{to}\", which is not a Matrix id", index + 1),
            (true, None) => Some(her.ok_or_else(|| anyhow!("message {} is a direct message to no one: a script names its recipient in `to`", index + 1))?.to_owned()),
            (false, Some(_)) => bail!("message {} names a recipient but is not a direct message", index + 1),
            (false, None) => None,
        };
        let reply_to = match message.reply_to {
            Some(number) if number >= 1 && number <= index => Some(number - 1),
            Some(number) => bail!("message {} replies to message {number}, which is not an earlier one", index + 1),
            None => None,
        };
        Ok(Self { from: message.from.clone(), name: message.name.clone(), room: message.room.clone(), at_ms: unix_ms(at), says: message.says.clone(), mentions, dm_to, follow_up: message.follow_up, reply_to, new: message.new })
    }

    /// What Matrix would say about this line to `viewer`: a direct message, a tag, a reply to a line of hers, or a
    /// follow-up.
    fn basis(&self, viewer: &str, lines: &[Line]) -> Option<Basis> {
        if self.dm_to.as_deref() == Some(viewer) {
            Some(Basis::Direct)
        } else if self.mentions.iter().any(|id| id == viewer) {
            Some(Basis::Tag)
        } else if self.reply_to.and_then(|at| lines.get(at)).is_some_and(|replied| replied.from == viewer) {
            Some(Basis::Reply)
        } else if self.follow_up {
            Some(Basis::FollowUp)
        } else {
            None
        }
    }

    /// Whether `viewer` sees the line: a direct message only its sender and recipient; any other line every member of its
    /// room (no rooms named is every room).
    pub(crate) fn visible_to(&self, viewer: &str, rooms: &[String]) -> bool {
        match &self.dm_to {
            Some(to) => to == viewer || self.from == viewer,
            None => rooms.is_empty() || rooms.contains(&self.room),
        }
    }

    pub(crate) fn delivery(&self, index: usize, viewer: &str, lines: &[Line]) -> ChatDelivery {
        ChatDelivery {
            seq: index as u64 + 1,
            room_alias: self.room.clone(),
            room_id: self.room.clone(),
            event_id: format!("$fixture-{index}"),
            sender: self.from.clone(),
            sender_display: self.name.clone(),
            text: self.says.clone(),
            at_unix_ms: self.at_ms,
            context: vec![],
            mentions: self.mentions.clone(),
            is_direct: self.dm_to.is_some(),
            image: None,
            basis: self.basis(viewer, lines),
            form: Form::Message,
            reply_to: Default::default(),
        }
    }
}

/// The mouth's pending read as `viewer` would get it from `lines`: the lines `new` says she has not had, from others,
/// in her rooms, as deliveries; and every line of her rooms, hers included, as the tail.
pub(crate) fn pending_for(lines: &[Line], viewer: &str, rooms: &[String], new: impl Fn(usize) -> bool) -> ChatPending {
    let hers = |line: &Line| line.visible_to(viewer, rooms);
    let deliveries = lines.iter().enumerate().filter(|(at, line)| new(*at) && hers(line) && line.from != viewer).map(|(at, line)| line.delivery(at, viewer, lines)).collect();
    let tail: Vec<TailLine> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| hers(line))
        .map(|(at, line)| TailLine { room: line.room.clone(), event_id: format!("$fixture-{at}"), at_unix_ms: line.at_ms, sender: line.from.clone(), form: Form::Message, basis: line.basis(viewer, lines), body: line.says.clone() })
        .collect();
    let tail = tail[tail.len().saturating_sub(TAIL_LINES)..].to_vec();
    ChatPending { persona: localpart(viewer).to_owned(), deliveries, tail: Some(tail), tail_gaps: vec![] }
}

// ---------------------------------------------------------------------------------------------
// The room fixture.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFile {
    her: String,
    #[serde(default)]
    names: Vec<String>,
    kind: TurnKind,
    now: DateTime<Utc>,
    rooms: Vec<String>,
    #[serde(default)]
    here: Option<String>,
    #[serde(default)]
    operators: Vec<String>,
    #[serde(default)]
    minds: Vec<String>,
    #[serde(default)]
    last_heard: BTreeMap<String, String>,
    #[serde(default, rename = "message")]
    messages: Vec<Message>,
}

/// A mocked room: it stands in for the mouth and the room tail, the clock, and who is who.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// Her Matrix id.
    pub her: String,
    /// More names she answers to, beside her handle; none leaves her own names (handle and public name).
    pub names: Vec<String>,
    pub kind: TurnKind,
    pub now: DateTime<Utc>,
    pub here: Option<String>,
    pub operators: Vec<String>,
    pub minds: Vec<String>,
    pub last_heard: Vec<(String, u64)>,
    lines: Vec<Line>,
}

impl Fixture {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("the room fixture {} could not be read", path.display()))?;
        Self::parse(&text).with_context(|| format!("the room fixture {} is not a room", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let file: FixtureFile = toml::from_str(text)?;
        if !matrix_id(&file.her) {
            bail!("her = \"{}\" is not a Matrix id", file.her);
        }
        if file.rooms.is_empty() {
            bail!("rooms names no room");
        }
        if let Some(here) = &file.here
            && !file.rooms.contains(here)
        {
            bail!("here = \"{here}\" is not one of the rooms");
        }
        let mut lines = Vec::new();
        for (index, message) in file.messages.iter().enumerate() {
            if message.round.is_some() {
                bail!("message {} has a round: a room fixture's messages say how long ago, not which round", index + 1);
            }
            if !message.dm && !file.rooms.contains(&message.room) {
                bail!("message {} is in {}, which is not one of the rooms", index + 1, message.room);
            }
            let at = file.now - ago(message.ago.as_deref().unwrap_or("0s"))?;
            lines.push(Line::of(message, at, Some(&file.her), index)?);
        }
        let last_heard = file.last_heard.iter().map(|(id, when)| Ok((id.clone(), unix_ms(file.now - ago(when)?)))).collect::<Result<_>>()?;
        Ok(Self { her: file.her, names: file.names, kind: file.kind, now: file.now, here: file.here, operators: file.operators, minds: file.minds, last_heard, lines })
    }

    /// Her addressee, as the heartbeat's table would carry it, when the fixture names her: her handle and its names.
    /// None leaves `project` her own names.
    pub fn addressee(&self) -> Option<&dyn HeartbeatView> {
        (!self.names.is_empty()).then_some(self as &dyn HeartbeatView)
    }
}

impl MouthReader for Fixture {
    fn pending(&self) -> Result<ChatPending> {
        Ok(pending_for(&self.lines, &self.her, &[], |at| self.lines[at].new))
    }
}

impl HeartbeatView for Fixture {
    fn addressing(&self) -> Result<Option<Addressing>> {
        let names = std::iter::once(localpart(&self.her)).chain(self.names.iter().map(String::as_str));
        Ok(Some(Addressing { minds: vec![Addressee::new(&self.her, names)] }))
    }
}

// ---------------------------------------------------------------------------------------------
// What every harness run shares.

/// What a harness run reads through beside its fixtures: her stored state (the persona door; only a batch state without
/// a file asks), the model, and her tuning.
pub struct Harness<'a> {
    pub instance: String,
    pub persona: Option<&'a dyn PersonaSource>,
    pub model: Option<&'a dyn Model>,
    pub tuning: PathBuf,
}

/// One turn's `project` request over a fixture.
pub fn fixture_request(harness: &Harness, fixture: &Fixture, state: Option<Value>, seed: u64, renders: Option<usize>, through: Through) -> Request {
    Request {
        instance: harness.instance.clone(),
        kind: fixture.kind,
        state,
        room: fixture.here.clone(),
        seed,
        renders,
        judge_against: None,
        operators: fixture.operators.clone(),
        minds: fixture.minds.clone(),
        own_sender: fixture.her.clone(),
        tuning: harness.tuning.clone(),
        through,
        last_heard: fixture.last_heard.clone(),
        recent: vec![],
        own_rooms: vec![],
    }
}

// ---------------------------------------------------------------------------------------------
// Batch.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchFile {
    #[serde(default = "one")]
    samples: usize,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    through: Through,
    #[serde(rename = "state")]
    states: Vec<StateEntry>,
    #[serde(rename = "scenario")]
    scenarios: Vec<ScenarioEntry>,
}

fn one() -> usize {
    1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateEntry {
    name: String,
    /// A state file, relative to the plan; none is her stored state, read through the persona door.
    #[serde(default)]
    file: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioEntry {
    name: String,
    /// A room fixture, relative to the plan.
    room: PathBuf,
}

fn relative(plan: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_owned() } else { plan.parent().unwrap_or(Path::new(".")).join(path) }
}

/// Every state by every scenario by every sample, each one `project`, as one Markdown report. A case that fails is
/// reported in its place and the batch goes on.
pub fn batch(harness: &Harness, plan_path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(plan_path).with_context(|| format!("the plan {} could not be read", plan_path.display()))?;
    let plan: BatchFile = toml::from_str(&text).with_context(|| format!("the plan {} is not a batch plan", plan_path.display()))?;
    if plan.samples == 0 || plan.states.is_empty() || plan.scenarios.is_empty() {
        bail!("the plan {} needs a state, a scenario and at least one sample", plan_path.display());
    }
    let states: Vec<(String, Option<Value>)> = plan.states.iter().map(|entry| Ok((entry.name.clone(), entry.file.as_ref().map(|file| state_file(&relative(plan_path, file), &harness.instance)).transpose()?))).collect::<Result<_>>()?;
    let scenarios: Vec<(String, Fixture)> = plan.scenarios.iter().map(|entry| Ok((entry.name.clone(), Fixture::load(&relative(plan_path, &entry.room))?))).collect::<Result<_>>()?;

    let mut cases = Vec::new();
    for (state_name, state) in &states {
        for (scenario_name, fixture) in &scenarios {
            for sample in 0..plan.samples {
                let seed = plan.seed.wrapping_add(sample as u64);
                let organs = Organs { persona: harness.persona, chat: Some(fixture), addressing: fixture.addressee(), model: harness.model };
                let request = fixture_request(harness, fixture, state.clone(), seed, Some(1), plan.through);
                let report = project(&organs, &request, fixture.now).unwrap_or_else(|fault| json!({ "error": format!("{fault:#}") }));
                cases.push(json!({ "state": state_name, "scenario": scenario_name, "sample": sample + 1, "seed": seed, "report": report }));
            }
        }
    }
    Ok(batch_markdown(plan_path, &plan, &cases))
}

fn render_of(case: &Value) -> &Value {
    &case["report"]["renders"][0]
}

fn prose_of(case: &Value) -> String {
    render_of(case)["persona"]["prose"].as_str().unwrap_or_default().to_owned()
}

fn batch_markdown(plan_path: &Path, plan: &BatchFile, cases: &[Value]) -> String {
    let mut out = String::new();
    let first = cases.iter().find(|case| case["report"].get("error").is_none()).map(|case| &case["report"]);
    let _ = writeln!(out, "# Persona seed batch\n");
    let _ = writeln!(out, "- plan: `{}`", plan_path.display());
    let _ = writeln!(out, "- states: {}; scenarios: {}; samples: {}; through: {:?}", plan.states.len(), plan.scenarios.len(), plan.samples, plan.through);
    if let Some(report) = first {
        let _ = writeln!(out, "- instance: `{}`; tuning: {} (`{}`)", report["instance"].as_str().unwrap_or_default(), report["tuning"]["source"].as_str().unwrap_or_default(), report["tuning"]["sha256"].as_str().unwrap_or_default());
        let _ = writeln!(out, "- Projector model: `{}`; Persona model: `{}`", report["model"].as_str().unwrap_or("none"), report["persona_model"].as_str().unwrap_or("none"));
    }
    let _ = writeln!(out, "- would-say lines are the double-quoted spans of her prose: a preview of what the Interpreter (C3b) will quote, not its decision");
    let _ = writeln!(out, "- read-only: no put, no trace, no send; the mouth is the fixture\n");

    // The summary: each state by scenario, then the states' voices against each other.
    let _ = writeln!(out, "## Summary\n\n| state | scenario | spoke | mean narration words | mean prose words |\n|---|---|---|---|---|");
    for state in &plan.states {
        for scenario in &plan.scenarios {
            let mine: Vec<&Value> = cases.iter().filter(|case| case["state"] == state.name.as_str() && case["scenario"] == scenario.name.as_str()).collect();
            let spoke = mine.iter().filter(|case| render_of(case)["interpreter"]["said"].as_array().is_some_and(|said| !said.is_empty())).count();
            let mean = |key: &dyn Fn(&Value) -> Option<u64>| mine.iter().filter_map(|case| key(render_of(case))).sum::<u64>() as f64 / mine.len().max(1) as f64;
            let _ = writeln!(out, "| {} | {} | {spoke}/{} | {:.0} | {:.0} |", state.name, scenario.name, mine.len(), mean(&|render| render["words"].as_u64()), mean(&|render| render["persona"]["words"].as_u64()));
        }
    }
    let corpora: Vec<(String, Vec<String>)> = plan.states.iter().map(|state| (state.name.clone(), cases.iter().filter(|case| case["state"] == state.name.as_str()).map(prose_of).filter(|prose| !prose.is_empty()).collect())).collect();
    let _ = writeln!(out, "\nDistinct-2 of her prose within each state (1 is no repeated word pair), and over all states pooled:\n");
    for (name, prose) in &corpora {
        let _ = writeln!(out, "- {name}: {:.3}", distinct_2(prose));
    }
    let _ = writeln!(out, "- pooled: {:.3}", distinct_2(&corpora.iter().flat_map(|(_, prose)| prose.clone()).collect::<Vec<_>>()));
    let _ = writeln!(out, "\nWord-use similarity between states' prose (cosine of word counts, 1 is the same words in the same proportions):\n");
    for (at, (a, a_prose)) in corpora.iter().enumerate() {
        for (b, b_prose) in &corpora[at + 1..] {
            let _ = writeln!(out, "- {a} / {b}: {:.3}", cosine(&a_prose.join(" "), &b_prose.join(" ")));
        }
    }

    for case in cases {
        let _ = writeln!(out, "\n## {} × {}, sample {} (seed {})\n", case["state"].as_str().unwrap_or_default(), case["scenario"].as_str().unwrap_or_default(), case["sample"], case["seed"]);
        let report = &case["report"];
        if let Some(error) = report.get("error") {
            let _ = writeln!(out, "**failed:** {}", error.as_str().unwrap_or_default());
            continue;
        }
        let packet = &report["packet"];
        let _ = writeln!(out, "- lens: `{}`; present: {}; room: {}", packet["lens"], packet["present"], report["room"]);
        let _ = writeln!(out, "\n<details><summary>packet</summary>\n");
        fenced(&mut out, "identity", packet["identity"].as_str().unwrap_or_default());
        fenced(&mut out, "context", packet["context"].as_str().unwrap_or_default());
        fenced(&mut out, "stimulus", packet["stimulus"].as_str().unwrap_or_default());
        fenced(&mut out, "heard", packet["heard"].as_str().unwrap_or_default());
        let _ = writeln!(out, "</details>\n");
        turn_markdown(&mut out, render_of(case));
    }
    let _ = writeln!(out, "\n## Total usage\n\n{}", usage_markdown(&usage_total(cases)));
    out
}

/// One render's narration, prose, would-say lines, word counts and usage.
pub(crate) fn turn_markdown(out: &mut String, render: &Value) {
    if let Some(failed) = render["failed"].as_str() {
        let _ = writeln!(out, "**Projector call failed:** {failed}

Usage: {}
", usage_line(&render["usage"]));
        return;
    }
    let _ = writeln!(out, "**Narration** ({} words{}):\n", render["words"], if render["cut_at_budget"] == true { ", cut at budget" } else { "" });
    quoted(out, render["narration"].as_str().unwrap_or_default());
    if let Some(names) = render["field_names"].as_array().filter(|names| !names.is_empty()) {
        let _ = writeln!(out, "**Field names leaked:** {}\n", Value::Array(names.clone()));
    }
    let _ = writeln!(
        out,
        "Narration checks: lexical overlap with what she heard {}{}{}\n",
        render["heard_overlap"].as_f64().map_or_else(|| "n/a".to_owned(), |overlap| format!("{overlap:.2}")),
        if render["heard_overlap"].as_f64().is_some_and(|overlap| overlap > OVERLAP_FLAG) { " (high)" } else { "" },
        if render["offers_options"] == true { "; offers her options" } else { "" }
    );
    let persona = &render["persona"];
    if let Some(failed) = persona["failed"].as_str() {
        let _ = writeln!(out, "**Persona call failed:** {failed}
");
    } else if !persona.is_null() {
        let _ = writeln!(out, "**Her prose** ({} words{}):\n", persona["words"], if persona["cut_at_budget"] == true { ", cut at budget" } else { "" });
        quoted(out, persona["prose"].as_str().unwrap_or_default());
    }
    interpreter_markdown(out, &render["interpreter"]);
    let _ = writeln!(out, "Usage: projector {}; persona {}; interpreter {}\n", usage_line(&render["usage"]), usage_line(&persona["usage"]), usage_line(&render["interpreter"]["usage"]));
}

/// The Interpreter's speech decision: each line said with its room, mentions, reply target and the span of her prose it
/// rests on; each line refused, with why; its gaps. None when the render did not run through the Interpreter.
pub(crate) fn interpreter_markdown(out: &mut String, decision: &Value) {
    if decision.is_null() {
        return;
    }
    if let Some(failed) = decision["failed"].as_str() {
        let _ = writeln!(out, "**Interpreter failed:** {failed}\n");
        return;
    }
    let said = decision["said"].as_array().cloned().unwrap_or_default();
    if said.is_empty() {
        let _ = writeln!(out, "**Interpreter: nothing said.**\n");
    } else {
        let _ = writeln!(out, "**Interpreter: said**\n");
        for line in &said {
            let mentions = line["mentions"].as_array().map(|ids| ids.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let reply = line["reply_to"].as_str().map_or_else(String::new, |id| format!("; reply to {id}"));
            let _ = writeln!(out, "- in {}{}{}: {}", line["room"].as_str().unwrap_or_default(), if mentions.is_empty() { String::new() } else { format!(", mentioning {mentions}") }, reply, line["text"].as_str().unwrap_or_default());
            let spans: Vec<String> = line["rests_on"].as_array().into_iter().flatten().map(|span| format!("\"{}\"", span["quote"].as_str().unwrap_or_default())).collect();
            let _ = writeln!(out, "  - her words: {}", spans.join(" + "));
        }
        out.push('\n');
    }
    for line in decision["refused"].as_array().into_iter().flatten() {
        let what = line["text"].as_str().map_or_else(|| line["spans"].to_string(), str::to_owned);
        let _ = writeln!(out, "- refused ({}): {what}", line["why"].as_str().unwrap_or_default());
    }
    for gap in decision["gaps"].as_array().into_iter().flatten() {
        let _ = writeln!(out, "- gap: {gap}");
    }
    out.push('\n');
}

fn fenced(out: &mut String, label: &str, text: &str) {
    if !text.is_empty() {
        let _ = writeln!(out, "{label}:\n\n````text\n{text}\n````\n");
    }
}

fn quoted(out: &mut String, text: &str) {
    for line in text.lines() {
        let _ = writeln!(out, "> {line}");
    }
    out.push('\n');
}

// ---------------------------------------------------------------------------------------------
// Usage and similarity, shared with the simulation.

const USAGE_FIELDS: [&str; 5] = ["input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens", "reasoning_tokens"];

/// Every `usage` object anywhere in `values`, summed per model and field, with the number of calls.
pub(crate) fn usage_total(values: &[Value]) -> BTreeMap<String, BTreeMap<String, u64>> {
    fn walk(value: &Value, into: &mut BTreeMap<String, BTreeMap<String, u64>>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    if key == "usage" && inner.is_object() {
                        let model = inner["model"].as_str().unwrap_or("unknown").to_owned();
                        let row = into.entry(model).or_default();
                        *row.entry("calls".into()).or_default() += 1;
                        for field in USAGE_FIELDS {
                            *row.entry(field.into()).or_default() += inner[field].as_u64().unwrap_or(0);
                        }
                    } else {
                        walk(inner, into);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|inner| walk(inner, into)),
            _ => {}
        }
    }
    let mut total = BTreeMap::new();
    values.iter().for_each(|value| walk(value, &mut total));
    total
}

pub(crate) fn usage_markdown(total: &BTreeMap<String, BTreeMap<String, u64>>) -> String {
    if total.is_empty() {
        return "No model call was counted.".into();
    }
    let mut out = String::from("| model | calls | input | output | cache read | cache write | reasoning |\n|---|---|---|---|---|---|---|\n");
    for (model, row) in total {
        let get = |field: &str| row.get(field).copied().unwrap_or(0);
        let _ = writeln!(out, "| {model} | {} | {} | {} | {} | {} | {} |", get("calls"), get("input_tokens"), get("output_tokens"), get("cache_read_tokens"), get("cache_write_tokens"), get("reasoning_tokens"));
    }
    out
}

fn usage_line(usage: &Value) -> String {
    if usage.is_null() {
        return "none".into();
    }
    format!("{} in, {} out, {} cache read", usage["input_tokens"], usage["output_tokens"], usage["cache_read_tokens"])
}

/// The words of a text as counted for similarity: lowercased, letters and digits, three characters or more.
fn word_counts(text: &str) -> BTreeMap<String, f64> {
    let mut counts = BTreeMap::new();
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|word| word.chars().count() >= 3) {
        *counts.entry(word.to_lowercase()).or_insert(0.0) += 1.0;
    }
    counts
}

/// The cosine of two texts' word counts: 1 is the same words in the same proportions, 0 no word shared. A cheap
/// lexical proxy for one voice, not a measure of meaning.
pub(crate) fn cosine(a: &str, b: &str) -> f64 {
    let (a, b) = (word_counts(a), word_counts(b));
    let dot: f64 = a.iter().filter_map(|(word, count)| b.get(word).map(|other| count * other)).sum();
    let norm = |counts: &BTreeMap<String, f64>| counts.values().map(|count| count * count).sum::<f64>().sqrt();
    let denominator = norm(&a) * norm(&b);
    if denominator == 0.0 { 0.0 } else { dot / denominator }
}

#[cfg(test)]
pub(crate) mod tests;
