//! The Projector: her persona state and her room, turned into the lived stream a Persona continues.
//!
//! Two stages. The Select stage (`select`) is code: it decides what she can feel this turn and renders it as a typed
//! projection packet, deterministic for a seed. The narration (`narrate`) is one tool-less model call through the
//! connector that renders the packet as first-person prose under Ghostlight's membrane (`build_projector_prompt`,
//! the pinned `ghostlight-persona-projection`). Every knob of both is `persona-tuning.toml` (`tuning`), read at each
//! wake, its hash carried with the packet.
//!
//! The Projector shows her self and her room: her persona state and what the mouth showed. It does not read her Mind;
//! that is the Interpreter's read tools' (C3b), when wondering leads to reading.
//!
//! `project` is the experiment tool over both (`mind-persona project`): what Select would build from her real or a
//! hypothetical state, what the Projector would narrate, and with `Through::Persona` what she would answer in her own
//! prose and which quoted lines of it would be said (the Persona stage, C3b's first piece). `harness` runs `project`
//! over fixtures: one turn from a mocked room (`--room`), a plan of states by scenarios (`--batch`), and the cast of the
//! swarm taking turns in one simulated room (`--simulate`). It adds no second turn: every case is one `project`. Its organs are read-only by type: `PersonaSource` gets,
//! `MouthReader` reads pending chat, `Model` asks. None of them can put, admit, send, ack or open a conversation, so
//! `project` cannot write and runs under a held brake.

pub mod harness;
mod interpret;
mod narrate;
mod select;
mod simulate;
mod tuning;

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use eureka_state::HuginnClient;
use huginn_mind::HuginnMindResponse;
use serde_json::{Value, json};

use crate::address::{Addressee, is_for, localpart};
use crate::chat_client::{Chat, ChatPending, Form};
use crate::heartbeat::HeartbeatView;

pub use narrate::{Answer, Failed, KitModel, Model, Spent, connector_key};
pub use select::TurnKind;
pub use tuning::{TUNING_FILE, TUNING_ROOT};

use narrate::{cap_words, distinct_2, field_names, heard_overlap, judge_prompt, judged, offers_options, persona_prompt, projector_prompt, word_budget, words};
use select::{History, Inputs, Packet, People, Room, RoomLine, select};

/// The agent name on the connector's request ids.
const AGENT: &str = "mind-persona";
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

/// The mouth, read: pending deliveries and the room tail, which acks nothing. The only chat port `project` holds.
pub trait MouthReader {
    fn pending(&self) -> Result<ChatPending>;
}

impl<T: Chat> MouthReader for T {
    fn pending(&self) -> Result<ChatPending> {
        Chat::pending(self, 0)
    }
}

/// What `project` reads through. Each is a read-only port by type: holding an `Organs`, no code can send, ack or open
/// a conversation, or put her state.
///
/// ```compile_fail
/// fn acks(organs: &mind_body::projection::Organs) {
///     organs.chat.unwrap().ack(1).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn sends(organs: &mind_body::projection::Organs, request: &mind_body::chat_client::ChatSendRequest) {
///     organs.chat.unwrap().send(request).unwrap();
/// }
/// ```
pub struct Organs<'a> {
    /// Her stored state; not asked when the request carries a state.
    pub persona: Option<&'a dyn PersonaSource>,
    /// The mouth, for the room; none shows no room.
    pub chat: Option<&'a dyn MouthReader>,
    /// The heartbeat's addressing table, read-only: her names and her projects' names, the ones the heartbeat woke her
    /// by. None, or a table without her, addresses her by her own names only, and the report says so.
    pub addressing: Option<&'a dyn HeartbeatView>,
    /// The Projector and the judge; required for renders.
    pub model: Option<&'a dyn Model>,
}

/// How far a render runs: the Projector's narration, or on through the Persona stage to her prose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Through {
    #[default]
    Projector,
    Persona,
    /// On through the Interpreter's speech step: what she says aloud, decided from her prose (`interpret`).
    Interpreter,
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
    /// The Matrix ids of the swarm's Minds: their bonds name them by localpart; anyone else is keyed by full id.
    pub minds: Vec<String>,
    /// Her own Matrix id. Required: without it her own lines would read as said to her.
    pub own_sender: String,
    pub tuning: PathBuf,
    pub through: Through,
    /// When she last heard from people (Matrix id, unix ms) beyond the room's tail: a fixture's table. Empty live.
    pub last_heard: Vec<(String, u64)>,
    /// The ids each earlier projection selected, oldest first, for freshness: a simulation's earlier turns of hers.
    /// Empty live until trace v2 (C3c) carries them.
    pub recent: Vec<Vec<String>>,
    /// The rooms that are hers, by the key the lines carry: a simulation's plan. The live body records none yet.
    pub own_rooms: Vec<String>,
}

