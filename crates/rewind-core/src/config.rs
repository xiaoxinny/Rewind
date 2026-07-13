//! `AppConfig` — every tunable (breaks, strictness, idle, reminders,
//! hydration, posture, quiet hours, system) and its defaults.
//!
//! The struct is mirrored verbatim by `src/lib/types.ts` on the frontend.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::session::state::Strictness;

/// Helper: minutes → `Duration`. Used for the timer fields below so
/// the underlying units stay in the `time` crate's vocabulary rather
/// than scattering `* 60_000` through the call sites.
fn min(m: u32) -> Duration {
    Duration::from_secs(u64::from(m) * 60)
}

/// Helper: seconds → `Duration`.
fn sec(s: u32) -> Duration {
    Duration::from_secs(u64::from(s))
}

/// Break cadence and pre/post-break behaviour.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreakConfig {
    /// Minutes between each micro (20-20-20) break. Default `20`.
    pub micro_interval_min: u32,
    /// Seconds a micro-break lasts. Default `20`.
    pub micro_duration_sec: u32,
    /// Minutes between each rest (long) break. Default `60`.
    pub rest_interval_min: u32,
    /// Seconds a rest break lasts. Default `300` (5 min).
    pub rest_duration_sec: u32,
    /// Whether the `PreBreak` countdown before a break is shown.
    /// Default `true`. The engine disables it implicitly when
    /// `strictness == Strict` to make breaks fire immediately.
    pub pre_break_warn: bool,
    /// Seconds the `PreBreak` countdown lasts. Default `10`.
    pub pre_break_warn_sec: u32,
    /// Seconds the user gains by postponing a break. Default `300`.
    pub postpone_sec: u32,
    /// Maximum number of postposes per break. Default `3`.
    pub max_postpones: u32,
}

impl Default for BreakConfig {
    fn default() -> Self {
        Self {
            micro_interval_min: 20,
            micro_duration_sec: 20,
            rest_interval_min: 60,
            rest_duration_sec: 300,
            pre_break_warn: true,
            pre_break_warn_sec: 10,
            postpone_sec: 300,
            max_postpones: 3,
        }
    }
}

impl BreakConfig {
    /// Returns the `PreBreak` duration, or `None` if pre-break is off.
    pub fn pre_break_duration(&self) -> Option<Duration> {
        if self.pre_break_warn {
            Some(sec(self.pre_break_warn_sec))
        } else {
            None
        }
    }

    /// Target duration of a micro break.
    pub fn micro_duration(&self) -> Duration {
        sec(self.micro_duration_sec)
    }

    /// Target duration of a rest break.
    pub fn rest_duration(&self) -> Duration {
        sec(self.rest_duration_sec)
    }

    /// Interval between micro breaks.
    pub fn micro_interval(&self) -> Duration {
        min(self.micro_interval_min)
    }

    /// Interval between rest breaks.
    pub fn rest_interval(&self) -> Duration {
        min(self.rest_interval_min)
    }

    /// How long a postpone extends the deadline.
    pub fn postpone_duration(&self) -> Duration {
        sec(self.postpone_sec)
    }
}

/// Idle pause/reset policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdleConfig {
    /// Whether idle-driven pause/reset is active. On GNOME Wayland
    /// the adapter reports `Unreliable` and the shell flips this off
    /// automatically; kept on by default.
    pub enabled: bool,
    /// Idle seconds before we transition to `Paused{Idle}`. Default
    /// `90`.
    pub pause_sec: u32,
    /// Idle seconds before a return resets the cycle to `Focus` and
    /// counts the absence as a natural break. Default `300`.
    pub reset_sec: u32,
    /// Idle seconds below which we consider the user back (hysteresis
    /// to prevent thrashing on the boundary). Default `10`.
    pub resume_sec: u32,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pause_sec: 90,
            reset_sec: 300,
            resume_sec: 10,
        }
    }
}

impl IdleConfig {
    pub fn pause(&self) -> Duration {
        sec(self.pause_sec)
    }
    pub fn reset(&self) -> Duration {
        sec(self.reset_sec)
    }
    pub fn resume(&self) -> Duration {
        sec(self.resume_sec)
    }
}

/// Reminder surface toggles. Each pillar can be muted
/// independently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReminderToggles {
    pub eye_breaks: bool,
    pub eye_exercises: bool,
    pub hydration: bool,
    pub posture: bool,
}

impl Default for ReminderToggles {
    fn default() -> Self {
        Self {
            eye_breaks: true,
            eye_exercises: true,
            hydration: true,
            posture: true,
        }
    }
}

