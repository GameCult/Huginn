//! `mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]`: carries
//! her one open Self run (`mind_body::self_run::carry`) inside
//! `mind-self@<instance>`. This reads the process's arguments and environment,
//! the things the library never reads, and prints the request lines and the
//! outcome. It exits 0 when the child exited 0, 1 when it exited otherwise or by
//! a signal, 3 when she has no single open Self run (no child started), and 2
//! when the mind cannot be reached or the arguments are wrong. Without
//! `--endpoint` the daemon is `HUGINN_ENDPOINT`.

use std::time::Duration;

use anyhow::Result;
use eureka_state::HuginnClient;
use mind_body::cli::instance_and_endpoint;
use mind_body::launch::SystemClock;
use mind_body::queue::HuginnMind;
use mind_body::self_run::{ClaudeSpawner, carry};

/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);

const USAGE: &str = "usage: mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]";

// The binary is the one place that reads the process: its arguments and the
// daemon's address. The library reads neither (clippy.toml, `#![forbid]` in lib.rs).
#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and the daemon's address, and hands them to the library")]
fn run() -> Result<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (instance, address) = instance_and_endpoint(&args, std::env::var("HUGINN_ENDPOINT").ok(), USAGE)?;
    let mind = HuginnMind::new(HuginnClient::new(address, instance, TIMEOUT));
    let outcome = carry(&mind, &SystemClock, &ClaudeSpawner, &mut std::io::stdout())?;
    println!("{outcome}");
    Ok(outcome.exit_status())
}

fn main() {
    match run() {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("mind-self-run: {error:#}");
            std::process::exit(2);
        }
    }
}
