// TypeScript mirror of `rewind-core` types. Keep in sync with the Rust
// definitions — see implementation plan §10 and §7c. M6 ships the
// full mirror: every CoreEvent / CoreCommand / AppConfig / DTO shape
// the engine emits on the IPC bridge.

export type Millis = number;

/** Unix milliseconds since the epoch. Mirror of `rewind_core::Timestamp`. */
export interface Timestamp {
  readonly unixMs: number;
}

export const REWIND_VERSION = "0.1.0-m6";

// ---------------------------------------------------------------------------
// BreakKind / Strictness / PauseReason (engine state enums).
// ---------------------------------------------------------------------------

/** `BreakKind`: `micro` (20-20-20) | `rest` (long Pomodoro + exercise). */
export type BreakKind = "micro" | "rest";

/** `Strictness`: Gentle (default) → Normal → Strict. */
export type Strictness = "gentle" | "normal" | "strict";

/** `PauseReason`: `idle` (auto) | `manual` (toggle). */
export type PauseReason = "idle" | "manual";

/** Engine → frontend: the high-level session phase. */
export type SessionState =
  | { type: "focus" }
  | { type: "pre_break"; kind: BreakKind; remaining_ms: number }
  | { type: "micro_break"; remaining_ms: number }
  | { type: "rest_break"; remaining_ms: number }
  | { type: "postponed"; kind: BreakKind; until_ms: number }
  | { type: "paused"; reason: PauseReason };

/** Helper used by the store: short human label. */
export function sessionStateLabel(s: SessionState): string {
  switch (s.type) {
    case "focus":
      return "Focus";
    case "pre_break":
      return s.kind === "micro" ? "Micro break soon" : "Rest break soon";
    case "micro_break":
      return "Micro break";
    case "rest_break":
      return "Rest break";
    case "postponed":
      return "Postponed";
    case "paused":
      return s.reason === "idle" ? "Paused (idle)" : "Paused";
  }
}

// ---------------------------------------------------------------------------
// Reminders — non-break nudges (hydration, posture).
// ---------------------------------------------------------------------------

export type ReminderKind = "eye_break" | "hydration" | "posture";
export type Priority = "low" | "medium" | "high";

// ---------------------------------------------------------------------------
// Hydration.
// ---------------------------------------------------------------------------

export interface HydrationProgress {
  consumed_ml: number;
  goal_ml: number;
}

// ---------------------------------------------------------------------------
// AppConfig — every tunable. Mirrors `rewind_core::AppConfig`.
// ---------------------------------------------------------------------------

export interface BreakConfig {
  microIntervalMin: number;
  microDurationSec: number;
  restIntervalMin: number;
  restDurationSec: number;
  preBreakWarn: boolean;
  preBreakWarnSec: number;
  postponeSec: number;
  maxPostpones: number;
}

export interface IdleConfig {
  enabled: boolean;
  pauseSec: number;
  resetSec: number;
  resumeSec: number;
}

export interface ReminderToggles {
  eyeBreaks: boolean;
  eyeExercises: boolean;
  hydration: boolean;
  posture: boolean;
}

export interface HydrationConfig {
  goalMl: number;
  glassMl: number;
  wakeStart: string; // "HH:MM"
  wakeEnd: string; // "HH:MM"
}

export interface PostureConfig {
  intervalMin: number;
}

export interface QuietHoursConfig {
  enabled: boolean;
  start: string;
  end: string;
}

export interface SystemConfig {
  autostart: boolean;
  startMinimized: boolean;
  sound: boolean;
  volume: number;
  theme: "system" | "light" | "dark";
}

export interface AppConfig {
  breaks: BreakConfig;
  strictness: Strictness;
  idle: IdleConfig;
  reminders: ReminderToggles;
  hydration: HydrationConfig;
  posture: PostureConfig;
  quietHours: QuietHoursConfig;
  system: SystemConfig;
}

// ---------------------------------------------------------------------------
// History DTOs.
// ---------------------------------------------------------------------------

export interface SessionRecord {
  id?: number;
  started_at: Timestamp;
  ended_at?: Timestamp;
  active_ms: number;
  end_reason?: "completed" | "idle_reset" | "quit";
}

export interface BreakRecord {
  id?: number;
  session_id: number;
  kind: BreakKind;
  scheduled_at: Timestamp;
  started_at?: Timestamp;
  ended_at?: Timestamp;
  outcome: "completed" | "skipped" | "postponed" | "natural";
  exercise_id?: string;
}

export interface HydrationEntry {
  id?: number;
  logged_at: Timestamp;
  amount_ml: number;
  source: "reminder" | "manual";
}

export interface DailyAggregate {
  day: string;
  active_ms: number;
  breaks_taken: number;
  breaks_skipped: number;
  water_ml: number;
  water_goal_ml: number;
  posture_prompts: number;
}

// ---------------------------------------------------------------------------
// CoreEvent — tagged union sent via the `core-event` Tauri bus.
// The shape matches `CoreEventDto` in `src-tauri/src/ipc.rs`.
// ---------------------------------------------------------------------------

export interface TrayStatusLike {
  title: string;
  tooltip_line: string;
  icon_hint: string;
}

export type CoreEvent =
  | { type: "state_changed"; state: SessionState }
  | { type: "tick"; phase: SessionState; remaining_ms: number; now_ms: number }
  | {
      type: "show_break";
      kind: BreakKind;
      presentation_strict: boolean;
      exercise_id: string | null;
    }
  | { type: "dismiss_break" }
  | {
      type: "fire_reminder";
      kind: ReminderKind;
      priority: Priority;
      message: string;
    }
  | { type: "hydration_updated"; consumed_ml: number; goal_ml: number }
  | { type: "tray_status"; tooltip_line: string; title: string; icon_hint: string }
  | { type: "tray_menu"; items: TrayMenuItem[] };

export interface TrayMenuItem {
  id: string;
  label: string;
  enabled: boolean;
}

// ---------------------------------------------------------------------------
// EngineSnapshot — the composite payload returned by `get_engine_snapshot`.
// ---------------------------------------------------------------------------

export interface EngineSnapshot {
  state: SessionState;
  config: AppConfig;
  hydration: HydrationProgress;
  today: DailyAggregate;
  idle_reliable: boolean;
}

// ---------------------------------------------------------------------------
// Defaults — defensive copy of `AppConfig::default()` for the
// frontend bootstrap before the first `get_engine_snapshot` returns.
// ---------------------------------------------------------------------------

export const DEFAULT_CONFIG: AppConfig = {
  breaks: {
    microIntervalMin: 20,
    microDurationSec: 20,
    restIntervalMin: 60,
    restDurationSec: 300,
    preBreakWarn: true,
    preBreakWarnSec: 10,
    postponeSec: 300,
    maxPostpones: 3,
  },
  strictness: "gentle",
  idle: { enabled: true, pauseSec: 90, resetSec: 300, resumeSec: 10 },
  reminders: {
    eyeBreaks: true,
    eyeExercises: true,
    hydration: true,
    posture: true,
  },
  hydration: { goalMl: 2000, glassMl: 250, wakeStart: "09:00", wakeEnd: "21:00" },
  posture: { intervalMin: 40 },
  quietHours: { enabled: false, start: "22:00", end: "08:00" },
  system: {
    autostart: false,
    startMinimized: true,
    sound: true,
    volume: 0.5,
    theme: "system",
  },
};
