//! IRQ conflicts: a dev server that has been listening on a well known port for a long time.

use super::{plural, Finding, Severity};

const ALLOWED_PORTS: &[u16] = &[
    1420, 3000, 3001, 4000, 4200, 4321, 5173, 5174, 8000, 8080, 8081, 8888, 9000, 9090,
];
const FINDING_TTL: u64 = 21600;
const MIN_AGE_SECS: u64 = 8 * 3600;

/// macOS system services that hold well known ports as shipped. They are never a dev server
/// somebody forgot about, so they are not an IRQ conflict.
const SYSTEM_COMMANDS: &[&str] = &[
    "ControlCenter",
    "AirPlayXPCHelper",
    "rapportd",
    "sharingd",
    "remoted",
    "identityservicesd",
    "mDNSResponder",
    "launchd",
];

type Listener = (u32, String, u16);
type Aged = (u32, String, u16, u64);

pub(crate) fn check() -> Option<Finding> {
    let output = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let listeners = parse_lsof(&text);

    let mut aged: Vec<Aged> = Vec::new();
    for (pid, command, port) in listeners {
        if let Some(age) = process_age(pid) {
            aged.push((pid, command, port, age));
        }
    }

    let (pid, command, port, age) = pick_oldest(&aged)?;
    let mut facts = std::collections::BTreeMap::new();
    facts.insert("irq.port".to_string(), port.to_string());
    facts.insert("irq.name".to_string(), command);
    facts.insert("irq.pid".to_string(), pid.to_string());
    facts.insert("irq.age".to_string(), format_age(age));
    Some(Finding {
        id: "irq_conflict".to_string(),
        severity: Severity::Warn,
        ttl: FINDING_TTL,
        facts,
    })
}

/// Parses `lsof -nP -iTCP -sTCP:LISTEN -Fpcn` field output into `(pid, command, port)` triples,
/// keeping only listeners on `ALLOWED_PORTS` whose command is not in `SYSTEM_COMMANDS`. A process
/// record can carry several `n` lines.
fn parse_lsof(text: &str) -> Vec<Listener> {
    let mut result = Vec::new();
    let mut pid: Option<u32> = None;
    let mut command: Option<String> = None;

    for line in text.lines() {
        let Some(tag) = line.chars().next() else {
            continue;
        };
        let rest = &line[tag.len_utf8()..];
        match tag {
            'p' => {
                pid = rest.parse().ok();
                command = None;
            }
            'c' => {
                command = Some(rest.to_string());
            }
            'n' => {
                let (Some(pid), Some(command)) = (pid, command.clone()) else {
                    continue;
                };
                let Some(port) = parse_port(rest) else {
                    continue;
                };
                if ALLOWED_PORTS.contains(&port) && !SYSTEM_COMMANDS.contains(&command.as_str()) {
                    result.push((pid, command, port));
                }
            }
            _ => {}
        }
    }

    result
}

/// Parses an `n` field's address, e.g. `*:3000`, `127.0.0.1:5173` or `[::1]:8080`.
fn parse_port(addr: &str) -> Option<u16> {
    if let Some(idx) = addr.rfind(']') {
        let rest = addr[idx + 1..].strip_prefix(':')?;
        return rest.parse().ok();
    }
    let idx = addr.rfind(':')?;
    addr[idx + 1..].parse().ok()
}

/// Parses a `ps -o etime=` value: `MM:SS`, `HH:MM:SS` or `D-HH:MM:SS`.
fn parse_etime(s: &str) -> Option<u64> {
    let s = s.trim();
    let (days, rest) = match s.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0, s),
    };
    let parts: Vec<&str> = rest.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [h, m, s] => (
            h.parse::<u64>().ok()?,
            m.parse::<u64>().ok()?,
            s.parse::<u64>().ok()?,
        ),
        [m, s] => (0, m.parse::<u64>().ok()?, s.parse::<u64>().ok()?),
        _ => return None,
    };
    Some(days * 86400 + hours * 3600 + minutes * 60 + seconds)
}

fn process_age(pid: u32) -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "etime=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    parse_etime(String::from_utf8_lossy(&output.stdout).trim())
}

/// The single oldest listener that is at least `MIN_AGE_SECS` old.
fn pick_oldest(candidates: &[Aged]) -> Option<Aged> {
    candidates
        .iter()
        .filter(|(_, _, _, age)| *age >= MIN_AGE_SECS)
        .max_by_key(|(_, _, _, age)| *age)
        .cloned()
}

/// "41 minutes" / "1 minute" under an hour, "1 hour" / "5 hours" under a day, "1 day" / "3 days"
/// otherwise.
fn format_age(seconds: u64) -> String {
    if seconds < 3600 {
        plural(seconds / 60, "minute", "minutes")
    } else if seconds < 86400 {
        plural(seconds / 3600, "hour", "hours")
    } else {
        plural(seconds / 86400, "day", "days")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        "p111\ncnode\nn*:3000\np222\ncchrome\nn127.0.0.1:5173\nn[::1]:8080\np333\ncfoo\nn*:22\n";

    #[test]
    fn parses_records_and_drops_ports_not_on_the_list() {
        let records = parse_lsof(FIXTURE);
        assert_eq!(
            records,
            vec![
                (111, "node".to_string(), 3000),
                (222, "chrome".to_string(), 5173),
                (222, "chrome".to_string(), 8080),
            ]
        );
    }

    #[test]
    fn a_system_command_is_dropped_even_on_an_allowed_port_while_others_survive() {
        let fixture = "p634\ncControlCenter\nn*:5173\np111\ncnode\nn*:3000\n";
        let records = parse_lsof(fixture);
        assert_eq!(records, vec![(111, "node".to_string(), 3000)]);
    }

    #[test]
    fn port_5000_is_no_longer_accepted() {
        let fixture = "p634\ncControlCenter\nn*:5000\n";
        assert!(parse_lsof(fixture).is_empty());
    }

    #[test]
    fn parse_etime_handles_all_three_shapes() {
        assert_eq!(parse_etime("12:33"), Some(12 * 60 + 33));
        assert_eq!(parse_etime("04:12:33"), Some(4 * 3600 + 12 * 60 + 33));
        assert_eq!(
            parse_etime("3-04:12:33"),
            Some(3 * 86400 + 4 * 3600 + 12 * 60 + 33)
        );
    }

    #[test]
    fn pick_oldest_requires_the_eight_hour_threshold_and_picks_the_oldest() {
        let candidates = vec![
            (1, "a".to_string(), 3000, MIN_AGE_SECS - 1),
            (2, "b".to_string(), 3001, MIN_AGE_SECS),
            (3, "c".to_string(), 4000, MIN_AGE_SECS + 3600),
        ];
        let picked = pick_oldest(&candidates).unwrap();
        assert_eq!(picked.0, 3);
    }

    #[test]
    fn pick_oldest_is_none_when_nothing_qualifies() {
        let candidates = vec![(1, "a".to_string(), 3000, MIN_AGE_SECS - 1)];
        assert!(pick_oldest(&candidates).is_none());
    }

    #[test]
    fn format_age_exact_strings() {
        assert_eq!(format_age(60), "1 minute");
        assert_eq!(format_age(41 * 60), "41 minutes");
        assert_eq!(format_age(3600), "1 hour");
        assert_eq!(format_age(5 * 3600), "5 hours");
        assert_eq!(format_age(86400), "1 day");
        assert_eq!(format_age(3 * 86400), "3 days");
    }
}
