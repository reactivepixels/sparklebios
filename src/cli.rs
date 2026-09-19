//! Clap definitions and dispatch.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::shell;

#[derive(Debug, Parser)]
#[command(
    name = "bios",
    version,
    about = "A 1995 POST screen for your terminal that is secretly a health check"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the shell hook for the given shell.
    Init {
        #[command(subcommand)]
        shell: InitShell,
    },
    /// Print the boot screen.
    Boot(BootArgs),
    /// List the available machines.
    Machines,
    /// Theme related commands.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
}

#[derive(Debug, Subcommand)]
enum InitShell {
    /// Print the zsh hook.
    Zsh,
}

#[derive(Debug, Args)]
struct BootArgs {
    /// The machine id to boot.
    #[arg(long)]
    machine: Option<String>,
    /// Force a full boot.
    #[arg(long, conflicts_with = "fast")]
    full: bool,
    /// Force a fast boot.
    #[arg(long)]
    fast: bool,
}

#[derive(Debug, Subcommand)]
enum ThemeCommand {
    /// Install the Ghostty theme files.
    Install {
        /// Directory to install into.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// Parse args and dispatch. Never panics outward.
pub fn run() -> i32 {
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            shell: InitShell::Zsh,
        } => {
            print!("{}", shell::ZSH_HOOK);
            0
        }
        Command::Boot(_args) => 0,
        Command::Machines => 0,
        Command::Theme {
            command: ThemeCommand::Install { dir: _ },
        } => 0,
    }
}
