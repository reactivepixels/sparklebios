//! Stale stashes: work put down in a git stash and never picked up again.

use std::path::{Path, PathBuf};

use super::{plural, Finding, Severity};

const FINDING_TTL: u64 = 57600;
/// A stash younger than this is just today's work parked for a minute.
const STALE_AFTER_DAYS: u64 = 30;

pub(crate) fn check(devices: &[PathBuf], now: u64) -> Option<Finding> {
    for dir in devices {
        let times = stash_times(dir);
        if times.is_empty() {
            continue;
        }
        let oldest = *times.iter().min()?;
        let age_days = now.saturating_sub(oldest) / 86_400;
        if age_days < STALE_AFTER_DAYS {
            continue;
        }

        let mut facts = std::collections::BTreeMap::new();
        facts.insert("stash.repo".to_string(), dir_name(dir));
        facts.insert(
            "stash.count".to_string(),
            plural(times.len() as u64, "stash", "stashes"),
        );
        facts.insert("stash.age".to_string(), age_words(age_days));
        return Some(Finding {
            id: "stale_stashes".to_string(),
            severity: Severity::Info,
            ttl: FINDING_TTL,
            facts,
        });
    }
    None
}

/// The commit time of every stash entry in `dir`, as unix seconds. Empty when the directory is
/// not a repository, has no stashes, or git cannot be run at all.
fn stash_times(dir: &Path) -> Vec<u64> {
    let Ok(output) = std::process::Command::new("git")
        .args(["--no-optional-locks", "stash", "list", "--format=%ct"])
        .current_dir(dir)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// An age in days, worded. Months once it is past two of them, because "94 days" is a number you
/// have to convert in your head and "3 months" is not.
fn age_words(days: u64) -> String {
    if days >= 60 {
        plural(days / 30, "month", "months")
    } else {
        plural(days, "day", "days")
    }
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_is_worded_in_days_then_months() {
        assert_eq!(age_words(1), "1 day");
        assert_eq!(age_words(30), "30 days");
        assert_eq!(age_words(59), "59 days");
        assert_eq!(age_words(60), "2 months");
        assert_eq!(age_words(94), "3 months");
    }

    #[test]
    fn a_repository_with_no_stashes_contributes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(stash_times(dir.path()).is_empty());
        assert!(check(&[dir.path().to_path_buf()], 1_700_000_000).is_none());
    }

    #[test]
    fn no_devices_means_no_finding() {
        assert!(check(&[], 1_700_000_000).is_none());
    }
}
