//! The swarm in one simulated room (`mind-persona project --simulate <plan>`): up to eight personas take turns, each turn
//! one `project` through the Persona stage, and each one's quoted lines join the room as from her Matrix id, so the
//! next one hears them.
//!
//! Who wakes is the heartbeat's: `heartbeat::wakes` under the plan's `WakePolicy`, over each persona's addressee as the
//! heartbeat's addressing table holds it (her stewarded projects' names included) when the plan names the table. The
//! beat is the heartbeat's too: when it falls, one Persona wakes, the winner of `tempo::pick`. A persona woken in several
//! rooms in one round takes one turn in each. The turns of a round go in an order drawn from the seed, each against the
//! room as it stands at that turn.
//!
//! Her state does not move during a run: the Interpreter (C3b) and the commit (C3c) are not built. `state_after_turn`
//! is the seam where they will. An explicit addressee (the Interpreter naming whom she speaks to) cannot be simulated
//! until then either: a spoken line addresses another persona only by the names in its text.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use super::harness::{Harness, Line, Message, OVERLAP_FLAG, cosine, pending_for, state_file, turn_markdown, usage_markdown, usage_total};
use super::select::Draw;
use super::tuning;
use super::{MouthReader, Organs, Request, Through, TurnKind, project};
use crate::address::{Addressee, is_for, localpart};
use crate::chat_client::ChatPending;
use crate::heartbeat::{Addressing, DEFAULT_BEAT_S, FileStore, HeartbeatView, LineFacts, Speaker, WakeCause, WakePolicy, addressed_by_member, wakes};
use crate::tempo::{Standing, pick};

