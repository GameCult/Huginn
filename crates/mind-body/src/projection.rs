//! The Projector: her persona state and her room, turned into the lived stream a Persona continues.
//!
//! Two stages. The Select stage (`select`) is code: it decides what she can feel this turn and renders it as a typed
//! projection packet, deterministic for a seed. The narration (`narrate`) is one tool-less model call through the
//! connector that renders the packet as first-person prose under Ghostlight's membrane (`build_projector_prompt`,
//! the pinned `ghostlight-persona-projection`). Every knob of both is `persona-tuning.toml` (`tuning`), read at each
//! wake, its hash carried with the packet.
//!
//! `project` is the experiment tool over both (`mind-persona project`): what Select would build from her real or a
//! hypothetical state, and what the Projector would narrate. It has no port that writes: it puts no persona state,
//! admits nothing, sends and acks no chat and appends no trace, so it runs under a held brake.

mod narrate;
mod select;
mod tuning;

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use eureka_state::HuginnClient;
use huginn_mind::HuginnMindResponse;
use serde_json::{Value, json};

use crate::chat_client::{Chat, Form};
use crate::queue::MindPort;
use crate::recall;

pub use narrate::{KitModel, Model, connector_key};
pub use select::TurnKind;
pub use tuning::{TUNING_FILE, TUNING_ROOT};

use narrate::{cap_words, distinct_2, field_names, judge_prompt, judged, projector_prompt, word_budget, words};
use select::{History, Inputs, Packet, People, Recalled, Room, RoomLine, select};

/// The agent name on the connector's request ids.
const AGENT: &str = "mind-persona";
/// The touch of a recalled item by the lane that brought it.
const TOUCH_ABOUT: f64 = 1.0;
const TOUCH_SALIENT: f64 = 0.3;
const TOUCH_RELEVANT_UNSCORED: f64 = 0.6;
/// The judge passes at this share of narrations attributed to the right packet.
const JUDGE_PASS: f64 = 0.8;

/// Her persona state, read and never put.
pub trait PersonaSource {
    /// Her persona state and the receipt it was stored under.
    fn persona(&self) -> Result<(Value, String)>;
}

/// Her persona state from her daemon.
pub struct HuginnPersonaSource {
    client: HuginnClient,
}

impl HuginnPersonaSource {
    pub fn new(client: HuginnClient) -> Self {
        Self { client }
    }
}

impl PersonaSource for HuginnPersonaSource {
    fn persona(&self) -> Result<(Value, String)> {
        match self.client.persona_get().map_err(|_| anyhow!("the mind could not be asked for the persona"))? {
            HuginnMindResponse::Persona(Some(view)) => Ok((view.value, view.receipt_id)),
            HuginnMindResponse::Persona(None) => bail!("the mind holds no persona state"),
            _ => bail!("the mind answered the persona request with something else"),
        }
    }
}

/// What `project` reads through. None of them is asked to write.
pub struct Organs<'a> {
    /// Her stored state; not asked when the request carries a state.
    pub persona: Option<&'a dyn PersonaSource>,
    /// Her Mind, for recall; none recalls nothing.
    pub mind: Option<&'a dyn MindPort>,
    /// The mouth, for the room; none shows no room.
    pub chat: Option<&'a dyn Chat>,
    /// The Projector and the judge; required for renders.
    pub model: Option<&'a dyn Model>,
}

/// What to project.
pub struct Request {
    pub instance: String,
    pub kind: TurnKind,
    /// A hypothetical state in place of the stored one.
    pub state: Option<Value>,
    /// The room the turn is in; none is every room of hers.
    pub room: Option<String>,
    pub seed: u64,
    /// None builds the packet only. Some(k) narrates k successive turns of the same state, each seeded `seed + i` and
    /// each under the freshness the ones before it leave.
    pub renders: Option<usize>,
    /// A second state: renders both and asks the judge which narration came from which.
    pub judge_against: Option<Value>,
    pub operators: Vec<String>,
    pub own_sender: String,
    pub tuning: PathBuf,
}

