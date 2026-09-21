//! Drawing the setup screen. Pure: takes the state and a size, returns the screen as a string.
//! Nothing here touches a terminal, so every pixel of it can be snapshot tested.

use super::model::{Dialog, State};

/// The smallest terminal `bios setup` runs in. Below this, `bios setup` refuses to start; that
/// check lives in `mod.rs`. The frame itself has no size of its own: it always spans whatever
/// terminal it is given, so long as it meets this floor. See `render`.
pub const MIN_WIDTH: usize = 80;
/// The smallest terminal height `bios setup` runs in. See `MIN_WIDTH`.
pub const MIN_HEIGHT: usize = 24;

/// The help pane's fixed width, at every terminal size. The left pane takes whatever is left.
const RIGHT: usize = 29;
/// The frame's three single-cell vertical rules: the left edge, the divider between the panes,
/// and the right edge.
const RULES: usize = 3;

/// One role the screen paints in, resolved to the SGR sequence that draws it. `ANSI` is the
/// screen's original look, drawn in the terminal's own ANSI blue; `TRUECOLOR` is the fixed CGA
/// palette used once the terminal is known to render it faithfully. Every escape sequence the
/// screen draws with comes from one of the two: see `palette` and `truecolor_capable`.
#[derive(Clone, Copy)]
struct Palette {
    /// Plain background fill, with no text sitting directly on it.
    bg: &'static str,
    /// Body text: the title, and any row that is neither selected nor changed.
    text: &'static str,
    /// The frame's own rules and pillars, the copyright line, and the help pane's heading.
    frame: &'static str,
    /// The selected row, in reverse video.
    selected: &'static str,
    /// A row whose value has been changed, once it is no longer the selected one.
    changed: &'static str,
    /// The help pane's text and the footer's key list.
    help: &'static str,
    /// A dialog box.
    dialog: &'static str,
    reset: &'static str,
}

// ANSI 44 renders as whatever blue the terminal's own theme defines, which is how this screen
// always drew until `TRUECOLOR` existed alongside it. Kept exactly as it was.
const ANSI: Palette = Palette {
    bg: "\x1b[44m",
    text: "\x1b[44;97m",
    frame: "\x1b[44;36m",
    selected: "\x1b[30;47m",
    changed: "\x1b[44;93m",
    help: "\x1b[44;37m",
    dialog: "\x1b[41;97m",
    reset: "\x1b[0m",
};

// The fixed CGA colours a real setup screen drew: the same blue, cyan, white, grey, yellow and
// red on every terminal, because the whole point of a BIOS screen is that it never looks like
// anything else. Used only once the terminal is known to render truecolor correctly, rather than
// merely claiming to; see `truecolor_capable`. Each sequence pairs a foreground with a background
// in one escape, the same way `styled_text` in `render.rs` does.
const TRUECOLOR: Palette = Palette {
    bg: "\x1b[48;2;0;0;168m",                      // #0000A8
    text: "\x1b[38;2;255;255;255;48;2;0;0;168m",   // #FFFFFF on #0000A8
    frame: "\x1b[38;2;85;255;255;48;2;0;0;168m",   // #55FFFF on #0000A8
    selected: "\x1b[38;2;0;0;0;48;2;170;170;170m", // #000000 on #AAAAAA
    changed: "\x1b[38;2;255;255;85;48;2;0;0;168m", // #FFFF55 on #0000A8
    help: "\x1b[38;2;170;170;170;48;2;0;0;168m",   // #AAAAAA on #0000A8
    dialog: "\x1b[38;2;255;255;255;48;2;170;0;0m", // #FFFFFF on #AA0000
    reset: "\x1b[0m",
};

// NO_COLOR: no SGR sequence at all, every field empty, so every `format!("{style}{body}{reset}")`
// site below draws `body` and nothing else. The selected row loses its reverse video here, and
// gets the leading `>` marker `pane_row` draws instead; see `render`.
const PLAIN: Palette = Palette {
    bg: "",
    text: "",
    frame: "",
    selected: "",
    changed: "",
    help: "",
    dialog: "",
    reset: "",
};

