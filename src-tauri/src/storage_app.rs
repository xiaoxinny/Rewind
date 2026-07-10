//! `StorageApp` — the shell-side handle to the SQLite-backed history
//! repo. Wraps the `rewind_storage::SqliteHistoryRepo` and adds:
//!
//! *   an in-memory mirror of `daily_aggregate` for cheap reads,
//! *   a session-id cache so `BreakRecord`s can carry a valid FK.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rewind_core::clock::Timestamp;
use rewind_core::model::aggregate::DailyAggregate;
use rewind_core::ports::HistoryRepo as _;
use rewind_storage::{connect, migrate, SqliteHistoryRepo, StorageError};
use thiserror::Error;
use time::OffsetDateTime;

use crate::storage_helpers::local_day_string;

#[derive(Debug, Error)]
pub enum StorageAppError {
    #[error("storage backend: {0}")]
    Backend(#[from] StorageError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("history repo: {0}")]
    Repo(String),
}

/// Cheap-to-clone handle the shell uses to reach the SQLite-backed
/// history repo. Internally an `Arc` around the pool + an in-memory
/// `DailyAggregate` mirror that is flushed to disk on demand.
#[derive(Clone)]
pub struct StorageApp {
    inner: Arc<StorageAppInner>,
}

struct StorageAppInner {
    repo: SqliteHistoryRepo,
    /// Last-known mirror of today's `daily_aggregate` row. The tick
    /// loop writes through here; the dashboard reads it via IPC.
    today: Mutex<DailyAggregate>,
    /// Latest session id issued for this process.
    session_id: Mutex<Option<i64>>,
}

impl StorageApp {
    /// Open the SQLite pool at `db_path`. Runs migrations before
    /// returning. Creates parent directories as needed.
    pub async fn open(db_path: PathBuf) -> Result<Self, StorageAppError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let pool = connect(&db_path).await?;
        migrate(&pool).await?;

        // Seed today's aggregate row mirror.
        let now = wall_now();
        let mut today = DailyAggregate::empty_for_today(local_day_string(now), 2000);
        today.active_ms = 0;
        let repo = SqliteHistoryRepo::new(pool);
        if let Ok(existing) = repo.today(now).await {
            today = existing;
        }

        Ok(Self {
            inner: Arc::new(StorageAppInner {
                repo,
                today: Mutex::new(today),
                session_id: Mutex::new(None),
            }),
        })
    }

    /// Borrow the inner repo. Used by the IPC layer for ad-hoc
    /// reads (recent sessions, recent hydration, export).
    pub fn repo(&self) -> &SqliteHistoryRepo {
        &self.inner.repo
    }

    /// Cheap `Arc`-clone of the inner repo for IPC callers that want
    /// to hold a typed handle.
    pub fn clone_repo(&self) -> SqliteHistoryRepo {
        self.inner.repo.clone()
    }

    /// Read today's cached aggregate. Cheap; the snapshot mirrors the
    /// last upsert_daily() we issued.
    pub fn today_snapshot(&self) -> DailyAggregate {
        let guard = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    /// Append a session row + remember its id for joins later.
    pub async fn record_session_started(
        &self,
        now: Timestamp,
    ) -> Result<i64, StorageAppError> {
        use rewind_core::model::session::SessionRecord;
        let id = self
            .inner
            .repo
            .append_session(&SessionRecord::new(now))
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        let mut guard = self
            .inner
            .session_id
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(id);
        Ok(id)
    }

    /// Issue a new session id if none has been recorded yet. Returns
    /// the live session id without creating one when already set.
    pub async fn ensure_session_id(&self) -> Result<i64, StorageAppError> {
        let cached = {
            let guard = self
                .inner
                .session_id
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *guard
        };
        if let Some(id) = cached {
            return Ok(id);
        }
        self.record_session_started(wall_now()).await
    }

    /// Persist a `BreakRecord` row. Auto-creates a session row if no
    /// session is live yet. Mirrors the break outcome into today's
    /// rollup.
    pub async fn record_break(
        &self,
        mut rec: rewind_core::model::break_record::BreakRecord,
    ) -> Result<i64, StorageAppError> {
        let session_id = self.ensure_session_id().await?;
        rec.session_id = session_id;
        rec.id = None;

        let outcome = rec.outcome;
        use rewind_core::model::break_record::BreakOutcome as MBo;
        let outcome_is_completed = matches!(outcome, MBo::Completed | MBo::Natural);

        let id = self
            .inner
            .repo
            .append_break(&rec)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;

        // Mirror into today's rollup.
        let snapshot = {
            let mut today = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
            if outcome_is_completed {
                today.breaks_taken = today.breaks_taken.saturating_add(1);
            } else {
                today.breaks_skipped = today.breaks_skipped.saturating_add(1);
            }
            today.clone()
        };
        self.inner
            .repo
            .upsert_daily(&snapshot)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        Ok(id)
    }

    /// Record a hydration entry and update today's rollup.
    pub async fn record_hydration(
        &self,
        entry: rewind_core::model::hydration::HydrationEntry,
    ) -> Result<i64, StorageAppError> {
        let amount = entry.amount_ml;
        let id = self
            .inner
            .repo
            .append_hydration(&entry)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        let snapshot = {
            let mut today = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
            today.water_ml = today.water_ml.saturating_add(amount);
            today.clone()
        };
        self.inner
            .repo
            .upsert_daily(&snapshot)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        Ok(id)
    }

    /// Increment today's `posture_prompts` counter (in-memory). Call
    /// [`flush_today`] to persist.
    pub fn bump_posture_prompt(&self) {
        let mut today = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
        today.posture_prompts = today.posture_prompts.saturating_add(1);
    }

    /// Persist the current `today` mirror to the DB. Idempotent; also
    /// refreshes `today.day` to handle the midnight-rollover edge.
    pub async fn flush_today(&self, now: Timestamp) -> Result<(), StorageAppError> {
        let snapshot = {
            let mut today = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
            today.day = local_day_string(now);
            today.clone()
        };
        self.inner
            .repo
            .upsert_daily(&snapshot)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        Ok(())
    }

    /// Read today's row from the disk (bypassing the in-memory
    /// mirror). Useful after a manual `clear()` so the IPC layer can
    /// resync.
    pub async fn reload_today(&self) -> Result<(), StorageAppError> {
        let now = wall_now();
        let row = self
            .inner
            .repo
            .today(now)
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        let mut today = self.inner.today.lock().unwrap_or_else(|e| e.into_inner());
        *today = row;
        Ok(())
    }

    /// Read the live session id, if one was started.
    pub fn current_session_id(&self) -> Option<i64> {
        *self
            .inner
            .session_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Wipe all history (Settings → Clear). After this, the in-memory
    /// today mirror is also reset to a fresh zero row.
    pub async fn clear(&self) -> Result<(), StorageAppError> {
        self.inner
            .repo
            .clear()
            .await
            .map_err(|e| StorageAppError::Repo(format!("{e:?}")))?;
        self.reload_today().await
    }
}

/// Wall-clock now as a `Timestamp` (unix ms). Convenience helper so
/// callers don't have to import `time` for the common case.
fn wall_now() -> Timestamp {
    let nanos = OffsetDateTime::now_utc().unix_timestamp_nanos();
    Timestamp((nanos / 1_000_000) as i64)
}
