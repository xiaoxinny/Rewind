<script lang="ts">
  // Dashboard — primary surface: today's rollup + quick actions.
  // M6 replaces the M0 placeholder cards with a real summary.

  import {
    hydrationProgress,
    isPaused,
    logWater,
    remainingLabel,
    state,
    strictness,
    togglePause,
  } from "../lib/stores.svelte";

  const hydration = $derived(hydrationProgress());
  const ratio = $derived(
    hydration.goal_ml > 0
      ? Math.min(1, hydration.consumed_ml / hydration.goal_ml)
      : 0,
  );
  const ratioPct = $derived(`${(ratio * 100).toFixed(0)}%`);

  // Phase label for the hero countdown. Reads `state.state` so the
  // label always matches what the engine is doing, instead of
  // rendering a bare `m:ss` and forcing the user to guess what it
  // counts to (R3 from docs/ADVERSARIAL_UX_REPORT.md).
  const phaseLabel = $derived.by((): string => {
    switch (state.state.type) {
      case "focus":
        // While in focus, the engine's *next* event is the upcoming
        // micro break (the 20-20-20 rule) — that's what the
        // countdown is actually counting to.
        return "Next micro break in";
      case "pre_break":
        return state.state.kind === "rest"
          ? "Rest break starts in"
          : "Micro break starts in";
      case "micro_break":
        return "Look away for";
      case "rest_break":
        return "Rest for";
      case "postponed":
        return "Break postponed — back in";
      case "paused":
        return "Paused — next break in";
    }
  });

  // Countdown text rendered as the hero. We use `state.remainingMs`
  // (driven by CoreEvent::Tick) instead of `state.trayStatus.tooltip_line`
  // so the value is stable while the engine is still bootstrapping
  // and the tooltip line is the literal "Loading…".
  function formatMs(ms: number): string {
    const total = Math.max(0, Math.round(ms / 1_000));
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }
  const countdownText = $derived(formatMs(state.remainingMs));

  // Has the engine pushed its first Tick event yet? The initial
  // tooltip line is literally "Loading…" (stores.svelte.ts:91) and
  // is overwritten with `formatRemaining(...)` on every tick
  // (stores.svelte.ts:130). Until that first tick lands, we show a
  // small "calculating…" caption in place of the bare "Loading…".
  const bootstrapping = $derived(
    state.trayStatus.tooltip_line === "Loading…",
  );
</script>

<section class="dashboard">
  <article class="hero">
    <h2>Today</h2>
    <p class="hero-label">{phaseLabel}</p>
    {#if bootstrapping}
      <p class="hero-line placeholder" aria-live="polite">
        <span class="calculating">calculating…</span>
      </p>
    {:else}
      <p class="hero-line">{countdownText}</p>
    {/if}
    <p class="hero-sub">
      {ratioPct} of water goal · {state.today.breaks_taken} breaks taken ·
      {state.today.posture_prompts} posture nudges
    </p>
    {#if isPaused()}
      <p class="paused-banner">⏸ Paused — {state.config.idle.enabled ? "idle detected" : "manual pause"}</p>
    {/if}
  </article>

  <section class="quick-actions">
    <button
      type="button"
      class="primary"
      onclick={() => togglePause()}
      aria-label={isPaused() ? "Resume" : "Pause"}
    >
      {isPaused() ? "Resume" : "Pause"}
    </button>
    <button
      type="button"
      onclick={() => logWater(state.config.hydration.glassMl)}
    >
      + Log water ({state.config.hydration.glassMl} ml)
    </button>
  </section>

  <section class="grid">
    <article>
      <h3>Hydration</h3>
      <p class="big">{hydration.consumed_ml} / {hydration.goal_ml} ml</p>
      <div class="bar" aria-hidden="true">
        <div class="bar-fill" style:width={ratioPct}></div>
      </div>
    </article>
    <article>
      <h3>Breaks</h3>
      <p class="big">
        {state.today.breaks_taken} <span class="dim">taken</span>
      </p>
      <p class="dim">{state.today.breaks_skipped} skipped</p>
    </article>
    <article>
      <h3>Posture</h3>
      <p class="big">{state.today.posture_prompts}</p>
      <p class="dim">nudges today</p>
    </article>
    <article>
      <h3>Strictness</h3>
      <p class="big">{strictness()}</p>
      <p class="dim">change in Settings</p>
    </article>
  </section>

  <section class="next">
    <h3>Next event</h3>
    <p class="muted">{remainingLabel()}</p>
  </section>
</section>

<style>
  .dashboard {
    display: grid;
    gap: 1rem;
  }

  .hero {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 12px;
    padding: 1.25rem 1.5rem;
  }

  .hero h2 {
    margin: 0 0 0.25rem;
    font-size: 1.1rem;
    color: #8b949e;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .hero-label {
    margin: 0.5rem 0 0.25rem;
    font-size: 1rem;
    font-weight: 500;
    color: #c9d1d9;
    letter-spacing: 0.01em;
  }

  .hero-line {
    margin: 0 0 0.5rem;
    font-size: 2.25rem;
    font-weight: 700;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }

  .hero-line.placeholder {
    margin: 0 0 0.5rem;
    min-height: 2.5rem;
    display: flex;
    align-items: baseline;
  }

  .hero-line .calculating {
    font-size: 0.95rem;
    font-weight: 400;
    font-style: italic;
    color: #8b949e;
  }

  .hero-sub {
    margin: 0;
    color: #8b949e;
    font-size: 0.9rem;
  }

  .paused-banner {
    margin: 0.75rem 0 0;
    background: #21262d;
    border: 1px solid #f85149;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 0.9rem;
    color: #ffa198;
  }

  .quick-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .quick-actions button {
    appearance: none;
    background: #21262d;
    border: 1px solid #30363d;
    border-radius: 6px;
    padding: 0.5rem 0.875rem;
    cursor: pointer;
  }

  .quick-actions button.primary {
    background: #238636;
    border-color: #2ea043;
  }

  .quick-actions button:hover {
    border-color: #58a6ff;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  article {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 0.875rem 1rem;
  }

  article h3 {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
    color: #8b949e;
  }

  .big {
    margin: 0;
    font-size: 1.4rem;
    font-weight: 600;
  }

  .dim {
    color: #8b949e;
    font-size: 0.85rem;
  }

  .bar {
    height: 6px;
    margin-top: 0.5rem;
    background: #21262d;
    border-radius: 999px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: #1f6feb;
    transition: width 200ms ease-in-out;
  }

  .next {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 0.875rem 1rem;
  }

  .next h3 {
    margin: 0 0 0.25rem;
    font-size: 0.95rem;
    color: #8b949e;
  }

  .muted {
    margin: 0;
    color: #8b949e;
    font-family: ui-monospace, "SF Mono", Consolas, monospace;
    font-size: 0.95rem;
  }
</style>
