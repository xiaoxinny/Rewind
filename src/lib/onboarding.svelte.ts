// Track 2 — onboarding flow's state machine + tauri-plugin-store
// persistence.
//
// Persistence model: two keys in `app_state.json` (a separate JSON
// store file from the AppConfig store at `config.json`):
//
//   * `first_run_complete`: boolean. Once true, the wizard never
//     auto-shows again. The user can still restart it from the
//     Dashboard `?` button or Settings → About "Restart tour".
//   * `onboarding_step`:    "welcome" | "evidence" | "intervals"
//                           | "enable" | null. Only relevant while
//                           `first_run_complete === false`; if the user
//                           quits mid-flow the next launch resumes
//                           them on the same screen.
//
// The intervals form's *draft* is NOT persisted — only the final
// applied AppConfig is (via `setConfig()` from `stores.svelte.ts`).
// That way the user can fiddle without committing.
//
// tauri-plugin-store access is **frontend-direct**: same pattern the
// overlay window uses (`Store.load(STORE_PATH)` + `get`/`set`/`save`).
// See `src/overlay/Overlay.svelte:144-167` for the reference. The
// Rust side already has the store plugin wired in `src-tauri/src/
// lib.rs:66`, so no IPC changes are required.
//
// The reactive rune and the imported `state` from `stores.svelte.ts`
// (which the Settings page aliases as `mirror`) share a name
// collision in Svelte 5; nothing in this file is a `$state` rune
// declaration, so no aliasing is needed here.

import { Store } from "@tauri-apps/plugin-store";
import type { AppConfig } from "./types";
import { DEFAULT_CONFIG } from "./types";
import { setConfig } from "./stores.svelte";

const STORE_PATH = "app_state.json";
const FIRST_RUN_KEY = "first_run_complete";
const STEP_KEY = "onboarding_step";

export type OnboardingStep = "welcome" | "evidence" | "intervals" | "enable";
type StoredStep = OnboardingStep | null;

// Order of the flow — `next()` advances through it. Used by the
// component buttons and tests can inspect it.
export const STEP_ORDER: readonly OnboardingStep[] = [
  "welcome",
  "evidence",
  "intervals",
  "enable",
] as const;

interface OnboardingStateShape {
  /** Current step; `null` means the flow is not active. */
  step: OnboardingStep | null;
  /** Whether the flow should be shown automatically. Resolves to
   *  `false` once `first_run_complete` is true. Resolves to
   *  `true` on first launch (no record yet). */
  firstRun: boolean;
  /** Has `loadInitial()` resolved? Until it has, the App-level router
   *  defers the auto-route to Onboarding — we don't know yet. */
  loaded: boolean;
  /** Local-only edit-buffer for the Intervals screen. NOT persisted;
   *  the user can fiddle and quit, next launch starts fresh on
   *  the Intervals screen with defaults re-seeded from
   *  `AppConfig::default()`. */
  draft: AppConfig;
}

export const onboardingState = $state<OnboardingStateShape>({
  step: null,
  firstRun: false,
  loaded: false,
  draft: structuredClone(DEFAULT_CONFIG) as AppConfig,
});

// ---------------------------------------------------------------------------
// Load / save helpers — fire-and-forget write path; fail-soft reads.
// ---------------------------------------------------------------------------

/** Open the JSON store once. Always returns the same handle; Tauri's
 *  plugin caches by path. */
async function openStore() {
  return Store.load(STORE_PATH);
}

/** Load both persisted keys. On a first launch neither key exists;
 *  treat as `{ firstRun: false → true, step: "welcome" }`. */
export async function loadInitial(): Promise<void> {
  try {
    const store = await openStore();
    const completed = await store.get<boolean>(FIRST_RUN_KEY);
    const step = (await store.get<StoredStep>(STEP_KEY)) ?? null;
    onboardingState.firstRun = completed !== true;
    onboardingState.step = onboardingState.firstRun ? step ?? "welcome" : null;
    // Re-seed draft from defaults each visit; the user is free to
    // adjust and the change is local until they hit "Apply".
    onboardingState.draft = structuredClone(DEFAULT_CONFIG) as AppConfig;
  } catch (e) {
    // First-ever launch, before the store file exists. This is the
    // expected cold-start path. Log only as a soft warning.
    console.warn("Rewind onboarding: app_state.json read failed, starting wizard", e);
    onboardingState.firstRun = true;
    onboardingState.step = "welcome";
    onboardingState.draft = structuredClone(DEFAULT_CONFIG) as AppConfig;
  } finally {
    onboardingState.loaded = true;
  }
}

