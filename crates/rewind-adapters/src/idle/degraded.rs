//! Timer-only degraded idle source for GNOME Wayland (and any other
//! compositor without a usable idle protocol). Reports
//! `IdleReliability::Unavailable` so the engine knows to skip
//! idle-driven pause/reset. The UI shows an honest "screen-time
//! tracking limited on this session" note. See implementation plan §7f.

// TODO M2: implement `IdleSource` that always returns `Duration::ZERO`
// TODO M2:   with `IdleReliability::Unavailable`.
