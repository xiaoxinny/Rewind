//! Idle sub-tree: pause/reset policy, idle-source seams.
//!
//! See implementation plan §7f for the pause/reset policy (DP-2).
//! The `IdleSource` trait itself lives in `crate::ports` — re-exported
//! here for ergonomic imports.

pub mod policy;

// Re-export the adapter trait so callers can do `use rewind_core::idle::*`.
pub use crate::ports::{IdleError, IdleReliability, IdleSource};

pub use policy::{evaluate, natural_break_satisfied, IdleAction};
