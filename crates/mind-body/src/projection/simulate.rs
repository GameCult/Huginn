//! The swarm in one simulated room (`mind-persona project --simulate <plan>`): up to eight personas take turns, each turn
//! one `project` through the Persona stage, and each one's quoted lines join the room as from her Matrix id, so the
//! next one hears them.
//!
//! Who wakes is the real rule: `address::is_for` over each persona's names, plus ruling
//! `persona-only-wake-humans-in-rooms` (an addressed message, any human message in her rooms, the beat). Another Mind's
//! unaddressed message wakes no one, unless the plan asks for the swarm's old loop (`wake = "all_messages"`), kept as a
//! control. Those woken in one round go in an order drawn from the seed, each against the room as it stands at her turn.
//!
//! Her state does not move during a run: the Interpreter (C3b) and the commit (C3c) are not built. `state_after_turn`
//! is the seam where they will.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use super::harness::{Harness, Line, Message, cosine, pending_for, state_file, turn_markdown, usage_markdown, usage_total};
use super::select::Draw;
use super::{MouthReader, Organs, Request, Through, TurnKind, project};
use crate::address::{Addressee, is_for, localpart};
use crate::chat_client::ChatPending;
use crate::heartbeat::{Addressing, HeartbeatView};

/// The most personas a simulation holds: the swarm.
const CAST_MAX: usize = 8;

/// Who a message wakes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Wake {
    /// The ruled set: a message for her, any human message in her rooms, the beat.
    #[default]
    HumansInRooms,
    /// The swarm's old loop, as a control: any message in her rooms, a Mind's too.
    AllMessages,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SimFile {
    rounds: usize,
    #[serde(default = "one")]
    repeats: usize,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    wake: Wake,
    /// Every this many rounds the beat wakes everyone who was not woken otherwise; 0 is no beat.
    #[serde(default)]
    beat_every: usize,
    #[serde(default = "two")]
    round_minutes: i64,
    start: DateTime<Utc>,
    rooms: Vec<String>,
    #[serde(default)]
    operators: Vec<String>,
    cast: Vec<CastEntry>,
    #[serde(default, rename = "message")]
    script: Vec<Message>,
}

fn one() -> usize {
    1
}

fn two() -> i64 {
    2
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CastEntry {
    id: String,
    name: String,
    /// Her persona state, relative to the plan; its `personaId` is her id's localpart.
    state: PathBuf,
    /// Her rooms; none is every room of the plan.
    #[serde(default)]
    rooms: Vec<String>,
    /// More names she answers to, beside her handle and display name.
    #[serde(default)]
    names: Vec<String>,
}

struct Member {
    entry: CastEntry,
    state: Value,
    addressee: Addressee,
    rooms: Vec<String>,
}

/// One persona's view of the room at her turn: the lines since her last turn are new.
struct View<'a> {
    lines: &'a [Line],
    viewer: &'a str,
    rooms: &'a [String],
    seen: usize,
}

impl MouthReader for View<'_> {
    fn pending(&self) -> Result<ChatPending> {
        Ok(pending_for(self.lines, self.viewer, self.rooms, |at| at >= self.seen))
    }
}

struct Names<'a>(&'a Addressee);

impl HeartbeatView for Names<'_> {
    fn addressing(&self) -> Result<Option<Addressing>> {
        Ok(Some(Addressing { minds: vec![self.0.clone()] }))
    }
}

/// The seam where the Interpreter (C3b) will turn her prose into proposals and the commit (C3c) will apply them, so her
/// next turn starts from the state this one left. Until they exist it returns her state unchanged.
fn state_after_turn(state: Value, _turn: &Value) -> Value {
    state
}

/// Why she wakes this round, by the plan's rule; none leaves her asleep.
fn wake_reason(lines: &[Line], seen: usize, member: &Member, cast: &[String], wake: Wake, beat: bool) -> Option<&'static str> {
    let mut reason = None;
    for (at, line) in lines.iter().enumerate().skip(seen) {
        if line.from == member.entry.id || !(member.rooms.contains(&line.room) || line.dm) {
            continue;
        }
        if is_for(&member.addressee, &line.delivery(at, &member.entry.id, lines)) {
            return Some("addressed");
        }
        if !cast.contains(&line.from) {
            reason = Some("a human in her rooms");
        } else if wake == Wake::AllMessages && reason.is_none() {
            reason = Some("a message in her rooms");
        }
    }
    reason.or(beat.then_some("the beat"))
}

