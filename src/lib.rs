#![warn(missing_docs)]
//! Module declarations only.

pub mod boot;
pub mod cache;
pub mod checks;
pub mod cli;
pub mod clock;
pub mod config;
pub mod facts;
pub mod fetch;
pub mod flavour;
pub mod machine;
pub mod mode;
pub mod paths;
pub mod presence;
pub mod render;
pub mod screensaver;
pub mod setup;
// Owned by another agent during this pass; excluded from the missing-docs
// survey below so this file can enforce doc coverage on the modules we do own.
#[allow(missing_docs)]
pub mod shell;
pub mod show;
pub mod sound;
pub mod sprinkles;
pub mod sprite;
pub mod state;
pub mod template;
pub mod term;
pub mod theme;
pub mod tomledit;
pub mod tty;
