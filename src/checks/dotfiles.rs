//! Dotfiles changed: a tracked dotfiles repository with work sitting in it uncommitted.

use std::path::{Path, PathBuf};

use super::{plural, Finding, Severity};

const FINDING_TTL: u64 = 57600;

/// Where a dotfiles repository usually lives, in the order they are tried. `HOME` itself is last:
/// some people track their home directory directly, and that is a real answer, but a repository
/// under a conventional name is the more likely one.
const CANDIDATE_LEAVES: &[&str] = &[".dotfiles", "dotfiles", ".config"];

pub(crate) fn check(home: Option<&str>) -> Option<Finding> {
    let dir = find_dotfiles_repo(home?)?;
    let count = dirty_count(&dir)?;
    if count == 0 {
        return None;
    }

    let mut facts = std::collections::BTreeMap::new();
    facts.insert("dotfiles.repo".to_string(), dir_name(&dir));
    facts.insert(
        "dotfiles.changes".to_string(),
        plural(count, "uncommitted change", "uncommitted changes"),
    );
    Some(Finding {
        id: "dotfiles_changed".to_string(),
        severity: Severity::Info,
        ttl: FINDING_TTL,
        facts,
    })
}

/// The first candidate under `home` that is a git repository, or `home` itself when it is one.
fn find_dotfiles_repo(home: &str) -> Option<PathBuf> {
    let home = PathBuf::from(home);
    for leaf in CANDIDATE_LEAVES {
        let candidate = home.join(leaf);
        if candidate.join(".git").exists() {
            return Some(candidate);
        }
    }
    home.join(".git").exists().then_some(home)
}

/// The number of changed paths in `dir`. `--no-optional-locks` keeps git from refreshing the
/// index on disk, which would make this probe look like activity to the boot device check.
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

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_home_with_no_dotfiles_repository_anywhere_yields_nothing() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(find_dotfiles_repo(home.path().to_str().unwrap()), None);
        assert!(check(home.path().to_str()).is_none());
    }

    #[test]
    fn the_conventional_names_are_found_in_order() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("dotfiles/.git")).unwrap();
        assert_eq!(
            find_dotfiles_repo(home.path().to_str().unwrap()),
            Some(home.path().join("dotfiles"))
        );
        // A .dotfiles directory is tried before a plain dotfiles one.
        std::fs::create_dir_all(home.path().join(".dotfiles/.git")).unwrap();
        assert_eq!(
            find_dotfiles_repo(home.path().to_str().unwrap()),
            Some(home.path().join(".dotfiles"))
        );
    }

    #[test]
    fn a_home_directory_that_is_itself_a_repository_counts_last() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".git")).unwrap();
        assert_eq!(
            find_dotfiles_repo(home.path().to_str().unwrap()),
            Some(home.path().to_path_buf())
        );
    }

    #[test]
    fn no_home_means_no_finding() {
        assert!(check(None).is_none());
    }
}
