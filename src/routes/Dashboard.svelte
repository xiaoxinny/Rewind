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
  // counts to.
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
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-hero);
    padding: 1.25rem 1.5rem;
    box-shadow: var(--shadow-hero);
  }

  .hero h2 {
    margin: 0 0 0.25rem;
    font-size: 1.1rem;
    color: var(--text);
    font-family: var(--font-display);
    /* DESIGN_LANGUAGE.md §10.1 drops the ALL-CAPS transform — "Today"
       stays sentence case. The `<h3>` tiles are already not uppercase. */
  }

  .hero-label {
    margin: 0.5rem 0 0.25rem;
    font-size: 1rem;
    font-weight: 500;
    color: var(--text);
  }

  .hero-line {
    margin: 0 0 0.5rem;
    font-size: 2.25rem;
    font-weight: 700;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
    font-family: var(--font-mono);
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
    color: var(--text-muted);
  }

  .hero-sub {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  /* Paused banner (DESIGN_LANGUAGE.md §6.7 — `<Banner variant="paused">`).
     In v0.1 the Banner component is not yet extracted (Track 3 PR 3);
     the inline shape here matches the language spec: --ink-3 surface,
     --text-muted left-border, --text-2 copy. The ⏸ emoji is removed
     in PR 3; v0.1.1 ships the icon-replace. */
  .paused-banner {
    margin: 0.75rem 0 0;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--text-muted);
    border-radius: var(--radius-card);
    padding: 0.5rem 0.75rem;
    font-size: 0.9rem;
    color: var(--text-2);
  }

  .quick-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .quick-actions button {
    appearance: none;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: 0.5rem 0.875rem;
    cursor: pointer;
    color: var(--text-2);
    transition: border-color var(--dur-small) var(--ease),
      background var(--dur-small) var(--ease);
  }

  .quick-actions button.primary {
    /* Per §6.3: one primary button per screen. Pause / Resume swaps
       into the primary slot; the label and aria follow. */
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }

  .quick-actions button:hover {
    border-color: var(--accent);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0.75rem;
  }

  article {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: 0.875rem 1rem;
  }

  article h3 {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
    color: var(--text-muted);
    font-family: var(--font-body);
  }

  .big {
    margin: 0;
    font-size: 1.4rem;
    font-weight: 600;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .dim {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .bar {
    height: 6px;
    margin-top: 0.5rem;
    background: var(--ink-3);
    border-radius: 999px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: var(--accent);
    transition: width var(--dur-small) var(--ease);
  }

  .next {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: 0.875rem 1rem;
  }

  .next h3 {
    margin: 0 0 0.25rem;
    font-size: 0.95rem;
    color: var(--text-muted);
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 0.95rem;
  }
</style>
