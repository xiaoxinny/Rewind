//! Small helpers shared between `storage_app.rs` and the IPC layer.
//!
//! Intentionally tiny — anything more complex lives next to the
//! type it operates on. Currently exposes a `local_day_string` so
//! the shell and the storage crate agree on the local-day bucket.

use rewind_core::clock::Timestamp;
use time::{Date, OffsetDateTime};

/// Render a `Timestamp` as the local-day bucket string (`YYYY-MM-DD`),
/// mirroring the helper in `rewind-storage`.
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
