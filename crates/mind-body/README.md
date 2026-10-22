# mind-body

The Mind's body: the control store, the launch primitive, the `mind-wake`
one-shot and the `mind-self-run` actuator.

`mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]` runs inside
`mind-self@<instance>`. It finds her one open Self run, starts `claude` over
stream-json with a fixed argument list, asks for the flush once at the context
ceiling and stops a child that outlives the flush (SIGINT, then SIGTERM and SIGKILL). It exits 0 when the child exited 0, 1
otherwise, 3 when she has no single open Self run and 2 when the arguments are
wrong or the mind cannot be reached.

## The container lane

`tests/wake.rs` holds one `#[ignore]`d test that runs the real `mind-wake`
binary against the real `huginn-daemon` and a real store at the brake's root,
and asserts that the host a run records is the kernel's hostname. It writes
`/etc/gamecult/minds`, so it runs in a container only (`/.dockerenv`), as root,
with the daemon built beside the binary. A plain `cargo test` skips it, and so
does `cargo mutants`, which therefore reports the three `host()` mutants in
`src/bin/mind-wake.rs` as missed. The lane that kills them:

```sh
cargo build -p huginn-daemon -p mind-body --all-targets \
  && cargo test -p mind-body --test wake -- --include-ignored
```

On Yggdrasil, run it through the verify stopgap in the Rust image
(`ygg-verify.sh <worktree> <rev> rust '<the command above>'`).

Mutation testing of the waker needs the glob form, because `-f src/wake.rs`
finds no mutants in this workspace: `cargo mutants -p mind-body -f
"**/src/wake.rs" -f "**/src/bin/mind-wake.rs"`. The second glob reaches
`host()`, whose mutants only the container lane kills.

The actuator has its own ignored test, `the_real_bin_finds_her_run_...` in
`tests/self_run.rs`: the real `mind-self-run` against the real daemon with a
fake `claude` first on `PATH`. It needs the daemon built beside the binary and a
POSIX shell, not root:

```sh
cargo build -p huginn-daemon -p mind-body --all-targets   && cargo test -p mind-body --test self_run -- --include-ignored
```

Mutation testing of the actuator, with the globs that reach the library and the
binary: `cargo mutants -p mind-body -f "**/src/self_run.rs" -f
"**/src/bin/mind-self-run.rs"`. The binary's mutants are killed only by the
lane above.
