//! Clap definitions and dispatch.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::shell;

const TOP_LEVEL_HELP_TEMPLATE: &str = "\
SparkleBIOS {version}
Boot every terminal tab like a 1995 PC. It counts your RAM, detects a unicorn,
and runs a real health check.

Usage: bios <COMMAND>

Everyday:
  boot               Play the boot screen now
  fetch              Show your machine at a glance
  resume             Change to the project you left work in
  refresh            Run the health checks again now
  flavours           List the personalities you can boot as
  use <FLAVOUR>      Boot as that flavour from now on
  flavour new <ID>   Start your own flavour from a working template
  sprinkles [LEVEL]  Optional effects: off, light or full
  theme list         List the matching Ghostty themes
  theme use <NAME>   Install the themes and switch Ghostty to one

Install:
  init zsh|bash|fish Print the hook. For zsh, add this to the end of ~/.zshrc:
                     command -v bios >/dev/null 2>&1 && eval \"$(bios init zsh)\"
  theme install      Install the theme files without switching
  setup              The CMOS Setup Utility. Blue. Arrow keys. You remember.
  config edit        Open the config file in your editor
  config path        Print where the config file lives
  config reset       Factory defaults. The unicorn will be notified.

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
    /// Show your machine at a glance: the mascot, a column of facts, and a palette swatch.
    Fetch,
    /// Refresh the fact cache used by the boot screen's health checks.
    Refresh(RefreshCliArgs),
    /// Print the path of the project you left work in. The shell hook wraps this in a function
    /// that changes directory for you, so with the hook installed `bios resume` takes you there.
    Resume,
    /// List the flavours you can boot as.
    Flavours,
    /// Show or set which flavour boots.
    Use(UseCliArgs),
    /// Flavour file commands.
    Flavour {
        #[command(subcommand)]
        command: FlavourCommand,
    },
    /// Show or set the sprinkles level.
    Sprinkles(SprinklesCliArgs),
    /// Print the current flavour's line for a moment. For tests and for shells we do not emit.
    #[command(hide = true)]
    Say(SayCliArgs),
    /// Print the mascot for one prompt. For starship, which spawns its custom commands anyway.
    #[command(hide = true)]
    Prompt,
    /// The CMOS Setup Utility.
    Setup(SetupCliArgs),
    /// Ghostty theme commands.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
    /// Config file commands.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
