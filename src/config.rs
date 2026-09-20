//! User config: `~/.config/sparklebios/config.toml`.

use serde::Deserialize;

/// How the boot mascot is drawn: `Auto` picks a Kitty image where the terminal supports it and
/// the half-block mascot everywhere else (today's behaviour); `Image` names that same choice
/// explicitly; `Blocks` always draws the half-block mascot, even in a Kitty-capable terminal,
/// since some terminals drop a Kitty image when their tab goes to sleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsPref {
    Auto,
    Image,
    Blocks,
}

impl GraphicsPref {
    /// An unknown or malformed value reads as `Auto`, the same tolerant way every other key here
    /// is read. Used for both the config file's `graphics` key and the `SPARKLEBIOS_GRAPHICS`
    /// environment override.
    pub(crate) fn parse(s: &str) -> GraphicsPref {
        match s {
            "image" => GraphicsPref::Image,
            "blocks" => GraphicsPref::Blocks,
            _ => GraphicsPref::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub animate: bool,
    pub flavour: String,
    pub checks: bool,
    pub project_dirs: Vec<String>,
    pub graphics: GraphicsPref,
    pub sprinkles: crate::sprinkles::Level,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            animate: true,
            flavour: "unicorn".to_string(),
            checks: true,
            project_dirs: Vec::new(),
            graphics: GraphicsPref::Auto,
            sprinkles: crate::sprinkles::Level::Off,
        }
    }
}

/// The commented default config file: what `bios config reset` writes, and what
/// `bios config edit` creates when no config file exists yet. Every value here matches
/// `Config::default()`; `the_template_parses_to_exactly_the_default` asserts that.
pub const TEMPLATE: &str = "\
# SparkleBIOS configuration.
#
# Every key is optional. A missing key falls back to the default shown here, and
# a file that cannot be read or parsed is ignored rather than reported, because
# nothing on the boot path is allowed to complain.

# Which personality boots. Run `bios flavours` for the roster.
flavour = \"unicorn\"

# Whether the first boot of the day is animated. Every other boot is drawn
# instantly.
animate = true

# How the mascot is drawn. \"auto\" uses an image where the terminal supports one,
# \"image\" asks for the image, and \"blocks\" always draws it with text. Set this
# to \"blocks\" if the mascot disappears when a tab goes to sleep.
graphics = \"auto\"

# Whether the health checks run. False turns off the checks, the findings on the
# boot screen, and the background refresh that feeds them.
checks = true

# Where to look for your git repositories. The boot screen lists the three
# touched most recently. Empty means the built-in list: ~/Code, ~/code,
# ~/Projects, ~/projects, ~/src, ~/dev, ~/Developer, ~/repos, ~/work and ~/git.
project_dirs = []

# Sprinkles: optional effects during the once-a-day animated boot.
# \"off\", \"light\" (text effects) or \"full\" (text effects and sound).
sprinkles = \"off\"
";

/// `<dir>/config.toml`.
pub fn path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("config.toml")
}

/// Writes `contents` to `path` atomically: a temp file beside it, then rename. Leaves no temp
/// file behind.
fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config.toml");
    let pid = std::process::id();
    let tmp_path = dir.join(format!("{file_name}.{pid}.tmp"));
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Creates `<dir>/config.toml` from `TEMPLATE` when it does not already exist, creating `dir`
/// too. An existing file, of any contents, is left untouched. Returns whether a file was
/// created.
pub fn ensure_exists(dir: &std::path::Path) -> std::io::Result<bool> {
    std::fs::create_dir_all(dir)?;
    let file = path(dir);
    if file.exists() {
        return Ok(false);
    }
    write_atomic(&file, TEMPLATE)?;
    Ok(true)
}

/// Resets `<dir>/config.toml` to `TEMPLATE`, creating `dir` if missing. When a config file
/// already exists and its contents differ from `TEMPLATE`, its old contents are preserved first
/// at `<dir>/config.toml.bak`, replacing any previous backup; a file already identical to the
/// template produces no backup. Both files are written atomically: a temp file beside the
/// target, then rename.
pub fn reset(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let file = path(dir);
    if let Ok(existing) = std::fs::read_to_string(&file) {
        if existing != TEMPLATE {
            write_atomic(&dir.join("config.toml.bak"), &existing)?;
        }
    }
    write_atomic(&file, TEMPLATE)
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    animate: Option<bool>,
    flavour: Option<String>,
    checks: Option<bool>,
    project_dirs: Option<Vec<String>>,
    graphics: Option<String>,
    sprinkles: Option<String>,
}

