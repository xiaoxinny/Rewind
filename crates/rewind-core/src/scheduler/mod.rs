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

pub use coordinator::{CoordinatorConfig, ReminderCoordinator};
pub use hydration::{HydrationScheduler, HydrationSchedulerConfig};
pub use posture::{PostureScheduler, PostureSchedulerConfig};
pub use reminder::{Priority, Reminder, ReminderKind};

// Modules remain independent and pure; the engine composes their
// candidates through `ReminderCoordinator` on each tick.
