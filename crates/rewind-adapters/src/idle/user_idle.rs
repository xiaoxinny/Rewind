//! Windows / macOS / X11 idle source backed by the `user-idle` crate.
//!
//! See implementation plan §4 and §18.

// TODO M2: implement `IdleSource` using the `user-idle` crate. The
// TODO M2:   underlying API returns `Result<Duration, user_idle::Error>`.
// TODO M2:   Map errors to `rewind_core::ports::IdleError`.
