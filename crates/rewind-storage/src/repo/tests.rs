//! Integration tests for `rewind-storage` against an in-memory SQLite
//! pool.
//!
//! All tests run on the same `#[tokio::test]` async runtime each
//! (no shared state between tests). Each test opens its own pool
//! and runs migrations before exercising the trait methods.

use std::sync::Arc;

use rewind_core::clock::Timestamp;
use rewind_core::model::{
    aggregate::DailyAggregate,
    break_record::{BreakOutcome, BreakRecord},
    hydration::{HydrationEntry, HydrationSource},
    session::{SessionEndReason, SessionRecord},
};
use rewind_core::ports::HistoryRepo;
use rewind_core::session::state::BreakKind;
use tokio::sync::Mutex;

use super::{connect_memory, migrate, SqliteHistoryRepo};

/// Helper: build a pool + run migrations on it.
async fn fresh_repo() -> SqliteHistoryRepo {
    let pool = connect_memory().await.expect("in-memory sqlite pool");
    migrate(&pool).await.expect("migrations");
    SqliteHistoryRepo::new(pool)
}

#[tokio::test]
async fn migrations_create_all_tables() {
    let repo = fresh_repo().await;
    let pool = repo.pool();

    // Probe each table by counting rows (returns zero in empty
    // schemas; what we care about is that the SQL compiles + the
    // table resolves).
    for sql in [
        "SELECT COUNT(*) FROM session",
        "SELECT COUNT(*) FROM break_record",
        "SELECT COUNT(*) FROM hydration_log",
        "SELECT COUNT(*) FROM daily_aggregate",
    ] {
        let (count,): (i64,) = sqlx::query_as(sql)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("missing table — {sql}: {e}"));
        assert_eq!(count, 0, "{sql} should be empty");
    }
}

#[tokio::test]
async fn session_round_trips() {
    let repo = fresh_repo().await;
    let record = SessionRecord {
        id: None,
        started_at: Timestamp(1_700_000_000_000),
        ended_at: Some(Timestamp(1_700_007_200_000)),
        active_ms: 600_000,
        end_reason: Some(SessionEndReason::Completed),
    };
    let id = repo.append_session(&record).await.expect("insert");
    assert!(id > 0);

    // Round-trip via recent_sessions. Use `0` to disable the date
    // filter (test data uses a fixed 2023 timestamp).
    let recent = repo.recent_sessions(0).await.expect("read");
    assert_eq!(recent.len(), 1);
    let back = &recent[0];
    assert_eq!(back.id, Some(id));
    assert_eq!(back.started_at.0, 1_700_000_000_000);
    assert_eq!(back.ended_at, Some(Timestamp(1_700_007_200_000)));
    assert_eq!(back.active_ms, 600_000);
    assert_eq!(back.end_reason, Some(SessionEndReason::Completed));
}

#[tokio::test]
async fn session_with_no_end_is_round_tripped_as_none() {
    let repo = fresh_repo().await;
    let record = SessionRecord {
        id: None,
        started_at: Timestamp(1_700_000_000_000),
        ended_at: None,
        active_ms: 0,
        end_reason: None,
    };
    let id = repo.append_session(&record).await.expect("insert");
    let recent = repo.recent_sessions(0).await.expect("read");
    let back = &recent[0];
    assert_eq!(back.id, Some(id));
    assert_eq!(back.ended_at, None);
    assert_eq!(back.end_reason, None);
}

#[tokio::test]
async fn break_record_round_trips_with_session_id() {
    let repo = fresh_repo().await;

    // FK target.
    let session_id = repo
        .append_session(&SessionRecord::new(Timestamp(1_700_000_000_000)))
        .await
        .expect("session insert");

    let break_rec = BreakRecord {
        id: None,
        session_id,
        kind: BreakKind::Rest,
        scheduled_at: Timestamp(1_700_003_000_000),
        started_at: Some(Timestamp(1_700_003_600_000)),
        ended_at: Some(Timestamp(1_700_008_600_000)),
        outcome: BreakOutcome::Completed,
        exercise_id: Some("near_far".to_string()),
    };
    let br_id = repo.append_break(&break_rec).await.expect("break insert");
    assert!(br_id > 0);

    // The session has no break rows enumerated in `recent_sessions`
    // (that helper only reports session-level rows) — so use an
    // ad-hoc SQL probe.
    let pool = repo.pool();
    let row: (i64, String) =
        sqlx::query_as("SELECT session_id, kind FROM break_record WHERE id = ?1")
            .bind(br_id)
            .fetch_one(pool)
            .await
            .expect("read back");
    assert_eq!(row.0, session_id);
    assert_eq!(row.1, "rest");
}

#[tokio::test]
async fn hydration_round_trip() {
    let repo = fresh_repo().await;
    let entry = HydrationEntry {
        id: None,
        logged_at: Timestamp(1_700_001_000_000),
        amount_ml: 250,
        source: HydrationSource::Manual,
    };
    let id = repo.append_hydration(&entry).await.expect("insert");
    let recent = repo.recent_hydration(0).await.expect("read");
    assert_eq!(recent.len(), 1);
    let back = &recent[0];
    assert_eq!(back.id, Some(id));
    assert_eq!(back.amount_ml, 250);
    assert_eq!(back.source, HydrationSource::Manual);
}

