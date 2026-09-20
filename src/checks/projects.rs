//! Boot device order: the repos you were most recently working in.

use std::path::{Path, PathBuf};

use super::{plural, Finding, Severity};

const DEFAULT_ROOT_LEAVES: &[&str] = &[
    "Code",
    "code",
    "Projects",
    "projects",
    "src",
    "dev",
    "Developer",
    "repos",
    "work",
    "git",
];

const FINDING_TTL: u64 = 57600;
const MAX_DIRS_EXAMINED: usize = 400;
const MAX_LINE_LEN: usize = 60;

/// Up to 3 absolute repository paths, most recently touched first.
pub fn boot_devices(config: &crate::config::Config) -> Vec<PathBuf> {
    let home = std::env::var("HOME").ok();
    let roots = resolve_roots(config, home.as_deref());

    let mut examined = 0usize;
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    for root in roots {
        if examined >= MAX_DIRS_EXAMINED {
            break;
        }
        walk_root(&root, &mut examined, &mut candidates);
    }

    candidates.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    candidates.into_iter().take(3).map(|(p, _)| p).collect()
}

/// The `boot_order` finding (always present when there is at least one device) and, when the
/// first device has uncommitted changes, the `boot_dirty` finding.
pub(crate) fn findings(devices: &[PathBuf]) -> Vec<Finding> {
    let mut out = Vec::new();
    if devices.is_empty() {
        return out;
    }

    let mut facts = std::collections::BTreeMap::new();
    facts.insert("boot.devices".to_string(), join_capped(devices));
    out.push(Finding {
        id: "boot_order".to_string(),
        severity: Severity::Info,
        ttl: FINDING_TTL,
        facts,
    });

    if let Some(first) = devices.first() {
        if let Some(count) = dirty_count(first) {
            if count > 0 {
                let mut facts = std::collections::BTreeMap::new();
                facts.insert("boot.device".to_string(), dir_name(first));
                facts.insert(
                    "boot.changes".to_string(),
                    plural(count, "uncommitted change", "uncommitted changes"),
                );
                out.push(Finding {
                    id: "boot_dirty".to_string(),
                    severity: Severity::Info,
                    ttl: FINDING_TTL,
                    facts,
                });
            }
        }
    }

    out
}

fn resolve_roots(config: &crate::config::Config, home: Option<&str>) -> Vec<PathBuf> {
    if !config.project_dirs.is_empty() {
        return config
            .project_dirs
            .iter()
            .map(|dir| expand_tilde(dir, home))
            .collect();
    }
    let Some(home) = home else {
        return Vec::new();
    };
    DEFAULT_ROOT_LEAVES
        .iter()
        .map(|leaf| PathBuf::from(home).join(leaf))
        .filter(|p| p.is_dir())
        .collect()
}

