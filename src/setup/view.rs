//! Drawing the setup screen. Pure: takes the state and a size, returns the screen as a string.
//! Nothing here touches a terminal, so every pixel of it can be snapshot tested.

use super::model::{Dialog, State};

/// The screen is drawn at a fixed size and centred in anything larger, the way a real setup
/// utility sat in the middle of whatever monitor you had.
pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 24;

/// Inside the frame: the settings pane, then the help pane.
const LEFT: usize = 48;
const RIGHT: usize = 29;

// The screen uses ANSI colours only, never truecolor, so it looks the same under every theme.
const BG: &str = "\x1b[44m";
const TEXT: &str = "\x1b[44;97m";
const FRAME: &str = "\x1b[44;36m";
const SELECTED: &str = "\x1b[30;47m";
const CHANGED: &str = "\x1b[44;93m";
const HELP: &str = "\x1b[44;37m";
const DIALOG: &str = "\x1b[41;97m";
const RESET: &str = "\x1b[0m";

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

/// The whole screen, as lines that each occupy exactly `WIDTH` cells, centred inside `cols` by
/// `rows` when the terminal is bigger than the screen.
pub fn render(state: &State, cols: usize, rows: usize, year: &str) -> String {
    let body = screen(state, year);
    let pad_left = cols.saturating_sub(WIDTH) / 2;
    let pad_top = rows.saturating_sub(HEIGHT) / 2;
    let indent = " ".repeat(pad_left);
    let mut out = String::new();
    for _ in 0..pad_top {
        out.push('\n');
    }
    for line in body {
        out.push_str(&indent);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn screen(state: &State, year: &str) -> Vec<String> {
    let mut lines = Vec::with_capacity(HEIGHT);

    lines.push(format!(
        "{TEXT}{}{RESET}",
        centre("SparkleBIOS CMOS Setup Utility", WIDTH)
    ));
    lines.push(format!(
        "{FRAME}{}{RESET}",
        centre(
            &format!("Copyright (C) 1985-{year}, Rainbows & Unicorns, Inc."),
            WIDTH
        )
    ));

    lines.push(rule('\u{2554}', '\u{2566}', '\u{2557}'));

    let help_lines = wrap(state.current().help, RIGHT - 2);
    // The panes are the same height, so the taller of the two decides it.
    let pane_rows = HEIGHT - 5;
    for i in 0..pane_rows {
        lines.push(pane_row(state, i, &help_lines));
    }

    lines.push(rule('\u{255a}', '\u{2569}', '\u{255d}'));
    lines.push(format!(
        "{HELP}{}{RESET}",
        fit(
            "  Up/Down: Select   Left/Right: Change   Enter: Preview   F10: Save   Esc: Exit",
            WIDTH
        )
    ));

    if state.dialog != Dialog::None {
        overlay_dialog(&mut lines, state.dialog);
    }
    lines
}

fn rule(left: char, middle: char, right: char) -> String {
    format!(
        "{FRAME}{left}{}{middle}{}{right}{RESET}",
        "\u{2550}".repeat(LEFT),
        "\u{2550}".repeat(RIGHT)
    )
}

/// One row inside the frame: a setting on the left, a line of help on the right.
fn pane_row(state: &State, index: usize, help_lines: &[String]) -> String {
    let left = match state.rows.get(index) {
        Some(row) => {
            let label = fit(&format!("  {}", row.label), 24);
            let value = format!("[{}]", row.value());
            let body = fit(&format!("{label}{value}"), LEFT);
            if index == state.selected {
                format!("{SELECTED}{body}{RESET}")
            } else if row.changed() {
                format!("{CHANGED}{body}{RESET}")
            } else {
                format!("{TEXT}{body}{RESET}")
            }
        }
        None => format!("{BG}{}{RESET}", " ".repeat(LEFT)),
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
        format!("{FRAME}{}{RESET}", fit(&right_text, RIGHT))
    } else {
        format!("{HELP}{}{RESET}", fit(&right_text, RIGHT))
    };

    format!("{FRAME}\u{2551}{RESET}{left}{FRAME}\u{2551}{RESET}{right}{FRAME}\u{2551}{RESET}")
}

/// The question, centred over the screen in a red box.
fn overlay_dialog(lines: &mut [String], dialog: Dialog) {
    let text = match dialog {
        Dialog::Save => "SAVE to CMOS and EXIT (Y/N)? Y",
        Dialog::Quit => "Quit Without Saving (Y/N)? N",
        Dialog::None => return,
    };
    let inner = text.chars().count() + 4;
    let box_left = (WIDTH - inner) / 2;
    let top = HEIGHT / 2 - 1;
    let blank = format!("{DIALOG}{}{RESET}", " ".repeat(inner));
    let body = format!("{DIALOG}  {text}  {RESET}");
    for (offset, content) in [blank.clone(), body, blank].into_iter().enumerate() {
        if let Some(line) = lines.get_mut(top + offset) {
            *line = overlay(line, &content, box_left);
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
fn overlay(line: &str, patch: &str, at: usize) -> String {
    let visible: Vec<char> = strip_sgr(line).chars().collect();
    let patch_width = strip_sgr(patch).chars().count();
    let last = visible.len().saturating_sub(1);
    let before: String = visible.iter().take(at).skip(1).collect();
    let after: String = visible.iter().take(last).skip(at + patch_width).collect();
    let left_pillar = visible.first().copied().unwrap_or(' ');
    let right_pillar = visible.get(last).copied().unwrap_or(' ');
    format!(
        "{FRAME}{left_pillar}{RESET}{TEXT}{before}{RESET}{patch}{TEXT}{after}{RESET}{FRAME}{right_pillar}{RESET}"
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
            row("Turbo", Setting::Turbo, &["On", "Off"], "Does nothing. It never did. This setting is not saved, in keeping with tradition."),
        ])
    }

    fn visible_lines(out: &str) -> Vec<String> {
        out.lines().map(strip_sgr).collect()
    }

    #[test]
    fn every_line_is_exactly_eighty_cells_at_the_smallest_size() {
        let out = render(&state(), WIDTH, HEIGHT, "2026");
        for (i, line) in visible_lines(&out).iter().enumerate() {
            assert_eq!(line.chars().count(), WIDTH, "line {i}: {line:?}");
        }
    }

    #[test]
    fn the_screen_is_the_full_height() {
        let out = render(&state(), WIDTH, HEIGHT, "2026");
        assert_eq!(out.lines().count(), HEIGHT);
    }

    #[test]
    fn a_bigger_terminal_centres_the_screen_rather_than_stretching_it() {
        let out = render(&state(), 120, 40, "2026");
        let lines: Vec<&str> = out.lines().collect();
        let blank_top = lines.iter().take_while(|l| l.is_empty()).count();
        assert_eq!(blank_top, (40 - HEIGHT) / 2);
        let first = strip_sgr(lines[blank_top]);
        assert_eq!(first.chars().count(), (120 - WIDTH) / 2 + WIDTH);
        assert!(first.starts_with(&" ".repeat((120 - WIDTH) / 2)));
    }

    #[test]
    fn the_selected_row_is_the_only_one_in_reverse_video() {
        let out = render(&state(), WIDTH, HEIGHT, "2026");
        assert_eq!(out.matches(SELECTED).count(), 1);
        assert!(out.contains(&format!("{SELECTED}  Flavour")));
    }

    #[test]
    fn a_changed_row_is_picked_out_in_yellow_once_it_is_not_the_selected_one() {
        let mut s = state();
        s.rows[1].selected = 1;
        s.selected = 0;
        let out = render(&s, WIDTH, HEIGHT, "2026");
        assert!(out.contains(CHANGED), "a changed row should stand out");
        assert!(out.contains("[Hidden]"));
    }

    #[test]
    fn the_help_pane_shows_the_selected_rows_text_wrapped_inside_the_pane() {
        let mut s = state();
        s.selected = 2;
        let out = render(&s, WIDTH, HEIGHT, "2026");
        assert!(out.contains("Item Help"));
        let joined = visible_lines(&out).join(" ");
        assert!(joined.contains("Does nothing. It never did."));
    }

    #[test]
    fn every_help_text_wraps_within_the_pane_and_fits_its_height() {
        let s = state();
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
                wrapped.len() <= HEIGHT - 5 - 2,
                "{:?} needs {} lines, the pane holds {}",
                row.label,
                wrapped.len(),
                HEIGHT - 5 - 2
            );
        }
    }

    #[test]
    fn the_save_dialog_sits_over_the_screen_without_changing_its_shape() {
        let mut s = state();
        s.dialog = Dialog::Save;
        let out = render(&s, WIDTH, HEIGHT, "2026");
        for line in visible_lines(&out) {
            assert_eq!(line.chars().count(), WIDTH);
        }
        assert!(out.contains("SAVE to CMOS and EXIT (Y/N)? Y"));
    }

    #[test]
    fn the_quit_dialog_asks_the_other_question() {
        let mut s = state();
        s.dialog = Dialog::Quit;
        let out = render(&s, WIDTH, HEIGHT, "2026");
        assert!(out.contains("Quit Without Saving (Y/N)? N"));
        for line in visible_lines(&out) {
            assert_eq!(line.chars().count(), WIDTH);
        }
    }

    #[test]
    fn the_year_comes_from_the_caller_rather_than_the_clock() {
        let out = render(&state(), WIDTH, HEIGHT, "1999");
        assert!(out.contains("Copyright (C) 1985-1999, Rainbows & Unicorns, Inc."));
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_cuts_only_a_word_too_long_to_fit() {
        assert_eq!(wrap("a b c", 3), vec!["a b", "c"]);
        assert_eq!(wrap("", 10), Vec::<String>::new());
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
    }

    #[test]
    fn the_footer_lists_the_keys() {
        let out = render(&state(), WIDTH, HEIGHT, "2026");
        let last = strip_sgr(out.lines().last().unwrap());
        assert!(last.contains("Up/Down: Select"));
        assert!(last.contains("F10: Save"));
        assert!(last.contains("Esc: Exit"));
    }
}