/// A seeded shuffle, so the order of those woken together is drawn, and replays.
fn shuffled<T>(mut items: Vec<T>, seed: u64) -> Vec<T> {
    let mut draw = Draw(seed);
    for at in (1..items.len()).rev() {
        let pick = (draw.next() % (at as u64 + 1)) as usize;
        items.swap(at, pick);
    }
    items
}

/// Runs the plan: every repeat one simulation, all of them in one Markdown report.
pub fn simulate(harness: &Harness, plan_path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(plan_path).with_context(|| format!("the plan {} could not be read", plan_path.display()))?;
    let plan: SimFile = toml::from_str(&text).with_context(|| format!("the plan {} is not a simulation plan", plan_path.display()))?;
    if plan.cast.is_empty() || plan.cast.len() > CAST_MAX {
        bail!("a simulation holds 1 to {CAST_MAX} personas, not {}", plan.cast.len());
    }
    if plan.rounds == 0 || plan.repeats == 0 || plan.rooms.is_empty() || plan.round_minutes <= 0 {
        bail!("a simulation needs rounds, repeats, a room and a positive round_minutes");
    }
    let base = plan_path.parent().unwrap_or(Path::new("."));
    let mut members = Vec::new();
    for entry in &plan.cast {
        let state = state_file(&base.join(&entry.state), localpart(&entry.id))?;
        let rooms = if entry.rooms.is_empty() { plan.rooms.clone() } else { entry.rooms.clone() };
        let public = state["publicName"].as_str().unwrap_or_default().to_owned();
        let names = [localpart(&entry.id), entry.name.as_str(), public.as_str()].into_iter().chain(entry.names.iter().map(String::as_str)).filter(|name| !name.is_empty());
        members.push(Member { addressee: Addressee::new(&entry.id, names), entry: entry.clone(), state, rooms });
    }
    for (at, message) in plan.script.iter().enumerate() {
        if message.ago.is_some() || message.new || message.tag {
            bail!("script message {} uses a room fixture's ago, new or tag: a script names its round and its mentions", at + 1);
        }
        if !plan.rooms.contains(&message.room) {
            bail!("script message {} is in {}, which is not one of the rooms", at + 1, message.room);
        }
    }
    let runs: Vec<Value> = (0..plan.repeats).map(|repeat| run(harness, &plan, &mut members, repeat)).collect::<Result<_>>()?;
    Ok(markdown(plan_path, &plan, &runs))
}

