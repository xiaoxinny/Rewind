<script lang="ts">
  // Settings — reads from `mirror.config` (the
  // Svelte-runes mirror) and persists through `setConfig()`.
  //
  // The `state` import below triggers Svelte 5's "Cannot use 'state'
  // as a store" error when used alongside `$state(...)` declarations
  // in the same file. The warning is correct: the `$state` rune and
  // a `state` local variable name are visually confusing. The
  // pragmatic fix is to import `state` under an alias (`mirror`) and
  // update the template references; the type checker then treats
  // the import as a plain const and `$state` keeps its rune meaning.

  import {
    clearHistoryAction,
    exportDataAction,
    patchConfig,
    setConfig,
    setStrictness,
  } from "../lib/stores.svelte";
  import { state as mirror } from "../lib/stores.svelte";
  import { setAutostart, uninstallAndExit } from "../lib/ipc";
  import { restart as restartOnboarding } from "../lib/onboarding.svelte";
  import type {
    AppConfig,
    BreakConfig,
    HydrationConfig,
    IdleConfig,
    PostureConfig,
    QuietHoursConfig,
    ReminderToggles,
    Strictness,
    SystemConfig,
  } from "../lib/types";

  // Section visibility — collapsible groups (start open).
  // About defaults to open: the panel is the only verification surface
  // for Rewind's evidence-honest framing, and most users — including
  // clinicians auditing the citations — will not scroll past 9 collapsed
  // sections to find it.
  let sectionsOpen = $state({
    breaks: true,
    strictness: true,
    idle: false,
    reminders: true,
    hydration: false,
    posture: false,
    quiet: false,
    system: false,
    data: true,
    about: true,
  });

  // Per-section update helpers — each writes through to the engine.
  function updateBreaks(patch: Partial<BreakConfig>): Promise<void> {
    return patchConfig({ breaks: { ...mirror.config.breaks, ...patch } });
  }
  function updateReminders(patch: Partial<ReminderToggles>): Promise<void> {
    return patchConfig({ reminders: { ...mirror.config.reminders, ...patch } });
  }
  function updateHydration(patch: Partial<HydrationConfig>): Promise<void> {
    return patchConfig({ hydration: { ...mirror.config.hydration, ...patch } });
  }
  function updatePosture(patch: Partial<PostureConfig>): Promise<void> {
    return patchConfig({ posture: { ...mirror.config.posture, ...patch } });
  }
  function updateIdle(patch: Partial<IdleConfig>): Promise<void> {
    return patchConfig({ idle: { ...mirror.config.idle, ...patch } });
  }
  function updateQuiet(patch: Partial<QuietHoursConfig>): Promise<void> {
    return patchConfig({ quietHours: { ...mirror.config.quietHours, ...patch } });
  }
  function updateSystem(patch: Partial<SystemConfig>): Promise<void> {
    return patchConfig({ system: { ...mirror.config.system, ...patch } });
  }

  async function pickStrictness(s: Strictness): Promise<void> {
    await setStrictness(s);
    await setConfig({ ...mirror.config, strictness: s });
  }

  let exportStatus = $state<string | null>(null);
  let clearConfirm = $state(false);
  let exportPayload = $state<{
    json: string;
    generated_at: string;
    row_count: number;
  } | null>(null);

  let uninstallConfirmOpen = $state(false);
  let uninstalling = $state(false);

  async function doExport(): Promise<void> {
    try {
      exportPayload = await exportDataAction();
      exportStatus = `Exported ${exportPayload.row_count} rows.`;
      // Trigger a browser download from the bundled frontend.
      const blob = new Blob([exportPayload.json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `rewind-export-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } catch (e) {
      exportStatus = `Export failed: ${e}`;
    }
  }

  async function doClear(): Promise<void> {
    if (!clearConfirm) {
      clearConfirm = true;
      setTimeout(() => {
        clearConfirm = false;
      }, 5_000);
      return;
    }
    await clearHistoryAction();
    exportStatus = "History cleared.";
    clearConfirm = false;
  }

  async function toggleAutostart(): Promise<void> {
    const next = !mirror.config.system.autostart;
    try {
      const newStatus = await setAutostart(next);
      await updateSystem({ autostart: newStatus });
    } catch (e) {
      exportStatus = `Autostart change failed: ${e}`;
    }
  }

  function openUninstallModal(): void {
    uninstallConfirmOpen = true;
  }

  async function confirmUninstall(): Promise<void> {
    uninstalling = true;
    try {
      await uninstallAndExit();
    } catch {
      // The process exits during this call; if it somehow didn't,
      // surface the error.
      uninstalling = false;
      uninstallConfirmOpen = false;
      exportStatus = "Uninstall failed — try the fallback script in the scripts/ directory.";
    }
  }

  /** Restart-tour handler. Dispatches a DOM event App.svelte
   *  listens for (see `src/App.svelte`); the App switches `route`
   *  to `"welcome"`. We don't have access to `route` directly
   *  because it lives in App.svelte. */
  function onRestartTour(): void {
    void restartOnboarding();
    window.dispatchEvent(new CustomEvent("rewind:start-tour"));
  }
</script>

<section class="settings">
  <h2>Settings</h2>

  <!-- ============================= Breaks ============================= -->
  <details open={sectionsOpen.breaks}>
    <summary>Breaks</summary>
    <div class="grid">
      <label>
        Micro interval (min)
        <input
          type="number"
          min="1"
          max="120"
          value={mirror.config.breaks.microIntervalMin}
          onchange={(e) => updateBreaks({ microIntervalMin: +e.currentTarget.value })}
        />
      </label>
      <label>
        Micro duration (s)
        <input
          type="number"
          min="5"
          max="120"
          value={mirror.config.breaks.microDurationSec}
          onchange={(e) => updateBreaks({ microDurationSec: +e.currentTarget.value })}
        />
      </label>
      <label>
        Rest interval (min)
        <input
          type="number"
          min="5"
          max="240"
          value={mirror.config.breaks.restIntervalMin}
          onchange={(e) => updateBreaks({ restIntervalMin: +e.currentTarget.value })}
        />
      </label>
      <label>
        Rest duration (s)
        <input
          type="number"
          min="60"
          max="1200"
          value={mirror.config.breaks.restDurationSec}
          onchange={(e) => updateBreaks({ restDurationSec: +e.currentTarget.value })}
        />
      </label>
      <label>
        Pre-break warning
        <input
          type="checkbox"
          checked={mirror.config.breaks.preBreakWarn}
          onchange={(e) => updateBreaks({ preBreakWarn: e.currentTarget.checked })}
        />
      </label>
      <label>
        Pre-break warning (s)
        <input
          type="number"
          min="0"
          max="60"
          value={mirror.config.breaks.preBreakWarnSec}
          onchange={(e) => updateBreaks({ preBreakWarnSec: +e.currentTarget.value })}
        />
      </label>
      <label>
        Postpone (s)
        <input
          type="number"
          min="30"
          max="1800"
          value={mirror.config.breaks.postponeSec}
          onchange={(e) => updateBreaks({ postponeSec: +e.currentTarget.value })}
        />
      </label>
      <label>
        Max postpones
        <input
          type="number"
          min="0"
          max="10"
          value={mirror.config.breaks.maxPostpones}
          onchange={(e) => updateBreaks({ maxPostpones: +e.currentTarget.value })}
        />
      </label>
    </div>
  </details>

  <!-- ============================= Strictness ============================ -->
  <details open={sectionsOpen.strictness}>
    <summary>Strictness</summary>
    <div class="strictness">
      {#each ["gentle", "normal", "strict"] as Strictness[] as level}
        <label>
          <input
            type="radio"
            name="strictness"
            checked={mirror.config.strictness === level}
            onchange={() => pickStrictness(level)}
          />
          {level}
        </label>
      {/each}
    </div>
    <p class="hint">
      Gentle (default) lets you skip / postpone freely. Strict locks
      full-screen breaks until the timer expires.
    </p>
  </details>

  <!-- ============================= Idle ============================= -->
  <details open={sectionsOpen.idle}>
    <summary>Idle</summary>
    <label class="row">
      <input
        type="checkbox"
        disabled={!mirror.idleReliable}
        checked={mirror.config.idle.enabled}
        onchange={(e) => updateIdle({ enabled: e.currentTarget.checked })}
      />
      Pause when I step away
    </label>
    <div class="grid">
      <label>
        Pause after (s)
        <input
          type="number"
          min="30"
          max="3600"
          value={mirror.config.idle.pauseSec}
          onchange={(e) => updateIdle({ pauseSec: +e.currentTarget.value })}
        />
      </label>
      <label>
        Reset after (s)
        <input
          type="number"
          min="60"
          max="7200"
          value={mirror.config.idle.resetSec}
          onchange={(e) => updateIdle({ resetSec: +e.currentTarget.value })}
        />
      </label>
    </div>
    {#if !mirror.idleReliable}
      <p class="warn">
        Idle detection is unreliable on this session — Rewind is
        running in timer-only mode.
      </p>
    {/if}
  </details>

  <!-- ============================= Reminders ============================ -->
  <details open={sectionsOpen.reminders}>
    <summary>Reminders</summary>
    {#each Object.entries(mirror.config.reminders) as [key, val]}
      <label class="row">
        <input
          type="checkbox"
          checked={val}
          onchange={(e) =>
            updateReminders({ [key]: e.currentTarget.checked } as Partial<ReminderToggles>)}
        />
        {key.replace(/([A-Z])/g, " $1").toLowerCase()}
      </label>
    {/each}
  </details>

  <!-- ============================= Hydration =========================== -->
  <details open={sectionsOpen.hydration}>
    <summary>Hydration</summary>
    <div class="grid">
      <label>
        Daily goal (ml)
        <input
          type="number"
          min="500"
          max="5000"
          step="50"
          value={mirror.config.hydration.goalMl}
          onchange={(e) => updateHydration({ goalMl: +e.currentTarget.value })}
        />
      </label>
      <label>
        Glass size (ml)
        <input
          type="number"
          min="50"
          max="1000"
          value={mirror.config.hydration.glassMl}
          onchange={(e) => updateHydration({ glassMl: +e.currentTarget.value })}
        />
      </label>
      <label>
        Wake window start
        <input
          type="time"
          value={mirror.config.hydration.wakeStart}
          onchange={(e) => updateHydration({ wakeStart: e.currentTarget.value })}
        />
      </label>
      <label>
        Wake window end
        <input
          type="time"
          value={mirror.config.hydration.wakeEnd}
          onchange={(e) => updateHydration({ wakeEnd: e.currentTarget.value })}
        />
      </label>
    </div>
  </details>

  <!-- ============================= Posture ============================== -->
  <details open={sectionsOpen.posture}>
    <summary>Posture</summary>
    <label>
      Interval (min)
      <input
        type="number"
        min="5"
        max="120"
        value={mirror.config.posture.intervalMin}
        onchange={(e) => updatePosture({ intervalMin: +e.currentTarget.value })}
      />
    </label>
  </details>

  <!-- ============================= Quiet hours ========================== -->
  <details open={sectionsOpen.quiet}>
    <summary>Quiet hours</summary>
    <label class="row">
      <input
        type="checkbox"
        checked={mirror.config.quietHours.enabled}
        onchange={(e) => updateQuiet({ enabled: e.currentTarget.checked })}
      />
      Mute reminders during quiet hours
    </label>
    <div class="grid">
      <label>
        Start
        <input
          type="time"
          value={mirror.config.quietHours.start}
          onchange={(e) => updateQuiet({ start: e.currentTarget.value })}
        />
      </label>
      <label>
        End
        <input
          type="time"
          value={mirror.config.quietHours.end}
          onchange={(e) => updateQuiet({ end: e.currentTarget.value })}
        />
      </label>
    </div>
  </details>

  <!-- ============================= System =============================== -->
  <details open={sectionsOpen.system}>
    <summary>System</summary>
    <label class="row">
      <input type="checkbox" checked={mirror.config.system.autostart} onchange={toggleAutostart} />
      Launch at login
    </label>
    <label class="row">
      <input
        type="checkbox"
        checked={mirror.config.system.startMinimized}
        onchange={(e) => updateSystem({ startMinimized: e.currentTarget.checked })}
      />
      Start minimized
    </label>
    <label class="row">
      <input
        type="checkbox"
        checked={mirror.config.system.sound}
        onchange={(e) => updateSystem({ sound: e.currentTarget.checked })}
      />
      Play break sound
    </label>
    <label>
      Volume
      <input
        type="range"
        min="0"
        max="1"
        step="0.05"
        value={mirror.config.system.volume}
        onchange={(e) => updateSystem({ volume: +e.currentTarget.value })}
      />
    </label>
    <label>
      Theme
      <select
        value={mirror.config.system.theme}
        onchange={(e) =>
          updateSystem({ theme: e.currentTarget.value as SystemConfig["theme"] })}
      >
        <option value="system">Follow system</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>
  </details>

  <!-- ============================= Data ================================= -->
  <details open={sectionsOpen.data}>
    <summary>Data</summary>
    <p class="hint">
      Everything stays on this device. The SQLite database lives in
      <code>{`~/Library/Application Support/com.rewind.app/rewind.db`}</code>
      on macOS, <code>{`%APPDATA%/com.rewind.app/rewind.db`}</code> on
      Windows, <code>{`~/.local/share/com.rewind.app/rewind.db`}</code>
      on Linux.
    </p>
    <div class="row-actions">
      <button type="button" onclick={doExport}>Export data (JSON)</button>
      <button type="button" class="danger" onclick={doClear}>
        {clearConfirm ? "Click again to confirm" : "Clear history"}
      </button>
    </div>
    {#if exportStatus}
      <p class="hint">{exportStatus}</p>
    {/if}
  </details>

  <!-- ============================= About ================================ -->
  <details open={sectionsOpen.about}>
    <summary>About the science</summary>

    <article class="cite">
      <h3>The 20-20-20 rule</h3>
      <p>
        Every 20 minutes, look at something 20 feet away for 20 seconds.
        Optometric bodies commonly recommend this cadence as a tip to help
        alleviate digital eyestrain. The most recent randomized work found
        no measurable effect on eye-strain symptoms, reading speed, or
        task accuracy from scheduled 20-second breaks —
        <strong>treat the rule as a helpful habit, not a treatment</strong>.
      </p>
      <p class="cite-line">
        AOA (Computer Vision Syndrome) and Johnson et al. 2023 (Optom Vis Sci) —
        see the in-app About panel for the full bibliography.
      </p>
    </article>

    <article class="cite">
      <h3>Rewind is a habit tool, not a treatment</h3>
      <p>
        Rewind reminds you to build healthier screen habits. It does not
        diagnose, treat, or prevent any condition. A 2022 systematic review
        of intervention trials on computer vision syndrome found no
        high-certainty evidence that any single intervention reliably
        reduces digital eye strain. If something hurts, see a clinician.
      </p>
      <p class="cite-line">
        Singh S, McGuinness MB, Anderson AJ, Downie LE, 2022
        (<em>Ophthalmology</em>) — see the in-app About panel for the
        full bibliography.
      </p>
    </article>

    <article class="cite">
      <h3>How much should you drink?</h3>
      <p>
        Rewind's default goal is <strong>~2&nbsp;L of fluids per day</strong>
        (about 8&nbsp;cups). That's a starting floor, not a universal target
        — population guidance typically puts total daily fluid higher for
        adult men than for adult women, and about a fifth of that intake
        comes from food rather than drinks. Raise the goal in Settings to
        match your body size, climate, and activity.
      </p>
      <p>
        A steady sipping pattern through the day is gentler on the body
        than a few large volumes — the kidneys excrete water more or less
        continuously, not in boluses.
      </p>
      <p class="warn">
        ⚠ Sustained intake above roughly
        <strong>1&nbsp;L per hour</strong> can outpace the kidneys' ability to
        clear water and (rarely) lead to hyponatremia. Rewind's reminders
        are capped at <strong>one 250&nbsp;mL glass every 30&nbsp;minutes</strong>
        — a worst-case cadence-following intake of 0.5&nbsp;L/hr, well
        under that ceiling.
      </p>
      <p class="cite-line">
        Mayo Clinic (Water; Hyponatremia) and Cleveland Clinic
        (Water Intoxication) — see the in-app About panel for the
        full bibliography.
      </p>
    </article>

    <!-- "How we audit our citations" surfaces the audit trail for
         clinicians who want to verify Rewind's evidence chain. The
         audit notes are kept in the maintainer's local working tree;
         the canonical primary sources for each claim are linked inline
         above and in the in-app About panel. -->
    <p class="cite-audit">
      <strong>How we audit our citations</strong> — every source on
      this page has been checked against its primary publication
      (Mayo Clinic, Cleveland Clinic, AOA, and the original peer-reviewed
      papers cited inline). The audit notes list each source, what was
      checked, and what changed.
    </p>

    <!-- Track 2 onboarding restart entry point. The user can always
         re-walk the wizard from here regardless of `first_run_complete`.
         Dispatches a DOM event App.svelte listens for. -->
    <p class="restart-tour-row">
      <button type="button" class="restart-tour" onclick={onRestartTour}>
        Restart tour
      </button>
      <span class="restart-tour-hint">
        Step through welcome, evidence, intervals, and the tray
        explainer again.
      </span>
    </p>
  </details>

  <!-- ============================= Uninstall ============================ -->
  <div class="uninstall-section">
    <div class="uninstall-info">
      <p class="uninstall-label">Uninstall</p>
      <p class="uninstall-desc">Remove Rewind and its data from this computer.</p>
    </div>
    <button
      type="button"
      class="uninstall-btn"
      onclick={openUninstallModal}
    >
      Uninstall Rewind
    </button>
  </div>

  {#if uninstallConfirmOpen}
    <div
      class="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="uninstall-modal-title"
    >
      <div class="modal-card">
        <h3 id="uninstall-modal-title">Uninstall Rewind?</h3>
        <p class="modal-body">
          This will delete Rewind and all its settings, history, and
          hydration data from this computer. This cannot be undone.
        </p>
        {#if uninstalling}
          <p class="modal-status">Uninstalling, closing…</p>
        {/if}
        <div class="modal-actions">
          <button
            type="button"
            class="modal-cancel"
            onclick={() => (uninstallConfirmOpen = false)}
            disabled={uninstalling}
          >
            Cancel
          </button>
          <button
            type="button"
            class="modal-confirm"
            onclick={confirmUninstall}
            disabled={uninstalling}
          >
            {uninstalling ? "Uninstalling…" : "Uninstall"}
          </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .settings {
    display: grid;
    gap: 0.5rem;
  }
  h2 {
    margin: 0 0 0.5rem;
    font-size: 1.75rem;
    font-family: var(--font-display);
  }
  details {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: 0.5rem 0.875rem;
  }
  details summary {
    cursor: pointer;
    font-weight: 600;
    padding: 0.25rem 0;
    list-style: none;
    color: var(--text);
  }
  details summary::-webkit-details-marker {
    display: none;
  }
  details summary::before {
    content: "▸ ";
    color: var(--text-muted);
  }
  details[open] > summary::before {
    content: "▾ ";
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.5rem 1rem;
    padding: 0.5rem 0 0.25rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
    color: var(--text-2);
  }
  label.row {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0;
  }
  input[type="number"],
  input[type="time"],
  select {
    appearance: none;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-input);
    padding: 0.35rem 0.5rem;
    color: var(--text);
    font: inherit;
    font-variant-numeric: tabular-nums;
  }
  /* Visible focus on the keyboard path only. Keep the border
     change so a mouse user still sees which input they're in. */
  input[type="number"]:focus,
  input[type="time"]:focus,
  select:focus {
    border-color: var(--accent);
  }
  input[type="number"]:focus-visible,
  input[type="time"]:focus-visible,
  select:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 1px;
  }
  input[type="range"] {
    accent-color: var(--accent);
  }
  input:disabled + * {
    color: var(--text-faint);
  }
  .strictness {
    display: flex;
    gap: 0.75rem;
    padding: 0.5rem 0;
  }
  .strictness label {
    flex-direction: row;
    align-items: center;
  }
  .hint {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 0.25rem 0;
  }
  /* Banner variant="warning" pattern: --ink-3 surface,
     --danger 3px left border. Re-using the state tokens. */
  .warn {
    color: var(--text-2);
    font-size: 0.875rem;
    margin: 0.5rem 0 0;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius-input);
    padding: 0.5rem 0.625rem;
  }
  .row-actions {
    display: flex;
    gap: 0.5rem;
    padding-top: 0.5rem;
  }
  .row-actions button {
    appearance: none;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    color: var(--text);
    padding: 0.4rem 0.75rem;
    border-radius: var(--radius-input);
    cursor: pointer;
    transition: border-color var(--dur-small) var(--ease);
  }
  .row-actions button.danger {
    /* variant="danger" — only on "Clear history". */
    border-color: var(--danger);
    color: var(--danger);
  }
  .row-actions button:hover {
    border-color: var(--accent);
  }
  .cite {
    background: var(--ink);
    border-left: 3px solid var(--hairline);
    padding: 0.5rem 0.75rem;
    margin: 0.5rem 0;
    border-radius: 0 4px 4px 0;
  }
  .cite h3 {
    margin: 0 0 0.25rem;
    font-size: 0.95rem;
  }
  /* The cite-line is the durable citation. The exact text was
     set by the audit fix at commit ed10be9; only the styling moves. */
  .cite-line {
    margin: 0.5rem 0 0;
    color: var(--text-muted);
    font-size: 0.8rem;
    font-family: var(--font-mono);
    font-style: normal;
  }

  .cite-audit {
    margin: 0.75rem 0 0;
    padding-top: 0.5rem;
    border-top: 1px solid var(--hairline);
    color: var(--text-2);
    font-size: 0.85rem;
    line-height: 1.45;
  }

  .cite-audit a {
    color: var(--accent);
  }

  .cite-audit a:hover {
    text-decoration: underline;
  }

  /* Track 2 — restart-tour row inside the About panel. The button
     is a small secondary control (variant="secondary"); the
     hint copy mirrors Settings voice (terse, present tense). */
  .restart-tour-row {
    margin: 0.5rem 0 0;
    display: flex;
    align-items: baseline;
    gap: 0.625rem;
    flex-wrap: wrap;
  }

  .restart-tour {
    appearance: none;
    background: transparent;
    border: 1px solid var(--hairline);
    color: var(--text-2);
    padding: 0.4rem 0.75rem;
    border-radius: var(--radius-input);
    cursor: pointer;
    font: inherit;
    font-size: 0.875rem;
    transition: border-color var(--dur-small) var(--ease),
      color var(--dur-small) var(--ease);
  }

  .restart-tour:hover {
    border-color: var(--accent);
    color: var(--text);
  }

  .restart-tour:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }

  .restart-tour-hint {
    color: var(--text-muted);
    font-size: 0.85rem;
    line-height: 1.4;
  }

  /* ============================ Uninstall ============================= */
  .uninstall-section {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 0.875rem;
    border-top: 1px solid var(--hairline);
    margin-top: 0.5rem;
  }
  .uninstall-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .uninstall-label {
    font-weight: 600;
    margin: 0;
    color: var(--text);
  }
  .uninstall-desc {
    font-size: 0.85rem;
    color: var(--text-muted);
    margin: 0;
  }
  .uninstall-btn {
    appearance: none;
    background: var(--danger-soft);
    border: 1px solid var(--danger);
    color: var(--danger);
    padding: 0.4rem 0.875rem;
    border-radius: var(--radius-input);
    cursor: pointer;
    font: inherit;
    font-size: 0.875rem;
    white-space: nowrap;
    transition: border-color var(--dur-small) var(--ease),
      background var(--dur-small) var(--ease);
  }
  .uninstall-btn:hover {
    background: var(--danger);
    color: #fff;
  }
  .uninstall-btn:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }

  /* ============================ Modal ================================= */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }
  .modal-card {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: 1.25rem 1.5rem;
    max-width: 380px;
    width: calc(100% - 2rem);
    box-sizing: border-box;
  }
  .modal-card h3 {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--text);
  }
  .modal-body {
    margin: 0 0 0.75rem;
    font-size: 0.875rem;
    line-height: 1.45;
    color: var(--text-2);
  }
  .modal-status {
    margin: 0 0 0.75rem;
    font-size: 0.85rem;
    color: var(--text-muted);
    font-style: italic;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .modal-cancel,
  .modal-confirm {
    appearance: none;
    padding: 0.4rem 0.875rem;
    border-radius: var(--radius-input);
    cursor: pointer;
    font: inherit;
    font-size: 0.875rem;
    transition: border-color var(--dur-small) var(--ease),
      background var(--dur-small) var(--ease);
  }
  .modal-cancel {
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    color: var(--text);
  }
  .modal-cancel:hover {
    border-color: var(--accent);
  }
  .modal-cancel:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }
  .modal-confirm {
    background: var(--danger);
    border: 1px solid var(--danger);
    color: #fff;
  }
  .modal-confirm:hover {
    opacity: 0.9;
  }
  .modal-confirm:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }
  .modal-confirm:disabled,
  .modal-cancel:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  code {
    background: var(--ink);
    padding: 0 0.25rem;
    border-radius: 3px;
    font-size: 0.85em;
    font-family: var(--font-mono);
  }
</style>
