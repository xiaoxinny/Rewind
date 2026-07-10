//! Engine ↔ shell boundary: `CoreEvent` (out) and `CoreCommand` (in).
//!
//! See implementation plan §7c. The engine never touches the OS; it
//! returns `CoreEvent`s, and the shell (`src-tauri`) dispatches them
//! to adapters and the frontend.
//!
//! **Stability contract.** All payloads serialize via `serde_json`.
//! The TypeScript mirror lives at `src/lib/types.ts` — keep them in
//! sync. Any rename is a breaking change for the IPC bridge.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::config::AppConfig;
use crate::scheduler::reminder::{Priority, Reminder, ReminderKind};
use crate::session::state::{BreakKind, SessionState, Strictness};

/// How a break should be **presented** to the user. The strictness
/// path (per §11, §15 DP-5) is decided by the engine based on
/// `AppConfig::strictness` and is consumed by the overlay adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakPresentation {
    /// Full-screen, input-capturing overlay (used by Strict).
    Strict,
    /// Small transparent banner that does not steal focus (used by
    /// Gentle — the default).
    Gentle,
}

impl BreakPresentation {
    /// Pick the right presentation for the current strictness.
    pub fn for_strictness(s: Strictness) -> Self {
        match s {
            // §11: "Gentle = the default". Strict is the only
            // branch that escalates to a full-screen capture.
            Strictness::Strict => BreakPresentation::Strict,
            Strictness::Normal | Strictness::Gentle => BreakPresentation::Gentle,
        }
    }
}

/// A live hydration progress snapshot — used by the dashboard and the
/// tray tooltip ("water 1.2/2.0 L").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HydrationProgress {
    pub consumed_ml: u32,
    pub goal_ml: u32,
}

impl HydrationProgress {
    pub fn new(consumed_ml: u32, goal_ml: u32) -> Self {
        Self {
            consumed_ml,
            goal_ml,
        }
    }

    pub fn ratio(&self) -> f32 {
        if self.goal_ml == 0 {
            0.0
        } else {
            self.consumed_ml as f32 / self.goal_ml as f32
        }
    }

    pub fn label(&self) -> String {
        format!("{} / {} ml", self.consumed_ml, self.goal_ml)
    }
}

/// What the tray should display right now. The shell calls
/// `Tray::set_status` whenever one of these is emitted. The
/// `tooltip_line` is the bread-and-butter countdown — M1's primary
/// visible behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayStatus {
    /// Short title for some platforms (e.g. macOS).
    pub title: String,
    /// Tooltip text. M1 cycles between "Next break in 19:42",
    /// "Micro break 0:18" and "Paused".
    pub tooltip_line: String,
    /// Optional emoji/glyph used by hosts that support it.
    pub icon_hint: String,
}

impl TrayStatus {
    /// Convenience constructor for the most common M1 case.
    pub fn countdown(next_break_in: Duration) -> Self {
        let secs = next_break_in.as_secs();
        let m = secs / 60;
        let s = secs % 60;
        Self {
            title: "Rewind".to_string(),
            tooltip_line: format!("Next break in {m}:{s:02}"),
            icon_hint: "focus".to_string(),
        }
    }

    /// Tray status for the user being inside a break.
    pub fn in_break(kind: BreakKind, remaining: Duration) -> Self {
        let secs = remaining.as_secs();
        let m = secs / 60;
        let s = secs % 60;
        let label = match kind {
            BreakKind::Micro => "Micro break",
            BreakKind::Rest => "Rest break",
        };
        Self {
            title: "Rewind".to_string(),
            tooltip_line: format!("{label} — {m}:{s:02} left"),
            icon_hint: "break".to_string(),
        }
    }

    /// Tray status when the engine is paused.
    pub fn paused(reason_text: &str) -> Self {
        Self {
            title: "Rewind".to_string(),
            tooltip_line: format!("Paused ({reason_text})"),
            icon_hint: "paused".to_string(),
        }
    }

    /// Tray status for the pre-break warning.
    pub fn pre_break(kind: BreakKind, remaining: Duration) -> Self {
        let secs = remaining.as_secs();
        let label = match kind {
            BreakKind::Micro => "Micro break soon",
            BreakKind::Rest => "Rest break soon",
        };
        Self {
            title: "Rewind".to_string(),
            tooltip_line: format!("{label} — {secs}s"),
            icon_hint: "pre-break".to_string(),
        }
    }
}

/// A menu item the tray surface should show. Clicks come back as a
/// `CoreCommand`. See §7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayMenuItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    /// `Some(id)` for separator / disabled menu-styling cases.
    pub kind: TrayMenuItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayMenuItemKind {
    /// Normal clickable item.
    Action,
    /// Non-interactive header / divider.
    Separator,
    /// A checkbox-style toggle; `checked` is its state.
    Checkbox { checked: bool },
}

impl TrayMenuItem {
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            kind: TrayMenuItemKind::Action,
        }
    }
    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: "—".to_string(),
            enabled: false,
            kind: TrayMenuItemKind::Separator,
        }
    }
}

