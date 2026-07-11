<script lang="ts">
  // Palming — cover closed eyes with palms (M4 research). The dot
  // pulses to a slow breath-pacing tempo (4 s in / 4 s out).
  let { durationMs = 30_000 }: { durationMs?: number } = $props();
  let elapsed = $state(0);
  let interval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (interval) clearInterval(interval);
    elapsed = 0;
    interval = setInterval(() => {
      elapsed += 100;
      if (elapsed >= durationMs && interval) {
        clearInterval(interval);
        interval = null;
      }
    }, 100);
    return () => {
      if (interval) clearInterval(interval);
    };
  });

  const phase = $derived((elapsed % 8_000) / 1_000); // 8 s cycle
  const scale = $derived(0.55 + 0.2 * Math.sin((phase * Math.PI) / 4));
</script>

<main>
  <h2>Palming</h2>
  <p class="muted">
    Cup your palms over closed eyes. Breathe slowly.
  </p>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <circle cx="50" cy="50" r={28 * scale} fill="#58a6ff" opacity="0.35" />
    <circle cx="50" cy="50" r="6" fill="#fff" />
  </svg>
  <p class="caption">{phase < 4 ? "Inhale" : "Exhale"} · 4 s · 4 s</p>
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
  /* Per Appendix A / §2.7: SVG fill attributes inside an exercise
     component are an explicit carveout — the design language document
     notes these remain inline hex literals. */
  svg {
    width: 200px;
    height: 200px;
  }
  .muted {
    color: var(--text-muted);
    margin: 0 0 1rem;
  }
  .caption {
    margin: 0.75rem 0 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
</style>
