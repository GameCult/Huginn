//! The `mind-control` command line: the operator's hand on one instance's
//! control store. It checks no caller identity: the file is
//! root-owned and the filesystem is the authority.
//!
//! ```text
//! mind-control [--root <dir>] --instance <slug> show
//! mind-control --instance <slug> brake hold        (also stops her running units)
//! mind-control --instance <slug> brake release
//! mind-control --instance <slug> dial set --heat H --base-cooldown-s S --base-run-usd U
//! mind-control [--root <dir>] --instance <slug> check
//! ```
//!
//! `check` exits 0 only when the brake is released and the dial in bounds; a
//! systemd unit runs it as its `ExecCondition`. It writes nothing.
//!
//! `brake hold` writes the brake held and then, whatever the write's outcome,
//! stops the instance's running units through the `stop_units` parameter. It
//! fails if either step failed. No other command stops anything.

use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use eureka_pipeline::{RunTurn, Slug};
use rust_decimal::Decimal;

use crate::control::{BurnRate, ControlWriter, Effective, FileSource, control_path, derive, load_state};
use crate::launch::unit;

const USAGE: &str =
    "usage: mind-control [--root <dir>] --instance <slug> show | check | brake hold | brake release | dial set --heat H --base-cooldown-s S --base-run-usd U";

/// Who made the change: `SUDO_USER`, else `USER`, else `unknown`. An empty
/// variable counts as unset.
pub fn operator_name(sudo_user: Option<String>, user: Option<String>) -> String {
    [sudo_user, user].into_iter().flatten().find(|name| !name.is_empty()).unwrap_or_else(|| "unknown".to_string())
}

/// Runs one command against `<root>/<instance>/control.cc`. `root` is the
/// default; a leading `--root <dir>` argument replaces it (argv, not environment,
/// because `sudo -E` carries the environment). The operator's name, the clock, the output and the stopper are parameters so a test drives
/// the whole command without touching `/etc` or systemd. `stop_units` is
/// called only by `brake hold`, after the write, whatever the write's outcome.
pub fn run(
    args: &[String],
    root: &Path,
    set_by: &str,
    now: DateTime<Utc>,
    out: &mut impl Write,
    stop_units: &mut dyn FnMut(&Slug) -> Result<()>,
) -> Result<()> {
    let (root, args) = match args {
        [flag, dir, rest @ ..] if flag == "--root" => (Path::new(dir), rest),
        _ => (root, args),
    };
    let (instance, command) = match args {
        [flag, instance, command @ ..] if flag == "--instance" => (Slug(instance.clone()), command),
        _ => bail!("{USAGE}"),
    };
    let path = control_path(root, &instance)?;
    let words: Vec<&str> = command.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["show"] => show(root, &instance, out),
        ["check"] => check(&path, &instance, out),
        ["brake", "hold"] => {
            let written = ControlWriter::open(&path).and_then(|mut writer| writer.set_brake(false, now, set_by));
            let stopped = stop_units(&instance);
            match (written, stopped) {
                (Ok(()), Ok(())) => {
                    writeln!(out, "units: stopped {} {}", unit(&instance, RunTurn::SelfRun)?, unit(&instance, RunTurn::PersonaTurn)?)?;
                    Ok(())
                }
                (Err(write), Ok(())) => Err(write.context("the brake was not written; the units were stopped")),
                (Ok(()), Err(stop)) => Err(stop.context("the brake is held; the units were not stopped")),
                (Err(write), Err(stop)) => Err(anyhow!("the brake was not written ({write:#}) and the units were not stopped ({stop:#})")),
            }
        }
        ["brake", "release"] => ControlWriter::open(&path)?.set_brake(true, now, set_by),
        ["dial", "set", flags @ ..] => {
            let dial = parse_dial(flags, now, set_by)?;
            ControlWriter::open(&path)?.set_dial(dial)
        }
        _ => bail!("{USAGE}"),
    }
}

fn parse_dial(flags: &[&str], now: DateTime<Utc>, set_by: &str) -> Result<BurnRate> {
    let (mut heat, mut cooldown, mut run_usd) = (None, None, None);
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        let value = rest.next().ok_or_else(|| anyhow!("{flag} needs a value"))?;
        let slot = match *flag {
            "--heat" => &mut heat,
            "--base-cooldown-s" => &mut cooldown,
            "--base-run-usd" => &mut run_usd,
            other => bail!("unknown flag {other:?}"),
        };
        if slot.replace(*value).is_some() {
            bail!("{flag} given twice");
        }
    }
    Ok(BurnRate {
        heat: Decimal::from_str(need(heat, "--heat")?).context("--heat is not a decimal")?,
        base_cooldown_s: need(cooldown, "--base-cooldown-s")?.parse().context("--base-cooldown-s is not a whole number of seconds")?,
        base_run_usd: Decimal::from_str(need(run_usd, "--base-run-usd")?).context("--base-run-usd is not a decimal")?,
        set_at: now,
        set_by: set_by.to_string(),
    })
}

fn need<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str> {
    value.ok_or_else(|| anyhow!("{name} is required"))
}

/// The gate a unit runs before it starts: Ok only when the one derivation
/// (`derive`, the reader behind `read_effective`) says Released. It is a pure
/// read through `FileSource`: it opens no writer and creates no file or
/// directory. Any other reading is an error, which `main` turns into exit 2,
/// and systemd's `ExecCondition` skips the unit on 1-254.
fn check(path: &Path, instance: &Slug, out: &mut impl Write) -> Result<()> {
    let effective = derive(&FileSource::new(path), instance);
    writeln!(out, "effective: {effective:?}")?;
    match effective {
        Effective::Released(_) => Ok(()),
        Effective::Held { reason } => bail!("held: {reason:?}"),
    }
}

fn show(root: &Path, instance: &Slug, out: &mut impl Write) -> Result<()> {
    let source = FileSource::new(control_path(root, instance)?);
    writeln!(out, "instance: {}", instance.0)?;
    match load_state(&source) {
        Ok(state) => {
            match &state.brake {
                Some(brake) => writeln!(
                    out,
                    "brake: {} (set_at {}, set_by {})",
                    if brake.released { "released" } else { "held" },
                    brake.set_at.to_rfc3339(),
                    brake.set_by
                )?,
                None => writeln!(out, "brake: absent")?,
            }
            match &state.dial {
                Some(dial) => writeln!(
                    out,
                    "dial: heat {} base_cooldown_s {} base_run_usd {} (set_at {}, set_by {})",
                    dial.heat,
                    dial.base_cooldown_s,
                    dial.base_run_usd,
                    dial.set_at.to_rfc3339(),
                    dial.set_by
                )?,
                None => writeln!(out, "dial: absent")?,
            }
        }
        Err(error) => writeln!(out, "store: undecodable ({error:#})")?,
    }
    writeln!(out, "effective: {:?}", derive(&source, instance))?;
    Ok(())
}