/// Notification payload carried by `CoreEvent::FireReminder` (and
/// handed to `Notifier::notify` by the shell). Distinct from
/// `BreakPresentation`: this is the **gentle** toast/banner channel
/// for non-break reminders (hydration, posture).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub kind: ReminderKind,
}

/// Engine → shell. Every interaction starts and ends with one of
/// these coming out of `Engine::tick` / `Engine::handle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoreEvent {
    /// The high-level session phase changed (Focus ↔ PreBreak ↔
    /// MicroBreak/RestBreak ↔ Postponed ↔ Paused). Emit at most once
    /// per transition.
    StateChanged(SessionState),

    /// A 1 Hz heartbeat carrying the current phase + remaining
    /// countdown. Drives the tray tooltip. Always emitted on every
    /// tick from M2 onward; M1 emits it opportunistically from the
    /// IPC layer.
    Tick {
        phase: SessionState,
        remaining: Duration,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        now: Option<Timestamp>,
    },

    /// Show the break overlay. The shell consults
    /// `BreakPresentation` to decide between full-screen (Strict)
    /// and gentle banner.
    ShowBreak {
        kind: BreakKind,
        presentation: BreakPresentation,
        /// Optional suggested exercise id (rest breaks in M4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exercise_id: Option<String>,
    },

    /// Dismiss the break overlay. The shell clears any active
    /// overlay window / banner.
    DismissBreak,

    /// A non-break reminder surfaced by the coordinator.
    FireReminder {
        kind: ReminderKind,
        priority: Priority,
        message: String,
    },

    /// Hydration totals updated. Payload is the latest snapshot.
    HydrationUpdated(HydrationProgress),

    /// Tray tooltip/icon should refresh.
    TrayStatus(TrayStatus),

    /// Tray menu changed (rare). The shell rebuilds the menu.
    TrayMenu(Vec<TrayMenuItem>),
}

impl CoreEvent {
    /// True for events the frontend should always observe (the ones
    /// the IPC layer forwards unconditionally). Useful for tests.
    pub fn is_state_change(&self) -> bool {
        matches!(self, CoreEvent::StateChanged(_))
    }
}

// Forward helper impls so callers don't have to spell out the full
// reminder types.
impl CoreEvent {
    pub fn fire_reminder(r: Reminder, message: impl Into<String>) -> Self {
        CoreEvent::FireReminder {
            kind: r.kind,
            priority: r.priority,
            message: message.into(),
        }
    }
}

/// Shell/frontend → engine. Every IPC command translates to one of
/// these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoreCommand {
    /// User asked to (re)start the focus cycle. For M1 this is a
    /// no-op while already in Focus; future milestones may surface
    /// "session start" semantics.
    StartFocus,
    /// Toggle manual pause/resume.
    PauseToggle,
    /// Skip the current break (allowed iff strictness != Strict).
    SkipBreak,
    /// Postpone the current/pending break by `postponeSec`.
    PostponeBreak,
    /// Log water consumption.
    LogWater(u32 /* ml */),
    /// Feed the engine an external idle observation. M1 ignores this
    /// (idle adapter lands in M2); the shell wires it through to
    /// prepare for M2.
    IdleObserved(Duration),
    /// Replace the entire config (the IPC layer merges & validates).
    ConfigUpdated(AppConfig),
    /// Set just the strictness field without resending the full
    /// bundle. Convenience for live settings tweaks.
    SetStrictness(Strictness),
}