/// The room through the mouth's pending read, which acks nothing: new messages and the tail, own lines kept for the
/// Select stage to drop. A refusal is reported, never hidden.
fn read_room(chat: &dyn Chat, request: &Request, public_name: &str) -> (Room, Value) {
    let pending = match chat.pending(0) {
        Ok(pending) => pending,
        Err(fault) => return (Room { here: request.room.clone(), ..Room::default() }, json!({ "read": false, "why": format!("{fault:#}") })),
    };
    let wanted = |room: &str, id: &str| request.room.as_deref().is_none_or(|here| here == room || here == id);
    let messages: Vec<RoomLine> = pending
        .deliveries
        .iter()
        .filter(|delivery| delivery.form == Form::Message && wanted(&delivery.room_alias, &delivery.room_id))
        .map(|delivery| RoomLine {
            room: delivery.room_alias.clone(),
            sender: delivery.sender.clone(),
            display: delivery.sender_display.clone(),
            text: delivery.text.clone(),
            at_ms: delivery.at_unix_ms,
            to_her: delivery.is_direct || delivery.mentions.contains(&request.own_sender) || (!public_name.is_empty() && crate::address::names_whole_word(&delivery.text, public_name, false)),
        })
        .collect();
    let tail: Vec<RoomLine> = pending
        .tail
        .iter()
        .flatten()
        .filter(|line| line.form == Form::Message && wanted(&line.room, &line.room))
        .map(|line| RoomLine { room: line.room.clone(), sender: line.sender.clone(), display: String::new(), text: line.body.clone(), at_ms: line.at_unix_ms, to_her: false })
        .collect();
    let report = json!({ "read": true, "messages": messages.len(), "tail": tail.len(), "tail_gaps": pending.tail_gaps });
    (Room { messages, tail, here: request.room.clone() }, report)
}

/// What her Mind recalls for this room, as Select candidates: memories of the tuning's `recall_sorts` and nothing else.
fn read_recall(mind: &dyn MindPort, room: &Room, sorts: &[String], now: DateTime<Utc>) -> Result<(Vec<Recalled>, Value)> {
    let lines = room.messages.iter().chain(room.tail.iter());
    let mut senders: Vec<&str> = lines.clone().map(|line| line.sender.as_str()).collect();
    senders.dedup();
    let names: BTreeMap<&str, &str> = lines.map(|line| (line.sender.as_str(), line.display.as_str())).filter(|(_, name)| !name.is_empty()).collect();
    let texts: Vec<&str> = room.messages.iter().rev().map(|line| line.text.as_str()).collect();
    let cue = recall::Cue { senders, texts, names, sorts, now };
    let recalled = recall::select(mind, &cue)?;
    let mut out = Vec::new();
    for shown in &recalled.lanes {
        for entry in &shown.entries {
            let touch = match shown.lane {
                recall::Lane::About => TOUCH_ABOUT,
                recall::Lane::Relevant => entry.nearness_permille.map_or(TOUCH_RELEVANT_UNSCORED, |near| f64::from(near.max(1)) / 1000.0),
                recall::Lane::Salient => TOUCH_SALIENT,
            };
            out.push(Recalled {
                id: entry.id.clone(),
                text: entry.line.trim().trim_start_matches("- ").to_owned(),
                salience: f64::from(entry.salience_permille) / 1000.0,
                admitted_at: DateTime::parse_from_rfc3339(&entry.admitted_at).ok().map(|at| at.with_timezone(&Utc)),
                touch,
            });
        }
    }
    let search = format!("{:?}", recalled.search).to_lowercase();
    let report = json!({ "asked": true, "sorts": sorts, "items": out.len(), "search": search });
    Ok((out, report))
}

fn packet_json(packet: &Packet) -> Value {
    json!({
        "kind": packet.kind,
        "seed": packet.seed,
        "lens": packet.lens,
        "present": packet.present,
        "selected": packet.selected,
        "bytes": { "identity": packet.identity.len(), "context": packet.context.len(), "identity_and_context": packet.identity.len() + packet.context.len(), "stimulus": packet.stimulus.len(), "heard": packet.heard.len() },
        "identity": packet.identity,
        "context": packet.context,
        "stimulus": packet.stimulus,
        "heard": packet.heard,
    })
}

/// One narration of one packet: the request id names the instance, the seed and the moment, because the kit's replay
/// store answers a repeated id with its first answer.
fn narrate_one(model: &dyn Model, instance: &str, packet: &Packet, tuning: &tuning::Tuning, tag: &str, now: DateTime<Utc>) -> Result<Value> {
    let request_id = format!("{AGENT}:{instance}:project:{tag}:{}:{}", packet.seed, now.timestamp_millis());
    let raw = model.ask(&request_id, &tuning.projector.model, &projector_prompt(packet, tuning))?;
    let budget = word_budget(packet.kind, tuning);
    let narration = cap_words(&raw, budget);
    let ids: Vec<String> = packet.selected.iter().map(|pick| pick.id.clone()).collect();
    Ok(json!({
        "seed": packet.seed,
        "lens": packet.lens,
        "selected": ids,
        "words": words(&narration),
        "cut_at_budget": words(&raw) > budget,
        "field_names": field_names(&narration, &ids),
        "narration": narration,
    }))
}

/// K successive turns over one state: each packet under the freshness the earlier ones left.
fn successive(inputs: &Inputs, tuning: &tuning::Tuning, renders: usize) -> Vec<Packet> {
    let mut history = inputs.history.clone();
    (0..renders)
        .map(|at| {
            let packet = select(&Inputs { seed: inputs.seed.wrapping_add(at as u64), history: &history, ..*inputs }, tuning);
            history.recent.push(packet.selected.iter().map(|pick| pick.id.clone()).collect());
            packet
        })
        .collect()
}

