//! Session — the break/state-machine half of the engine.
//!
//! See implementation plan §7e. The state machine is the heart of the
//! app (DP-1).

pub mod machine;
pub mod state;

pub use machine::SessionMachine;
pub use state::{BreakKind, PauseReason, SessionState, Strictness};
