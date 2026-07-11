<script lang="ts">
  // Top-level Svelte 5 shell for the Rewind dashboard. M6 introduces:
  //   * a simple route state ($state) — no router dependency
  //   * bootstrap() that wires the engine + storage listeners on mount
  //   * nav links between Dashboard / Settings / Stats

  import { onMount } from "svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import Settings from "./routes/Settings.svelte";
  import Stats from "./routes/Stats.svelte";
  import { bootstrap } from "./lib/stores.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // Simple route state — no router dependency. M6 ships a single
  // shell; routing could move to a real router post-v1.
  type Route = "dashboard" | "settings" | "stats";
  let route = $state<Route>("dashboard");

  let teardown: UnlistenFn | null = null;

  onMount(() => {
    let cancelled = false;
    bootstrap().then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        teardown = unlisten;
      }
    });
    return () => {
      cancelled = true;
      if (teardown) teardown();
    };
  });

  function goto(next: Route): void {
    route = next;
  }
</script>

<main>
  <header>
    <h1>Rewind</h1>
    <p class="tagline">Rest, and rewind, for you and your eyes.</p>
  </header>

  <nav class="tabs" aria-label="Sections">
    <button
      type="button"
      class:active={route === "dashboard"}
      onclick={() => goto("dashboard")}
    >
      Dashboard
    </button>
    <button
      type="button"
      class:active={route === "settings"}
      onclick={() => goto("settings")}
    >
      Settings
    </button>
    <button
      type="button"
      class:active={route === "stats"}
      onclick={() => goto("stats")}
    >
      Stats
    </button>
  </nav>

  <section class="route-host">
    {#if route === "dashboard"}
      <Dashboard />
    {:else if route === "settings"}
      <Settings />
    {:else if route === "stats"}
      <Stats />
    {/if}
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: var(--font-body);
    background: var(--ink);
    color: var(--text);
  }

  :global(button) {
    font: inherit;
    color: inherit;
  }

  main {
    max-width: 880px;
    margin: 0 auto;
    padding: 1.5rem 1.5rem 4rem;
  }

  header h1 {
    margin: 0 0 0.25rem;
    font-size: 2rem;
    letter-spacing: -0.02em;
    font-family: var(--font-display);
  }

  .tagline {
    margin: 0 0 1rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .tabs {
    display: flex;
    gap: 0.25rem;
    margin: 0 0 1.5rem;
    border-bottom: 1px solid var(--hairline);
  }

  .tabs button {
    appearance: none;
    background: transparent;
    border: none;
    color: var(--text-muted);
    padding: 0.5rem 0.875rem;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: color var(--dur-small) var(--ease),
      border-bottom-color var(--dur-small) var(--ease);
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .route-host {
    display: block;
  }

  /* Visible focus ring (DESIGN_LANGUAGE.md §9.2); keyboard-only. */
  :global(:focus-visible) {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }
</style>
