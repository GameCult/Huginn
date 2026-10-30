# mind-body

The Mind's body: the control store, the launch primitive, the `mind-heartbeat`
timeline and the `mind-self-run` actuator.

`mind-self-run --instance <slug> [--endpoint rudp://<ip>:<port>]` runs inside
`mind-self@<instance>`. It finds her one open Self run, starts `claude` over
stream-json with a fixed argument list, asks for the flush once at the context
ceiling and stops a child that outlives the flush (SIGINT, then SIGTERM and SIGKILL). It exits 0 when the child exited 0, 1
otherwise, 3 when she has no single open Self run and 2 when the arguments are
wrong or the mind cannot be reached.

## The heartbeat

`mind-heartbeat` is the one process that decides who wakes and when: acting burns initiative by spend, the classifier's urgency multiplies how fast it regains, and the operator tagging or naming a Persona fills it. `mind-heartbeat timeline` prints its latest record.

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
