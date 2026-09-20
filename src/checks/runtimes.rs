//! Runtime drift: the repository pins a version of a runtime, and a different one is on PATH.

use std::path::{Path, PathBuf};

use super::{Finding, Severity};

/// Shorter than the other checks': PATH changes the moment a version manager switches, so a
/// stale answer here would be wrong rather than merely old.
const FINDING_TTL: u64 = 21600;

/// A runtime we know how to ask for its version, and the flag that asks.
struct Runtime {
    name: &'static str,
    command: &'static str,
    arg: &'static str,
}

const RUNTIMES: &[Runtime] = &[
    Runtime {
        name: "node",
        command: "node",
        arg: "--version",
    },
    Runtime {
        name: "rust",
        command: "rustc",
        arg: "--version",
    },
    Runtime {
        name: "python",
        command: "python3",
        arg: "--version",
    },
    Runtime {
        name: "ruby",
        command: "ruby",
        arg: "--version",
    },
];

pub(crate) fn check(devices: &[PathBuf]) -> Option<Finding> {
    let dir = devices.first()?;
    for (name, wanted) in pinned_versions(dir) {
        let runtime = RUNTIMES.iter().find(|r| r.name == name)?;
        let Some(found) = installed_version(runtime) else {
            continue;
        };
        if versions_agree(&wanted, &found) {
            continue;
        }
        let mut facts = std::collections::BTreeMap::new();
        facts.insert("runtime.repo".to_string(), dir_name(dir));
        facts.insert("runtime.name".to_string(), name.to_string());
        facts.insert("runtime.wanted".to_string(), wanted);
        facts.insert("runtime.found".to_string(), found);
        return Some(Finding {
            id: "runtime_drift".to_string(),
            severity: Severity::Warn,
            ttl: FINDING_TTL,
            facts,
        });
    }
    None
}

/// Every runtime `dir` pins, as (name, version), from the files that do the pinning. Reads only:
/// nothing here runs a version manager.
fn pinned_versions(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(v) = read_trimmed(&dir.join(".nvmrc")) {
        out.push(("node".to_string(), v.trim_start_matches('v').to_string()));
    }
    if let Some(v) = read_trimmed(&dir.join("rust-toolchain")) {
        out.push(("rust".to_string(), v));
    }
    if let Some(v) =
        read_trimmed(&dir.join("rust-toolchain.toml")).and_then(|s| toolchain_channel(&s))
    {
        out.push(("rust".to_string(), v));
    }
    if let Some(text) = read_trimmed(&dir.join(".tool-versions")) {
        out.extend(parse_tool_versions(&text));
    }
    out
}

/// The `channel = "1.79.0"` line of a `rust-toolchain.toml`. A channel like `stable` pins no
/// number, so there is nothing to compare and it is skipped.
fn toolchain_channel(text: &str) -> Option<String> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("channel"))?;
    let value = line
        .split_once('=')?
        .1
        .trim()
        .trim_matches(['"', '\''].as_ref());
    value
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
        .then(|| value.to_string())
}

/// The lines of a `.tool-versions` that name a runtime we know, as (name, version).
fn parse_tool_versions(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let (tool, version) = line.split_once(char::is_whitespace)?;
            let version = version.trim();
            let name = match tool {
                "nodejs" | "node" => "node",
                "rust" => "rust",
                "python" => "python",
                "ruby" => "ruby",
                _ => return None,
            };
            (!version.is_empty() && version.chars().next()?.is_ascii_digit())
                .then(|| (name.to_string(), version.to_string()))
        })
        .collect()
}

/// The version of `runtime` on PATH, reduced to its first dotted number. Runs off the boot path,
/// in `bios refresh`, so spawning is allowed here.
fn installed_version(runtime: &Runtime) -> Option<String> {
    let output = std::process::Command::new(runtime.command)
        .arg(runtime.arg)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let text = if text.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        text.to_string()
    };
    first_version(&text)
}

/// The first dotted version number in a line such as `rustc 1.79.0 (abc 2024-01-01)`.
fn first_version(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|piece| {
            piece.contains('.') && piece.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|piece| piece.trim_end_matches('.').to_string())
}

/// Whether the installed version satisfies the pin. A pin may be less precise than the answer:
/// `20` is satisfied by `20.11.0`, and `20.11` by `20.11.0`, but not the other way round.
fn versions_agree(wanted: &str, found: &str) -> bool {
    let want: Vec<&str> = wanted.split('.').collect();
    let have: Vec<&str> = found.split('.').collect();
    want.iter().zip(have.iter()).all(|(a, b)| a == b) && want.len() <= have.len()
}

fn read_trimmed(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
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
    fn a_pin_is_satisfied_by_a_more_precise_answer_but_not_a_different_one() {
        assert!(versions_agree("20", "20.11.0"));
        assert!(versions_agree("20.11", "20.11.0"));
        assert!(versions_agree("20.11.0", "20.11.0"));
        assert!(!versions_agree("20.11.0", "22.3.0"));
        assert!(!versions_agree("20.11.0", "20.11"));
        assert!(!versions_agree("1.79.0", "1.80.0"));
    }

    #[test]
    fn the_first_dotted_number_is_pulled_out_of_a_version_line() {
        assert_eq!(first_version("v20.11.0\n").as_deref(), Some("20.11.0"));
        assert_eq!(
            first_version("rustc 1.79.0 (129f3b996 2024-06-10)").as_deref(),
            Some("1.79.0")
        );
        assert_eq!(first_version("Python 3.12.4").as_deref(), Some("3.12.4"));
        assert_eq!(first_version("no version here"), None);
    }

    #[test]
    fn tool_versions_lines_are_read_for_the_runtimes_we_know() {
        let text = "nodejs 20.11.0\nrust 1.79.0\nterraform 1.8.0\n# a comment\npython 3.12.4\n";
        assert_eq!(
            parse_tool_versions(text),
            vec![
                ("node".to_string(), "20.11.0".to_string()),
                ("rust".to_string(), "1.79.0".to_string()),
                ("python".to_string(), "3.12.4".to_string()),
            ]
        );
    }

    #[test]
    fn a_tool_versions_entry_with_no_number_is_skipped() {
        assert!(parse_tool_versions("nodejs lts\nrust stable\n").is_empty());
    }

    #[test]
    fn a_rust_toolchain_channel_is_read_only_when_it_pins_a_number() {
        assert_eq!(
            toolchain_channel("[toolchain]\nchannel = \"1.79.0\"\n").as_deref(),
            Some("1.79.0")
        );
        assert_eq!(
            toolchain_channel("[toolchain]\nchannel = \"stable\"\n"),
            None
        );
        assert_eq!(toolchain_channel("[toolchain]\n"), None);
    }

    #[test]
    fn the_pinning_files_are_all_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".nvmrc"), "v20.11.0\n").unwrap();
        std::fs::write(dir.path().join("rust-toolchain"), "1.79.0\n").unwrap();
        let pinned = pinned_versions(dir.path());
        assert!(pinned.contains(&("node".to_string(), "20.11.0".to_string())));
        assert!(pinned.contains(&("rust".to_string(), "1.79.0".to_string())));
    }

    #[test]
    fn a_repository_that_pins_nothing_yields_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        assert!(pinned_versions(dir.path()).is_empty());
        assert!(check(&[dir.path().to_path_buf()]).is_none());
    }

    #[test]
    fn no_devices_means_no_finding() {
        assert!(check(&[]).is_none());
    }
}
