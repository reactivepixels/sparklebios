//! User config: `~/.config/sparklebios/config.toml`.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub full: String,
    pub fast: String,
    pub animate: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            full: "pc95".to_string(),
            fast: "pc85".to_string(),
            animate: true,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    full: Option<String>,
    fast: Option<String>,
    animate: Option<bool>,
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
    }
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
            }
        );
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
}