/// The most personas a simulation holds: the swarm.
const CAST_MAX: usize = 8;
/// How many transcript lines the report quotes as representative.
const REPRESENTATIVE_LINES: usize = 18;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SimFile {
    rounds: usize,
    #[serde(default = "one")]
    repeats: usize,
    #[serde(default)]
    seed: u64,
    #[serde(default = "ruled")]
    wake: WakePolicy,
    /// The heartbeat's beat, in seconds of simulated time; 0 is no beat.
    #[serde(default = "beat_s")]
    beat_s: u32,
    #[serde(default = "two")]
    round_minutes: i64,
    /// She is in a room's exchange when she has spoken there within this many minutes before the line (`humans_pace`):
    /// others talking on does not keep her in it.
    #[serde(default = "exchange_minutes")]
    exchange_minutes: i64,
    start: DateTime<Utc>,
    rooms: Vec<String>,
    #[serde(default)]
    operators: Vec<String>,
    /// The heartbeat's store (`heartbeat.cc`), read without a lock or a write, for its addressing table: each persona's
    /// names and her stewarded projects' names. None addresses each by her own names.
    #[serde(default)]
    addressing: Option<PathBuf>,
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

fn exchange_minutes() -> i64 {
    30
}

fn ruled() -> WakePolicy {
    WakePolicy::Ruled
}

fn beat_s() -> u32 {
    DEFAULT_BEAT_S
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
    /// The rooms that are hers, where any human line wakes her under `humans_pace`. Neither the heartbeat's table nor
    /// `persona.env` records which rooms a Mind owns, so the plan says; a direct message is always hers.
    #[serde(default)]
    own_rooms: Vec<String>,
}

struct Member {
    entry: CastEntry,
    state: Value,
    addressee: Addressee,
    /// Where her addressee came from: the heartbeat's table, or her own names.
    names_from: &'static str,
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

fn speaker(line: &Line, member: &Member, operators: &[String], cast: &[String]) -> Speaker {
    if line.from == member.entry.id {
        Speaker::Her
    } else if operators.contains(&line.from) {
        Speaker::Operator
    } else if cast.contains(&line.from) {
        Speaker::Mind
    } else {
        Speaker::Member
    }
}

/// For each line, how many Mind lines in a row came before it in her current Mind-only run: one run across every room
/// she sees, which a human's line ends only when it is in a room the run has lines in. Meta chatting in #eureka does not
/// end a loop in #minds.
fn mind_runs(lines: &[Line], cast: &[String], member: &Member) -> Vec<usize> {
    let mut run = 0usize;
    let mut run_rooms: Vec<&str> = Vec::new();
    lines
        .iter()
        .map(|line| {
            let before = run;
            if line.visible_to(&member.entry.id, &member.rooms) {
                if cast.contains(&line.from) {
                    run += 1;
                    run_rooms.push(&line.room);
                } else if run_rooms.contains(&line.room.as_str()) {
                    run = 0;
                    run_rooms.clear();
                }
            }
            before
        })
        .collect()
}

/// How a Mind's line addressed her, by the evidence `is_for` had, strongest first: a Matrix tag or a reply to her, her
/// Matrix id written as plain text, or one of her names in the text.
fn how_addressed(line: &Line, at: usize, member: &Member, lines: &[Line]) -> &'static str {
    let delivery = line.delivery(at, &member.entry.id, lines);
    if delivery.mentions.contains(&member.entry.id) || delivery.basis.is_some() {
        "mention or reply"
    } else if line.says.to_lowercase().contains(&member.entry.id.to_lowercase()) {
        "id as text"
    } else {
        "name in text"
    }
}

/// One wake: why (the strongest cause among her new lines, under the plan's policy) and, when only Minds' lines caused
/// it, the strongest evidence that addressed her and the lines that did.
struct Woken {
    cause: WakeCause,
    /// The newest line with that cause: its room is the turn's room.
    line: usize,
    how: Option<&'static str>,
    by: Vec<Value>,
}

/// Whether she is in the exchange a line belongs to: she spoke in its room within `window` before it.
fn in_exchange(lines: &[Line], at: usize, her: &str, window: Duration) -> bool {
    let line = &lines[at];
    let since = line.at_ms.saturating_sub(u64::try_from(window.num_milliseconds()).unwrap_or(0));
    lines[..at].iter().any(|earlier| earlier.room == line.room && earlier.from == her && earlier.at_ms >= since)
}

/// Her wakes: one per room a new line woke her in, in the order of each room's first waking line. A room's turn takes
/// the strongest cause among its lines, so no room that woke her, a direct message least of all, is passed over.
#[allow(clippy::too_many_arguments, reason = "one wake decision's inputs, each a plan or room fact")]
fn woken(lines: &[Line], runs: &[usize], seen: usize, member: &Member, operators: &[String], cast: &[String], policy: WakePolicy, gap: Duration) -> Vec<Woken> {
    let mut found: Vec<(usize, WakeCause)> = Vec::new();
    for (at, line) in lines.iter().enumerate().skip(seen) {
        if !line.visible_to(&member.entry.id, &member.rooms) {
            continue;
        }
        let delivery = line.delivery(at, &member.entry.id, lines);
        let facts = LineFacts { mind_run: runs[at], own_room: member.entry.own_rooms.contains(&line.room), in_exchange: in_exchange(lines, at, &member.entry.id, gap) };
        if let Some(cause) = wakes(policy, &member.addressee, &delivery, speaker(line, member, operators, cast), facts) {
            found.push((at, cause));
        }
    }
    let mut rooms: Vec<&str> = Vec::new();
    for (at, _) in &found {
        if !rooms.contains(&lines[*at].room.as_str()) {
            rooms.push(&lines[*at].room);
        }
    }
    rooms
        .into_iter()
        .filter_map(|room| {
            let found: Vec<(usize, WakeCause)> = found.iter().copied().filter(|(at, _)| lines[*at].room == room).collect();
            let cause = found.iter().map(|(_, cause)| *cause).min()?;
            let line = found.iter().rev().find(|(_, found)| *found == cause).map_or(0, |(at, _)| *at);
            if cause != WakeCause::AddressedByMind {
                return Some(Woken { cause, line, how: None, by: vec![] });
            }
            let addressing: Vec<usize> = found.iter().filter(|(_, cause)| *cause == WakeCause::AddressedByMind).map(|(at, _)| *at).collect();
            let rank = |how: &str| ["mention or reply", "id as text", "name in text"].iter().position(|one| *one == how).unwrap_or(3);
            let how = addressing.iter().map(|at| how_addressed(&lines[*at], *at, member, lines)).min_by_key(|how| rank(how));
            let by = addressing.iter().map(|at| json!({ "from": lines[*at].from, "how": how_addressed(&lines[*at], *at, member, lines), "says": lines[*at].says })).collect();
            Some(Woken { cause, line, how, by })
        })
        .collect()
}

/// The wire spelling of a cause, and of the beat.
fn cause_word(cause: Option<WakeCause>) -> &'static str {
    match cause {
        Some(WakeCause::AddressedByHuman) => "addressed_by_human",
        Some(WakeCause::Human) => "human",
        Some(WakeCause::AddressedByMind) => "addressed_by_mind",
        Some(WakeCause::Mind) => "mind",
        None => "beat",
    }
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
    let mut members = cast_of(&plan, base)?;
    let group_state = tuning::load(&harness.tuning).map(|loaded| loaded.tuning.select.group_state).ok();
    let runs: Vec<Value> = (0..plan.repeats).map(|repeat| run(harness, &plan, &mut members, repeat)).collect::<Result<_>>()?;
    Ok(markdown(plan_path, &plan, &members, group_state, &runs))
}