fn expand_tilde(value: &str, home: Option<&str>) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = home {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

/// Entries of `dir` that are themselves directories, sorted by name for deterministic traversal.
fn sorted_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn recency(dir: &Path) -> Option<std::time::SystemTime> {
    let index = dir.join(".git").join("index");
    if let Ok(meta) = std::fs::metadata(&index) {
        if let Ok(modified) = meta.modified() {
            return Some(modified);
        }
    }
    std::fs::metadata(dir.join(".git")).ok()?.modified().ok()
}

/// Walks `root` at depth 1 and depth 2, looking for a child directory that contains a `.git`
/// entry. A directory that is itself a repo is not descended into. Stops examining further
/// directories once `examined` reaches `MAX_DIRS_EXAMINED`.
fn walk_root(
    root: &Path,
    examined: &mut usize,
    candidates: &mut Vec<(PathBuf, std::time::SystemTime)>,
) {
    for depth1 in sorted_subdirs(root) {
        if *examined >= MAX_DIRS_EXAMINED {
            return;
        }
        *examined += 1;

        if is_repo(&depth1) {
            if let Some(t) = recency(&depth1) {
                candidates.push((depth1, t));
            }
            continue;
        }

        for depth2 in sorted_subdirs(&depth1) {
            if *examined >= MAX_DIRS_EXAMINED {
                return;
            }
            *examined += 1;

            if is_repo(&depth2) {
                if let Some(t) = recency(&depth2) {
                    candidates.push((depth2, t));
                }
            }
        }
    }
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// The devices' directory names joined with ", ", capped at `MAX_LINE_LEN` characters by
/// dropping devices from the end. Never truncates a name mid-word.
fn join_capped(devices: &[PathBuf]) -> String {
    let names: Vec<String> = devices.iter().map(|d| dir_name(d)).collect();
    let mut count = names.len();
    loop {
        let joined = names[..count].join(", ");
        if joined.chars().count() <= MAX_LINE_LEN || count <= 1 {
            return joined;
        }
        count -= 1;
    }
}

/// The number of non-empty `git status --porcelain` lines in `dir`. `--no-optional-locks`
/// matters: without it, git refreshes the index on disk and this probe would pin the repo at the
/// top of the list forever.
fn dirty_count(dir: &Path) -> Option<u64> {
    let output = std::process::Command::new("git")
        .args(["--no-optional-locks", "status", "--porcelain"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().filter(|l| !l.is_empty()).count() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    fn make_repo(path: &Path, seconds_ago: u64) {
        std::fs::create_dir_all(path.join(".git")).unwrap();
        let index = path.join(".git").join("index");
        std::fs::write(&index, "").unwrap();
        let modified = SystemTime::now() - Duration::from_secs(seconds_ago);
        let file = File::options().write(true).open(&index).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
    }

    fn config_with_root(root: &Path) -> crate::config::Config {
        crate::config::Config {
            project_dirs: vec![root.to_string_lossy().to_string()],
            ..crate::config::Config::default()
        }
    }

    #[test]
    fn most_recent_repo_comes_first() {
        let root = tempfile::tempdir().unwrap();
        make_repo(&root.path().join("old"), 1000);
        make_repo(&root.path().join("new"), 10);
        make_repo(&root.path().join("mid"), 500);

        let devices = boot_devices(&config_with_root(root.path()));
        let names: Vec<_> = devices
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["new", "mid", "old"]);
    }

    #[test]
    fn a_depth_two_repo_is_found() {
        let root = tempfile::tempdir().unwrap();
        make_repo(&root.path().join("group").join("nested"), 5);

        let devices = boot_devices(&config_with_root(root.path()));
        assert_eq!(devices.len(), 1);
        assert!(devices[0].ends_with("group/nested"));
    }

    #[test]
    fn a_directory_with_no_git_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("plain")).unwrap();

        assert!(boot_devices(&config_with_root(root.path())).is_empty());
    }

    #[test]
    fn a_repo_is_not_descended_into() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        make_repo(&repo, 5);
        make_repo(&repo.join("nested"), 1);

        let devices = boot_devices(&config_with_root(root.path()));
        assert_eq!(devices.len(), 1);
        assert!(devices[0].ends_with("repo"));
    }

    #[test]
    fn the_400_directory_cap_holds() {
        let root = tempfile::tempdir().unwrap();
        for i in 0..410 {
            std::fs::create_dir_all(root.path().join(format!("d{i:04}"))).unwrap();
        }
        // Sorts after every "d..." directory, so it is only reached once the cap is lifted.
        make_repo(&root.path().join("zzz_should_not_be_found"), 1);

        assert!(boot_devices(&config_with_root(root.path())).is_empty());
    }

    #[test]
    fn join_capped_drops_devices_from_the_end_without_mid_word_truncation() {
        let devices = vec![
            PathBuf::from("/x/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            PathBuf::from("/x/second"),
        ];
        let joined = join_capped(&devices);
        assert!(joined.chars().count() > MAX_LINE_LEN);
        assert!(!joined.contains(", second"));
    }
}