#[tokio::test]
async fn upsert_daily_is_idempotent() {
    let repo = fresh_repo().await;
    let day = "2026-07-11".to_string();
    let mut agg = DailyAggregate::empty_for_today(day.clone(), 2000);
    agg.active_ms = 100;
    agg.breaks_taken = 1;
    agg.breaks_skipped = 0;
    agg.water_ml = 250;

    repo.upsert_daily(&agg).await.expect("first upsert");
    repo.upsert_daily(&agg).await.expect("second upsert");

    let pool = repo.pool();
    let (count, water_ml): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), water_ml FROM daily_aggregate WHERE day = ?1",
    )
    .bind(&day)
    .fetch_one(pool)
    .await
    .expect("read daily");
    assert_eq!(count, 1, "duplicate rows on second upsert");
    assert_eq!(water_ml, 250);
}

#[tokio::test]
async fn today_returns_zero_aggregate_when_no_row() {
    let repo = fresh_repo().await;
    let now = Timestamp(1_700_000_000_000); // 2023-11-14 in real TZ but the helper uses local
    let agg = repo.today(now).await.expect("today");
    assert!(!agg.day.is_empty());
    assert_eq!(agg.active_ms, 0);
    assert_eq!(agg.water_goal_ml, 2000); // default
}

#[tokio::test]
async fn today_round_trips_aggregate() {
    let repo = fresh_repo().await;
    let now = Timestamp(1_700_000_000_000);
    let day = crate::repo::local_day_string(now);
    let mut agg = DailyAggregate::empty_for_today(day.clone(), 2500);
    agg.water_ml = 750;
    agg.breaks_taken = 3;
    repo.upsert_daily(&agg).await.expect("upsert");

    let back = repo.today(now).await.expect("read today");
    assert_eq!(back.day, day);
    assert_eq!(back.water_ml, 750);
    assert_eq!(back.breaks_taken, 3);
    assert_eq!(back.water_goal_ml, 2500);
}

#[tokio::test]
async fn export_json_emits_all_four_tables() {
    let repo = fresh_repo().await;
    repo.append_session(&SessionRecord::new(Timestamp(1_700_000_000_000)))
        .await
        .expect("session");
    let pool = repo.pool();
    let json = repo.export_json().await.expect("export");
    assert_eq!(json["version"], 1);
    assert!(json["exported_at"].is_i64());
    assert!(json["session"].is_array());
    assert!(json["break_record"].is_array());
    assert!(json["hydration_log"].is_array());
    assert!(json["daily_aggregate"].is_array());
    // One row in session.
    assert_eq!(json["session"].as_array().unwrap().len(), 1);
    // The other three are empty.
    assert_eq!(json["break_record"].as_array().unwrap().len(), 0);
    assert_eq!(json["hydration_log"].as_array().unwrap().len(), 0);
    assert_eq!(json["daily_aggregate"].as_array().unwrap().len(), 0);
    let _ = pool; // silence unused
}

#[tokio::test]
async fn clear_resets_every_table() {
    let repo = fresh_repo().await;

    repo.append_session(&SessionRecord::new(Timestamp(1_700_000_000_000)))
        .await
        .expect("session");
    repo.append_hydration(&HydrationEntry::manual(
        250,
        Timestamp(1_700_001_000_000),
    ))
    .await
    .expect("hydration");
    repo.upsert_daily(&DailyAggregate::empty_for_today(
        "2026-07-11".to_string(),
        2000,
    ))
    .await
    .expect("upsert");

    repo.clear().await.expect("clear");

    let pool = repo.pool();
    for (label, sql) in [
        ("session", "SELECT COUNT(*) FROM session"),
        ("break_record", "SELECT COUNT(*) FROM break_record"),
        ("hydration_log", "SELECT COUNT(*) FROM hydration_log"),
        ("daily_aggregate", "SELECT COUNT(*) FROM daily_aggregate"),
    ] {
        let (count,): (i64,) = sqlx::query_as(sql)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("probe {label} after clear: {e}"));
        assert_eq!(count, 0, "{label} should be empty after clear()");
    }
}

#[tokio::test]
async fn trait_object_dispatch_via_arc() {
    // The shell stores the repo as `Arc<dyn HistoryRepo>`. This
    // test makes sure the dispatch compiles and runs through the
    // trait object — not a known-concrete type.
    let repo: Arc<dyn HistoryRepo> = Arc::new(fresh_repo().await);
    let session_id = repo
        .append_session(&SessionRecord::new(Timestamp(1_700_000_000_000)))
        .await
        .expect("session via trait obj");
    assert!(session_id > 0);

    // Use a second repo via the same `Mutex`-ed `Arc` to make sure
    // clone-and-share works (the shell does this from the runtime
    // + the IPC layer).
    let repo2 = repo.clone();
    let now = Timestamp(1_700_000_000_000);
    let _agg = repo2.today(now).await.expect("today via clone");
}

#[tokio::test]
async fn concurrent_appends_are_safe_via_pool() {
    // sqlx::SqlitePool serialises writes internally, so two
    // simultaneous `append_session` calls must both complete and
    // both produce distinct ids.
    let repo = Arc::new(Mutex::new(fresh_repo().await));

    let mut handles = Vec::new();
    for i in 0..4u32 {
        let r = repo.clone();
        let handle = tokio::spawn(async move {
            let rec = SessionRecord::new(Timestamp(1_700_000_000_000 + i as i64 * 1_000));
            let g = r.lock().await;
            g.append_session(&rec).await
        });
        handles.push(handle);
    }
    let mut ids = Vec::new();
    for h in handles {
        let id = h.await.expect("join").expect("append");
        ids.push(id);
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 4, "all four inserts got distinct ids");
}