/// The plan's cast: each persona's state, checked as a put would check it, and her addressee, from the heartbeat's
/// table when the plan names it and the table holds her; and the script checked against the rooms.
fn cast_of(plan: &SimFile, base: &Path) -> Result<Vec<Member>> {
    let table = match &plan.addressing {
        Some(path) => FileStore::new(base.join(path)).addressing().with_context(|| format!("the heartbeat store {} could not be read", path.display()))?.unwrap_or_default(),
        None => Addressing::default(),
    };
    let mut members = Vec::new();
    for entry in &plan.cast {
        let state = state_file(&base.join(&entry.state), localpart(&entry.id))?;
        let rooms = if entry.rooms.is_empty() { plan.rooms.clone() } else { entry.rooms.clone() };
        let (addressee, names_from) = match table.minds.iter().find(|row| row.matrix_id() == entry.id) {
            Some(row) => (row.clone(), "the heartbeat's table"),
            None => {
                let public = state["publicName"].as_str().unwrap_or_default().to_owned();
                let names = [localpart(&entry.id), entry.name.as_str(), public.as_str()].into_iter().filter(|name| !name.is_empty()).map(str::to_owned).collect::<Vec<_>>();
                (Addressee::new(&entry.id, names.iter().map(String::as_str)), "her own names")
            }
        };
        members.push(Member { addressee, names_from, entry: entry.clone(), state, rooms });
    }
    for (at, message) in plan.script.iter().enumerate() {
        if message.ago.is_some() || message.new || message.tag {
            bail!("script message {} uses a room fixture's ago, new or tag: a script names its round and its mentions", at + 1);
        }
        if !message.dm && !plan.rooms.contains(&message.room) {
            bail!("script message {} is in {}, which is not one of the rooms", at + 1, message.room);
        }
        if message.dm && message.to.as_ref().is_none_or(|to| !plan.cast.iter().any(|entry| entry.id == *to)) {
            bail!("script message {} is a direct message: name its recipient, a cast member, in `to`", at + 1);
        }
        if message.dm && plan.rooms.contains(&message.room) {
            bail!("script message {} is a direct message in {}, one of the plan's rooms: a direct message has its own room", at + 1, message.room);
        }
    }
    Ok(members)
}

