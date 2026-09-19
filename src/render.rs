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

/// One line from a resolved quip, starting at `seed % quips.len()` and wrapping around.
fn resolve_quip(quips: &[String], facts: &Facts, seed: u64) -> Option<String> {
    if quips.is_empty() {
        return None;
    }
    let start = (seed % quips.len() as u64) as usize;
    (0..quips.len()).find_map(|offset| {
        let idx = (start + offset) % quips.len();
        template::render(&quips[idx], facts)
    })
}

/// A step resolved to its output, ready for colouring. None means the line is omitted.
enum RenderedLine {
    /// A whole line painted in one style.
    Plain(String, Style),
    /// A label (normal colour) followed by a result (styled).
    Detect(String, String, Style),
}

/// Resolves a single step to its output, or None to omit the line.
fn render_step(machine: &Machine, facts: &Facts, seed: u64, step: &Step) -> Option<RenderedLine> {
    match step {
        Step::Print { text, style, .. } => {
            Some(RenderedLine::Plain(template::render(text, facts)?, *style))
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
            Some(RenderedLine::Plain(line, Style::Normal))
        }
        Step::Detect {
            label,
            result,
            style,
            ..
        } => {
            let result = template::render(result, facts)?;
            Some(RenderedLine::Detect(format!("{label}... "), result, *style))
        }
        Step::Quip { style, .. } => {
            let quip = resolve_quip(&machine.quips, facts, seed)?;
            Some(RenderedLine::Plain(quip, *style))
        }
    }
}

/// The finished screen as text. Every line ends with '\n'.
/// `seed` picks the quip: start at `seed % quips.len()`.
pub fn render_static(machine: &Machine, facts: &Facts, mode: ColorMode, seed: u64) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(machine.steps.len());
    for step in &machine.steps {
        match render_step(machine, facts, seed, step) {
            Some(RenderedLine::Plain(text, _)) if text.is_empty() => {
                lines.push(String::new());
            }
            Some(RenderedLine::Plain(text, style)) => {
                lines.push(paint(machine, mode, style, &text));
            }
            Some(RenderedLine::Detect(label, result, style)) => {
                let painted = format!(
                    "{}{}",
                    paint(machine, mode, Style::Normal, &label),
                    paint(machine, mode, style, &result)
                );
                lines.push(painted);
            }
            None => {}
        }
    }

    // Never emit two blank lines in a row, and never start or end with a blank line.
    let mut collapsed: Vec<String> = Vec::with_capacity(lines.len());
    for line in lines {
        let previous_is_blank = match collapsed.last() {
            Some(l) => l.is_empty(),
            None => true,
        };
        if line.is_empty() && previous_is_blank {
            continue;
        }
        collapsed.push(line);
    }
    while collapsed.last().is_some_and(|l| l.is_empty()) {
        collapsed.pop();
    }

    let mut out = String::new();
    for line in collapsed {
        out.push_str(&line);
        out.push('\n');
    }
    out
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

    #[test]
    fn pc95_matches_golden() {
        let m = machine::find("pc95", None).unwrap();
        assert_eq!(
            render_static(&m, &Facts::fixture(), ColorMode::None, 0),
            golden("pc95")
        );
    }
    #[test]
    fn pc85_matches_golden() {
        let m = machine::find("pc85", None).unwrap();
        assert_eq!(
            render_static(&m, &Facts::fixture(), ColorMode::None, 0),
            golden("pc85")
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
        let out = render_static(&m, &f, ColorMode::None, 0);
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
            let out = render_static(&m, &Facts::new(), ColorMode::None, 0);
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
        let out = render_static(&m, &Facts::fixture(), ColorMode::TrueColor, 0);
        assert!(out.contains("\x1b[38;2;255;255;255m     Sparkle Modular BIOS"));
        assert!(out.contains(
            "\x1b[38;2;170;170;170mDetecting Horn             ... \x1b[0m\x1b[38;2;255;255;85m1 found"
        ));
        assert!(!out.contains("\x1b[1m"));
    }
    #[test]
    fn ansi16_uses_basic_codes() {
        let m = machine::find("pc85", None).unwrap();
        let out = render_static(&m, &Facts::fixture(), ColorMode::Ansi16, 0);
        assert!(out.contains("\x1b[37m37748736K OK\x1b[0m"));
    }
    #[test]
    fn seed_rotates_quips_and_skips_unresolvable_ones() {
        let m = machine::find("pc95", None).unwrap();
        let out1 = render_static(&m, &Facts::fixture(), ColorMode::None, 1);
        assert!(out1.contains("Plug and Pray devices found: 1 unicorn"));
        // quip 7 needs {mem.kb}; without it the next resolvable quip is used
        let mut f = Facts::new();
        f.insert("date.year", "2026");
        let out7 = render_static(&m, &f, ColorMode::None, 7);
        assert!(out7.contains("Floppy drive A: not found. Nobody is surprised."));
        let wrapped = render_static(&m, &Facts::fixture(), ColorMode::None, m.quips.len() as u64);
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
}
