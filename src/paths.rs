//! Directory resolution.

use std::path::PathBuf;

/// Resolves a directory from an XDG base directory variable, falling back to
/// `HOME/default_rel`, then appending `leaf`. An empty XDG value counts as
/// unset. `None` when neither the XDG variable nor `HOME` is set.
fn resolve(
    xdg: Option<&str>,
    home: Option<&str>,
    default_rel: &str,
    leaf: &str,
) -> Option<PathBuf> {
    if let Some(xdg) = xdg {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join(leaf));
        }
    }
    home.map(|home| PathBuf::from(home).join(default_rel).join(leaf))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

pub fn config_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
        ".config",
        "sparklebios",
    )
}

pub fn state_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_STATE_HOME").as_deref(),
        env("HOME").as_deref(),
        ".local/state",
        "sparklebios",
    )
}

pub fn user_machines_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("machines"))
}

pub fn ghostty_themes_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
        ".config",
        "ghostty/themes",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_value_wins() {
        assert_eq!(
            resolve(Some("/xdg"), Some("/home"), ".config", "sparklebios"),
            Some(PathBuf::from("/xdg/sparklebios"))
        );
    }

    #[test]
    fn home_plus_default_relative_path_is_the_fallback() {
        assert_eq!(
            resolve(None, Some("/home"), ".config", "sparklebios"),
            Some(PathBuf::from("/home/.config/sparklebios"))
        );
    }

    #[test]
    fn an_empty_xdg_value_is_treated_as_unset() {
        assert_eq!(
            resolve(Some(""), Some("/home"), ".config", "sparklebios"),
            Some(PathBuf::from("/home/.config/sparklebios"))
        );
    }

    #[test]
    fn both_missing_yields_none() {
        assert_eq!(resolve(None, None, ".config", "sparklebios"), None);
    }
}
