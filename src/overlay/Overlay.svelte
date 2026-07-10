<script lang="ts">
  // Break overlay shell. M0 is a stub — just a centered message. The
  // real overlay (countdown, exercise host, Skip/Postpone per
  // strictness) lands in M3 per the implementation plan.
</script>

<main>
  <div class="card">
    <h1>Time to rest</h1>
    <p>The break overlay lands in M3.</p>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    background: transparent;
    color: #e6edf3;
  }

  main {
    height: 100vh;
    display: grid;
    place-items: center;
  }

  .card {
    background: rgba(14, 17, 22, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 1.5rem 2rem;
    text-align: center;
  }

  h1 {
    margin: 0 0 0.5rem;
    font-size: 1.5rem;
  }

  p {
    margin: 0;
    color: #8b949e;
    font-size: 0.9rem;
  }
</style>
