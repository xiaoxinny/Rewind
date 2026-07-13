//! Reminder data types.

use serde::{Deserialize, Serialize};

use crate::clock::Millis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReminderKind {
    EyeBreak,
    Hydration,
    Posture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl From<ReminderKind> for Priority {
    fn from(k: ReminderKind) -> Self {
        // EyeBreak=High, Posture=Medium, Hydration=Low.
        match k {
            ReminderKind::EyeBreak => Priority::High,
            ReminderKind::Posture => Priority::Medium,
            ReminderKind::Hydration => Priority::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Reminder {
    pub kind: ReminderKind,
    pub priority: Priority,
    /// Earliest monotonic time at which this reminder may fire.
    pub earliest: Millis,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_from_kind() {
        assert_eq!(Priority::from(ReminderKind::EyeBreak), Priority::High);
        assert_eq!(Priority::from(ReminderKind::Posture), Priority::Medium);
        assert_eq!(Priority::from(ReminderKind::Hydration), Priority::Low);
    }
}
