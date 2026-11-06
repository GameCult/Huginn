//! `mind-stream --instance <slug> [--once] [--dry-run]`: her stream of thought.
//!
//! Without `--once` it runs forever: one `mind_body::stream::tick` per `POLL_S`, or per
//! `STREAM_MIN_GAP_S` after a blank or a fault. `--dry-run` runs one tick that admits nothing and
//! prints the thought it would admit as JSON. A model or mind fault ends the tick, never the
//! process: it is one line on stderr. This reads the process's arguments and environment, which
//! the library never does.
//!
//! Environment: `HUGINN_ENDPOINT` (`rudp://<ip>:<port>`, her own daemon), `EUREKA_INSTANCE`
//! (when `--instance` is not given), `MIND_STREAM_ENDPOINT` (`http://<loopback>:<port>`, the
//! model host), `MIND_STREAM_PROJECTOR_MODEL` and `MIND_STREAM_BASE_MODEL`.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use eureka_pipeline::Slug;
use eureka_state::HuginnClient;
use mind_body::control::read_effective;
use mind_body::queue::HuginnMind;
use mind_body::stream::{HuginnPersonaSource, Mode, OllamaStream, Organs, Outcome, STREAM_MIN_GAP_S, tick};

const TIMEOUT: Duration = Duration::from_secs(30);
const POLL_S: u64 = 60;

#[allow(clippy::disallowed_methods, reason = "the binary reads its own environment, and hands it to the library")]
fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn required(name: &str) -> Result<String> {
    setting(name).ok_or_else(|| anyhow!("{name} is not set"))
}

struct Args {
    instance: Option<String>,
    once: bool,
    dry_run: bool,
}

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
    let model = OllamaStream::open(&required("MIND_STREAM_ENDPOINT")?, &projector_model, &base_model)?;
    let mind = HuginnMind::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let persona = HuginnPersonaSource::new(HuginnClient::new(address, instance.clone(), TIMEOUT));
    let organs = Organs { mind: &mind, persona: &persona, model: &model, projector_model: &projector_model, base_model: &base_model };
    let mode = if args.dry_run { Mode::DryRun } else { Mode::Live };
    let once = args.once || args.dry_run;
    loop {
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
                true
            }
        };
        if once {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(if waits { STREAM_MIN_GAP_S as u64 } else { POLL_S }));
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
