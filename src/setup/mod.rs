//! The CMOS Setup Utility: `bios setup`.
//!
//! The state machine is in `model`, the drawing in `view`, the key table in `keys`. All three
//! are pure. This module is the only part that touches a terminal, and its one job beyond the
//! loop is making sure the terminal is handed back exactly as it was found, on every path out
//! including a panic.

pub mod keys;
pub mod memtest;
pub mod model;
pub mod view;

use std::io::Write;
use std::os::unix::io::AsRawFd;

use model::{Effect, Row, Setting, State};

/// How long to wait for the rest of an escape sequence before deciding a lone escape was Esc.
/// Long enough for a terminal to deliver an arrow key's three bytes, short enough that pressing
/// Esc does not feel stuck.
const ESCAPE_GRACE_MS: u64 = 40;

/// Puts the terminal on the alternate screen with the cursor hidden, and puts it back on drop.
///
/// This is separate from the raw mode guard so the two are undone independently. Dropping
/// happens on a normal return, on an early return, and while a panic unwinds, which is the whole
/// point of it being a guard rather than a pair of calls.
struct Screen {
    out: std::fs::File,
}

impl Screen {
    fn enter(out: std::fs::File) -> Screen {
        let mut screen = Screen { out };
        screen.write("\x1b[?1049h\x1b[?25l\x1b[2J");
        screen
    }

    fn write(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
        let _ = self.out.flush();
    }

    /// Redraws from the top left. The screen is always drawn whole, so there is nothing to erase
    /// beyond the clear.
    fn draw(&mut self, body: &str) {
        self.write("\x1b[H\x1b[2J");
        self.write(body);
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // Cursor shown, off the alternate screen, attributes reset. Failures are ignored: there
        // is nothing useful to do about them, and the attempt must always be made.
        self.write("\x1b[0m\x1b[?25h\x1b[?1049l");
    }
}

enum Outcome {
    Exit,
    Save,
}

