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

/// `$XDG_CONFIG_HOME/sparklebios`, falling back to `~/.config/sparklebios`.
pub fn config_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
        ".config",
        "sparklebios",
    )
}

/// `$XDG_STATE_HOME/sparklebios`, falling back to `~/.local/state/sparklebios`.
pub fn state_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_STATE_HOME").as_deref(),
        env("HOME").as_deref(),
        ".local/state",
        "sparklebios",
    )
}

/// `$XDG_CACHE_HOME/sparklebios`, falling back to `~/.cache/sparklebios`.
pub fn cache_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_CACHE_HOME").as_deref(),
        env("HOME").as_deref(),
        ".cache",
        "sparklebios",
    )
}

/// Where the synthesised ULTRA sounds are cached, under `cache_dir`.
pub fn sounds_dir() -> Option<PathBuf> {
    cache_dir().map(|dir| dir.join("sounds"))
}

/// Where a user's own `machines/*.toml` files live, under `config_dir`.
pub fn user_machines_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("machines"))
}

/// Where a user's own `flavours/*.toml` files live, under `config_dir`.
pub fn user_flavours_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("flavours"))
}

/// Where the two `ultra` shader files are installed, under `config_dir`. Not a Ghostty directory:
/// unlike a Ghostty theme, a `custom-shader` line takes an absolute path to any file, so this only
/// needs to be somewhere of our own that does not move.
pub fn shaders_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("shaders"))
}

/// `$XDG_CONFIG_HOME/ghostty/themes`, falling back to `~/.config/ghostty/themes`.
pub fn ghostty_themes_dir() -> Option<PathBuf> {
    resolve(
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
        ".config",
        "ghostty/themes",
    )
}

/// `~/.config/starship.toml`, resolved the same way as the paths above, honouring
/// `XDG_CONFIG_HOME`.
pub fn starship_config_path() -> Option<PathBuf> {
    resolve(
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
        ".config",
        "starship.toml",
    )
}

/// The Ghostty config file candidates, in search order: on macOS,
/// `~/Library/Application Support/com.mitchellh.ghostty/config.ghostty` and `.../config`
/// (skipped when `home` is unset, and never included at all off macOS); then
/// `$XDG_CONFIG_HOME/ghostty/config.ghostty` and `.../config`, `XDG_CONFIG_HOME` defaulting to
/// `home/.config` (skipped when neither `xdg` nor `home` is set).
fn ghostty_config_candidate_list(
    is_macos: bool,
    xdg: Option<&str>,
    home: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if is_macos {
        if let Some(home) = home {
            let support =
                PathBuf::from(home).join("Library/Application Support/com.mitchellh.ghostty");
            candidates.push(support.join("config.ghostty"));
            candidates.push(support.join("config"));
        }
    }
    if let Some(dir) = resolve(xdg, home, ".config", "ghostty") {
        candidates.push(dir.join("config.ghostty"));
        candidates.push(dir.join("config"));
    }
    candidates
}

/// The Ghostty config file candidates for this machine, in search order. See
/// `ghostty_config_candidate_list` for the exact order.
pub fn ghostty_config_candidates() -> Vec<PathBuf> {
    ghostty_config_candidate_list(
        cfg!(target_os = "macos"),
        env("XDG_CONFIG_HOME").as_deref(),
        env("HOME").as_deref(),
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

    #[test]
    fn ghostty_candidates_lead_with_macos_app_support_then_xdg() {
        let candidates = ghostty_config_candidate_list(true, Some("/xdg"), Some("/home"));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from(
                    "/home/Library/Application Support/com.mitchellh.ghostty/config.ghostty"
                ),
                PathBuf::from("/home/Library/Application Support/com.mitchellh.ghostty/config"),
                PathBuf::from("/xdg/ghostty/config.ghostty"),
                PathBuf::from("/xdg/ghostty/config"),
            ]
        );
    }

    #[test]
    fn ghostty_candidates_off_macos_are_xdg_only() {
        let candidates = ghostty_config_candidate_list(false, Some("/xdg"), Some("/home"));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/xdg/ghostty/config.ghostty"),
                PathBuf::from("/xdg/ghostty/config"),
            ]
        );
    }

    #[test]
    fn ghostty_candidates_fall_back_to_home_dot_config() {
        let candidates = ghostty_config_candidate_list(false, None, Some("/home"));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/home/.config/ghostty/config.ghostty"),
                PathBuf::from("/home/.config/ghostty/config"),
            ]
        );
    }

    #[test]
    fn starship_config_path_honours_xdg_config_home() {
        assert_eq!(
            resolve(Some("/xdg"), Some("/home"), ".config", "starship.toml"),
            Some(PathBuf::from("/xdg/starship.toml"))
        );
        assert_eq!(
            resolve(None, Some("/home"), ".config", "starship.toml"),
            Some(PathBuf::from("/home/.config/starship.toml"))
        );
    }

    #[test]
    fn ghostty_candidates_are_empty_with_nothing_set() {
        assert_eq!(
            ghostty_config_candidate_list(true, None, None),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            ghostty_config_candidate_list(false, None, None),
            Vec::<PathBuf>::new()
        );
    }
}
