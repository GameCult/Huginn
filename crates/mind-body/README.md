# mind-body

The Mind's body: the control store, the launch primitive and the `mind-wake`
one-shot.

## The container lane

`tests/wake.rs` holds one `#[ignore]`d test that runs the real `mind-wake`
binary against the real `huginn-daemon` and a real store at the brake's root,
and asserts that the host a run records is the kernel's hostname. It writes
`/etc/gamecult/minds`, so it runs in a container only (`/.dockerenv`), as root,
with the daemon built beside the binary. A plain `cargo test` skips it, and so
does `cargo mutants`, which therefore reports the three `host()` mutants in
`src/wake.rs` as missed. The lane that kills them:

```sh
cargo build -p huginn-daemon -p mind-body --all-targets \
  && cargo test -p mind-body --test wake -- --include-ignored
```

On Yggdrasil, run it through the verify stopgap in the Rust image
(`ygg-verify.sh <worktree> <rev> rust '<the command above>'`).

Mutation testing of the waker needs the glob form, because `-f src/wake.rs`
finds no mutants in this workspace: `cargo mutants -p mind-body -f
"**/src/wake.rs"`.
