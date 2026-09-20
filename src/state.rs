//! State file load and save, streak logic.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Boot history: streaks and the last boot time, persisted between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// When the last boot happened, in seconds since the epoch.
    pub last_boot: Option<u64>,
    /// The date of the last full (non-quiet) boot.
    pub last_full_day: Option<String>,
    /// How many consecutive days a boot has happened.
    pub streak_days: u32,
    /// The last day counted toward `streak_days`.
    pub streak_last_day: Option<String>,
    /// The Memory Test easter egg's high score: the most K ever cleared in one game.
    pub memory_test_best_kb: u64,
    /// Whether a Memory Test game has ever been cleared in full.
    pub memory_test_cleared: bool,
}

impl State {
    /// Missing, unreadable or corrupt file yields State::default().
    pub fn load(dir: &Path) -> State {
        let Ok(contents) = std::fs::read_to_string(dir.join("state.json")) else {
            return State::default();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    /// Writes `<dir>/state.json` atomically: write `state.json.<pid>.tmp`, then rename. Creates the directory.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let pid = std::process::id();
        let tmp_path = dir.join(format!("state.json.{pid}.tmp"));
        let json = serde_json::to_string(self)?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, dir.join("state.json"))?;
        Ok(())
    }

    /// Same day: unchanged. Last day was `yesterday`: increment. Anything else: reset to 1.
    pub fn advance_streak(&mut self, today: &str, yesterday: &str) {
        if self.streak_last_day.as_deref() == Some(today) {
            return;
        }
        if self.streak_last_day.as_deref() == Some(yesterday) {
            self.streak_days += 1;
        } else {
            self.streak_days = 1;
        }
        self.streak_last_day = Some(today.to_string());
    }

    /// "1 day" or "N days".
    pub fn streak_label(&self) -> String {
        if self.streak_days == 1 {
            "1 day".to_string()
        } else {
            format!("{} days", self.streak_days)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_and_corrupt_files_load_as_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(dir.path()), State::default());
        std::fs::write(dir.path().join("state.json"), "{ not json").unwrap();
        assert_eq!(State::load(dir.path()), State::default());
    }
    #[test]
    fn save_then_load_round_trips_and_creates_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        let s = State {
            last_boot: Some(42),
            last_full_day: Some("2026-09-19".into()),
            streak_days: 3,
            streak_last_day: Some("2026-09-19".into()),
            memory_test_best_kb: 18874368,
            memory_test_cleared: true,
        };
        s.save(&nested).unwrap();
        assert_eq!(State::load(&nested), s);
        let leftovers: Vec<_> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("state.json")]);
    }
    #[test]
    fn unknown_fields_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.json"),
            r#"{"streak_days":2,"future_field":true}"#,
        )
        .unwrap();
        assert_eq!(State::load(dir.path()).streak_days, 2);
    }
    #[test]
    fn old_state_files_without_the_memory_test_keys_load_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("state.json"), r#"{"streak_days":2}"#).unwrap();
        let s = State::load(dir.path());
        assert_eq!(s.memory_test_best_kb, 0);
        assert!(!s.memory_test_cleared);
    }
    #[test]
    fn the_memory_test_high_score_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let s = State {
            memory_test_best_kb: 37748736,
            memory_test_cleared: true,
            ..State::default()
        };
        s.save(dir.path()).unwrap();
        let loaded = State::load(dir.path());
        assert_eq!(loaded.memory_test_best_kb, 37748736);
        assert!(loaded.memory_test_cleared);
    }
    #[test]
    fn streak_rules() {
        let mut s = State::default();
        s.advance_streak("2026-09-19", "2026-09-18");
        assert_eq!((s.streak_days, s.streak_label()), (1, "1 day".to_string()));
        s.advance_streak("2026-09-19", "2026-09-18");
        assert_eq!(s.streak_days, 1);
        s.advance_streak("2026-09-20", "2026-09-19");
        assert_eq!((s.streak_days, s.streak_label()), (2, "2 days".to_string()));
        s.advance_streak("2026-09-25", "2026-09-24");
        assert_eq!(s.streak_days, 1);
    }
}
