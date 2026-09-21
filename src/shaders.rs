//! The two Ghostty shaders `ultra` turns on: embedded files, install, and toggling their
//! `custom-shader` lines in a Ghostty config. Follows the shape of `theme.rs`'s own install and
//! config editing.

use std::path::{Path, PathBuf};

/// The two shader files, as `(file name, contents)` pairs, in the order they are installed, added
/// to a Ghostty config, and named in `bios sprinkles ultra`'s own line.
pub const SHADERS: [(&str, &str); 2] = [
    (
        "scanlines.glsl",
        include_str!("../extras/shaders/scanlines.glsl"),
    ),
    (
        "cursor-trail.glsl",
        include_str!("../extras/shaders/cursor-trail.glsl"),
    ),
];

/// The absolute path each shader file lives at once installed into `dir` (see `install`), in
/// `SHADERS`' order. Does not touch the filesystem: used both by `install` itself and by a caller
/// that only needs to know the paths a previous `install` would have written, for instance to
/// remove their `custom-shader` lines again.
pub fn paths_in(dir: &Path) -> Vec<PathBuf> {
    SHADERS.iter().map(|(name, _)| dir.join(name)).collect()
}

/// Writes both shader files into `dir`, creating it. Returns the paths written, in `SHADERS`'
/// order.
pub fn install(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let paths = paths_in(dir);
    for ((_, contents), path) in SHADERS.iter().zip(&paths) {
        std::fs::write(path, contents)?;
    }
    Ok(paths)
}

/// Whether `line` is a `custom-shader` directive whose value is exactly `path`: `custom-shader`,
/// then zero or more spaces, then `=`, then zero or more spaces, then `path` and nothing more. A
/// `custom-shader` line pointing at a different path does not match, so it is never touched by
/// `enable` or `disable`.
fn is_shader_line_for(line: &str, path: &str) -> bool {
    let Some(rest) = line.strip_prefix("custom-shader") else {
        return false;
    };
    let Some(rest) = rest.trim_start_matches(' ').strip_prefix('=') else {
        return false;
    };
    rest.trim_start_matches(' ') == path
}

/// Writes `contents` to `path` atomically: temp file then rename.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let pid = std::process::id();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.{pid}.tmp"));
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Splits `contents` into its lines and whether it ended in a trailing newline (or was empty,
/// which counts as one), the same way `theme::set_ghostty_theme` reads a config file.
fn split_lines(contents: &str) -> (Vec<&str>, bool) {
    let had_trailing_newline = contents.is_empty() || contents.ends_with('\n');
    let body = contents.strip_suffix('\n').unwrap_or(contents);
    let lines = if contents.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };
    (lines, had_trailing_newline)
}

/// Appends a `custom-shader = <path>` line for each of `paths` not already present (see
/// `is_shader_line_for`), leaving every other byte, including an unrelated `custom-shader` line,
/// alone. Creates `config_path`'s parent directories and the file itself when they do not exist.
/// Writes atomically: temp file then rename.
pub fn enable(config_path: &Path, paths: &[PathBuf]) -> std::io::Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = std::fs::read_to_string(config_path).unwrap_or_default();
    let (lines, had_trailing_newline) = split_lines(&contents);

    let mut out_lines: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
    for path in paths {
        let value = path.to_string_lossy().into_owned();
        if !lines.iter().any(|line| is_shader_line_for(line, &value)) {
            out_lines.push(format!("custom-shader = {value}"));
        }
    }

    let mut new_contents = out_lines.join("\n");
    if had_trailing_newline {
        new_contents.push('\n');
    }
    write_atomic(config_path, &new_contents)
}

