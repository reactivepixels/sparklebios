//! Drawing the Memory Test screen. Pure: takes the game and a size, returns the screen as a
//! string. Nothing here touches a terminal, so every pixel of it can be tested directly.

use super::model::{
    brick_widths, Game, Phase, BRICKS_PER_ROW, BRICK_COLOURS, BRICK_ROWS, FIELD_HEIGHT_MARGIN,
    FIELD_WIDTH_MARGIN, PADDLE_WIDTH,
};

/// The smallest terminal `setup::run` ever lets this screen see, and so the default size the
/// tests below build a game and render at. The field itself is not this size: it fills whatever
/// terminal `Game::new` was actually given, which grows the field in both directions on a bigger
/// one.
#[cfg(test)]
const MIN_COLS: usize = 80;
#[cfg(test)]
const MIN_ROWS: usize = 24;

/// Cuts `s` to `width` visible characters and pads it out to exactly that. `s` must not contain
/// any escape sequence: this measures with `chars().count()`, which an escape code would throw
/// off.
fn fit_plain(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.push_str(&" ".repeat(width - len));
    }
    out
}

fn centre_plain(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return fit_plain(s, width);
    }
    let left = (width - len) / 2;
    format!(
        "{}{}{}",
        " ".repeat(left),
        s,
        " ".repeat(width - len - left)
    )
}

/// Wraps `core`, the frame's own content (`+---+`, `|####|`, or a centred message), in the one
/// column margin that sits on each side of it. Safe to call with a `core` that carries ANSI
/// colour codes: the margin here is a fixed two characters, never a measurement of `core` itself.
fn framed(core: &str) -> String {
    format!(" {core} ")
}

/// The whole screen, as lines that each occupy exactly as many cells as the field this `game` was
/// built for needs, centred inside `cols` by `rows` on the rare chance the terminal has grown
/// since. Ordinarily the two already match, since `Game::new` was handed the same size. Under
/// `no_color` the bricks draw as plain `#` with no SGR at all, rather than their rainbow colours;
/// see `crate::render::color_mode_from_env`, the decision the caller makes `no_color` from.
pub fn render(game: &Game, cols: usize, rows: usize, no_color: bool) -> String {
    let width = game.field_cols + FIELD_WIDTH_MARGIN;
    let height = game.field_rows + FIELD_HEIGHT_MARGIN;
    let body = screen(game, width, height, no_color);
    let pad_left = cols.saturating_sub(width) / 2;
    let pad_top = rows.saturating_sub(height) / 2;
    let indent = " ".repeat(pad_left);
    let mut out = String::new();
    for _ in 0..pad_top {
        out.push('\n');
    }
    // Joined, never terminated: a trailing newline scrolls a terminal exactly as tall as the
    // screen, the same reason setup's own render avoids it.
    for (i, line) in body.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&indent);
        out.push_str(line);
    }
    out
}

fn screen(game: &Game, width: usize, height: usize, no_color: bool) -> Vec<String> {
    let content = content_lines(game, no_color);
    let blank_line = " ".repeat(width);
    let top_pad = (height - content.len()) / 2;
    let bottom_pad = height - content.len() - top_pad;
    let mut lines = Vec::with_capacity(height);
    for _ in 0..top_pad {
        lines.push(blank_line.clone());
    }
    lines.extend(content);
    for _ in 0..bottom_pad {
        lines.push(blank_line.clone());
    }
    lines
}

/// The header, the frame, the field's own rows, and the footer, which becomes the result line
/// once the game has ended. `field_rows + 4` lines long, whatever the field's height.
fn content_lines(game: &Game, no_color: bool) -> Vec<String> {
    let content_width = game.field_cols + 2;
    let mut lines = Vec::with_capacity(2 + game.field_rows + 2);
    lines.push(framed(&header_core(game, content_width)));
    lines.push(framed(&border_core(game.field_cols)));
    for row in 0..game.field_rows {
        lines.push(framed(&field_core(game, row, no_color)));
    }
    let bottom = if matches!(game.phase, Phase::Result { .. }) && game.is_new_record() {
        framed(&centre_plain("New record.", content_width))
    } else {
        framed(&border_core(game.field_cols))
    };
    lines.push(bottom);
    lines.push(footer_or_result_row(game, content_width + 2));
    lines
}

