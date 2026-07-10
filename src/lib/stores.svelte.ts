// Svelte 5 runes-based reactive mirror of the engine's state. M6
// replaces the M0 stub with a full shape:
//   * state, remainingMs, hydration — driven by `core-event`
//   * config — read via `get_engine_snapshot`, written via `update_config`
//   * today + recentSessions — reads from the SQLite-backed `StorageApp`
//   * `setEngineState()` mapper from a fresh `CoreEvent` into store mutations
//   * `startEventListener()` wires Tauri `listen()` to `setEngineState`

import {
  clearHistory,
  exportData,
  getEngineSnapshot,
  getRecentHydration,
  getRecentSessions,
  getToday,
  onCoreEvent,
  pauseToggle,
  logWater as ipcLogWater,
  updateConfig as ipcUpdateConfig,
} from "./ipc";
import type {
  AppConfig,
  BreakKind,
  CoreEvent,
  DailyAggregate,
  EngineSnapshot,
  HydrationEntry,
  HydrationProgress,
  SessionRecord,
  SessionState,
  Strictness,
} from "./types";
import { DEFAULT_CONFIG } from "./types";

// ---------------------------------------------------------------------------
// Internal types — the runes shape is loosely-typed (any-property
// runes in TS need explicit casts). The store consumes the strictly
// typed CoreEvent via setEngineState() and re-renders subscribers.
// ---------------------------------------------------------------------------

interface ReactiveState {
  /** Most recent session phase; defaults to `Focus`. */
  state: SessionState;
  /** Current countdown to the next user-visible engine event. */
  remainingMs: number;
  /** Latest hydration snapshot (consumed / goal). */
  todayHydrationMl: number;
  todayWaterGoalMl: number;
  /** Latest tray tooltip line (driver of the visible header copy). */
  trayStatus: { title: string; tooltip_line: string; icon_hint: string };
  /** Latest effective `AppConfig` — first read from snapshot, then
   *  mutated locally by `setConfig()` (which also persists). */
  config: AppConfig;
  /** Today's aggregated counters from the SQLite rollup. */
  today: DailyAggregate;
  /** Last N sessions for the Stats page. */
  recentSessions: SessionRecord[];
  /** Last N hydration entries for the Stats page. */
  recentHydration: HydrationEntry[];
  /** Whether the idle adapter reports `Reliable` — gates the idle
   *  controls in Settings (per §13, "auto-greyed with explanation
   *  on GNOME Wayland"). */
  idleReliable: boolean;
  /** UI-level transient state — true after the user clicks "Pause"
   *  and the engine confirms. */
  pendingCommandAck: boolean;
}

// ---------------------------------------------------------------------------
// Initial state — defaults are set immediately so the dashboard renders
// before `getEngineSnapshot` resolves.
// ---------------------------------------------------------------------------

const initialAggregate = (): DailyAggregate => ({
  day: new Date().toISOString().slice(0, 10),
  active_ms: 0,
  breaks_taken: 0,
  breaks_skipped: 0,
  water_ml: 0,
  water_goal_ml: 2000,
  posture_prompts: 0,
});

export const state = $state<ReactiveState>({
  state: { type: "focus" },
  remainingMs: 20 * 60 * 1_000,
  todayHydrationMl: 0,
  todayWaterGoalMl: 2000,
  trayStatus: {
    title: "Rewind",
    tooltip_line: "Loading…",
    icon_hint: "focus",
  },
  config: DEFAULT_CONFIG,
  today: initialAggregate(),
  recentSessions: [],
  recentHydration: [],
  idleReliable: true,
  pendingCommandAck: false,
});

// ---------------------------------------------------------------------------
// Internal helpers.
// ---------------------------------------------------------------------------

/** Hydration progress from a `HydrationProgress`-shaped payload. */
function setHydration(p: { consumed_ml: number; goal_ml: number }): void {
  state.todayHydrationMl = p.consumed_ml;
  state.todayWaterGoalMl = p.goal_ml;
  state.today = {
    ...state.today,
    water_ml: p.consumed_ml,
    water_goal_ml: p.goal_ml,
  };
}

/** Map a single `CoreEvent` onto the reactive store. Idempotent on
 *  any event shape — unknown variants just fall through silently. */
export function setEngineState(ev: CoreEvent): void {
  switch (ev.type) {
    case "state_changed":
      state.state = ev.state;
      break;
    case "tick":
      state.remainingMs = ev.remaining_ms;
      // On every tick, also refresh the tray line — keeps the
      // dashboard countdown and tray tooltip in sync.
      state.trayStatus = {
        title: state.trayStatus.title,
        tooltip_line: formatRemaining(ev.remaining_ms),
        icon_hint: state.trayStatus.icon_hint,
      };
      break;
    case "show_break":
      // Trigger animation state — handled per-route.
      state.remainingMs = state.remainingMs;
      state.trayStatus = {
        title: "Rewind",
        tooltip_line:
          ev.kind === "micro" ? "Micro break" : "Rest break",
        icon_hint: ev.kind,
      };
      break;
    case "dismiss_break":
      state.trayStatus = {
        title: "Rewind",
        tooltip_line: "Back to focus",
        icon_hint: "focus",
      };
      break;
    case "hydration_updated":
      setHydration({
        consumed_ml: ev.consumed_ml,
        goal_ml: ev.goal_ml,
      });
      break;
    case "tray_status":
      state.trayStatus = {
        title: ev.title,
        tooltip_line: ev.tooltip_line,
        icon_hint: ev.icon_hint,
      };
      break;
    case "fire_reminder":
    case "tray_menu":
      // Display-layer ignores these; the dashboard surfaces
      // hydration/posture nudges via separate toasts (roadmap).
      break;
  }
}

