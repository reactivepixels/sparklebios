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
/// padding, the line's text truncated to `cols` characters and filled out with background, the
/// padding and pillars mirrored, then a trailing reset.
fn painted_row(
    machine: &Machine,
    bg: (u8, u8, u8),
    border: Option<(u8, u8, u8)>,
    pad_x: usize,
    cols: usize,
    spans: &[Span],
) -> String {
    let (bg_r, bg_g, bg_b) = bg;
    let mut row = String::new();
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str(&bg_block(bg, pad_x));
    let mut used = 0usize;
    for span in spans {
        if used >= cols {
            break;
        }
        let text: String = span.text.chars().take(cols - used).collect();
        if text.is_empty() {
            continue;
        }
        used += text.chars().count();
        let (fr, fg, fb) = hex_rgb(colour_for(machine, span.style));
        row.push_str(&format!(
            "\x1b[38;2;{fr};{fg};{fb};48;2;{bg_r};{bg_g};{bg_b}m{text}\x1b[0m"
        ));
    }
    if used < cols {
        row.push_str(&bg_block(bg, cols - used));
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

/// The painted block: a border bar, `pad_y` blank rows, the text rows, `pad_y` more blank rows,
/// then another border bar.
fn render_painted(machine: &Machine, lines: &[Vec<Span>]) -> String {
    let bg = hex_rgb(machine.bg.as_deref().unwrap_or("#000000"));
    let border = machine.border.as_deref().map(hex_rgb);
    let pad_x = machine.pad_x as usize;
    let pad_y = machine.pad_y as usize;
    let cols = machine.cols as usize;
    let total_width = painted_total_width(machine) as usize;
    let blank: Vec<Span> = Vec::new();

    let mut out = String::new();
    if let Some(border) = border {
        out.push_str(&bg_block(border, total_width));
        out.push_str("\x1b[0m\n");
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(machine, bg, border, pad_x, cols, &blank));
    }
    for line in lines {
        out.push_str(&painted_row(machine, bg, border, pad_x, cols, line));
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(machine, bg, border, pad_x, cols, &blank));
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
/// when the width is unknown, or the mode is not TrueColor).
pub fn render_static(
    machine: &Machine,
    facts: &Facts,
    mode: ColorMode,
    seed: u64,
    term_cols: Option<u16>,
) -> String {
    let lines = layout(machine, facts, seed);
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        render_painted(machine, &lines)
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

    /// Strips `\x1b...m` escape sequences, leaving only the visible characters.
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for c2 in chars.by_ref() {
                    if c2 == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
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
            render_static(&m, &Facts::fixture(), ColorMode::None, 0, None),
            golden("pc95")
        );
    }
    #[test]
    fn pc85_matches_golden() {
        let m = machine::find("pc85", None).unwrap();
        assert_eq!(
            render_static(&m, &Facts::fixture(), ColorMode::None, 0, None),
            golden("pc85")
        );
    }
    #[test]
    fn c64_matches_golden() {
        let m = machine::find("c64", None).unwrap();
        assert_eq!(
            render_static(&m, &Facts::fixture(), ColorMode::None, 0, None),
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
        let out = render_static(&m, &f, ColorMode::None, 0, None);
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
            let out = render_static(&m, &Facts::new(), ColorMode::None, 0, None);
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
        let out = render_static(&m, &Facts::fixture(), ColorMode::TrueColor, 0, None);
        assert!(out.contains("\x1b[38;2;255;255;255mSparkle Modular BIOS"));
        assert!(out.contains(
            "\x1b[38;2;170;170;170mDetecting Horn             ... \x1b[0m\x1b[38;2;255;255;85m1 found"
        ));
        assert!(!out.contains("\x1b[1m"));
    }
    #[test]
    fn ansi16_uses_basic_codes() {
        let m = machine::find("pc85", None).unwrap();
        let out = render_static(&m, &Facts::fixture(), ColorMode::Ansi16, 0, None);
        assert!(out.contains("\x1b[37m37748736K OK\x1b[0m"));
    }
    #[test]
    fn seed_rotates_quips_and_skips_unresolvable_ones() {
        let m = machine::find("pc95", None).unwrap();
        let out1 = render_static(&m, &Facts::fixture(), ColorMode::None, 1, None);
        assert!(out1.contains("Plug and Pray devices found: 1 unicorn"));
        // quip 7 needs {mem.kb}; without it the next resolvable quip is used
        let mut f = Facts::new();
        f.insert("date.year", "2026");
        let out7 = render_static(&m, &f, ColorMode::None, 7, None);
        assert!(out7.contains("Floppy drive A: not found. Nobody is surprised."));
        let wrapped = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            m.quips.len() as u64,
            None,
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
        let out = render_static(&m, &Facts::fixture(), ColorMode::TrueColor, 0, Some(80));
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
            let out = render_static(&m, &Facts::fixture(), ColorMode::TrueColor, 0, term_cols);
            assert!(!out.contains("48;2;"));
        }
    }
    #[test]
    fn none_mode_with_paint_is_byte_identical_to_the_golden() {
        let m = machine::find("c64", None).unwrap();
        assert_eq!(
            render_static(&m, &Facts::fixture(), ColorMode::None, 0, Some(80)),
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
        let out = render_static(&m, &Facts::new(), ColorMode::None, 0, None);
        assert_eq!(out, "short\n");
    }
}