fn header_core(game: &Game, content_width: usize) -> String {
    let (left_kb, left_suffix) = match game.phase {
        Phase::Result { won: true } => (game.mem_kb, " OK"),
        Phase::FailPause { .. } | Phase::Result { won: false } => (game.remaining_kb, " FAIL"),
        Phase::Ready | Phase::Playing => (game.remaining_kb, ""),
    };
    let left = format!("Memory Testing : {left_kb}K{left_suffix}");

    let best_suffix = if game.cleared { " OK" } else { "" };
    let right = format!("Best: {}K{best_suffix}", game.best_kb);

    let gap = content_width
        .saturating_sub(left.chars().count() + right.chars().count())
        .max(1);
    fit_plain(&format!("{left}{}{right}", " ".repeat(gap)), content_width)
}

fn border_core(field_cols: usize) -> String {
    format!("+{}+", "-".repeat(field_cols))
}

/// One of the field's rows: a brick row for the first five, otherwise blank except for the
/// paddle and, while it is in play, the ball.
fn field_core(game: &Game, row: usize, no_color: bool) -> String {
    if row < BRICK_ROWS {
        return brick_row_core(game, row, no_color);
    }

    let mut cells = vec![' '; game.field_cols];
    if row == game.field_rows - 1 {
        for i in 0..PADDLE_WIDTH {
            if let Some(c) = cells.get_mut(game.paddle_col + i) {
                *c = '=';
            }
        }
    }
    if matches!(game.phase, Phase::Ready | Phase::Playing) {
        let ball_row = game.ball_y.round().clamp(0.0, (game.field_rows - 1) as f64) as usize;
        if ball_row == row {
            let ball_col = game.ball_x.round().clamp(0.0, (game.field_cols - 1) as f64) as usize;
            if let Some(c) = cells.get_mut(ball_col) {
                *c = 'o';
            }
        }
    }
    format!("|{}|", cells.into_iter().collect::<String>())
}

fn brick_row_core(game: &Game, row: usize, no_color: bool) -> String {
    let widths = brick_widths(game.field_cols);
    let mut inner = String::with_capacity(game.field_cols);
    for (col, &width) in widths.iter().enumerate() {
        let brick = &game.bricks[row * BRICKS_PER_ROW + col];
        if brick.alive {
            if no_color {
                inner.push_str(&"#".repeat(width));
            } else {
                let colour = BRICK_COLOURS[row];
                inner.push_str(&format!("\x1b[{colour}m{}\x1b[0m", "#".repeat(width)));
            }
        } else {
            inner.push_str(&" ".repeat(width));
        }
    }
    format!("|{inner}|")
}

const HELP_LINE: &str = "Left/Right or A/D: move   Space: launch   Esc: give up";

