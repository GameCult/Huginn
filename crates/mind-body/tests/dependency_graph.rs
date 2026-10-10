//! The huginn CLI's argument parser is not in mind-body's build. `eureka-state`
//! carries it behind its default `cli` feature; mind-body takes the client
//! core only, so its organs and the `mind-control` binary compile none of it.
//!
//! The graph is read for every target (`--target all`), not the host's: cargo
//! unifies features per target, so a `[target.'cfg(windows)'.dependencies]` line
//! that restores the default features would put the parser into a Windows build
//! and leave a Linux graph clean.

use std::process::Command;

/// The crates `package` pulls in through normal and build edges, on any target,
/// one name per line.
fn dependencies_of(package: &str) -> Vec<String> {
    let output = Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "--locked", "-p", package, "-e", "normal,build", "--target", "all", "--prefix", "none"])
        .output()
        .expect("cargo tree runs");
    assert!(output.status.success(), "cargo tree -p {package} failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| line.split_whitespace().next()).map(str::to_string).collect()
}

#[test]
fn clap_is_not_in_mind_bodys_graph_on_any_target_though_the_cli_crate_has_it() {
    // The control: with its default features the client crate does carry clap, so
    // an empty answer for mind-body is the dependency's doing and not a blind probe.
    assert!(dependencies_of("eureka-state").iter().any(|name| name == "clap"), "eureka-state's default build has clap");
    assert!(!dependencies_of("mind-body").iter().any(|name| name == "clap"), "mind-body compiles the huginn CLI's parser on some target");
}
