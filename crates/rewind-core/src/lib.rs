//! `rewind-core` — portable, deterministic engine for Rewind.
//!
//! This crate **must not** depend on `tauri`, on any OS-specific crate
//! (e.g. `user-idle`), or on the database (`sqlx`). The compiler enforces
//! the engine↔adapter boundary; tests run in pure userspace with fake
//! clocks and fake idle sources.

#![deny(unsafe_code)]

pub mod clock;
pub mod config;
pub mod engine;
pub mod events;
pub mod exercises;
pub mod idle;
pub mod model;
pub mod ports;
pub mod scheduler;
pub mod session;

pub use clock::{Clock, FakeClock, Millis, RealClock, Timestamp};
pub use config::AppConfig;
pub use engine::Engine;
pub use events::{
    BreakPresentation, CoreCommand, CoreEvent, HydrationProgress, Notification, TrayMenuItem,
    TrayMenuItemKind, TrayStatus,
};
pub use idle::{IdleAction, IdleError, IdleReliability, IdleSource};
pub use model::{
    aggregate::DailyAggregate, break_record::BreakOutcome as ModelBreakOutcome,
    break_record::BreakRecord, hydration::HydrationEntry, hydration::HydrationSource,
    session::SessionEndReason, session::SessionRecord,
};
pub use scheduler::{
    reminder::{Priority, Reminder, ReminderKind},
    CoordinatorConfig, HydrationScheduler, HydrationSchedulerConfig, PostureScheduler,
    PostureSchedulerConfig, ReminderCoordinator,
};
pub use session::{BreakKind, PauseReason, SessionMachine, SessionState, Strictness};