fn footer_or_result_row(game: &Game, width: usize) -> String {
    let text = match game.phase {
        Phase::Result { won: true } => {
            "All memory tested. It was fine the whole time. Any key.".to_string()
        }
        Phase::Result { won: false } => format!(
            "{}K untested. The BIOS will assume the best. Any key.",
            game.remaining_kb
        ),
        Phase::Ready | Phase::Playing | Phase::FailPause { .. } => HELP_LINE.to_string(),
    };
    fit_plain(&format!("  {text}"), width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::memtest::model::{Key, TOTAL_BRICKS};

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

    fn visible_lines(out: &str) -> Vec<String> {
        out.lines().map(strip_sgr).collect()
    }

    #[test]
    fn every_line_is_exactly_as_wide_as_the_terminal_at_the_smallest_size() {
        let g = Game::new(
            18874368,
            18874368,
            true,
            1,
            MIN_COLS as u16,
            MIN_ROWS as u16,
        );
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        for (i, line) in visible_lines(&out).iter().enumerate() {
            assert_eq!(line.chars().count(), MIN_COLS, "line {i}: {line:?}");
        }
        assert_eq!(out.lines().count(), MIN_ROWS);
    }

    #[test]
    fn a_taller_wider_terminal_grows_the_field_rather_than_stretching_a_fixed_one() {
        let g = Game::new(18874368, 18874368, true, 1, 120, 40);
        let out = render(&g, 120, 40, false);
        let lines = visible_lines(&out);
        assert_eq!(lines.len(), 40);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(line.chars().count(), 120, "line {i}: {line:?}");
        }
        // The field fills almost the whole terminal: five brick rows plus thirty of open space
        // above a bat sitting one row from the very bottom.
        assert_eq!(g.field_cols, 116);
        assert_eq!(g.field_rows, 35);
    }

    #[test]
    fn a_terminal_grown_since_the_game_was_built_centres_the_field_rather_than_stretching_it() {
        let g = Game::new(
            18874368,
            18874368,
            true,
            1,
            MIN_COLS as u16,
            MIN_ROWS as u16,
        );
        let out = render(&g, 120, 40, false);
        let lines: Vec<&str> = out.lines().collect();
        let pad_top = (40 - MIN_ROWS) / 2;
        assert_eq!(lines.len(), pad_top + MIN_ROWS);
        let blank_top = lines.iter().take_while(|l| l.trim().is_empty()).count();
        // The screen's own content is itself centred inside the field's nominal height, so the
        // blank run at the top is that inner padding as well as the outer one.
        let inner_pad_top = (MIN_ROWS - (g.field_rows + 4)) / 2;
        assert_eq!(blank_top, pad_top + inner_pad_top);
    }

    /// The initial, freshly opened screen at the smallest terminal setup allows: matches
    /// `private/plans/2026-09-20-memory-test.md` in wording, and the feel fix's own numbers in
    /// geometry (a taller, wider field; a ten wide bat).
    #[test]
    fn the_initial_screen_matches_the_plan() {
        let g = Game::new(
            18874368,
            18874368,
            true,
            1,
            MIN_COLS as u16,
            MIN_ROWS as u16,
        );
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        let lines = visible_lines(&out);

        let expected_header = format!(
            " Memory Testing : 18874368K{}Best: 18874368K OK ",
            " ".repeat(
                g.field_cols + 2 - "Memory Testing : 18874368K".len() - "Best: 18874368K OK".len()
            )
        );
        assert_eq!(lines[0].trim_end(), expected_header.trim_end());

        let border = format!(" +{}+ ", "-".repeat(g.field_cols));
        assert_eq!(lines[1].trim_end(), border.trim_end());
        for r in 0..BRICK_ROWS {
            let row = &lines[2 + r];
            assert_eq!(row.matches('#').count(), g.field_cols, "brick row {r}");
        }
        let ball_line = 2 + g.field_rows - 1 - 2;
        let paddle_line = 2 + g.field_rows - 1;
        assert!(lines[ball_line].contains('o'));
        assert!(lines[paddle_line].contains(&"=".repeat(PADDLE_WIDTH)));
        assert_eq!(lines[2 + g.field_rows].trim_end(), border.trim_end());
        let footer = &lines[2 + g.field_rows + 1];
        assert!(footer.contains("Left/Right or A/D: move"));
        assert!(footer.contains("Space: launch"));
        assert!(footer.contains("Esc: give up"));
    }

    #[test]
    fn the_header_counts_down_as_bricks_go() {
        let mut g = Game::new(600, 0, false, 1, MIN_COLS as u16, MIN_ROWS as u16);
        g.remaining_kb -= 10;
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        assert!(out.contains("Memory Testing : 590K"));
        assert!(!out.contains("OK"));
        assert!(!out.contains("FAIL"));
    }

    #[test]
    fn the_header_shows_ok_once_the_game_is_won() {
        let mut g = Game::new(60, 0, false, 1, MIN_COLS as u16, MIN_ROWS as u16);
        for b in &mut g.bricks {
            b.alive = false;
        }
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        let widths = brick_widths(g.field_cols);
        let start: usize = widths[..BRICKS_PER_ROW - 1].iter().sum();
        g.ball_x = (start + widths[BRICKS_PER_ROW - 1] / 2) as f64;
        g.ball_y = (BRICK_ROWS - 1) as f64;
        g.ball_dx = 0.0;
        g.ball_dy = 0.0;
        assert_eq!(g.tick(), crate::setup::memtest::model::Effect::Ended);
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        assert!(out.contains("Memory Testing : 60K OK"));
        assert!(out.contains("All memory tested. It was fine the whole time. Any key."));
    }

    #[test]
    fn the_header_shows_fail_after_the_last_ball_is_lost() {
        let mut g = Game::new(600, 0, false, 1, MIN_COLS as u16, MIN_ROWS as u16);
        g.balls_left = 1;
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0;
        g.ball_y = (g.field_rows - 1) as f64 - 0.4;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        g.tick();
        assert!(matches!(g.phase, Phase::FailPause { .. }));
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        assert!(out.contains(&format!("Memory Testing : {}K FAIL", g.remaining_kb)));
        assert!(
            out.contains("Left/Right or A/D: move"),
            "the fail pause keeps the key help"
        );
    }

    #[test]
    fn the_lost_result_screen_shows_the_remaining_k_and_any_key() {
        let mut g = Game::new(600, 0, false, 1, MIN_COLS as u16, MIN_ROWS as u16);
        g.balls_left = 1;
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0;
        g.ball_y = (g.field_rows - 1) as f64 - 0.4;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        g.tick();
        for _ in 0..30 {
            g.tick();
        }
        assert_eq!(g.phase, Phase::Result { won: false });
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        assert!(out.contains(&format!(
            "{}K untested. The BIOS will assume the best. Any key.",
            g.remaining_kb
        )));
    }

    #[test]
    fn a_new_record_prints_above_the_result_line() {
        let mut g = Game::new(60, 0, false, 1, MIN_COLS as u16, MIN_ROWS as u16);
        for b in &mut g.bricks {
            b.alive = false;
        }
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        let widths = brick_widths(g.field_cols);
        let start: usize = widths[..BRICKS_PER_ROW - 1].iter().sum();
        g.ball_x = (start + widths[BRICKS_PER_ROW - 1] / 2) as f64;
        g.ball_y = (BRICK_ROWS - 1) as f64;
        g.ball_dx = 0.0;
        g.ball_dy = 0.0;
        g.tick();
        assert!(g.is_new_record());
        let out = render(&g, MIN_COLS, MIN_ROWS, false);
        let lines: Vec<&str> = out.lines().collect();
        let result_line = lines
            .iter()
            .position(|l| l.contains("All memory tested"))
            .unwrap();
        assert!(lines[result_line - 1].contains("New record."));
    }

    #[test]
    fn no_line_is_ever_wider_than_the_terminal() {
        let mut g = Game::new(37748736, 0, false, 7, MIN_COLS as u16, MIN_ROWS as u16);
        g.key(Key::Space);
        for _ in 0..200 {
            g.tick();
            let out = render(&g, MIN_COLS, MIN_ROWS, false);
            for line in out.lines() {
                assert!(strip_sgr(line).chars().count() <= MIN_COLS);
            }
        }
    }

    /// The never-flash rule: consecutive frames must not change more than a quarter of the
    /// cells.
    #[test]
    fn consecutive_frames_change_fewer_than_a_quarter_of_the_cells() {
        let mut g = Game::new(37748736, 0, false, 7, MIN_COLS as u16, MIN_ROWS as u16);
        g.key(Key::Space);
        let mut previous = visible_lines(&render(&g, MIN_COLS, MIN_ROWS, false));
        let total_cells = MIN_COLS * MIN_ROWS;
        for _ in 0..200 {
            g.tick();
            let current = visible_lines(&render(&g, MIN_COLS, MIN_ROWS, false));
            let changed: usize = previous
                .iter()
                .zip(current.iter())
                .map(|(a, b)| a.chars().zip(b.chars()).filter(|(x, y)| x != y).count())
                .sum();
            assert!(
                changed * 4 < total_cells,
                "a frame changed {changed} of {total_cells} cells"
            );
            previous = current;
        }
    }

    #[test]
    fn no_color_emits_no_escape_sequence_at_any_size() {
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 50)] {
            let g = Game::new(18874368, 18874368, true, 1, cols, rows);
            let out = render(&g, cols as usize, rows as usize, true);
            assert!(
                !out.contains('\x1b'),
                "{cols}x{rows}: NO_COLOR output should carry no SGR sequence at all"
            );
        }
    }

    #[test]
    fn no_color_still_fills_every_line_to_the_terminal_width_at_any_size() {
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 50)] {
            let g = Game::new(18874368, 18874368, true, 1, cols, rows);
            let out = render(&g, cols as usize, rows as usize, true);
            for (i, line) in out.lines().enumerate() {
                assert_eq!(
                    line.chars().count(),
                    cols as usize,
                    "{cols}x{rows} line {i}: {line:?}"
                );
            }
            assert_eq!(out.lines().count(), rows as usize, "{cols}x{rows}");
        }
    }

    #[test]
    fn no_color_still_draws_the_bricks_as_plain_hashes() {
        let g = Game::new(
            18874368,
            18874368,
            true,
            1,
            MIN_COLS as u16,
            MIN_ROWS as u16,
        );
        let out = render(&g, MIN_COLS, MIN_ROWS, true);
        let lines: Vec<&str> = out.lines().collect();
        for r in 0..BRICK_ROWS {
            let row = lines[2 + r];
            assert_eq!(row.matches('#').count(), g.field_cols, "brick row {r}");
        }
    }
}