/// Removes any `custom-shader = <path>` line for each of `paths` (see `is_shader_line_for`),
/// leaving every other byte, including an unrelated `custom-shader` line, alone. Does nothing when
/// `config_path` cannot be read. Writes atomically: temp file then rename.
pub fn disable(config_path: &Path, paths: &[PathBuf]) -> std::io::Result<()> {
    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return Ok(());
    };
    let (lines, had_trailing_newline) = split_lines(&contents);

    let values: Vec<String> = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let out_lines: Vec<&str> = lines
        .into_iter()
        .filter(|line| !values.iter().any(|value| is_shader_line_for(line, value)))
        .collect();

    let mut new_contents = out_lines.join("\n");
    if had_trailing_newline && !new_contents.is_empty() {
        new_contents.push('\n');
    }
    write_atomic(config_path, &new_contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_both_files_and_returns_their_paths() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("shaders");
        let paths = install(&target).unwrap();
        assert_eq!(
            paths,
            vec![
                target.join("scanlines.glsl"),
                target.join("cursor-trail.glsl"),
            ]
        );
        for path in &paths {
            assert!(path.is_file(), "{}", path.display());
        }
        assert!(std::fs::read_to_string(&paths[0])
            .unwrap()
            .contains("mainImage"));
    }

    #[test]
    fn is_shader_line_for_matches_exact_value_with_flexible_spacing() {
        assert!(is_shader_line_for("custom-shader = /a/b.glsl", "/a/b.glsl"));
        assert!(is_shader_line_for(
            "custom-shader   =   /a/b.glsl",
            "/a/b.glsl"
        ));
        assert!(is_shader_line_for("custom-shader=/a/b.glsl", "/a/b.glsl"));
        assert!(!is_shader_line_for(
            "custom-shader = /a/c.glsl",
            "/a/b.glsl"
        ));
        assert!(!is_shader_line_for("theme = /a/b.glsl", "/a/b.glsl"));
        assert!(!is_shader_line_for(
            "custom-shader-extra = /a/b.glsl",
            "/a/b.glsl"
        ));
    }

    #[test]
    fn enable_appends_the_missing_lines_and_leaves_the_rest_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "font-size = 14\ncursor-style = block\n").unwrap();
        let shaders_dir = dir.path().join("shaders");
        let paths = install(&shaders_dir).unwrap();
        enable(&path, &paths).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents,
            format!(
                "font-size = 14\ncursor-style = block\ncustom-shader = {}\ncustom-shader = {}\n",
                paths[0].display(),
                paths[1].display()
            )
        );
    }

    #[test]
    fn enable_twice_does_not_duplicate_the_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "font-size = 14\n").unwrap();
        let paths = paths_in(&dir.path().join("shaders"));
        enable(&path, &paths).unwrap();
        enable(&path, &paths).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches("custom-shader").count(), 2);
    }

    #[test]
    fn enable_leaves_an_unrelated_custom_shader_line_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "custom-shader = /mine/own.glsl\n").unwrap();
        let paths = paths_in(&dir.path().join("shaders"));
        enable(&path, &paths).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("custom-shader = /mine/own.glsl"));
        assert_eq!(contents.matches("custom-shader").count(), 3);
    }

    #[test]
    fn disable_removes_exactly_the_two_lines_and_restores_the_original_byte_for_byte() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let original = "font-size = 14\ntheme = old-theme\ncursor-style = block\n";
        std::fs::write(&path, original).unwrap();
        let paths = paths_in(&dir.path().join("shaders"));
        enable(&path, &paths).unwrap();
        assert_ne!(std::fs::read_to_string(&path).unwrap(), original);
        disable(&path, &paths).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn disable_leaves_an_unrelated_custom_shader_line_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "custom-shader = /mine/own.glsl\n").unwrap();
        let paths = paths_in(&dir.path().join("shaders"));
        enable(&path, &paths).unwrap();
        disable(&path, &paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "custom-shader = /mine/own.glsl\n"
        );
    }

    #[test]
    fn disable_does_nothing_when_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        let paths = paths_in(&dir.path().join("shaders"));
        disable(&path, &paths).unwrap();
        assert!(!path.exists());
    }
}