/// Hydration tunables.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HydrationConfig {
    /// Daily fluid intake goal in millilitres. Default `2000`.
    pub goal_ml: u32,
    /// Quick-log amount per click (one glass). Default `250`.
    pub glass_ml: u32,
    /// Waking-window start, "HH:MM" local. Default `09:00`.
    pub wake_start: String,
    /// Waking-window end, "HH:MM" local. Default `21:00`.
    pub wake_end: String,
}

impl Default for HydrationConfig {
    fn default() -> Self {
        Self {
            goal_ml: 2000,
            glass_ml: 250,
            wake_start: "09:00".to_string(),
            wake_end: "21:00".to_string(),
        }
    }
}

/// Posture tunables.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PostureConfig {
    /// Minutes between posture nudges. Default `40`.
    pub interval_min: u32,
}

impl Default for PostureConfig {
    fn default() -> Self {
        Self { interval_min: 40 }
    }
}

impl PostureConfig {
    pub fn interval(&self) -> Duration {
        min(self.interval_min)
    }
}

/// Quiet hours. When `enabled` is `true` and the wall
/// clock is inside `[start, end)`, the scheduler defers reminders.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuietHoursConfig {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".to_string(),
            end: "08:00".to_string(),
        }
    }
}

/// System-level tunables.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemConfig {
    pub autostart: bool,
    pub start_minimized: bool,
    pub sound: bool,
    /// `0.0..=1.0`
    pub volume: f32,
    /// `system | light | dark`
    pub theme: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            start_minimized: true,
            sound: true,
            volume: 0.5,
            theme: "system".to_string(),
        }
    }
}

/// The full tunables bundle. Mirrored exactly by `src/lib/types.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub breaks: BreakConfig,
    pub strictness: Strictness,
    pub idle: IdleConfig,
    pub reminders: ReminderToggles,
    pub hydration: HydrationConfig,
    pub posture: PostureConfig,
    pub quiet_hours: QuietHoursConfig,
    pub system: SystemConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            breaks: BreakConfig::default(),
            strictness: Strictness::default(),
            idle: IdleConfig::default(),
            reminders: ReminderToggles::default(),
            hydration: HydrationConfig::default(),
            posture: PostureConfig::default(),
            quiet_hours: QuietHoursConfig::default(),
            system: SystemConfig::default(),
        }
    }
}

impl AppConfig {
    /// True if the engine should emit eye-break (PreBreak /
    /// MicroBreak) cycles at all. `Strictness == Strict` short-circuits
    /// the pre-break warning but **not** the break itself.
    pub fn eye_breaks_enabled(&self) -> bool {
        self.reminders.eye_breaks
    }

    /// Hydration waking window as local-clock minutes from midnight.
    /// Parsing uses `time::Time` so invalid clock values do not leak
    /// into scheduler math; malformed strings fall back to `09:00`.
    pub fn waking_window_minutes(&self) -> (u32, u32) {
        (
            parse_hh_mm_minutes(&self.hydration.wake_start),
            parse_hh_mm_minutes(&self.hydration.wake_end),
        )
    }

    /// Quiet-hours window as local-clock minutes from midnight.
    pub fn quiet_hours_minutes(&self) -> (u32, u32) {
        (
            parse_hh_mm_minutes(&self.quiet_hours.start),
            parse_hh_mm_minutes(&self.quiet_hours.end),
        )
    }
}