enum InitShell {
    /// Print the zsh hook.
    Zsh,
    /// Print the bash hook.
    Bash,
    /// Print the fish hook.
    Fish,
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
struct SayCliArgs {
    /// The moment: done, failed, goodbye, not_found, refresh, resume or switched.
    event: String,
    /// How long the command took, in seconds, for the lines that mention it.
    #[arg(long)]
    took: Option<u64>,
    /// The project name, for the lines that mention it.
    #[arg(long)]
    device: Option<String>,
    /// The command that was not found, for the line that mentions it.
    #[arg(long)]
    cmd: Option<String>,
}

#[derive(Debug, Args)]
struct UseCliArgs {
    /// The flavour to use from now on, for example unicorn or sumo. Omit to print the current one.
    id: Option<String>,
}

#[derive(Debug, Subcommand)]
enum FlavourCommand {
    /// Start a new flavour file, a complete working template, from your own name.
    New(FlavourNewCliArgs),
}

#[derive(Debug, Args)]
struct FlavourNewCliArgs {
    /// The flavour id: lowercase letters, digits and hyphens, starting with a letter.
    id: String,
}

#[derive(Debug, Args)]
struct SprinklesCliArgs {
    /// The level to use from now on: off, light or full. Omit to print the current one.
    level: Option<String>,
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
        /// Starship config file to edit, overriding the usual search order.
        #[arg(long, hide = true)]
        starship_config: Option<PathBuf>,
        /// Skip pointing the starship prompt at a matching palette.
        #[arg(long)]
        no_prompt: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print the path of the config file.
    Path,
    /// Open the config file in your editor.
    ///
    /// Creates the file from the commented defaults first if it does not exist yet. Uses
    /// $VISUAL, then $EDITOR, then vi.
    Edit,
    /// Restore the config file to its commented defaults.
    ///
    /// If a config file already exists and differs from the defaults, its old contents are
    /// saved to config.toml.bak first, replacing any previous backup.
    Reset,
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
        } => init(shell::Shell::Zsh),
        Command::Init {
            shell: InitShell::Bash,
        } => init(shell::Shell::Bash),
        Command::Init {
            shell: InitShell::Fish,
        } => init(shell::Shell::Fish),
        Command::Boot(args) => {
            crate::boot::run(&args.into());
            0
        }
        Command::Fetch => crate::fetch::run(),
        Command::Refresh(args) => refresh(args.force, args.print),
        Command::Resume => resume(),
        Command::Flavours => {
            for flavour in crate::flavour::list(crate::paths::user_flavours_dir().as_deref()) {
                println!("{:<10}{}", flavour.id, flavour.name);
            }
            0
        }
        Command::Use(args) => use_flavour(args),
        Command::Flavour {
            command: FlavourCommand::New(args),
        } => flavour_new(&args.id),
        Command::Sprinkles(args) => sprinkles(args),
        Command::Theme {
            command: ThemeCommand::Install { dir },
        } => install_theme(dir),
        Command::Say(args) => say(args),
        Command::Prompt => prompt(),
        Command::Setup(args) => crate::setup::run(args.memory_test),
        Command::Theme {
            command: ThemeCommand::List,
        } => theme_list(),
        Command::Theme {
            command:
                ThemeCommand::Use {
                    name,
                    dir,
                    config,
                    starship_config,
                    no_prompt,
                },
        } => theme_use(&name, dir, config, starship_config, no_prompt),
        Command::Config {
            command: ConfigCommand::Path,
        } => config_path(),
        Command::Config {
            command: ConfigCommand::Edit,
        } => config_edit(),
        Command::Config {
            command: ConfigCommand::Reset,
        } => config_reset(),
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
    // A boot spawns this with its output thrown away, so only a person who typed it sees this.
    if !print {
        if let Some(line) = presence_line("refresh", &[]) {
            println!("{line}");
        }
    }
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
    let carried = crate::checks::Carried {
        disk_history: cache.disk_history,
        battery_step: cache.battery_step,
    };
    let (findings, carried) = crate::checks::run_all(&config, carried, now);
    let cache = crate::cache::Cache {
        generated: now,
        findings,
        disk_history: carried.disk_history,
        battery_step: carried.battery_step,
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
/// The configured flavour's line for `event`, or `None` when presence is switched off, the
/// flavour has nothing to say, or a slot it needs was not supplied.
/// Prints a shell's hook with the current flavour's lines written into it. Everything the shell
/// will say is decided here, once, so the shell itself never spawns `bios` to speak.
fn init(shell: shell::Shell) -> i32 {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let flavour = crate::flavour::find(
        &config.flavour,
        crate::paths::user_flavours_dir().as_deref(),
    );
    let mascot = mascot_for(flavour.as_ref(), shell.wrap());
    print!(
        "{}",
        shell::render(shell, &config, flavour.as_ref(), mascot)
    );
    0
}

/// What goes in front of the prompt: the mascot where the terminal can draw one, and the boot
/// streak where there is one worth mentioning. Empty when there is neither, so an untouched
/// prompt stays untouched.
///
/// The two image protocols are not the same deal. Kitty stores an image and places it, so the
/// picture goes once and each prompt costs a few dozen bytes. iTerm2 has no stored image, so the
/// picture itself is what the shell reprints every time. Both are worth having; only one is cheap.
fn mascot_for(flavour: Option<&crate::flavour::Flavour>, wrap: shell::Wrap) -> shell::Mascot {
    image_for(flavour, wrap).with_streak(streak_days())
}

fn image_for(flavour: Option<&crate::flavour::Flavour>, wrap: shell::Wrap) -> shell::Mascot {
    let protocol = crate::sprite::detect_image_protocol(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("LC_TERMINAL").ok().as_deref(),
    );
    let Some(flavour) = flavour else {
        return shell::Mascot::none();
    };
    let Some(png) = crate::sprite::builtin(&flavour.sprite) else {
        return shell::Mascot::none();
    };
    match protocol {
        Some(crate::sprite::ImageProtocol::Kitty) => {
            // An id of our own, steady for a flavour, so a second shell reuses the same image.
            let id = 9000 + (flavour.id.bytes().map(u32::from).sum::<u32>() % 900);
            shell::Mascot::kitty(png, id, 1, wrap)
        }
        Some(crate::sprite::ImageProtocol::Iterm) => shell::Mascot::iterm(png, 1, wrap),
        None => shell::Mascot::none(),
    }
}

/// The boot streak, read once while the hook is being generated. Read only: generating a hook
/// must never change what the next boot shows.
fn streak_days() -> u32 {
    crate::paths::state_dir()
        .map(|dir| crate::state::State::load(&dir))
        .map_or(0, |state| state.streak_days)
}

#[derive(Args, Debug)]
pub struct SetupCliArgs {
    /// Go straight to the memory test.
    #[arg(long, hide = true)]
    pub memory_test: bool,
}

fn say(args: SayCliArgs) -> i32 {
    let took = args.took.map(crate::presence::duration);
    let mut slots: Vec<(&str, &str)> = Vec::new();
    if let Some(took) = &took {
        slots.push(("took", took));
    }
    if let Some(device) = &args.device {
        slots.push(("device", device));
    }
    if let Some(cmd) = &args.cmd {
        slots.push(("cmd", cmd));
    }
    if let Some(line) = presence_line(&args.event, &slots) {
        println!("{line}");
    }
    // Saying nothing is a normal outcome: presence off, an unknown moment, a flavour with no
    // words for it. None of those is an error worth an exit code.
    0
}

/// Prints the mascot for one prompt: the same placement `bios init` bakes into
/// `SPARKLEBIOS_MASCOT`, unwrapped (`shell::Wrap::None`), since starship does its own width
/// accounting. Prints the image transmission first unless `SPARKLEBIOS_PROMPT_IMG` is set, so a
/// starship user who wants to pay for it only once can set that after the first prompt. Exits 0
/// always, and prints nothing at all on any error or wherever the terminal cannot draw an image:
/// a prompt command must never be able to break a prompt.
fn prompt() -> i32 {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let flavour = crate::flavour::find(
        &config.flavour,
        crate::paths::user_flavours_dir().as_deref(),
    );
    let mascot = mascot_for(flavour.as_ref(), shell::Wrap::None);
    if mascot.placement.is_empty() {
        return 0;
    }
    if std::env::var("SPARKLEBIOS_PROMPT_IMG").is_err() {
        print!("{}", mascot.transmit);
    }
    print!("{}", mascot.placement);
    0
}

fn presence_line(event: &str, slots: &[(&str, &str)]) -> Option<String> {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    if !config.presence {
        return None;
    }
    let flavour = crate::flavour::find(
        &config.flavour,
        crate::paths::user_flavours_dir().as_deref(),
    )?;
    crate::presence::line(&flavour, event, slots)
}

fn resume() -> i32 {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let devices = crate::checks::projects::boot_devices(&config);
    let Some(first) = devices.first() else {
        eprintln!("bios: no boot device to resume.");
        return 1;
    };
    // The line goes to stderr on purpose. Stdout is the path, and the shell function does
    // `cd "$(bios resume)"` with it, so a second line there would be read as part of the path.
    if let Some(line) = presence_line("resume", &[("device", &dir_name(first))]) {
        eprintln!("{line}");
    }
    println!("{}", first.display());
    0
}

fn dir_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
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
    if args.id.is_some() {
        if let Some(line) = presence_line("switched", &[]) {
            println!("{line}");
        }
    }
    0
}

/// Writes `<user flavours dir>/<id>.toml`, a complete working flavour built from `id` alone, so
/// `bios boot --flavour <id>` works before a single character of it is edited. Writes nothing on
/// any of the four error cases: a malformed id, an id that names a built-in, a file that already
/// exists, or a flavours directory that cannot be worked out.
fn flavour_new(id: &str) -> i32 {
    if !crate::flavour::well_formed_new_id(id) {
        eprintln!(
            "bios: a flavour id is lowercase letters, digits and hyphens, starting with a letter."
        );
        return 1;
    }
    if crate::flavour::is_builtin_id(id) {
        eprintln!("bios: {id} is a built-in flavour. Pick another name.");
        return 1;
    }
    let Some(dir) = crate::paths::user_flavours_dir() else {
        eprintln!("bios: could not work out where your flavours live.");
        return 1;
    };
    let path = dir.join(format!("{id}.toml"));
    if path.exists() {
        eprintln!(
            "bios: {} already exists. Delete it or pick another name.",
            path.display()
        );
        return 1;
    }
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("bios: could not work out where your flavours live.");
        return 1;
    }
    let name = crate::flavour::display_name(id);
    let template = crate::flavour::starter_template(id, &name);
    if std::fs::write(&path, template).is_err() {
        eprintln!("bios: could not work out where your flavours live.");
        return 1;
    }
    println!("Created {}", path.display());
    println!();
    println!("Open it and make it yours, then:");
    println!("  bios boot --flavour {id}    Preview it");
    println!("  bios use {id}               Boot as it from now on");
    0
}

