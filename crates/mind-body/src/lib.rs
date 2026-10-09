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

pub mod cli;
pub mod control;
pub mod launch;
pub mod queue;

#[cfg(test)]
mod testkit;
