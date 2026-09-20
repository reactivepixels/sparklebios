//! Severity, Finding, and the probe registry. Only ever run by `bios refresh`, never on the
//! boot path.

pub mod battery;
pub mod disk;
pub mod dotfiles;
pub mod ports;
pub mod projects;
pub mod runtimes;
pub mod secrets;
pub mod stashes;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// How urgent a finding is.
pub enum Severity {
    /// Worth knowing, nothing to fix.
    Info,
    /// Worth a look.
    Warn,
    /// Needs attention.
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// One probe's result: what it found, how urgent, and the facts behind it.
pub struct Finding {
    /// Which probe this came from.
    pub id: String,
    /// How urgent this finding is.
    pub severity: Severity,
    /// Seconds this finding may still be shown, counted from the cache's `generated` time.
    pub ttl: u64,
    /// The probed values behind this finding, for rendering.
    pub facts: std::collections::BTreeMap<String, String>,
}

/// Runs every probe. Only ever called by `bios refresh`, never on the boot path. A probe that
/// fails contributes nothing rather than erring. Findings come back in a fixed order, which is
/// also their render order: `boot_order`, `boot_dirty`, `irq_conflict`, then `virus_one` or
/// `virus_many`.
/// The few things a refresh carries over from the last one: a check cannot see history unless
/// something keeps it. Disk trend needs a series of readings, and battery health needs to know
/// the step it last spoke at so it does not repeat itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Carried {
    /// Free space readings from past refreshes, oldest first.
    pub disk_history: Vec<disk::Reading>,
    /// The battery health step already reported, so it is never reported twice.
    pub battery_step: Option<u8>,
}

/// Runs every probe and returns its findings alongside the state to carry into the next
/// refresh. See the module doc comment for probe order and failure handling.
pub fn run_all(
    config: &crate::config::Config,
    carried: Carried,
    now: u64,
) -> (Vec<Finding>, Carried) {
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

    let free_gb = crate::facts::gather()
        .get("disk.free_gb")
        .and_then(|v| v.parse().ok());
    let disk_history = disk::record(carried.disk_history, now, free_gb);
    if let Some(finding) = disk::check(&disk_history) {
        findings.push(finding);
    }

    let (battery_finding, battery_step) = battery::check(carried.battery_step);
    if let Some(finding) = battery_finding {
        findings.push(finding);
    }

    (
        findings,
        Carried {
            disk_history,
            battery_step,
        },
    )
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