/// What Select builds, and with renders what the Projector narrates, as one JSON report. Writes nothing.
pub fn project(organs: &Organs, request: &Request, now: DateTime<Utc>) -> Result<Value> {
    let loaded = tuning::load(&request.tuning)?;
    let tuning = &loaded.tuning;
    let (state, receipt) = match &request.state {
        Some(state) => (state.clone(), Value::Null),
        None => {
            let (state, receipt) = organs.persona.ok_or_else(|| anyhow!("no state was given and her persona cannot be read"))?.persona()?;
            (state, Value::String(receipt))
        }
    };
    let public_name = state.get("publicName").and_then(Value::as_str).unwrap_or_default().to_owned();
    let (room, room_report) = match organs.chat {
        Some(chat) => read_room(chat, request, &public_name),
        None => (Room { here: request.room.clone(), ..Room::default() }, json!({ "read": false, "why": "no mouth is configured" })),
    };
    let (recall, recall_report) = match organs.mind {
        Some(mind) => read_recall(mind, &room, &tuning.select.recall_sorts, now)?,
        None => (Vec::new(), json!({ "asked": false, "sorts": tuning.select.recall_sorts })),
    };
    // Freshness and her own recent words come from trace v2, which the commit writes (C3c); until then there is none.
    let history = History::default();
    let people = People { operators: &request.operators, own_sender: &request.own_sender };
    let inputs = Inputs { kind: request.kind, state: &state, room: &room, recall: &recall, history: &history, observation: None, people, now, seed: request.seed };

    let mut report = json!({
        "instance": request.instance,
        "persona_receipt": receipt,
        "state": if request.state.is_some() { "given" } else { "stored" },
        "tuning": { "path": request.tuning.display().to_string(), "source": loaded.source, "sha256": loaded.sha256 },
        "room": room_report,
        "recall": recall_report,
        "history": "none: trace v2 is not written yet",
    });
    let Some(renders) = request.renders else {
        report["packet"] = packet_json(&select(&inputs, tuning));
        return Ok(report);
    };
    let model = organs.model.ok_or_else(|| anyhow!("renders need the connector, and none is configured"))?;
    report["model"] = json!(tuning.projector.model);
    let packets = successive(&inputs, tuning, renders.max(1));
    report["packet"] = packet_json(&packets[0]);
    let narrations: Vec<Value> = packets.iter().map(|packet| narrate_one(model, &request.instance, packet, tuning, "a", now)).collect::<Result<_>>()?;
    let texts = |narrations: &[Value]| narrations.iter().map(|one| one["narration"].as_str().unwrap_or_default().to_owned()).collect::<Vec<_>>();
    report["distinct_2"] = json!(distinct_2(&texts(&narrations)));
    report["embedding_distance"] = json!("not measured: the daemon offers no embedding verb");
    let leaked = narrations.iter().any(|one| one["field_names"].as_array().is_some_and(|names| !names.is_empty()));
    report["field_names_found"] = json!(leaked);

    if let Some(other) = &request.judge_against {
        let other_inputs = Inputs { state: other, ..inputs };
        let others = successive(&other_inputs, tuning, renders.max(1));
        let other_narrations: Vec<Value> = others.iter().map(|packet| narrate_one(model, &request.instance, packet, tuning, "b", now)).collect::<Result<_>>()?;
        let mut verdicts = Vec::new();
        let mut right = 0usize;
        for (at, (narration, truth)) in narrations.iter().map(|one| (one, 'A')).chain(other_narrations.iter().map(|one| (one, 'B'))).enumerate() {
            let (a, b) = (&packets[at % packets.len()], &others[at % others.len()]);
            // The two descriptions swap places on odd turns, so a judge that always answers one letter scores half.
            let swapped = at % 2 == 1;
            let (first, second) = if swapped { (b, a) } else { (a, b) };
            let text = narration["narration"].as_str().unwrap_or_default();
            let answer = model.ask(&format!("{AGENT}:{}:judge:{at}:{}", request.instance, now.timestamp_millis()), &tuning.projector.model, &judge_prompt(first, second, text))?;
            let letter = judged(&answer).map(|letter| if swapped { if letter == 'A' { 'B' } else { 'A' } } else { letter });
            let correct = letter == Some(truth);
            right += usize::from(correct);
            verdicts.push(json!({ "from": truth.to_string(), "judged": letter.map(String::from), "correct": correct }));
        }
        let accuracy = right as f64 / verdicts.len().max(1) as f64;
        report["against"] = json!({ "packet": packet_json(&others[0]), "renders": other_narrations, "distinct_2": distinct_2(&texts(&other_narrations)) });
        report["judge"] = json!({ "accuracy": accuracy, "pass_at": JUDGE_PASS, "passed": accuracy >= JUDGE_PASS && !leaked, "verdicts": verdicts });
    }
    report["renders"] = Value::Array(narrations);
    Ok(report)
}

#[cfg(test)]
mod tests;
