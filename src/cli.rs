//! Clap definitions and dispatch.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::shell;

const TOP_LEVEL_HELP_TEMPLATE: &str = "\
SparkleBIOS {version}
A 1995 POST screen for your terminal that is secretly a health check.

Usage: bios <COMMAND>

Everyday:
  boot               Play the boot screen now
  resume             Change to the project you left work in
  flavours           List the personalities you can boot as
  use <FLAVOUR>      Boot as that flavour from now on
  theme list         List the matching Ghostty themes
  theme use <NAME>   Install the themes and switch Ghostty to one

Setup:
  init zsh           Print the hook. Add this to the end of ~/.zshrc:
                     command -v bios >/dev/null 2>&1 && eval \"$(bios init zsh)\"
  theme install      Install the theme files without switching

Try:
  bios boot --flavour sumo     Preview a flavour without changing anything
  bios use sumo                Make it permanent
  bios resume                  Go back to the project you left work in
  bios use                     Show which flavour is set
  bios theme use mane          Switch Ghostty to the Mane theme
  SPARKLEBIOS_BOOT=0           Set this in a shell to stop it booting there

Options:
  -h, --help         Print help
  -V, --version      Print version
";

/// The hand-written top-level help text, with the crate version substituted in. Replaces clap's
/// generated help for `bios --help`, `bios help` and a bare `bios`; every other subcommand's help
/// stays clap generated.
fn top_level_help() -> String {
    TOP_LEVEL_HELP_TEMPLATE.replace("{version}", env!("CARGO_PKG_VERSION"))
}

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
    /// Print the shell hook for a shell, to add to your shell's startup file.
    Init {
        #[command(subcommand)]
        shell: InitShell,
    },
    /// Play the boot screen.
    Boot(BootCliArgs),
    /// Refresh the fact cache used by the boot screen's health checks.
    Refresh(RefreshCliArgs),
    /// Print the path of the project you left work in.
    Resume,
    /// List the flavours you can boot as.
    Flavours,
    /// Show or set which flavour boots.
    Use(UseCliArgs),
    /// Ghostty theme commands.
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
    /// The machine id to boot: for testing your own machine files.
    #[arg(long, hide = true)]
    machine: Option<String>,
    /// Play the show against /dev/tty and print only the bytes typed during it. Used by the
    /// shell hook; not meant to be run by hand.
    #[arg(long)]
    hook: bool,
    /// Skip the animated show, even where a terminal and the config would otherwise play it.
    #[arg(long)]
    no_animate: bool,
    /// Preview with this flavour, without changing your configured one.
    #[arg(long)]
    flavour: Option<String>,
}

impl From<BootCliArgs> for crate::boot::BootArgs {
    fn from(args: BootCliArgs) -> Self {
        crate::boot::BootArgs {
            machine: args.machine,
            hook: args.hook,
            no_animate: args.no_animate,
            flavour: args.flavour,
        }
    }
}

#[derive(Debug, Args)]
struct RefreshCliArgs {
    /// Ignore the lock and the cache age, and refresh anyway.
    #[arg(long)]
    force: bool,
    /// Print the refreshed cache as JSON after refreshing.
    #[arg(long, hide = true)]
    print: bool,
}

#[derive(Debug, Args)]
struct UseCliArgs {
    /// The flavour to use from now on, for example unicorn or sumo. Omit to print the current one.
    id: Option<String>,
}

#[derive(Debug, Subcommand)]
enum ThemeCommand {
    /// Install the Ghostty theme files without switching Ghostty to one.
    Install {
        /// Directory to install the theme files into.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// List the themes and their short names.
    List,
    /// Install the themes and switch Ghostty's config to one.
    Use {
        /// The theme's short name (for example mane) or its full name.
        name: String,
        /// Directory to install the theme files into.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Ghostty config file to edit, overriding the usual search order.
        #[arg(long, hide = true)]
        config: Option<PathBuf>,
    },
}

/// Parse args and dispatch. Never panics outward.
pub fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let is_bare = args.len() == 1;
    let is_top_help = args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h" | "help");
    if is_bare || is_top_help {
        print!("{}", top_level_help());
        return 0;
    }

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
        Command::Refresh(args) => refresh(args.force, args.print),
        Command::Resume => resume(),
        Command::Flavours => {
            for flavour in crate::flavour::list(crate::paths::user_flavours_dir().as_deref()) {
                println!("{:<10}{}", flavour.id, flavour.name);
            }
            0
        }
        Command::Use(args) => use_flavour(args),
        Command::Theme {
            command: ThemeCommand::Install { dir },
        } => install_theme(dir),
        Command::Theme {
            command: ThemeCommand::List,
        } => theme_list(),
        Command::Theme {
            command: ThemeCommand::Use { name, dir, config },
        } => theme_use(&name, dir, config),
    }
}

