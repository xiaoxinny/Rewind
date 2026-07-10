//! Exercise catalog (DP-6).
//!
//! Four built-in eye exercises; a rest break shows exactly one
//! (default: rotation). Each definition is rendered by a matching
//! Svelte component on the frontend. See implementation plan §7i.

pub mod catalog;

pub use catalog::{pick, Exercise, EXERCISES};
