//! The boot flow: decide, gather, render or animate, print, save.

use std::io::{IsTerminal, Write};
use std::os::unix::io::AsRawFd;

use crate::mode::BootMode;

pub struct BootArgs {
    pub machine: Option<String>,
    pub full: bool,
    pub fast: bool,
    pub hook: bool,
    pub no_animate: bool,
    pub flavour: Option<String>,
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn debug(msg: impl FnOnce() -> String) {
    if env_var("SPARKLEBIOS_DEBUG").as_deref() == Some("1") {
        eprintln!("{}", msg());
    }
}

fn seed_from_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn write_stdout_bytes(bytes: &[u8]) {
    let mut stdout = std::io::stdout();
    if stdout.write_all(bytes).is_err() {
        debug(|| "bios: failed to write to stdout".to_string());
    }
}

fn color_mode() -> crate::render::ColorMode {
    crate::render::color_mode_from_env(
        env_var("NO_COLOR").as_deref(),
        env_var("COLORTERM").as_deref(),
    )
}

/// Kitty on a terminal that supports it, half-blocks otherwise. `render_static` only draws a
/// logo or badge when its painted path is actually in use, so this is safe to pass unconditionally.
fn graphics() -> crate::render::Graphics {
    if crate::sprite::supports_kitty(
        env_var("TERM").as_deref(),
        env_var("TERM_PROGRAM").as_deref(),
    ) {
        crate::render::Graphics::Kitty
    } else {
        crate::render::Graphics::HalfBlocks
    }
}

/// Animation is off when the config or `SPARKLEBIOS_ANIMATE=0` disables it, `--no-animate` was
/// passed, the mode is not TrueColor, or the output is not a terminal.
fn animate_enabled(
    config: &crate::config::Config,
    no_animate_flag: bool,
    mode: crate::render::ColorMode,
    is_tty: bool,
) -> bool {
    config.animate
        && env_var("SPARKLEBIOS_ANIMATE").as_deref() != Some("0")
        && !no_animate_flag
        && mode == crate::render::ColorMode::TrueColor
        && is_tty
}

/// Draws `machine` into `out`: animated (honouring `key_fd` as the skip key source) when
/// `animate` is true and, if a key is to be polled, raw mode can actually be entered; the final
/// static screen otherwise. `flavour`, for a flavoured machine, supplies its quips and its logo
/// sprite. Returns the raw bytes read from `key_fd` while the show played, in order, empty when
/// it did not animate or nothing was typed.
#[allow(clippy::too_many_arguments)]
fn play_or_render(
    machine: &crate::machine::Machine,
    facts: &crate::facts::Facts,
    seed: u64,
    mode: crate::render::ColorMode,
    term_cols: Option<u16>,
    graphics: crate::render::Graphics,
    flavour: Option<&crate::flavour::Flavour>,
    animate: bool,
    out: &mut dyn Write,
    key_fd: Option<i32>,
) -> Vec<u8> {
    if animate {
        let guard = key_fd.and_then(crate::tty::RawGuard::new);
        let raw_mode_ok = key_fd.is_none() || guard.is_some();
        if raw_mode_ok {
            let geometry = crate::show::Geometry {
                mode,
                term_cols,
                graphics,
            };
            return crate::show::play(machine, facts, seed, geometry, flavour, out, key_fd, 1.0)
                .typed;
        }
    }
    let output =
        crate::render::render_static(machine, facts, mode, seed, term_cols, graphics, flavour);
    let _ = out.write_all(output.as_bytes());
    let _ = out.flush();
    Vec::new()
}

/// Resolves the flavour to use: the explicit id when given and known, else the configured one,
/// else the built-in `unicorn`. `None` only if even `unicorn` cannot be found, which never
/// happens for the shipped built-ins.
fn resolve_flavour(
    explicit: Option<&str>,
    config_flavour: &str,
) -> Option<crate::flavour::Flavour> {
    let dir = crate::paths::user_flavours_dir();
    let id = explicit.unwrap_or(config_flavour);
    crate::flavour::find(id, dir.as_deref())
        .or_else(|| crate::flavour::find("unicorn", dir.as_deref()))
}

/// Never panics outward and never returns an error: a boot that cannot happen prints nothing.
pub fn run(args: &BootArgs) {
    if args.hook {
        run_shell_boot(args, true);
    } else if args.machine.is_some() || args.full || args.fast || args.flavour.is_some() {
        run_preview(args);
    } else {
        run_shell_boot(args, false);
    }
}

fn run_preview(args: &BootArgs) {
    let user_dir = crate::paths::user_machines_dir();
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let machine = if let Some(id) = &args.machine {
        crate::machine::find(id, user_dir.as_deref())
    } else {
        let id = if args.full {
            &config.full
        } else {
            &config.fast
        };
        crate::machine::find(id, user_dir.as_deref()).or_else(|| {
            crate::machine::find(if args.full { "pc95" } else { "pc85" }, user_dir.as_deref())
        })
    };
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };
    let mut facts = crate::facts::gather();
    // A preview is not a real shell startup, so a stale or meaningless boot time never appears.
    facts.remove("shell.boot_ms");
    let flavour = resolve_flavour(args.flavour.as_deref(), &config.flavour);
    if let Some(f) = &flavour {
        crate::flavour::apply(f, &mut facts);
    }
    let mode = color_mode();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let term_cols = crate::term::cols(1);
    let animate = animate_enabled(&config, args.no_animate, mode, stdout_is_tty);
    let key_fd = std::io::stdin().is_terminal().then_some(0);
    let mut stdout = std::io::stdout();
    play_or_render(
        &machine,
        &facts,
        seed_from_time(),
        mode,
        term_cols,
        graphics(),
        flavour.as_ref(),
        animate,
        &mut stdout,
        key_fd,
    );
}