fn run(harness: &Harness, plan: &SimFile, members: &mut [Member], repeat: usize) -> Result<Value> {
    let seed = plan.seed.wrapping_add(repeat as u64);
    let cast: Vec<String> = members.iter().map(|member| member.entry.id.clone()).collect();
    let at_round = |round: usize| plan.start + Duration::minutes(plan.round_minutes * round as i64);
    let mut lines: Vec<Line> = Vec::new();
    // Per line: the round, and the cause of the turn that spoke it (none for a script line).
    let mut rounds_of: Vec<usize> = Vec::new();
    let mut causes: Vec<Option<&'static str>> = Vec::new();
    let scripted = |round: usize, lines: &mut Vec<Line>, rounds_of: &mut Vec<usize>, causes: &mut Vec<Option<&'static str>>| -> Result<()> {
        for (at, message) in plan.script.iter().enumerate().filter(|(_, message)| message.round.unwrap_or(0) == round) {
            lines.push(Line::of(message, at_round(round) + Duration::seconds(at as i64), None, at)?);
            rounds_of.push(round);
            causes.push(None);
        }
        Ok(())
    };
    scripted(0, &mut lines, &mut rounds_of, &mut causes)?;
    let opening = lines.len();
    let mut seen = vec![0usize; members.len()];
    let mut last_turn: Vec<Option<DateTime<Utc>>> = vec![None; members.len()];
    let mut last_beat = plan.start;
    let mut recent: Vec<Vec<Vec<String>>> = vec![Vec::new(); members.len()];
    let mut turns = Vec::new();
    let mut counter = 0u64;
    for round in 1..=plan.rounds {
        scripted(round, &mut lines, &mut rounds_of, &mut causes)?;
        let round_at = at_round(round);
        // One turn per room that woke her, each in its room; all of hers this round read from where the round found her.
        let round_seen = seen.clone();
        let mut woke: Vec<(usize, Option<Woken>)> = members.iter().enumerate().flat_map(|(at, member)| woken(&lines, &mind_runs(&lines, &cast, member), seen[at], member, &plan.operators, &cast, plan.wake, Duration::minutes(plan.exchange_minutes)).into_iter().map(move |why| (at, Some(why)))).collect();
        // The beat, as the heartbeat's: when it has fallen, one more wake for the winner of `tempo::pick` among those
        // not already woken.
        if plan.beat_s > 0 && (round_at - last_beat).num_seconds() >= i64::from(plan.beat_s) {
            let standings: Vec<Standing> = members
                .iter()
                .enumerate()
                .map(|(at, member)| Standing {
                    slug: localpart(&member.entry.id).to_owned(),
                    eligible: !woke.iter().any(|(woken, _)| *woken == at),
                    waited_s: last_turn[at].map(|turn| u64::try_from((round_at - turn).num_seconds()).unwrap_or(0)),
                    last_turn_ms: last_turn[at].map(|turn| turn.timestamp_millis()),
                    topic_permille: 0,
                    addressed_by_member: addressed_by_member(&plan.operators, &member.addressee, &pending_for(&lines, &member.entry.id, &member.rooms, |index| index >= seen[at]).deliveries),
                })
                .collect();
            if let Some(winner) = pick(&standings, plan.beat_s) {
                woke.push((winner, None));
                last_beat = round_at;
            }
        }
        for (at, why) in shuffled(woke, seed ^ (round as u64).wrapping_mul(0x9E37_79B9)) {
            counter += 1;
            let member = &members[at];
            let id = member.entry.id.clone();
            let fresh: Vec<&Line> = lines[round_seen[at]..].iter().filter(|line| line.from != id && line.visible_to(&id, &member.rooms)).collect();
            // The turn is in the room of the line that woke her; a beat's, in the room of the newest line she has not seen.
            let woke_on = why.as_ref().map(|why| &lines[why.line]).or(fresh.last().copied());
            let here = woke_on.map_or_else(|| member.rooms[0].clone(), |line| line.room.clone());
            // Woken by a direct message, she answers in it: to its sender alone.
            let dm_to = woke_on.filter(|line| line.dm_to.as_deref() == Some(id.as_str())).map(|line| line.from.clone());
            let kind = if fresh.iter().any(|line| line.room == here) { TurnKind::Chat } else { TurnKind::Beat };
            let now = round_at + Duration::seconds(30 + counter as i64);
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
                through: Through::Interpreter,
                last_heard: vec![],
                recent: recent[at].clone(),
                own_rooms: member.entry.own_rooms.clone(),
            };
            let view = View { lines: &lines, viewer: &id, rooms: &member.rooms, seen: round_seen[at] };
            let names = Names(&member.addressee);
            let organs = Organs { persona: None, chat: Some(&view), addressing: Some(&names), model: harness.model };
            let report = project(&organs, &request, now).unwrap_or_else(|fault| json!({ "error": format!("{fault:#}") }));
            seen[at] = lines.len();
            last_turn[at] = Some(now);
            let render = &report["renders"][0];
            if let Some(selected) = render["selected"].as_array() {
                recent[at].push(selected.iter().filter_map(Value::as_str).map(str::to_owned).collect());
            }
            // A failed call is a failure, never a silence.
            let failed = report.get("error").or_else(|| render.get("failed")).or_else(|| render["persona"].get("failed")).or_else(|| render["interpreter"].get("failed")).map(|fault| fault.as_str().unwrap_or_default().to_owned());
            // What she says is the Interpreter's decision alone; who it wakes is its structured mentions and reply target.
            let said: Vec<Value> = if failed.is_some() { vec![] } else { render["interpreter"]["said"].as_array().cloned().unwrap_or_default() };
            let cause = cause_word(why.as_ref().map(|why| why.cause));
            let mut spoken: Vec<String> = Vec::new();
            for (offset, decided) in said.iter().enumerate() {
                let room = decided["room"].as_str().unwrap_or(&here).to_owned();
                let says = decided["text"].as_str().unwrap_or_default().to_owned();
                let mentions: Vec<String> = decided["mentions"].as_array().map(|ids| ids.iter().filter_map(Value::as_str).filter(|id| cast.contains(&(*id).to_owned()) || plan.operators.contains(&(*id).to_owned())).map(str::to_owned).collect()).unwrap_or_default();
                let reply_to = decided["reply_to"].as_str().and_then(|person| lines.iter().rposition(|line| line.from == person && line.room == room));
                let at_ms = u64::try_from(now.timestamp_millis()).unwrap_or_default() + offset as u64;
                let to = if room == here { dm_to.clone() } else { None };
                lines.push(Line { from: id.clone(), name: member.entry.name.clone(), room, at_ms, says: says.clone(), mentions, dm_to: to, follow_up: false, reply_to, new: false });
                rounds_of.push(round);
                causes.push(Some(cause));
                spoken.push(says);
            }
            let line = (!spoken.is_empty()).then(|| spoken.join(" / "));
            let outcome = if failed.is_some() { "failed" } else if line.is_some() { "spoke" } else { "silent" };
            let (how, by) = why.map_or((None, vec![]), |why| (why.how, why.by));
            let turn = json!({ "round": round, "persona": member.entry.name, "id": id, "woke": cause, "how": how, "addressed_by": by, "kind": kind, "room": here, "outcome": outcome, "failed": failed, "said": line, "report": report });
            let member = &mut members[at];
            member.state = state_after_turn(std::mem::take(&mut member.state), &turn);
            turns.push(turn);
        }
    }
    let metrics = metrics(members, &lines, &rounds_of, &causes, opening, &turns, plan.rounds);
    let transcript: Vec<Value> = lines.iter().zip(&rounds_of).zip(&causes).map(|((line, round), cause)| json!({ "round": round, "room": line.room, "from": line.from, "name": line.name, "says": line.says, "mind": cast.contains(&line.from), "woke": cause })).collect();
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

