//! Embedded theme files and install.

use std::path::{Path, PathBuf};

pub const THEMES: [(&str, &str); 9] = [
    (
        "rainbows-and-unicorns",
        include_str!("../themes/rainbows-and-unicorns"),
    ),
    (
        "rainbows-and-unicorns-paper",
        include_str!("../themes/rainbows-and-unicorns-paper"),
    ),
    (
        "rainbows-and-unicorns-ega",
        include_str!("../themes/rainbows-and-unicorns-ega"),
    ),
    (
        "rainbows-and-unicorns-workbench",
        include_str!("../themes/rainbows-and-unicorns-workbench"),
    ),
    (
        "rainbows-and-unicorns-mane",
        include_str!("../themes/rainbows-and-unicorns-mane"),
    ),
    (
        "rainbows-and-unicorns-miami",
        include_str!("../themes/rainbows-and-unicorns-miami"),
    ),
    (
        "rainbows-and-unicorns-arcade",
        include_str!("../themes/rainbows-and-unicorns-arcade"),
    ),
    (
        "rainbows-and-unicorns-vhs",
        include_str!("../themes/rainbows-and-unicorns-vhs"),
    ),
    (
        "rainbows-and-unicorns-den",
        include_str!("../themes/rainbows-and-unicorns-den"),
    ),
];

/// Writes all nine files into `dir`, creating it. Returns the paths written.
pub fn install(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut paths = Vec::with_capacity(THEMES.len());
    for (name, contents) in THEMES {
        let path = dir.join(name);
        std::fs::write(&path, contents)?;
        paths.push(path);
    }
    Ok(paths)
}

/// The short name `bios theme use` accepts for each theme, in the order `bios theme list` shows
/// them.
pub const SHORT_NAMES: [(&str, &str); 9] = [
    ("six", "rainbows-and-unicorns"),
    ("paper", "rainbows-and-unicorns-paper"),
    ("ega", "rainbows-and-unicorns-ega"),
    ("workbench", "rainbows-and-unicorns-workbench"),
    ("mane", "rainbows-and-unicorns-mane"),
    ("miami", "rainbows-and-unicorns-miami"),
    ("arcade", "rainbows-and-unicorns-arcade"),
    ("vhs", "rainbows-and-unicorns-vhs"),
    ("den", "rainbows-and-unicorns-den"),
];

/// Resolves `name`, a short name or a full theme name, to its full theme name. `None` when
/// `name` is neither.
pub fn resolve_name(name: &str) -> Option<&'static str> {
    if let Some((_, full)) = SHORT_NAMES.iter().find(|(short, _)| *short == name) {
        return Some(full);
    }
    THEMES
        .iter()
        .find(|(full, _)| *full == name)
        .map(|(full, _)| *full)
}

/// Whether `line` starts with `theme`, then zero or more spaces, then `=`: Ghostty's own syntax
/// for setting the theme.
fn is_theme_line(line: &str) -> bool {
    match line.strip_prefix("theme") {
        Some(rest) => rest.trim_start_matches(' ').starts_with('='),
        None => false,
    }
}

/// The config file to edit: the first of `candidates` whose contents already have a `theme` line
/// (see `is_theme_line`); failing that, the first candidate that exists at all; `None` when no
/// candidate exists.
pub fn pick_config_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Ok(contents) = std::fs::read_to_string(candidate) {
            if contents.lines().any(is_theme_line) {
                return Some(candidate.clone());
            }
        }
    }
    candidates.iter().find(|c| c.exists()).cloned()
}

