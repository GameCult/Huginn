//! `mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>]`:
//! one start of the Persona organ (`mind_body::persona::carry`) inside
//! `mind-persona@<instance>`. This reads the process's arguments and
//! environment, the things the library never reads, and prints the outcome:
//! `dead` when a brake is held, `no organ` when they allow a turn, since she has
//! none until the three-stage turn lands. It exits 0 for both, and 2 when the
//! arguments or the environment are wrong.
//!
//! The persona flags (`--rooms`, `--operator`, `--minds`, `--self-sender`, or
//! `MIND_PERSONA_ROOMS`, `MIND_PERSONA_OPERATOR`, `MIND_PERSONA_MINDS`,
//! `MIND_PERSONA_SELF_SENDER` and `MIND_PERSONA_MODEL` from the unit's
//! `persona.env`) are still parsed, so a start refuses the configuration the turn
//! will need. Without `--endpoint` the daemon is `HUGINN_ENDPOINT`; no call is made
//! to it.
//!
//! `mind-persona project --instance <slug> [--endpoint rudp://<ip>:<port>] [--kind chat|beat|reflection] [--in <room>]
//! [--room <fixture.toml>] [--seed N] [--state <file>] [--select-only | --renders K [--through projector|persona]
//! [--judge <state-file>]]` is the experiment tool over the Projector and the Persona stage
//! (`mind_body::projection::project`): it prints, as JSON, the packet the Select stage builds from her stored state (or
//! `--state`, checked as a put checks its content, rules P1 to P3; P4 and P5 compare it with the stored document and are
//! not checked), and with `--renders` the Projector's narrations of K successive turns;
//! `--through persona` adds her prose and the quoted lines it would say; `--judge` renders a second state too and asks
//! the model which narration came from which. It writes nothing and reads no brake.
//!
//! Live, it reads `MIND_PERSONA_OPERATOR`, `MIND_PERSONA_MINDS` and `MIND_PERSONA_SELF_SENDER` (required), the heartbeat's
//! addressing from `MIND_HEARTBEAT_STATE`, which a run by hand must set as the unit does (`/var/lib/mind-heartbeat/heartbeat.cc`):
//! without it "said to her" knows only her own names, never her projects', and stderr says so before the report. The
//! room comes from the mouth when `BIFROST_CHAT_ENDPOINT` and
//! `BIFROST_CHAT_KEY_FILE` are set; `--in` keeps one room. `--room <fixture>` replaces all of that with a mocked room
//! (`projection::harness::Fixture`): her id, who is who, the messages, the clock and the "last heard from" table. With
//! `--room` and `--state` no daemon and no mouth is needed: without `--endpoint` or `HUGINN_ENDPOINT` none is reached.
//!
//! `mind-persona project --instance <slug> (--batch <plan.toml> | --simulate <plan.toml>) --out <report.md>` runs a plan
//! and writes one Markdown report: a batch of states by scenarios by samples (`harness::batch`), or the swarm's cast taking
//! turns in one simulated room (`harness::simulate`). Each case is one `project`.
//!
//! The tuning is `MIND_PERSONA_TUNING`, else `/etc/gamecult/minds/<instance>/persona-tuning.toml`; renders and plans need
//! `MIND_PERSONA_CONNECTOR_ENDPOINT` (`<loopback ip>:<port>`) and the `connector-key` credential in
//! `CREDENTIALS_DIRECTORY`. Without `--select-only` or `--renders` it renders once.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use eureka_state::HuginnClient;
use mind_body::chat_rudp::{RudpChat, connection_id};
use mind_body::cli::{instance_and_endpoint, instance_and_maybe_endpoint};
use mind_body::control::{CONTROL_ROOT, FileSource, control_path, swarm_path};
use mind_body::persona::{Config, carry};
use mind_body::heartbeat::{FileStore, HeartbeatView};
use mind_body::projection::harness::{Fixture, Harness, batch, fixture_request, simulate, state_file};
use mind_body::projection::{HuginnPersonaSource, KitModel, Model, MouthReader, Organs as ProjectOrgans, PersonaSource, Request as ProjectRequest, TUNING_FILE, TUNING_ROOT, Through, TurnKind, connector_key, project};

const USAGE: &str = "usage: mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>] [--rooms a,b] [--operator id[,id]] [--minds id[,id]] [--self-sender id]";
const PROJECT_USAGE: &str = "usage: mind-persona project --instance <slug> [--endpoint rudp://<ip>:<port>] [--kind chat|beat|reflection] [--in <room>] [--room <fixture.toml>] [--seed N] [--state <file>] [--select-only | --renders K [--through projector|persona] [--judge <state-file>]] | (--batch <plan> | --simulate <plan>) --out <report.md>";
/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How long the mouth may take to answer one read.
const CHAT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the kit may take to answer one Projector or judge turn.
const MODEL_TIMEOUT: Duration = Duration::from_secs(300);

