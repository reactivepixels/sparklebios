//! `bios fetch`: a one-shot system summary screen, the neofetch slot. The mascot on the left, a
//! column of facts on the right, and a palette swatch at the bottom, printed once and never
//! touched again. Not on the boot path, so it may take its time, but it still must never touch
//! the network.

use crate::facts::Facts;
use crate::render::{ColorMode, Graphics};
use crate::sprite::{Grid, Sprite};

/// The label and value template for each line of the fact panel, in the exact order the screen
/// shows them. A line whose template does not resolve against the facts is simply absent: never
/// blank, never a placeholder.
const ROWS: [(&str, &str); 9] = [
    ("OS", "{os.name} {os.version}"),
    ("Shell", "{shell.name}"),
    ("Terminal", "{terminal.name}"),
    ("CPU", "{cpu.name}, {cpu.cores} cores"),
    ("Memory", "{mem.gb} GB"),
    ("Disk", "{disk.size_gb}GB, {disk.used_pct}% used"),
    ("Flavour", "{flavour.name}"),
    ("Theme", "{ghostty.theme}"),
    ("Streak", "{streak.label}"),
];

/// The gap, in cells, between the mascot and the fact panel beside it.
const MASCOT_GAP: usize = 3;

/// The width, in cells, of one palette swatch block.
const SWATCH_BLOCK_WIDTH: usize = 3;

/// Every label padded to this width before its colon: two more than the longest of `ROWS`'s own
/// labels, fixed regardless of which lines end up resolving, so omitting one line never shifts
/// where the colons land on the others.
fn label_width() -> usize {
    ROWS.iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0)
        + 2
}

/// One `"Label     : value"` line per row whose template resolves against `facts`.
fn fact_lines(facts: &Facts) -> Vec<String> {
    let width = label_width();
    ROWS.iter()
        .filter_map(|&(label, template)| {
            let value = crate::template::render(template, facts)?;
            Some(format!("{label:<width$}: {value}"))
        })
        .collect()
}

/// One row of 8 background coloured blocks, ANSI colours `start..start + 8`, using the plain 16
/// colour SGR codes (40-47, then the aixterm bright extension 100-107) so the swatch shows
/// whatever palette the terminal itself is rendering with, rather than a fixed RGB guess.
fn palette_row(start: u8) -> String {
    (start..start + 8)
        .map(|code| {
            let sgr = if code < 8 { 40 + code } else { 92 + code };
            format!("\x1b[{sgr}m{}\x1b[0m", " ".repeat(SWATCH_BLOCK_WIDTH))
        })
        .collect()
}

/// The wide 28 by 28 grid when a screen `term_cols` cells wide still leaves `widest` (the widest
/// line of the panel beside it) room after the grid and `MASCOT_GAP`; the original 14 by 14 grid
/// otherwise. An unknown `term_cols` is treated as room enough, since a caller with no terminal
/// to measure has no way to know otherwise.
fn choose_grid(sprite: &Sprite, widest: usize, term_cols: Option<u16>) -> &Grid {
    let needed = sprite.grid_wide.cols() + MASCOT_GAP + widest;
    let fits_wide = term_cols.map_or(true, |w| w as usize >= needed);
    if fits_wide {
        &sprite.grid_wide
    } else {
        &sprite.grid
    }
}

