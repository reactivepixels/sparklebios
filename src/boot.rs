//! The boot flow: decide, gather, render or animate, print, save.

use std::io::{IsTerminal, Write};
use std::os::unix::io::AsRawFd;
use std::process::Stdio;

use crate::mode::BootMode;

pub struct BootArgs {
    pub machine: Option<String>,
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

/// Spawns a detached `bios refresh`, never waited on: its own process, its own stdio, its own
/// failures. A failure to find the current executable or to spawn the child is silent.
fn spawn_refresh() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            debug(|| format!("bios: cannot find own executable to refresh: {e}"));
            return;
        }
    };
    if let Err(e) = std::process::Command::new(exe)
        .arg("refresh")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        debug(|| format!("bios: failed to spawn a detached refresh: {e}"));
    }
}

/// Loads the fact cache when `config.checks` is true, `None` otherwise (and when there is no
/// cache directory to read). Kept as the `Cache` itself, not just its fresh findings, so a caller
/// can check `is_stale` after the screen is drawn without loading it twice.
fn load_cache(config: &crate::config::Config) -> Option<crate::cache::Cache> {
    if !config.checks {
        return None;
    }
    crate::paths::cache_dir().map(|dir| crate::cache::load(&dir))
}

/// Spawns a detached refresh when `cache` is stale. A missing cache (checks off, or no cache
/// directory) never spawns anything.
fn refresh_if_stale(cache: Option<&crate::cache::Cache>, now: u64) {
    if cache.is_some_and(|c| c.is_stale(now)) {
        spawn_refresh();
    }
}

fn color_mode() -> crate::render::ColorMode {
    crate::render::color_mode_from_env(
        env_var("NO_COLOR").as_deref(),
        env_var("COLORTERM").as_deref(),
    )
}

/// The effective graphics preference: `graphics_env`, when it is set, wins over `config_graphics`
/// (an unknown value in it reads as `Auto`, same as the config key does) the same way
/// `SPARKLEBIOS_GRAPHICS` overrides the config file's `graphics` key.
fn resolve_graphics_pref(
    config_graphics: crate::config::GraphicsPref,
    graphics_env: Option<&str>,
) -> crate::config::GraphicsPref {
    match graphics_env {
        Some(value) => crate::config::GraphicsPref::parse(value),
        None => config_graphics,
    }
}

/// Kitty when `supports_kitty` is true, half-blocks otherwise, unless `pref` is `Blocks`, which
/// always draws the half-block mascot even in a Kitty-capable terminal (some terminals drop a
/// Kitty image when their tab goes to sleep, and `Blocks` is how a user opts out of that).
fn graphics_for(
    pref: crate::config::GraphicsPref,
    supports_kitty: bool,
) -> crate::render::Graphics {
    if pref != crate::config::GraphicsPref::Blocks && supports_kitty {
        crate::render::Graphics::Kitty
    } else {
        crate::render::Graphics::HalfBlocks
    }
}

/// Kitty on a terminal that supports it, half-blocks otherwise, unless `config.graphics` (or the
/// `SPARKLEBIOS_GRAPHICS` override) is `Blocks`. `render_static` only draws a logo or badge when
/// its painted path is actually in use, so this is safe to pass unconditionally.
fn graphics(config: &crate::config::Config) -> crate::render::Graphics {
    let pref = resolve_graphics_pref(config.graphics, env_var("SPARKLEBIOS_GRAPHICS").as_deref());
    let supports_kitty = crate::sprite::supports_kitty(
        env_var("TERM").as_deref(),
        env_var("TERM_PROGRAM").as_deref(),
    );
    graphics_for(pref, supports_kitty)
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
/// sprite. `findings` supplies the `Findings` and `F1` lines, empty when checks are off or there
/// is nothing fresh in the cache. `sprinkles` is only ever read once the show actually animates:
/// the static path never spends a cycle on it. Returns the raw bytes read from `key_fd` while the
/// show played, in order, empty when it did not animate or nothing was typed.
#[allow(clippy::too_many_arguments)]
fn play_or_render(
    machine: &crate::machine::Machine,
    facts: &crate::facts::Facts,
    seed: u64,
    mode: crate::render::ColorMode,
    term_cols: Option<u16>,
    graphics: crate::render::Graphics,
    flavour: Option<&crate::flavour::Flavour>,
    findings: &[crate::checks::Finding],
    animate: bool,
    out: &mut dyn Write,
    key_fd: Option<i32>,
    sprinkles: crate::sprinkles::Level,
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
            return crate::show::play(
                machine, facts, seed, geometry, flavour, findings, out, key_fd, 1.0, sprinkles,
            )
            .typed;
        }
    }
    let output = crate::render::render_static(
        machine, facts, mode, seed, term_cols, graphics, flavour, findings,
    );
    let _ = out.write_all(output.as_bytes());
    let _ = out.flush();
    Vec::new()
}

