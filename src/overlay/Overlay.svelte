<script lang="ts">
  // Break overlay shell. Replaces the M0 stub ("lands in M3") which
  // never got replaced when the rest of M3/M5/M6 shipped, leaving the
  // four CSS-only exercise components orphaned. See the implementation
  // plan §3 (UI tree) and §11 (strict-mode kill-switch invariant).
  //
  // The overlay window receives events from the Rust runtime in two
  // places:
  //   1. `core-event` (global, broadcasts every CoreEvent including
  //      ShowBreak + DismissBreak) — this is the canonical signal.
  //   2. `show-exercise` (window-scoped, emitted by runtime.rs alongside
  //      the ShowBreak side-effect). We listen for it as well so the
  //      overlay stays in sync even if the global bus is ever filtered.
  //
  // The wire format is the CoreEventDto — see src-tauri/src/ipc.rs and
  // src/lib/types.ts. Specifically the ShowBreak variant carries
  // `presentation_strict: boolean` (NOT `presentation: "Gentle" | "Strict"`
  // — the DTO collapses the BreakPresentation enum to a bool).
  //
  // Round-robin rotation is persisted to tauri-plugin-store so it
  // survives app restart (first break after install always picks
  // Palming, then NearFar, Blink, FigureEight, loop).

  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Store } from "@tauri-apps/plugin-store";

  import Palming from "./exercises/Palming.svelte";
  import NearFar from "./exercises/NearFar.svelte";
  import Blink from "./exercises/Blink.svelte";
  import FigureEight from "./exercises/FigureEight.svelte";

  import type { BreakKind, CoreEvent } from "../lib/types";

  // -------------------------------------------------------------------------
  // Round-robin order. Insertion order == rotation order. Palming first.
  // -------------------------------------------------------------------------

  type ExerciseId = "palming" | "nearfar" | "blink" | "figureeight";
  type ExerciseComponent =
    | typeof Palming
    | typeof NearFar
    | typeof Blink
    | typeof FigureEight;

  interface ExerciseDef {
    id: ExerciseId;
    name: string;
    component: ExerciseComponent;
    /** Default duration when the engine doesn't pass one through. */
    defaultMs: number;
  }

  const EXERCISES: readonly ExerciseDef[] = [
    { id: "palming",     name: "Palming",          component: Palming,     defaultMs: 30_000 },
    { id: "nearfar",     name: "Near ⇄ Far focus", component: NearFar,     defaultMs: 30_000 },
    { id: "blink",       name: "Blink",            component: Blink,       defaultMs: 20_000 },
    { id: "figureeight", name: "Figure-eight",     component: FigureEight, defaultMs: 30_000 },
  ] as const;

  const STORE_PATH = "overlay_state.json";
  const LAST_SHOWN_KEY = "last_shown_exercise";

  /** Pick the next exercise in rotation. `prev === ""` (unknown)
   *  returns the first exercise — guarantees Palming on first break. */
  function nextExercise(prev: string): ExerciseDef {
    if (!prev) return EXERCISES[0];
    const idx = EXERCISES.findIndex((e) => e.id === prev);
    if (idx === -1) return EXERCISES[0];
    return EXERCISES[(idx + 1) % EXERCISES.length];
  }

  // -------------------------------------------------------------------------
  // Reactive state.
  // -------------------------------------------------------------------------

  /** Visible iff a ShowBreak is currently being presented. */
  let visible = $state(false);
  /** Whether the current break is presented in strict mode (kill
   *  switch is the only escape — no buttons per plan §11). */
  let strict = $state(false);
  /** Which break kind the engine just announced. Drives the copy. */
  let kind: BreakKind = $state("micro");
  /** The exercise we'll host for the current break. `null` until the
   *  first rotation has resolved (the persisted `last_shown_exercise`
   *  read may take one async tick on cold start). */
  let exercise: ExerciseDef | null = $state(null);
  /** Remaining countdown in ms. Drives the SVG stroke-dashoffset. */
  let remainingMs = $state(0);
  /** Total duration in ms. Drives the SVG circumference reference. */
  let totalMs = $state(1);
  /** Disabled-button guard: when `remainingMs <= 0`, late clicks must
   *  not re-fire IPC calls (the runtime has already started a new
   *  tick by then). */
  let expired = $state(false);

  // -------------------------------------------------------------------------
  // Derived view-model.
  // -------------------------------------------------------------------------

  /** mm:ss label. Pure. */
  const remainingLabel = $derived.by((): string => {
    const total = Math.max(0, Math.round(remainingMs / 1_000));
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  });

  /** SVG geometry. Circle r=88, circumference = 2π·88.
   *  viewBox 0 0 200 200; circle centred at 100,100. */
  const RADIUS = 88;
  const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

  /** stroke-dashoffset: starts at 0 (full circle drawn) and animates
   *  to `CIRCUMFERENCE` (circle emptied) as time runs out. */
  const dashOffset = $derived.by((): number => {
    if (totalMs <= 0) return CIRCUMFERENCE;
    const progress = Math.min(1, Math.max(0, 1 - remainingMs / totalMs));
    return progress * CIRCUMFERENCE;
  });

  /** Strict-mode backdrop is heavier than gentle. */
  const backdropClass = $derived(strict ? "backdrop strict" : "backdrop gentle");

  /** Headline copy per kind. Micro = 20-20-20; rest = step away. */
  const headline = $derived(
    kind === "micro"
      ? "Take a 20-second break"
      : "Step away from the screen",
  );

  // -------------------------------------------------------------------------
  // Persisted round-robin state — read once on mount, written on every
  // ShowBreak. Defensive: if the store file is missing or the read
  // throws (first ever break, before any store exists), fall back to
  // the first exercise so the UI still mounts.
  // -------------------------------------------------------------------------

  /** In-memory mirror of the last-shown exercise id. The disk read on
   *  mount populates this; subsequent ShowBreak events advance it. */
  let lastShown: string = "";

  async function loadInitialRotation(): Promise<void> {
    try {
      const store = await Store.load(STORE_PATH);
      const value = await store.get<string>(LAST_SHOWN_KEY);
      lastShown = typeof value === "string" ? value : "";
    } catch (e) {
      // First-ever break before any store exists: this is the expected
      // cold-start path. lastShown stays "" → nextExercise returns
      // EXERCISES[0] (Palming). Log only as a soft warning.
      console.warn("Rewind overlay: rotation store read failed, using defaults", e);
      lastShown = "";
    }
  }

  /** Persist the just-shown exercise id. Fire-and-forget — UI never
   *  blocks on disk I/O. */
  async function persistLastShown(id: string): Promise<void> {
    try {
      const store = await Store.load(STORE_PATH);
      await store.set(LAST_SHOWN_KEY, id);
      await store.save();
    } catch (e) {
      console.warn("Rewind overlay: rotation store write failed", e);
    }
  }

  // -------------------------------------------------------------------------
  // Countdown. Drives the SVG ring. 50 ms tick (per spec); the engine
  // ticks at 1 Hz and broadcasts remaining_ms on `tick`, but the
  // overlay window doesn't see those (the engine emits to the global
  // bus only). We approximate by anchoring to `totalMs` at start and
  // ticking down locally — this matches the rest of the UI.
  // -------------------------------------------------------------------------

  let tickHandle: ReturnType<typeof setInterval> | null = null;

  function clearTick(): void {
    if (tickHandle !== null) {
      clearInterval(tickHandle);
      tickHandle = null;
    }
  }

  function startCountdown(durationMs: number): void {
    clearTick();
    totalMs = Math.max(1, durationMs);
    remainingMs = totalMs;
    expired = false;
    tickHandle = setInterval(() => {
      remainingMs -= 50;
      if (remainingMs <= 0) {
        remainingMs = 0;
        expired = true;
        clearTick();
        // Countdown hit zero → end the break. The runtime will also
        // emit DismissBreak via core-event (in response to the engine
        // seeing the break phase complete); our handler will clean up
        // state at that point. We invoke `skip_break` so the engine
        // records a "skipped (timeout)" outcome rather than waiting
        // for the engine to detect the timeout itself — better UX.
        void invoke("skip_break", { now: { unixMs: Date.now() } }).catch(
          (e) => console.warn("Rewind overlay: auto-dismiss invoke failed", e),
        );
      }
    }, 50);
  }

  // -------------------------------------------------------------------------
  // ShowBreak handler. Two callers can trigger this:
  //   * the global `core-event` channel carrying ShowBreak
  //   * the window-scoped `show-exercise` event emitted by runtime.rs
  // We dedupe with a single source of truth: this function. Whichever
  // listener fires first sets the rotation; the other listener is a
  // no-op while `visible` is already true.
  // -------------------------------------------------------------------------

  let pendingShowBreak: number | null = null;

  function applyShowBreak(args: {
    kind: BreakKind;
    strictMode: boolean;
  }): void {
    // Dedup rapid double-fire (e.g. user triggers manual break right
    // after the scheduled one). Defer to the next animation frame
    // so back-to-back events coalesce.
    if (pendingShowBreak !== null) {
      cancelAnimationFrame(pendingShowBreak);
    }
    pendingShowBreak = requestAnimationFrame(() => {
      pendingShowBreak = null;
      const next = nextExercise(lastShown);
      lastShown = next.id;
      exercise = next;
      kind = args.kind;
      strict = args.strictMode;
      visible = true;
      startCountdown(next.defaultMs);
      void persistLastShown(next.id);
    });
  }

  function applyDismiss(): void {
    pendingShowBreak = null;
    clearTick();
    visible = false;
    expired = false;
    remainingMs = 0;
    totalMs = 1;
    // Don't reset `exercise` / `kind` / `strict` — preserving them
    // means a re-mount of the exercise component on the next show
    // re-fires its `$effect` with a fresh prop pass.
  }

  // -------------------------------------------------------------------------
  // Listeners.
  // -------------------------------------------------------------------------

  let unlistenCore: UnlistenFn | null = null;
  let unlistenWindow: UnlistenFn | null = null;

  onMount(() => {
    void loadInitialRotation();

    void listen<CoreEvent>("core-event", (event) => {
      const ev = event.payload;
      switch (ev.type) {
        case "show_break":
          applyShowBreak({
            kind: ev.kind,
            strictMode: ev.presentation_strict,
          });
          break;
        case "dismiss_break":
          applyDismiss();
          break;
        default:
          // Other CoreEvents are dashboard-side concerns.
          break;
      }
    }).then((u) => {
      unlistenCore = u;
    });

    // Window-scoped "show-exercise" event from runtime.rs. Carries
    // the same payload shape but with `presentation` as the full
    // BreakPresentation (we only care about strictness).
    type ShowExercisePayload = {
      kind: string;
      presentation: string;
      exerciseId: string | null;
    };
    void listen<ShowExercisePayload>("show-exercise", (event) => {
      const p = event.payload;
      const mappedKind: BreakKind = p.kind === "rest" ? "rest" : "micro";
      applyShowBreak({
        kind: mappedKind,
        strictMode: p.presentation === "Strict",
      });
    }).then((u) => {
      unlistenWindow = u;
    });
  });

  onDestroy(() => {
    pendingShowBreak = null;
    clearTick();
    if (unlistenCore) {
      unlistenCore();
      unlistenCore = null;
    }
    if (unlistenWindow) {
      unlistenWindow();
      unlistenWindow = null;
    }
  });

  // -------------------------------------------------------------------------
  // Button handlers — gentle mode only. Strict mode renders no
  // buttons (plan §11 invariant: the kill switch is the only escape).
  // "Done" and "Skip" both route through `skip_break`: the engine
  // treats a manual exit the same way regardless of which button the
  // user pressed; the outcome distinction lives in the persisted
  // BreakRecord (M3+ storage layer).
  // -------------------------------------------------------------------------

  function handleSkip(): void {
    if (expired || !visible) return;
    expired = true;
    void invoke("skip_break", { now: { unixMs: Date.now() } }).catch(
      (e) => console.warn("Rewind overlay: skip_break failed", e),
    );
    // Don't optimistically hide — wait for the runtime's DismissBreak
    // event so the engine confirms the transition.
  }

  function handlePostpone(): void {
    if (expired || !visible) return;
    expired = true;
    void invoke("postpone_break", { now: { unixMs: Date.now() } }).catch(
      (e) => console.warn("Rewind overlay: postpone_break failed", e),
    );
  }

  function handleDone(): void {
    if (expired || !visible) return;
    expired = true;
    void invoke("skip_break", { now: { unixMs: Date.now() } }).catch(
      (e) => console.warn("Rewind overlay: skip_break failed", e),
    );
  }
