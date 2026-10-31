//! `mind-heartbeat [timeline]`: the swarm's shared heartbeat.
//!
//! Without an argument it runs forever: one `mind_body::heartbeat::poll` every
//! `HEARTBEAT_POLL_S` seconds (default 20), printing each outcome; a poll that
//! cannot reach its mind, mouth or store prints the error and the loop goes on.
//! It refuses to start on a `heartbeat.cc` that holds anything it does not read
//! (an old dollar initiative store), naming the file; nothing migrates it.
//! `timeline` prints the latest trace record and exits. This reads the process's
//! arguments, environment and credential files, the things the library never
//! reads. Key files are read here and never printed.
//!
//! Environment: `HEARTBEAT_PERSONAS` (`slug,handle,Public Name,@matrix:id,rudp://<ip>:<port>`
//! entries separated by `;`, the last field her own Huginn daemon: each Mind is
//! its own daemon), `MIND_PERSONA_OPERATOR` (id[,id]), `HEARTBEAT_POLL_S`,
//! `BIFROST_CHAT_ENDPOINT` and `BIFROST_CHAT_KEY_FILE`, and `STATE_DIRECTORY`
//! (`heartbeat.cc` lives there). The heartbeat holds no connector key.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use eureka_state::HuginnClient;
use mind_body::chat_rudp::{RudpChat, connection_id};
use mind_body::heartbeat::{Config, FileControl, FileStore, Organs, PersonaId, Seat, TempoView, poll};
use mind_body::launch::{SystemClock, SystemdLauncher};
use mind_body::queue::HuginnMind;

const TIMEOUT: Duration = Duration::from_secs(15);
const CHAT_TIMEOUT: Duration = Duration::from_secs(20);

fn host() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|name| name.trim().to_string()).ok().filter(|name| !name.is_empty()).unwrap_or_else(|| "unknown".into())
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own environment, and hands it to the library")]
fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn required(name: &str) -> Result<String> {
    setting(name).ok_or_else(|| anyhow!("{name} is not set"))
}

fn credential(name: &str) -> Result<String> {
    std::fs::read_to_string(required(name)?).with_context(|| format!("the file named by {name} could not be read"))
}

fn rudp(name: &str) -> Result<SocketAddr> {
    required(name)?.strip_prefix("rudp://").and_then(|address| address.parse().ok()).ok_or_else(|| anyhow!("{name} is not rudp://<ip>:<port>"))
}

fn whole(name: &str, default: i64) -> Result<i64> {
    setting(name).map_or(Ok(default), |value| value.parse().map_err(|_| anyhow!("{name} is not a whole number")))
}

fn config() -> Result<Config> {
    let personas = required("HEARTBEAT_PERSONAS")?
        .split(';')
        .filter(|spec| !spec.trim().is_empty())
        .map(PersonaId::parse)
        .collect::<Result<Vec<_>>>()?;
    let operators = required("MIND_PERSONA_OPERATOR")?.split(',').map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).collect();
    Config::new(personas, operators)
}

fn store() -> Result<FileStore> {
    let Some(state) = setting("STATE_DIRECTORY") else { bail!("no STATE_DIRECTORY") };
    Ok(FileStore::new(Path::new(&state).join("heartbeat.cc")))
}

fn timeline() -> Result<()> {
    match store()?.latest()? {
        Some(trace) => print!("{trace}"),
        None => println!("no poll has been recorded"),
    }
    Ok(())
}

fn run() -> Result<()> {
    let config = config()?;
    let store = store()?;
    store.check()?;
    let chat_address = rudp("BIFROST_CHAT_ENDPOINT")?;
    let chat_id = connection_id(&credential("BIFROST_CHAT_KEY_FILE")?);
    let poll_s = u64::try_from(whole("HEARTBEAT_POLL_S", 20)?).map_err(|_| anyhow!("HEARTBEAT_POLL_S is negative"))?.max(1);
    let minds: Vec<HuginnMind> = config.personas.iter().map(|who| HuginnMind::new(HuginnClient::new(who.huginn, who.slug.clone(), TIMEOUT))).collect();
    let chats: Vec<RudpChat> = config.personas.iter().map(|who| RudpChat::new(chat_address, chat_id, who.slug.0.clone(), CHAT_TIMEOUT)).collect();
    let seats: Vec<Seat> = config
        .personas
        .iter()
        .zip(minds.iter().zip(chats.iter()))
        .map(|(who, (mind, chat))| Seat { who: who.clone(), mind, chat })
        .collect();
    let launcher = SystemdLauncher::new();
    let host = host();
    let organs = Organs { seats: &seats, control: &FileControl, launcher: &launcher, clock: &SystemClock, host: &host, store: &store };
    loop {
        match poll(&organs, &config) {
            Ok(outcome) => println!("{outcome}"),
            Err(error) => eprintln!("mind-heartbeat: {error:#}"),
        }
        std::thread::sleep(Duration::from_secs(poll_s));
    }
}

#[allow(clippy::disallowed_methods, reason = "the binary reads its own arguments")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => run(),
        [word] if word == "timeline" => timeline(),
        _ => Err(anyhow!("usage: mind-heartbeat [timeline]")),
    };
    if let Err(error) = result {
        eprintln!("mind-heartbeat: {error:#}");
        std::process::exit(2);
    }
}
