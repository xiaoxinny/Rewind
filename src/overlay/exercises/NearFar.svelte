<script lang="ts">
  // Near/far — 5 s near focus, 5 s far focus. CSS animates a focus
  // target that grows / shrinks.
  let { durationMs = 30_000 }: { durationMs?: number } = $props();

  let elapsed = $state(0);
  let phaseMs = $state(0); // 0..5000
  const phases = ["Near focus", "Far focus"] as const;
  let phaseIdx = $derived(Math.floor(phaseMs / 2500) % 2); // 0 = near, 1 = far
  let interval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (interval) clearInterval(interval);
    elapsed = 0;
    phaseMs = 0;
    interval = setInterval(() => {
      elapsed += 100;
      phaseMs = (phaseMs + 100) % 10_000;
      if (elapsed >= durationMs && interval) {
        clearInterval(interval);
        interval = null;
      }
    }, 100);
    return () => {
      if (interval) clearInterval(interval);
    };
  });
</script>

<main>
  <h2>Near ⇄ Far focus shift</h2>
  <p class="muted">
    Hold a finger ~20 cm from your face. Focus on it, then look past
    it to the far wall. Alternate.
  </p>
  <div class="target" class:far={phaseIdx === 1} aria-hidden="true">
    <div class="dot"></div>
  </div>
  <p class="caption">{phases[phaseIdx]}</p>
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
  .target {
    /* The blue background uses --micro-break (the "look at this"
       hue family. The dot fill below is the exercise's
       inline SVG carveout. */
    width: 48px;
    height: 48px;
    border-radius: 12px;
    background: var(--accent-soft);
    border: 1px solid var(--accent);
    display: grid;
    place-items: center;
    transition: width 800ms var(--ease), height 800ms var(--ease);
  }
  .target.far {
    width: 240px;
    height: 240px;
    border-radius: 50%;
  }
  /* .dot is a CSS-only div (no inline SVG in NearFar), so this
     uses --micro-break: "the animated rings / dots
     currently use #58a6ff directly. Switch to var(--micro-break)". */
  .dot {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--micro-break);
  }
  .caption {
    margin: 1rem 0 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
</style>
