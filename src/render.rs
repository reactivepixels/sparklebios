//! Static rendering and colour modes.

use crate::facts::Facts;
use crate::machine::{Machine, Step, Style};
use crate::template;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    None,
    Ansi16,
    TrueColor,
}

/// How the logo is drawn in the painted path. Plain and unpainted output never show a logo or
/// a badge, regardless of this setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graphics {
    None,
    HalfBlocks,
    Kitty,
}

/// One resolved, styled run of text within a logical line.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// NO_COLOR set to anything non-empty: None. COLORTERM of "truecolor" or "24bit": TrueColor. Otherwise Ansi16.
pub fn color_mode_from_env(no_color: Option<&str>, colorterm: Option<&str>) -> ColorMode {
    if matches!(no_color, Some(v) if !v.is_empty()) {
        return ColorMode::None;
    }
    match colorterm {
        Some("truecolor") | Some("24bit") => ColorMode::TrueColor,
        _ => ColorMode::Ansi16,
    }
}

/// Parses a `#RRGGBB` colour (validated already by `machine::parse`) into its components.
fn hex_rgb(colour: &str) -> (u8, u8, u8) {
    let bytes = colour.as_bytes();
    let byte = |i: usize| u8::from_str_radix(std::str::from_utf8(&bytes[i..i + 2]).unwrap(), 16);
    (
        byte(1).unwrap_or(0),
        byte(3).unwrap_or(0),
        byte(5).unwrap_or(0),
    )
}

fn colour_for(machine: &Machine, style: Style) -> &str {
    match style {
        Style::Normal => &machine.fg,
        Style::Bright => &machine.bright,
        Style::Accent => &machine.accent,
    }
}

/// Wraps `text` in the escape codes for `style`, or returns it unchanged for `ColorMode::None`.
fn paint(machine: &Machine, mode: ColorMode, style: Style, text: &str) -> String {
    match mode {
        ColorMode::None => text.to_string(),
        ColorMode::TrueColor => {
            let (r, g, b) = hex_rgb(colour_for(machine, style));
            format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m")
        }
        ColorMode::Ansi16 => {
            let code = match style {
                Style::Normal => "37",
                Style::Bright => "97",
                Style::Accent => "93",
            };
            format!("\x1b[{code}m{text}\x1b[0m")
        }
    }
}

/// One line from a resolved quip, starting at `seed % quips.len()` and wrapping around. A
/// candidate whose rendered length exceeds `machine.cols` is treated as unresolvable.
fn resolve_quip(machine: &Machine, facts: &Facts, seed: u64) -> Option<String> {
    let quips = &machine.quips;
    if quips.is_empty() {
        return None;
    }
    let start = (seed % quips.len() as u64) as usize;
    (0..quips.len()).find_map(|offset| {
        let idx = (start + offset) % quips.len();
        let text = template::render(&quips[idx], facts)?;
        if text.chars().count() > machine.cols as usize {
            None
        } else {
            Some(text)
        }
    })
}

/// Applies `machine.uppercase`, if set.
fn apply_case(machine: &Machine, text: String) -> String {
    if machine.uppercase {
        text.to_uppercase()
    } else {
        text
    }
}

/// A line is blank when every one of its spans is empty text.
fn is_blank(line: &[Span]) -> bool {
    line.iter().all(|span| span.text.is_empty())
}

/// Resolves a single step to its logical line, or None to omit it.
fn layout_step(machine: &Machine, facts: &Facts, seed: u64, step: &Step) -> Option<Vec<Span>> {
    match step {
        Step::Print { text, style, .. } => {
            let text = template::render(text, facts)?;
            if text.is_empty() {
                Some(vec![])
            } else {
                Some(vec![Span {
                    text: apply_case(machine, text),
                    style: *style,
                }])
            }
        }
        Step::Count {
            template: tmpl,
            to,
            suffix,
            ..
        } => {
            let n = template::render(to, facts)?;
            let filled = tmpl.replacen("{n}", &n, 1);
            let mut line = template::render(&filled, facts)?;
            line.push_str(suffix);
            if line.is_empty() {
                Some(vec![])
            } else {
                Some(vec![Span {
                    text: apply_case(machine, line),
                    style: Style::Normal,
                }])
            }
        }
        Step::Detect {
            label,
            result,
            style,
            ..
        } => {
            let result = template::render(result, facts)?;
            Some(vec![
                Span {
                    text: apply_case(machine, format!("{label}... ")),
                    style: Style::Normal,
                },
                Span {
                    text: apply_case(machine, result),
                    style: *style,
                },
            ])
        }
        Step::Quip { style, .. } => {
            let quip = resolve_quip(machine, facts, seed)?;
            Some(vec![Span {
                text: apply_case(machine, quip),
                style: *style,
            }])
        }
    }
}

