//! Error type for `rewind-storage`.
//!
//! Kept as its own module so the surface stays stable when sqlx
//! version bumps change its internal error variants. We wrap the
//! underlying sqlx error rather than re-export it — that way callers
//! don't see a 17-deep error enum nor pin us to a major version
//! across the workspace.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    /// An SQL operation failed (insert / update / select).
    #[error("sqlite error: {0}")]
    Db(#[from] sqlx::Error),

    /// Migration runner failed (file missing, checksum mismatch,
    /// unsupported target version).
    #[error("sqlite migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

/// Convenience alias so call sites don't have to spell out the full
/// `Result<T, StorageError>` shape.
pub type Result<T> = std::result::Result<T, StorageError>;