</script>

{#if visible}
  <div class={backdropClass} role="dialog" aria-modal="true" aria-label="Break">
    <div class="card">
      {#if strict}
        <span class="strict-badge" aria-hidden="true">STRICT</span>
      {/if}

      <svg
        class="ring"
        viewBox="0 0 200 200"
        aria-hidden="true"
      >
        <circle
          class="ring-track"
          cx="100"
          cy="100"
          r={RADIUS}
          fill="none"
          stroke-width="6"
        />
        <circle
          class="ring-progress"
          cx="100"
          cy="100"
          r={RADIUS}
          fill="none"
          stroke-width="6"
          stroke-linecap="round"
          stroke-dasharray={CIRCUMFERENCE}
          stroke-dashoffset={dashOffset}
          transform="rotate(-90 100 100)"
        />
        <text
          class="ring-text"
          x="100"
          y="100"
          text-anchor="middle"
          dominant-baseline="central"
        >{remainingLabel}</text>
      </svg>

      <h1>{headline}</h1>

      {#if exercise}
        <p class="exercise-name">{exercise.name}</p>
        <div class="exercise-host">
          <exercise.component durationMs={totalMs} />
        </div>
      {:else}
        <p class="exercise-name muted">Loading…</p>
      {/if}

      {#if !strict}
        <div class="actions">
          <button
            type="button"
            class="btn btn-ghost"
            onclick={handleSkip}
            disabled={expired}
          >Skip</button>
          <button
            type="button"
            class="btn btn-secondary"
            onclick={handlePostpone}
            disabled={expired}
          >Postpone 5 min</button>
          <button
            type="button"
            class="btn btn-primary"
            onclick={handleDone}
            disabled={expired}
          >Done</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    background: transparent;
    color: #e6edf3;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    z-index: 1;
  }

  .backdrop.gentle {
    background: rgba(0, 0, 0, 0.85);
  }

  .backdrop.strict {
    background: rgba(0, 0, 0, 0.95);
  }

  .card {
    position: relative;
    background: rgba(14, 17, 22, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 1.75rem 2rem 1.5rem;
    max-width: 520px;
    width: min(92vw, 520px);
    text-align: center;
    box-shadow: 0 12px 48px rgba(0, 0, 0, 0.45);
  }

  .strict-badge {
    position: absolute;
    top: 10px;
    right: 12px;
    padding: 2px 8px;
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    border-radius: 4px;
    background: rgba(248, 81, 73, 0.15);
    color: #f85149;
    border: 1px solid rgba(248, 81, 73, 0.4);
    pointer-events: none;
    user-select: none;
  }

  .ring {
    width: 200px;
    height: 200px;
    display: block;
    margin: 0 auto 0.5rem;
  }

  .ring-track {
    stroke: rgba(255, 255, 255, 0.08);
  }

  .ring-progress {
    stroke: #58a6ff;
    transition: stroke-dashoffset 80ms linear;
  }

  .ring-text {
    fill: #e6edf3;
    font-family: ui-monospace, "SF Mono", Consolas, monospace;
    font-size: 28px;
    font-weight: 500;
  }

  h1 {
    margin: 0.25rem 0 0.25rem;
    font-size: 1.25rem;
    font-weight: 600;
  }

  .exercise-name {
    margin: 0 0 0.5rem;
    color: #c9d1d9;
    font-size: 0.95rem;
  }

  .exercise-name.muted {
    color: #8b949e;
  }

  .exercise-host {
    /* Constrain the host so each exercise SVG stays compact inside
       the card instead of overflowing. */
    height: 220px;
    display: grid;
    place-items: center;
    margin: 0 0 0.5rem;
    overflow: hidden;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.35);
  }

  /* Override each exercise component's `height: 100vh` so it lives
     inside the host cell rather than blowing out the card. */
  .exercise-host :global(main) {
    height: 100% !important;
    padding: 0.75rem !important;
  }
  .exercise-host :global(svg) {
    max-width: 180px;
    max-height: 180px;
    width: auto !important;
    height: auto !important;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .btn {
    appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: #e6edf3;
    padding: 0.5rem 0.95rem;
    border-radius: 6px;
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn:active:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #238636;
    border-color: rgba(35, 134, 54, 0.6);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2ea043;
    border-color: rgba(46, 160, 67, 0.7);
  }

  .btn-secondary {
    background: rgba(88, 166, 255, 0.12);
    border-color: rgba(88, 166, 255, 0.35);
    color: #c9d1d9;
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(88, 166, 255, 0.2);
    border-color: rgba(88, 166, 255, 0.55);
  }

  .btn-ghost {
    background: transparent;
    color: #8b949e;
  }

  .btn-ghost:hover:not(:disabled) {
    color: #e6edf3;
  }
</style>