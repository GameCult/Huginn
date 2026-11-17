//! The source index's refresh job: one run brings the forge corpus and, when
//! configured, the rooms the chat reader has joined (minus the estate and DM
//! exclusion) to current state, journals one line per repo and room, and exits
//! nonzero if either failed or the collection could not be used. The
//! `chat-purge` subcommand writes a steward's purge tombstone and nothing else.
//!
//! Exit status: 0 clean, 1 a run failed, 2 bad arguments, 3 another run holds
//! the lock. The Matrix token is read from a file (a systemd credential),
//! never from the environment or argv.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use huginn_daemon::index::ollama::pool;
use huginn_daemon::index::qdrant::QdrantIndex;
use huginn_daemon::sources::forge::HttpForge;
use huginn_daemon::sources::job::{self, Chat, Purge};
use huginn_daemon::sources::matrix::HttpMatrix;
use huginn_daemon::sources::room_scope::{ESTATE_ALIASES, Estate};

/// Where the forge answers when the flag is absent: the loopback address of the
/// forge on the host that runs this job.
const DEFAULT_FORGE_URL: &str = "http://127.0.0.1:3310";

/// The chat flags, which come all together or not at all.
struct ChatOptions {
    matrix_url: String,
    token: String,
    estate: Estate,
}

struct Options {
    state_root: PathBuf,
    qdrant_url: String,
    /// The GPU hosts: all take batches at once.
    ollama_urls: Vec<String>,
    /// The CPU host: takes batches only while no GPU host can.
    reserve_urls: Vec<String>,
    embedding_model: String,
    forge_url: String,
    chat: Option<ChatOptions>,
    lock_file: Option<PathBuf>,
}

enum Command {
    Run(Options),
    Purge { qdrant_url: String, lock_file: PathBuf, purge: Purge },
}

/// How a run ends other than by finishing.
enum Stop {
    Usage(anyhow::Error),
    Locked,
}

/// The options named, each once, except those in `repeated`, which keep every value.
fn collect(args: impl Iterator<Item = String>, once: &[&str], repeated: &[&str]) -> Result<(BTreeMap<String, String>, BTreeMap<String, Vec<String>>)> {
    let mut args = args;
    let mut singles = BTreeMap::new();
    let mut many: BTreeMap<String, Vec<String>> = BTreeMap::new();
    while let Some(name) = args.next() {
        let name = name.strip_prefix("--").with_context(|| format!("expected --option, got {name:?}"))?;
        ensure!(once.contains(&name) || repeated.contains(&name), "unsupported huginn-sources option --{name}");
        let value = args.next().with_context(|| format!("missing value for --{name}"))?;
        if repeated.contains(&name) {
            many.entry(name.to_owned()).or_default().push(value);
        } else {
            ensure!(singles.insert(name.to_owned(), value).is_none(), "duplicate option --{name}");
        }
    }
    Ok((singles, many))
}

fn http(name: &str, value: String) -> Result<String> {
    ensure!(value.starts_with("http://"), "--{name} must be a plain http:// address, got {value:?}");
    Ok(value)
}

/// The Matrix reader's bearer travels in clear, and this build speaks no TLS: only a loopback host may be named.
fn loopback_http(name: &str, value: String) -> Result<String> {
    let value = http(name, value)?;
    let authority = value["http://".len()..].split(['/', '?', '#']).next().unwrap_or("");
    let host = match authority.strip_prefix('[') {
        // The bracket must end the authority or be followed by the port, so `[::1].evil.example` names no loopback.
        Some(inner) => match inner.split_once(']') {
            Some((host, "")) => host,
            Some((host, port)) if port.starts_with(':') => host,
            _ => "",
        },
        None => authority.split(':').next().unwrap_or(""),
    };
    let loopback = !authority.contains('@') && (host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()));
    ensure!(loopback, "--{name} must name a loopback host (localhost, 127.0.0.0/8 or [::1]): the bearer token travels in clear and this build has no TLS, got {value:?}");
    Ok(value)
}

fn absolute(name: &str, value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    ensure!(path.is_absolute(), "--{name} must be an absolute path");
    Ok(path)
}

