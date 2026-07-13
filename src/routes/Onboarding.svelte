<script lang="ts">
  // Track 2 — first-run wizard. Four screens (welcome / evidence /
  // intervals / enable), per the Track 2 brief. Persistence:
  // tauri-plugin-store keys `first_run_complete` + `onboarding_step`
  // in app_state.json.
  // See `src/lib/onboarding.svelte.ts` for the state machine.
  //
  // This is a SINGLE component with four `<section>`s gated by
  // `onboardingState.step` — the brief mandates "single component,
  // 4 sections" so the state handoff is direct and zero routing
  // happens between screens. A Back button is rendered on screens
  // 2/3/4; a Skip button is rendered top-right of every screen.
  //
  // The Dashboard `?` button and Settings → About "Restart tour"
  // button both call `restart()` from this module, which sets
  // `onboardingState.step = "welcome"` — that's how a re-entrant
  // restart works.

  import {
    advance,
    apply,
    complete,
    nextFrom,
    onboardingState,
    prevFrom,
    setDraft,
    skip,
  } from "../lib/onboarding.svelte";

  // Step counts for the numeric badge (1/4, 2/4, …). Requires
  // `font-variant-numeric: tabular-nums` on every numeric so the
  // counter doesn't shift width as the user advances.
  const totalSteps = 4;
  let currentIndex = $derived(
    onboardingState.step === null
      ? 0
      : (["welcome", "evidence", "intervals", "enable"] as const).indexOf(
          onboardingState.step,
        ) + 1,
  );
  const stepCounter = $derived(
    `${currentIndex.toString().padStart(1, "0")} / ${totalSteps}`,
  );

  // ---------------------------------------------------------------------------
  // Per-screen CTAs. Each one calls `advance()` to persist the new
  // step, then the Onboarding will naturally re-render the matching
  // section in the next micro-tick. The last screen (`enable`) uses
  // `complete()` instead of `advance()` because advancing past
  // "enable" exits the flow.
  // ---------------------------------------------------------------------------

  function goNext(): void {
    if (onboardingState.step === null) return;
    const next = nextFrom(onboardingState.step);
    if (next === null) {
      // Past the end — treat as completion.
      void complete();
    } else {
      void advance(next);
    }
  }

  function goBack(): void {
    if (onboardingState.step === null) return;
    const prev = prevFrom(onboardingState.step);
    if (prev !== null) void advance(prev);
  }

  async function onApply(): Promise<void> {
    await apply();
    // After Apply, route forward to the Enable screen so the user
    // still sees the permission explainer + Start CTA.
    void advance("enable");
  }

  async function onSkip(): Promise<void> {
    await skip();
  }
</script>

