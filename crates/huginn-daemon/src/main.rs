//! The process: open one instance's mind, bind one socket, serve until a stop
//! signal. A mind that will not open ends the process with the refusal on
//! stderr and no socket ever bound.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use huginn_daemon::serve::{ServeOptions, parse_options, run, startup};

fn main() -> Result<()> {
    let options = parse_options(std::env::args().skip(1))?;
    let (mut daemon, mut hub, registry) = match startup(&options) {
        Ok(started) => started,
        Err(error) => {
            eprintln!("huginn-daemon refuses to serve {}: {error:#}", options.instance.0);
            std::process::exit(1);
        }
    };

    // Idunn runs this process as PID 1 of its own PID namespace, and a
    // namespace init ignores every signal it has not caught. Without this,
    // a stop is a ninety-second wait for SIGKILL.
    let stopping = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&stopping))
            .context("registering Huginn's stop signal handler")?;
    }

    eprintln!("huginn-daemon serving {} on {}", options.instance.0, hub.local_addr()?);
    run(&mut daemon, &mut hub, &registry, &stopping, &ServeOptions::default())
}
