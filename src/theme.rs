//! Embedded theme files and install.

use std::path::{Path, PathBuf};

pub const THEMES: [(&str, &str); 10] = [
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
    (
        "rainbows-and-unicorns-sorbet",
        include_str!("../themes/rainbows-and-unicorns-sorbet"),
    ),
];

/// Writes all ten files into `dir`, creating it. Returns the paths written.
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
pub const SHORT_NAMES: [(&str, &str); 10] = [
    ("six", "rainbows-and-unicorns"),
    ("paper", "rainbows-and-unicorns-paper"),
    ("ega", "rainbows-and-unicorns-ega"),
    ("workbench", "rainbows-and-unicorns-workbench"),
    ("mane", "rainbows-and-unicorns-mane"),
    ("miami", "rainbows-and-unicorns-miami"),
    ("arcade", "rainbows-and-unicorns-arcade"),
    ("vhs", "rainbows-and-unicorns-vhs"),
    ("den", "rainbows-and-unicorns-den"),
    ("sorbet", "rainbows-and-unicorns-sorbet"),
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

/// The display name `bios fetch`'s Theme row shows for each of our ten themes: the section
/// headings of `docs/theme.md`, the canonical display names.
pub const DISPLAY_NAMES: [(&str, &str); 10] = [
    ("rainbows-and-unicorns", "Six Stripes"),
    ("rainbows-and-unicorns-paper", "Paper White"),
    ("rainbows-and-unicorns-ega", "EGA '85"),
    ("rainbows-and-unicorns-workbench", "Workbench 1.0"),
    ("rainbows-and-unicorns-mane", "Mane"),
    ("rainbows-and-unicorns-miami", "Miami"),
    ("rainbows-and-unicorns-arcade", "Arcade"),
    ("rainbows-and-unicorns-vhs", "VHS"),
    ("rainbows-and-unicorns-den", "Den"),
    ("rainbows-and-unicorns-sorbet", "Sorbet"),
];

/// The display name for `full_name`, one of ours. `None` when `full_name` is not one of ours,
/// for instance an unrelated Ghostty theme the user has configured: the caller should fall back
/// to showing `full_name` itself rather than omitting it or guessing a prettier form.
pub fn display_name(full_name: &str) -> Option<&'static str> {
    DISPLAY_NAMES
        .iter()
        .find(|(name, _)| *name == full_name)
        .map(|(_, display)| *display)
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

/// The source of truth for the starship palettes: see `extras/starship-palette.toml`.
const STARSHIP_PALETTE_SOURCE: &str = include_str!("../extras/starship-palette.toml");

/// The starship palette name for `full_name`: the paper palette for Paper White, the palette
/// that follows the terminal's own colours for every other theme.
pub fn starship_palette_name(full_name: &str) -> &'static str {
    if full_name == "rainbows-and-unicorns-paper" {
        "rainbows_and_unicorns_paper"
    } else {
        "rainbows_and_unicorns_auto"
    }
}

/// The index of `lines`' top level `palette =` line (see
/// `tomledit::find_top_level_key_line`): one that is live TOML and appears before any live table
/// header, so a `palette` key nested inside a table such as `[palettes.foo]` does not count, and
/// nor does anything that merely looks like one inside a multi-line string. `None` when there is
/// no such line.
fn find_top_level_palette_line(lines: &[&str]) -> Option<usize> {
    crate::tomledit::find_top_level_key_line(lines, "palette")
}

/// The quote character used by `line`'s value: the first `'` or `"` found in it, `'` when
/// neither appears.
fn detect_quote(line: &str) -> char {
    line.chars()
        .find(|c| *c == '\'' || *c == '"')
        .unwrap_or('\'')
}

/// The `[palettes.<name>]` table from the embedded `extras/starship-palette.toml`: from its
/// header line up to the next line that starts a top level table, or the end of the file, with
/// trailing blank lines and trailing comment lines trimmed off the end (they belong to the next
/// table's write-up in the source file, not to this one; a comment sitting between key lines
/// would be kept, but none of the embedded tables have one). Panics when `name`'s table is not
/// there: a bug in this program, not a runtime failure, since the two names it ever asks for are
/// both baked into that file.
fn extract_palette_table(name: &str) -> String {
    let header = format!("[palettes.{name}]");
    let lines: Vec<&str> = STARSHIP_PALETTE_SOURCE.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == header)
        .unwrap_or_else(|| panic!("extras/starship-palette.toml has no {header} table"));
    let mut end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| crate::tomledit::is_table_header(line))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    while end > start + 1 {
        let trimmed = lines[end - 1].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            end -= 1;
        } else {
            break;
        }
    }
    lines[start..end].join("\n")
}