/// Never two blank lines in a row, and never a leading or trailing blank line.
fn collapse_blank_lines(lines: Vec<Vec<Span>>) -> Vec<Vec<Span>> {
    let mut collapsed: Vec<Vec<Span>> = Vec::with_capacity(lines.len());
    for line in lines {
        let previous_is_blank = collapsed.last().map(|l| is_blank(l)).unwrap_or(true);
        if is_blank(&line) && previous_is_blank {
            continue;
        }
        collapsed.push(line);
    }
    while collapsed.last().is_some_and(|l| is_blank(l)) {
        collapsed.pop();
    }
    collapsed
}

/// The machine's steps resolved into logical lines, with unresolvable and too-long-to-fit
/// lines omitted and blank lines collapsed. `seed` picks the quip: start at `seed % quips.len()`.
pub fn layout(machine: &Machine, facts: &Facts, seed: u64) -> Vec<Vec<Span>> {
    let lines: Vec<Vec<Span>> = machine
        .steps
        .iter()
        .filter_map(|step| layout_step(machine, facts, seed, step))
        .collect();
    collapse_blank_lines(lines)
}

/// The plain rendering: every span painted in place, one line per row.
fn render_plain(machine: &Machine, mode: ColorMode, lines: &[Vec<Span>]) -> String {
    let mut out = String::new();
    for line in lines {
        for span in line {
            out.push_str(&paint(machine, mode, span.style, &span.text));
        }
        out.push('\n');
    }
    out
}

/// `width` cells of `rgb`, as a single coloured span, reset at the end.
fn bg_block(rgb: (u8, u8, u8), width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let (r, g, b) = rgb;
    format!("\x1b[48;2;{r};{g};{b}m{}\x1b[0m", " ".repeat(width))
}

/// One row of a painted block, built from a logical line's spans: border pillars, background
/// padding, an optional logo box, the line's text (truncated to its budget, shrunk further to
/// leave room for a badge when one applies) filled out with background, the badge itself, the
/// padding and pillars mirrored, then a trailing reset.
///
/// `logo` is `Some((cell, kitty_escape))` for the first 7 text rows of a machine with a unicorn
/// logo: `cell` is the pre-rendered 14-cell half-block row for that sprite row (absent in Kitty
/// mode, where the box is left as background), and `kitty_escape` is the transmission escape,
/// present only on the row that must emit it (the first text row, once).
///
/// `badge`, when present, is right-aligned inside the `cols` text area, in the accent colour.
/// The text budget shrinks by the badge's width plus a 2 cell gap, so the badge never lands
/// within 2 cells of text; if the badge is too wide to reserve that room at all (rare, only on
/// very narrow machines), it is dropped instead and the row's text keeps its full budget.
#[allow(clippy::too_many_arguments)]
fn painted_row(
    machine: &Machine,
    bg: (u8, u8, u8),
    border: Option<(u8, u8, u8)>,
    pad_x: usize,
    cols: usize,
    spans: &[Span],
    logo: Option<(Option<&str>, Option<&str>)>,
    badge: Option<&str>,
) -> String {
    let (bg_r, bg_g, bg_b) = bg;
    let mut row = String::new();
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str(&bg_block(bg, pad_x));

    let shift = logo.is_some();
    if let Some((cell, kitty_escape)) = logo {
        if let Some(escape) = kitty_escape {
            row.push_str(escape);
        }
        match cell {
            Some(cell) => row.push_str(cell),
            None => row.push_str(&bg_block(bg, 14)),
        }
        row.push_str(&bg_block(bg, 2));
    }
    let shift_offset = if shift { 16 } else { 0 };
    let normal_budget = cols.saturating_sub(shift_offset);
    let badge_len = badge.map(|b| b.chars().count()).unwrap_or(0);
    let reserved = badge_len + 2;
    let show_badge = badge.is_some() && reserved <= normal_budget;
    let text_budget = if show_badge {
        normal_budget - reserved
    } else {
        normal_budget
    };

    let mut used = 0usize;
    for span in spans {
        if used >= text_budget {
            break;
        }
        let text: String = span.text.chars().take(text_budget - used).collect();
        if text.is_empty() {
            continue;
        }
        used += text.chars().count();
        let (fr, fg, fb) = hex_rgb(colour_for(machine, span.style));
        row.push_str(&format!(
            "\x1b[38;2;{fr};{fg};{fb};48;2;{bg_r};{bg_g};{bg_b}m{text}\x1b[0m"
        ));
    }

    let text_end = shift_offset + used;
    if show_badge {
        let badge = badge.unwrap();
        let badge_start = cols - badge_len;
        if badge_start > text_end {
            row.push_str(&bg_block(bg, badge_start - text_end));
        }
        let (ar, ag, ab) = hex_rgb(&machine.accent);
        row.push_str(&format!(
            "\x1b[38;2;{ar};{ag};{ab};48;2;{bg_r};{bg_g};{bg_b}m{badge}\x1b[0m"
        ));
    } else if text_end < cols {
        row.push_str(&bg_block(bg, cols - text_end));
    }

    row.push_str(&bg_block(bg, pad_x));
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str("\x1b[0m\n");
    row
}

