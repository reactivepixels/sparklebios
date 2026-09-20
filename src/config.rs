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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            animate: true,
            flavour: "unicorn".to_string(),
            checks: true,
            project_dirs: Vec::new(),
            graphics: GraphicsPref::Auto,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    animate: Option<bool>,
    flavour: Option<String>,
    checks: Option<bool>,
    project_dirs: Option<Vec<String>>,
    graphics: Option<String>,
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
    }
}

/// Sets `flavour` in `<dir>/config.toml`, preserving every other key. Starts from an empty table
/// when the file is missing or invalid. Creates `dir` if missing, and writes atomically: temp
/// file then rename, the same way `state::save` does.
pub fn set_flavour(dir: &std::path::Path, id: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("config.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| contents.parse::<toml::Table>().ok())
        .unwrap_or_default();

    table.insert("flavour".to_string(), toml::Value::String(id.to_string()));

    let contents = toml::to_string(&table).map_err(std::io::Error::other)?;
    let pid = std::process::id();
    let tmp_path = dir.join(format!("config.toml.{pid}.tmp"));
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
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
    fn an_unknown_graphics_value_reads_as_auto() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "graphics = \"holographic\"\n",
        )
        .unwrap();
        assert_eq!(load(Some(dir.path())).graphics, GraphicsPref::Auto);
    }
}