fn run(harness: &Harness, plan: &SimFile, members: &mut [Member], repeat: usize) -> Result<Value> {
    let seed = plan.seed.wrapping_add(repeat as u64);
    let cast: Vec<String> = members.iter().map(|member| member.entry.id.clone()).collect();
    let at_round = |round: usize| plan.start + Duration::minutes(plan.round_minutes * round as i64);
    let mut lines: Vec<Line> = Vec::new();
    let mut rounds_of: Vec<usize> = Vec::new();
    let scripted = |round: usize, lines: &mut Vec<Line>, rounds_of: &mut Vec<usize>| -> Result<()> {
        for (at, message) in plan.script.iter().enumerate().filter(|(_, message)| message.round.unwrap_or(0) == round) {
            lines.push(Line::of(message, at_round(round) + Duration::seconds(at as i64), None, at)?);
            rounds_of.push(round);
        }
        Ok(())
    };
    scripted(0, &mut lines, &mut rounds_of)?;
    let opening = lines.len();
    let mut seen = vec![0usize; members.len()];
    let mut recent: Vec<Vec<Vec<String>>> = vec![Vec::new(); members.len()];
    let mut turns = Vec::new();
    let mut counter = 0u64;
    for round in 1..=plan.rounds {
        scripted(round, &mut lines, &mut rounds_of)?;
        let beat = plan.beat_every > 0 && round % plan.beat_every == 0;
        let woken: Vec<(usize, &'static str)> = members.iter().enumerate().filter_map(|(at, member)| wake_reason(&lines, seen[at], member, &cast, plan.wake, beat).map(|why| (at, why))).collect();
        for (at, why) in shuffled(woken, seed ^ (round as u64).wrapping_mul(0x9E37_79B9)) {
            counter += 1;
            let member = &members[at];
            let id = member.entry.id.clone();
            let fresh: Vec<&Line> = lines[seen[at]..].iter().filter(|line| line.from != id && (member.rooms.contains(&line.room) || line.dm)).collect();
            let here = fresh.iter().rev().find(|line| line.mentions.contains(&id)).or(fresh.last()).map_or_else(|| member.rooms[0].clone(), |line| line.room.clone());
            let kind = if fresh.is_empty() { TurnKind::Beat } else { TurnKind::Chat };
            let now = at_round(round) + Duration::seconds(30 + counter as i64);
            let request = Request {
                instance: localpart(&id).to_owned(),
                kind,
                state: Some(member.state.clone()),
                room: Some(here.clone()),
                seed: seed.wrapping_mul(1_000_003).wrapping_add(counter),
                renders: Some(1),
                judge_against: None,
                operators: plan.operators.clone(),
                minds: cast.clone(),
                own_sender: id.clone(),
                tuning: harness.tuning.clone(),
                through: Through::Persona,
                last_heard: vec![],
                recent: recent[at].clone(),
            };
            let view = View { lines: &lines, viewer: &id, rooms: &member.rooms, seen: seen[at] };
            let names = Names(&member.addressee);
            let organs = Organs { persona: None, chat: Some(&view), addressing: Some(&names), model: harness.model };
            let report = project(&organs, &request, now).unwrap_or_else(|fault| json!({ "error": format!("{fault:#}") }));
            seen[at] = lines.len();
            let render = &report["renders"][0];
            if let Some(selected) = render["selected"].as_array() {
                recent[at].push(selected.iter().filter_map(Value::as_str).map(str::to_owned).collect());
            }
            let said: Vec<String> = render["persona"]["would_say"].as_array().map(|said| said.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
            let line = (!said.is_empty()).then(|| said.join(" "));
            if let Some(says) = &line {
                lines.push(Line { from: id.clone(), name: member.entry.name.clone(), room: here.clone(), at_ms: u64::try_from(now.timestamp_millis()).unwrap_or_default(), says: says.clone(), mentions: vec![], dm: false, follow_up: false, reply_to: None, new: false });
                rounds_of.push(round);
            }
            let turn = json!({ "round": round, "persona": member.entry.name, "id": id, "woke": why, "kind": kind, "room": here, "said": line, "report": report });
            let member = &mut members[at];
            member.state = state_after_turn(std::mem::take(&mut member.state), &turn);
            turns.push(turn);
        }
    }
    let metrics = metrics(members, &lines, &rounds_of, opening, &turns, plan.rounds);
    let transcript: Vec<Value> = lines.iter().zip(&rounds_of).map(|(line, round)| json!({ "round": round, "room": line.room, "from": line.from, "name": line.name, "says": line.says, "mind": cast.contains(&line.from) })).collect();
    Ok(json!({ "repeat": repeat + 1, "seed": seed, "transcript": transcript, "turns": turns, "metrics": metrics }))
}

/// Words that mark talk about the work. A cheap lexical heuristic for the work-talk share, not a classifier: it counts
/// a line as work talk when any of these appears as a word in it.
const WORK_WORDS: [&str; 26] = [
    "repo", "repos", "pr", "commit", "merge", "deploy", "build", "test", "tests", "bug", "branch", "code", "release", "issue", "pipeline", "crate", "schema", "spec", "cargo", "clippy", "daemon", "patch", "refactor", "migration", "server", "ticket",
];

fn work_talk(text: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric()).any(|word| WORK_WORDS.contains(&word.to_lowercase().as_str()))
}

/// Mean pairwise cosine between the speakers' texts; speakers with no text are left out.
fn mean_pairwise(texts: &BTreeMap<String, String>) -> Option<f64> {
    let spoken: Vec<&String> = texts.values().filter(|text| !text.trim().is_empty()).collect();
    let mut sum = 0.0;
    let mut pairs = 0usize;
    for (at, a) in spoken.iter().enumerate() {
        for b in &spoken[at + 1..] {
            sum += cosine(a, b);
            pairs += 1;
        }
    }
    (pairs > 0).then(|| sum / pairs as f64)
}

/// Which rounds a measure keeps.
type Keep<'a> = &'a dyn Fn(usize) -> bool;

fn metrics(members: &[Member], lines: &[Line], rounds_of: &[usize], opening: usize, turns: &[Value], rounds: usize) -> Value {
    let cast: Vec<&str> = members.iter().map(|member| member.entry.id.as_str()).collect();
    let mut per = BTreeMap::new();
    for member in members {
        let mine: Vec<&Value> = turns.iter().filter(|turn| turn["id"] == member.entry.id.as_str()).collect();
        let spoke = mine.iter().filter(|turn| !turn["said"].is_null()).count();
        let mut addressed = BTreeMap::new();
        for (at, line) in lines.iter().enumerate().skip(opening).filter(|(_, line)| line.from == member.entry.id) {
            for other in members.iter().filter(|other| other.entry.id != member.entry.id) {
                if is_for(&other.addressee, &line.delivery(at, &other.entry.id, lines)) {
                    *addressed.entry(other.entry.name.clone()).or_insert(0u64) += 1;
                }
            }
        }
        per.insert(member.entry.name.clone(), json!({ "turns": mine.len(), "messages": spoke, "silence_rate": if mine.is_empty() { Value::Null } else { json!(1.0 - spoke as f64 / mine.len() as f64) }, "addressed": addressed }));
    }
    // Mind-to-Mind chains: runs of Mind lines with no human line between them.
    let (mut longest, mut long_chains, mut run) = (0usize, 0usize, 0usize);
    for line in &lines[opening..] {
        if cast.contains(&line.from.as_str()) {
            run += 1;
            longest = longest.max(run);
        } else {
            long_chains += usize::from(run >= 3);
            run = 0;
        }
    }
    long_chains += usize::from(run >= 3);
    let half = rounds.div_ceil(2);
    let spoken_by = |keep: &dyn Fn(usize) -> bool| -> BTreeMap<String, String> {
        let mut texts = BTreeMap::new();
        for (line, _) in lines.iter().zip(rounds_of).skip(opening).filter(|(line, round)| cast.contains(&line.from.as_str()) && keep(**round)) {
            let text: &mut String = texts.entry(line.from.clone()).or_default();
            text.push(' ');
            text.push_str(&line.says);
        }
        texts
    };
    let prose_by = |keep: &dyn Fn(usize) -> bool| -> BTreeMap<String, String> {
        let mut texts = BTreeMap::new();
        for turn in turns.iter().filter(|turn| turn["round"].as_u64().is_some_and(|round| keep(round as usize))) {
            let text: &mut String = texts.entry(turn["id"].as_str().unwrap_or_default().to_owned()).or_default();
            text.push(' ');
            text.push_str(turn["report"]["renders"][0]["persona"]["prose"].as_str().unwrap_or_default());
        }
        texts
    };
    let (all, early, late): (Keep, Keep, Keep) = (&|_| true, &|round| round <= half, &|round| round > half);
    let mind_lines: Vec<&Line> = lines[opening..].iter().filter(|line| cast.contains(&line.from.as_str())).collect();
    let work = mind_lines.iter().filter(|line| work_talk(&line.says)).count();
    json!({
        "personas": per,
        "turns": turns.len(),
        "mind_messages": mind_lines.len(),
        "silence_rate": if turns.is_empty() { Value::Null } else { json!(turns.iter().filter(|turn| turn["said"].is_null()).count() as f64 / turns.len() as f64) },
        "mind_chain_longest": longest,
        "mind_chains_of_3_or_more": long_chains,
        "convergence_lines": { "all": mean_pairwise(&spoken_by(all)), "first_half": mean_pairwise(&spoken_by(early)), "second_half": mean_pairwise(&spoken_by(late)) },
        "convergence_prose": { "all": mean_pairwise(&prose_by(all)), "first_half": mean_pairwise(&prose_by(early)), "second_half": mean_pairwise(&prose_by(late)) },
        "work_talk_share": if mind_lines.is_empty() { Value::Null } else { json!(work as f64 / mind_lines.len() as f64) },
    })
}

fn number(value: &Value) -> String {
    value.as_f64().map_or_else(|| "n/a".into(), |number| format!("{number:.3}"))
}

fn markdown(plan_path: &Path, plan: &SimFile, runs: &[Value]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Swarm simulation\n");
    let _ = writeln!(out, "> **State does not evolve during a run.** The Interpreter (C3b) and the commit (C3c) are not built, so every persona starts each turn from the state the plan gave her. `state_after_turn` in `projection/simulate.rs` is the seam where they will move it between rounds. Freshness carries over: each persona's earlier packets in the run count against repeating an item.\n");
    let _ = writeln!(out, "- plan: `{}`", plan_path.display());
    let _ = writeln!(out, "- cast: {}", plan.cast.iter().map(|entry| format!("{} (`{}`, `{}`)", entry.name, entry.id, entry.state.display())).collect::<Vec<_>>().join(", "));
    let _ = writeln!(out, "- rooms: {}; rounds: {}; repeats: {}; seed: {}; minutes a round: {}", plan.rooms.join(", "), plan.rounds, plan.repeats, plan.seed, plan.round_minutes);
    let _ = writeln!(out, "- wake: {}; beat: {}", match plan.wake { Wake::HumansInRooms => "ruled set (addressed, any human message in her rooms, the beat; another Mind's unaddressed message wakes no one)", Wake::AllMessages => "control: any message in her rooms wakes her, a Mind's too (the old loop)" }, if plan.beat_every == 0 { "off".to_owned() } else { format!("every {} rounds", plan.beat_every) });
    let _ = writeln!(out, "- speech: the double-quoted spans of her prose, joined, posted to the room she woke in; none is silence. A preview of the Interpreter's speech, not its decision");
    let _ = writeln!(out, "- convergence is the mean pairwise cosine of speakers' word counts (lexical, cheap); work-talk share is a lexical heuristic over a fixed word list, not a classifier");
    let _ = writeln!(out, "- read-only: nothing is posted, no state is put, no Mind is asked\n");
    for run in runs {
        let metrics = &run["metrics"];
        let _ = writeln!(out, "## Repeat {} (seed {})\n", run["repeat"], run["seed"]);
        let _ = writeln!(out, "### Metrics\n");
        let _ = writeln!(out, "- turns: {}; Mind messages: {}; silence rate: {}", metrics["turns"], metrics["mind_messages"], number(&metrics["silence_rate"]));
        let _ = writeln!(out, "- longest Mind-to-Mind chain with no human between: {}; chains of 3 or more: {}", metrics["mind_chain_longest"], metrics["mind_chains_of_3_or_more"]);
        let _ = writeln!(out, "- convergence of spoken lines: all {}, first half {}, second half {}", number(&metrics["convergence_lines"]["all"]), number(&metrics["convergence_lines"]["first_half"]), number(&metrics["convergence_lines"]["second_half"]));
        let _ = writeln!(out, "- convergence of prose: all {}, first half {}, second half {}", number(&metrics["convergence_prose"]["all"]), number(&metrics["convergence_prose"]["first_half"]), number(&metrics["convergence_prose"]["second_half"]));
        let _ = writeln!(out, "- work-talk share of Mind lines (heuristic): {}\n", number(&metrics["work_talk_share"]));
        let _ = writeln!(out, "| persona | turns | messages | silence rate | addressed |\n|---|---|---|---|---|");
        if let Some(personas) = metrics["personas"].as_object() {
            for (name, row) in personas {
                let addressed = row["addressed"].as_object().map(|map| map.iter().map(|(who, count)| format!("{who} {count}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let _ = writeln!(out, "| {name} | {} | {} | {} | {addressed} |", row["turns"], row["messages"], number(&row["silence_rate"]));
            }
        }
        let _ = writeln!(out, "\n### Transcript\n");
        for line in run["transcript"].as_array().into_iter().flatten() {
            let who = if line["mind"] == true { line["name"].as_str().unwrap_or_default().to_owned() } else { format!("{} (human)", line["name"].as_str().unwrap_or_default()) };
            let _ = writeln!(out, "- r{} {} **{who}**: {}", line["round"], line["room"].as_str().unwrap_or_default(), line["says"].as_str().unwrap_or_default());
        }
        let _ = writeln!(out, "\n### Turns\n");
        for turn in run["turns"].as_array().into_iter().flatten() {
            let said = if turn["said"].is_null() { "silent".to_owned() } else { "spoke".to_owned() };
            let _ = writeln!(out, "<details><summary>round {}, {} (woke: {}, {} turn, {}): {said}</summary>\n", turn["round"], turn["persona"].as_str().unwrap_or_default(), turn["woke"].as_str().unwrap_or_default(), turn["kind"].as_str().unwrap_or_default(), turn["room"].as_str().unwrap_or_default());
            let report = &turn["report"];
            if let Some(error) = report.get("error") {
                let _ = writeln!(out, "**failed:** {}\n", error.as_str().unwrap_or_default());
            } else {
                let _ = writeln!(out, "Stimulus:\n\n````text\n{}\n````\n", report["packet"]["stimulus"].as_str().unwrap_or_default());
                turn_markdown(&mut out, &report["renders"][0]);
            }
            let _ = writeln!(out, "</details>\n");
        }
    }
    let _ = writeln!(out, "## Total usage\n\n{}", usage_markdown(&usage_total(runs)));
    out
}

#[cfg(test)]
mod tests;