/// Replaces `path`'s `theme` line with `theme = <full_name>` (see `is_theme_line`), or appends it
/// when the file has no such line, leaving every other byte alone. Creates `path`'s parent
/// directories and the file itself when they do not exist. Writes atomically: temp file then
/// rename.
pub fn set_ghostty_theme(path: &Path, full_name: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    let had_trailing_newline = contents.is_empty() || contents.ends_with('\n');
    let body = contents.strip_suffix('\n').unwrap_or(&contents);
    let lines: Vec<&str> = if contents.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };

    let new_line = format!("theme = {full_name}");
    let mut found = false;
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len() + 1);
    for line in &lines {
        if !found && is_theme_line(line) {
            out_lines.push(new_line.clone());
            found = true;
        } else {
            out_lines.push((*line).to_string());
        }
    }
    if !found {
        out_lines.push(new_line);
    }

    let mut new_contents = out_lines.join("\n");
    if had_trailing_newline {
        new_contents.push('\n');
    }

    let pid = std::process::id();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.{pid}.tmp"));
    std::fs::write(&tmp_path, new_contents)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Installs the theme files into `themes_dir` (see `install`), then points `config_path`'s
/// `theme` line at `full_name` (see `set_ghostty_theme`).
pub fn install_and_use(
    themes_dir: &Path,
    config_path: &Path,
    full_name: &str,
) -> std::io::Result<()> {
    install(themes_dir)?;
    set_ghostty_theme(config_path, full_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_name_accepts_short_and_full_names() {
        assert_eq!(resolve_name("mane"), Some("rainbows-and-unicorns-mane"));
        assert_eq!(
            resolve_name("rainbows-and-unicorns-paper"),
            Some("rainbows-and-unicorns-paper")
        );
        assert_eq!(resolve_name("nope"), None);
    }

    #[test]
    fn pick_config_path_prefers_a_candidate_with_a_theme_line() {
        let dir = tempfile::tempdir().unwrap();
        let no_theme = dir.path().join("no_theme");
        let has_theme = dir.path().join("has_theme");
        std::fs::write(&no_theme, "font-size = 14\n").unwrap();
        std::fs::write(&has_theme, "theme = old\n").unwrap();
        let candidates = vec![no_theme.clone(), has_theme.clone()];
        assert_eq!(pick_config_path(&candidates), Some(has_theme));
    }

    #[test]
    fn pick_config_path_falls_back_to_the_first_existing_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let exists = dir.path().join("exists");
        let missing = dir.path().join("missing");
        std::fs::write(&exists, "font-size = 14\n").unwrap();
        let candidates = vec![exists.clone(), missing];
        assert_eq!(pick_config_path(&candidates), Some(exists));
    }

    #[test]
    fn pick_config_path_is_none_when_nothing_exists() {
        let dir = tempfile::tempdir().unwrap();
        let candidates = vec![dir.path().join("a"), dir.path().join("b")];
        assert_eq!(pick_config_path(&candidates), None);
    }

    #[test]
    fn set_ghostty_theme_replaces_the_line_and_leaves_the_rest_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(
            &path,
            "font-size = 14\ntheme = old-theme\ncursor-style = block\n",
        )
        .unwrap();
        set_ghostty_theme(&path, "rainbows-and-unicorns-mane").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "font-size = 14\ntheme = rainbows-and-unicorns-mane\ncursor-style = block\n"
        );
    }

    #[test]
    fn set_ghostty_theme_matches_a_theme_line_with_extra_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "theme   = old-theme\n").unwrap();
        set_ghostty_theme(&path, "rainbows-and-unicorns").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "theme = rainbows-and-unicorns\n"
        );
    }

    #[test]
    fn set_ghostty_theme_appends_when_there_is_no_theme_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "font-size = 14\n").unwrap();
        set_ghostty_theme(&path, "rainbows-and-unicorns").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "font-size = 14\ntheme = rainbows-and-unicorns\n"
        );
    }

    #[test]
    fn set_ghostty_theme_creates_a_missing_file_and_its_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/config");
        set_ghostty_theme(&path, "rainbows-and-unicorns").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "theme = rainbows-and-unicorns\n"
        );
    }

    #[test]
    fn install_and_use_installs_files_and_sets_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        let config_path = dir.path().join("config");
        install_and_use(&themes_dir, &config_path, "rainbows-and-unicorns-ega").unwrap();
        assert!(themes_dir.join("rainbows-and-unicorns-ega").is_file());
        assert_eq!(
            std::fs::read_to_string(&config_path).unwrap(),
            "theme = rainbows-and-unicorns-ega\n"
        );
    }
}