fn chat_options(singles: &BTreeMap<String, String>, excluded: Vec<String>) -> Result<Option<ChatOptions>> {
    let given = [singles.contains_key("matrix-url"), singles.contains_key("matrix-token-file"), !excluded.is_empty()];
    if !given.contains(&true) {
        return Ok(None);
    }
    for (present, name) in given.iter().zip(["matrix-url", "matrix-token-file", "chat-exclude-room"]) {
        ensure!(*present, "--{name} is missing: the chat flags (--matrix-url, --matrix-token-file, --chat-exclude-room) come all together or not at all");
    }
    let ids: BTreeSet<String> = excluded.iter().cloned().collect();
    ensure!(ids.len() == excluded.len(), "--chat-exclude-room must not repeat a room id");
    ensure!(ids.len() == ESTATE_ALIASES.len(), "--chat-exclude-room must be given exactly {} times (the ids of {})", ESTATE_ALIASES.len(), ESTATE_ALIASES.join(", "));
    ensure!(ids.iter().all(|id| id.starts_with('!')), "--chat-exclude-room takes room ids beginning with !, not aliases");
    let path = absolute("matrix-token-file", &singles["matrix-token-file"])?;
    let token = std::fs::read_to_string(&path).with_context(|| format!("reading --matrix-token-file {}", path.display()))?.trim().to_owned();
    ensure!(!token.is_empty(), "--matrix-token-file {} is empty", path.display());
    Ok(Some(ChatOptions { matrix_url: loopback_http("matrix-url", singles["matrix-url"].clone())?, token, estate: Estate::new(ids)? }))
}

fn parse(mut args: impl Iterator<Item = String>, now: DateTime<Utc>) -> Result<Command> {
    let mut rest: Vec<String> = Vec::new();
    let first = args.next();
    if first.as_deref() == Some("chat-purge") {
        let (singles, _) = collect(args, &["qdrant-url", "room", "from", "to", "request", "lock-file"], &[])?;
        let take = |name: &str| singles.get(name).cloned().with_context(|| format!("--{name} is required"));
        return Ok(Command::Purge {
            qdrant_url: http("qdrant-url", take("qdrant-url")?)?,
            lock_file: absolute("lock-file", &take("lock-file")?)?,
            purge: Purge::check(&take("room")?, &take("from")?, &take("to")?, &take("request")?, now)?,
        });
    }
    rest.extend(first);
    let (mut singles, mut many) = collect(
        rest.into_iter().chain(args),
        &["state-root", "qdrant-url", "embedding-model", "forge-url", "matrix-url", "matrix-token-file", "lock-file"],
        &["ollama-url", "reserve-ollama-url", "chat-exclude-room"],
    )?;
    let mut hosts = |name: &str| -> Result<Vec<String>> { many.remove(name).unwrap_or_default().into_iter().map(|url| http(name, url)).collect() };
    let ollama_urls = hosts("ollama-url")?;
    let reserve_urls = hosts("reserve-ollama-url")?;
    let excluded = many.remove("chat-exclude-room").unwrap_or_default();
    let take = |singles: &mut BTreeMap<String, String>, name: &str| singles.remove(name).with_context(|| format!("--{name} is required"));
    let state_root = absolute("state-root", &take(&mut singles, "state-root")?)?;
    let embedding_model = take(&mut singles, "embedding-model")?;
    ensure!(!embedding_model.is_empty(), "--embedding-model must name a model");
    ensure!(!ollama_urls.is_empty(), "at least one --ollama-url is required; repeat it for each GPU host");
    let chat = chat_options(&singles, excluded)?;
    let lock_file = singles.get("lock-file").map(|path| absolute("lock-file", path)).transpose()?;
    ensure!(chat.is_none() || lock_file.is_some(), "--lock-file is required whenever the chat flags are given");
    Ok(Command::Run(Options {
        state_root,
        qdrant_url: http("qdrant-url", take(&mut singles, "qdrant-url")?)?,
        ollama_urls,
        reserve_urls,
        embedding_model,
        forge_url: http("forge-url", singles.remove("forge-url").unwrap_or_else(|| DEFAULT_FORGE_URL.to_owned()))?,
        chat,
        lock_file,
    }))
}

/// Takes the job's lock for the life of the returned file. A held lock means a run (or a
/// purge request) is in progress and nothing may change: the cursors object is replaced whole.
fn take_lock(path: &Path) -> Result<File, Stop> {
    let file = OpenOptions::new().create(true).truncate(false).write(true).open(path).with_context(|| format!("opening --lock-file {}", path.display())).map_err(Stop::Usage)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(Stop::Locked),
        Err(TryLockError::Error(error)) => Err(Stop::Usage(anyhow::Error::new(error).context(format!("locking --lock-file {}", path.display())))),
    }
}

fn lock_if(path: &Option<PathBuf>) -> Result<Option<File>, Stop> {
    path.as_deref().map(take_lock).transpose()
}