/// Points `path`'s top level `palette =` line (see `find_top_level_palette_line`) at `palette`,
/// preserving the line's original quote style and leaving every other byte alone (see
/// `tomledit::set_top_level_key`), and appends `palette`'s table (see `extract_palette_table`)
/// when a table of that exact name is not already present. Does nothing, returning `Ok(false)`,
/// when `path` does not exist, cannot be read, or has no top level `palette =` line. Writes
/// atomically: temp file then rename.
pub fn set_starship_palette(path: &Path, palette: &str) -> std::io::Result<bool> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(false);
    };
    let body = contents.strip_suffix('\n').unwrap_or(&contents);
    let lines: Vec<&str> = if contents.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };

    let Some(index) = find_top_level_palette_line(&lines) else {
        return Ok(false);
    };

    let quote = detect_quote(lines[index]);
    let value = format!("{quote}{palette}{quote}");
    let mut new_contents = crate::tomledit::set_top_level_key(&contents, "palette", &value);

    let table_header = format!("[palettes.{palette}]");
    let live = crate::tomledit::live_toml_lines(&lines);
    let has_table = lines
        .iter()
        .zip(live.iter())
        .any(|(line, is_live)| *is_live && line.trim() == table_header);

    if !has_table {
        if !new_contents.ends_with('\n') {
            new_contents.push('\n');
        }
        new_contents.push('\n');
        new_contents.push_str(&extract_palette_table(palette));
        new_contents.push('\n');
    }

    let pid = std::process::id();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = path.with_file_name(format!("{file_name}.{pid}.tmp"));
    std::fs::write(&tmp_path, new_contents)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_returns_the_display_name_for_each_theme() {
        assert_eq!(display_name("rainbows-and-unicorns"), Some("Six Stripes"));
        assert_eq!(
            display_name("rainbows-and-unicorns-paper"),
            Some("Paper White")
        );
        assert_eq!(display_name("rainbows-and-unicorns-ega"), Some("EGA '85"));
        assert_eq!(
            display_name("rainbows-and-unicorns-workbench"),
            Some("Workbench 1.0")
        );
        assert_eq!(display_name("rainbows-and-unicorns-mane"), Some("Mane"));
        assert_eq!(display_name("rainbows-and-unicorns-miami"), Some("Miami"));
        assert_eq!(display_name("rainbows-and-unicorns-arcade"), Some("Arcade"));
        assert_eq!(display_name("rainbows-and-unicorns-vhs"), Some("VHS"));
        assert_eq!(display_name("rainbows-and-unicorns-den"), Some("Den"));
        assert_eq!(display_name("rainbows-and-unicorns-sorbet"), Some("Sorbet"));
    }

    #[test]
    fn display_name_is_none_for_a_theme_that_is_not_one_of_ours() {
        assert_eq!(display_name("gruvbox_dark"), None);
    }

    #[test]
    fn every_theme_and_every_display_name_pair_up_with_no_gaps_on_either_side() {
        for (full_name, _) in THEMES {
            assert!(
                DISPLAY_NAMES.iter().any(|(name, _)| *name == full_name),
                "{full_name} is in THEMES but has no display name"
            );
        }
        for (full_name, _) in DISPLAY_NAMES {
            assert!(
                THEMES.iter().any(|(name, _)| *name == full_name),
                "{full_name} has a display name but is not in THEMES"
            );
        }
    }

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
    fn starship_palette_name_is_paper_only_for_the_paper_theme() {
        assert_eq!(
            starship_palette_name("rainbows-and-unicorns-paper"),
            "rainbows_and_unicorns_paper"
        );
        for other in [
            "rainbows-and-unicorns",
            "rainbows-and-unicorns-mane",
            "rainbows-and-unicorns-arcade",
        ] {
            assert_eq!(starship_palette_name(other), "rainbows_and_unicorns_auto");
        }
    }

    #[test]
    fn set_starship_palette_repoints_the_line_and_appends_the_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        std::fs::write(
            &path,
            "format = \"$all\"\n\npalette = 'gruvbox_dark'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n",
        )
        .unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with(
            "format = \"$all\"\n\npalette = 'rainbows_and_unicorns_auto'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n"
        ));
        assert!(contents.contains("[palettes.rainbows_and_unicorns_auto]"));
        assert_eq!(
            contents
                .matches("[palettes.rainbows_and_unicorns_auto]")
                .count(),
            1
        );
    }

    #[test]
    fn set_starship_palette_does_not_duplicate_an_existing_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        std::fs::write(
            &path,
            "palette = 'gruvbox_dark'\n\n[palettes.rainbows_and_unicorns_auto]\ncolor_fg0 = 'placeholder'\n",
        )
        .unwrap();
        set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents
                .matches("[palettes.rainbows_and_unicorns_auto]")
                .count(),
            1
        );
        assert!(contents.contains("color_fg0 = 'placeholder'"));
        assert!(contents.starts_with("palette = 'rainbows_and_unicorns_auto'\n"));
    }

    #[test]
    fn set_starship_palette_does_nothing_without_a_top_level_palette_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original = "format = \"$all\"\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(!changed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn set_starship_palette_ignores_a_nested_palette_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original = "[palettes.foo]\npalette = 'inner'\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(!changed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn set_starship_palette_ignores_a_commented_out_palette_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original = "# palette = 'gruvbox_dark'\nformat = \"$all\"\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(!changed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn set_starship_palette_finds_the_palette_line_after_a_multiline_basic_format_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        // Shaped like starship's real Gruvbox Rainbow preset: a `format` built from segment
        // syntax that is full of `[...]` lines, none of which are TOML table headers.
        let original = r#"format = """
[](color_orange)\
$os\
[](bg:color_yellow fg:color_orange)\
$directory\
[](fg:color_yellow bg:color_aqua)\
$git_branch\
[ ](fg:color_bg1)\
$line_break$character"""

palette = 'gruvbox_dark'

[palettes.gruvbox_dark]
color_fg0 = '#fbf1c7'
"#;
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\npalette = 'rainbows_and_unicorns_auto'\n"));
        assert!(contents.contains("[palettes.rainbows_and_unicorns_auto]"));
        // The segment syntax inside the format string survives untouched.
        assert!(contents.contains("[](color_orange)\\\n"));
    }

    #[test]
    fn set_starship_palette_handles_a_multiline_literal_string_the_same_way() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original = "junk = '''\n[not_a_table]\npalette = 'not the real one'\n'''\n\npalette = 'gruvbox_dark'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#fbf1c7'\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\npalette = 'rainbows_and_unicorns_auto'\n"));
        // The line inside the literal string is untouched, not mistaken for the real one.
        assert!(contents.contains("palette = 'not the real one'"));
    }

    #[test]
    fn set_starship_palette_is_not_stuck_inside_a_string_that_opens_and_closes_on_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original =
            "junk = \"\"\"same line\"\"\"\npalette = 'gruvbox_dark'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#fbf1c7'\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents
            .starts_with("junk = \"\"\"same line\"\"\"\npalette = 'rainbows_and_unicorns_auto'\n"));
    }

    #[test]
    fn set_starship_palette_ignores_a_palette_line_inside_a_multiline_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let original = "junk = \"\"\"\npalette = 'fake'\n\"\"\"\n\npalette = 'gruvbox_dark'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#fbf1c7'\n";
        std::fs::write(&path, original).unwrap();
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(changed);
        let contents = std::fs::read_to_string(&path).unwrap();
        // The one inside the string is left exactly as it was.
        assert!(contents.contains("palette = 'fake'"));
        // The genuine top level one, after the string closes, is the one that changed.
        assert!(contents.contains("\npalette = 'rainbows_and_unicorns_auto'\n"));
    }

    #[test]
    fn set_starship_palette_does_nothing_when_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        let changed = set_starship_palette(&path, "rainbows_and_unicorns_auto").unwrap();
        assert!(!changed);
        assert!(!path.exists());
    }

    #[test]
    fn set_starship_palette_preserves_double_quote_style() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("starship.toml");
        std::fs::write(&path, "palette = \"gruvbox_dark\"\n").unwrap();
        set_starship_palette(&path, "rainbows_and_unicorns_paper").unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("palette = \"rainbows_and_unicorns_paper\"\n"));
    }

    #[test]
    fn extract_palette_table_trims_the_next_tables_write_up_off_the_auto_table() {
        let table = extract_palette_table("rainbows_and_unicorns_auto");
        assert!(table.starts_with("[palettes.rainbows_and_unicorns_auto]\n"));
        assert!(table.ends_with("color_yellow = '3'"));
        assert!(!table.contains("rainbows_and_unicorns_paper"));
    }

    #[test]
    fn extract_palette_table_handles_the_last_table_in_the_file() {
        let table = extract_palette_table("rainbows_and_unicorns_paper");
        assert!(table.starts_with("[palettes.rainbows_and_unicorns_paper]\n"));
        assert!(table.ends_with("color_yellow = '3'"));
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