/** Persist the just-stepped-into screen. Best-effort: a disk failure
 *  here means the user might resume at the previous step next launch,
 *  which is recoverable. */
export async function advance(next: OnboardingStep): Promise<void> {
  onboardingState.step = next;
  try {
    const store = await openStore();
    await store.set(STEP_KEY, next);
    await store.save();
  } catch (e) {
    console.warn("Rewind onboarding: failed to persist step", e);
  }
}

/** Patch the local Intervals draft. Pure in-memory; nothing hits the
 *  store or the Rust engine until the user clicks Apply. */
export function setDraft(patch: Partial<AppConfig>): void {
  // Shallow merge at top-level keys; per-section merge keeps the
  // existing fields the user hasn't touched.
  const merged: AppConfig = { ...onboardingState.draft };
  for (const key of Object.keys(patch) as Array<keyof AppConfig>) {
    const v = patch[key];
    if (v === undefined) continue;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (merged as any)[key] = { ...(onboardingState.draft as any)[key], ...(v as any) };
  }
  onboardingState.draft = merged;
}

/** Apply the Intervals draft to the Rust engine (which persists via
 *  `update_config`). Only the user-clicked Apply path calls this;
 *  the skip path never changes the config. */
export async function apply(): Promise<void> {
  await setConfig(onboardingState.draft);
}

/** Finish — flip `first_run_complete=true` and exit the flow to the
 *  Dashboard. Doesn't change the config (the user either Applied
 *  in step 3, or chose Defaults). */
export async function complete(): Promise<void> {
  onboardingState.step = null;
  onboardingState.firstRun = false;
  try {
    const store = await openStore();
    await store.set(FIRST_RUN_KEY, true);
    await store.set(STEP_KEY, null);
    await store.save();
  } catch (e) {
    console.warn("Rewind onboarding: failed to persist completion", e);
  }
}

/** Skip — same as complete but no `Apply` happened, so the Rust
 *  engine keeps whatever AppConfig was on disk (likely `default()`).
 *  Sets `first_run_complete=true` and routes out. */
export async function skip(): Promise<void> {
  onboardingState.step = null;
  onboardingState.firstRun = false;
  try {
    const store = await openStore();
    await store.set(FIRST_RUN_KEY, true);
    await store.set(STEP_KEY, null);
    await store.save();
  } catch (e) {
    console.warn("Rewind onboarding: failed to persist skip", e);
  }
}

/** Restart — called from the Dashboard `?` button and from the
 *  Settings → About "Restart tour" button. Re-shows step 1 and
 *  clears the persisted `first_run_complete` flag so the next launch
 *  will auto-show again. NEVER changes the AppConfig. */
export async function restart(): Promise<void> {
  onboardingState.step = "welcome";
  onboardingState.firstRun = true;
  onboardingState.draft = structuredClone(DEFAULT_CONFIG) as AppConfig;
  try {
    const store = await openStore();
    await store.set(FIRST_RUN_KEY, false);
    await store.set(STEP_KEY, "welcome");
    await store.save();
  } catch (e) {
    console.warn("Rewind onboarding: failed to persist restart", e);
  }
}

/** Resolves the next step in the wizard order. Convenience for the
 *  component's "Next:" buttons. */
export function nextFrom(current: OnboardingStep): OnboardingStep | null {
  const idx = STEP_ORDER.indexOf(current);
  if (idx < 0 || idx >= STEP_ORDER.length - 1) return null;
  return STEP_ORDER[idx + 1];
}

/** Resolves the previous step (used by `Back`). */
export function prevFrom(current: OnboardingStep): OnboardingStep | null {
  const idx = STEP_ORDER.indexOf(current);
  if (idx <= 0) return null;
  return STEP_ORDER[idx - 1];
}
