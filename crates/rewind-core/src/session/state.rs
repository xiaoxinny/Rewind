//! Session state — enums for state, break kind, strictness, pause reason.
//!
//! See implementation plan §7e. The state machine is the heart of
//! Rewind (DP-1). These types are serialized across the IPC bridge
//! (`CoreEvent::StateChanged`) — keep the derive set conservative and
//! the variants stable.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Which kind of break the user is on. Micro = frequent+short (the
/// 20-20-20 eye rule); Rest = infrequent+long (the Workrave-style
/// Pomodoro + a guided eye exercise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BreakKind {
    Micro,
    Rest,
}

impl BreakKind {
    /// Lower-case stable string label for sqlite / JSON. Plan §8a
    /// uses `"micro"` and `"rest"`.
    pub fn as_str(self) -> &'static str {
        match self {
            BreakKind::Micro => "micro",
            BreakKind::Rest => "rest",
        }
    }
}

impl fmt::Display for BreakKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BreakKind::Micro => f.write_str("micro"),
            BreakKind::Rest => f.write_str("rest"),
        }
    }
}

/// Why the engine is paused. `Idle` is when the user has been inactive
/// past the pause threshold; `Manual` is when the user toggled pause
/// from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PauseReason {
    Idle,
    Manual,
}

impl fmt::Display for PauseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PauseReason::Idle => f.write_str("idle"),
            PauseReason::Manual => f.write_str("manual"),
        }
    }
}

/// How strict the engine should be when surfacing breaks. Gentle
/// (default) allows skipping/postponing freely; Normal allows
/// postponing but disallows skipping; Strict disallows both. The
/// tray tooltip and overlay copy reflect this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strictness {
    Gentle,
    Normal,
    Strict,
}

impl fmt::Display for Strictness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Strictness::Gentle => f.write_str("gentle"),
            Strictness::Normal => f.write_str("normal"),
            Strictness::Strict => f.write_str("strict"),
        }
    }
}

impl Strictness {
    /// Whether the user is allowed to skip the *current* break. Strict
    /// locks them out; Normal + Gentle allow it.
    pub fn allows_skip(self) -> bool {
        !matches!(self, Strictness::Strict)
    }
}

impl Default for Strictness {
    fn default() -> Self {
        // Per §13 / §8b — ship a Gentle default.
        Strictness::Gentle
    }
}

impl From<&str> for Strictness {
    fn from(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "strict" => Strictness::Strict,
            "normal" => Strictness::Normal,
            _ => Strictness::Gentle,
        }
    }
}

/// The current high-level session phase the engine is in. The variants
/// are deliberately **first-class** (DP-1) instead of flags — each
/// represents a coherent state with its own armed timers and emitted
/// events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Working. Both micro and rest timers are armed.
    Focus,

    /// A short warning countdown before a break fires (default 10 s;
    /// disabled under Gentle). `remaining_ms` ticks down so the UI
    /// can show a live countdown even before the break itself.
    PreBreak { kind: BreakKind, remaining_ms: u64 },

    /// The 20-20-20 micro-break. `remaining_ms` ticks down so the
    /// tray tooltip cycles 20 → 0.
    MicroBreak { remaining_ms: u64 },

    /// The longer rest break — full exercise routine + count-down.
    RestBreak { remaining_ms: u64 },

    /// The user postponed the pending/active break; wake back up at
    /// `until_ms` (monotonic). On wake the engine returns to
    /// `PreBreak` of the same kind.
    Postponed { kind: BreakKind, until_ms: u64 },

    /// Timers frozen. Either the user stepped away (Idle) or hit
    /// pause themselves (Manual).
    Paused { reason: PauseReason },
}

