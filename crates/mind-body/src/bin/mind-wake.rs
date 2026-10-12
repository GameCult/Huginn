//! `mind-wake --instance <slug> [--endpoint rudp://<ip>:<port>]`: one waker
//! cycle for one Mind. All behaviour is `mind_body::wake::cycle`; this reads the
//! process's arguments, environment and clock, the things the library never
//! reads, and prints the outcome. It exits 0 for every outcome and 2 when the
//! mind cannot be reached or the arguments are wrong. Without `--endpoint` the
//! daemon is `HUGINN_ENDPOINT`.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use eureka_pipeline::Slug;
use eureka_state::HuginnClient;
use mind_body::control::{CONTROL_ROOT, FileSource, control_path};
use mind_body::launch::{Ports, SystemClock, SystemdLauncher};
use mind_body::queue::HuginnMind;
use mind_body::wake::cycle;

/// How long one call to the mind may take.
const TIMEOUT: Duration = Duration::from_secs(15);

const USAGE: &str = "usage: mind-wake --instance <slug> [--endpoint rudp://<ip>:<port>]";

/// The instance and the endpoint as given. A flag given twice or an unknown
/// argument is refused without printing it.
fn parse(args: &[String]) -> Result<(Slug, Option<String>)> {
    let (mut instance, mut endpoint) = (None, None);
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--instance" => &mut instance,
            "--endpoint" => &mut endpoint,
            _ => bail!("{USAGE}"),
        };
        let value = rest.next().ok_or_else(|| anyhow!("{USAGE}"))?;
        if slot.replace(value.clone()).is_some() {
            bail!("{USAGE}");
        }
    }
    Ok((Slug(instance.ok_or_else(|| anyhow!("{USAGE}"))?), endpoint))
}

/// The host a run records: the kernel's name for this machine.
fn host() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|name| name.trim().to_string()).ok().filter(|name| !name.is_empty()).unwrap_or_else(|| "unknown".into())
}

// The binary is the one place that reads the process: its arguments and the
// daemon's address. The library reads neither (clippy.toml, `#![forbid]` in lib.rs).
#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments and the daemon's address, and hands them to the library")]
fn run() -> Result<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (instance, endpoint) = parse(&args)?;
    let source = FileSource::new(control_path(Path::new(CONTROL_ROOT), &instance)?);
    let endpoint = endpoint.or_else(|| std::env::var("HUGINN_ENDPOINT").ok()).ok_or_else(|| anyhow!("no daemon endpoint: --endpoint or HUGINN_ENDPOINT"))?;
    let address = endpoint
        .strip_prefix("rudp://")
        .and_then(|address| address.parse::<SocketAddr>().ok())
        .ok_or_else(|| anyhow!("the endpoint is not rudp://<ip>:<port>"))?;
    let mind = HuginnMind::new(HuginnClient::new(address, instance, TIMEOUT));
    let launcher = SystemdLauncher::new();
    let host = host();
    let ports = Ports { mind: &mind, launcher: &launcher, clock: &SystemClock, host: &host };
    Ok(cycle(&ports, &source)?.to_string())
}

fn main() {
    match run() {
        Ok(outcome) => println!("{outcome}"),
        Err(error) => {
            eprintln!("mind-wake: {error:#}");
            std::process::exit(2);
        }
    }
}
