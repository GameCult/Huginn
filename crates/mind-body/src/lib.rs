//! `mind-body`: the organs of one Mind's body that live in Huginn's workspace.
//!
//! Today that is `control`, the operator's brakes, and `cli`, the operator's
//! hand on them. The brakes are the operator's consent, not her body: the store
//! is root-owned and no organ of hers writes it.
//!
//! `launch` and `queue` are the launch primitive: the work queue, the
//! repetition breaker, the one-live-Self-run rule, and the single path that
//! opens a run in the mind and only then starts its unit.
//!
//! `heartbeat` runs the swarm's shared heartbeat: the one process, the
//! `mind-heartbeat` binary, that decides which Persona wakes and when, on the
//! configured beat by the pick in `tempo`. It starts her unit and writes no mind.
//!
//! `persona` is the Persona organ: one start of `mind-persona@<instance>` reads the brakes, and
//! until the three-stage turn lands it carries no turn. It keeps what a turn's input is made of
//! (`chat_client` is the chat wire and port and `chat_rudp` its RUDP transport to Bifrost's mouth).
//!
//! `lease_client` asks the Claude connector for a session's credential and the
//! plan's usage windows. `self_run` carries one Self run inside `mind-self@<instance>`: the
//! `mind-self-run` binary runs `claude` over stream-json for her one open Self
//! run, asks for the flush at the context ceiling and stops a child that outlives it.

// The control root is a constant, so the library reads no part of the process
// environment (clippy.toml lists the readers); `forbid` keeps an `allow` from
// putting one back. `forbid(unsafe_code)` closes the readers the list cannot
// name: a foreign `getenv` or any other FFI call needs `unsafe`. The rest of the
// Grant rule is ruling `eureka-body:ruling:mind-launch-forge-scanner-bound`.
#![forbid(clippy::disallowed_methods)]
#![forbid(unsafe_code)]

mod address;
pub mod chat_client;
#[allow(dead_code, reason = "the loader is kept for the three-stage turn's perception (map C3): its first caller removes this allow; `Image` is live through `self_run::session`")]
mod chat_image;
pub mod chat_rudp;
pub mod cli;
pub mod connector;
pub mod control;
pub mod heartbeat;
pub mod launch;
pub mod lease_client;
pub mod persona;
pub mod queue;
#[allow(dead_code, reason = "kept for the three-stage turn's Select stage (map C3 reuses recall.rs): its first caller removes this allow")]
mod recall;
pub mod self_run;
pub mod tempo;

#[cfg(test)]
mod testkit;
