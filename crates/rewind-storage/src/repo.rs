//! `SqliteHistoryRepo` — the concrete `HistoryRepo` implementation
//! backed by `sqlx::SqlitePool`. See implementation plan §8a and
//! §16.
//!
//! The storage crate has **no** `tauri` dependency; the shell wires
//! a fully-built `SqlitePool` to the constructor. This is in line
//! with the plan §4 ("Used directly, not via `tauri-plugin-sql`").
//!
//! Plain `sqlx::query(...)` bindings are used everywhere rather than
//! the `sqlx::query!` macros so a build-time `DATABASE_URL` is not
//! needed (the plan §16 calls this out explicitly).

use std::path::Path;

use async_trait::async_trait;
use rewind_core::clock::Timestamp;
use rewind_core::model::{
    aggregate::DailyAggregate,
    break_record::{BreakOutcome as CoreBreakOutcome, BreakRecord},
    hydration::{HydrationEntry, HydrationSource as CoreHydrationSource},
    session::{SessionEndReason as CoreSessionEndReason, SessionRecord},
};
use rewind_core::ports::{HistoryRepo, HistoryRepoError, RepoResult};
use rewind_core::session::state::BreakKind as CoreBreakKind;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo};
use time::{Date, OffsetDateTime, Time};
use tracing::warn;

use crate::storage_error::{Result as StorageResult, StorageError};

/// Concrete `HistoryRepo` against an `sqlx::SqlitePool`. Cheap to
/// `Clone` (`SqlitePool` is internally an `Arc`).
#[derive(Debug, Clone)]
pub struct SqliteHistoryRepo {
    pool: SqlitePool,
}

impl SqliteHistoryRepo {
    /// Wrap an existing pool. The pool must already have migrations
    /// applied (use [`open_db`] for the full setup).
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Borrow the inner pool. Useful for tests and for ad-hoc shell
    /// queries (export / clear).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ---- Ad-hoc shell-side operations (used by IPC for Stats/Settings) ----

