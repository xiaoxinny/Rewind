-- Initial schema for `rewind-storage` (M6 — the ship milestone).
--
-- Schema mirrors `rewind_core::model::*` 1-for-1. See implementation
-- plan §8a. `daily_aggregate` is a denormalized rollup (cheap "today"
-- reads for tray/dashboard), updated on each relevant event and fully
-- reconstructable from the raw tables — treat it as a cache, not a
-- source of truth.

CREATE TABLE IF NOT EXISTS session (
    id          INTEGER PRIMARY KEY,
    started_at  INTEGER NOT NULL,            -- unix ms
    ended_at    INTEGER,                     -- null = in progress
    active_ms   INTEGER NOT NULL DEFAULT 0,  -- accumulated non-idle time
    end_reason  TEXT                         -- 'completed' | 'idle_reset' | 'quit'
);

CREATE TABLE IF NOT EXISTS break_record (
    id           INTEGER PRIMARY KEY,
    session_id   INTEGER REFERENCES session(id),
    kind         TEXT NOT NULL,              -- 'micro' | 'rest'
    scheduled_at INTEGER NOT NULL,
    started_at   INTEGER,
    ended_at     INTEGER,
    outcome      TEXT NOT NULL,              -- 'completed' | 'skipped' | 'postponed' | 'natural'
    exercise_id  TEXT                        -- which guided exercise (rest breaks)
);

CREATE TABLE IF NOT EXISTS hydration_log (
    id         INTEGER PRIMARY KEY,
    logged_at  INTEGER NOT NULL,
    amount_ml  INTEGER NOT NULL,
    source     TEXT NOT NULL                 -- 'reminder' | 'manual'
);

CREATE TABLE IF NOT EXISTS daily_aggregate (
    day             TEXT PRIMARY KEY,        -- 'YYYY-MM-DD' local
    active_ms       INTEGER NOT NULL DEFAULT 0,
    breaks_taken    INTEGER NOT NULL DEFAULT 0,
    breaks_skipped  INTEGER NOT NULL DEFAULT 0,
    water_ml        INTEGER NOT NULL DEFAULT 0,
    water_goal_ml   INTEGER NOT NULL DEFAULT 0,
    posture_prompts INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_break_session ON break_record(session_id);
CREATE INDEX IF NOT EXISTS idx_hydration_day  ON hydration_log(logged_at);
