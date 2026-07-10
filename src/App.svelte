<script lang="ts">
  // Top-level Svelte 5 shell for the Rewind dashboard. M0 is a stub:
  // it renders placeholder cards for the four pillars (eye breaks,
  // exercises, hydration, posture) and a "Today" summary line. The
  // real data flow lands in M1 (`core-event` subscription) and M6
  // (history rollup).

  // import { invoke } from "@tauri-apps/api/core";
</script>

<main>
  <header>
    <h1>Rewind</h1>
    <p class="tagline">Rest, and rewind, for you and your eyes.</p>
  </header>

  <section class="today">
    <h2>Today</h2>
    <p>No data yet — the engine starts in M1.</p>
  </section>

  <section class="pillars">
    <article>
      <h3>Eye breaks</h3>
      <p>20-20-20 micro-breaks, every 20 min.</p>
    </article>
    <article>
      <h3>Eye exercises</h3>
      <p>Guided rest-break exercises (palming, near/far, blink, figure-8).</p>
    </article>
    <article>
      <h3>Hydration</h3>
      <p>Adaptive, capped, gentle reminders.</p>
    </article>
    <article>
      <h3>Posture</h3>
      <p>Stand/stretch nudges, coalesced onto rest breaks.</p>
    </article>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    background: #0e1116;
    color: #e6edf3;
  }

  main {
    max-width: 880px;
    margin: 0 auto;
    padding: 2rem 1.5rem;
  }

  header h1 {
    margin: 0 0 0.25rem;
    font-size: 2rem;
    letter-spacing: -0.02em;
  }

  .tagline {
    margin: 0 0 2rem;
    color: #8b949e;
    font-style: italic;
  }

  .today {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 1rem 1.25rem;
    margin-bottom: 1.5rem;
  }

  .pillars {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 1rem;
  }

  article {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }

  article h3 {
    margin: 0 0 0.5rem;
    font-size: 1rem;
  }

  article p {
    margin: 0;
    color: #8b949e;
    font-size: 0.875rem;
  }
</style>