/// The effective sprinkles level: `SPARKLEBIOS_SPRINKLES`, when set, wins over
/// `config_sprinkles`, even when it does not parse to a known level (it then reads as `Off`), the
/// same rule `resolve_graphics_pref` follows for `graphics`.
fn resolve_sprinkles(
    config_sprinkles: crate::sprinkles::Level,
    sprinkles_env: Option<&str>,
) -> crate::sprinkles::Level {
    crate::sprinkles::resolve(config_sprinkles, sprinkles_env)
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
    } else if args.machine.is_some() || args.flavour.is_some() {
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
        crate::machine::find("pc95", user_dir.as_deref())
    };
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };
    let mut facts = crate::facts::gather();
    // A preview is not a real shell startup, so a stale or meaningless boot time never appears.
    facts.remove("shell.boot_ms");

    let now = crate::clock::now_unix();
    let cache = load_cache(&config);
    let findings: Vec<crate::checks::Finding> = cache
        .as_ref()
        .map(|c| c.fresh(now).into_iter().cloned().collect())
        .unwrap_or_default();
    for finding in &findings {
        for (key, value) in &finding.facts {
            facts.insert(key, value.clone());
        }
    }

    let flavour = resolve_flavour(args.flavour.as_deref(), &config.flavour);
    if let Some(f) = &flavour {
        crate::flavour::apply(f, &mut facts);
    }
    let mode = color_mode();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let term_cols = crate::term::cols(1);
    let animate = animate_enabled(&config, args.no_animate, mode, stdout_is_tty);
    let key_fd = std::io::stdin().is_terminal().then_some(0);
    let sprinkles = resolve_sprinkles(
        config.sprinkles,
        env_var("SPARKLEBIOS_SPRINKLES").as_deref(),
    );
    let mut stdout = std::io::stdout();
    play_or_render(
        &machine,
        &facts,
        seed_from_time(),
        mode,
        term_cols,
        graphics(&config),
        flavour.as_ref(),
        &findings,
        animate,
        &mut stdout,
        key_fd,
        sprinkles,
    );

    refresh_if_stale(cache.as_ref(), now);
}

/// The real boot: `bios boot` (writes to stdout, keys discarded) or `bios boot --hook` (writes
/// to `/dev/tty`, and prints only the filtered typed bytes to stdout). Both share the same
/// decision, screen (always `pc95`), streak and state handling; only the target, whether the
/// screen animates (only on a `Full` decision) and what happens to the typed bytes differ.
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
    if decision == BootMode::Off {
        return;
    }

    let config = crate::config::load(crate::paths::config_dir().as_deref());

    if decision == BootMode::Quiet {
        run_quiet_fail_line(&config, now, tty_file);
        return;
    }

    let user_dir = crate::paths::user_machines_dir();
    let machine = crate::machine::find("pc95", user_dir.as_deref());
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };

    let yesterday = crate::clock::day_string(now as i64 - 86_400);
    state.advance_streak(&today, &yesterday);
    let mut facts = crate::facts::gather();
    facts.insert("streak.days", state.streak_days.to_string());
    facts.insert("streak.label", state.streak_label());

    let cache = load_cache(&config);
    let findings: Vec<crate::checks::Finding> = cache
        .as_ref()
        .map(|c| c.fresh(now).into_iter().cloned().collect())
        .unwrap_or_default();
    for finding in &findings {
        for (key, value) in &finding.facts {
            facts.insert(key, value.clone());
        }
    }

    let flavour = resolve_flavour(None, &config.flavour);
    if let Some(f) = &flavour {
        crate::flavour::apply(f, &mut facts);
    }

    let mode = color_mode();
    let cols_fd = tty_file.as_ref().map_or(1, |f| f.as_raw_fd());
    let term_cols = crate::term::cols(cols_fd);
    // Fast is the same screen drawn instantly: only a Full decision ever animates.
    let animate =
        decision == BootMode::Full && animate_enabled(&config, args.no_animate, mode, is_tty);
    let key_fd = tty_file
        .as_ref()
        .map(|f| f.as_raw_fd())
        .or_else(|| std::io::stdin().is_terminal().then_some(0));

    let sprinkles = resolve_sprinkles(
        config.sprinkles,
        env_var("SPARKLEBIOS_SPRINKLES").as_deref(),
    );
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
        graphics(&config),
        flavour.as_ref(),
        &findings,
        animate,
        out,
        key_fd,
        sprinkles,
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

    refresh_if_stale(cache.as_ref(), now);
}

