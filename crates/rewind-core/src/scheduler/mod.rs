//! Scheduler sub-tree: reminder types, anti-collision coordinator, and
//! the per-pillar schedulers (hydration, posture).
//!
//! See implementation plan §7g (DP-3), §7h (DP-4). M1 keeps the data
//! types (`Reminder`, `ReminderKind`, `Priority`) and the coordinator
//! stubs the §7e machine needs; the per-pillar schedulers land in M5.

pub mod coordinator;
pub mod hydration;
pub mod posture;
pub mod reminder;

pub use coordinator::ReminderCoordinator;
pub use reminder::{Priority, Reminder, ReminderKind};

// M5 will populate hydration and posture schedulers; coordinators
// arrive in M2/M5. Keeping the modules declared so the engine can
// refer to `scheduler::coordinator::ReminderCoordinator` placeholder.