/// `level`, strictly: unlike `sprinkles::Level::parse`, which reads anything unknown as `Off` for
/// the config file and the environment override, a level typed on the command line either is one
/// of the three or is rejected outright.
fn parse_sprinkle_level(level: &str) -> Option<crate::sprinkles::Level> {
    match level {
        "off" => Some(crate::sprinkles::Level::Off),
        "light" => Some(crate::sprinkles::Level::Light),
        "full" => Some(crate::sprinkles::Level::Full),
        _ => None,
    }
}

/// The line `bios sprinkles <level>` prints once it has set `level`.
fn sprinkles_set_message(level: crate::sprinkles::Level) -> &'static str {
    match level {
        crate::sprinkles::Level::Off => {
            "Sprinkles : off. The BIOS respects your decision and is not hurt."
        }
        crate::sprinkles::Level::Light => "Sprinkles : light. Tasteful.",
        crate::sprinkles::Level::Full => "Sprinkles : full. You asked for this.",
    }
}

/// Shows or sets the sprinkles level, the same shape `use_flavour` follows for the flavour: no
/// argument prints the effective level, an argument sets it (preserving every other config key)
/// and prints the level-specific line, and an unknown level is rejected, exit 1, with nothing
/// written.
fn sprinkles(args: SprinklesCliArgs) -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };

    if let Some(level) = &args.level {
        let Some(parsed) = parse_sprinkle_level(level) else {
            eprintln!("bios: no sprinkle level called {level}. Try: off, light, full");
            return 1;
        };
        if crate::config::set_sprinkles(&dir, parsed).is_err() {
            return 1;
        }
        println!("{}", sprinkles_set_message(parsed));
        if matches!(
            parsed,
            crate::sprinkles::Level::Light | crate::sprinkles::Level::Full
        ) {
            println!("Preview it now: bios boot");
        }
        return 0;
    }

    let config = crate::config::load(Some(&dir));
    println!("Sprinkles : {}", config.sprinkles.as_str());
    0
}

