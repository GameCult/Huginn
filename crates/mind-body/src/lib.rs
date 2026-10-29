//! `mind-body`: the organs of one Mind's body that live in Huginn's workspace.
//!
//! Today that is `control`, the operator's brake and burn-rate dial, and `cli`,
//! the operator's hand on it. The brake and the dial are the operator's consent
//! (ruling `eureka-body:ruling:brake-dial-root-store-now`), not her body: the
//! store is root-owned and no organ of hers writes it.
//!
//! `launch` and `queue` are the launch primitive: the work queue, the
//! repetition breaker, the one-live-Self-run rule, and the single path that
//! opens a run in the mind and only then starts its unit.
//!
//! `wake` is the waker's one cycle over them: the `mind-wake` binary.
//!
//! `persona` is the Persona organ: one start of `mind-persona@<instance>` drains her
//! pending chat through the connector, one typed reply per turn (`chat_client` is
//! the chat wire and port, `chat_rudp` its RUDP transport to Bifrost's mouth, and
//! `connector` the model transport over ModelConnector's kit client).
//!
//! `lease_client` asks the Claude connector for a Self run's credential and settles
//! what the run used. `self_run` carries one Self run inside `mind-self@<instance>`: the
//! `mind-self-run` binary runs `claude` over stream-json for her one open Self
//! run, asks for the flush at the context ceiling and stops a child that outlives it.

// The control root is a constant, so the library reads no part of the process
// environment (clippy.toml lists the readers); `forbid` keeps an `allow` from
// putting one back. `forbid(unsafe_code)` closes the readers the list cannot
// name: a foreign `getenv` or any other FFI call needs `unsafe`. The rest of the
// Grant rule is ruling `eureka-body:ruling:mind-launch-forge-scanner-bound`.
#![forbid(clippy::disallowed_methods)]
#![forbid(unsafe_code)]

pub mod chat_client;
pub mod chat_rudp;
pub mod cli;
pub mod connector;
pub mod control;
pub mod launch;
pub mod lease_client;
pub mod persona;
pub mod queue;
pub mod self_run;
pub mod wake;

#[cfg(test)]
mod testkit;
