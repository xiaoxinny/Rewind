//! Idle sub-tree — multiple implementations behind one trait.
//!
//! M0: stub. The three real backends land in M2.

pub mod degraded;
pub mod user_idle;
pub mod wayland;

// TODO M2: add `pub use degraded::DegradedIdleSource;`
// TODO M2: add `pub use user_idle::UserIdleSource;`
// TODO M2: add `pub use wayland::WaylandIdleSource;`
// TODO M2: implement the per-OS `IdleSource` factory and the
// TODO M2:   `IdleReliability` reporting.