// The binary is the one place that reads the process: its arguments and
// the environment. The library reads neither (clippy.toml, `#![forbid]` in lib.rs).
#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and environment, and hands them to the library")]
fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and environment, and hands them to the library")]
fn run() -> Result<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_config, rest) = Config::from_args_and_env(&args, setting, USAGE)?;
    let (instance, _endpoint) = instance_and_endpoint(&rest, setting("HUGINN_ENDPOINT"), USAGE)?;
    let control = FileSource::new(control_path(Path::new(CONTROL_ROOT), &instance)?);
    let swarm = FileSource::new(swarm_path(Path::new(CONTROL_ROOT)));
    Ok(carry(&control, &swarm, &instance).to_string())
}

fn required(name: &str) -> Result<String> {
    setting(name).ok_or_else(|| anyhow!("{name} is not set"))
}

/// The text of the credential file named by `name`; never echoed.
fn credential(name: &str) -> Result<String> {
    std::fs::read_to_string(required(name)?).with_context(|| format!("the file named by {name} could not be read"))
}

/// The credentials directory systemd hands the unit; its absence names the variable.
#[allow(clippy::disallowed_methods, reason = "the binary reads its own environment, and hands it to the library")]
fn credentials_directory() -> Result<PathBuf> {
    std::env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from).ok_or_else(|| anyhow!("CREDENTIALS_DIRECTORY is not set"))
}

