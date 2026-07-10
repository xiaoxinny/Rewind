//! Built-in eye exercise catalog (DP-6).
//!
//! | id           | Name              | Duration | Guidance |
//! |--------------|-------------------|----------|----------|
//! | `palming`    | Palming           | 30 s     | Cover closed eyes with palms, breathe. |
//! | `near_far`   | Near/far focus    | 30 s     | Alternate fingertip (5 s) / far (5 s) ×3. |
//! | `blink`      | Blinking reset    | 20 s     | Paced full-blink prompt every ~3 s. |
//! | `figure_eight` | Figure-eights   | 30 s     | Animated lazy-8 dot for the eyes. |

/// An exercise definition rendered by the matching Svelte
/// component. M4 populates the per-step structure; M1 carries just
/// id/name/duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exercise {
    pub id: &'static str,
    pub name: &'static str,
    pub duration_sec: u32,
}

/// The four built-ins. Order matters — `pick(rotation_index)` indexes
/// into this slice.
pub const EXERCISES: &[Exercise] = &[
    Exercise {
        id: "palming",
        name: "Palming",
        duration_sec: 30,
    },
    Exercise {
        id: "near_far",
        name: "Near/far focus",
        duration_sec: 30,
    },
    Exercise {
        id: "blink",
        name: "Blinking reset",
        duration_sec: 20,
    },
    Exercise {
        id: "figure_eight",
        name: "Figure-eights",
        duration_sec: 30,
    },
];

/// Pick an exercise by rotation index.
pub fn pick(rotation_index: usize) -> &'static Exercise {
    &EXERCISES[rotation_index % EXERCISES.len()]
}

/// Find an exercise by id.
pub fn find(id: &str) -> Option<&'static Exercise> {
    EXERCISES.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_rotates() {
        assert_eq!(pick(0).id, "palming");
        assert_eq!(pick(1).id, "near_far");
        assert_eq!(pick(2).id, "blink");
        assert_eq!(pick(3).id, "figure_eight");
        assert_eq!(pick(4).id, "palming");
        assert_eq!(pick(99).id, "figure_eight");
    }

    #[test]
    fn find_returns_known_and_misses_unknown() {
        assert!(find("palming").is_some());
        assert!(find("near_far").is_some());
        assert!(find("blink").is_some());
        assert!(find("figure_eight").is_some());
        assert!(find("not-a-real-id").is_none());
    }

    #[test]
    fn exercises_have_expected_durations() {
        assert_eq!(find("palming").unwrap().duration_sec, 30);
        assert_eq!(find("near_far").unwrap().duration_sec, 30);
        assert_eq!(find("blink").unwrap().duration_sec, 20);
        assert_eq!(find("figure_eight").unwrap().duration_sec, 30);
    }
}
