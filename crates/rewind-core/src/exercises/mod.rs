//! Exercise catalog.
//!
//! Four built-in eye exercises; a rest break shows exactly one
//! (default: rotation). Each definition is rendered by a matching
//! Svelte component on the frontend.

pub mod catalog;

pub use catalog::{pick, Exercise, EXERCISES};
