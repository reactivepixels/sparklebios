//! User config: `~/.config/sparklebios/config.toml`.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub animate: bool,
    pub flavour: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            animate: true,
            flavour: "unicorn".to_string(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    animate: Option<bool>,
    flavour: Option<String>,
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
    fn set_flavour_preserves_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "animate = false\nfuture_field = true\n",
        )
        .unwrap();
        set_flavour(dir.path(), "sumo").unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(contents.contains("animate = false"));
        assert!(contents.contains("future_field = true"));
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
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
            }
        );
    }
}