fn run_project(args: &[String]) -> Result<String> {
    let (mut kind, mut here, mut fixture, mut seed, mut state, mut renders, mut select_only, mut judge, mut through) = (None, None, None, None, None, None, false, None, Through::Projector);
    let (mut batch_plan, mut simulate_plan, mut out, mut rest) = (None, None, None, Vec::new());
    let mut given = args.iter();
    while let Some(flag) = given.next() {
        let mut value = || given.next().cloned().ok_or_else(|| anyhow!("{PROJECT_USAGE}"));
        match flag.as_str() {
            "--kind" => {
                kind = Some(match value()?.as_str() {
                    "chat" => TurnKind::Chat,
                    "beat" => TurnKind::Beat,
                    "reflection" => TurnKind::Reflection,
                    _ => bail!("{PROJECT_USAGE}"),
                })
            }
            "--in" => here = Some(value()?),
            "--room" => fixture = Some(Fixture::load(Path::new(&value()?))?),
            "--seed" => seed = Some(value()?.parse::<u64>().map_err(|_| anyhow!("{PROJECT_USAGE}"))?),
            "--state" => state = Some(value()?),
            "--renders" => renders = Some(value()?.parse::<usize>().ok().filter(|k| *k > 0).ok_or_else(|| anyhow!("{PROJECT_USAGE}"))?),
            "--judge" => judge = Some(value()?),
            "--through" => {
                through = match value()?.as_str() {
                    "projector" => Through::Projector,
                    "persona" => Through::Persona,
                    "interpreter" => bail!("--through interpreter is not built yet (map C3b)"),
                    _ => bail!("{PROJECT_USAGE}"),
                }
            }
            "--batch" => batch_plan = Some(PathBuf::from(value()?)),
            "--simulate" => simulate_plan = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--select-only" => select_only = true,
            "--instance" | "--endpoint" => rest.extend([flag.clone(), value()?]),
            _ => bail!("{PROJECT_USAGE}"),
        }
    }
    let planned = batch_plan.is_some() || simulate_plan.is_some();
    // A plan carries its own states, rooms, seeds and stage; a fixture carries its own kind and room.
    if (select_only && (renders.is_some() || judge.is_some() || through != Through::Projector))
        || (batch_plan.is_some() && simulate_plan.is_some())
        || (planned && (out.is_none() || fixture.is_some() || state.is_some() || kind.is_some() || here.is_some() || seed.is_some() || renders.is_some() || judge.is_some() || select_only || through != Through::Projector))
        || (!planned && out.is_some())
        || (fixture.is_some() && (kind.is_some() || here.is_some()))
    {
        bail!("{PROJECT_USAGE}");
    }
    let (instance, address) = instance_and_maybe_endpoint(&rest, setting("HUGINN_ENDPOINT"), PROJECT_USAGE)?;
    let tuning = setting("MIND_PERSONA_TUNING").map_or_else(|| Path::new(TUNING_ROOT).join(&instance.0).join(TUNING_FILE), PathBuf::from);
    // Her stored state is asked only when no state is given; with a state file no daemon is needed or reached.
    let persona = address.map(|address| HuginnPersonaSource::new(HuginnClient::new(address, instance.clone(), TIMEOUT)));
    let renders = if select_only || planned { None } else { Some(renders.unwrap_or(1)) };
    let model = if renders.is_some() || planned {
        let endpoint: SocketAddr = required("MIND_PERSONA_CONNECTOR_ENDPOINT")?.parse().map_err(|_| anyhow!("MIND_PERSONA_CONNECTOR_ENDPOINT is not <ip>:<port>"))?;
        Some(KitModel::open(endpoint, connector_key(&credentials_directory()?)?, &instance.0, MODEL_TIMEOUT)?)
    } else {
        None
    };
    let harness = Harness { instance: instance.0.clone(), persona: persona.as_ref().map(|persona| persona as &dyn PersonaSource), model: model.as_ref().map(|model| model as &dyn Model), tuning };
    if let Some(out) = out {
        let report = match (&batch_plan, &simulate_plan) {
            (Some(plan), _) => batch(&harness, plan)?,
            (_, Some(plan)) => simulate(&harness, plan)?,
            _ => unreachable!("a plan was given"),
        };
        std::fs::write(&out, &report).with_context(|| format!("the report {} could not be written", out.display()))?;
        return Ok(format!("wrote {} ({} bytes)", out.display(), report.len()));
    }
    let state = state.map(|path| state_file(Path::new(&path), &instance.0)).transpose()?;
    let judge = judge.map(|path| state_file(Path::new(&path), &instance.0)).transpose()?;
    let seed_now = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default();
    if let Some(fixture) = &fixture {
        // The fixture is the room, the clock and who is who: no mouth, no heartbeat table, no persona.env.
        let organs = ProjectOrgans { persona: harness.persona, chat: Some(fixture), addressing: fixture.addressee(), model: harness.model };
        let request = ProjectRequest { judge_against: judge, ..fixture_request(&harness, fixture, state, seed.unwrap_or(seed_now), renders, through) };
        return Ok(serde_json::to_string_pretty(&project(&organs, &request, fixture.now)?)?);
    }
    let chat = match (setting("BIFROST_CHAT_ENDPOINT"), setting("BIFROST_CHAT_KEY_FILE")) {
        (Some(endpoint), Some(_)) => {
            let endpoint: SocketAddr = endpoint.strip_prefix("rudp://").and_then(|address| address.parse().ok()).ok_or_else(|| anyhow!("BIFROST_CHAT_ENDPOINT is not rudp://<ip>:<port>"))?;
            Some(RudpChat::new(endpoint, connection_id(&credential("BIFROST_CHAT_KEY_FILE")?), instance.0.clone(), CHAT_TIMEOUT))
        }
        _ => None,
    };
    let heartbeat = setting("MIND_HEARTBEAT_STATE").map(FileStore::new);
    let organs = ProjectOrgans {
        persona: harness.persona,
        chat: chat.as_ref().map(|chat| chat as &dyn MouthReader),
        addressing: heartbeat.as_ref().map(|store| store as &dyn HeartbeatView),
        model: harness.model,
    };
    let request = ProjectRequest {
        instance: instance.0.clone(),
        kind: kind.unwrap_or(TurnKind::Beat),
        state,
        room: here,
        seed: seed.unwrap_or(seed_now),
        renders,
        judge_against: judge,
        operators: setting("MIND_PERSONA_OPERATOR").map(|ids| ids.split(',').map(str::to_owned).collect()).unwrap_or_default(),
        minds: setting("MIND_PERSONA_MINDS").map(|ids| ids.split(',').map(str::to_owned).collect()).unwrap_or_default(),
        own_sender: required("MIND_PERSONA_SELF_SENDER")?,
        tuning: harness.tuning.clone(),
        through,
        last_heard: vec![],
        recent: vec![],
    };
    let report = project(&organs, &request, chrono::Utc::now())?;
    let addressing = &report["addressing"];
    // The addressing source is what decides "said to her": said first, where a person running this by hand sees it.
    eprintln!(
        "mind-persona project: addressing from {} ({}); table written {}",
        addressing["source"].as_str().unwrap_or("?"),
        addressing["why"].as_str().unwrap_or("?"),
        addressing["age_minutes"].as_i64().map_or_else(|| "never".to_owned(), |minutes| format!("{minutes} min ago")),
    );
    Ok(serde_json::to_string_pretty(&report)?)
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("project") {
        match run_project(&args[1..]) {
            Ok(report) => println!("{report}"),
            Err(error) => {
                eprintln!("mind-persona project: {error:#}");
                std::process::exit(2);
            }
        }
        return;
    }
    match run() {
        Ok(outcome) => println!("{outcome}"),
        Err(error) => {
            eprintln!("mind-persona: {error:#}");
            std::process::exit(2);
        }
    }
}