/// The finished screen as text. `header`, the current flavour's rendered firmware line, sits on
/// its own above everything else. `sprite` is the current flavour's mascot; with `None`, or when
/// `mode` is not `TrueColor`, the screen falls back to a plain column with no mascot (matching the
/// boot screen's own rule that a logo only ever appears in the truecolor painted path). The
/// palette swatch is skipped entirely when `mode` is `ColorMode::None`.
pub fn render_screen(
    facts: &Facts,
    header: Option<&str>,
    sprite: Option<&Sprite>,
    mode: ColorMode,
    term_cols: Option<u16>,
    graphics: Graphics,
) -> String {
    let mut out = String::new();
    if let Some(header) = header {
        out.push_str(header);
        out.push('\n');
    }

    let show_color = mode != ColorMode::None;
    let show_mascot = mode == ColorMode::TrueColor && sprite.is_some();

    // What appears beside (or, with no mascot, simply after) the header: the fact lines, then,
    // when colour is on, a blank separator and the two palette rows.
    let mut panel = fact_lines(facts);
    let mut widths: Vec<usize> = panel.iter().map(|l| l.chars().count()).collect();
    if show_color {
        widths.push(8 * SWATCH_BLOCK_WIDTH);
        panel.push(String::new());
        panel.push(palette_row(0));
        panel.push(palette_row(8));
    }

    if !show_mascot {
        out.push('\n');
        for line in &panel {
            out.push_str(line);
            out.push('\n');
        }
        return out;
    }

    let sprite = sprite.unwrap();
    let widest = widths.into_iter().max().unwrap_or(0);
    let grid = choose_grid(sprite, widest, term_cols);
    let grid_cols = grid.cols();
    let grid_rows = grid.half_rows();

    let half_rows = if graphics == Graphics::HalfBlocks {
        crate::sprite::half_blocks(grid, None)
    } else {
        Vec::new()
    };
    let kitty_escape = if graphics == Graphics::Kitty {
        Some(crate::sprite::kitty_image(
            sprite.png,
            grid_cols as u16,
            grid_rows as u16,
        ))
    } else {
        None
    };

    out.push('\n');
    let total_rows = grid_rows.max(panel.len());
    for i in 0..total_rows {
        if i == 0 {
            if let Some(escape) = &kitty_escape {
                out.push_str(escape);
            }
        }
        if i < grid_rows {
            if kitty_escape.is_some() {
                out.push_str(&" ".repeat(grid_cols));
            } else if let Some(row) = half_rows.get(i) {
                out.push_str(row);
            }
        } else {
            out.push_str(&" ".repeat(grid_cols));
        }
        out.push_str(&" ".repeat(MASCOT_GAP));
        if let Some(line) = panel.get(i) {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// The flavour to draw the screen as: the same resolution rule `bios boot` uses, the configured
/// flavour falling back to the built-in `unicorn`.
fn resolve_flavour(config_flavour: &str) -> Option<crate::flavour::Flavour> {
    let dir = crate::paths::user_flavours_dir();
    crate::flavour::find(config_flavour, dir.as_deref())
        .or_else(|| crate::flavour::find("unicorn", dir.as_deref()))
}

/// The effective graphics choice: the same rule `bios boot` uses, kitty on a terminal that
/// supports it unless `config.graphics` (or `SPARKLEBIOS_GRAPHICS`) is `blocks`.
fn resolve_graphics(config: &crate::config::Config) -> Graphics {
    let pref = std::env::var("SPARKLEBIOS_GRAPHICS")
        .ok()
        .as_deref()
        .map(crate::config::GraphicsPref::parse)
        .unwrap_or(config.graphics);
    let supports_kitty = crate::sprite::supports_kitty(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
    );
    if pref != crate::config::GraphicsPref::Blocks && supports_kitty {
        Graphics::Kitty
    } else {
        Graphics::HalfBlocks
    }
}

/// `bios fetch`: gathers the real facts and prints the screen once. Never panics outward.
pub fn run() -> i32 {
    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let flavour = resolve_flavour(&config.flavour);

    let mut facts = crate::facts::gather();
    if let Some(kb) = facts.get("mem.kb").and_then(|v| v.parse::<u64>().ok()) {
        facts.insert("mem.gb", crate::facts::kb_to_whole_gb(kb).to_string());
    }
    if let Some(terminal) = crate::facts::terminal_name(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    ) {
        facts.insert("terminal.name", terminal);
    }
    if let Some(theme) = crate::facts::ghostty_theme(&crate::paths::ghostty_config_candidates()) {
        facts.insert("ghostty.theme", theme);
    }
    if let Some(dir) = crate::paths::state_dir() {
        let state = crate::state::State::load(&dir);
        if state.streak_days > 0 {
            facts.insert("streak.label", state.streak_label());
        }
    }
    if let Some(f) = &flavour {
        facts.insert("flavour.name", f.name.clone());
    }

    let header = flavour
        .as_ref()
        .and_then(|f| crate::template::render(&f.firmware, &facts));
    let sprite = flavour
        .as_ref()
        .and_then(|f| crate::sprite::builtin(&f.sprite));

    let mode = crate::render::color_mode_from_env(
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    );
    let term_cols = crate::term::cols(1);
    let graphics = resolve_graphics(&config);

    let output = render_screen(
        &facts,
        header.as_deref(),
        sprite.as_ref(),
        mode,
        term_cols,
        graphics,
    );
    print!("{output}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flavour::Flavour;

    fn golden(id: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/golden/{id}.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn unicorn_flavour() -> Flavour {
        crate::flavour::find("unicorn", None).unwrap()
    }

    /// `Facts::fixture()` plus the fetch-only facts a real `run()` would add, deterministic for
    /// tests and golden files: `terminal.name`, `mem.gb` and `ghostty.theme`.
    fn fixture() -> Facts {
        let mut facts = Facts::fixture();
        facts.insert("mem.gb", "36");
        facts.insert("terminal.name", "Ghostty");
        facts.insert("ghostty.theme", "rainbows-and-unicorns-mane");
        facts.insert("flavour.name", "Unicorn");
        facts
    }

    fn firmware(facts: &Facts) -> String {
        crate::template::render(&unicorn_flavour().firmware, facts).unwrap()
    }

    /// Strips `\x1b[...m` SGR sequences and `\x1b_G...\x1b\` kitty sequences, leaving only the
    /// visible characters.
    fn strip_ansi(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'_') {
                i += 2;
                while i < chars.len() && !(chars[i] == '\x1b' && chars.get(i + 1) == Some(&'\\')) {
                    i += 1;
                }
                i += 2;
            } else if chars[i] == '\x1b' {
                i += 1;
                while i < chars.len() && chars[i] != 'm' {
                    i += 1;
                }
                i += 1;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    #[test]
    fn matches_the_fetch_unicorn_golden_with_colour_off() {
        let facts = fixture();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            None,
            ColorMode::None,
            None,
            Graphics::HalfBlocks,
        );
        assert_eq!(out, golden("fetch-unicorn"));
    }

    #[test]
    fn every_labels_colon_lines_up() {
        let facts = fixture();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            None,
            ColorMode::None,
            None,
            Graphics::HalfBlocks,
        );
        let columns: Vec<usize> = out.lines().filter_map(|line| line.find(": ")).collect();
        assert!(columns.len() >= 9, "expected 9 labelled lines: {out:?}");
        let first = columns[0];
        for col in &columns {
            assert_eq!(*col, first, "colons do not line up in {out:?}");
        }
    }

    #[test]
    fn a_missing_value_omits_its_line_and_nothing_else_shifts() {
        let with_streak = fixture();
        let mut without_streak = fixture();
        without_streak.remove("streak.label");
        without_streak.remove("streak.days");

        let with_out = render_screen(
            &with_streak,
            Some(&firmware(&with_streak)),
            None,
            ColorMode::None,
            None,
            Graphics::HalfBlocks,
        );
        let without_out = render_screen(
            &without_streak,
            Some(&firmware(&without_streak)),
            None,
            ColorMode::None,
            None,
            Graphics::HalfBlocks,
        );

        assert!(with_out.contains("Streak    : 12 days"));
        assert!(!without_out.contains("Streak"));

        let with_lines: Vec<&str> = with_out.lines().collect();
        let without_lines: Vec<&str> = without_out.lines().collect();
        assert_eq!(with_lines.len(), without_lines.len() + 1);
        // Every line other than Streak is untouched: the CPU line, in particular, sits at the
        // same position in both outputs.
        assert!(with_lines.iter().any(|l| l.starts_with("CPU")));
        for line in &without_lines {
            if line.starts_with("CPU") {
                assert!(with_lines.contains(line));
            }
        }
    }

    #[test]
    fn the_wide_mascot_is_used_when_there_is_room() {
        let facts = fixture();
        let sprite = crate::sprite::builtin("unicorn").unwrap();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            Some(&sprite),
            ColorMode::TrueColor,
            Some(120),
            Graphics::HalfBlocks,
        );
        let block_rows = out
            .lines()
            .skip(2)
            .take_while(|line| line.contains('\u{2580}') || line.contains('\u{2584}'))
            .count();
        assert_eq!(block_rows, 14, "expected the wide 14 row mascot: {out}");
    }

    #[test]
    fn the_narrow_mascot_is_used_when_the_terminal_is_too_narrow() {
        let facts = fixture();
        let sprite = crate::sprite::builtin("unicorn").unwrap();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            Some(&sprite),
            ColorMode::TrueColor,
            Some(60),
            Graphics::HalfBlocks,
        );
        let block_rows = out
            .lines()
            .skip(2)
            .take_while(|line| line.contains('\u{2580}') || line.contains('\u{2584}'))
            .count();
        assert_eq!(block_rows, 7, "expected the narrow 7 row mascot: {out}");
    }

    #[test]
    fn no_color_produces_no_escape_sequences_at_all() {
        let facts = fixture();
        let sprite = crate::sprite::builtin("unicorn").unwrap();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            Some(&sprite),
            ColorMode::None,
            Some(120),
            Graphics::HalfBlocks,
        );
        assert!(!out.contains('\x1b'));
        assert_eq!(strip_ansi(&out), out);
    }

    #[test]
    fn the_palette_swatch_is_skipped_when_colour_is_off() {
        let facts = fixture();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            None,
            ColorMode::None,
            None,
            Graphics::HalfBlocks,
        );
        // The header, the blank separator, and exactly the 9 fact lines: no swatch rows tacked
        // on the end.
        assert_eq!(out.lines().count(), 11, "{out:?}");
    }

    #[test]
    fn ansi16_shows_the_swatch_but_no_mascot() {
        let facts = fixture();
        let sprite = crate::sprite::builtin("unicorn").unwrap();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            Some(&sprite),
            ColorMode::Ansi16,
            Some(120),
            Graphics::HalfBlocks,
        );
        assert!(!out.contains('\u{2580}') && !out.contains('\u{2584}'));
        assert!(out.contains("\x1b[40m"));
    }

    #[test]
    fn kitty_graphics_emits_the_transmission_escape_once() {
        let facts = fixture();
        let sprite = crate::sprite::builtin("unicorn").unwrap();
        let out = render_screen(
            &facts,
            Some(&firmware(&facts)),
            Some(&sprite),
            ColorMode::TrueColor,
            Some(120),
            Graphics::Kitty,
        );
        assert_eq!(out.matches("\x1b_Ga=T").count(), 1);
    }
}