    /// Read the most recent `n` days of session rows, newest first.
    /// Sessions whose `ended_at` is `NULL` (in-progress) are included.
    /// Pass `n == 0` to read *all* sessions (no cutoff).
    pub async fn recent_sessions(&self, days: u32) -> StorageResult<Vec<SessionRecord>> {
        let rows = if days == 0 {
            sqlx::query(
                "SELECT id, started_at, ended_at, active_ms, end_reason \
                 FROM session \
                 ORDER BY started_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            let cutoff =
                OffsetDateTime::now_utc().date() - time::Duration::days(days as i64);
            let cutoff_ms = cutoff_millis(cutoff);
            sqlx::query(
                "SELECT id, started_at, ended_at, active_ms, end_reason \
                 FROM session \
                 WHERE started_at >= ?1 \
                 ORDER BY started_at DESC",
            )
            .bind(cutoff_ms)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(session_from_row)
            .collect::<Vec<_>>())
    }

    /// Read the most recent `n` days of hydration entries, newest first.
    pub async fn recent_hydration(
        &self,
        days: u32,
    ) -> StorageResult<Vec<HydrationEntry>> {
        let rows = if days == 0 {
            sqlx::query(
                "SELECT id, logged_at, amount_ml, source \
                 FROM hydration_log \
                 ORDER BY logged_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            let cutoff =
                OffsetDateTime::now_utc().date() - time::Duration::days(days as i64);
            let cutoff_ms = cutoff_millis(cutoff);
            sqlx::query(
                "SELECT id, logged_at, amount_ml, source \
                 FROM hydration_log \
                 WHERE logged_at >= ?1 \
                 ORDER BY logged_at DESC",
            )
            .bind(cutoff_ms)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(hydration_from_row)
            .collect::<Vec<_>>())
    }

    /// JSON dump of every history table. Used by the Settings
    /// "Export data" button. The shape is stable enough for v1 —
    /// `M6.md` documents it.
    pub async fn export_json(&self) -> StorageResult<serde_json::Value> {
        let sessions = sqlx::query("SELECT * FROM session ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        let breaks = sqlx::query("SELECT * FROM break_record ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        let hydration = sqlx::query("SELECT * FROM hydration_log ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        let aggregate = sqlx::query("SELECT * FROM daily_aggregate ORDER BY day")
            .fetch_all(&self.pool)
            .await?;

        Ok(serde_json::json!({
            "version": 1,
            "exported_at": OffsetDateTime::now_utc().unix_timestamp(),
            "session":         rows_to_json(&sessions),
            "break_record":    rows_to_json(&breaks),
            "hydration_log":   rows_to_json(&hydration),
            "daily_aggregate": rows_to_json(&aggregate),
        }))
    }

    /// Wipe all history. Used by the Settings "Clear history" button.
    /// Wrapped in a transaction so the schema itself survives.
    pub async fn clear(&self) -> StorageResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM break_record")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM hydration_log")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM daily_aggregate")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM session")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl HistoryRepo for SqliteHistoryRepo {
    async fn append_session(&self, s: &SessionRecord) -> RepoResult<i64> {
        let end_reason: Option<String> = s.end_reason.map(|r| r.as_str().to_string());
        let ended_at: Option<i64> = s.ended_at.map(|t| t.0);
        let result = sqlx::query(
            "INSERT INTO session (started_at, ended_at, active_ms, end_reason) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(s.started_at.0)
        .bind(ended_at)
        .bind(s.active_ms as i64)
        .bind(end_reason)
        .execute(&self.pool)
        .await
        .map_err(|e| HistoryRepoError::Backend(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn append_break(&self, b: &BreakRecord) -> RepoResult<i64> {
        let result = sqlx::query(
            "INSERT INTO break_record \
             (session_id, kind, scheduled_at, started_at, ended_at, outcome, exercise_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(b.session_id)
        .bind(b.kind.as_str())
        .bind(b.scheduled_at.0)
        .bind(b.started_at.map(|t| t.0))
        .bind(b.ended_at.map(|t| t.0))
        .bind(b.outcome.as_str())
        .bind(b.exercise_id.clone())
        .execute(&self.pool)
        .await
        .map_err(|e| HistoryRepoError::Backend(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn append_hydration(&self, h: &HydrationEntry) -> RepoResult<i64> {
        let result = sqlx::query(
            "INSERT INTO hydration_log (logged_at, amount_ml, source) \
             VALUES (?1, ?2, ?3)",
        )
        .bind(h.logged_at.0)
        .bind(h.amount_ml as i64)
        .bind(h.source.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| HistoryRepoError::Backend(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn upsert_daily(&self, a: &DailyAggregate) -> RepoResult<()> {
        sqlx::query(
            "INSERT INTO daily_aggregate (\
                day, active_ms, breaks_taken, breaks_skipped, \
                water_ml, water_goal_ml, posture_prompts\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(day) DO UPDATE SET \
                active_ms       = excluded.active_ms, \
                breaks_taken    = excluded.breaks_taken, \
                breaks_skipped  = excluded.breaks_skipped, \
                water_ml        = excluded.water_ml, \
                water_goal_ml   = excluded.water_goal_ml, \
                posture_prompts = excluded.posture_prompts",
        )
        .bind(&a.day)
        .bind(a.active_ms as i64)
        .bind(a.breaks_taken as i64)
        .bind(a.breaks_skipped as i64)
        .bind(a.water_ml as i64)
        .bind(a.water_goal_ml as i64)
        .bind(a.posture_prompts as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| HistoryRepoError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn today(&self, now: Timestamp) -> RepoResult<DailyAggregate> {
        let day = local_day_string(now);
        let row = sqlx::query(
            "SELECT day, active_ms, breaks_taken, breaks_skipped, \
                    water_ml, water_goal_ml, posture_prompts \
             FROM daily_aggregate WHERE day = ?1",
        )
        .bind(&day)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HistoryRepoError::Backend(e.to_string()))?;

        Ok(match row {
            Some(r) => aggregate_from_row(&r),
            None => DailyAggregate::empty_for_today(day, default_water_goal()),
        })
    }
}

// ---------------------------------------------------------------------------
// Module-level helpers — pure functions, easy to unit-test.
// ---------------------------------------------------------------------------

/// Default water goal used by `today()` when no row exists yet. The
/// shell passes through `engine.config().hydration.goal_ml` for the
/// dashboard, so this default is only an initial bootstrap value.
fn default_water_goal() -> u32 {
    2000
}

/// Render a `Timestamp` as the local-day bucket string (`YYYY-MM-DD`).
/// Falls back to UTC date when the local offset is unavailable (the
/// shell is the only consumer and always has TZ data).
pub fn local_day_string(now: Timestamp) -> String {
    let secs = now.0.div_euclid(1_000);
    let utc = OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local_date: Date = utc.to_offset(offset).date();
    format!(
        "{:04}-{:02}-{:02}",
        local_date.year(),
        u8::from(local_date.month()),
        local_date.day()
    )
}

/// Render the `started_at` / `logged_at` etc. column cutoff for the
/// "recent N days" queries. We compute the UTC midnight of the date.
fn cutoff_millis(date: Date) -> i64 {
    let midnight = match Time::from_hms(0, 0, 0) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let dt = date.with_time(midnight).assume_utc();
    (dt.unix_timestamp_nanos() / 1_000_000) as i64
}

// ---------------------------------------------------------------------------
// Row → DTO converters.
// ---------------------------------------------------------------------------

fn session_from_row(row: sqlx::sqlite::SqliteRow) -> SessionRecord {
    let id: i64 = row.get("id");
    let started_at: i64 = row.get("started_at");
    let ended_at: Option<i64> = row.get("ended_at");
    let active_ms: i64 = row.get("active_ms");
    let end_reason: Option<String> = row.get("end_reason");
    SessionRecord {
        id: Some(id),
        started_at: Timestamp(started_at),
        ended_at: ended_at.map(Timestamp),
        active_ms: active_ms.max(0) as u64,
        end_reason: end_reason.as_deref().and_then(parse_end_reason),
    }
}

#[allow(dead_code)]
fn break_from_row(row: sqlx::sqlite::SqliteRow) -> BreakRecord {
    let id: i64 = row.get("id");
    let session_id: i64 = row.get("session_id");
    let kind: String = row.get("kind");
    let scheduled_at: i64 = row.get("scheduled_at");
    let started_at: Option<i64> = row.get("started_at");
    let ended_at: Option<i64> = row.get("ended_at");
    let outcome: String = row.get("outcome");
    let exercise_id: Option<String> = row.get("exercise_id");
    BreakRecord {
        id: Some(id),
        session_id,
        kind: parse_break_kind(&kind),
        scheduled_at: Timestamp(scheduled_at),
        started_at: started_at.map(Timestamp),
        ended_at: ended_at.map(Timestamp),
        outcome: parse_break_outcome(&outcome),
        exercise_id,
    }
}

fn hydration_from_row(row: sqlx::sqlite::SqliteRow) -> HydrationEntry {
    let id: i64 = row.get("id");
    let logged_at: i64 = row.get("logged_at");
    let amount_ml: i64 = row.get("amount_ml");
    let source: String = row.get("source");
    HydrationEntry {
        id: Some(id),
        logged_at: Timestamp(logged_at),
        amount_ml: amount_ml.max(0) as u32,
        source: parse_hydration_source(&source),
    }
}

fn aggregate_from_row(row: &sqlx::sqlite::SqliteRow) -> DailyAggregate {
    let day: String = row.get("day");
    let active_ms: i64 = row.get("active_ms");
    let breaks_taken: i64 = row.get("breaks_taken");
    let breaks_skipped: i64 = row.get("breaks_skipped");
    let water_ml: i64 = row.get("water_ml");
    let water_goal_ml: i64 = row.get("water_goal_ml");
    let posture_prompts: i64 = row.get("posture_prompts");
    DailyAggregate {
        day,
        active_ms: active_ms.max(0) as u64,
        breaks_taken: breaks_taken.max(0) as u32,
        breaks_skipped: breaks_skipped.max(0) as u32,
        water_ml: water_ml.max(0) as u32,
        water_goal_ml: water_goal_ml.max(0) as u32,
        posture_prompts: posture_prompts.max(0) as u32,
    }
}

fn parse_end_reason(s: &str) -> Option<CoreSessionEndReason> {
    Some(match s {
        "completed" => CoreSessionEndReason::Completed,
        "idle_reset" => CoreSessionEndReason::IdleReset,
        "quit" => CoreSessionEndReason::Quit,
        _ => {
            warn!("unknown session end_reason: {s}");
            return None;
        }
    })
}

fn parse_break_kind(s: &str) -> CoreBreakKind {
    match s {
        "rest" => CoreBreakKind::Rest,
        // Plan §8a: "micro" | "rest". Anything else falls back to
        // Micro — the column is a tag, not user input.
        _ => CoreBreakKind::Micro,
    }
}

fn parse_break_outcome(s: &str) -> CoreBreakOutcome {
    match s {
        "completed" => CoreBreakOutcome::Completed,
        "skipped" => CoreBreakOutcome::Skipped,
        "postponed" => CoreBreakOutcome::Postponed,
        "natural" => CoreBreakOutcome::Natural,
        _ => {
            warn!("unknown break outcome: {s}; falling back to completed");
            CoreBreakOutcome::Completed
        }
    }
}

fn parse_hydration_source(s: &str) -> CoreHydrationSource {
    match s {
        "manual" => CoreHydrationSource::Manual,
        _ => CoreHydrationSource::Reminder,
    }
}

fn rows_to_json(rows: &[sqlx::sqlite::SqliteRow]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let mut map = serde_json::Map::new();
            for (i, col) in r.columns().iter().enumerate() {
                let key = col.name().to_string();
                let val = decode_column(r, i, col.type_info().name());
                map.insert(key, val);
            }
            serde_json::Value::Object(map)
        })
        .collect();
    serde_json::Value::Array(arr)
}

fn decode_column(
    row: &sqlx::sqlite::SqliteRow,
    i: usize,
    ty: &str,
) -> serde_json::Value {
    // Hand-decode the columns we know about rather than pull a
    // generic deserializer; keeps the JSON shape stable across
    // sqlx version bumps.
    match ty {
        "INTEGER" => {
            let opt: Option<i64> = row.try_get(i).unwrap_or(None);
            opt.map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        }
        "TEXT" => {
            let opt: Option<String> = row.try_get(i).unwrap_or(None);
            opt.map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        }
        "REAL" => {
            let opt: Option<f64> = row.try_get(i).unwrap_or(None);
            opt.and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        _ => {
            // Best-effort: stringify.
            serde_json::Value::String(format!("<{}>", ty))
        }
    }
}

// ---------------------------------------------------------------------------
// Connect helpers — used by the shell at boot.
// ---------------------------------------------------------------------------

/// Connect to a file path. The connection is opened with
/// `create_if_missing` and `journal_mode = WAL` so multiple readers
/// (e.g. an export run while the tick loop is writing) don't block
/// each other.
pub async fn connect(path: &Path) -> StorageResult<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect_with(opts)
        .await?;
    Ok(pool)
}

/// Connect an in-memory SQLite pool. Used by tests.
pub async fn connect_memory() -> StorageResult<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

// ---------------------------------------------------------------------------
// Re-export local-day helper for the shell wiring module.
// ---------------------------------------------------------------------------
pub use self::local_day_string as local_day_string_exported;

/// Migrate the schema from the bundled `migrations/` directory.
/// Idempotent — safe to call on every boot.
///
/// `sqlx::migrate!()` 0.8 quirks: the macro refuses to resolve a
/// bare `migrations` literal (treating it as a "file" relative to
/// nothing). Using `./migrations` works because `parent()` returns
/// `Some("")` — the form the macro actually accepts.
pub async fn migrate(pool: &SqlitePool) -> StorageResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| StorageError::Migration(e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
