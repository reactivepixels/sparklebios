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
    Boot(BootCliArgs),
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
struct BootCliArgs {
    /// The machine id to boot.
    #[arg(long)]
    machine: Option<String>,
    /// Force a full boot.
    #[arg(long, conflicts_with = "fast")]
    full: bool,
    /// Force a fast boot.
    #[arg(long)]
    fast: bool,
    /// Play the show against /dev/tty and print only the bytes typed during it. Used by the
    /// shell hook; not meant to be run by hand.
    #[arg(long)]
    hook: bool,
    /// Skip the animated show, even where a terminal and the config would otherwise play it.
    #[arg(long)]
    no_animate: bool,
}

impl From<BootCliArgs> for crate::boot::BootArgs {
    fn from(args: BootCliArgs) -> Self {
        crate::boot::BootArgs {
            machine: args.machine,
            full: args.full,
            fast: args.fast,
            hook: args.hook,
            no_animate: args.no_animate,
        }
    }
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
        Command::Boot(args) => {
            crate::boot::run(&args.into());
            0
        }
        Command::Machines => {
            for machine in crate::machine::list(crate::paths::user_machines_dir().as_deref()) {
                println!("{:<8}{}", machine.id, machine.name);
            }
            0
        }
        Command::Theme {
            command: ThemeCommand::Install { dir },
        } => install_theme(dir),
    }
}

fn install_theme(dir: Option<PathBuf>) -> i32 {
    let Some(dir) = dir.or_else(crate::paths::ghostty_themes_dir) else {
        eprintln!("bios: cannot find a themes directory, pass --dir");
        return 1;
    };
    let Ok(paths) = crate::theme::install(&dir) else {
        return 0;
    };
    for path in paths {
        println!("{}", path.display());
    }
    println!("Add to your Ghostty config:");
    println!("  theme = dark:rainbows-and-unicorns,light:rainbows-and-unicorns-paper");
    println!("  cursor-style = block");
    println!("  cursor-style-blink = true");
    println!("  bold-is-bright = false");
    0
}
