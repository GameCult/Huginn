//! The huginn CLI's argument parser is not in mind-body's build. `eureka-state`
//! carries it behind its default `cli` feature; mind-body takes the client
//! core only, so its organs and the `mind-control` binary compile none of it.

use std::process::Command;

/// The crates `package` pulls in through normal and build edges, one name per line.
fn dependencies_of(package: &str) -> Vec<String> {
    let cargo = std::env::var_os("CARGO").expect("cargo sets CARGO for its tests");
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["tree", "--offline", "--locked", "-p", package, "-e", "normal,build", "--prefix", "none"])
        .output()
        .expect("cargo tree runs");
    assert!(output.status.success(), "cargo tree -p {package} failed");
    String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| line.split_whitespace().next()).map(str::to_string).collect()
}

#[test]
fn clap_is_not_in_mind_bodys_graph_though_the_cli_crate_has_it() {
    // The control: with its default features the client crate does carry clap, so
    // an empty answer for mind-body is the dependency's doing and not a blind probe.
    assert!(dependencies_of("eureka-state").iter().any(|name| name == "clap"), "eureka-state's default build has clap");
    assert!(!dependencies_of("mind-body").iter().any(|name| name == "clap"), "mind-body compiles the huginn CLI's parser");
}
