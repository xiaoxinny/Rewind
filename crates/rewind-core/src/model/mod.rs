//! Persistence-agnostic DTOs. Mirror of the SQLite schema in
//! `rewind-storage/src/migrations/0001_init.sql` but **without** any DB
//! types. See implementation plan §8a.

pub mod aggregate;
pub mod break_record;
pub mod hydration;
pub mod session;
