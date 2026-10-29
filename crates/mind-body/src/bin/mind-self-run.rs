//! `mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]`: carries
//! her one open Self run (`mind_body::self_run::carry`) inside
//! `mind-self@<instance>`. This reads the process's arguments and environment,
//! the things the library never reads, and prints the request lines and the
//! outcome. It exits 0 when the child exited 0, 1 when it exited otherwise or by
//! a signal, 3 when she has no single open Self run (no child started), 4 when
//! the connector refused the run's credential lease (the run is closed, no child
//! started), and 2 when the mind or the connector cannot be reached or the
//! arguments are wrong. Without `--endpoint` the daemon is `HUGINN_ENDPOINT`.
//! `MIND_SELF_RUN_CAP_S` is the unit's `RuntimeMaxSec` in seconds, the one length of a run.
//! `RUNTIME_DIRECTORY` (the unit's `RuntimeDirectory=`) holds each child's fresh
//! configuration; `MIND_SELF_CONFIG_SEED` optionally names a root-owned directory copied into it.
//! The connector's lease socket is `CLAUDE_LEASE_SOCKET`: the token itself is
//! never in this process's environment, only the descriptor the lease grants.

use std::time::Duration;

use anyhow::{Result, anyhow};
use eureka_state::HuginnClient;
use mind_body::cli::instance_and_endpoint;
use mind_body::launch::SystemClock;
use mind_body::lease_client::{LEASE_SOCKET_VARIABLE, SocketLender};
use mind_body::queue::HuginnMind;
use mind_body::self_run::{ClaudeSpawner, RUN_CAP_VARIABLE, carry};

/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);

const USAGE: &str = "usage: mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]";

// The binary is the one place that reads the process: its arguments and the
// daemon's address. The library reads neither (clippy.toml, `#![forbid]` in lib.rs).
#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and environment, and hands them to the library")]
fn run() -> Result<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (instance, address) = instance_and_endpoint(&args, std::env::var("HUGINN_ENDPOINT").ok(), USAGE)?;
    let socket = std::env::var_os(LEASE_SOCKET_VARIABLE).ok_or_else(|| anyhow!("{LEASE_SOCKET_VARIABLE} is not set"))?;
    let run_cap_s: u64 = std::env::var(RUN_CAP_VARIABLE)
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| anyhow!("{RUN_CAP_VARIABLE} is not set to the unit's RuntimeMaxSec in seconds"))?;
    let mind = HuginnMind::new(HuginnClient::new(address, instance, TIMEOUT));
    let outcome = carry(&mind, &SystemClock, &SocketLender::new(socket), &ClaudeSpawner::new(std::env::vars_os())?, run_cap_s, &mut std::io::stdout())?;
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