/// `cols + 2*pad_x`, plus 4 more when there is a border.
fn painted_total_width(machine: &Machine) -> u16 {
    machine.cols + 2 * u16::from(machine.pad_x) + if machine.border.is_some() { 4 } else { 0 }
}

/// The logo box is 14 cells wide and 7 rows tall, at the top left of the text area, shown only
/// when the machine names a known logo, the block is wide enough, and graphics are on.
const LOGO_ROWS: usize = 7;
const MIN_COLS_FOR_GRAPHICS: usize = 60;

/// The painted block: a border bar, `pad_y` blank rows, the text rows, `pad_y` more blank rows,
/// then another border bar. The logo (if any) is drawn over the first 7 text rows, and the badge
/// (if any) is right-aligned over the fill of the first few text rows.
fn render_painted(machine: &Machine, lines: &[Vec<Span>], graphics: Graphics) -> String {
    let bg = hex_rgb(machine.bg.as_deref().unwrap_or("#000000"));
    let border = machine.border.as_deref().map(hex_rgb);
    let pad_x = machine.pad_x as usize;
    let pad_y = machine.pad_y as usize;
    let cols = machine.cols as usize;
    let total_width = painted_total_width(machine) as usize;
    let blank: Vec<Span> = Vec::new();

    let show_logo = graphics != Graphics::None
        && cols >= MIN_COLS_FOR_GRAPHICS
        && machine.logo.as_deref() == Some("unicorn");
    let sprite_rows = if show_logo && graphics == Graphics::HalfBlocks {
        crate::sprite::half_blocks(crate::sprite::UNICORN_GRID, bg)
    } else {
        Vec::new()
    };
    let kitty_escape = if show_logo && graphics == Graphics::Kitty {
        Some(crate::sprite::kitty_image(
            crate::sprite::UNICORN_PNG,
            14,
            7,
        ))
    } else {
        None
    };
    let show_badge =
        graphics != Graphics::None && cols >= MIN_COLS_FOR_GRAPHICS && !machine.badge.is_empty();

    let mut out = String::new();
    if let Some(border) = border {
        out.push_str(&bg_block(border, total_width));
        out.push_str("\x1b[0m\n");
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, &blank, None, None,
        ));
    }
    for (i, line) in lines.iter().enumerate() {
        let logo = if show_logo && i < LOGO_ROWS {
            let cell = sprite_rows.get(i).map(String::as_str);
            let escape = if i == 0 {
                kitty_escape.as_deref()
            } else {
                None
            };
            Some((cell, escape))
        } else {
            None
        };
        let badge = if show_badge {
            machine.badge.get(i).map(String::as_str)
        } else {
            None
        };
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, line, logo, badge,
        ));
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, &blank, None, None,
        ));
    }
    if let Some(border) = border {
        out.push_str(&bg_block(border, total_width));
        out.push_str("\x1b[0m\n");
    }
    out
}

