//! The virus scan: tracked secrets in a boot device's git repository.

use std::path::{Path, PathBuf};

use super::{Finding, Severity};

const FINDING_TTL: u64 = 86400;

const NEVER_HIT_SUFFIXES: &[&str] = &[".pub", ".example", ".sample", ".template", ".dist"];
const NEVER_HIT_EXTENSIONS: &[&str] = &["crt", "cert", "cer"];
const EXACT_HIT_NAMES: &[&str] = &[
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "credentials.json",
    ".netrc",
    ".pgpass",
    "secrets.yml",
    "secrets.yaml",
];
const HIT_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "jks"];
const NEVER_HIT_SEGMENTS: &[&str] = &[
    "test",
    "tests",
    "fixture",
    "fixtures",
    "__fixtures__",
    "example",
    "examples",
    "testdata",
    "node_modules",
    "vendor",
    "target",
];

pub(crate) fn check(devices: &[PathBuf]) -> Option<Finding> {
    for dir in devices {
        let hits = list_hits(dir);
        if hits.is_empty() {
            continue;
        }

        let repo = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let count = hits.len();
        let id = if count == 1 {
            "virus_one"
        } else {
            "virus_many"
        };

        let mut facts = std::collections::BTreeMap::new();
        facts.insert("virus.repo".to_string(), repo);
        facts.insert("virus.file".to_string(), hits[0].clone());
        facts.insert("virus.count".to_string(), count.to_string());

        return Some(Finding {
            id: id.to_string(),
            severity: Severity::Fail,
            ttl: FINDING_TTL,
            facts,
        });
    }
    None
}

/// The file names of every tracked path in `dir` that matches `is_secret`.
fn list_hits(dir: &Path) -> Vec<String> {
    let Ok(output) = std::process::Command::new("git")
        .args(["--no-optional-locks", "ls-files", "-z"])
        .current_dir(dir)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split('\0')
        .filter(|p| !p.is_empty())
        .filter(|p| is_secret(p))
        .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
        .collect()
}

/// Whether a git-tracked path (forward slash separated) is a tracked secret.
fn is_secret(path: &str) -> bool {
    if path.split('/').any(|seg| NEVER_HIT_SEGMENTS.contains(&seg)) {
        return false;
    }

    let name = path.rsplit('/').next().unwrap_or(path);
    if NEVER_HIT_SUFFIXES.iter().any(|suf| name.ends_with(suf)) {
        return false;
    }

    let extension = name.rsplit_once('.').map(|(_, ext)| ext);
    if extension.is_some_and(|ext| NEVER_HIT_EXTENSIONS.contains(&ext)) {
        return false;
    }

    if name == ".env" || (name.starts_with(".env.") && name.len() > ".env.".len()) {
        return true;
    }
    if EXACT_HIT_NAMES.contains(&name) {
        return true;
    }
    if name.starts_with("service-account") && name.ends_with(".json") {
        return true;
    }
    extension.is_some_and(|ext| HIT_EXTENSIONS.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits() {
        for path in [
            ".env",
            ".env.local",
            "certs/server.key",
            "id_rsa",
            "service-account-prod.json",
            "config/secrets.yml",
        ] {
            assert!(is_secret(path), "{path} should be a hit");
        }
    }

    #[test]
    fn misses() {
        for path in [
            ".env.example",
            "id_rsa.pub",
            "tests/fixtures/id_rsa",
            "node_modules/x/.env",
            "server.crt",
            "README.md",
            "src/key.rs",
        ] {
            assert!(!is_secret(path), "{path} should not be a hit");
        }
    }
}