/// The whole of what a `Quiet` decision does: nothing, unless `config.checks` is true and a fresh
/// `Fail` finding is sitting in the cache, in which case its rendered line (flavour phrasing
/// first, then the machine's) is written to `target` (the same `/dev/tty` or stdout a real boot
/// would use) as plain text: no paint, no border, no padding, no colour, and no F1 line. Spawns a
/// detached refresh when the cache is stale either way. Never touches state; that stays exactly
/// as the `Fast` and `Full` paths leave it.
fn run_quiet_fail_line(
    config: &crate::config::Config,
    now: u64,
    mut target: Option<std::fs::File>,
) {
    if !config.checks {
        return;
    }
    let Some(dir) = crate::paths::cache_dir() else {
        return;
    };
    let cache = crate::cache::load(&dir);
    let fail = cache
        .fresh(now)
        .into_iter()
        .find(|f| f.severity == crate::checks::Severity::Fail)
        .cloned();
    if let Some(finding) = fail {
        let user_dir = crate::paths::user_machines_dir();
        if let Some(machine) = crate::machine::find("pc95", user_dir.as_deref()) {
            let mut facts = crate::facts::gather();
            for (key, value) in &finding.facts {
                facts.insert(key, value.clone());
            }
            let flavour = resolve_flavour(None, &config.flavour);
            if let Some(f) = &flavour {
                crate::flavour::apply(f, &mut facts);
            }
            if let Some(text) =
                crate::render::finding_text(&machine, &facts, flavour.as_ref(), &finding)
            {
                let mut line = text;
                line.push('\n');
                let mut stdout_handle = std::io::stdout();
                let out: &mut dyn Write = match &mut target {
                    Some(f) => f,
                    None => &mut stdout_handle,
                };
                let _ = out.write_all(line.as_bytes());
                let _ = out.flush();
            }
        }
    }
    if cache.is_stale(now) {
        spawn_refresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GraphicsPref;
    use crate::render::Graphics;

    #[test]
    fn graphics_for_auto_and_image_pick_kitty_when_supported() {
        for pref in [GraphicsPref::Auto, GraphicsPref::Image] {
            assert_eq!(graphics_for(pref, true), Graphics::Kitty, "{pref:?}");
            assert_eq!(graphics_for(pref, false), Graphics::HalfBlocks, "{pref:?}");
        }
    }

    #[test]
    fn graphics_for_blocks_is_always_half_blocks_even_when_kitty_is_supported() {
        assert_eq!(
            graphics_for(GraphicsPref::Blocks, true),
            Graphics::HalfBlocks
        );
        assert_eq!(
            graphics_for(GraphicsPref::Blocks, false),
            Graphics::HalfBlocks
        );
    }

    #[test]
    fn resolve_graphics_pref_falls_back_to_the_config_value_with_no_env_override() {
        for pref in [
            GraphicsPref::Auto,
            GraphicsPref::Image,
            GraphicsPref::Blocks,
        ] {
            assert_eq!(resolve_graphics_pref(pref, None), pref, "{pref:?}");
        }
    }

    #[test]
    fn resolve_graphics_pref_env_override_wins_and_parses_all_three_values() {
        for (value, expected) in [
            ("auto", GraphicsPref::Auto),
            ("image", GraphicsPref::Image),
            ("blocks", GraphicsPref::Blocks),
        ] {
            assert_eq!(
                resolve_graphics_pref(GraphicsPref::Blocks, Some(value)),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn resolve_graphics_pref_an_unknown_env_value_reads_as_auto_and_still_overrides() {
        assert_eq!(
            resolve_graphics_pref(GraphicsPref::Blocks, Some("holographic")),
            GraphicsPref::Auto
        );
    }

    #[test]
    fn resolve_sprinkles_falls_back_to_config_with_no_env_and_env_wins_including_when_unknown() {
        use crate::sprinkles::Level;
        assert_eq!(resolve_sprinkles(Level::Full, None), Level::Full);
        assert_eq!(resolve_sprinkles(Level::Off, Some("full")), Level::Full);
        assert_eq!(
            resolve_sprinkles(Level::Full, Some("holographic")),
            Level::Off
        );
    }
}