/// Her addressee: the heartbeat's row for her when its table holds one, else her own names (handle and public name).
/// Her addressee and where it came from: `{source, why, written_at, age_minutes}`. The heartbeat's row when its table
/// holds one; else her own names (handle and public name), with why: no store configured, no heartbeat file, a file
/// that could not be read, or a table without her row. The table's age is the store's last write: under a held swarm
/// brake the heartbeat does not poll, so an old table is a stale one.
fn own_addressee(addressing: Option<&dyn HeartbeatView>, own_sender: &str, public_name: &str, now: DateTime<Utc>) -> (Addressee, Value) {
    let written_at = addressing.and_then(HeartbeatView::written_at);
    let age = written_at.map(|at| (now - at).num_minutes());
    let report = |source: &str, why: &str| json!({ "source": source, "why": why, "written_at": written_at.map(|at| at.to_rfc3339()), "age_minutes": age });
    let table = addressing.map(|view| view.addressing());
    if let Some(Ok(Some(table))) = &table
        && let Some(row) = table.minds.iter().find(|row| row.matrix_id() == own_sender)
    {
        return (row.clone(), report("heartbeat", "her row in the heartbeat's addressing table"));
    }
    let why = match (&table, written_at) {
        (None, _) => "no heartbeat addressing is configured (MIND_HEARTBEAT_STATE is not set)".to_owned(),
        (Some(Err(fault)), _) => format!("the heartbeat addressing could not be read: {fault:#}"),
        (Some(Ok(Some(_))), _) => "the heartbeat's addressing table has no row for her".to_owned(),
        (Some(Ok(None)), None) => "there is no heartbeat file".to_owned(),
        (Some(Ok(None)), Some(_)) => "the heartbeat file holds no addressing table".to_owned(),
    };
    (Addressee::new(own_sender, [localpart(own_sender), public_name]), report("own names only", &why))
}

/// The room through the mouth's pending read, which acks nothing: new messages and the tail, own lines kept for the
/// Select stage to drop. A refusal is reported, never hidden.
fn read_room(chat: &dyn MouthReader, request: &Request, own: &Addressee) -> (Room, Value) {
    let pending = match chat.pending() {
        Ok(pending) => pending,
        Err(fault) => return (Room { here: request.room.clone(), last_heard: request.last_heard.clone(), own: request.own_rooms.clone(), ..Room::default() }, json!({ "read": false, "why": format!("{fault:#}") })),
    };
    // One key names a room everywhere: the one the tail carries, the alias for a public room and the id for a direct
    // message (the mouth's alias of a direct room is its id). No table in her body maps a public room's id to its
    // alias, so a public room given by its id matches nothing, and the report says so rather than guessing.
    let here = request.room.clone();
    let wanted = |room: &str| here.as_deref().is_none_or(|here| here == room);
    let messages: Vec<RoomLine> = pending
        .deliveries
        .iter()
        .filter(|delivery| delivery.form == Form::Message && wanted(&delivery.room_alias))
        .map(|delivery| RoomLine {
            room: delivery.room_alias.clone(),
            sender: delivery.sender.clone(),
            display: delivery.sender_display.clone(),
            text: delivery.text.clone(),
            at_ms: delivery.at_unix_ms,
            // The heartbeat's rule, the one that woke her: never a second opinion here.
            to_her: is_for(own, delivery),
        })
        .collect();
    let tail: Vec<RoomLine> = pending
        .tail
        .iter()
        .flatten()
        .filter(|line| line.form == Form::Message && wanted(&line.room))
        .map(|line| RoomLine { room: line.room.clone(), sender: line.sender.clone(), display: String::new(), text: line.body.clone(), at_ms: line.at_unix_ms, to_her: false })
        .collect();
    let mut report = json!({ "read": true, "messages": messages.len(), "tail": tail.len(), "tail_gaps": pending.tail_gaps });
    if let Some(here) = &here
        && messages.is_empty()
        && tail.is_empty()
        && here.starts_with('!')
    {
        report["why"] = json!("no line carries this room id: a public room is named by its alias (#name:server), a direct message by its id");
    }
    let direct = pending.deliveries.iter().filter(|delivery| delivery.is_direct).map(|delivery| delivery.room_alias.clone()).collect();
    (Room { messages, tail, here, last_heard: request.last_heard.clone(), direct, own: request.own_rooms.clone() }, report)
}

