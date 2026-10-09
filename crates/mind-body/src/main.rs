//! `mind-control`: the operator's hand on a Mind's brake and burn-rate dial.
//! All behaviour is `mind_body::cli`; this reads the process's environment and
//! clock, the two things the library never reads.

use std::path::Path;

fn main() {
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
