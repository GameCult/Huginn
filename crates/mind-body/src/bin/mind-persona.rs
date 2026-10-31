//! `mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>] [--max-turns n]`:
//! one start of the Persona organ (`mind_body::persona::start`) inside
//! `mind-persona@<instance>`. This reads the process's arguments and
//! environment, the things the library never reads, and prints the outcome. It
//! exits 0 for every outcome, including a unit that is dead because the brake is
//! held or the dial out of bounds, and 2 when the mind cannot be reached, the
//! arguments or the environment are wrong or a transport is not configured.
//!
//! The unit's `persona.env` and credentials name everything else:
//! `MIND_PERSONA_ROOMS` (a,b), `MIND_PERSONA_OPERATOR`
//! (id[,id]) and `MIND_PERSONA_SELF_SENDER`, each also a flag that overrides it
//! (`--rooms`, `--operator`, `--self-sender`), and `MIND_HEARTBEAT_STATE` (optional: the path of the
//! heartbeat's `heartbeat.cc`, which the unit reads and never writes, for the tempo in her input); `MIND_PERSONA_DOCTRINE`
//! (optional, absolute paths separated by `:`) names the doctrine files read at each
//! turn start and put ahead of her persona;
//! `MIND_CONNECTOR_ENDPOINT` (`<ip>:<port>`, the connector's loopback listener),
//! `MIND_CONNECTOR_KEY_FILE` and `MIND_CONNECTOR_MODEL`; `BIFROST_CHAT_ENDPOINT`
//! (`rudp://<ip>:<port>`, the mouth) and `BIFROST_CHAT_KEY_FILE`. Without
//! `--endpoint` the daemon is `HUGINN_ENDPOINT`; the trace is `persona-trace.cc`
//! under `STATE_DIRECTORY`. Key files are read here and their text goes to the
//! transports, which hold it; nothing is printed.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use eureka_state::HuginnClient;
use mind_body::chat_rudp::{RudpChat, connection_id};
use mind_body::cli::instance_and_endpoint;
use mind_body::connector::{self, KitConnector};
use mind_body::control::{CONTROL_ROOT, FileSource, control_path, swarm_path};
use mind_body::launch::{SystemClock, SystemdLauncher};
use mind_body::heartbeat::FileStore;
use mind_body::persona::{Config, FileTrace, HuginnPersona, Organs, start};
use mind_body::queue::HuginnMind;

/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How long the mouth may take to answer one chat request (a send is a Matrix PUT).
const CHAT_TIMEOUT: Duration = Duration::from_secs(20);

const USAGE: &str = "usage: mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>] [--max-turns n] [--rooms a,b] [--operator id[,id]] [--self-sender id]";

fn host() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|name| name.trim().to_string()).ok().filter(|name| !name.is_empty()).unwrap_or_else(|| "unknown".into())
}

// The binary is the one place that reads the process: its arguments and
// the environment. The library reads neither (clippy.toml, `#![forbid]` in lib.rs).
#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and environment, and hands them to the library")]
fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn required(name: &str) -> Result<String> {
    setting(name).ok_or_else(|| anyhow!("{name} is not set"))
}

/// The text of the credential file named by `name`; never echoed.
fn credential(name: &str) -> Result<String> {
    std::fs::read_to_string(required(name)?).with_context(|| format!("the file named by {name} could not be read"))
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and environment, and hands them to the library")]
fn run() -> Result<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (config, rest) = Config::from_args_and_env(&args, setting, USAGE)?;
    let (instance, address) = instance_and_endpoint(&rest, setting("HUGINN_ENDPOINT"), USAGE)?;
    let Some(state) = setting("STATE_DIRECTORY") else { bail!("no STATE_DIRECTORY") };
    let chat_endpoint: SocketAddr = required("BIFROST_CHAT_ENDPOINT")?
        .strip_prefix("rudp://")
        .and_then(|address| address.parse().ok())
        .ok_or_else(|| anyhow!("BIFROST_CHAT_ENDPOINT is not rudp://<ip>:<port>"))?;
    let chat = RudpChat::new(chat_endpoint, connection_id(&credential("BIFROST_CHAT_KEY_FILE")?), instance.0.clone(), CHAT_TIMEOUT);
    let connector = KitConnector::new(connector::endpoint(&required("MIND_CONNECTOR_ENDPOINT")?).context("MIND_CONNECTOR_ENDPOINT must be <ip>:<port> or tcp://<ip>:<port>")?, &credential("MIND_CONNECTOR_KEY_FILE")?, required("MIND_CONNECTOR_MODEL")?, instance.0.clone())?;
    let source = FileSource::new(control_path(Path::new(CONTROL_ROOT), &instance)?);
    let mind = HuginnMind::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let persona = HuginnPersona::new(HuginnClient::new(address, instance, TIMEOUT));
    let trace = FileTrace::new(Path::new(&state).join("persona-trace.cc"));
    let launcher = SystemdLauncher::new();
    let host = host();
    let tempo = setting("MIND_HEARTBEAT_STATE").map(FileStore::new);
    let organs = Organs { mind: &mind, launcher: &launcher, clock: &SystemClock, host: &host, chat: &chat, connector: &connector, persona: &persona, trace: &trace, tempo: tempo.as_ref().map(|store| store as &dyn mind_body::heartbeat::TempoView) };
    let swarm = FileSource::new(swarm_path(Path::new(CONTROL_ROOT)));
    Ok(start(&organs, &source, &swarm, &config)?.to_string())
}

fn main() {
    match run() {
        Ok(outcome) => println!("{outcome}"),
        Err(error) => {
            eprintln!("mind-persona: {error:#}");
            std::process::exit(2);
        }
    }
}