/** `mm:ss` format for a remaining-ms count. */
function formatRemaining(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1_000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Reformat a `CoreEvent`-style `tick` event for the dashboard. */
export function remainingLabel(): string {
  return formatRemaining(state.remainingMs);
}

// ---------------------------------------------------------------------------
// Public actions — call these from the UI; they wrap the IPC layer
// with optimistic store updates so the UI stays snappy.
// ---------------------------------------------------------------------------

/** Pause / resume. Returns the engine events the shell observed. */
export async function togglePause(): Promise<void> {
  await pauseToggle();
  state.pendingCommandAck = true;
  // The engine also emits StateChanged → setEngineState → store stays
  // authoritative. We just un-set the ack flag after a short delay.
  setTimeout(() => {
    state.pendingCommandAck = false;
  }, 1_000);
}

/** Quick-log water via the IPC helper. */
export async function logWater(amountMl: number): Promise<void> {
  await ipcLogWater(amountMl);
  // Optimistic local update — the engine's CoreEvent::HydrationUpdated
  // will land on `core-event` within ~1 tick.
  setHydration({
    consumed_ml: state.todayHydrationMl + amountMl,
    goal_ml: state.todayWaterGoalMl,
  });
}

/** Persist a new `AppConfig`. Updates `state.config` immediately so
 *  the UI reflects the change; the backend engine re-arms live. */
export async function setConfig(cfg: AppConfig): Promise<void> {
  state.config = cfg;
  await ipcUpdateConfig(cfg);
}

/** Patch a single field (e.g. only `breaks.micro_interval_min`). */
export async function patchConfig(
  patch: Partial<AppConfig>,
): Promise<void> {
  const merged = mergeConfig(state.config, patch);
  await setConfig(merged);
}

/** Recursive shallow merge for the AppConfig tree. */
function mergeConfig(
  base: AppConfig,
  patch: Partial<AppConfig>,
): AppConfig {
  const out: AppConfig = { ...base };
  for (const key of Object.keys(patch) as Array<keyof AppConfig>) {
    const v = patch[key];
    if (v === undefined) continue;
    // Each top-level is itself an object → merge.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (out as any)[key] = { ...(base as any)[key], ...(v as any) };
  }
  return out;
}

/** Read the full `EngineSnapshot`. Called at boot + after a clear. */
export async function refreshSnapshot(): Promise<void> {
  const snap = await getEngineSnapshot();
  applySnapshot(snap);
}

/** Apply a freshly fetched snapshot to the store. */
export function applySnapshot(snap: EngineSnapshot): void {
  state.state = snap.state;
  state.config = snap.config;
  state.todayHydrationMl = snap.hydration.consumed_ml;
  state.todayWaterGoalMl = snap.hydration.goal_ml;
  state.today = snap.today;
  state.idleReliable = snap.idle_reliable;
}

/** Refresh today's aggregate (used by Settings → Clear). */
export async function refreshToday(): Promise<void> {
  const today = await getToday();
  state.today = today;
  state.todayHydrationMl = today.water_ml;
  state.todayWaterGoalMl = today.water_goal_ml;
}

/** Refresh recent sessions / hydration lists (Stats page). */
export async function refreshRecent(days: number = 7): Promise<void> {
  const [sessions, hydration] = await Promise.all([
    getRecentSessions(days),
    getRecentHydration(days),
  ]);
  state.recentSessions = sessions;
  state.recentHydration = hydration;
}

/** Wire `core-event` → `setEngineState` until the subscription
 *  function is called. Returns the unsubscribe handle. */
export async function startEventListener(): Promise<() => void> {
  const unlisten = await onCoreEvent(setEngineState);
  return unlisten;
}

/** Convenience: full snapshot + event subscription in one call. */
export async function bootstrap(): Promise<() => void> {
  await refreshSnapshot();
  // `refreshToday` after `refreshSnapshot` so any in-flight
  // persist-side updates that landed since the snapshot were
  // captured into the today mirror.
  await refreshToday();
  return startEventListener();
}

/** Settings → Data → Clear. */
export async function clearHistoryAction(): Promise<void> {
  await clearHistory();
  await refreshToday();
}

/** Settings → Data → Export. Returns the JSON string + metadata. */
export async function exportDataAction() {
  return exportData();
}

// ---------------------------------------------------------------------------
// Read-only accessors the dashboard uses to format values.
// ---------------------------------------------------------------------------

export const hydrationProgress = (): HydrationProgress => ({
  consumed_ml: state.todayHydrationMl,
  goal_ml: state.todayWaterGoalMl,
});

export const isBreakActive = (): boolean =>
  state.state.type === "pre_break" ||
  state.state.type === "micro_break" ||
  state.state.type === "rest_break";

export const isPaused = (): boolean => state.state.type === "paused";

export const currentBreakKind = (): BreakKind | null =>
  state.state.type === "pre_break" ||
  state.state.type === "micro_break" ||
  state.state.type === "rest_break" ||
  state.state.type === "postponed"
    ? // All four states carry a `kind` field at the top level.
      (state.state as Extract<SessionState, { kind: BreakKind }>).kind
    : null;

/** Set just the strictness field and persist. Used by the
 *  Settings → Strictness radio buttons. */
export async function setStrictness(s: Strictness): Promise<void> {
  state.config = { ...state.config, strictness: s };
  await ipcUpdateConfig(state.config);
}

/** Read the current strictness — used by the Dashboard's pill. */
export const strictness = (): Strictness => state.config.strictness;