/// A Mind-to-Mind reply: a Mind's line spoken in a turn another Mind's line woke.
fn mind_reply(cause: Option<&str>) -> bool {
    matches!(cause, Some("addressed_by_mind" | "mind"))
}

/// Per room: Mind-to-Mind replies in a row with no human line and no human-woken answer between them.
fn reply_chains(lines: &[Line], causes: &[Option<&str>], opening: usize, cast: &[&str]) -> Value {
    let mut rooms: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for (line, cause) in lines.iter().zip(causes).skip(opening) {
        let (longest, chains, total, run) = rooms.entry(line.room.clone()).or_default();
        if cast.contains(&line.from.as_str()) && mind_reply(*cause) {
            *run += 1;
            *total += 1;
            *longest = (*longest).max(*run);
        } else {
            *chains += usize::from(*run >= 3);
            *run = 0;
        }
    }
    let out: BTreeMap<String, Value> = rooms
        .into_iter()
        .map(|(room, (longest, chains, total, run))| (room, json!({ "longest": longest, "chains_of_3_or_more": chains + usize::from(run >= 3), "mind_to_mind_replies": total })))
        .collect();
    json!(out)
}

fn metrics(members: &[Member], lines: &[Line], rounds_of: &[usize], causes: &[Option<&str>], opening: usize, turns: &[Value], rounds: usize) -> Value {
    let cast: Vec<&str> = members.iter().map(|member| member.entry.id.as_str()).collect();
    let outcome = |turn: &&Value, what: &str| turn["outcome"] == what;
    let mut per = BTreeMap::new();
    for member in members {
        let mine: Vec<&Value> = turns.iter().filter(|turn| turn["id"] == member.entry.id.as_str()).collect();
        let (spoke, silent, failed) = (mine.iter().filter(|turn| outcome(turn, "spoke")).count(), mine.iter().filter(|turn| outcome(turn, "silent")).count(), mine.iter().filter(|turn| outcome(turn, "failed")).count());
        let mut addressed = BTreeMap::new();
        for (at, line) in lines.iter().enumerate().skip(opening).filter(|(_, line)| line.from == member.entry.id) {
            for other in members.iter().filter(|other| other.entry.id != member.entry.id) {
                if is_for(&other.addressee, &line.delivery(at, &other.entry.id, lines)) {
                    *addressed.entry(other.entry.name.clone()).or_insert(0u64) += 1;
                }
            }
        }
        let answered = spoke + silent;
        per.insert(member.entry.name.clone(), json!({ "turns": mine.len(), "messages": spoke, "failed": failed, "silence_rate": if answered == 0 { Value::Null } else { json!(silent as f64 / answered as f64) }, "addressed": addressed }));
    }
    let half = rounds.div_ceil(2);
    let spoken_by = |keep: Keep| -> BTreeMap<String, String> {
        let mut texts = BTreeMap::new();
        for (line, _) in lines.iter().zip(rounds_of).skip(opening).filter(|(line, round)| cast.contains(&line.from.as_str()) && keep(**round)) {
            let text: &mut String = texts.entry(line.from.clone()).or_default();
            text.push(' ');
            text.push_str(&line.says);
        }
        texts
    };
    let prose_by = |keep: Keep| -> BTreeMap<String, String> {
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
    const CAUSES: [&str; 5] = ["addressed_by_human", "human", "addressed_by_mind", "mind", "beat"];
    let count_causes = |these: &[&Value]| -> BTreeMap<&str, usize> { CAUSES.iter().map(|cause| (*cause, these.iter().filter(|turn| turn["woke"] == *cause).count())).collect() };
    let wakes_by_round: Vec<Value> = (1..=rounds)
        .map(|round| {
            let these: Vec<&Value> = turns.iter().filter(|turn| turn["round"] == round).collect();
            json!({ "round": round, "causes": count_causes(&these) })
        })
        .collect();
    let every: Vec<&Value> = turns.iter().collect();
    let mind_woken: Vec<&Value> = turns.iter().filter(|turn| turn["woke"] == "addressed_by_mind").collect();
    let by_how = |how: &str| mind_woken.iter().filter(|turn| turn["how"] == how).count();
    let id_as_text: Vec<Value> = mind_woken.iter().filter(|turn| turn["how"] == "id as text").map(|turn| json!({ "round": turn["round"], "persona": turn["persona"], "by": turn["addressed_by"] })).collect();
    let (silent, failed) = (turns.iter().filter(|turn| turn["outcome"] == "silent").count(), turns.iter().filter(|turn| turn["outcome"] == "failed").count());
    let answered = turns.len() - failed;
    let work = mind_lines.iter().filter(|line| work_talk(&line.says)).count();
    let overlaps: Vec<f64> = turns.iter().filter_map(|turn| turn["report"]["renders"][0]["heard_overlap"].as_f64()).collect();
    let narration = json!({
        "mean_heard_overlap": if overlaps.is_empty() { Value::Null } else { json!(overlaps.iter().sum::<f64>() / overlaps.len() as f64) },
        "high_overlap": overlaps.iter().filter(|overlap| **overlap > OVERLAP_FLAG).count(),
        "offers_options": turns.iter().filter(|turn| turn["report"]["renders"][0]["offers_options"] == true).count(),
        "narrations": overlaps.len(),
    });
    json!({
        "narration": narration,
        "personas": per,
        "turns": turns.len(),
        "wakes": count_causes(&every),
        "failed": failed,
        "mind_messages": mind_lines.len(),
        "silence_rate": if answered == 0 { Value::Null } else { json!(silent as f64 / answered as f64) },
        "reply_chains": reply_chains(lines, causes, opening, &cast),
        "convergence_lines": { "all": mean_pairwise(&spoken_by(all)), "first_half": mean_pairwise(&spoken_by(early)), "second_half": mean_pairwise(&spoken_by(late)) },
        "convergence_prose": { "all": mean_pairwise(&prose_by(all)), "first_half": mean_pairwise(&prose_by(early)), "second_half": mean_pairwise(&prose_by(late)) },
        "work_talk_share": if mind_lines.is_empty() { Value::Null } else { json!(work as f64 / mind_lines.len() as f64) },
        "wakes_by_round": wakes_by_round,
        "mind_addressed_wakes": { "total": mind_woken.len(), "mention_or_reply": by_how("mention or reply"), "id_as_text": by_how("id as text"), "name_in_text": by_how("name in text"), "id_as_text_wakes": id_as_text },
    })
}

/// A line's text on one Markdown line: its paragraph breaks shown as ` / `.
fn one_line(text: &str) -> String {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" / ")
}

