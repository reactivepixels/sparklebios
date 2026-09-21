//! The boot flow: decide, gather, render or animate, print, save.

use std::io::{IsTerminal, Write};
use std::os::unix::io::AsRawFd;
use std::process::Stdio;

use crate::mode::BootMode;

/// Arguments to `bios boot`.
pub struct BootArgs {
    /// Preview this machine id instead of the configured one (hidden flag).
    pub machine: Option<String>,
    /// Called from the shell hook, so unusual conditions stay silent rather than erroring.
    pub hook: bool,
    /// Skip the animation and draw the final frame directly.
    pub no_animate: bool,
    /// Preview this flavour instead of the configured one.
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

/// The image protocol the terminal speaks when `pref` is `Auto`; no mascot at all otherwise,
/// whether because the terminal speaks neither protocol or because `pref` is `Off`.
fn graphics_for(
    pref: crate::config::GraphicsPref,
    protocol: Option<crate::sprite::ImageProtocol>,
) -> crate::render::Graphics {
    if pref != crate::config::GraphicsPref::Auto {
        return crate::render::Graphics::None;
    }
    match protocol {
        Some(crate::sprite::ImageProtocol::Kitty) => crate::render::Graphics::Kitty,
        Some(crate::sprite::ImageProtocol::Iterm) => crate::render::Graphics::Iterm,
        None => crate::render::Graphics::None,
    }
}

/// Kitty on a terminal that supports it, no mascot otherwise, whether because the terminal cannot
/// draw one or because `config.graphics` (or the `SPARKLEBIOS_GRAPHICS` override) is `off`.
/// `render_static` only draws a logo or badge when its painted path is actually in use, so this
/// is safe to pass unconditionally.
fn graphics(config: &crate::config::Config) -> crate::render::Graphics {
    let pref = resolve_graphics_pref(config.graphics, env_var("SPARKLEBIOS_GRAPHICS").as_deref());
    let protocol = crate::sprite::detect_image_protocol(
        env_var("TERM").as_deref(),
        env_var("TERM_PROGRAM").as_deref(),
        env_var("LC_TERMINAL").as_deref(),
    );
    graphics_for(pref, protocol)
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
/// is nothing fresh in the cache. `calendar_line`, when given, may replace the `quip` step's line;
/// whether one is passed at all is entirely this function's caller's decision (see
/// `calendar_line_for`), forwarded unchanged to whichever of the two paths below actually draws
/// the screen, so a calendar day can never look different depending on whether it animated.
/// `sprinkles` is only ever read once the show actually animates: the static path never spends a
/// cycle on it. Returns the raw bytes read from `key_fd` while the show played, in order, empty
/// when it did not animate or nothing was typed.
///
/// `memory_count_sound`, when `Some`, is forwarded to `show::play` unchanged, and so only ever
/// takes effect when this call actually animates; the static path drops it silently, the same way
/// it never spends a cycle on `sprinkles`. See `memory_count_sound_for` for the one place that
/// decides whether to build one at all.
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
    calendar_line: Option<&str>,
    animate: bool,
    out: &mut dyn Write,
    key_fd: Option<i32>,
    sprinkles: crate::sprinkles::Level,
    memory_count_sound: Option<crate::show::MemoryCountSound>,
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
                machine,
                facts,
                seed,
                geometry,
                flavour,
                findings,
                calendar_line,
                out,
                key_fd,
                1.0,
                sprinkles,
                memory_count_sound,
            )
            .typed;
        }
    }
    let output = crate::render::render_static_with_calendar(
        machine,
        facts,
        mode,
        seed,
        term_cols,
        graphics,
        flavour,
        findings,
        calendar_line,
    );
    let _ = out.write_all(output.as_bytes());
    let _ = out.flush();
    Vec::new()
}

