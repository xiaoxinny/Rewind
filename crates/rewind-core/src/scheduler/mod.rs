//! Scheduler sub-tree: reminder types, anti-collision coordinator, and
//! the per-pillar schedulers (hydration, posture).
//!
//! Holds the data types (`Reminder`, `ReminderKind`, `Priority`) and the
//! coordinator the state machine needs, plus the per-pillar schedulers.

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
