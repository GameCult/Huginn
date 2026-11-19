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

use std::path::Path;

use anyhow::Result;
use mind_body::cli::instance_and_endpoint;
use mind_body::control::{CONTROL_ROOT, FileSource, control_path, swarm_path};
use mind_body::persona::{Config, carry};

const USAGE: &str = "usage: mind-persona --instance <slug> [--endpoint rudp://<ip>:<port>] [--rooms a,b] [--operator id[,id]] [--minds id[,id]] [--self-sender id]";

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

fn main() {
    match run() {
        Ok(outcome) => println!("{outcome}"),
        Err(error) => {
            eprintln!("mind-persona: {error:#}");
            std::process::exit(2);
        }
    }
}
