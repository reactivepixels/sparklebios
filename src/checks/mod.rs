//! Severity, Finding, and the probe registry. Only ever run by `bios refresh`, never on the
//! boot path.

pub mod dotfiles;
pub mod ports;
pub mod projects;
pub mod runtimes;
pub mod secrets;
pub mod stashes;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    /// Seconds this finding may still be shown, counted from the cache's `generated` time.
    pub ttl: u64,
    pub facts: std::collections::BTreeMap<String, String>,
}

/// Runs every probe. Only ever called by `bios refresh`, never on the boot path. A probe that
/// fails contributes nothing rather than erring. Findings come back in a fixed order, which is
/// also their render order: `boot_order`, `boot_dirty`, `irq_conflict`, then `virus_one` or
/// `virus_many`.
pub fn run_all(config: &crate::config::Config) -> Vec<Finding> {
    let devices = projects::boot_devices(config);
    let mut findings = projects::findings(&devices);
    if let Some(finding) = ports::check() {
        findings.push(finding);
    }
    if let Some(finding) = secrets::check(&devices) {
        findings.push(finding);
    }
    if let Some(finding) = stashes::check(&devices, crate::clock::now_unix()) {
        findings.push(finding);
    }
    if let Some(finding) = dotfiles::check(std::env::var("HOME").ok().as_deref()) {
        findings.push(finding);
    }
    if let Some(finding) = runtimes::check(&devices) {
        findings.push(finding);
    }
    findings
}

/// "1 uncommitted change" or "3 uncommitted changes".
pub fn plural(n: u64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_produces_the_one_and_many_forms() {
        assert_eq!(
            plural(1, "uncommitted change", "uncommitted changes"),
            "1 uncommitted change"
        );
        assert_eq!(
            plural(3, "uncommitted change", "uncommitted changes"),
            "3 uncommitted changes"
        );
    }
}