/// The finished screen as text. Every line ends with '\n'. `seed` picks the quip: start at
/// `seed % quips.len()`. `term_cols` is the terminal's width, when known: a painted machine uses
/// it to decide whether the block fits, falling back to the plain rendering when it does not (or
/// when the width is unknown, or the mode is not TrueColor). `graphics` chooses how a logo is
/// drawn in the painted path; the plain path never shows a logo or badge regardless of it.
pub fn render_static(
    machine: &Machine,
    facts: &Facts,
    mode: ColorMode,
    seed: u64,
    term_cols: Option<u16>,
    graphics: Graphics,
) -> String {
    let lines = layout(machine, facts, seed);
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        render_painted(machine, &lines, graphics)
    } else {
        render_plain(machine, mode, &lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{facts::Facts, machine};

    fn golden(id: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/golden/{id}.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
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
                i += 2; // skip the ESC \ terminator
            } else if chars[i] == '\x1b' {
                i += 1;
                while i < chars.len() && chars[i] != 'm' {
                    i += 1;
                }
                i += 1; // skip the 'm'
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    fn visible_width(s: &str) -> usize {
        strip_ansi(s).chars().count()
    }

    #[test]
    fn pc95_matches_golden() {
        let m = machine::find("pc95", None).unwrap();
        assert_eq!(
            render_static(
                &m,
                &Facts::fixture(),
                ColorMode::None,
                0,
                None,
                Graphics::None
            ),
            golden("pc95")
        );
    }
    #[test]
    fn pc85_matches_golden() {
        let m = machine::find("pc85", None).unwrap();
        assert_eq!(
            render_static(
                &m,
                &Facts::fixture(),
                ColorMode::None,
                0,
                None,
                Graphics::None
            ),
            golden("pc85")
        );
    }
    #[test]
    fn c64_matches_golden() {
        let m = machine::find("c64", None).unwrap();
        assert_eq!(
            render_static(
                &m,
                &Facts::fixture(),
                ColorMode::None,
                0,
                None,
                Graphics::None
            ),
            golden("c64")
        );
    }
    #[test]
    fn unresolved_lines_vanish_and_blank_lines_collapse() {
        let m = machine::find("pc95", None).unwrap();
        let mut f = Facts::fixture();
        let mut g = Facts::new();
        for k in ["date.year", "date.bios", "cpu.name", "cpu.cores", "mem.kb"] {
            g.insert(k, f.get(k).unwrap());
        }
        f = g;
        let out = render_static(&m, &f, ColorMode::None, 0, None, Graphics::None);
        assert!(!out.contains("Detecting Shell"));
        assert!(!out.contains("Boot streak"));
        assert!(out.contains("Detecting Horn"));
        assert!(!out.contains("\n\n\n"));
        assert!(!out.starts_with('\n') && !out.ends_with("\n\n"));
        assert!(!out.contains('{'));
    }
    #[test]
    fn empty_facts_never_leak_a_slot() {
        for m in machine::builtins() {
            let out = render_static(&m, &Facts::new(), ColorMode::None, 0, None, Graphics::None);
            assert!(
                !out.contains('{') && !out.contains('}'),
                "{} leaked a slot",
                m.id
            );
        }
    }
    #[test]
    fn truecolor_wraps_spans_and_styles_only_the_detect_result() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            None,
            Graphics::None,
        );
        assert!(out.contains("\x1b[38;2;255;255;255mSparkle Modular BIOS"));
        assert!(out.contains(
            "\x1b[38;2;170;170;170mDetecting Horn             ... \x1b[0m\x1b[38;2;255;255;85m1 found"
        ));
        assert!(!out.contains("\x1b[1m"));
    }
    #[test]
    fn ansi16_uses_basic_codes() {
        let m = machine::find("pc85", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::Ansi16,
            0,
            None,
            Graphics::None,
        );
        assert!(out.contains("\x1b[37m37748736K OK\x1b[0m"));
    }
    #[test]
    fn seed_rotates_quips_and_skips_unresolvable_ones() {
        let m = machine::find("pc95", None).unwrap();
        let out1 = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            1,
            None,
            Graphics::None,
        );
        assert!(out1.contains("Plug and Pray devices found: 1 unicorn"));
        // quip 7 needs {mem.kb}; without it the next resolvable quip is used
        let mut f = Facts::new();
        f.insert("date.year", "2026");
        let out7 = render_static(&m, &f, ColorMode::None, 7, None, Graphics::None);
        assert!(out7.contains("Floppy drive A: not found. Nobody is surprised."));
        let wrapped = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            m.quips.len() as u64,
            None,
            Graphics::None,
        );
        assert!(wrapped.contains("Turbo button engaged."));
    }
    #[test]
    fn color_mode_from_env_rules() {
        assert_eq!(
            color_mode_from_env(Some("1"), Some("truecolor")),
            ColorMode::None
        );
        assert_eq!(
            color_mode_from_env(Some(""), Some("truecolor")),
            ColorMode::TrueColor
        );
        assert_eq!(
            color_mode_from_env(None, Some("24bit")),
            ColorMode::TrueColor
        );
        assert_eq!(color_mode_from_env(None, None), ColorMode::Ansi16);
    }
    #[test]
    fn painted_c64_at_80_columns_has_a_44_wide_border_and_text_rows() {
        let m = machine::find("c64", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            Some(80),
            Graphics::None,
        );
        let rows: Vec<&str> = out.lines().collect();
        assert!(!rows.is_empty());
        for row in &rows {
            assert_eq!(visible_width(row), 44, "row {row:?} is not 44 wide");
        }
        let first = rows.first().unwrap();
        let last = rows.last().unwrap();
        for border_row in [first, last] {
            assert!(border_row.contains("48;2;108;94;181"));
            assert!(!border_row.contains("38;2;"));
        }
        let text_row = rows.iter().find(|r| r.contains("READY")).unwrap();
        assert!(text_row.contains("38;2;108;94;181;48;2;53;40;121"));
    }
    #[test]
    fn c64_falls_back_to_plain_truecolor_when_too_narrow_or_unknown() {
        let m = machine::find("c64", None).unwrap();
        for term_cols in [Some(30), None] {
            let out = render_static(
                &m,
                &Facts::fixture(),
                ColorMode::TrueColor,
                0,
                term_cols,
                Graphics::None,
            );
            assert!(!out.contains("48;2;"));
        }
    }
    #[test]
    fn none_mode_with_paint_is_byte_identical_to_the_golden() {
        let m = machine::find("c64", None).unwrap();
        assert_eq!(
            render_static(
                &m,
                &Facts::fixture(),
                ColorMode::None,
                0,
                Some(80),
                Graphics::None
            ),
            golden("c64")
        );
    }
    #[test]
    fn a_quip_longer_than_cols_is_skipped() {
        let src = r##"
id = "t3"
name = "Test"
cols = 20
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
quips = ["this quip is much too long to fit", "short"]
[[step]]
quip = true
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(&m, &Facts::new(), ColorMode::None, 0, None, Graphics::None);
        assert_eq!(out, "short\n");
    }

    #[test]
    fn painted_pc95_with_half_blocks_places_the_logo_and_badge() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
        );
        let rows: Vec<&str> = out.lines().collect();
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }
        assert!(rows[1].contains('\u{2580}'));
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        let pad_x = m.pad_x as usize;
        let byte_col = stripped[1].find("Sparkle Modular BIOS").unwrap();
        let text_col = stripped[1][..byte_col].chars().count();
        assert_eq!(text_col, pad_x + 16);
        assert!(stripped[1].trim_end().ends_with("enchantment"));
        assert!(stripped[2].trim_end().ends_with("*STAR* ALLY"));
        assert!(stripped[3].trim_end().ends_with("GLITTER SAFE"));
    }

    #[test]
    fn painted_pc95_with_kitty_emits_one_image_and_no_half_blocks() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::Kitty,
        );
        assert_eq!(out.matches("\x1b_Ga=T").count(), 1);
        assert!(!out.contains('\u{2580}'));
    }

    #[test]
    fn painted_pc95_with_graphics_none_has_no_logo_or_badge() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::None,
        );
        assert!(!out.contains("\x1b_G"));
        assert!(!out.contains('\u{2580}'));
        assert!(!out.contains("enchantment"));
    }

    #[test]
    fn plain_pc95_has_no_logo_or_badge() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            None,
            Graphics::HalfBlocks,
        );
        assert!(!out.contains("\x1b_G"));
        assert!(!out.contains('\u{2580}'));
        assert!(!out.contains("enchantment"));
    }
}
