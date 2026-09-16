//! `huginn-daemon`: one process that serves one mind over CultNet.
//!
//! The crate owns four things and no rule. `Daemon::open` owns "may this
//! process serve this mind", by delegating to `Mind::open` and refusing before
//! anything listens. `Daemon::handle` owns "which `Mind` method answers this
//! request": it dispatches, it asks `Mind::require_instance` for every read
//! that names an instance, and it compares nothing itself. `envelope` owns "is
//! this a request at all": a message it cannot decode is answered with a typed
//! failure and touches no mind. `serve` owns "which session gets which reply".
//!
//! Every admission rule, the instance check, the receipt, the derived status
//! and the wire vocabulary are `huginn-mind`'s. The leaf and the store type
//! are reached through `huginn_mind`, so one crate pins one revision of each.

pub mod daemon;
pub mod envelope;
pub mod serve;

pub use daemon::{Daemon, IndexSink, NoIndex};
pub use envelope::{FAILURE_SCHEMA, OperationFailure};
pub use serve::{Options, ServeOptions, parse_options, run, startup};
