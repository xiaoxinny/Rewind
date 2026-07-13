//! Session — the break/state-machine half of the engine.
//!
//! The state machine is the heart of the app.

pub mod machine;
pub mod state;

pub use machine::SessionMachine;
pub use state::{BreakKind, PauseReason, SessionState, Strictness};