impl SessionState {
    /// Short human-readable label for tray/UI display. Used by
    /// `SessionMachine::state_label` and tray status text.
    pub fn label(&self) -> &'static str {
        match self {
            SessionState::Focus => "Focus",
            SessionState::PreBreak { kind, .. } => match kind {
                BreakKind::Micro => "Micro break soon",
                BreakKind::Rest => "Rest break soon",
            },
            SessionState::MicroBreak { .. } => "Micro break",
            SessionState::RestBreak { .. } => "Rest break",
            SessionState::Postponed { .. } => "Postponed",
            SessionState::Paused { reason } => match reason {
                PauseReason::Idle => "Paused (idle)",
                PauseReason::Manual => "Paused",
            },
        }
    }

    /// Whether we're in a break that should be visible to the user
    /// (i.e. `MicroBreak` / `RestBreak` / `PreBreak`). Used by the
    /// shell to decide whether to show the overlay and to gate
    /// reminder arbitration (breaks outrank all reminders).
    pub fn is_break(&self) -> bool {
        matches!(
            self,
            SessionState::MicroBreak { .. }
                | SessionState::RestBreak { .. }
                | SessionState::PreBreak { .. }
        )
    }

    /// Which kind of break is the current active break, if any.
    /// Returns `None` for non-break states.
    pub fn break_kind(&self) -> Option<BreakKind> {
        match self {
            SessionState::MicroBreak { .. } => Some(BreakKind::Micro),
            SessionState::RestBreak { .. } => Some(BreakKind::Rest),
            SessionState::PreBreak { kind, .. } => Some(*kind),
            SessionState::Postponed { kind, .. } => Some(*kind),
            _ => None,
        }
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictness_allows_skip_matrix() {
        assert!(Strictness::Gentle.allows_skip());
        assert!(Strictness::Normal.allows_skip());
        assert!(!Strictness::Strict.allows_skip());
    }

    #[test]
    fn strictness_default_is_gentle() {
        assert_eq!(Strictness::default(), Strictness::Gentle);
    }

    #[test]
    fn strictness_from_str() {
        assert_eq!(Strictness::from("gentle"), Strictness::Gentle);
        assert_eq!(Strictness::from("Strict"), Strictness::Strict);
        assert_eq!(Strictness::from("NORMAL"), Strictness::Normal);
        // Unknown → Gentle (forgiving).
        assert_eq!(Strictness::from("???"), Strictness::Gentle);
    }

    #[test]
    fn break_kind_display() {
        assert_eq!(BreakKind::Micro.to_string(), "micro");
        assert_eq!(BreakKind::Rest.to_string(), "rest");
    }

    #[test]
    fn pause_reason_display() {
        assert_eq!(PauseReason::Idle.to_string(), "idle");
        assert_eq!(PauseReason::Manual.to_string(), "manual");
    }

    #[test]
    fn session_state_label() {
        assert_eq!(SessionState::Focus.label(), "Focus");
        assert_eq!(
            SessionState::PreBreak {
                kind: BreakKind::Micro,
                remaining_ms: 5_000
            }
            .label(),
            "Micro break soon"
        );
        assert_eq!(
            SessionState::PreBreak {
                kind: BreakKind::Rest,
                remaining_ms: 5_000
            }
            .label(),
            "Rest break soon"
        );
        assert_eq!(
            SessionState::MicroBreak {
                remaining_ms: 18_000
            }
            .label(),
            "Micro break"
        );
        assert_eq!(
            SessionState::RestBreak {
                remaining_ms: 240_000
            }
            .label(),
            "Rest break"
        );
        assert_eq!(
            SessionState::Postponed {
                kind: BreakKind::Micro,
                until_ms: 0
            }
            .label(),
            "Postponed"
        );
        assert_eq!(
            SessionState::Paused {
                reason: PauseReason::Idle
            }
            .label(),
            "Paused (idle)"
        );
        assert_eq!(
            SessionState::Paused {
                reason: PauseReason::Manual
            }
            .label(),
            "Paused"
        );
    }

    #[test]
    fn session_state_is_break() {
        assert!(!SessionState::Focus.is_break());
        assert!(SessionState::PreBreak {
            kind: BreakKind::Micro,
            remaining_ms: 1
        }
        .is_break());
        assert!(SessionState::PreBreak {
            kind: BreakKind::Rest,
            remaining_ms: 1
        }
        .is_break());
        assert!(SessionState::MicroBreak { remaining_ms: 1 }.is_break());
        assert!(SessionState::RestBreak { remaining_ms: 1 }.is_break());
        assert!(!SessionState::Postponed {
            kind: BreakKind::Micro,
            until_ms: 0
        }
        .is_break());
        assert!(!SessionState::Paused {
            reason: PauseReason::Manual
        }
        .is_break());
        assert!(!SessionState::Paused {
            reason: PauseReason::Idle
        }
        .is_break());
    }

    #[test]
    fn session_state_break_kind() {
        assert_eq!(SessionState::Focus.break_kind(), None);
        assert_eq!(
            SessionState::MicroBreak { remaining_ms: 1 }.break_kind(),
            Some(BreakKind::Micro)
        );
        assert_eq!(
            SessionState::RestBreak { remaining_ms: 1 }.break_kind(),
            Some(BreakKind::Rest)
        );
        assert_eq!(
            SessionState::PreBreak {
                kind: BreakKind::Rest,
                remaining_ms: 1
            }
            .break_kind(),
            Some(BreakKind::Rest)
        );
        assert_eq!(
            SessionState::Postponed {
                kind: BreakKind::Micro,
                until_ms: 0
            }
            .break_kind(),
            Some(BreakKind::Micro)
        );
    }

    #[test]
    fn session_state_serde_round_trip() {
        // The state must serde-roundtrip — the IPC bridge relies on it.
        let cases = [
            SessionState::Focus,
            SessionState::PreBreak {
                kind: BreakKind::Micro,
                remaining_ms: 9_000,
            },
            SessionState::MicroBreak { remaining_ms: 20 },
            SessionState::RestBreak { remaining_ms: 300 },
            SessionState::Postponed {
                kind: BreakKind::Rest,
                until_ms: 123,
            },
            SessionState::Paused {
                reason: PauseReason::Idle,
            },
        ];
        for s in &cases {
            let json = serde_json::to_string(s).unwrap();
            let back: SessionState = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, s);
        }
    }
}