/// Whether the terminal can be trusted to show the setup screen's own fixed colours rather than
/// whatever a theme has redefined ANSI 44 as: `COLORTERM` of exactly `truecolor`, or a `TERM`
/// naming one of the terminals known to render truecolor correctly. Anything else falls back to
/// the plain ANSI codes the screen always drew. A pure decision over the two env values, in the
/// same shape as `render::color_mode_from_env`, so it can be tested without a real terminal.
pub fn truecolor_capable(colorterm: Option<&str>, term: Option<&str>) -> bool {
    if colorterm == Some("truecolor") {
        return true;
    }
    let term = term.unwrap_or("");
    ["ghostty", "kitty", "iterm", "wezterm"]
        .iter()
        .any(|name| term.contains(name))
}

/// `no_color` wins over `truecolor`: NO_COLOR is a stronger instruction than any terminal
/// capability guess. See `crate::render::color_mode_from_env`, the decision this screen's caller
/// makes `no_color` from.
fn palette(no_color: bool, truecolor: bool) -> &'static Palette {
    if no_color {
        &PLAIN
    } else if truecolor {
        &TRUECOLOR
    } else {
        &ANSI
    }
}

/// Cuts `s` to `width` visible characters and pads it out to exactly that.
fn fit(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.push_str(&" ".repeat(width - len));
    }
    out
}

fn centre(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return fit(s, width);
    }
    let left = (width - len) / 2;
    format!(
        "{}{}{}",
        " ".repeat(left),
        s,
        " ".repeat(width - len - left)
    )
}

