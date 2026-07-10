//! Wayland idle source — `ext-idle-notify-v1` / `kwin_idle`.
//!
//! KWin and Sway expose idle via standard Wayland protocols; GNOME/Mutter
//! does **not** and is the case the degraded source handles. See
//! implementation plan §18.

// TODO M2: implement `IdleSource` against the wayland-protocols
// TODO M2:   `ext-idle-notify-v1` generator crate.