<section class="onboarding" aria-label="First-run onboarding">
  <!-- Top-right "Skip tour". Always available on every screen -->
  <!-- (brief mandate). Touches `first_run_complete` so the wizard -->
  <!-- never auto-shows again after a skip. -->
  <button type="button" class="skip" onclick={onSkip} aria-label="Skip the tour">
    Skip tour
  </button>

  <!-- Step counter. Sentence case, tabular-nums. -->
  <p class="counter" aria-live="polite">
    <span>Step</span>
    <span class="counter-num">{stepCounter}</span>
  </p>

  <!-- ============================== Welcome ============================== -->
  {#if onboardingState.step === "welcome"}
    <article class="welcome hero">
      <h2>A habit tool, not a treatment.</h2>
      <p class="hero-sub">
        Rewind reminds you to look away from the screen, drink water, and
        stretch. It's a habit tool — it does not diagnose, treat, or
        prevent any condition.
      </p>
      <p class="hero-copy">
        Dr. Lin asked us to be evidence-honest before anything else —
        no claims we can't cite, no "wellness" promises we can't keep.
        A 2022 systematic review of 45 randomised trials on computer
        vision syndrome interventions found no high-certainty evidence
        that any single approach reliably reduces digital eye strain.
        Treat the nudges as a helpful habit, not a treatment.
      </p>
      <p class="cite-line">
        Singh et al. 2022, Ophthalmology 129(10):1192–1215,
        doi:10.1016/j.ophtha.2022.05.009 (PMID 35597519).
      </p>

      <div class="actions primary-row">
        <button type="button" class="primary" onclick={goNext}>
          Next: show me why
        </button>
      </div>
    </article>

  <!-- ============================== Evidence ============================= -->
  {:else if onboardingState.step === "evidence"}
    <article class="evidence">
      <h2>The evidence, briefly</h2>
      <p class="lede">
        Three sources, each cited verbatim from the audited
        references. Cards are short — the full audit trail lives in
        Settings → About the science.
      </p>

      <section class="cards">
        <!-- Card 1 — AOA 20-20-20. -->
        <article class="card">
          <h3>The 20-20-20 rule</h3>
          <p>
            The American Optometric Association lists this as a tip to
            help alleviate digital eyestrain. A 2023 Optometry &amp;
            Vision Science study (Johnson et al.) tested scheduled
            20-second breaks and found no measurable effect on eye
            strain, reading speed, or accuracy — treat the rule as a
            helpful habit, not a treatment.
          </p>
          <p class="cite-line">
            AOA, aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome;
            Johnson et al. 2023, Optom Vis Sci 100(1):52–56,
            doi:10.1097/OPX.0000000000001971 (PMID 36473088).
          </p>
          <a
            class="ext"
            href="https://www.aoa.org/healthy-eyes/eye-and-vision-conditions/computer-vision-syndrome"
            target="_blank"
            rel="noopener"
          >
            Open the AOA page
          </a>
        </article>

        <!-- Card 2 — Mayo Hyponatremia. -->
        <article class="card">
          <h3>Why 2&nbsp;L a day, not 5</h3>
          <p>
            Sustained intake above roughly 1&nbsp;L/hr can outpace the
            kidneys' ability to clear water and lead to hyponatremia
            (blood sodium &lt;135&nbsp;mmol/L). Rewind's reminders cap
            at one 250&nbsp;mL glass every 30&nbsp;minutes — a
            worst-case cadence-following intake of 0.5&nbsp;L/hr, well
            under that ceiling.
          </p>
          <p class="cite-line">
            Mayo Clinic, Hyponatremia — Symptoms &amp; causes
            (reviewed July 2025);
            Cleveland Clinic, Water Intoxication (reviewed September 2024).
          </p>
          <a
            class="ext"
            href="https://www.mayoclinic.org/diseases-conditions/hyponatremia/symptoms-causes/syc-20373711"
            target="_blank"
            rel="noopener"
          >
            Open the Mayo page
          </a>
        </article>

        <!-- Card 3 — Cleveland Clinic Water Intoxication. -->
        <article class="card">
          <h3>Where the 1&nbsp;L/hr line comes from</h3>
          <p>
            The Cleveland Clinic Health Library states that more than
            32&nbsp;oz (~1&nbsp;L) per hour is "probably too much."
            Rewind is intentionally gentler than this ceiling, and
            pauses reminders during your configured quiet hours and
            outside your waking window.
          </p>
          <p class="cite-line">
            Cleveland Clinic, Water Intoxication: Toxicity,
            Symptoms &amp; Treatment (reviewed September 2024).
            my.clevelandclinic.org/health/diseases/water-intoxication.
          </p>
          <a
            class="ext"
            href="https://my.clevelandclinic.org/health/diseases/water-intoxication"
            target="_blank"
            rel="noopener"
          >
            Open the Cleveland Clinic page
          </a>
        </article>
      </section>

      <p class="audit-link">
        <em>
          How we audit our citations — every source above has been
          checked against its primary publication (Mayo Clinic,
          Cleveland Clinic, AOA, and the original peer-reviewed papers
          cited inline). The audit notes list each source, what was
          checked, and what changed.
        </em>
      </p>

      <div class="actions primary-row">
        <button type="button" class="ghost" onclick={goBack}>
          Back
        </button>
        <button type="button" class="primary" onclick={goNext}>
          Next: pick my intervals
        </button>
      </div>
    </article>

  <!-- ============================== Intervals ============================ -->
  {:else if onboardingState.step === "intervals"}
    <article class="intervals">
      <h2>Pick your intervals</h2>
      <p class="lede">
        Defaults match <code>AppConfig::default()</code>. Adjust to taste
        — nothing is saved until you click apply.
      </p>

      <!-- Micro break — 20-20-20 cadence. -->
      <section class="knob">
        <div class="knob-row">
          <label for="micro-interval">Micro interval</label>
          <div class="knob-input">
            <input
              id="micro-interval"
              type="number"
              min="5"
              max="120"
              value={onboardingState.draft.breaks.microIntervalMin}
              oninput={(e) =>
                setDraft({
                  breaks: {
                    ...onboardingState.draft.breaks,
                    microIntervalMin: +e.currentTarget.value,
                  },
                })}
            />
            <span class="unit">min</span>
          </div>
        </div>
        <p class="hint">
          Look away from the screen for
          {onboardingState.draft.breaks.microDurationSec}&nbsp;s every
          {onboardingState.draft.breaks.microIntervalMin}&nbsp;min — the
          20-20-20 rule from the American Optometric Association.
        </p>
      </section>

      <!-- Rest break — longer pause for stretches + eye exercises. -->
      <section class="knob">
        <div class="knob-row">
          <label for="rest-interval">Rest interval</label>
          <div class="knob-input">
            <input
              id="rest-interval"
              type="number"
              min="5"
              max="240"
              value={onboardingState.draft.breaks.restIntervalMin}
              oninput={(e) =>
                setDraft({
                  breaks: {
                    ...onboardingState.draft.breaks,
                    restIntervalMin: +e.currentTarget.value,
                  },
                })}
            />
            <span class="unit">min</span>
          </div>
        </div>
        <p class="hint">
          A longer {onboardingState.draft.breaks.restDurationSec / 60}-minute step
          away with eye exercises. Default {onboardingState.draft.breaks.restIntervalMin}&nbsp;min.
        </p>
      </section>

      <!-- Hydration goal. -->
      <section class="knob">
        <div class="knob-row">
          <label for="hydration-goal">Hydration goal</label>
          <div class="knob-input">
            <input
              id="hydration-goal"
              type="number"
              min="500"
              max="5000"
              step="50"
              value={onboardingState.draft.hydration.goalMl}
              oninput={(e) =>
                setDraft({
                  hydration: {
                    ...onboardingState.draft.hydration,
                    goalMl: +e.currentTarget.value,
                  },
                })}
            />
            <span class="unit">ml</span>
          </div>
        </div>
        <p class="hint">
          A safe floor — most adults actually need
          2.7&nbsp;L–3.7&nbsp;L. Raise this in Settings if you're
          larger / more active; the scheduler will not exceed
          1&nbsp;glass per 30&nbsp;min.
        </p>
      </section>

      <div class="actions primary-row">
        <button type="button" class="ghost" onclick={goBack}>
          Back
        </button>
        <button type="button" class="primary" onclick={onApply}>
          Apply &amp; continue
        </button>
      </div>
    </article>

  <!-- ============================== Enable =============================== -->
  {:else if onboardingState.step === "enable"}
    <article class="enable hero">
      <h2>One last thing.</h2>
      <p class="hero-sub">
        Rewind needs to run quietly in the background and show a tray
        icon so its reminders can fire on schedule. No microphone, no
        camera, no network — just a small process that watches the
        timer.
      </p>
      <p class="hero-copy">
        A tray icon (<svg
          class="tray-glyph"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          aria-hidden="true"
        >
          <rect x="3" y="6" width="18" height="12" rx="2" />
          <line x1="3" y1="18" x2="21" y2="18" />
          <line x1="12" y1="18" x2="12" y2="22" />
          <line x1="9" y1="22" x2="15" y2="22" />
        </svg>) appears in your menu bar — open it for "Pause", "Resume",
        and "Quit" controls. Launching at login is a separate choice in
        Settings → System, opt-in only.
      </p>

      <div class="actions primary-row">
        <button type="button" class="ghost" onclick={goBack}>Back</button>
        <button type="button" class="primary" onclick={complete}>
          Start Rewind
        </button>
      </div>
    </article>
  {/if}
</section>

<style>
  /* ---------- Shell ---------- */
  .onboarding {
    position: relative;
    /* Above the App.svelte main padding so the Skip button can
       hard-anchor top-right without colliding with the tab nav. */
    margin: 0 auto;
    max-width: 720px;
    display: grid;
    gap: var(--space-2);
    padding-top: var(--space-4);
  }

  /* Skip button. Top-right ghost; this is the only button
     that exists outside the normal row at the bottom. */
  .skip {
    position: absolute;
    top: var(--space-4);
    right: 0;
    appearance: none;
    background: transparent;
    border: none;
    padding: 0.25rem 0.5rem;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.875rem;
    font-family: var(--font-body);
  }

  .skip:hover {
    color: var(--text);
  }

  .skip:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }

  /* Step counter — small, mono, tabular-nums. */
  .counter {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: var(--space-1);
    font-family: var(--font-mono);
    color: var(--text-muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .counter-num {
    color: var(--text-2);
  }

  /* ---------- Hero / welcome / enable ---------- */
  .hero {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-hero);
    padding: var(--space-4) var(--space-4);
    box-shadow: var(--shadow-hero);
  }

  .hero h2 {
    margin: 0 0 var(--space-2);
    font-family: var(--font-display);
    font-size: clamp(1.6rem, 3.5vw, 2rem);
    font-weight: 600;
    line-height: 1.15;
    color: var(--text);
  }

  .hero-sub {
    margin: 0 0 var(--space-3);
    color: var(--text);
    font-size: 1.05rem;
    line-height: 1.45;
    font-weight: 500;
  }

  .hero-copy {
    margin: 0 0 var(--space-2);
    color: var(--text-2);
    font-size: 0.95rem;
    line-height: 1.55;
  }

  /* ---------- Evidence cards ---------- */
  .evidence h2 {
    margin: 0 0 var(--space-1);
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--text);
  }

  .lede {
    margin: 0 0 var(--space-3);
    color: var(--text-2);
    font-size: 0.95rem;
    line-height: 1.55;
  }

  .cards {
    display: grid;
    gap: var(--space-2);
  }

  .card {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-card);
    padding: var(--space-2) var(--space-3);
  }

  .card h3 {
    margin: 0 0 var(--space-1);
    font-size: 1rem;
    font-weight: 600;
    color: var(--text);
    font-family: var(--font-body);
  }

  .card p {
    margin: 0 0 var(--space-1);
    color: var(--text-2);
    font-size: 0.92rem;
    line-height: 1.55;
  }

  .ext {
    display: inline-block;
    margin-top: var(--space-half);
    color: var(--accent);
    font-size: 0.85rem;
  }

  .ext:hover {
    text-decoration: underline;
  }

  /* "How we audit our citations" link strip — same shape as the
     Settings cite-audit line (Dr. Lin's G1). */
  .audit-link {
    margin: var(--space-3) 0 0;
    padding-top: var(--space-2);
    border-top: 1px solid var(--hairline);
    color: var(--text-2);
    font-size: 0.85rem;
    line-height: 1.45;
  }

  .audit-link a {
    color: var(--accent);
  }

  .audit-link a:hover {
    text-decoration: underline;
  }

  /* ---------- Intervals form ---------- */
  .intervals h2 {
    margin: 0 0 var(--space-1);
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--text);
  }

  .knob {
    background: var(--ink-2);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    padding: var(--space-2) var(--space-3);
  }

  .knob-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .knob-row label {
    font-size: 0.92rem;
    color: var(--text);
    font-weight: 500;
    margin: 0;
  }

  .knob-input {
    display: flex;
    align-items: baseline;
    gap: var(--space-half);
  }

  .knob-input input {
    appearance: none;
    background: var(--ink-3);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-input);
    padding: 0.4rem 0.6rem;
    color: var(--text);
    font: inherit;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    width: 5rem;
    text-align: right;
  }

  .knob-input input:focus {
    border-color: var(--accent);
  }

  .knob-input input:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 1px;
  }

  .unit {
    color: var(--text-muted);
    font-size: 0.85rem;
    font-family: var(--font-mono);
  }

  .hint {
    margin: var(--space-half) 0 0;
    color: var(--text-muted);
    font-size: 0.85rem;
    line-height: 1.5;
  }

  /* ---------- Cite-line (T1/T2/T3 templates) ---------- */
  /* Style matches the existing Settings.svelte .cite-line (mono,
     small, --text-muted). All citations on every screen follow the
     T1/T2/T3 templates verbatim. */
  :global(.onboarding) .cite-line,
  .cite-line {
    margin: var(--space-1) 0 0;
    color: var(--text-muted);
    font-size: 0.8rem;
    font-family: var(--font-mono);
    font-style: normal;
    line-height: 1.45;
  }

  code {
    background: var(--ink);
    padding: 0 0.25rem;
    border-radius: 3px;
    font-size: 0.85em;
    font-family: var(--font-mono);
  }

  /* ---------- Tray glyph (Enable screen) ---------- */
  /* Inline SVG uses `currentColor` so the icon picks up the body
     text colour — no hex literals, no emoji. */
  .tray-glyph {
    display: inline-block;
    width: 1em;
    height: 1em;
    vertical-align: -0.15em;
    color: var(--text-2);
  }

  /* ---------- Action rows ---------- */
  .actions {
    display: flex;
    gap: var(--space-1);
    flex-wrap: wrap;
  }

  .primary-row {
    margin-top: var(--space-3);
    /* Send the Back button to the left and the primary CTA to the
       right. When only one CTA exists
       (welcome), the row still aligns it to the right via the
       `justify-content: flex-end` override below. */
    justify-content: flex-end;
  }

  /* When there's only a primary button (welcome), keep it on the
     right. */
  .actions:has(> :only-child) {
    justify-content: flex-end;
  }

  /* Buttons — primary / ghost, exact design tokens. */
  .actions button {
    appearance: none;
    font: inherit;
    cursor: pointer;
    border-radius: var(--radius-card);
    padding: 0.625rem 1rem;
    font-size: 1rem;
  }

  .actions button.primary {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }

  .actions button.primary:hover {
    background: var(--accent-hi);
    border-color: var(--accent-hi);
  }

  .actions button.primary:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }

  .actions button.ghost {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-muted);
  }

  .actions button.ghost:hover {
    color: var(--text);
  }

  .actions button.ghost:focus-visible {
    outline: var(--focus-ring);
    outline-offset: 2px;
  }
</style>
