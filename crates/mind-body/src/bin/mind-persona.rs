//! `mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>]`:
//! one start of the Persona organ (`mind_body::persona::carry`) inside
//! `mind-persona@<instance>`. This reads the process's arguments and
//! environment, the things the library never reads, and prints the outcome. It
//! exits 0 for every outcome, including a unit that is dead because the brake is
//! held or the dial out of bounds, and 2 when the mind cannot be reached, the
//! arguments or the environment are wrong or a transport is not configured.
//!
//! The unit's `persona.env` and credentials name everything else:
//! `MIND_PERSONA_ROOMS` (a,b), `MIND_PERSONA_OPERATOR`
//! (id[,id]), `MIND_PERSONA_MINDS` (id[,id]: every Mind of the swarm, required) and `MIND_PERSONA_SELF_SENDER`, each also a flag that overrides it
//! (`--rooms`, `--operator`, `--minds`, `--self-sender`), and `MIND_HEARTBEAT_STATE` (optional: the path of the
//! heartbeat's `heartbeat.cc`, which the unit reads and never writes, for the tempo in her input); `MIND_PERSONA_DOCTRINE`
//! (optional, absolute paths separated by `:`) names the doctrine files read at each
//! process start and put ahead of her persona; `MIND_PERSONA_MODEL` is the model her session runs;
//! `MIND_PERSONA_RUN_CAP_S` is the unit's `RuntimeMaxSec` in seconds, the one length of a start;
//! `MIND_PERSONA_CONFIG_SEED` optionally names a root-owned directory copied into each
//! process's fresh configuration; `CLAUDE_LEASE_SOCKET` is the connector's lease socket (the token itself
//! is never in this process's environment, only the descriptor a lease grants); `RUNTIME_DIRECTORY` (the
//! unit's `RuntimeDirectory=`) holds each process's configuration and instructions; `BIFROST_CHAT_ENDPOINT`
//! (`rudp://<ip>:<port>`, the mouth) and `BIFROST_CHAT_KEY_FILE`; `BIFROST_CHAT_PENDING_V4` set to exactly `1` asks the mouth for the
//! reply link (`chat_pending.v4`), anything else or unset does not. `BIFROST_CHAT_FETCH` set to exactly `1` lets her ask the mouth to keep one event's image (`save_attachment`); the operator sets it only once the mouth runs `CHAT_FETCH=1`. Without
//! `--endpoint` the daemon is `HUGINN_ENDPOINT`; the trace is `persona-trace.cc`
//! under `STATE_DIRECTORY`, and her sandbox is `STATE_DIRECTORY/sandbox`. The key file is read here and its
//! text goes to the transport, which holds it; nothing is printed.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use eureka_state::HuginnClient;
use mind_body::chat_rudp::{RudpChat, connection_id};
use mind_body::cli::instance_and_endpoint;
use mind_body::control::{CONTROL_ROOT, FileSource, control_path, swarm_path};
use mind_body::launch::{SystemClock, SystemdLauncher};
use mind_body::heartbeat::FileStore;
use mind_body::lease_client::{LEASE_SOCKET_VARIABLE, SocketLender};
use mind_body::persona::{Config, ENV_CONFIG_SEED, ENV_RUN_CAP, FileTrace, HuginnPersona, Organs, SANDBOX_DIR, carry};
use mind_body::queue::HuginnMind;
use mind_body::self_run::{ClaudeSpawner, SCRATCH_VARIABLE};

/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How long the mouth may take to answer one chat request (a send is a Matrix PUT).
const CHAT_TIMEOUT: Duration = Duration::from_secs(20);

const USAGE: &str = "usage: mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>] [--rooms a,b] [--operator id[,id]] [--minds id[,id]] [--self-sender id]";

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
    let reply_link = setting("BIFROST_CHAT_PENDING_V4").as_deref() == Some("1");
    let chat = RudpChat::new(chat_endpoint, connection_id(&credential("BIFROST_CHAT_KEY_FILE")?), instance.0.clone(), CHAT_TIMEOUT).with_reply_link(reply_link);
    let run_cap_s: u64 = setting(ENV_RUN_CAP).and_then(|seconds| seconds.parse().ok()).filter(|seconds| *seconds > 0).ok_or_else(|| anyhow!("{ENV_RUN_CAP} is not set to the unit's RuntimeMaxSec in seconds"))?;
    let scratch = PathBuf::from(required(SCRATCH_VARIABLE)?);
    let sandbox = Path::new(&state).join(SANDBOX_DIR);
    let lender = SocketLender::new(required(LEASE_SOCKET_VARIABLE)?);
    let spawner = ClaudeSpawner::in_sandbox(std::env::vars_os(), ENV_CONFIG_SEED, Some(sandbox.clone()))?;
    let source = FileSource::new(control_path(Path::new(CONTROL_ROOT), &instance)?);
    let mind = HuginnMind::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let persona = HuginnPersona::new(HuginnClient::new(address, instance, TIMEOUT));
    let trace = FileTrace::new(Path::new(&state).join("persona-trace.cc"));
    let launcher = SystemdLauncher::new();
    let host = host();
    let heartbeat = setting("MIND_HEARTBEAT_STATE").map(FileStore::new);
    let organs = Organs { mind: &mind, launcher: &launcher, clock: &SystemClock, host: &host, chat: &chat, persona: &persona, trace: &trace, heartbeat: heartbeat.as_ref().map(|store| store as &dyn mind_body::heartbeat::HeartbeatView), lender: &lender, spawner: &spawner, run_cap_s, sandbox: &sandbox, scratch: &scratch };
    let swarm = FileSource::new(swarm_path(Path::new(CONTROL_ROOT)));
    Ok(carry(&organs, &source, &swarm, &config)?.to_string())
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
