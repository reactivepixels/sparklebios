//! Embedded theme files and install.

use std::path::{Path, PathBuf};

pub const THEMES: [(&str, &str); 4] = [
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
];

/// Writes all four files into `dir`, creating it. Returns the paths written.
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