/// Wraps `text` to `width`, breaking on spaces, never mid-word unless a word is longer than the
/// line itself.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if candidate.chars().count() <= width {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        // A word too long for the pane is cut rather than allowed to overflow it.
        if word.chars().count() > width {
            let mut rest: &str = word;
            while rest.chars().count() > width {
                let head: String = rest.chars().take(width).collect();
                lines.push(head);
                rest = &rest[rest
                    .char_indices()
                    .nth(width)
                    .map(|(i, _)| i)
                    .unwrap_or(rest.len())..];
            }
            line = rest.to_string();
        } else {
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The whole screen, as `rows` lines each occupying exactly `cols` cells: the frame always spans
/// the terminal it is drawn into, rather than sitting as a fixed-size island inside it.
/// `truecolor` chooses the fixed CGA palette over the plain ANSI one; see `truecolor_capable`.
/// `no_color` wins over both, drawing no SGR sequence at all; see `palette`. Callers must have
/// already checked `cols >= MIN_WIDTH` and `rows >= MIN_HEIGHT`.
pub fn render(
    state: &State,
    cols: usize,
    rows: usize,
    year: &str,
    truecolor: bool,
    no_color: bool,
) -> String {
    // Joined, never terminated. A newline after the last row scrolls the whole screen up by one
    // on a terminal exactly as tall as the screen, losing the title, and again on every redraw.
    screen(
        state,
        cols,
        rows,
        year,
        palette(no_color, truecolor),
        no_color,
    )
    .join("\n")
}

fn screen(
    state: &State,
    cols: usize,
    rows: usize,
    year: &str,
    p: &Palette,
    no_color: bool,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(rows);
    let Palette {
        text,
        frame,
        help,
        reset,
        ..
    } = *p;
    let left_width = cols.saturating_sub(RULES + RIGHT);

    lines.push(format!(
        "{text}{}{reset}",
        centre("SparkleBIOS CMOS Setup Utility", cols)
    ));
    lines.push(format!(
        "{frame}{}{reset}",
        centre(
            &format!("Copyright (C) 1985-{year}, Rainbows & Unicorns, Inc."),
            cols
        )
    ));

    lines.push(rule(left_width, '\u{2554}', '\u{2566}', '\u{2557}', p));

    let help_lines = wrap(state.current().help, RIGHT - 2);
    // The panes are the same height, so the taller of the two decides it.
    let pane_rows = rows.saturating_sub(5);
    for i in 0..pane_rows {
        lines.push(pane_row(state, i, &help_lines, left_width, p, no_color));
    }

    lines.push(rule(left_width, '\u{255a}', '\u{2569}', '\u{255d}', p));
    lines.push(format!(
        "{help}{}{reset}",
        fit(
            "  Up/Down: Select   Left/Right: Change   Enter: Preview   F10: Save   Esc: Exit",
            cols
        )
    ));

    if state.dialog != Dialog::None {
        overlay_dialog(&mut lines, state.dialog, cols, rows, p);
    }
    lines
}

fn rule(left_width: usize, left: char, middle: char, right: char, p: &Palette) -> String {
    let Palette { frame, reset, .. } = *p;
    format!(
        "{frame}{left}{}{middle}{}{right}{reset}",
        "\u{2550}".repeat(left_width),
        "\u{2550}".repeat(RIGHT)
    )
}

/// One row inside the frame: a setting on the left, a line of help on the right. Under
/// `no_color`, the selected row has no reverse video to mark it, so its label's leading two
/// spaces become `> ` instead: the line stays exactly as wide, and no other row changes.
fn pane_row(
    state: &State,
    index: usize,
    help_lines: &[String],
    left_width: usize,
    p: &Palette,
    no_color: bool,
) -> String {
    let Palette {
        bg,
        text,
        selected,
        changed,
        frame,
        help,
        reset,
        ..
    } = *p;

    let left = match state.rows.get(index) {
        Some(row) => {
            let is_selected = index == state.selected;
            let marker = if no_color && is_selected { "> " } else { "  " };
            let label = fit(&format!("{marker}{}", row.label), 24);
            let value = format!("[{}]", row.value());
            let body = fit(&format!("{label}{value}"), left_width);
            if is_selected {
                format!("{selected}{body}{reset}")
            } else if row.changed() {
                format!("{changed}{body}{reset}")
            } else {
                format!("{text}{body}{reset}")
            }
        }
        None => format!("{bg}{}{reset}", " ".repeat(left_width)),
    };

    // The help pane's own heading sits on the first row, then a blank, then the text.
    let right_text = match index {
        0 => "  Item Help".to_string(),
        1 => String::new(),
        _ => help_lines
            .get(index - 2)
            .map(|l| format!("  {l}"))
            .unwrap_or_default(),
    };
    let right = if index == 0 {
        format!("{frame}{}{reset}", fit(&right_text, RIGHT))
    } else {
        format!("{help}{}{reset}", fit(&right_text, RIGHT))
    };

    format!("{frame}\u{2551}{reset}{left}{frame}\u{2551}{reset}{right}{frame}\u{2551}{reset}")
}

/// The question, centred over the screen in a red box.
fn overlay_dialog(lines: &mut [String], dialog: Dialog, cols: usize, rows: usize, p: &Palette) {
    let text = match dialog {
        Dialog::Save => "SAVE to CMOS and EXIT (Y/N)? Y",
        Dialog::Quit => "Quit Without Saving (Y/N)? N",
        Dialog::None => return,
    };
    let Palette {
        dialog: dialog_style,
        reset,
        ..
    } = *p;
    let inner = text.chars().count() + 4;
    let box_left = cols.saturating_sub(inner) / 2;
    let top = rows / 2 - 1;
    let blank = format!("{dialog_style}{}{reset}", " ".repeat(inner));
    let body = format!("{dialog_style}  {text}  {reset}");
    for (offset, content) in [blank.clone(), body, blank].into_iter().enumerate() {
        if let Some(line) = lines.get_mut(top + offset) {
            *line = overlay(line, &content, box_left, p);
        }
    }
}

/// Puts `patch` over `line` starting at visible column `at`, keeping what is either side. The
/// line is rebuilt from its visible characters, which is enough here because every line the
/// screen draws is a run of styled text with no cursor movement in it.
///
/// The frame's outermost pillar on each side keeps its own colour. Restyling the whole remainder
/// as plain text turned those two characters white, which is visible: the frame appears to break
/// wherever a dialog crosses it.
fn overlay(line: &str, patch: &str, at: usize, p: &Palette) -> String {
    let Palette {
        frame, text, reset, ..
    } = *p;
    let visible: Vec<char> = strip_sgr(line).chars().collect();
    let patch_width = strip_sgr(patch).chars().count();
    let last = visible.len().saturating_sub(1);
    let before: String = visible.iter().take(at).skip(1).collect();
    let after: String = visible.iter().take(last).skip(at + patch_width).collect();
    let left_pillar = visible.first().copied().unwrap_or(' ');
    let right_pillar = visible.get(last).copied().unwrap_or(' ');
    format!(
        "{frame}{left_pillar}{reset}{text}{before}{reset}{patch}{text}{after}{reset}{frame}{right_pillar}{reset}"
    )
}

fn strip_sgr(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\x1b' {
            while i < chars.len() && chars[i] != 'm' {
                i += 1;
            }
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::model::{Row, Setting};

    /// The sizes setup is exercised at: the floor, and two terminals larger than it.
    const SIZES: [(usize, usize); 3] = [(80, 24), (120, 40), (160, 50)];

    fn row(label: &'static str, setting: Setting, values: &[&str], help: &'static str) -> Row {
        Row {
            label,
            setting,
            values: values.iter().map(|v| v.to_string()).collect(),
            selected: 0,
            initial: 0,
            help,
        }
    }

    fn state() -> State {
        State::new(vec![
            row(
                "Flavour",
                Setting::Flavour,
                &["Unicorn", "Sumo"],
                "The mascot and its firmware. All flavours run the same checks. Only the attitude changes.",
            ),
            row("Mascot", Setting::Mascot, &["Shown", "Hidden"], "Shown draws the mascot as a real image where the terminal supports one."),
            row("Turbo", Setting::Turbo, &["On", "Off"], "Does nothing. It never did. Saved anyway, so the prompt can say 66 MHz."),
        ])
    }

    fn visible_lines(out: &str) -> Vec<String> {
        out.lines().map(strip_sgr).collect()
    }

    #[test]
    fn every_line_is_exactly_the_terminal_width_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, false);
            for (i, line) in visible_lines(&out).iter().enumerate() {
                assert_eq!(
                    line.chars().count(),
                    cols,
                    "{cols}x{rows} line {i}: {line:?}"
                );
            }
        }
    }

    /// A newline after the last row scrolls a terminal that is exactly as tall as the screen,
    /// which loses the title line and does it again on every redraw. Comparing rendered strings
    /// did not catch this: it compares the string, not what a terminal does with it.
    #[test]
    fn the_frame_does_not_end_in_a_newline_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, false);
            assert!(
                !out.ends_with('\n'),
                "{cols}x{rows}: the last row is newline terminated"
            );
            assert_eq!(
                out.matches('\n').count(),
                rows - 1,
                "{cols}x{rows}: one newline between rows and none after the last"
            );
        }
    }

    #[test]
    fn the_screen_is_the_full_height_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, false);
            assert_eq!(out.lines().count(), rows, "{cols}x{rows}");
        }
    }

    #[test]
    fn the_frame_fills_a_bigger_terminal_rather_than_sitting_as_an_island_in_it() {
        for (cols, rows) in [(120, 40), (160, 50)] {
            let out = render(&state(), cols, rows, "2026", false, false);
            let lines: Vec<&str> = out.lines().collect();
            assert!(
                strip_sgr(lines[0]).contains("SparkleBIOS"),
                "{cols}x{rows}: the title should be on row zero, not padded down into the terminal"
            );
            assert_eq!(strip_sgr(lines[0]).chars().count(), cols);
            let top_rule = lines
                .iter()
                .find(|l| strip_sgr(l).contains('\u{2554}'))
                .unwrap();
            assert!(
                strip_sgr(top_rule).ends_with('\u{2557}'),
                "{cols}x{rows}: the top rule should reach the last column"
            );
        }
    }

    #[test]
    fn the_selected_row_is_the_only_one_in_reverse_video() {
        let out = render(&state(), 80, 24, "2026", false, false);
        assert_eq!(out.matches(ANSI.selected).count(), 1);
        assert!(out.contains(&format!("{}  Flavour", ANSI.selected)));
    }

    #[test]
    fn a_changed_row_is_picked_out_in_yellow_once_it_is_not_the_selected_one() {
        let mut s = state();
        s.rows[1].selected = 1;
        s.selected = 0;
        let out = render(&s, 80, 24, "2026", false, false);
        assert!(out.contains(ANSI.changed), "a changed row should stand out");
        assert!(out.contains("[Hidden]"));
    }

    #[test]
    fn the_help_pane_shows_the_selected_rows_text_wrapped_inside_the_pane() {
        let mut s = state();
        s.selected = 2;
        let out = render(&s, 80, 24, "2026", false, false);
        assert!(out.contains("Item Help"));
        let joined = visible_lines(&out).join(" ");
        assert!(joined.contains("Does nothing. It never did."));
    }

    #[test]
    fn every_help_text_wraps_within_the_pane_and_fits_its_height_at_any_size() {
        let s = state();
        for (_, rows) in SIZES {
            let pane_rows = rows - 5;
            for row in &s.rows {
                let wrapped = wrap(row.help, RIGHT - 2);
                for line in &wrapped {
                    assert!(
                        line.chars().count() <= RIGHT - 2,
                        "{:?} does not fit the pane: {line:?}",
                        row.label
                    );
                }
                assert!(
                    wrapped.len() <= pane_rows - 2,
                    "{:?} needs {} lines, the pane holds {}",
                    row.label,
                    wrapped.len(),
                    pane_rows - 2
                );
            }
        }
    }

    #[test]
    fn item_help_wraps_within_a_narrow_pane() {
        let text = "Shown draws the mascot as a real image where the terminal supports one.";
        let wrapped = wrap(text, 10);
        assert!(wrapped.len() > 1, "a narrow pane should need several lines");
        for line in &wrapped {
            assert!(
                line.chars().count() <= 10,
                "{line:?} overflows a 10-wide pane"
            );
        }
        assert_eq!(wrapped.join(" "), text);
    }

    #[test]
    fn item_help_wraps_within_a_wide_pane() {
        let text = "Shown draws the mascot as a real image where the terminal supports one.";
        let wrapped = wrap(text, 50);
        for line in &wrapped {
            assert!(
                line.chars().count() <= 50,
                "{line:?} overflows a 50-wide pane"
            );
        }
        assert_eq!(wrapped.join(" "), text);
    }

    #[test]
    fn the_save_dialog_sits_over_the_screen_without_changing_its_shape() {
        for (cols, rows) in SIZES {
            let mut s = state();
            s.dialog = Dialog::Save;
            let out = render(&s, cols, rows, "2026", false, false);
            for line in visible_lines(&out) {
                assert_eq!(line.chars().count(), cols, "{cols}x{rows}");
            }
            assert!(out.contains("SAVE to CMOS and EXIT (Y/N)? Y"));
        }
    }

    #[test]
    fn the_quit_dialog_asks_the_other_question() {
        let mut s = state();
        s.dialog = Dialog::Quit;
        let out = render(&s, 80, 24, "2026", false, false);
        assert!(out.contains("Quit Without Saving (Y/N)? N"));
        for line in visible_lines(&out) {
            assert_eq!(line.chars().count(), 80);
        }
    }

    #[test]
    fn the_year_comes_from_the_caller_rather_than_the_clock() {
        let out = render(&state(), 80, 24, "1999", false, false);
        assert!(out.contains("Copyright (C) 1985-1999, Rainbows & Unicorns, Inc."));
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_cuts_only_a_word_too_long_to_fit() {
        assert_eq!(wrap("a b c", 3), vec!["a b", "c"]);
        assert_eq!(wrap("", 10), Vec::<String>::new());
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
    }

    #[test]
    fn the_footer_lists_the_keys_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, false);
            let last = strip_sgr(out.lines().last().unwrap());
            assert!(last.contains("Up/Down: Select"), "{cols}x{rows}");
            assert!(last.contains("F10: Save"), "{cols}x{rows}");
            assert!(last.contains("Esc: Exit"), "{cols}x{rows}");
        }
    }

    #[test]
    fn truecolor_and_non_truecolor_both_render_at_any_size() {
        for (cols, rows) in SIZES {
            for truecolor in [false, true] {
                let out = render(&state(), cols, rows, "2026", truecolor, false);
                assert_eq!(
                    out.lines().count(),
                    rows,
                    "{cols}x{rows} truecolor={truecolor}"
                );
            }
        }
    }

    #[test]
    fn truecolor_draws_the_exact_cga_background_and_the_fallback_draws_the_old_ansi_code() {
        let out_true = render(&state(), 80, 24, "2026", true, false);
        assert!(
            out_true.contains("48;2;0;0;168"),
            "truecolor output should contain the exact CGA blue background"
        );

        let out_false = render(&state(), 80, 24, "2026", false, false);
        assert!(
            out_false.contains("\x1b[44"),
            "the fallback output should still contain the old ANSI background code"
        );
        assert!(
            !out_false.contains("48;2;"),
            "the fallback output should never contain a truecolor sequence"
        );
    }

    #[test]
    fn truecolor_capable_follows_colorterm_and_the_known_terminals() {
        assert!(truecolor_capable(Some("truecolor"), None));
        assert!(truecolor_capable(None, Some("xterm-ghostty")));
        assert!(truecolor_capable(None, Some("xterm-kitty")));
        assert!(truecolor_capable(None, Some("iterm-something")));
        assert!(truecolor_capable(None, Some("wezterm")));
        assert!(!truecolor_capable(None, None));
        assert!(!truecolor_capable(Some("24bit"), Some("xterm-256color")));
        assert!(!truecolor_capable(None, Some("xterm-256color")));
    }

    #[test]
    fn no_color_emits_no_escape_sequence_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, true);
            assert!(
                !out.contains('\x1b'),
                "{cols}x{rows}: NO_COLOR output should carry no SGR sequence at all"
            );
        }
    }

    #[test]
    fn no_color_still_fills_every_line_to_the_terminal_width_at_any_size() {
        for (cols, rows) in SIZES {
            let out = render(&state(), cols, rows, "2026", false, true);
            for (i, line) in out.lines().enumerate() {
                assert_eq!(
                    line.chars().count(),
                    cols,
                    "{cols}x{rows} line {i}: {line:?}"
                );
            }
            assert_eq!(out.lines().count(), rows, "{cols}x{rows}");
        }
    }

    #[test]
    fn no_color_marks_the_selected_row_with_a_marker_and_no_other_row() {
        let mut s = state();
        s.selected = 1; // Mascot
        let out = render(&s, 80, 24, "2026", false, true);
        assert!(
            out.contains("> Mascot"),
            "the selected row should carry the > marker in place of reverse video"
        );
        assert!(!out.contains("> Flavour"));
        assert!(!out.contains("> Turbo"));
        assert_eq!(
            out.matches('>').count(),
            1,
            "no row other than the selected one should carry the marker"
        );
    }

    #[test]
    fn no_color_dialog_also_emits_no_escape_sequence() {
        let mut s = state();
        s.dialog = Dialog::Save;
        let out = render(&s, 80, 24, "2026", false, true);
        assert!(!out.contains('\x1b'));
        assert!(out.contains("SAVE to CMOS and EXIT (Y/N)? Y"));
        for line in out.lines() {
            assert_eq!(line.chars().count(), 80);
        }
    }
}
