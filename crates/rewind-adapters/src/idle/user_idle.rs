//! Windows / macOS / Linux (X11) idle source backed by the
//! `user-idle` crate (XScreenSaverQueryInfo on X11,
//! `GetLastInputInfo` on Windows, CoreGraphics on macOS).
//!
//! See implementation plan §4 ("Rust crates — `user-idle`"),
//! §7f (`idle/policy.rs`), and §18 (Wayland caveat — Wayland is
//! **not** handled here; the picker routes Wayland sessions to
//! `WaylandIdleSource` or `DegradedIdleSource`).
//!
//! Gated behind the `x11-idle` feature so a build that doesn't need
//! X11 support (e.g. macOS / Windows users) can drop the libX11
//! dependency entirely.

#![cfg(feature = "x11-idle")]

use std::time::Duration;

use rewind_core::ports::{IdleError, IdleReliability, IdleSource};

/// `user-idle` based idle source. Synchronous, cheap to clone
/// (it's just a unit struct).
#[derive(Debug, Clone, Copy, Default)]
pub struct UserIdleSource;

impl UserIdleSource {
    pub fn new() -> Self {
        Self
    }
}

impl IdleSource for UserIdleSource {
    fn idle_time(&self) -> Result<Duration, IdleError> {
        // `user_idle::UserIdle::get_time()` returns a wrapper
        // around a `u64` seconds count. 0.3 also exposes a
        // convenience `.duration()` method returning
        // `std::time::Duration` directly — prefer that so we
        // don't hand-roll the `from_secs` plumbing.
        let idle = user_idle::UserIdle::get_time().map_err(map_err)?;
        // Saturate at `Duration::MAX` rather than wrap. (For
        // `u64` -> `Duration::from_secs` this is a no-op
        // anyway — `from_secs` doesn't take `u128`, but the
        // value already is `u64` seconds.)
        Ok(idle.duration())
    }

    fn reliability(&self) -> IdleReliability {
        // The `user-idle` crate talks to a well-tested OS API
        // (XScreenSaverQueryInfo / GetLastInputInfo /
        // CGEventSourceSecondsSinceLastEventType). We treat its
        // responses as `Reliable` whenever we get one back; if
        // the call fails, `idle_time()` returns `Transient` and
        // the engine retries. This is the only backend the
        // engine trusts for idle pause/reset decisions.
        IdleReliability::Reliable
    }
}

/// Map `user_idle::Error` to our `IdleError`.
///
/// The `user-idle` crate doesn't expose a public `Error` enum
/// in 0.3 (only `Display`), so we fall back to its `Debug`
/// output for the message. Either way the engine treats it as
/// transient and retries next tick.
fn map_err(e: user_idle::Error) -> IdleError {
    // `Debug` is the most informative thing 0.3's `Error`
    // exposes; `Display` is just the string "Error". Future
    // versions split the variant, but for now a generic
    // "transient" classification is correct: the underlying
    // `libXss`, `winapi`, or CoreFoundation call will usually
    // succeed on the next tick if it failed.
    let msg = format!("user_idle error: {:?}", e);
    IdleError::Transient(msg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rewind_core::ports::IdleSource;

    /// Some test environments are headless (`$DISPLAY` empty, no
    /// Wayland socket), and `user-idle` on Linux tries to open
    /// an X display during `get_time()`. On those hosts the call
    /// would crash inside libX11 / libxcb rather than returning
    /// an error we can match. Skip the actual-X11-call tests
    /// when no display server looks reachable; the rest of the
    /// suite (reliability, ergonomics) still runs.
    #[cfg(target_os = "linux")]
    fn has_display() -> bool {
        std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    fn has_display() -> bool {
        // On Windows / macOS the user-idle crate talks to OS
        // APIs that don't need a display server; let those
        // tests run unconditionally.
        true
    }

    #[test]
    fn reliability_is_reliable() {
        let s = UserIdleSource::new();
        assert_eq!(s.reliability(), IdleReliability::Reliable);
    }

    #[test]
    fn idle_time_call_succeeds_or_transiently_errors() {
        if !has_display() {
            // Headless CI: libX11/libxcb in our sysroot
            // segfault rather than returning a graceful
            // error when there's no display to talk to.
            // The `reliability()` and ergonomic tests still
            // prove the struct compiles & behaves; the
            // X11 call path is exercised on a real desktop.
            return;
        }
        // Don't assert specific durations — the OS may
        // return 0 or any positive value. Just check it
        // doesn't panic and errors classify correctly.
        let s = UserIdleSource::new();
        let r = s.idle_time();
        match r {
            Ok(d) => assert!(d >= Duration::ZERO),
            Err(IdleError::Transient(_)) => { /* acceptable */ }
            Err(IdleError::Unsupported(_)) => {
                panic!("user-idle should not be Unsupported on a real platform")
            }
        }
    }

    #[test]
    fn idle_time_recovers_from_repeated_calls() {
        if !has_display() {
            // Same rationale as above; see `has_display`.
            return;
        }
        // Calling `idle_time()` many times must not deadlock
        // or leak — important because the runtime calls it
        // every second.
        let s = UserIdleSource::new();
        for _ in 0..5 {
            let _ = s.idle_time();
        }
    }

    #[test]
    fn is_default_and_copy() {
        let a: UserIdleSource = Default::default();
        let b = a; // Copy
        assert_eq!(a.reliability(), b.reliability());
    }
}
