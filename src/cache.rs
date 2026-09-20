//! The fact cache: the last known findings from `bios refresh`, read on the boot path.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::checks::Finding;

/// The last findings gathered by `bios refresh`, plus the trend history checks carry over.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cache {
    /// When these findings were gathered, in seconds since the epoch.
    pub generated: u64,
    /// The findings themselves, as last reported by `bios refresh`.
    pub findings: Vec<Finding>,
    /// Free space over the last fortnight, so the disk trend check has something to trend.
    #[serde(default)]
    pub disk_history: Vec<crate::checks::disk::Reading>,
    /// The battery health step already reported, so it is never reported twice.
    #[serde(default)]
    pub battery_step: Option<u8>,
}

/// Spawn a detached refresh once the cache is older than this, in seconds.
pub const REFRESH_AFTER_SECS: u64 = 1800;

/// Missing, unreadable or corrupt file yields `Cache::default()`.
pub fn load(dir: &Path) -> Cache {
    let Ok(contents) = std::fs::read_to_string(dir.join("facts.json")) else {
        return Cache::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

impl Cache {
    /// Writes `<dir>/facts.json` atomically: write `facts.json.<pid>.tmp`, then rename. Creates
    /// the directory.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let pid = std::process::id();
        let tmp_path = dir.join(format!("facts.json.{pid}.tmp"));
        let json = serde_json::to_string(self)?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, dir.join("facts.json"))?;
        Ok(())
    }

    /// True when `generated` is zero, in the future, or more than `REFRESH_AFTER_SECS` old.
    pub fn is_stale(&self, now: u64) -> bool {
        self.generated == 0 || self.generated > now || now - self.generated > REFRESH_AFTER_SECS
    }

    /// Findings whose own ttl has not passed.
    pub fn fresh(&self, now: u64) -> Vec<&Finding> {
        if self.generated > now {
            return Vec::new();
        }
        let age = now - self.generated;
        self.findings.iter().filter(|f| age <= f.ttl).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;

    fn finding(id: &str, ttl: u64) -> Finding {
        Finding {
            id: id.to_string(),
            severity: Severity::Info,
            ttl,
            facts: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn save_then_load_round_trips_and_creates_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        let cache = Cache {
            generated: 42,
            findings: vec![finding("boot_order", 100)],
            ..Cache::default()
        };
        cache.save(&nested).unwrap();
        assert_eq!(load(&nested), cache);
        let leftovers: Vec<_> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("facts.json")]);
    }

    #[test]
    fn missing_and_corrupt_files_load_as_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Cache::default());
        std::fs::write(dir.path().join("facts.json"), "{ not json").unwrap();
        assert_eq!(load(dir.path()), Cache::default());
    }

    #[test]
    fn is_stale_at_the_boundary_and_for_zero_or_future_generated() {
        let mut cache = Cache {
            generated: 1000,
            ..Cache::default()
        };
        assert!(!cache.is_stale(1000 + REFRESH_AFTER_SECS));
        assert!(cache.is_stale(1000 + REFRESH_AFTER_SECS + 1));

        cache.generated = 0;
        assert!(cache.is_stale(1000));

        cache.generated = 5000;
        assert!(cache.is_stale(1000));
    }

    #[test]
    fn fresh_keeps_a_finding_inside_its_ttl_and_drops_one_past_it() {
        let cache = Cache {
            generated: 1000,
            findings: vec![finding("a", 100), finding("b", 10)],
            ..Cache::default()
        };
        let fresh = cache.fresh(1050);
        let ids: Vec<_> = fresh.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["a"]);
    }
}