/// The calendar line to show in the `quip` slot today, or `None` when the current boot should
/// never show one. This is the single place that decision is made: `render.rs` only ever obeys
/// whatever it is given, and never asks which show it is in. `applies` is `true` for a Full boot
/// and for a preview (`run_preview` always plays the Full show), `false` for Fast and Quiet, so a
/// calendar day never changes what those two print. Returns the raw, unrendered text (still
/// carrying its `{slot}`s) of the first of `machine`'s calendar rules that fires on `(year, month,
/// day)`, or `None` when `applies` is `false` or no rule fires.
fn calendar_line_for(
    machine: &crate::machine::Machine,
    applies: bool,
    year: i32,
    month: u32,
    day: u32,
) -> Option<&str> {
    if !applies {
        return None;
    }
    crate::machine::matching_calendar_text(machine, year, month, day)
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

/// The ULTRA `memory_count` sound to hand `show::play`, or `None`. Built only for a real `Full`
/// daily boot (`decision`) at sprinkles `ultra`: never for Fast or Quiet, and never for a preview,
/// since `run_preview` never calls this at all. Beyond that gate, it still needs somewhere to find
/// a player: `player_env` (the raw `$SPARKLEBIOS_ULTRA_PLAYER`, set by the shell hook) must name
/// one, or there is no sound. The boot path never searches `PATH` for one itself (that search
/// exists for a person to run deliberately, in `bios sprinkles --check`, not for every boot to pay
/// for), so an unset or empty value reads the same as no player at all. `sounds_dir` must resolve
/// too, to find `memory_count.wav` in; taken as a parameter, like `player_env`, rather than read
/// from `paths::sounds_dir()` directly, so both can be fed fixed values in a test.
fn memory_count_sound_for(
    decision: BootMode,
    sprinkles: crate::sprinkles::Level,
    player_env: Option<&str>,
    sounds_dir: Option<std::path::PathBuf>,
    volume: f64,
) -> Option<crate::show::MemoryCountSound> {
    if decision != BootMode::Full || sprinkles != crate::sprinkles::Level::Ultra {
        return None;
    }
    let player = player_env.filter(|p| !p.is_empty())?;
    let sound_path = sounds_dir?.join("memory_count.wav");
    Some(crate::show::MemoryCountSound {
        player: player.to_string(),
        sound_path,
        volume,
    })
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
        run_shell_boot(args);
    } else {
        run_preview(args);
    }
}

