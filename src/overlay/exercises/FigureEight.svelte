<script lang="ts">
  // Figure-eight — animated dot traces a lazy-8.
  let { durationMs = 30_000 }: { durationMs?: number } = $props();

  let elapsed = $state(0);
  const cycle = 4_000; // ms per figure-8
  let interval: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (interval) clearInterval(interval);
    elapsed = 0;
    interval = setInterval(() => {
      elapsed += 50;
      if (elapsed >= durationMs && interval) {
        clearInterval(interval);
        interval = null;
      }
    }, 50);
    return () => {
      if (interval) clearInterval(interval);
    };
  });

  const t = $derived((elapsed % cycle) / cycle); // 0..1
  // Lemniscate of Bernoulli — slight variant with a smooth path.
  function lemniscate(t: number, scale = 38): { x: number; y: number } {
    const a = Math.cos(Math.PI * t);
    const denom = 1 + a * a;
    return {
      x: 50 + (scale * Math.cos(Math.PI * t)) / denom,
      y: 50 + (scale * Math.sin(2 * Math.PI * t)) / denom,
    };
  }
  const pos = $derived(lemniscate(t));
</script>

<main>
  <h2>Figure-eight</h2>
  <p class="muted">Trace the dot with your eyes (don't move your head).</p>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <line
      x1="50"
      y1="10"
      x2="50"
      y2="90"
      stroke="#30363d"
      stroke-width="1"
      stroke-dasharray="2 2"
    />
    <line
      x1="10"
      y1="50"
      x2="90"
      y2="50"
      stroke="#30363d"
      stroke-width="1"
      stroke-dasharray="2 2"
    />
    <circle cx={pos.x} cy={pos.y} r="3" fill="#58a6ff" />
  </svg>
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
  /* The big SVG has both its dot (carveout: fill on <circle>) and the
     crosshair lines (carveout: stroke on <line>). The surrounding
     card shape uses tokens. */
  svg {
    width: 280px;
    height: 280px;
    background: var(--ink);
    border-radius: 50%;
    border: 1px solid var(--hairline);
  }
</style>