/// Missing, unreadable or invalid file yields the default. Unknown keys, including the retired
/// `full` and `fast`, are ignored.
pub fn load(dir: Option<&std::path::Path>) -> Config {
    let default = Config::default();
    let Some(dir) = dir else {
        return default;
    };
    let Ok(contents) = std::fs::read_to_string(dir.join("config.toml")) else {
        return default;
    };
    let Ok(raw) = toml::from_str::<RawConfig>(&contents) else {
        return default;
    };
    Config {
        animate: raw.animate.unwrap_or(default.animate),
        flavour: raw.flavour.unwrap_or(default.flavour),
        checks: raw.checks.unwrap_or(default.checks),
        project_dirs: raw.project_dirs.unwrap_or(default.project_dirs),
        graphics: raw
            .graphics
            .as_deref()
            .map(GraphicsPref::parse)
            .unwrap_or(default.graphics),
        sprinkles: raw
            .sprinkles
            .as_deref()
            .map(crate::sprinkles::Level::parse)
            .unwrap_or(default.sprinkles),
    }
}

/// The contents to hand-edit `key`'s value into: `path`'s own contents when it exists and parses
/// as TOML, `TEMPLATE` otherwise (a missing or unparsable file falls back to the commented
/// template rather than a bare one-liner, so the user ends up with the documented file either
/// way).
fn edit_base(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .ok()
        .filter(|contents| contents.parse::<toml::Table>().is_ok())
        .unwrap_or_else(|| TEMPLATE.to_string())
}

/// Sets `flavour` in `<dir>/config.toml`, preserving every other key, comment, blank line and the
/// existing key order: a hand edit of the `flavour` line (see
/// `tomledit::set_top_level_key`), never a parse-and-reserialise, which would destroy all of
/// that. Starts from `TEMPLATE` when the file is missing or invalid (see `edit_base`). Creates
/// `dir` if missing, and writes atomically: temp file then rename, the same way `state::save`
/// does.
pub fn set_flavour(dir: &std::path::Path, id: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("config.toml");
    let contents = edit_base(&path);
    let new_contents =
        crate::tomledit::set_top_level_key(&contents, "flavour", &format!("\"{id}\""));
    write_atomic(&path, &new_contents)
}