/// `bios setup`. Returns the process exit code. `memory_test` skips straight to the Memory Test
/// easter egg instead of the CMOS screen: a hidden door for tests, and for anyone who reads the
/// source far enough to find it. The screen itself gives no hint that either way in exists.
pub fn run(memory_test: bool) -> i32 {
    let not_a_terminal = "bios: setup needs a terminal. This is not one.";
    let Ok(tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    let fd = tty.as_raw_fd();
    // SAFETY: `fd` is a valid open file descriptor; `isatty` only reads it.
    if unsafe { libc::isatty(fd) } == 0 {
        eprintln!("{not_a_terminal}");
        return 1;
    }
    let Some((cols, rows)) = crate::term::size(fd) else {
        eprintln!("{not_a_terminal}");
        return 1;
    };
    if (cols as usize) < view::MIN_WIDTH || (rows as usize) < view::MIN_HEIGHT {
        eprintln!("bios: setup needs a terminal of at least 80x24. Yours is {cols}x{rows}.");
        return 1;
    }
    let Ok(write_tty) = tty.try_clone() else {
        eprintln!("{not_a_terminal}");
        return 1;
    };

    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let mut state = State::new(rows_for(&config));
    let year = crate::facts::gather()
        .get("date.year")
        .unwrap_or("2026")
        .to_string();
    let truecolor = view::truecolor_capable(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    );
    // The same decision `bios boot` and `bios fetch` already make; see `render::color_mode_from_env`.
    let no_color = crate::render::color_mode_from_env(
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    ) == crate::render::ColorMode::None;

    let outcome = {
        let Some(_raw) = crate::tty::RawGuard::new(fd) else {
            eprintln!("{not_a_terminal}");
            return 1;
        };
        let mut screen = Screen::enter(write_tty);
        if memory_test {
            launch_memory_test(&mut screen, fd, cols, rows);
            Outcome::Exit
        } else {
            run_loop(
                &mut state,
                &mut screen,
                fd,
                cols,
                rows,
                &year,
                truecolor,
                no_color,
            )
        }
        // Both guards drop here, so the terminal is itself again before anything below prints.
    };

    match outcome {
        Outcome::Exit => 0,
        Outcome::Save => save(&state),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_loop(
    state: &mut State,
    screen: &mut Screen,
    fd: i32,
    cols: u16,
    rows: u16,
    year: &str,
    truecolor: bool,
    no_color: bool,
) -> Outcome {
    let redraw = |screen: &mut Screen, state: &State| {
        screen.draw(&view::render(
            state,
            cols as usize,
            rows as usize,
            year,
            truecolor,
            no_color,
        ));
    };
    redraw(screen, state);

    let mut pending: Vec<u8> = Vec::new();
    let mut code = CodeTracker::new();
    // Toggling Turbo ten times in one visit is the second, easier door in: a reward for anyone
    // who pokes at the one setting that does nothing.
    let mut turbo_toggles = 0u32;
    loop {
        // Bounded waits rather than a blocking read, so nothing can wedge here.
        pending.extend_from_slice(&crate::tty::wait_for_key(fd, 250));
        if pending.is_empty() {
            continue;
        }
        // A lone escape both is a key and begins every arrow key, so give the rest a moment to
        // turn up before deciding it was Esc on its own.
        if pending == [0x1b] {
            pending.extend_from_slice(&crate::tty::wait_for_key(fd, ESCAPE_GRACE_MS));
        }

        while !pending.is_empty() {
            let Some((input, used)) = keys::decode(&pending) else {
                // Not a key setup acts on, but 'b' and 'a' still feed the secret sequence: it is
                // watched at the byte level because setup otherwise ignores them completely.
                let triggered = code.feed(SeqKey::from_byte(pending[0]));
                pending.remove(0);
                if triggered {
                    launch_memory_test(screen, fd, cols, rows);
                    code = CodeTracker::new();
                    turbo_toggles = 0;
                    redraw(screen, state);
                }
                continue;
            };
            pending.drain(..used);

            let code_triggered = code.feed(SeqKey::from_input(input));
            let is_turbo_step = is_turbo_toggle(input, state.current().setting);

            let effect = match input {
                keys::Input::Key(key) => state.key(key),
                keys::Input::Answer(yes) => state.answer(yes),
            };
            if is_turbo_step {
                turbo_toggles += 1;
            }

            match effect {
                Effect::Nothing => {}
                Effect::Redraw => redraw(screen, state),
                Effect::Preview => {
                    preview(state, screen, fd);
                    redraw(screen, state);
                }
                Effect::Save => return Outcome::Save,
                Effect::Exit => return Outcome::Exit,
            }

            if code_triggered || turbo_toggles >= 10 {
                launch_memory_test(screen, fd, cols, rows);
                code = CodeTracker::new();
                turbo_toggles = 0;
                redraw(screen, state);
            }
        }
    }
}

/// Whether `input` is a Left or Right press landing on the Turbo row: one step towards the
/// second door in, ten of these in one visit.
fn is_turbo_toggle(input: keys::Input, current: Setting) -> bool {
    matches!(
        input,
        keys::Input::Key(model::Key::Left | model::Key::Right)
    ) && current == Setting::Turbo
}

/// One step of the way in: the setup screen's own arrow keys, or the plain `b` and `a` it
/// otherwise ignores. Everything else is `Other`, which resets the sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeqKey {
    Up,
    Down,
    Left,
    Right,
    B,
    A,
    Other,
}

impl SeqKey {
    fn from_input(input: keys::Input) -> SeqKey {
        match input {
            keys::Input::Key(model::Key::Up) => SeqKey::Up,
            keys::Input::Key(model::Key::Down) => SeqKey::Down,
            keys::Input::Key(model::Key::Left) => SeqKey::Left,
            keys::Input::Key(model::Key::Right) => SeqKey::Right,
            keys::Input::Key(_) | keys::Input::Answer(_) => SeqKey::Other,
        }
    }

    fn from_byte(byte: u8) -> SeqKey {
        match byte {
            b'a' => SeqKey::A,
            b'b' => SeqKey::B,
            _ => SeqKey::Other,
        }
    }
}

/// Up Up Down Down Left Right Left Right b a: matched on the last ten keys, the classic way in.
const CODE: [SeqKey; 10] = [
    SeqKey::Up,
    SeqKey::Up,
    SeqKey::Down,
    SeqKey::Down,
    SeqKey::Left,
    SeqKey::Right,
    SeqKey::Left,
    SeqKey::Right,
    SeqKey::B,
    SeqKey::A,
];

/// Tracks progress through `CODE`. Any key that is not the next one in the sequence resets
/// progress to zero, unless it happens to be the sequence's own first key, in which case progress
/// resets to one rather than being thrown away.
struct CodeTracker {
    progress: usize,
}

impl CodeTracker {
    fn new() -> CodeTracker {
        CodeTracker { progress: 0 }
    }

    /// Feeds one key in. Returns whether the sequence has just completed.
    fn feed(&mut self, key: SeqKey) -> bool {
        if key == CODE[self.progress] {
            self.progress += 1;
            if self.progress == CODE.len() {
                self.progress = 0;
                return true;
            }
        } else if key == CODE[0] {
            self.progress = 1;
        } else {
            self.progress = 0;
        }
        false
    }
}

/// Gathers what the Memory Test needs from the real machine and `state.json`, and plays one game
/// on the screen and reader `run_loop` already has open.
fn launch_memory_test(screen: &mut Screen, fd: i32, cols: u16, rows: u16) {
    let mem_kb: u64 = crate::facts::gather()
        .get("mem.kb")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let (best_kb, cleared_before) = match crate::paths::state_dir() {
        Some(dir) => {
            let state = crate::state::State::load(&dir);
            (state.memory_test_best_kb, state.memory_test_cleared)
        }
        None => (0, false),
    };
    // The seed is the one place this door is allowed to be impure: the model it feeds is
    // otherwise fully deterministic.
    let seed = crate::clock::now_unix();
    memtest::run(
        screen,
        memtest::Session {
            fd,
            cols,
            rows,
            mem_kb,
            best_kb,
            cleared_before,
            seed,
        },
    );
}

/// Leaves the setup screen, draws the boot screen as the pending settings would make it, and
/// waits for a key before going back.
fn preview(state: &State, screen: &mut Screen, fd: i32) {
    let flavour = value_for(state, Setting::Flavour)
        .and_then(|name| {
            crate::flavour::list(crate::paths::user_flavours_dir().as_deref())
                .into_iter()
                .find(|f| f.name == name)
        })
        .or_else(|| crate::flavour::find("unicorn", None));
    let Some(machine) = crate::machine::find("pc95", crate::paths::user_machines_dir().as_deref())
    else {
        return;
    };

    let mut facts = crate::facts::gather();
    // A preview is not a boot, so a boot time from one would be meaningless.
    facts.remove("shell.boot_ms");
    if let Some(f) = &flavour {
        crate::flavour::apply(f, &mut facts);
    }
    // The mascot is an image, and an image cannot be drawn into a preview that is about to be
    // wiped by the alternate screen, so the preview is drawn without one either way.
    let body = crate::render::render_static(
        &machine,
        &facts,
        crate::render::ColorMode::TrueColor,
        0,
        None,
        crate::render::Graphics::None,
        flavour.as_ref(),
        &[],
    );

    screen.write("\x1b[?1049l\x1b[?25h");
    screen.write(&body);
    screen.write("\nPress any key to return to SETUP.\n");
    // Drop anything already buffered, so a key pressed while it drew does not skip the pause.
    let _ = crate::tty::wait_for_key(fd, 1);
    while crate::tty::wait_for_key(fd, 250).is_empty() {}
    screen.write("\x1b[?1049h\x1b[?25l");
}

fn value_for(state: &State, setting: Setting) -> Option<&str> {
    state
        .rows
        .iter()
        .find(|r| r.setting == setting)
        .map(|r| r.value())
}

const HELP_FLAVOUR: &str =
    "The mascot and its firmware. All flavours run the same checks. Only the attitude changes.";
const HELP_THEME: &str = "The Ghostty colour theme. Unchanged leaves your terminal colours alone.";
const HELP_MASCOT: &str = "Shown draws the mascot as a real image where the terminal supports one. Terminals that cannot show images boot without it. Hidden boots without it everywhere.";
const HELP_SPRINKLES: &str =
    "Optional effects during the daily show. Off by default, because not everyone wants sprinkles.";
const HELP_DAILY: &str = "The animated boot, once a day. Disabled boots instantly every time.";
const HELP_BOOT: &str =
    "The master switch. Disabled means new tabs print nothing at all. The BIOS will wait.";
const HELP_TURBO: &str =
    "Does nothing. It never did. This setting is not saved, in keeping with tradition.";

/// The rows, built from the config as it stands, so every one starts on its current value.
fn rows_for(config: &crate::config::Config) -> Vec<Row> {
    let user_dir = crate::paths::user_flavours_dir();
    let flavours: Vec<String> = crate::flavour::list(user_dir.as_deref())
        .into_iter()
        .map(|f| f.name)
        .collect();
    let current = crate::flavour::find(&config.flavour, user_dir.as_deref())
        .map(|f| f.name)
        .unwrap_or_else(|| "Unicorn".to_string());
    let flavour_at = flavours.iter().position(|n| *n == current).unwrap_or(0);

    // The active Ghostty theme is not ours to read, so the row starts on leaving it alone.
    let mut themes = vec!["Unchanged".to_string()];
    themes.extend(
        crate::theme::SHORT_NAMES
            .iter()
            .map(|(short, _)| capitalise(short)),
    );

    let sprinkles_at = match config.sprinkles {
        crate::sprinkles::Level::Off => 0,
        crate::sprinkles::Level::Light => 1,
        crate::sprinkles::Level::Full => 2,
    };

    vec![
        row(
            "Flavour",
            Setting::Flavour,
            flavours,
            flavour_at,
            HELP_FLAVOUR,
        ),
        row("Theme", Setting::Theme, themes, 0, HELP_THEME),
        row(
            "Mascot",
            Setting::Mascot,
            strings(&["Shown", "Hidden"]),
            usize::from(config.graphics == crate::config::GraphicsPref::Off),
            HELP_MASCOT,
        ),
        row(
            "Sprinkles",
            Setting::Sprinkles,
            strings(&["Off", "Light", "Full"]),
            sprinkles_at,
            HELP_SPRINKLES,
        ),
        row(
            "Daily Show",
            Setting::DailyShow,
            strings(&["Enabled", "Disabled"]),
            usize::from(!config.animate),
            HELP_DAILY,
        ),
        row(
            "Boot Screen",
            Setting::BootScreen,
            strings(&["Enabled", "Disabled"]),
            usize::from(!config.boot),
            HELP_BOOT,
        ),
        row(
            "Turbo",
            Setting::Turbo,
            strings(&["On", "Off"]),
            0,
            HELP_TURBO,
        ),
    ]
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn row(
    label: &'static str,
    setting: Setting,
    values: Vec<String>,
    initial: usize,
    help: &'static str,
) -> Row {
    Row {
        label,
        setting,
        values,
        selected: initial,
        initial,
        help,
    }
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Writes the changed settings, and returns the exit code.
fn save(state: &State) -> i32 {
    let Some(dir) = crate::paths::config_dir() else {
        eprintln!("bios: cannot find a config directory");
        return 1;
    };
    let user_dir = crate::paths::user_flavours_dir();
    let mut theme: Option<String> = None;

    for (setting, value) in state.changes() {
        let wrote = match setting {
            Setting::Flavour => match crate::flavour::list(user_dir.as_deref())
                .into_iter()
                .find(|f| f.name == value)
            {
                Some(f) => crate::config::set_flavour(&dir, &f.id),
                None => Ok(()),
            },
            Setting::Mascot => crate::config::set_key(
                &dir,
                "graphics",
                &format!("\"{}\"", if value == "Hidden" { "off" } else { "auto" }),
            ),
            Setting::Sprinkles => {
                crate::config::set_key(&dir, "sprinkles", &format!("\"{}\"", value.to_lowercase()))
            }
            Setting::DailyShow => {
                crate::config::set_key(&dir, "animate", bool_str(value == "Enabled"))
            }
            Setting::BootScreen => {
                crate::config::set_key(&dir, "boot", bool_str(value == "Enabled"))
            }
            Setting::Theme => {
                theme = Some(value.to_lowercase());
                Ok(())
            }
            Setting::Turbo => Ok(()),
        };
        if wrote.is_err() {
            eprintln!("bios: cannot write the config file");
            return 1;
        }
    }

    println!("CMOS updated. Changes take effect at the next boot, which is the next tab.");
    match theme {
        Some(short) => crate::cli::theme_use_for_setup(&short),
        None => 0,
    }
}

fn bool_str(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_is_capitalised_for_the_screen() {
        assert_eq!(capitalise("sorbet"), "Sorbet");
        assert_eq!(capitalise("six"), "Six");
        assert_eq!(capitalise(""), "");
    }

    #[test]
    fn the_rows_start_on_whatever_the_config_says() {
        let config = crate::config::Config {
            animate: false,
            boot: false,
            graphics: crate::config::GraphicsPref::Off,
            sprinkles: crate::sprinkles::Level::Full,
            ..crate::config::Config::default()
        };
        let rows = rows_for(&config);
        let value = |s: Setting| {
            rows.iter()
                .find(|r| r.setting == s)
                .map(|r| r.value().to_string())
                .unwrap()
        };
        assert_eq!(value(Setting::DailyShow), "Disabled");
        assert_eq!(value(Setting::BootScreen), "Disabled");
        assert_eq!(value(Setting::Mascot), "Hidden");
        assert_eq!(value(Setting::Sprinkles), "Full");
        assert_eq!(
            value(Setting::Theme),
            "Unchanged",
            "the active theme is not ours to read, so it is never assumed"
        );
    }

    #[test]
    fn nothing_starts_out_changed() {
        let state = State::new(rows_for(&crate::config::Config::default()));
        assert!(!state.dirty());
        assert!(state.changes().is_empty());
    }

    #[test]
    fn every_flavour_is_offered_by_name() {
        let rows = rows_for(&crate::config::Config::default());
        let flavours = rows.iter().find(|r| r.setting == Setting::Flavour).unwrap();
        assert!(flavours.values.contains(&"Unicorn".to_string()));
        assert!(flavours.values.contains(&"Raccoon".to_string()));
        assert_eq!(flavours.values.len(), crate::flavour::builtins().len());
    }

    #[test]
    fn the_theme_row_offers_unchanged_first_then_every_theme() {
        let rows = rows_for(&crate::config::Config::default());
        let themes = rows.iter().find(|r| r.setting == Setting::Theme).unwrap();
        assert_eq!(themes.values[0], "Unchanged");
        assert_eq!(themes.values.len(), crate::theme::SHORT_NAMES.len() + 1);
        assert!(themes.values.contains(&"Sorbet".to_string()));
    }

    #[test]
    fn every_row_has_help_that_fits_its_pane() {
        for r in rows_for(&crate::config::Config::default()) {
            assert!(!r.help.is_empty(), "{} has no help", r.label);
        }
    }

    fn feed_all(code: &mut CodeTracker, keys: &[SeqKey]) -> bool {
        let mut triggered = false;
        for &k in keys {
            triggered = code.feed(k);
        }
        triggered
    }

    #[test]
    fn the_exact_key_sequence_starts_the_game() {
        let mut code = CodeTracker::new();
        assert!(feed_all(&mut code, &CODE));
    }

    #[test]
    fn a_broken_sequence_does_not_start_it() {
        let mut code = CodeTracker::new();
        let mut broken = CODE.to_vec();
        broken[4] = SeqKey::Right; // Left, where the real sequence expects it, swapped out
        assert!(!feed_all(&mut code, &broken));
    }

    #[test]
    fn near_misses_reset_progress_rather_than_leaving_it_stuck() {
        let mut code = CodeTracker::new();
        // Up Up Down Down Left, then something else entirely, then the sequence proper.
        assert!(!feed_all(
            &mut code,
            &[
                SeqKey::Up,
                SeqKey::Up,
                SeqKey::Down,
                SeqKey::Down,
                SeqKey::Left,
                SeqKey::Other,
            ]
        ));
        assert!(feed_all(&mut code, &CODE));
    }

    #[test]
    fn an_extra_key_before_the_sequence_does_not_stop_it_starting_fresh_right_after() {
        let mut code = CodeTracker::new();
        // One stray key, then the sequence proper: the stray does not linger.
        assert!(!code.feed(SeqKey::Other));
        assert!(feed_all(&mut code, &CODE));
    }

    #[test]
    fn ten_turbo_toggles_start_it_but_nine_do_not() {
        let toggle = keys::Input::Key(model::Key::Left);
        for count in [9, 10] {
            let mut turbo_toggles = 0u32;
            for _ in 0..count {
                if is_turbo_toggle(toggle, Setting::Turbo) {
                    turbo_toggles += 1;
                }
            }
            assert_eq!(turbo_toggles >= 10, count == 10, "count={count}");
        }
    }

    #[test]
    fn only_left_and_right_on_the_turbo_row_count_as_a_toggle() {
        assert!(is_turbo_toggle(
            keys::Input::Key(model::Key::Left),
            Setting::Turbo
        ));
        assert!(is_turbo_toggle(
            keys::Input::Key(model::Key::Right),
            Setting::Turbo
        ));
        assert!(!is_turbo_toggle(
            keys::Input::Key(model::Key::Left),
            Setting::Flavour
        ));
        assert!(!is_turbo_toggle(
            keys::Input::Key(model::Key::Up),
            Setting::Turbo
        ));
        assert!(!is_turbo_toggle(keys::Input::Answer(true), Setting::Turbo));
    }
}
