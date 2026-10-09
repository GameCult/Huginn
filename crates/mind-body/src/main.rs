//! `mind-control`: the operator's hand on a Mind's brake and burn-rate dial.
//! All behaviour is `mind_body::cli`; this reads the process's environment and
//! clock, the two things the library never reads, and owns the process umask.

use std::path::Path;

/// CultCache stages the store file with the process umask and offers no mode,
/// so the umask is what keeps a staged file from being group- or world-writable
/// for the instant between its rename and the writer's chmod. The binary owns
/// its process state; the library never touches the umask.
#[cfg(unix)]
fn restrict_umask() {
    // SAFETY: umask only sets this process's file-creation mask.
    unsafe {
        libc::umask(0o077);
    }
}

#[cfg(not(unix))]
fn restrict_umask() {}

fn main() {
    restrict_umask();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let set_by = mind_body::cli::operator_name(std::env::var("SUDO_USER").ok(), std::env::var("USER").ok());
    let result = mind_body::cli::run(
        &args,
        Path::new(mind_body::control::CONTROL_ROOT),
        &set_by,
        chrono::Utc::now(),
        &mut std::io::stdout(),
    );
    if let Err(error) = result {
        eprintln!("mind-control: {error:#}");
        std::process::exit(2);
    }
}