/// Public re-export alias so consumers can name "the config snapshot
/// type" without importing the full `AppConfig`. Currently a plain
/// type alias — the snapshot view lands in M6.
pub type CoreConfigSnapshot = crate::config::AppConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::PauseReason;

    #[test]
    fn hydration_progress_label_and_ratio() {
        let p = HydrationProgress::new(500, 2000);
        assert_eq!(p.label(), "500 / 2000 ml");
        assert!((p.ratio() - 0.25).abs() < 1e-6);

        // Zero goal → ratio 0, no panic.
        let p = HydrationProgress::new(100, 0);
        assert_eq!(p.ratio(), 0.0);
    }

    #[test]
    fn tray_status_countdown_format_minutes_seconds() {
        let s = TrayStatus::countdown(Duration::from_secs(19 * 60 + 7));
        assert_eq!(s.tooltip_line, "Next break in 19:07");
        assert_eq!(s.icon_hint, "focus");
    }

    #[test]
    fn tray_status_countdown_zero() {
        let s = TrayStatus::countdown(Duration::from_secs(0));
        assert_eq!(s.tooltip_line, "Next break in 0:00");
    }

    #[test]
    fn tray_status_in_break_minutes() {
        let s = TrayStatus::in_break(BreakKind::Micro, Duration::from_secs(45));
        assert_eq!(s.tooltip_line, "Micro break — 0:45 left");

        let s = TrayStatus::in_break(BreakKind::Rest, Duration::from_secs(125));
        assert_eq!(s.tooltip_line, "Rest break — 2:05 left");
    }

    #[test]
    fn tray_status_paused_text() {
        let s = TrayStatus::paused("idle");
        assert_eq!(s.tooltip_line, "Paused (idle)");
    }

    #[test]
    fn tray_status_pre_break_text() {
        let s = TrayStatus::pre_break(BreakKind::Micro, Duration::from_secs(7));
        assert_eq!(s.tooltip_line, "Micro break soon — 7s");

        let s = TrayStatus::pre_break(BreakKind::Rest, Duration::from_secs(10));
        assert_eq!(s.tooltip_line, "Rest break soon — 10s");
    }

    #[test]
    fn tray_menu_item_helpers() {
        let action = TrayMenuItem::action("toggle", "Toggle pause");
        assert_eq!(action.id, "toggle");
        assert_eq!(action.label, "Toggle pause");
        assert!(action.enabled);
        assert_eq!(action.kind, TrayMenuItemKind::Action);

        let sep = TrayMenuItem::separator();
        assert!(!sep.enabled);
        assert_eq!(sep.kind, TrayMenuItemKind::Separator);
    }

    #[test]
    fn break_presentation_for_strictness() {
        assert_eq!(
            BreakPresentation::for_strictness(Strictness::Strict),
            BreakPresentation::Strict
        );
        assert_eq!(
            BreakPresentation::for_strictness(Strictness::Normal),
            BreakPresentation::Gentle
        );
        assert_eq!(
            BreakPresentation::for_strictness(Strictness::Gentle),
            BreakPresentation::Gentle
        );
    }

    #[test]
    fn core_event_serde_round_trip() {
        let cases = [
            CoreEvent::StateChanged(SessionState::Focus),
            CoreEvent::StateChanged(SessionState::PreBreak {
                kind: BreakKind::Micro,
                remaining_ms: 9_000,
            }),
            CoreEvent::StateChanged(SessionState::MicroBreak {
                remaining_ms: 18_000,
            }),
            CoreEvent::StateChanged(SessionState::RestBreak {
                remaining_ms: 250_000,
            }),
            CoreEvent::StateChanged(SessionState::Postponed {
                kind: BreakKind::Rest,
                until_ms: 123,
            }),
            CoreEvent::StateChanged(SessionState::Paused {
                reason: PauseReason::Idle,
            }),
            CoreEvent::ShowBreak {
                kind: BreakKind::Rest,
                presentation: BreakPresentation::Gentle,
                exercise_id: None,
            },
            CoreEvent::DismissBreak,
            CoreEvent::FireReminder {
                kind: ReminderKind::Hydration,
                priority: Priority::Low,
                message: "Drink a glass of water".to_string(),
            },
            CoreEvent::HydrationUpdated(HydrationProgress::new(250, 2000)),
            CoreEvent::TrayStatus(TrayStatus::countdown(Duration::from_secs(60))),
            CoreEvent::TrayMenu(vec![TrayMenuItem::action("pause", "Pause")]),
        ];
        for ev in &cases {
            let json = serde_json::to_string(ev).unwrap();
            let back: CoreEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, ev, "round-trip mismatch for {ev:?}");
        }
    }

    #[test]
    fn core_command_serde_round_trip() {
        let cmds = [
            CoreCommand::StartFocus,
            CoreCommand::PauseToggle,
            CoreCommand::SkipBreak,
            CoreCommand::PostponeBreak,
            CoreCommand::LogWater(250),
            CoreCommand::IdleObserved(Duration::from_secs(45)),
            CoreCommand::SetStrictness(Strictness::Strict),
        ];
        for c in &cmds {
            let json = serde_json::to_string(c).unwrap();
            let back: CoreCommand = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, c);
        }
    }

    #[test]
    fn notification_constructors() {
        let n = Notification {
            title: "Hydration".to_string(),
            body: "You're a bit behind — drink a glass".to_string(),
            kind: ReminderKind::Hydration,
        };
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("\"kind\":\"hydration\""));
        assert!(json.contains("\"title\":\"Hydration\""));
    }

    #[test]
    fn fire_reminder_helper_propagates_kind_and_priority() {
        let r = Reminder {
            kind: ReminderKind::Posture,
            priority: Priority::Medium,
            earliest: 0,
        };
        let ev = CoreEvent::fire_reminder(r, "Stand up and stretch");
        match ev {
            CoreEvent::FireReminder {
                kind,
                priority,
                message,
            } => {
                assert_eq!(kind, ReminderKind::Posture);
                assert_eq!(priority, Priority::Medium);
                assert_eq!(message, "Stand up and stretch");
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn state_changed_detection() {
        assert!(CoreEvent::StateChanged(SessionState::Focus).is_state_change());
        assert!(!CoreEvent::DismissBreak.is_state_change());
        assert!(!CoreEvent::TrayStatus(TrayStatus::countdown(Duration::ZERO)).is_state_change());
    }
}
