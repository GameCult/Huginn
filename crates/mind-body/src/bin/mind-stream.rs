//! `mind-stream --instance <slug> [--once] [--dry-run]`: her stream of thought.
//!
//! Without `--once` it runs forever: one `mind_body::stream::tick` per `STREAM_POLL_S`, or per
//! `STREAM_MIN_GAP_S` after a blank or a fault. `--dry-run` runs one tick that admits nothing and
//! prints the thought it would admit as JSON. A model or mind fault ends the tick, never the
//! process: it is one line on stderr. This reads the process's arguments and environment, which
//! the library never does.
//!
//! Environment: `HUGINN_ENDPOINT` (`rudp://<ip>:<port>`, her own daemon), `EUREKA_INSTANCE`
//! (when `--instance` is not given), `MIND_STREAM_CONNECTOR_ENDPOINT` (`<loopback ip>:<port>`, the
//! connector kit the Projector is asked through), `MIND_STREAM_PROJECTOR_MODEL` (the model id the
//! kit's caller row allows), `MIND_STREAM_ENDPOINT` (`http://<loopback>:<port>`, the model host) and
//! `MIND_STREAM_BASE_MODEL`. The connector key is the file `connector-key` in `CREDENTIALS_DIRECTORY`
//! (systemd's `LoadCredential`), read once; the caller id is the instance.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use eureka_pipeline::Slug;
use eureka_state::HuginnClient;
use mind_body::control::read_effective;
use mind_body::queue::HuginnMind;
use mind_body::stream::{HuginnPersonaSource, KitProjector, Mode, OllamaContinuer, Organs, Outcome, STREAM_MIN_GAP_S, STREAM_POLL_S, tick};

const TIMEOUT: Duration = Duration::from_secs(30);
/// How long the kit may take to answer one Projector turn.
const PROJECTOR_TIMEOUT: Duration = Duration::from_secs(300);

#[allow(clippy::disallowed_methods, reason = "the binary reads its own environment, and hands it to the library")]
fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The connector key: the credential file, read once and trimmed. An absence names the variable, never a value.
#[allow(clippy::disallowed_methods, reason = "the binary reads its own environment, and hands it to the library")]
fn connector_key() -> Result<String> {
    let directory = std::env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from).ok_or_else(|| anyhow!("CREDENTIALS_DIRECTORY is not set"))?;
    let key = std::fs::read_to_string(directory.join("connector-key")).map_err(|_| anyhow!("the connector-key credential could not be read"))?;
    Ok(key.trim().to_owned())
}

fn required(name: &str) -> Result<String> {
    setting(name).ok_or_else(|| anyhow!("{name} is not set"))
}

struct Args {
    instance: Option<String>,
    once: bool,
    dry_run: bool,
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments")]
fn arguments() -> Result<Args> {
    let mut args = Args { instance: None, once: false, dry_run: false };
    let mut given = std::env::args().skip(1);
    while let Some(argument) = given.next() {
        match argument.as_str() {
            "--instance" => args.instance = Some(given.next().ok_or_else(|| anyhow!("--instance needs a slug"))?),
            "--once" => args.once = true,
            "--dry-run" => args.dry_run = true,
            other => bail!("unknown argument {other}; usage: mind-stream --instance <slug> [--once] [--dry-run]"),
        }
    }
    Ok(args)
}

fn run() -> Result<()> {
    let args = arguments()?;
    let instance = Slug(args.instance.map_or_else(|| required("EUREKA_INSTANCE"), Ok)?);
    let address: SocketAddr = required("HUGINN_ENDPOINT")?
        .strip_prefix("rudp://")
        .and_then(|address| address.parse().ok())
        .ok_or_else(|| anyhow!("HUGINN_ENDPOINT is not rudp://<ip>:<port>"))?;
    let projector_model = required("MIND_STREAM_PROJECTOR_MODEL")?;
    let base_model = required("MIND_STREAM_BASE_MODEL")?;
    let connector: SocketAddr = required("MIND_STREAM_CONNECTOR_ENDPOINT")?.parse().map_err(|_| anyhow!("MIND_STREAM_CONNECTOR_ENDPOINT is not <ip>:<port>"))?;
    let projector = KitProjector::open(connector, connector_key()?, &instance.0, &projector_model, PROJECTOR_TIMEOUT)?;
    let continuer = OllamaContinuer::open(&required("MIND_STREAM_ENDPOINT")?, &base_model)?;
    let mind = HuginnMind::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let persona = HuginnPersonaSource::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let organs = Organs { mind: &mind, persona: &persona, projector: &projector, continuer: &continuer, projector_model: &projector_model, base_model: &base_model };
    let mode = if args.dry_run { Mode::DryRun } else { Mode::Live };
    let once = args.once || args.dry_run;
    loop {
        let mut faulted = false;
        let waits = match tick(&organs, &|| read_effective(&instance), mode, Utc::now()) {
            Ok(Outcome::Thought { thought, admitted }) => {
                if admitted {
                    println!("admitted {}:thought:{}", instance.0, thought.label.0);
                } else {
                    println!("{}", serde_json::to_string_pretty(&*thought)?);
                }
                false
            }
            Ok(Outcome::Blank) => true,
            Ok(Outcome::Held(reason)) => {
                eprintln!("mind-stream: held ({reason:?}); nothing ticked");
                false
            }
            Ok(Outcome::NotDue) => false,
            Err(fault) => {
                eprintln!("mind-stream: {}", format!("{fault:#}").replace('\n', " "));
                faulted = true;
                true
            }
        };
        if once {
            return if faulted { Err(anyhow!("the tick faulted")) } else { Ok(()) };
        }
        std::thread::sleep(Duration::from_secs(if waits { STREAM_MIN_GAP_S as u64 } else { STREAM_POLL_S as u64 }));
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("mind-stream: {}", format!("{fault:#}").replace('\n', " "));
            ExitCode::FAILURE
        }
    }
}