fn packet_json(packet: &Packet) -> Value {
    json!({
        "kind": packet.kind,
        "seed": packet.seed,
        "lens": packet.lens,
        "present": packet.present,
        "selected": packet.selected,
        "bytes": { "identity": packet.identity.len(), "context": packet.context.len(), "identity_and_context": packet.identity.len() + packet.context.len(), "stimulus": packet.stimulus.len(), "heard": packet.heard.len() },
        "overflow": packet.overflow,
        "identity": packet.identity,
        "context": packet.context,
        "stimulus": packet.stimulus,
        "heard": packet.heard,
    })
}

/// A request id no earlier call carried: the kit's replay store answers a repeated id with its first answer, and a
/// turn's clock may be a fixture's, the same on every run, so the id takes the wall clock.
fn request_id(instance: &str, stage: &str, tag: &str, seed: u64) -> String {
    format!("{AGENT}:{instance}:project:{stage}:{tag}:{seed}:{}", Utc::now().timestamp_nanos_opt().unwrap_or_default())
}

/// One render of one packet: the Projector's narration and, through the Persona stage, her prose over it, each its own
/// call with its usage. A failed call is a record with its reason and what it cost, never the end of the report.
fn narrate_one(model: &dyn Model, instance: &str, packet: &Packet, tuning: &tuning::Tuning, tag: &str, through: Through) -> Value {
    let ids: Vec<String> = packet.selected.iter().map(|pick| pick.id.clone()).collect();
    let answer = match model.ask(&request_id(instance, "projector", tag, packet.seed), &tuning.projector.model, &projector_prompt(packet, tuning)) {
        Ok(answer) => answer,
        Err(failed) => return json!({ "seed": packet.seed, "lens": packet.lens, "selected": ids, "failed": failed.reason, "usage": failed.usage }),
    };
    let raw = answer.text;
    let budget = word_budget(packet.kind, tuning);
    let narration = cap_words(&raw, budget);
    let named: Vec<String> = packet.ids.clone();
    let mut render = json!({
        "seed": packet.seed,
        "lens": packet.lens,
        "selected": ids,
        "words": words(&narration),
        "cut_at_budget": words(&raw) > budget,
        "field_names": field_names(&narration, &named),
        "heard_overlap": heard_overlap(&narration, &packet.heard),
        "offers_options": offers_options(&narration),
        "usage": answer.usage,
        "narration": narration,
    });
    if through == Through::Projector {
        return render;
    }
    let prose = match model.ask(&request_id(instance, "persona", tag, packet.seed), &tuning.persona.model, &persona_prompt(packet, &narration, tuning)) {
        Ok(spoke) => {
            let prose = cap_words(&spoke.text, tuning.persona.words);
            render["persona"] = json!({ "words": words(&prose), "cut_at_budget": words(&spoke.text) > tuning.persona.words, "usage": spoke.usage, "prose": prose });
            prose
        }
        Err(failed) => {
            render["persona"] = json!({ "failed": failed.reason, "usage": failed.usage });
            return render;
        }
    };
    if through == Through::Interpreter {
        render["interpreter"] = interpret::decide(model, &request_id(instance, "interpreter", tag, packet.seed), packet, &narration, &prose, tuning);
    }
    render
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
    if !request.own_sender.starts_with('@') || !request.own_sender.contains(':') {
        bail!("her own Matrix id (MIND_PERSONA_SELF_SENDER) is required: without it her own lines would read as said to her");
    }
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
    let (own, addressing) = own_addressee(organs.addressing, &request.own_sender, &public_name, now);
    let (room, room_report) = match organs.chat {
        Some(chat) => read_room(chat, request, &own),
        None => (Room { here: request.room.clone(), last_heard: request.last_heard.clone(), own: request.own_rooms.clone(), ..Room::default() }, json!({ "read": false, "why": "no mouth is configured" })),
    };
    // Freshness and her own recent words come from trace v2, which the commit writes (C3c); until then only what the
    // caller carried from earlier turns of its own.
    let history = History { recent: request.recent.clone(), ..History::default() };
    let people = People { operators: &request.operators, minds: &request.minds, own_sender: &request.own_sender };
    let inputs = Inputs { kind: request.kind, state: &state, room: &room, history: &history, observation: None, people, now, seed: request.seed };

    let mut report = json!({
        "instance": request.instance,
        "persona_receipt": receipt,
        "state": if request.state.is_some() { "given" } else { "stored" },
        "tuning": { "path": request.tuning.display().to_string(), "source": loaded.source, "sha256": loaded.sha256 },
        "room": room_report,
        "addressing": addressing,
        "history": if request.recent.is_empty() { json!("none: freshness and her own recent words come from trace v2, which the commit writes (C3c)") } else { json!({ "recent_projections": request.recent.len() }) },
    });
    let Some(renders) = request.renders else {
        report["packet"] = packet_json(&select(&inputs, tuning));
        return Ok(report);
    };
    let model = organs.model.ok_or_else(|| anyhow!("renders need the connector, and none is configured"))?;
    report["model"] = json!(tuning.projector.model);
    if request.through != Through::Projector {
        report["persona_model"] = json!(tuning.persona.model);
    }
    if request.through == Through::Interpreter {
        report["interpreter_model"] = json!(tuning.interpreter.model);
    }
    let packets = successive(&inputs, tuning, renders.max(1));
    report["packet"] = packet_json(&packets[0]);
    let narrations: Vec<Value> = packets.iter().map(|packet| narrate_one(model, &request.instance, packet, tuning, "a", request.through)).collect();
    let texts = |narrations: &[Value]| narrations.iter().filter_map(|one| one["narration"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    let failed = |narrations: &[Value]| narrations.iter().filter(|one| one.get("failed").is_some()).count();
    report["failed_renders"] = json!(failed(&narrations));
    report["distinct_2"] = json!(distinct_2(&texts(&narrations)));
    report["embedding_distance"] = json!("not measured: the daemon offers no embedding verb");
    let leaked = narrations.iter().any(|one| one["field_names"].as_array().is_some_and(|names| !names.is_empty()));
    report["field_names_found"] = json!(leaked);

    if let Some(other) = &request.judge_against {
        let other_inputs = Inputs { state: other, ..inputs };
        let others = successive(&other_inputs, tuning, renders.max(1));
        let other_narrations: Vec<Value> = others.iter().map(|packet| narrate_one(model, &request.instance, packet, tuning, "b", Through::Projector)).collect();
        let mut verdicts = Vec::new();
        let mut right = 0usize;
        for (at, (narration, truth)) in narrations.iter().map(|one| (one, 'A')).chain(other_narrations.iter().map(|one| (one, 'B'))).enumerate() {
            let (a, b) = (&packets[at % packets.len()], &others[at % others.len()]);
            // The two descriptions swap places on odd turns, so a judge that always answers one letter scores half.
            let swapped = at % 2 == 1;
            let (first, second) = if swapped { (b, a) } else { (a, b) };
            // A narration that failed has nothing to judge; it is counted under the renders' failures.
            let Some(text) = narration["narration"].as_str() else { continue };
            let answer = match model.ask(&request_id(&request.instance, "judge", &at.to_string(), request.seed), &tuning.projector.model, &judge_prompt(first, second, text)) {
                Ok(answer) => answer,
                Err(failed) => {
                    verdicts.push(json!({ "from": truth.to_string(), "failed": failed.reason, "usage": failed.usage }));
                    continue;
                }
            };
            let letter = judged(&answer.text).map(|letter| if swapped { if letter == 'A' { 'B' } else { 'A' } } else { letter });
            let correct = letter == Some(truth);
            right += usize::from(correct);
            verdicts.push(json!({ "from": truth.to_string(), "judged": letter.map(String::from), "correct": correct, "usage": answer.usage }));
        }
        // Accuracy over the verdicts the judge gave; a failed call is no verdict, and any failure fails the eval.
        let judged_count = verdicts.iter().filter(|verdict| verdict.get("failed").is_none()).count();
        let failures = verdicts.len() - judged_count + failed(&narrations) + failed(&other_narrations);
        let accuracy = right as f64 / judged_count.max(1) as f64;
        report["against"] = json!({ "packet": packet_json(&others[0]), "renders": other_narrations, "distinct_2": distinct_2(&texts(&other_narrations)) });
        report["judge"] = json!({ "accuracy": accuracy, "pass_at": JUDGE_PASS, "failures": failures, "passed": accuracy >= JUDGE_PASS && !leaked && failures == 0, "verdicts": verdicts });
    }
    report["renders"] = Value::Array(narrations);
    Ok(report)
}

#[cfg(test)]
mod tests;