/// Prints the absolute path of the config file, whether or not it exists.
fn config_path() -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };
    println!("{}", crate::config::path(&dir).display());
    0
}

/// Opens the config file in `$VISUAL`, then `$EDITOR`, then `vi`, creating it from the template
/// first if it does not exist. Runs the editor in the foreground with stdio inherited.
fn config_edit() -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };
    if crate::config::ensure_exists(&dir).is_err() {
        return 1;
    }
    let path = crate::config::path(&dir);
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("EDITOR").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "vi".to_string());
    match std::process::Command::new(&editor)
        .arg(&path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
    {
        Ok(status) if status.success() => 0,
        _ => 1,
    }
}

/// Overwrites the config file with the commented defaults, backing up a differing existing file
/// to config.toml.bak first.
fn config_reset() -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };
    if crate::config::reset(&dir).is_err() {
        return 1;
    }
    println!("Factory defaults restored. The unicorn has been notified.");
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

/// The setup screen's way in to the same theme switch `bios theme use` performs, with the
/// defaults it would use from the command line.
pub fn theme_use_for_setup(short: &str) -> i32 {
    theme_use(short, None, None, None, false)
}

fn theme_use(
    name: &str,
    dir: Option<PathBuf>,
    config: Option<PathBuf>,
    starship_config: Option<PathBuf>,
    no_prompt: bool,
) -> i32 {
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
    println!("Ghostty theme  : {full_name}");
    if !no_prompt {
        if let Some(prompt_path) = resolve_starship_config_path(starship_config) {
            let palette = crate::theme::starship_palette_name(full_name);
            if crate::theme::set_starship_palette(&prompt_path, palette).unwrap_or(false) {
                println!("Prompt palette : {palette}");
            }
        }
    }
    println!("Reload Ghostty's config or restart the terminal to see it.");
    0
}

/// The starship config file to consider pointing at a matching palette: `explicit` when given
/// (from `--starship-config`); otherwise `$STARSHIP_CONFIG` when set and non-empty; otherwise
/// `~/.config/starship.toml` (see `paths::starship_config_path`). `None` when none of those
/// resolve to a path.
fn resolve_starship_config_path(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path);
    }
    if let Ok(env_path) = std::env::var("STARSHIP_CONFIG") {
        if !env_path.is_empty() {
            return Some(PathBuf::from(env_path));
        }
    }
    crate::paths::starship_config_path()
}