/// The real boot: `bios boot` (writes to stdout, keys discarded) or `bios boot --hook` (writes
/// to `/dev/tty`, and prints only the filtered typed bytes to stdout). Both share the same
/// decision, machine selection, streak and state handling; only the target and what happens to
/// the typed bytes differ.
fn run_shell_boot(args: &BootArgs, hook: bool) {
    let tty_file = if hook {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
        {
            Ok(file) => Some(file),
            Err(_) => return,
        }
    } else {
        None
    };

    let is_tty = match &tty_file {
        // SAFETY: `f` is a valid, open file; `isatty` only reads its fd.
        Some(f) => unsafe { libc::isatty(f.as_raw_fd()) != 0 },
        None => std::io::stdout().is_terminal(),
    };

    let now = crate::clock::now_unix();
    let today = crate::clock::day_string(now as i64);
    let state_dir = crate::paths::state_dir();
    let mut state = match &state_dir {
        Some(dir) => crate::state::State::load(dir),
        None => crate::state::State::default(),
    };

    let inputs = crate::mode::BootInputs {
        stdout_is_tty: is_tty,
        term: env_var("TERM"),
        kill_switch: env_var("SPARKLEBIOS_BOOT"),
        already_booted: env_var("SPARKLEBIOS_BOOTED").is_some(),
        now,
        today: today.clone(),
        last_boot: state.last_boot,
        last_full_day: state.last_full_day.clone(),
        burst_window_secs: 10,
    };
    let decision = crate::mode::decide(&inputs);
    if matches!(decision, BootMode::Off | BootMode::Quiet) {
        return;
    }

    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let id = if decision == BootMode::Full {
        &config.full
    } else {
        &config.fast
    };
    let user_dir = crate::paths::user_machines_dir();
    let machine = crate::machine::find(id, user_dir.as_deref()).or_else(|| {
        crate::machine::find(
            if decision == BootMode::Full {
                "pc95"
            } else {
                "pc85"
            },
            user_dir.as_deref(),
        )
    });
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };

    let yesterday = crate::clock::day_string(now as i64 - 86_400);
    state.advance_streak(&today, &yesterday);
    let mut facts = crate::facts::gather();
    facts.insert("streak.days", state.streak_days.to_string());
    facts.insert("streak.label", state.streak_label());
    let flavour = resolve_flavour(None, &config.flavour);
    if let Some(f) = &flavour {
        crate::flavour::apply(f, &mut facts);
    }

    let mode = color_mode();
    let cols_fd = tty_file.as_ref().map_or(1, |f| f.as_raw_fd());
    let term_cols = crate::term::cols(cols_fd);
    let animate = animate_enabled(&config, args.no_animate, mode, is_tty);
    let key_fd = tty_file
        .as_ref()
        .map(|f| f.as_raw_fd())
        .or_else(|| std::io::stdin().is_terminal().then_some(0));

    let mut stdout_handle = std::io::stdout();
    let mut tty_write = tty_file;
    let out: &mut dyn Write = match &mut tty_write {
        Some(f) => f,
        None => &mut stdout_handle,
    };
    let typed = play_or_render(
        &machine,
        &facts,
        seed_from_time(),
        mode,
        term_cols,
        graphics(),
        flavour.as_ref(),
        animate,
        out,
        key_fd,
    );
    if hook {
        write_stdout_bytes(&crate::show::filter_typed(&typed));
    }

    state.last_boot = Some(now);
    if decision == BootMode::Full {
        state.last_full_day = Some(today);
    }
    if let Some(dir) = &state_dir {
        if let Err(e) = state.save(dir) {
            debug(|| format!("bios: failed to save state: {e}"));
        }
    }
}
