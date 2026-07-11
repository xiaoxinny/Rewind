<script lang="ts">
  // Blink — paced full-blink prompt every ~3 s. The "screen-induced
  // under-blinking" effect (people blink ~50% less at screens) is
  // what this counters.
  let { durationMs = 20_000 }: { durationMs?: number } = $props();

  const cycle = 3000; // ms per blink prompt
  let elapsed = $state(0);
  let lastBlink = $state(0);
  let interval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (interval) clearInterval(interval);
    elapsed = 0;
    lastBlink = 0;
    interval = setInterval(() => {
      elapsed += 100;
      if (elapsed - lastBlink >= cycle) {
        lastBlink = elapsed;
      }
      if (elapsed >= durationMs && interval) {
        clearInterval(interval);
        interval = null;
      }
    }, 100);
    return () => {
      if (interval) clearInterval(interval);
    };
  });

  const sinceBlink = $derived(elapsed - lastBlink);
  const blinkingNow = $derived(sinceBlink < 400); // 400 ms blink
  const progress = $derived(Math.min(1, sinceBlink / cycle));
</script>

<main>
  <h2>Blink</h2>
  <p class="muted">Blink fully (close + open) when the dot flashes.</p>
  <div class="eye" class:blinking={blinkingNow} aria-hidden="true">
    <div class="lid"></div>
    <div class="pupil"></div>
  </div>
  <div class="bar">
    <div class="bar-fill" style:width={`${progress * 100}%`}></div>
  </div>
</main>

<style>
  main {
    height: 100vh;
    display: grid;
    place-items: center;
    text-align: center;
    padding: 2rem;
    color: var(--text);
    font-family: var(--font-body);
  }
  h2 {
    margin: 0 0 0.5rem;
    font-family: var(--font-display);
  }
  .muted {
    color: var(--text-muted);
    margin: 0 0 1rem;
  }
  /* CSS-only; .eye / .pupil / .bar-fill swap to the micro-break hue
     family per §10.5. */
  .eye {
    width: 220px;
    height: 110px;
    border: 4px solid var(--micro-break);
    border-radius: 50%;
    display: grid;
    place-items: center;
    position: relative;
    background: var(--ink);
    overflow: hidden;
  }
  .lid {
    position: absolute;
    inset: 0;
    background: var(--ink-2);
    transform-origin: center;
    transform: scaleY(0);
    transition: transform 120ms var(--ease);
  }
  .eye.blinking .lid {
    transform: scaleY(0.95);
    transition-duration: 80ms;
  }
  .pupil {
    width: 24px;
    height: 24px;
    background: var(--micro-break);
    border-radius: 50%;
    z-index: 1;
  }
  .bar {
    margin-top: 1.25rem;
    width: 220px;
    height: 4px;
    background: var(--ink-3);
    border-radius: 999px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    background: var(--micro-break);
    transition: width 100ms linear;
  }
</style>
