<script lang="ts">
  // Stats — minimal daily summary for v1. CSS-only bar chart for the
  // last 7 days; no chart library (per implementation plan §10).
  // Rich analytics (streaks, charts, exports) is roadmap.
  //
  // Same `$state` vs `state`-name collision as Settings.svelte: the
  // rune and the imported mirror are visually too similar, so we
  // import the mirror under an alias.

  import { onMount } from "svelte";
  import { refreshRecent, state as mirror } from "../lib/stores.svelte";
  import type { DailyAggregate } from "../lib/types";

  const DAYS = 7;

  let dailyBuckets: DailyAggregate[] = $state([]);
  let hydrationByDay: { day: string; ml: number }[] = $state([]);

  onMount(async () => {
    await refreshRecent(DAYS);
    // Build the per-day bar from the recent sessions + hydration.
    dailyBuckets = bucketDailyAggregates(mirror.recentSessions, mirror.today);
    hydrationByDay = bucketHydration(mirror.recentHydration);
  });

  // Compute per-day totals from the raw events (the storage layer
  // retains the full history; we reconstruct here for the bar chart).
  function bucketDailyAggregates(
    sessions: { started_at: { unixMs: number }; active_ms: number }[],
    today: DailyAggregate,
  ): DailyAggregate[] {
    const byDay = new Map<string, DailyAggregate>();
    for (const s of sessions) {
      const day = new Date(s.started_at.unixMs);
      const key = day.toISOString().slice(0, 10);
      const cur =
        byDay.get(key) ??
        ({
          day: key,
          active_ms: 0,
          breaks_taken: 0,
          breaks_skipped: 0,
          water_ml: 0,
          water_goal_ml: today.water_goal_ml,
          posture_prompts: 0,
        } satisfies DailyAggregate);
      cur.active_ms = (cur.active_ms ?? 0) + s.active_ms;
      byDay.set(key, cur);
    }
    // Fill in any days we didn't see a session for.
    const out: DailyAggregate[] = [];
    const dayMs = 24 * 60 * 60 * 1000;
    for (let i = DAYS - 1; i >= 0; i--) {
      const d = new Date(Date.now() - i * dayMs);
      const key = d.toISOString().slice(0, 10);
      out.push(byDay.get(key) ?? todayForDay(key, today));
    }
    return out;
  }

  function todayForDay(day: string, today: DailyAggregate): DailyAggregate {
    return {
      ...today,
      day,
      active_ms: 0,
      breaks_taken: 0,
      breaks_skipped: 0,
    };
  }

  function bucketHydration(entries: { logged_at: { unixMs: number }; amount_ml: number }[]) {
    const byDay = new Map<string, number>();
    for (const h of entries) {
      const d = new Date(h.logged_at.unixMs);
      const key = d.toISOString().slice(0, 10);
      byDay.set(key, (byDay.get(key) ?? 0) + h.amount_ml);
    }
    const dayMs = 24 * 60 * 60 * 1000;
    const out: { day: string; ml: number }[] = [];
    for (let i = DAYS - 1; i >= 0; i--) {
      const d = new Date(Date.now() - i * dayMs);
      const key = d.toISOString().slice(0, 10);
      out.push({ day: key, ml: byDay.get(key) ?? 0 });
    }
    return out;
  }

  const maxBreaks = $derived(
    Math.max(1, ...dailyBuckets.map((d) => d.breaks_taken + d.breaks_skipped)),
  );
  const maxWater = $derived(Math.max(1, ...hydrationByDay.map((h) => h.ml)));
</script>

<section class="stats">
  <h2>Last {DAYS} days</h2>

  <article class="chart">
    <h3>Breaks per day</h3>
    <div class="bar-row">
      {#each dailyBuckets as day}
        <div class="bar-cell" title={day.day}>
          <div
            class="bar bar-taken"
            style:height={`${((day.breaks_taken + day.breaks_skipped) / maxBreaks) * 100}%`}
          ></div>
        </div>
      {/each}
    </div>
    <div class="legend">
      <span class="day-label">most recent</span>
      <span class="day-label">oldest</span>
    </div>
  </article>

  <article class="chart">
    <h3>Hydration per day</h3>
    <div class="bar-row">
      {#each hydrationByDay as h}
        <div class="bar-cell" title={`${h.day} — ${h.ml} ml`}>
          <div class="bar bar-water" style:height={`${(h.ml / maxWater) * 100}%`}></div>
        </div>
      {/each}
    </div>
  </article>

  <article class="totals">
    <h3>Today</h3>
    <ul>
      <li>{mirror.today.breaks_taken} breaks taken</li>
      <li>{mirror.today.breaks_skipped} breaks skipped</li>
      <li>{mirror.today.water_ml} / {mirror.today.water_goal_ml} ml water</li>
      <li>{mirror.today.posture_prompts} posture nudges</li>
    </ul>
    <p class="dim">
      Rich charts and streaks are post-v1 — see docs/M6.md.
    </p>
  </article>
</section>

<style>
  .stats {
    display: grid;
    gap: 1rem;
  }
  h2 {
    margin: 0 0 0.25rem;
    font-size: 1.25rem;
  }
  article {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }
  article h3 {
    margin: 0 0 0.75rem;
    font-size: 0.95rem;
    color: #8b949e;
  }
  .bar-row {
    display: flex;
    gap: 0.25rem;
    align-items: flex-end;
    height: 120px;
  }
  .bar-cell {
    flex: 1 1 auto;
    height: 100%;
    background: #0d1117;
    border-radius: 4px;
    position: relative;
    display: flex;
    align-items: flex-end;
  }
  .bar {
    width: 100%;
    border-radius: 4px 4px 0 0;
    transition: height 200ms ease-in-out;
  }
  .bar-taken {
    background: linear-gradient(to top, #1f6feb, #58a6ff);
  }
  .bar-water {
    background: linear-gradient(to top, #1f6feb, #4dd0e1);
  }
  .legend {
    display: flex;
    justify-content: space-between;
    margin-top: 0.5rem;
    color: #8b949e;
    font-size: 0.75rem;
  }
  .day-label {
    margin-top: 0.25rem;
  }
  ul {
    margin: 0;
    padding-left: 1.125rem;
    color: #c9d1d9;
  }
  .dim {
    margin: 0.75rem 0 0;
    color: #8b949e;
    font-size: 0.85rem;
    font-style: italic;
  }
</style>
