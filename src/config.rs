//! User config: `~/.config/sparklebios/config.toml`.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub full: String,
    pub fast: String,
    pub animate: bool,
    pub flavour: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            full: "pc95".to_string(),
            fast: "pc85".to_string(),
            animate: true,
            flavour: "unicorn".to_string(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    full: Option<String>,
    fast: Option<String>,
    animate: Option<bool>,
    flavour: Option<String>,
}

/// Missing, unreadable or invalid file yields the default. Unknown keys are ignored.
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
        full: raw.full.unwrap_or(default.full),
        fast: raw.fast.unwrap_or(default.fast),
        animate: raw.animate.unwrap_or(default.animate),
        flavour: raw.flavour.unwrap_or(default.flavour),
    }
}

/// Sets or clears `full` and `fast` in `<dir>/config.toml`, preserving every other key. Starts
/// from an empty table when the file is missing or invalid. `reset` removes both keys; otherwise
/// each of `full` and `fast` is set when given and left alone when `None`. Creates `dir` if
/// missing, and writes atomically: temp file then rename, the same way `state::save` does.
pub fn set_machines(
    dir: &std::path::Path,
    full: Option<&str>,
    fast: Option<&str>,
    reset: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("config.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| contents.parse::<toml::Table>().ok())
        .unwrap_or_default();

    if reset {
        table.remove("full");
        table.remove("fast");
    } else {
        if let Some(full) = full {
            table.insert("full".to_string(), toml::Value::String(full.to_string()));
        }
        if let Some(fast) = fast {
            table.insert("fast".to_string(), toml::Value::String(fast.to_string()));
        }
    }

    let contents = toml::to_string(&table).map_err(std::io::Error::other)?;
    let pid = std::process::id();
    let tmp_path = dir.join(format!("config.toml.{pid}.tmp"));
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Sets `flavour` in `<dir>/config.toml`, preserving every other key. Starts from an empty table
/// when the file is missing or invalid. Creates `dir` if missing, and writes atomically: temp
/// file then rename, the same way `set_machines` does.
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
    fn reads_full_fast_and_animate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "full = \"c64\"\nfast = \"pc95\"\nanimate = false\n",
        )
        .unwrap();
        assert_eq!(
            load(Some(dir.path())),
            Config {
                full: "c64".to_string(),
                fast: "pc95".to_string(),
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
            "full = \"c64\"\nanimate = false\n",
        )
        .unwrap();
        set_flavour(dir.path(), "sumo").unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(contents.contains("full = \"c64\""));
        assert!(contents.contains("animate = false"));
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
    }

    #[test]
    fn set_machines_preserves_flavour_and_reset_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        set_flavour(dir.path(), "sumo").unwrap();
        set_machines(dir.path(), Some("c64"), None, false).unwrap();
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
        set_machines(dir.path(), None, None, true).unwrap();
        assert_eq!(load(Some(dir.path())).flavour, "sumo");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "full = \"c64\"\nfuture_field = true\n",
        )
        .unwrap();
        assert_eq!(load(Some(dir.path())).full, "c64");
    }

    #[test]
    fn set_machines_creates_the_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        set_machines(&nested, Some("c64"), None, false).unwrap();
        assert_eq!(load(Some(&nested)).full, "c64");
        assert!(nested.join("config.toml").is_file());
    }

    #[test]
    fn set_machines_preserves_animate_and_an_unknown_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "animate = false\nfuture_field = true\n",
        )
        .unwrap();
        set_machines(dir.path(), Some("c64"), None, false).unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(contents.contains("animate = false"));
        assert!(contents.contains("future_field = true"));
        assert_eq!(load(Some(dir.path())).full, "c64");
        assert!(!load(Some(dir.path())).animate);
    }

    #[test]
    fn set_machines_reset_removes_full_and_fast_but_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "full = \"c64\"\nfast = \"c64\"\nanimate = false\n",
        )
        .unwrap();
        set_machines(dir.path(), None, None, true).unwrap();
        let contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(!contents.contains("full"));
        assert!(!contents.contains("fast"));
        assert!(contents.contains("animate = false"));
        assert_eq!(
            load(Some(dir.path())),
            Config {
                full: "pc95".to_string(),
                fast: "pc85".to_string(),
                animate: false,
                flavour: "unicorn".to_string(),
            }
        );
    }

    #[test]
    fn set_machines_replaces_an_invalid_existing_file_without_erroring() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "full = ").unwrap();
        set_machines(dir.path(), Some("c64"), Some("pc95"), false).unwrap();
        assert_eq!(
            load(Some(dir.path())),
            Config {
                full: "c64".to_string(),
                fast: "pc95".to_string(),
                animate: true,
                flavour: "unicorn".to_string(),
            }
        );
    }
}