/// Sets `sprinkles` in `<dir>/config.toml` to `level.as_str()`, preserving every other key, the
/// same way `set_flavour` does.
pub fn set_sprinkles(dir: &std::path::Path, level: crate::sprinkles::Level) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("config.toml");
    let contents = edit_base(&path);
    let new_contents = crate::tomledit::set_top_level_key(
        &contents,
        "sprinkles",
        &format!("\"{}\"", level.as_str()),
    );
    write_atomic(&path, &new_contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_dir_yields_the_default() {
        assert_eq!(load(None), Config::default());
    }

    #[test]
    fn a_missing_file_yields_the_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(Some(dir.path())), Config::default());
    }

    #[test]
    fn an_invalid_file_yields_the_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "full = ").unwrap();
        assert_eq!(load(Some(dir.path())), Config::default());
    }

    #[test]
    fn reads_animate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "animate = false\n").unwrap();
        assert_eq!(
            load(Some(dir.path())),
            Config {
                animate: false,
                flavour: "unicorn".to_string(),
                checks: true,
                project_dirs: Vec::new(),
                graphics: GraphicsPref::Auto,
                sprinkles: crate::sprinkles::Level::Off,
            }
        );
    }

    #[test]
    fn flavour_defaults_to_unicorn_and_is_read() {
        assert_eq!(Config::default().flavour, "unicorn");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "flavour = \"sumo\"\n").unwrap();
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
    }

    #[test]
    fn checks_defaults_to_true_and_is_read() {
        assert!(Config::default().checks);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "checks = false\n").unwrap();
        assert!(!load(Some(dir.path())).checks);
    }

    #[test]
    fn project_dirs_defaults_to_empty_and_is_read() {
        assert!(Config::default().project_dirs.is_empty());
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "project_dirs = [\"~/Code\", \"~/work\"]\n",
        )
        .unwrap();
        assert_eq!(
            load(Some(dir.path())).project_dirs,
            vec!["~/Code".to_string(), "~/work".to_string()]
        );
    }

    #[test]
    fn set_flavour_preserves_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "animate = false\nfuture_field = true\nchecks = false\nproject_dirs = [\"~/work\"]\n",
        )
        .unwrap();
        set_flavour(dir.path(), "sumo").unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(contents.contains("animate = false"));
        assert!(contents.contains("future_field = true"));
        assert!(contents.contains("checks = false"));
        assert!(contents.contains("project_dirs"));
        let loaded = load(Some(dir.path()));
        assert_eq!(loaded.flavour, "sumo");
        assert!(!loaded.checks);
        assert_eq!(loaded.project_dirs, vec!["~/work".to_string()]);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "flavour = \"sumo\"\nfuture_field = true\n",
        )
        .unwrap();
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
    }

    #[test]
    fn the_retired_full_and_fast_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "full = \"c64\"\nfast = \"pc85\"\nanimate = false\n",
        )
        .unwrap();
        assert_eq!(
            load(Some(dir.path())),
            Config {
                animate: false,
                flavour: "unicorn".to_string(),
                checks: true,
                project_dirs: Vec::new(),
                graphics: GraphicsPref::Auto,
                sprinkles: crate::sprinkles::Level::Off,
            }
        );
    }

    #[test]
    fn set_flavour_creates_the_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        set_flavour(&nested, "sumo").unwrap();
        assert_eq!(load(Some(&nested)).flavour, "sumo");
        assert!(nested.join("config.toml").is_file());
    }

    #[test]
    fn set_flavour_replaces_an_invalid_existing_file_without_erroring() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "flavour = ").unwrap();
        set_flavour(dir.path(), "sumo").unwrap();
        assert_eq!(
            load(Some(dir.path())),
            Config {
                animate: true,
                flavour: "sumo".to_string(),
                checks: true,
                project_dirs: Vec::new(),
                graphics: GraphicsPref::Auto,
                sprinkles: crate::sprinkles::Level::Off,
            }
        );
    }

    #[test]
    fn graphics_defaults_to_auto_and_reads_all_three_values() {
        assert_eq!(Config::default().graphics, GraphicsPref::Auto);
        for (value, expected) in [
            ("auto", GraphicsPref::Auto),
            ("image", GraphicsPref::Image),
            ("blocks", GraphicsPref::Blocks),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("config.toml"),
                format!("graphics = \"{value}\"\n"),
            )
            .unwrap();
            assert_eq!(load(Some(dir.path())).graphics, expected, "{value}");
        }
    }

    #[test]
    fn the_template_parses_to_exactly_the_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), TEMPLATE).unwrap();
        assert_eq!(load(Some(dir.path())), Config::default());
    }

    #[test]
    fn ensure_exists_creates_the_template_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        assert!(ensure_exists(&nested).unwrap());
        assert_eq!(
            std::fs::read_to_string(nested.join("config.toml")).unwrap(),
            TEMPLATE
        );
    }

    #[test]
    fn ensure_exists_leaves_an_existing_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "flavour = \"sumo\"\n").unwrap();
        assert!(!ensure_exists(dir.path()).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.toml")).unwrap(),
            "flavour = \"sumo\"\n"
        );
    }

    #[test]
    fn reset_writes_the_template() {
        let dir = tempfile::tempdir().unwrap();
        reset(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.toml")).unwrap(),
            TEMPLATE
        );
    }

    #[test]
    fn reset_over_a_different_file_leaves_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "flavour = \"sumo\"\n").unwrap();
        reset(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.toml.bak")).unwrap(),
            "flavour = \"sumo\"\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.toml")).unwrap(),
            TEMPLATE
        );
    }

    #[test]
    fn reset_over_a_file_identical_to_the_template_writes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), TEMPLATE).unwrap();
        reset(dir.path()).unwrap();
        assert!(!dir.path().join("config.toml.bak").exists());
    }

    #[test]
    fn reset_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "flavour = \"sumo\"\n").unwrap();
        reset(dir.path()).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["config.toml", "config.toml.bak"]);
    }

    #[test]
    fn an_unknown_graphics_value_reads_as_auto() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "graphics = \"holographic\"\n",
        )
        .unwrap();
        assert_eq!(load(Some(dir.path())).graphics, GraphicsPref::Auto);
    }

    #[test]
    fn sprinkles_defaults_to_off_and_reads_each_level() {
        use crate::sprinkles::Level;
        assert_eq!(Config::default().sprinkles, Level::Off);
        for (value, expected) in [
            ("off", Level::Off),
            ("light", Level::Light),
            ("full", Level::Full),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("config.toml"),
                format!("sprinkles = \"{value}\"\n"),
            )
            .unwrap();
            assert_eq!(load(Some(dir.path())).sprinkles, expected, "{value}");
        }
    }

    #[test]
    fn an_unknown_sprinkles_value_reads_as_off() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "sprinkles = \"disco\"\n").unwrap();
        assert_eq!(
            load(Some(dir.path())).sprinkles,
            crate::sprinkles::Level::Off
        );
    }

    #[test]
    fn set_sprinkles_preserves_other_keys() {
        use crate::sprinkles::Level;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "animate = false\nflavour = \"sumo\"\n",
        )
        .unwrap();
        set_sprinkles(dir.path(), Level::Full).unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(contents.contains("animate = false"));
        assert!(contents.contains("flavour = \"sumo\""));
        let loaded = load(Some(dir.path()));
        assert_eq!(loaded.sprinkles, Level::Full);
        assert_eq!(loaded.flavour, "sumo");
    }
}
