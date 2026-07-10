// Typed `invoke()` wrappers for every Rust command + an event
// listener for `core-event`. M6 wires in every command the engine
// + storage expose — see `src-tauri/src/ipc.rs` for the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppConfig,
  BreakKind,
  BreakRecord,
  CoreEvent,
  DailyAggregate,
  EngineSnapshot,
  HydrationEntry,
  HydrationProgress,
  PauseReason,
  SessionRecord,
  SessionState,
  Strictness,
  Timestamp,
} from "./types";

// ---------------------------------------------------------------------------
// Helpers — small ergonomic layer on top of `invoke`.
// ---------------------------------------------------------------------------

/** Wall-clock "now" as a `Timestamp`. The engine prefers unix-ms
 *  monotonically increasing — `Date.now()` is fine for our needs. */
function now(): Timestamp {
  return { unixMs: Date.now() };
}

// ---------------------------------------------------------------------------
// Engine commands — pass through to `CoreCommand`s and return the
// observed events as a `CoreEventDto[]`. The frontend rarely needs
// the return value because the same events stream over `core-event`,
// but the return value is useful for tests and for commands that
// caller wants synchronous confirmation on.
// ---------------------------------------------------------------------------

export const startFocus = () =>
  invoke<unknown[]>("start_focus", { now: now() });

export const pauseToggle = () =>
  invoke<unknown[]>("pause_toggle", { now: now() });

export const skipBreak = () => invoke<unknown[]>("skip_break", { now: now() });

export const postponeBreak = () =>
  invoke<unknown[]>("postpone_break", { now: now() });

export const logWater = (amountMl: number) =>
  invoke<unknown[]>("log_water", { amountMl, now: now() });

export const updateConfig = (config: AppConfig) =>
  invoke<unknown[]>("update_config", { config, now: now() });

export const setStrictness = (strictness: Strictness) =>
  invoke<unknown[]>("set_strictness", { strictness, now: now() });

// ---------------------------------------------------------------------------
// Autostart — direct passthrough to `tauri-plugin-autostart`.
// Returns the new `is_enabled()` status so the UI can resync.
// ---------------------------------------------------------------------------

export const setAutostart = (enabled: boolean): Promise<boolean> =>
  invoke<boolean>("set_autostart", { enabled });

// ---------------------------------------------------------------------------
// Engine snapshot — composite state read.
// ---------------------------------------------------------------------------

export const getEngineSnapshot = (): Promise<EngineSnapshot> =>
  invoke<EngineSnapshot>("get_engine_snapshot");

// ---------------------------------------------------------------------------
// History reads — wired through to the SQLite-backed `StorageApp`.
// ---------------------------------------------------------------------------

export const getToday = (): Promise<DailyAggregate> =>
  invoke<DailyAggregate>("get_today");

export const getRecentSessions = (days: number): Promise<SessionRecord[]> =>
  invoke<SessionRecord[]>("get_recent_sessions", { days });

export const getRecentHydration = (days: number): Promise<HydrationEntry[]> =>
  invoke<HydrationEntry[]>("get_recent_hydration", { days });

export interface ExportPayload {
  json: string;
  generated_at: string;
  row_count: number;
}

export const exportData = (): Promise<ExportPayload> =>
  invoke<ExportPayload>("export_data");

export const clearHistory = (): Promise<void> =>
  invoke<void>("clear_history");

// ---------------------------------------------------------------------------
// Type-only label helpers — kept here so the frontend doesn't have
// to import the Tauri command namespace for trivial strings.
// ---------------------------------------------------------------------------

export const getBreakKindLabel = (kind: BreakKind): Promise<string> =>
  invoke<string>("get_break_kind_label", { kind });

export const getPauseReasonLabel = (reason: PauseReason): Promise<string> =>
  invoke<string>("get_pause_reason_label", { reason });

// ---------------------------------------------------------------------------
// Event subscription — wraps Tauri's `listen` with strict typing.
// Returns the `UnlistenFn` so callers can drop the subscription.
// ---------------------------------------------------------------------------

export async function onCoreEvent(
  handler: (event: CoreEvent) => void,
): Promise<UnlistenFn> {
  return listen<CoreEvent>("core-event", (e) => handler(e.payload));
}

export async function onTrayPauseToggle(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen("tray-pause-toggle", () => handler());
}

// ---------------------------------------------------------------------------
// Re-export the helpers that don't need a re-declaration.
// ---------------------------------------------------------------------------

export type {
  AppConfig,
  BreakKind,
  BreakRecord,
  CoreEvent,
  DailyAggregate,
  EngineSnapshot,
  HydrationEntry,
  HydrationProgress,
  PauseReason,
  SessionRecord,
  SessionState,
  Strictness,
  Timestamp,
};
