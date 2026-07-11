<script lang="ts">
  // Top-level Svelte 5 shell for the Rewind dashboard. M6 introduces:
  //   * a simple route state ($state) — no router dependency
  //   * bootstrap() that wires the engine + storage listeners on mount
  //   * nav links between Dashboard / Settings / Stats

  import { onMount } from "svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import Settings from "./routes/Settings.svelte";
  import Stats from "./routes/Stats.svelte";
  import Onboarding from "./routes/Onboarding.svelte";
  import { bootstrap } from "./lib/stores.svelte";
  import { loadInitial, onboardingState, restart } from "./lib/onboarding.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // Simple route state — no router dependency. M6 ships a single
  // shell; routing could move to a real router post-v1. Track 2
  // adds the four onboarding route values; `route` switches into
  // Onboarding.svelte when `onboardingState.step !== null`.
  type Route =
    | "dashboard"
    | "settings"
    | "stats"
    | "welcome"
    | "evidence"
    | "intervals"
    | "enable";
  let route = $state<Route>("dashboard");

  /** Reflect the active onboarding screen into `route` so the nav
   *  buttons hide while the wizard is up. `welcome` doubles as the
   *  "start of onboarding" route — callers (the Dashboard `?` button,
   *  the Settings Restart-tour button) can set route="welcome" to
   *  trigger the wizard even after it's been completed. */
  let isOnboardingActive = $derived(
    route === "welcome" ||
      route === "evidence" ||
      route === "intervals" ||
      route === "enable",
  );

  let teardown: UnlistenFn | null = null;
  let tourEventTeardown: (() => void) | null = null;

  onMount(() => {
    let cancelled = false;
    // Bootstrap engine state and onboarding persistence in parallel.
    // The engine bootstrap is critical (drives the dashboard); the
    // onboarding bootstrap just gates the auto-route into the wizard
    // if it's the user's first run.
    Promise.all([bootstrap(), loadInitial()])
      .then(([unlisten]) => {
        if (cancelled) {
          unlisten();
        } else {
          teardown = unlisten;
          // First-run detection: the persisted `first_run_complete`
          // is read; if it's false (or missing), route the user
          // into the onboarding wizard starting at step 1. The
          // exact step is `onboardingState.step` (persisted too).
          if (onboardingState.firstRun && onboardingState.step !== null) {
            route = onboardingState.step;
          }
        }
      })
      .catch((e) => {
        // Don't block first paint on a store read failure; the
        // wizard still has a console warning from inside
        // loadInitial(). Log only.
        console.error("Rewind: bootstrap failed", e);
      });

    // Listen for the Settings → About "Restart tour" event so a
    // sub-route can flip the top-level `route` back to `welcome`.
    // The Settings page can't mutate `route` directly because it
    // lives in this App shell.
    const onTour = (): void => {
      void restart();
      route = "welcome";
    };
    window.addEventListener("rewind:start-tour", onTour);
    tourEventTeardown = () => window.removeEventListener("rewind:start-tour", onTour);

    return () => {
      cancelled = true;
      if (teardown) teardown();
      if (tourEventTeardown) tourEventTeardown();
    };
  });

  function goto(next: Route): void {
    route = next;
  }

  /** Restart the onboarding wizard from the Dashboard's `?` button.
   *  Always re-shows step 1 regardless of `first_run_complete`. */
  function startTour(): void {
    void restart();
    route = "welcome";
  }
</script>

<main>
  <header>
    <h1>Rewind</h1>
    <p class="tagline">Rest, and rewind, for you and your eyes.</p>
    <!-- Dashboard `?` button — only visible on the Dashboard, and
         only when the user isn't already inside the wizard. It's the
         second of two restart entry points (the other lives in
         Settings → About "Restart tour"). -->
    {#if route === "dashboard" && !isOnboardingActive}
      <button
        type="button"
        class="tour-help"
        onclick={startTour}
        aria-label="Restart the first-run tour"
        title="Restart the first-run tour"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="9" />
          <path
            d="M9.5 9.5a2.5 2.5 0 0 1 5 0c0 1.5-2.5 2-2.5 3.5"
            stroke-linecap="round"
          />
          <circle cx="12" cy="17" r="0.5" fill="currentColor" />
        </svg>
      </button>
    {/if}
  </header>

  <!-- Tab nav. Hidden during onboarding — the wizard is full-screen
       and the user shouldn't be able to navigate around it. -->
  {#if !isOnboardingActive}
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
  {/if}

  <section class="route-host">
    {#if route === "dashboard"}
      <Dashboard />
    {:else if route === "settings"}
      <Settings />
    {:else if route === "stats"}
      <Stats />
    {:else if route === "welcome" || route === "evidence" || route === "intervals" || route === "enable"}
      <Onboarding />
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

  header {
    position: relative;
  }

  header h1 {
    margin: 0 0 0.25rem;
    font-size: 2rem;
    letter-spacing: -0.02em;
    font-family: var(--font-display);
  }

  /* The Dashboard `?` button (Track 2). Anchored top-right of the
     header so it doesn't shove the title around. 20px hit-target,
     outline-only stroke icon (§7.1). */
  .tour-help {
    position: absolute;
    top: 0;
    right: 0;
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: 999px;
    transition: color var(--dur-small) var(--ease);
  }

  .tour-help:hover {
    color: var(--text);
  }

  .tour-help:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }

  .tour-help svg {
    width: 20px;
    height: 20px;
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