/// `bios boot` typed by hand, with or without `--machine` or `--flavour`: a viewing command, not
/// the real boot. Always plays the `Full` show (never Off, Quiet or Fast): it ignores
/// `SPARKLEBIOS_BOOTED`, the burst window and the once-a-day rule entirely, and never touches the
/// state file, so looking at the screen never consumes the day's real boot or advances the
/// streak. It still respects everything about what the terminal can do rather than about boot
/// policy: a non-tty stdout draws the final screen instead of animating, same as `NO_COLOR`,
/// `SPARKLEBIOS_ANIMATE=0`, `--no-animate` and the `graphics` setting. The streak line shows the
/// streak already on disk, read without advancing it; a preview's `shell.boot_ms` is never a real
/// shell startup, so it is dropped rather than shown.
/// The facts that come from the state file, for both the preview and a real boot.
///
/// Zero is never inserted for either of these. "Boot streak: 0 days" and a best memory test of
/// 0K read as broken counters rather than true ones, so the slots stay unset and the lines that
/// use them are left out under the usual omission rule.
fn insert_state_facts(facts: &mut crate::facts::Facts, state: &crate::state::State) {
    if state.streak_days > 0 {
        facts.insert("streak.days", state.streak_days.to_string());
        facts.insert("streak.label", state.streak_label());
    }
    if state.memory_test_best_kb > 0 {
        facts.insert(
            "memory.best_test",
            format!("{}K", state.memory_test_best_kb),
        );
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

    // Read only: a preview shows the streak already on disk without advancing it.
    let state = match &crate::paths::state_dir() {
        Some(dir) => crate::state::State::load(dir),
        None => crate::state::State::default(),
    };
    insert_state_facts(&mut facts, &state);

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
    let (year, month, day) = crate::clock::local_ymd(now as i64);
    let calendar_line = calendar_line_for(&machine, true, year, month, day);
    let mut stdout = std::io::stdout();
    // A preview is never the real daily boot, so the ULTRA memory count sound never plays here,
    // whatever the level: see `memory_count_sound_for`, which only `run_shell_boot` calls.
    play_or_render(
        &machine,
        &facts,
        seed_from_time(),
        mode,
        term_cols,
        graphics(&config),
        flavour.as_ref(),
        &findings,
        calendar_line,
        animate,
        &mut stdout,
        key_fd,
        sprinkles,
        None,
    );

    refresh_if_stale(cache.as_ref(), now);
}

/// The real boot, run only by the shell hook as `bios boot --hook`: writes to `/dev/tty` and
/// prints only the filtered typed bytes to stdout. The Off, Quiet, Fast and Full decision
/// (`mode::decide`, including `SPARKLEBIOS_BOOTED` and the burst window), the screen (always
/// `pc95`), the streak and the state handling are all unchanged from before the hand-typed
/// viewing command split off into `run_preview`.
fn run_shell_boot(args: &BootArgs) {
    let mut tty_file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    {
        Ok(file) => file,
        Err(_) => return,
    };

    // SAFETY: `tty_file` is a valid, open file; `isatty` only reads its fd.
    let is_tty = unsafe { libc::isatty(tty_file.as_raw_fd()) != 0 };

    let now = crate::clock::now_unix();
    let today = crate::clock::day_string(now as i64);
    let state_dir = crate::paths::state_dir();
    let mut state = match &state_dir {
        Some(dir) => crate::state::State::load(dir),
        None => crate::state::State::default(),
    };

    // Loaded before the decision because the master switch lives in it. One small TOML read,
    // well inside the budget for deciding not to boot at all.
    let config = crate::config::load(crate::paths::config_dir().as_deref());

    let inputs = crate::mode::BootInputs {
        stdout_is_tty: is_tty,
        term: env_var("TERM"),
        kill_switch: env_var("SPARKLEBIOS_BOOT"),
        boot_enabled: config.boot,
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
    insert_state_facts(&mut facts, &state);

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
    let term_cols = crate::term::cols(tty_file.as_raw_fd());
    // Fast is the same screen drawn instantly: only a Full decision ever animates.
    let animate =
        decision == BootMode::Full && animate_enabled(&config, args.no_animate, mode, is_tty);
    let key_fd = Some(tty_file.as_raw_fd());

    let sprinkles = resolve_sprinkles(
        config.sprinkles,
        env_var("SPARKLEBIOS_SPRINKLES").as_deref(),
    );
    let memory_count_sound = memory_count_sound_for(
        decision,
        sprinkles,
        env_var("SPARKLEBIOS_ULTRA_PLAYER").as_deref(),
        crate::paths::sounds_dir(),
        config.ultra_volume,
    );
    // A calendar line only ever replaces the quip on a Full boot: Fast and Quiet stay exactly as
    // they were before calendar lines existed. Quiet never reaches this far (it returned above).
    let (year, month, day) = crate::clock::local_ymd(now as i64);
    let calendar_line = calendar_line_for(&machine, decision == BootMode::Full, year, month, day);
    let typed = play_or_render(
        &machine,
        &facts,
        seed_from_time(),
        mode,
        term_cols,
        graphics(&config),
        flavour.as_ref(),
        &findings,
        calendar_line,
        animate,
        &mut tty_file,
        key_fd,
        sprinkles,
        memory_count_sound,
    );
    write_stdout_bytes(&crate::show::filter_typed(&typed));

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
/// first, then the machine's) is written to `target` (the `/dev/tty` a real boot writes to) as
/// plain text: no paint, no border, no padding, no colour, and no F1 line. Spawns a detached
/// refresh when the cache is stale either way. Never touches state; that stays exactly as the
/// `Fast` and `Full` paths leave it.
fn run_quiet_fail_line(config: &crate::config::Config, now: u64, mut target: std::fs::File) {
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
                let _ = target.write_all(line.as_bytes());
                let _ = target.flush();
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
    use crate::sprite::ImageProtocol;

    #[test]
    fn graphics_for_auto_follows_whichever_protocol_the_terminal_speaks() {
        assert_eq!(
            graphics_for(GraphicsPref::Auto, Some(ImageProtocol::Kitty)),
            Graphics::Kitty
        );
        assert_eq!(
            graphics_for(GraphicsPref::Auto, Some(ImageProtocol::Iterm)),
            Graphics::Iterm
        );
        assert_eq!(graphics_for(GraphicsPref::Auto, None), Graphics::None);
    }

    #[test]
    fn graphics_for_off_is_always_none_whatever_the_terminal_speaks() {
        for protocol in [Some(ImageProtocol::Kitty), Some(ImageProtocol::Iterm), None] {
            assert_eq!(graphics_for(GraphicsPref::Off, protocol), Graphics::None);
        }
    }

    #[test]
    fn resolve_graphics_pref_falls_back_to_the_config_value_with_no_env_override() {
        for pref in [GraphicsPref::Auto, GraphicsPref::Off] {
            assert_eq!(resolve_graphics_pref(pref, None), pref, "{pref:?}");
        }
    }

    #[test]
    fn resolve_graphics_pref_env_override_wins_and_parses_both_values() {
        for (value, expected) in [("auto", GraphicsPref::Auto), ("off", GraphicsPref::Off)] {
            assert_eq!(
                resolve_graphics_pref(GraphicsPref::Off, Some(value)),
                expected,
                "{value}"
            );
        }
    }

    /// The retired `"image"` and `"blocks"` values, and any unknown value, all read as `Auto`,
    /// silently, through the environment override the same way they do through the config file
    /// (see `config::tests::graphics_reads_the_retired_and_unknown_values_as_auto`).
    #[test]
    fn resolve_graphics_pref_env_override_reads_retired_and_unknown_values_as_auto() {
        for value in ["image", "blocks", "holographic"] {
            assert_eq!(
                resolve_graphics_pref(GraphicsPref::Off, Some(value)),
                GraphicsPref::Auto,
                "{value}"
            );
        }
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

    // --- The ULTRA memory count sound ------------------------------------------------------------

    #[test]
    fn memory_count_sound_only_builds_for_a_full_boot_at_ultra_with_a_player_and_a_sounds_dir() {
        use crate::sprinkles::Level;
        let dir = Some(std::path::PathBuf::from("/sounds"));
        let sound = memory_count_sound_for(BootMode::Full, Level::Ultra, Some("afplay"), dir, 0.5)
            .expect("Full, ultra, a player and a sounds dir should build a sound");
        assert_eq!(sound.player, "afplay");
        assert_eq!(
            sound.sound_path,
            std::path::Path::new("/sounds/memory_count.wav")
        );
        assert_eq!(sound.volume, 0.5);
    }

    #[test]
    fn memory_count_sound_is_none_off_full_never_mind_the_level() {
        use crate::sprinkles::Level;
        let dir = || Some(std::path::PathBuf::from("/sounds"));
        for decision in [BootMode::Fast, BootMode::Quiet, BootMode::Off] {
            assert!(
                memory_count_sound_for(decision, Level::Ultra, Some("afplay"), dir(), 0.5)
                    .is_none(),
                "{decision:?} must never play the memory count sound"
            );
        }
    }

    #[test]
    fn memory_count_sound_is_none_below_ultra_even_on_a_full_boot() {
        use crate::sprinkles::Level;
        let dir = || Some(std::path::PathBuf::from("/sounds"));
        for level in [Level::Off, Level::Light, Level::Full] {
            assert!(
                memory_count_sound_for(BootMode::Full, level, Some("afplay"), dir(), 0.5).is_none(),
                "{level:?} must never play the memory count sound"
            );
        }
    }

    #[test]
    fn memory_count_sound_is_none_with_no_player() {
        use crate::sprinkles::Level;
        let dir = Some(std::path::PathBuf::from("/sounds"));
        assert!(
            memory_count_sound_for(BootMode::Full, Level::Ultra, None, dir.clone(), 0.5).is_none()
        );
        assert!(memory_count_sound_for(BootMode::Full, Level::Ultra, Some(""), dir, 0.5).is_none());
    }

    #[test]
    fn memory_count_sound_is_none_with_no_sounds_dir() {
        use crate::sprinkles::Level;
        assert!(
            memory_count_sound_for(BootMode::Full, Level::Ultra, Some("afplay"), None, 0.5)
                .is_none()
        );
    }

    // --- Calendar lines -------------------------------------------------------------------------

    /// This is the single place Full/preview vs Fast/Quiet is decided for a calendar line:
    /// `render.rs` and `show.rs` only ever obey whatever they are handed. `applies` stands in for
    /// that decision (`true` for Full and for a preview, `false` for Fast; Quiet never reaches
    /// this function at all, see `run_shell_boot`). 1 January is a real date `pc95` carries a
    /// rule for, checked directly against `machine::matching_calendar_text` rather than assumed.
    #[test]
    fn a_zero_counter_never_becomes_a_fact() {
        // Both of these read as broken hardware rather than a true reading, so the line that
        // would use them is omitted instead.
        let mut facts = crate::facts::Facts::new();
        insert_state_facts(&mut facts, &crate::state::State::default());
        assert_eq!(facts.get("streak.days"), None);
        assert_eq!(facts.get("memory.best_test"), None);
    }

    #[test]
    fn the_best_memory_test_is_a_fact_once_there_is_one() {
        let state = crate::state::State {
            streak_days: 3,
            memory_test_best_kb: 18_874_368,
            ..Default::default()
        };
        let mut facts = crate::facts::Facts::new();
        insert_state_facts(&mut facts, &state);
        assert_eq!(facts.get("streak.days"), Some("3"));
        assert_eq!(facts.get("memory.best_test"), Some("18874368K"));
    }

    #[test]
    fn calendar_line_for_applies_only_when_told_to_even_on_a_real_calendar_date() {
        let m = crate::machine::find("pc95", None).unwrap();
        let real_match = crate::machine::matching_calendar_text(&m, 2026, 1, 1);
        assert!(
            real_match.is_some(),
            "1 January should be a real calendar date for pc95"
        );
        assert_eq!(calendar_line_for(&m, true, 2026, 1, 1), real_match);
        assert_eq!(calendar_line_for(&m, false, 2026, 1, 1), None);
    }

    #[test]
    fn calendar_line_for_is_none_on_an_ordinary_day_regardless_of_whether_it_applies() {
        let m = crate::machine::find("pc95", None).unwrap();
        assert_eq!(calendar_line_for(&m, true, 2026, 6, 15), None);
        assert_eq!(calendar_line_for(&m, false, 2026, 6, 15), None);
    }

    /// Fast draws through `play_or_render` with `animate: false` and whatever `calendar_line` it
    /// is given; on a real calendar date it is given `None` (`calendar_line_for(..., false, ...)`,
    /// exactly as `run_shell_boot` computes for a Fast decision), so its output must be byte for
    /// byte identical to `render::render_static`, the calendar-oblivious entry point that existed
    /// before calendar lines did. This is checked against 1 January, a real date `pc95` has a rule
    /// for, not a mocked matcher.
    #[test]
    fn fast_renders_byte_for_byte_the_same_as_the_calendar_oblivious_render_on_a_real_calendar_date(
    ) {
        let m = crate::machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let mut facts = crate::facts::Facts::fixture();
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-01");
        crate::flavour::apply(&flavour, &mut facts);

        assert!(crate::machine::matching_calendar_text(&m, 2026, 1, 1).is_some());
        let calendar_line = calendar_line_for(&m, false, 2026, 1, 1);
        assert_eq!(calendar_line, None, "Fast must never apply a calendar line");

        let mut fast_out: Vec<u8> = Vec::new();
        play_or_render(
            &m,
            &facts,
            0,
            crate::render::ColorMode::None,
            None,
            crate::render::Graphics::None,
            Some(&flavour),
            &[],
            calendar_line,
            false,
            &mut fast_out,
            None,
            crate::sprinkles::Level::Off,
            None,
        );

        let expected = crate::render::render_static(
            &m,
            &facts,
            crate::render::ColorMode::None,
            0,
            None,
            crate::render::Graphics::None,
            Some(&flavour),
            &[],
        );
        assert_eq!(String::from_utf8(fast_out).unwrap(), expected);
    }

    /// Quiet is unaffected by construction, not merely by test: `run_shell_boot` returns through
    /// `run_quiet_fail_line` before any calendar computation runs (see the `decision ==
    /// BootMode::Quiet` branch above `calendar_line_for` is ever called), and `run_quiet_fail_line`
    /// draws its one possible line through `render::finding_text`, a function this milestone never
    /// gave a `calendar_line` parameter, so there is no path by which a calendar day could reach
    /// it. Exercising `run_quiet_fail_line` itself needs a real cache directory resolved from
    /// `XDG_CACHE_HOME`/`HOME`, which is not sandboxed anywhere else in this file; mutating those
    /// process wide variables from a test in a shared, concurrently edited tree risks exactly the
    /// kind of cross-test leakage the sandboxing rule exists to prevent, so that path is left to
    /// the integration suite rather than added here.
    #[test]
    fn quiet_never_calls_the_calendar_override() {
        let m = crate::machine::find("pc95", None).unwrap();
        // Quiet's own render call, `render::finding_text`, takes no `calendar_line` of any kind.
        let finding = crate::checks::Finding {
            id: "boot_order".to_string(),
            severity: crate::checks::Severity::Info,
            ttl: 100,
            facts: vec![("boot.devices".to_string(), "eko-pro".to_string())]
                .into_iter()
                .collect(),
        };
        let mut facts = crate::facts::Facts::fixture();
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-01");
        facts.insert("boot.devices", "eko-pro");
        let with_calendar_date = crate::render::finding_text(&m, &facts, None, &finding);
        facts.insert("date.today", "2026-06-15");
        let on_an_ordinary_day = crate::render::finding_text(&m, &facts, None, &finding);
        assert_eq!(with_calendar_date, on_an_ordinary_day);
    }
}