fn number(value: &Value) -> String {
    value.as_f64().map_or_else(|| "n/a".into(), |number| format!("{number:.3}"))
}

fn policy_words(policy: WakePolicy) -> String {
    match policy {
        WakePolicy::Live => "live: the operator or another Mind addressing her (the heartbeat's rule today)".into(),
        WakePolicy::Ruled => "ruled: a line addressing her, from anyone, and any human line in her rooms".into(),
        WakePolicy::HumansPace(cap) => format!("humans_pace (cap {cap}): a human's line wakes a Mind in her own room always, and in a public room when it addresses her or continues an exchange she is in; a Mind's line wakes another only by an explicit mention or a reply, while her current Mind-only run, across her rooms, is under {cap} lines, never by a name in prose"),
        WakePolicy::NamesWithCap(cap) => format!("names_with_cap {cap}: ruled, but a Mind's line wakes no one once her current Mind-only run, across her rooms, has {cap} lines, until a human speaks where it runs"),
        WakePolicy::AllMessages => "all_messages: any line in her rooms, a Mind's too (the old loop, the control)".into(),
    }
}

/// `count` lines spread evenly over the transcript, the first and the last included.
fn representative(transcript: &[Value], count: usize) -> Vec<&Value> {
    if transcript.len() <= count {
        return transcript.iter().collect();
    }
    let mut at: Vec<usize> = (0..count).map(|index| index * (transcript.len() - 1) / (count - 1)).collect();
    at.dedup();
    at.into_iter().map(|index| &transcript[index]).collect()
}