fn debug(msg: impl FnOnce() -> String) {
    if std::env::var("SPARKLEBIOS_DEBUG").as_deref() == Ok("1") {
        eprintln!("{}", msg());
    }
}

/// Refreshes the fact cache. Always exits 0; errors print only under `SPARKLEBIOS_DEBUG=1`.
/// Takes `<cache_dir>/refresh.lock` first: a lock younger than 120 seconds means another refresh
/// is already running, so this exits at once without doing anything (unless `force`, which
/// ignores the lock's freshness and the cache's age). The lock, once taken, is always removed
/// before returning.
fn refresh(force: bool, print: bool) -> i32 {
    let Some(cache_dir) = crate::paths::cache_dir() else {
        debug(|| "bios: cannot find a cache directory".to_string());
        return 0;
    };
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        debug(|| format!("bios: cannot create cache directory: {e}"));
        return 0;
    }

    let lock_path = cache_dir.join("refresh.lock");
    if !force {
        if let Ok(meta) = std::fs::metadata(&lock_path) {
            let is_fresh = meta
                .modified()
                .ok()
                .and_then(|m| m.elapsed().ok())
                .map(|age| age.as_secs() < 120)
                .unwrap_or(true);
            if is_fresh {
                return 0;
            }
        }
    }
    // The lock is missing, stale, or being ignored via `--force`: take it over.
    let _ = std::fs::remove_file(&lock_path);
    if std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .is_err()
    {
        return 0;
    }

    let now = crate::clock::now_unix();
    do_refresh(&cache_dir, now, force, print);
    let _ = std::fs::remove_file(&lock_path);
    0
}

/// The refresh itself, once the lock is held: without `force`, a cache that is not stale is left
/// untouched (still printed when `print` is set); otherwise every probe runs and the cache is
/// rewritten.
fn do_refresh(cache_dir: &std::path::Path, now: u64, force: bool, print: bool) {
    let cache = crate::cache::load(cache_dir);
    if !force && !cache.is_stale(now) {
        if print {
            print_cache(&cache);
        }
        return;
    }
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let findings = crate::checks::run_all(&config);
    let cache = crate::cache::Cache {
        generated: now,
        findings,
    };
    if let Err(e) = cache.save(cache_dir) {
        debug(|| format!("bios: failed to save the cache: {e}"));
    }
    if print {
        print_cache(&cache);
    }
}

fn print_cache(cache: &crate::cache::Cache) {
    match serde_json::to_string(cache) {
        Ok(json) => println!("{json}"),
        Err(e) => debug(|| format!("bios: failed to serialise the cache: {e}")),
    }
}

/// Prints the absolute path of boot device 1. Not on the boot path, so it runs the project scan
/// inline rather than trusting the cache, and never writes the cache.
fn resume() -> i32 {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let devices = crate::checks::projects::boot_devices(&config);
    let Some(first) = devices.first() else {
        eprintln!("bios: no boot device to resume.");
        return 1;
    };
    println!("{}", first.display());
    0
}

fn use_flavour(args: UseCliArgs) -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };

    if let Some(id) = &args.id {
        if crate::flavour::find(id, crate::paths::user_flavours_dir().as_deref()).is_none() {
            eprintln!("bios: no flavour called {id}. Try: bios flavours");
            return 1;
        }
        if crate::config::set_flavour(&dir, id).is_err() {
            return 1;
        }
    }

    let config = crate::config::load(Some(&dir));
    let flavour_id = crate::flavour::find(
        &config.flavour,
        crate::paths::user_flavours_dir().as_deref(),
    )
    .map(|f| f.id)
    .unwrap_or_else(|| "unicorn".to_string());
    println!("Flavour : {flavour_id}");
    0
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

fn theme_list() -> i32 {
    for (short, full) in crate::theme::SHORT_NAMES {
        println!("{short:<11}{full}");
    }
    0
}

fn theme_use(name: &str, dir: Option<PathBuf>, config: Option<PathBuf>) -> i32 {
    let Some(full_name) = crate::theme::resolve_name(name) else {
        eprintln!("bios: no theme called {name}. Try: bios theme list");
        return 1;
    };
    let Some(themes_dir) = dir.or_else(crate::paths::ghostty_themes_dir) else {
        eprintln!("bios: cannot find a themes directory, pass --dir");
        return 1;
    };
    let config_path = match config {
        Some(path) => path,
        None => {
            let candidates = crate::paths::ghostty_config_candidates();
            let picked =
                crate::theme::pick_config_path(&candidates).or_else(|| candidates.last().cloned());
            let Some(picked) = picked else {
                eprintln!("bios: cannot find a Ghostty config, pass --config");
                return 1;
            };
            picked
        }
    };
    if crate::theme::install_and_use(&themes_dir, &config_path, full_name).is_err() {
        return 1;
    }
    println!("Ghostty theme : {full_name}");
    println!("Reload Ghostty's config or restart the terminal to see it.");
    0
}
