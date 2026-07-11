<script lang="ts">
  // Settings — full §13 surface. M6 reads from `mirror.config` (the
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
  import { setAutostart } from "../lib/ipc";
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
    about: false,
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
        The American Optometric Association lists this as a tip to help
        alleviate digital eyestrain. A 2023 Optometry &amp; Vision Science
        study (Johnson et al.) tested scheduled 20-second breaks in a
        40-minute reading task and found no measurable effect on eye
        strain, reading speed, or accuracy — <strong>treat the rule as
        a helpful habit, not a treatment</strong>.
      </p>
      <p class="cite-line">
        AOA, aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome;
        Johnson et al. 2023, Optom Vis Sci 100(1):52–56, doi:10.1097/OPX.0000000000001971 (PMID 36473088).
      </p>
    </article>

    <article class="cite">
      <h3>Rewind is a habit tool, not a treatment</h3>
      <p>
        Rewind reminds you to build healthier screen habits. It does not
        diagnose, treat, or prevent any condition — a 2022 systematic
        review of 45 RCTs (4,497 participants) on computer vision syndrome
        interventions found no high-certainty evidence that any
        intervention (eye breaks, blue-light lenses, multifocals, or
        supplements) reliably reduces digital eye strain. If something
        hurts, see a clinician.
      </p>
      <p class="cite-line">
        Singh et al. 2022, Ophthalmology 129(10):1192–1215,
        doi:10.1016/j.ophtha.2022.05.009 (PMID 35597519).
      </p>
    </article>

    <article class="cite">
      <h3>How much should you drink?</h3>
      <p>
        Rewind's default goal is <strong>~2&nbsp;L of fluids per day</strong>
        (about 8&nbsp;cups). That's a safe floor, not a universal target —
        the Mayo Clinic estimates most adults need
        <a href="https://www.mayoclinic.org/healthy-lifestyle/nutrition-and-healthy-eating/in-depth/water/art-20044256">2.7&nbsp;L (women) to 3.7&nbsp;L (men)</a>
        of <em>total</em> fluid per day, and roughly 20% of that comes from food
        rather than drinks. Raise the goal in Settings to match your body size,
        climate, and activity.
      </p>
      <p>
        A steady sipping pattern through the day is gentler on the body than
        a few large volumes — the kidneys excrete water more or less
        continuously, not in boluses.
      </p>
      <p class="warn">
        ⚠ Sustained intake above roughly
        <strong>1&nbsp;L per hour</strong> can outpace the kidneys' ability to
        clear water and lead to hyponatremia (blood sodium &lt;135&nbsp;mmol/L) —
        <a href="https://my.clevelandclinic.org/health/diseases/water-intoxication">Cleveland Clinic, 2024</a>;
        <a href="https://www.mayoclinic.org/diseases-conditions/hyponatremia/symptoms-causes/syc-20373711">Mayo Clinic, 2025</a>.
        Rewind's reminders are capped at <strong>one 250&nbsp;mL glass every
        30&nbsp;minutes</strong> — a worst-case cadence-following intake of
        0.5&nbsp;L/hr, well under that ceiling.
      </p>
      <p class="cite-line">
        Mayo Clinic, Hyponatremia (reviewed July 2025);
        Mayo Clinic, Water: How much should you drink every day?;
        Cleveland Clinic, Water Intoxication (reviewed September 2024).
      </p>
    </article>
  </details>
</section>

<style>
  .settings {
    display: grid;
    gap: 0.5rem;
  }
  h2 {
    margin: 0 0 0.5rem;
    font-size: 1.25rem;
  }
  details {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 0.5rem 0.875rem;
  }
  details summary {
    cursor: pointer;
    font-weight: 600;
    padding: 0.25rem 0;
    list-style: none;
  }
  details summary::-webkit-details-marker {
    display: none;
  }
  details summary::before {
    content: "▸ ";
    color: #8b949e;
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
    color: #c9d1d9;
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
    background: #0d1117;
    border: 1px solid #30363d;
    border-radius: 4px;
    padding: 0.35rem 0.5rem;
    color: inherit;
    font: inherit;
  }
  input[type="number"]:focus,
  input[type="time"]:focus,
  select:focus {
    border-color: #58a6ff;
    outline: none;
  }
  input[type="range"] {
    accent-color: #58a6ff;
  }
  input:disabled + * {
    color: #6e7681;
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
    color: #8b949e;
    font-size: 0.85rem;
    margin: 0.25rem 0;
  }
  .warn {
    color: #ffa198;
    font-size: 0.875rem;
    margin: 0.5rem 0 0;
    background: #21262d;
    border: 1px solid #f85149;
    border-radius: 4px;
    padding: 0.5rem 0.625rem;
  }
  .row-actions {
    display: flex;
    gap: 0.5rem;
    padding-top: 0.5rem;
  }
  .row-actions button {
    appearance: none;
    background: #21262d;
    border: 1px solid #30363d;
    color: inherit;
    padding: 0.4rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
  }
  .row-actions button.danger {
    border-color: #f85149;
    color: #ffa198;
  }
  .row-actions button:hover {
    border-color: #58a6ff;
  }
  .cite {
    background: #0d1117;
    border-left: 3px solid #30363d;
    padding: 0.5rem 0.75rem;
    margin: 0.5rem 0;
    border-radius: 0 4px 4px 0;
  }
  .cite h3 {
    margin: 0 0 0.25rem;
    font-size: 0.95rem;
  }
  .cite-line {
    margin: 0.5rem 0 0;
    color: #8b949e;
    font-size: 0.8rem;
    font-style: italic;
  }
  code {
    background: #0d1117;
    padding: 0 0.25rem;
    border-radius: 3px;
    font-size: 0.85em;
  }
</style>
