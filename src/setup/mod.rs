//! The CMOS Setup Utility: `bios setup`.
//!
//! The state machine is in `model`, the drawing in `view`, the key table in `keys`. All three
//! are pure. This module is the only part that touches a terminal, and its one job beyond the
//! loop is making sure the terminal is handed back exactly as it was found, on every path out
//! including a panic.

pub mod keys;
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

/// `bios setup`. Returns the process exit code.
pub fn run() -> i32 {
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
    if (cols as usize) < view::WIDTH || (rows as usize) < view::HEIGHT {
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

    let outcome = {
        let Some(_raw) = crate::tty::RawGuard::new(fd) else {
            eprintln!("{not_a_terminal}");
            return 1;
        };
        let mut screen = Screen::enter(write_tty);
        run_loop(&mut state, &mut screen, fd, cols, rows, &year)
        // Both guards drop here, so the terminal is itself again before anything below prints.
    };

    match outcome {
        Outcome::Exit => 0,
        Outcome::Save => save(&state),
    }
}

fn run_loop(
    state: &mut State,
    screen: &mut Screen,
    fd: i32,
    cols: u16,
    rows: u16,
    year: &str,
) -> Outcome {
    let redraw = |screen: &mut Screen, state: &State| {
        screen.draw(&view::render(state, cols as usize, rows as usize, year));
    };
    redraw(screen, state);

    let mut pending: Vec<u8> = Vec::new();
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
                pending.remove(0);
                continue;
            };
            pending.drain(..used);
            let effect = match input {
                keys::Input::Key(key) => state.key(key),
                keys::Input::Answer(yes) => state.answer(yes),
            };
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
        }
    }
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
}