fn parse_hh_mm_minutes(s: &str) -> u32 {
    let mut parts = s.split(':');
    let hour = parts.next().and_then(|h| h.parse::<u8>().ok()).unwrap_or(9);
    let minute = parts.next().and_then(|m| m.parse::<u8>().ok()).unwrap_or(0);
    match time::Time::from_hms(hour, minute, 0) {
        Ok(t) => u32::from(t.hour()) * 60 + u32::from(t.minute()),
        Err(_) => 9 * 60,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_section_8b() {
        let cfg = AppConfig::default();

        // breaks
        assert_eq!(cfg.breaks.micro_interval_min, 20);
        assert_eq!(cfg.breaks.micro_duration_sec, 20);
        assert_eq!(cfg.breaks.rest_interval_min, 60);
        assert_eq!(cfg.breaks.rest_duration_sec, 300);
        assert!(cfg.breaks.pre_break_warn);
        assert_eq!(cfg.breaks.pre_break_warn_sec, 10);
        assert_eq!(cfg.breaks.postpone_sec, 300);
        assert_eq!(cfg.breaks.max_postpones, 3);

        // strictness
        assert_eq!(cfg.strictness, Strictness::Gentle);

        // idle
        assert!(cfg.idle.enabled);
        assert_eq!(cfg.idle.pause_sec, 90);
        assert_eq!(cfg.idle.reset_sec, 300);

        // reminders
        assert!(cfg.reminders.eye_breaks);
        assert!(cfg.reminders.eye_exercises);
        assert!(cfg.reminders.hydration);
        assert!(cfg.reminders.posture);

        // hydration
        assert_eq!(cfg.hydration.goal_ml, 2000);
        assert_eq!(cfg.hydration.glass_ml, 250);
        assert_eq!(cfg.hydration.wake_start, "09:00");
        assert_eq!(cfg.hydration.wake_end, "21:00");

        // posture
        assert_eq!(cfg.posture.interval_min, 40);

        // quiet hours
        assert!(!cfg.quiet_hours.enabled);
        assert_eq!(cfg.quiet_hours.start, "22:00");
        assert_eq!(cfg.quiet_hours.end, "08:00");

        // system
        assert!(!cfg.system.autostart);
        assert!(cfg.system.start_minimized);
        assert!(cfg.system.sound);
        assert!((cfg.system.volume - 0.5).abs() < f32::EPSILON);
        assert_eq!(cfg.system.theme, "system");
    }

    #[test]
    fn duration_helpers() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.breaks.micro_interval(), Duration::from_secs(20 * 60));
        assert_eq!(cfg.breaks.micro_duration(), Duration::from_secs(20));
        assert_eq!(cfg.breaks.rest_interval(), Duration::from_secs(60 * 60));
        assert_eq!(cfg.breaks.rest_duration(), Duration::from_secs(300));
        assert_eq!(cfg.breaks.postpone_duration(), Duration::from_secs(300));
        assert_eq!(
            cfg.breaks.pre_break_duration(),
            Some(Duration::from_secs(10))
        );

        // Disabling the pre-break warning should collapse to None.
        let mut cfg = cfg;
        cfg.breaks.pre_break_warn = false;
        assert_eq!(cfg.breaks.pre_break_duration(), None);

        assert_eq!(cfg.idle.pause(), Duration::from_secs(90));
        assert_eq!(cfg.idle.reset(), Duration::from_secs(300));
        assert_eq!(cfg.idle.resume(), Duration::from_secs(10));

        assert_eq!(cfg.posture.interval(), Duration::from_secs(40 * 60));
    }

    #[test]
    fn eye_breaks_enabled_default() {
        let cfg = AppConfig::default();
        assert!(cfg.eye_breaks_enabled());
        let mut cfg = cfg;
        cfg.reminders.eye_breaks = false;
        assert!(!cfg.eye_breaks_enabled());
    }

    #[test]
    fn waking_window_minutes_parses_hh_mm() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.waking_window_minutes(), (9 * 60, 21 * 60));

        let mut cfg = cfg;
        cfg.hydration.wake_start = "07:30".to_string();
        cfg.hydration.wake_end = "21:00".to_string();
        assert_eq!(cfg.waking_window_minutes(), (7 * 60 + 30, 21 * 60));
    }

    #[test]
    fn waking_window_minutes_allows_same_start_end_for_disabled_window() {
        let mut cfg = AppConfig::default();
        cfg.hydration.wake_start = "21:00".to_string();
        cfg.hydration.wake_end = "21:00".to_string();
        assert_eq!(cfg.waking_window_minutes(), (21 * 60, 21 * 60));
    }

    #[test]
    fn waking_window_minutes_falls_back_for_invalid_values() {
        let mut cfg = AppConfig::default();
        cfg.hydration.wake_start = "not-a-time".to_string();
        cfg.hydration.wake_end = "99:99".to_string();
        assert_eq!(cfg.waking_window_minutes(), (9 * 60, 9 * 60));
    }

    #[test]
    fn quiet_hours_minutes_parse_default_overnight_window() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.quiet_hours_minutes(), (22 * 60, 8 * 60));
    }

    #[test]
    fn serde_roundtrip_matches_section_8b_json_shape() {
        // The store on disk mirrors this exact shape; the frontend
        // mirrors the same. Verifying a JSON roundtrip locks the
        // shape in.
        let cfg = AppConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        // A handful of representative assertions against the
        // expected JSON shape.
        assert!(
            json.contains("\"microIntervalMin\"") == false,
            "snake_case in core; frontend mirrors"
        );
        assert!(json.contains("\"micro_interval_min\""));
        assert!(json.contains("\"rest_interval_min\": 60"));
        assert!(json.contains("\"goal_ml\": 2000"));
        assert!(json.contains("\"strictness\": \"gentle\""));

        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