fn execute(command: Command) -> Result<u8, Stop> {
    match command {
        Command::Purge { qdrant_url, lock_file, purge } => {
            let _lock = take_lock(&lock_file)?;
            let mut index = QdrantIndex::new(&qdrant_url);
            Ok(match job::purge(&mut index, &purge) {
                Ok(line) => {
                    println!("{line}");
                    0
                }
                Err(error) => {
                    eprintln!("huginn-sources: {error:#}");
                    1
                }
            })
        }
        Command::Run(options) => {
            let _lock = lock_if(&options.lock_file)?;
            let mut embedder = pool(&options.ollama_urls, &options.reserve_urls, &options.embedding_model);
            let mut index = QdrantIndex::new(&options.qdrant_url);
            let mut forge = HttpForge::new(&options.forge_url);
            let mut matrix = options.chat.as_ref().map(|chat| HttpMatrix::new(&chat.matrix_url, &chat.token));
            let chat = matrix.as_mut().zip(options.chat.as_ref()).map(|(history, chat)| Chat { history, estate: &chat.estate });
            Ok(job::run(&mut embedder, &mut index, &mut forge, &options.state_root, chat, &mut |line| println!("{line}"), &mut |line| eprintln!("{line}")))
        }
    }
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1), Utc::now()).map_err(Stop::Usage).and_then(execute) {
        Ok(status) => ExitCode::from(status),
        Err(Stop::Usage(error)) => {
            eprintln!("huginn-sources: {error:#}");
            ExitCode::from(2)
        }
        Err(Stop::Locked) => {
            eprintln!("huginn-sources: huginn-sources is running");
            ExitCode::from(3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-17T00:00:00Z";

    fn args(text: &str) -> impl Iterator<Item = String> {
        text.split_whitespace().map(str::to_owned).collect::<Vec<_>>().into_iter()
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&Utc)
    }

    fn error(text: &str) -> String {
        match parse(args(text), now()) {
            Ok(_) => panic!("parsed: {text}"),
            Err(error) => format!("{error:#}"),
        }
    }

    const BASE: &str = "--state-root /s --qdrant-url http://q --ollama-url http://o --embedding-model m";
    const IDS: &str = "--chat-exclude-room !a:s --chat-exclude-room !b:s --chat-exclude-room !c:s";

    fn with_token(secret: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, secret).unwrap();
        let path = path.display().to_string();
        (dir, path)
    }

    #[test]
    fn chat_flags_come_all_or_none() {
        let (_dir, token) = with_token("  sekret-token\n");
        let lock = "--lock-file /l";
        let full = format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} {IDS} {lock}");
        let Ok(Command::Run(options)) = parse(args(&full), now()) else { panic!("full chat flags must parse") };
        assert_eq!(options.chat.as_ref().unwrap().token, "sekret-token", "trimmed");
        assert!(matches!(parse(args(BASE), now()), Ok(Command::Run(Options { chat: None, lock_file: None, .. }))));
        assert!(error(&format!("{BASE} --matrix-url http://localhost {lock}")).contains("--matrix-token-file is missing"));
        assert!(error(&format!("{BASE} --matrix-token-file {token} {IDS} {lock}")).contains("--matrix-url is missing"));
        assert!(error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} {lock}")).contains("--chat-exclude-room is missing"));
        assert!(error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} {IDS}")).contains("--lock-file is required"));
    }

    #[test]
    fn chat_flag_values_are_checked_and_never_print_the_token() {
        let (_dir, token) = with_token("sekret-token");
        let on = |extra: &str| format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} --lock-file /l {extra}");
        assert!(error(&on("--chat-exclude-room !a:s --chat-exclude-room !b:s")).contains("exactly 3"));
        assert!(error(&on(&format!("{IDS} --chat-exclude-room !d:s"))).contains("exactly 3"));
        assert!(error(&on("--chat-exclude-room !a:s --chat-exclude-room !a:s --chat-exclude-room !b:s")).contains("repeat"));
        assert!(error(&on("--chat-exclude-room !a:s --chat-exclude-room !b:s --chat-exclude-room #c:s")).contains("beginning with !"));
        assert!(error(&format!("{BASE} --matrix-url https://m --matrix-token-file {token} --lock-file /l {IDS}")).contains("plain http://"));
        assert!(error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file rel --lock-file /l {IDS}")).contains("absolute"));
        assert!(error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} --lock-file rel {IDS}")).contains("absolute"));
        let (_empty, empty) = with_token(" \n");
        let message = error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file {empty} --lock-file /l {IDS}"));
        assert!(message.contains(&empty) && message.contains("empty"));
        let missing = error(&format!("{BASE} --matrix-url http://localhost --matrix-token-file /nowhere/t --lock-file /l {IDS}"));
        assert!(missing.contains("/nowhere/t"));
        for text in [on(IDS), on("--chat-exclude-room !a:s")] {
            assert!(!error(&format!("{text} --nope x")).contains("sekret-token"));
        }
        let dead = ["--chat", "room"].join("-");
        assert!(error(&format!("{BASE} {dead} !x:s")).contains(&format!("unsupported huginn-sources option {dead}")));
    }

    fn purge(extra: &str) -> String {
        error(&format!("chat-purge --qdrant-url http://q --lock-file /l {extra}"))
    }

    #[test]
    fn chat_purge_arguments() {
        let good = "--room !r:s --from 2026-11-10T00:00:00Z --to 2026-11-11T00:00:00Z --request ref-1";
        let Ok(Command::Purge { purge: asked, .. }) = parse(args(&format!("chat-purge --qdrant-url http://q --lock-file /l {good}")), now()) else { panic!() };
        assert_eq!((asked.window.from_ms, asked.window.to_ms), (1_794_268_800_000, 1_794_355_200_000));
        let bad = |from: &str, to: &str, room: &str, request: &str| purge(&format!("--room {room} --from {from} --to {to} --request {request}"));
        let (f, t) = ("2026-11-10T00:00:00Z", "2026-11-11T00:00:00Z");
        assert!(bad(f, t, "#a:s", "r").contains("--room"));
        assert!(bad("2026-11-10T00:00:00+00:00", t, "!r:s", "r").contains("--from"));
        assert!(bad(f, "2026-11-11T00:00:00", "!r:s", "r").contains("--to"));
        assert!(bad(t, f, "!r:s", "r").contains("before"));
        assert!(bad(f, "2026-11-18T00:00:00Z", "!r:s", "r").contains("after now"));
        assert!(bad("2025-11-09T00:00:00Z", t, "!r:s", "r").contains("366"));
        assert!(purge(&format!("--room !r:s --from {f} --to {t} --request {}", "x".repeat(121))).contains("--request"));
        assert!(purge("--room !r:s --from 2026-11-10T00:00:00Z --to 2026-11-11T00:00:00Z").contains("--request is required"));
        assert!(parse(args(&format!("chat-purge --qdrant-url http://q --lock-file /l {good} --matrix-url http://localhost")), now()).is_err(), "no Matrix flags");
    }

    #[test]
    fn matrix_url_must_be_loopback_http() {
        let (_dir, token) = with_token("sekret-token");
        let with = |url: &str| format!("{BASE} --matrix-url {url} --matrix-token-file {token} --lock-file /l {IDS}");
        for good in ["http://localhost", "http://127.0.0.1:8008", "http://127.9.9.9/", "http://[::1]:8008", "http://LOCALHOST:1/x"] {
            assert!(parse(args(&with(good)), now()).is_ok(), "{good}");
        }
        for bad in ["http://matrix.example", "http://10.0.0.5:8008", "http://localhost@evil.example", "http://127.0.0.1.evil.example", "http://localhost.evil.example", "http://[::2]", "https://localhost", "http://127.0.0.1:pw@evil.example", "http://[::1].evil.example", "http://[::1]x"] {
            assert!(error(&with(bad)).contains("--matrix-url"), "{bad}");
        }
    }

    #[test]
    fn a_held_lock_exits_3_through_execute_for_both_modes() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("lock");
        let held = take_lock(&lock).ok().unwrap();
        let lock = lock.display().to_string();
        let good = "--room !r:s --from 2026-11-10T00:00:00Z --to 2026-11-11T00:00:00Z --request r";
        let purge = parse(args(&format!("chat-purge --qdrant-url http://127.0.0.1:1 --lock-file {lock} {good}")), now()).unwrap();
        assert!(matches!(execute(purge), Err(Stop::Locked)));
        let (_t, token) = with_token("sekret-token");
        let run = parse(args(&format!("{BASE} --matrix-url http://localhost --matrix-token-file {token} {IDS} --lock-file {lock}")), now()).unwrap();
        assert!(matches!(execute(run), Err(Stop::Locked)));
        drop(held);
    }

    #[test]
    fn a_held_lock_stops_the_run_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let held = take_lock(&path).ok().unwrap();
        assert!(matches!(take_lock(&path), Err(Stop::Locked)));
        drop(held);
        assert!(take_lock(&path).is_ok());
    }
}