fn transcript_line(line: &Value) -> String {
    let who = if line["mind"] == true { line["name"].as_str().unwrap_or_default().to_owned() } else { format!("{} (human)", line["name"].as_str().unwrap_or_default()) };
    let woke = line["woke"].as_str().map_or_else(String::new, |cause| format!(" _(woke: {cause})_"));
    format!("- r{} {} **{who}**{woke}: {}", line["round"], line["room"].as_str().unwrap_or_default(), one_line(line["says"].as_str().unwrap_or_default()))
}

fn markdown(plan_path: &Path, plan: &SimFile, members: &[Member], group_state: Option<bool>, runs: &[Value]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Swarm simulation\n");
    let _ = writeln!(out, "> **State does not evolve during a run.** The Interpreter (C3b) and the commit (C3c) are not built, so every persona starts each turn from the state the plan gave her. `state_after_turn` in `projection/simulate.rs` is the seam where they will move it between rounds. Freshness carries over: each persona's earlier packets in the run count against repeating an item.\n");
    let _ = writeln!(out, "> **Speech is the Interpreter's.** What she says is decided from her prose by the Interpreter's speech step (C3b, one tool-less structured call), held by code to her prose: each line rests on a verbatim span of it, in a room she may speak in, and is never someone else's words. Its structured mentions and reply target are the only explicit addressees; an `@` in the text tags no one.\n");
    let _ = writeln!(out, "- plan: `{}`", plan_path.display());
    let _ = writeln!(out, "- cast: {}", members.iter().map(|member| format!("{} (`{}`, `{}`, names from {})", member.entry.name, member.entry.id, member.entry.state.display(), member.names_from)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(out, "- rooms: {}; rounds: {}; repeats: {}; seed: {}; minutes a round: {}; she is in a room's exchange when she spoke there within {} min", plan.rooms.join(", "), plan.rounds, plan.repeats, plan.seed, plan.round_minutes, plan.exchange_minutes);
    let owners: Vec<String> = members.iter().filter(|member| !member.entry.own_rooms.is_empty()).map(|member| format!("{}: {}", member.entry.name, member.entry.own_rooms.join(", "))).collect();
    let _ = writeln!(out, "- own rooms (from the plan: neither the heartbeat's table nor `persona.env` records room ownership): {}", if owners.is_empty() { "none".to_owned() } else { owners.join("; ") });
    let _ = writeln!(out, "- wake: {}; beat: {}", policy_words(plan.wake), if plan.beat_s == 0 { "off".to_owned() } else { format!("every {} s of simulated time, one Persona by `tempo::pick`", plan.beat_s) });
    let _ = writeln!(out, "- group state in the Projector's packet: {}", match group_state { Some(true) => "on", Some(false) => "off", None => "unknown (the tuning did not load)" });
    let _ = writeln!(out, "- speech: the Interpreter's lines, each posted to its room; none is silence");
    let _ = writeln!(out, "- wakes are counted per wake, by its strongest cause: addressed by a human, a human line, addressed by a Mind, a Mind's line (control only), the beat. A Mind-addressed wake's evidence is its strongest: a Matrix mention or reply, her id written as text, a name in the text");
    let _ = writeln!(out, "- silence rate is per woken turn that did not fail; a failed call is counted apart. Reply chains are per room: Mind lines spoken in turns another Mind's line woke, in a row");
    let _ = writeln!(out, "- convergence is the mean pairwise cosine of speakers' word counts (lexical, cheap); work-talk share is a lexical heuristic over a fixed word list, not a classifier");
    let _ = writeln!(out, "- read-only: nothing is posted, no state is put, no Mind is asked\n");
    for run in runs {
        let metrics = &run["metrics"];
        let wakes = &metrics["wakes"];
        let _ = writeln!(out, "## Repeat {} (seed {})\n", run["repeat"], run["seed"]);
        let _ = writeln!(out, "### Metrics\n");
        let _ = writeln!(
            out,
            "- wakes: {} (addressed by a human {}, a human line {}, addressed by a Mind {}, a Mind's line {}, the beat {}); failed {}",
            metrics["turns"], wakes["addressed_by_human"], wakes["human"], wakes["addressed_by_mind"], wakes["mind"], wakes["beat"], metrics["failed"]
        );
        let _ = writeln!(out, "- Mind messages: {}; silence rate per woken turn: {}", metrics["mind_messages"], number(&metrics["silence_rate"]));
        if let Some(rooms) = metrics["reply_chains"].as_object() {
            for (room, chain) in rooms {
                let _ = writeln!(out, "- reply chains in {room}: longest {}, chains of 3 or more {}, Mind-to-Mind replies {}", chain["longest"], chain["chains_of_3_or_more"], chain["mind_to_mind_replies"]);
            }
        }
        let addressed = &metrics["mind_addressed_wakes"];
        let _ = writeln!(out, "- Mind-addressed wakes: {} (Matrix mention or reply {}, her id written as text {}, a name in the text {})", addressed["total"], addressed["mention_or_reply"], addressed["id_as_text"], addressed["name_in_text"]);
        let _ = writeln!(out, "- convergence of spoken lines: all {}, first half {}, second half {}", number(&metrics["convergence_lines"]["all"]), number(&metrics["convergence_lines"]["first_half"]), number(&metrics["convergence_lines"]["second_half"]));
        let _ = writeln!(out, "- convergence of prose: all {}, first half {}, second half {}", number(&metrics["convergence_prose"]["all"]), number(&metrics["convergence_prose"]["first_half"]), number(&metrics["convergence_prose"]["second_half"]));
        let _ = writeln!(out, "- work-talk share of Mind lines (heuristic): {}", number(&metrics["work_talk_share"]));
        let narration = &metrics["narration"];
        let _ = writeln!(out, "- narrations: {}; mean lexical overlap with what she heard {}; over {OVERLAP_FLAG}: {}; offering her options {}\n", narration["narrations"], number(&narration["mean_heard_overlap"]), narration["high_overlap"], narration["offers_options"]);
        let _ = writeln!(out, "| persona | wakes | messages | failed | silence rate | addressed |\n|---|---|---|---|---|---|");
        if let Some(personas) = metrics["personas"].as_object() {
            for (name, row) in personas {
                let named = row["addressed"].as_object().map(|map| map.iter().map(|(who, count)| format!("{who} {count}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let _ = writeln!(out, "| {name} | {} | {} | {} | {} | {named} |", row["turns"], row["messages"], row["failed"], number(&row["silence_rate"]));
            }
        }
        let _ = writeln!(out, "\n### Wakes by round\n\n| round | addressed by a human | a human line | addressed by a Mind | a Mind's line | beat |\n|---|---|---|---|---|---|");
        for row in metrics["wakes_by_round"].as_array().into_iter().flatten() {
            let causes = &row["causes"];
            let _ = writeln!(out, "| {} | {} | {} | {} | {} | {} |", row["round"], causes["addressed_by_human"], causes["human"], causes["addressed_by_mind"], causes["mind"], causes["beat"]);
        }
        let as_text = addressed["id_as_text_wakes"].as_array().cloned().unwrap_or_default();
        if !as_text.is_empty() {
            let _ = writeln!(out, "\nWakes whose only address was an id written as text (no mention, no reply):\n");
            for one in as_text {
                let by = one["by"].as_array().into_iter().flatten().map(|by| format!("{}: {}", by["from"].as_str().unwrap_or_default(), one_line(by["says"].as_str().unwrap_or_default()))).collect::<Vec<_>>().join("; ");
                let _ = writeln!(out, "- r{} {}: {by}", one["round"], one["persona"].as_str().unwrap_or_default());
            }
        }
        let transcript = run["transcript"].as_array().cloned().unwrap_or_default();
        let _ = writeln!(out, "\n### Representative lines\n");
        for line in representative(&transcript, REPRESENTATIVE_LINES) {
            let _ = writeln!(out, "{}", transcript_line(line));
        }
        let _ = writeln!(out, "\n### Transcript\n");
        for line in &transcript {
            let _ = writeln!(out, "{}", transcript_line(line));
        }
        let _ = writeln!(out, "\n### Turns\n");
        for turn in run["turns"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "<details><summary>round {}, {} (woke: {}, {} turn, {}): {}</summary>\n", turn["round"], turn["persona"].as_str().unwrap_or_default(), turn["woke"].as_str().unwrap_or_default(), turn["kind"].as_str().unwrap_or_default(), turn["room"].as_str().unwrap_or_default(), turn["outcome"].as_str().unwrap_or_default());
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
