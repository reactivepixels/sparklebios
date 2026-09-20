//! Battery health: how much of its original capacity the battery still holds.
//!
//! This one is deliberately quiet. Capacity barely moves week to week, so a check that fired
//! every day would say the same number forever and become wallpaper. It speaks once when the
//! battery first drops below 80 percent of its design capacity, and then only when it crosses
//! each further ten point step. The step it last spoke at is carried in the cache so it never
//! repeats itself.

use super::{Finding, Severity};

const FINDING_TTL: u64 = 86400;
/// Apple calls a battery "service recommended" around here, and it is the point at which the
/// number first tells you something you did not know.
const FIRST_STEP: u8 = 80;

/// The finding, if any, and the step to carry forward. The step is returned whether or not
/// anything fired, so a battery that has not crossed a new threshold stays silent next time too.
pub(crate) fn check(previous_step: Option<u8>) -> (Option<Finding>, Option<u8>) {
    let Some((health, cycles)) = read_battery() else {
        return (None, previous_step);
    };
    let Some(step) = step_for(health) else {
        return (None, previous_step);
    };
    // Only when this is a step further down than the one already spoken at.
    if previous_step.is_some_and(|p| step >= p) {
        return (None, previous_step);
    }

    let mut facts = std::collections::BTreeMap::new();
    facts.insert("battery.health".to_string(), format!("{health}%"));
    facts.insert("battery.cycles".to_string(), cycles.to_string());
    let finding = Finding {
        id: "battery_health".to_string(),
        severity: Severity::Info,
        ttl: FINDING_TTL,
        facts,
    };
    (Some(finding), Some(step))
}

/// The ten point step a health percentage has fallen below, or `None` while it is still healthy.
/// 79 percent has crossed 80, 68 percent has crossed 70.
fn step_for(health: u8) -> Option<u8> {
    if health >= FIRST_STEP {
        return None;
    }
    Some(((health / 10) + 1) * 10)
}

/// Health as a whole percentage of design capacity, and the cycle count.
#[cfg(target_os = "macos")]
fn read_battery() -> Option<(u8, u64)> {
    let output = std::process::Command::new("ioreg")
        .args(["-r", "-c", "AppleSmartBattery"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ioreg(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
fn read_battery() -> Option<(u8, u64)> {
    // Linux exposes the same numbers through sysfs, in either charge (uAh) or energy (uWh) units
    // depending on the driver.
    let base = std::path::Path::new("/sys/class/power_supply");
    for name in ["BAT0", "BAT1"] {
        let dir = base.join(name);
        let read = |leaf: &str| -> Option<u64> {
            std::fs::read_to_string(dir.join(leaf))
                .ok()?
                .trim()
                .parse()
                .ok()
        };
        let full = read("charge_full").or_else(|| read("energy_full"));
        let design = read("charge_full_design").or_else(|| read("energy_full_design"));
        if let (Some(full), Some(design)) = (full, design) {
            if design > 0 {
                let cycles = read("cycle_count").unwrap_or(0);
                return Some((percentage(full, design), cycles));
            }
        }
    }
    None
}

/// Pulls `NominalChargeCapacity` (what the battery actually holds now), `DesignCapacity` and
/// `CycleCount` out of `ioreg` text output. `AppleRawMaxCapacity` is the fallback on machines
/// that do not report a nominal figure. `MaxCapacity` is deliberately not used: on current macOS
/// it reads 100 on a worn battery, because it means percent of the current maximum, not of the
/// design.
fn parse_ioreg(text: &str) -> Option<(u8, u64)> {
    let field = |name: &str| -> Option<u64> {
        let needle = format!("\"{name}\" = ");
        text.lines().find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix(&needle)?;
            rest.trim().parse().ok()
        })
    };
    let design = field("DesignCapacity")?;
    if design == 0 {
        return None;
    }
    let full = field("NominalChargeCapacity").or_else(|| field("AppleRawMaxCapacity"))?;
    let cycles = field("CycleCount").unwrap_or(0);
    Some((percentage(full, design), cycles))
}

/// `full` as a whole percentage of `design`, never above 100: a fresh battery often reads a
/// little over its design capacity, and "103%" is not a health figure anybody wants.
fn percentage(full: u64, design: u64) -> u8 {
    ((full.saturating_mul(100) / design).min(100)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_healthy_battery_has_no_step_and_says_nothing() {
        assert_eq!(step_for(100), None);
        assert_eq!(step_for(85), None);
        assert_eq!(step_for(80), None);
    }

    #[test]
    fn each_ten_point_band_below_eighty_is_its_own_step() {
        assert_eq!(step_for(79), Some(80));
        assert_eq!(step_for(70), Some(80));
        assert_eq!(step_for(69), Some(70));
        assert_eq!(step_for(60), Some(70));
        assert_eq!(step_for(59), Some(60));
        assert_eq!(step_for(5), Some(10));
    }

    #[test]
    fn capacity_is_a_percentage_of_design_and_never_over_a_hundred() {
        assert_eq!(percentage(5288, 6249), 84);
        assert_eq!(percentage(6249, 6249), 100);
        assert_eq!(percentage(6400, 6249), 100);
        assert_eq!(percentage(0, 6249), 0);
    }

    #[test]
    fn the_real_ioreg_shape_is_parsed() {
        // trimmed from a real `ioreg -r -c AppleSmartBattery` on this machine
        let text = r#"
      "NominalChargeCapacity" = 5440
      "MaxCapacity" = 100
      "DesignCapacity" = 6249
      "CycleCount" = 319
      "AppleRawMaxCapacity" = 5288
"#;
        assert_eq!(parse_ioreg(text), Some((87, 319)));
    }

    #[test]
    fn a_missing_design_capacity_is_not_a_battery_we_can_report_on() {
        assert_eq!(parse_ioreg("\"CycleCount\" = 10\n"), None);
        assert_eq!(parse_ioreg("\"DesignCapacity\" = 0\n"), None);
    }

    #[test]
    fn a_step_already_spoken_at_does_not_speak_again() {
        // 79 percent is step 80; having already said 80, it stays quiet.
        assert!(Some(80u8).is_some_and(|p| step_for(79).unwrap() >= p));
        // but crossing 70 is new
        assert!(!Some(80u8).is_some_and(|p| step_for(69).unwrap() >= p));
    }
}
