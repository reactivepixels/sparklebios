//! Drawing the Memory Test screen. Pure: takes the game and a size, returns the screen as a
//! string. Nothing here touches a terminal, so every pixel of it can be tested directly.

use super::model::{
    Game, Phase, BRICKS_PER_ROW, BRICK_COLOURS, BRICK_ROWS, BRICK_WIDTH, FIELD_COLS, FIELD_ROWS,
    PADDLE_WIDTH,
};

/// The screen is drawn at a fixed size and centred in anything larger, same as setup.
pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 24;

/// The frame's own width: `+` or `|`, seventy two columns, `+` or `|`.
const CONTENT_WIDTH: usize = FIELD_COLS + 2;
/// The frame plus its one column left margin, before the trailing pad out to `WIDTH`.
const FRAMED_WIDTH: usize = CONTENT_WIDTH + 1;

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

/// Wraps a seventy four column wide `core` (the frame's own content, `+---+`, `|####|`, or a
/// centred message) in its one column left margin and the trailing pad out to `WIDTH`. Safe to
/// call with a `core` that carries ANSI colour codes: the padding here is a fixed count, never a
/// measurement of `core` itself.
fn framed(core: &str) -> String {
    format!(" {core}{}", " ".repeat(WIDTH - FRAMED_WIDTH))
}

/// The whole screen, as lines that each occupy exactly `WIDTH` cells, centred inside `cols` by
/// `rows` when the terminal is bigger than the screen.
pub fn render(game: &Game, cols: usize, rows: usize) -> String {
    let body = screen(game);
    let pad_left = cols.saturating_sub(WIDTH) / 2;
    let pad_top = rows.saturating_sub(HEIGHT) / 2;
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

fn screen(game: &Game) -> Vec<String> {
    let content = content_lines(game);
    let blank_line = " ".repeat(WIDTH);
    let top_pad = (HEIGHT - content.len()) / 2;
    let bottom_pad = HEIGHT - content.len() - top_pad;
    let mut lines = Vec::with_capacity(HEIGHT);
    for _ in 0..top_pad {
        lines.push(blank_line.clone());
    }
    lines.extend(content);
    for _ in 0..bottom_pad {
        lines.push(blank_line.clone());
    }
    lines
}

/// The fourteen lines of the game itself: the header, the frame, the ten field rows, and the
/// footer, which becomes the result line once the game has ended.
fn content_lines(game: &Game) -> Vec<String> {
    let mut lines = Vec::with_capacity(2 + FIELD_ROWS + 2);
    lines.push(framed(&header_core(game)));
    lines.push(framed(&border_core()));
    for row in 0..FIELD_ROWS {
        lines.push(framed(&field_core(game, row)));
    }
    let bottom = if matches!(game.phase, Phase::Result { .. }) && game.is_new_record() {
        framed(&centre_plain("New record.", CONTENT_WIDTH))
    } else {
        framed(&border_core())
    };
    lines.push(bottom);
    lines.push(footer_or_result_row(game));
    lines
}

fn header_core(game: &Game) -> String {
    let (left_kb, left_suffix) = match game.phase {
        Phase::Result { won: true } => (game.mem_kb, " OK"),
        Phase::FailPause { .. } | Phase::Result { won: false } => (game.remaining_kb, " FAIL"),
        Phase::Ready | Phase::Playing => (game.remaining_kb, ""),
    };
    let left = format!("Memory Testing : {left_kb}K{left_suffix}");

    let best_suffix = if game.cleared { " OK" } else { "" };
    let right = format!("Best: {}K{best_suffix}", game.best_kb);

    let gap = CONTENT_WIDTH
        .saturating_sub(left.chars().count() + right.chars().count())
        .max(1);
    fit_plain(&format!("{left}{}{right}", " ".repeat(gap)), CONTENT_WIDTH)
}

fn border_core() -> String {
    format!("+{}+", "-".repeat(FIELD_COLS))
}

/// One of the ten field rows: a brick row for the first five, otherwise blank except for the
/// paddle and, while it is in play, the ball.
fn field_core(game: &Game, row: usize) -> String {
    if row < BRICK_ROWS {
        return brick_row_core(game, row);
    }

    let mut cells = vec![' '; FIELD_COLS];
    if row == FIELD_ROWS - 1 {
        for i in 0..PADDLE_WIDTH {
            if let Some(c) = cells.get_mut(game.paddle_col + i) {
                *c = '=';
            }
        }
    }
    if matches!(game.phase, Phase::Ready | Phase::Playing) {
        let ball_row = game.ball_y.round().clamp(0.0, (FIELD_ROWS - 1) as f64) as usize;
        if ball_row == row {
            let ball_col = game.ball_x.round().clamp(0.0, (FIELD_COLS - 1) as f64) as usize;
            if let Some(c) = cells.get_mut(ball_col) {
                *c = 'o';
            }
        }
    }
    format!("|{}|", cells.into_iter().collect::<String>())
}

fn brick_row_core(game: &Game, row: usize) -> String {
    let colour = BRICK_COLOURS[row];
    let mut inner = String::with_capacity(FIELD_COLS);
    for col in 0..BRICKS_PER_ROW {
        let brick = &game.bricks[row * BRICKS_PER_ROW + col];
        if brick.alive {
            inner.push_str(&format!("\x1b[{colour}m{}\x1b[0m", "#".repeat(BRICK_WIDTH)));
        } else {
            inner.push_str(&" ".repeat(BRICK_WIDTH));
        }
    }
    format!("|{inner}|")
}

const HELP_LINE: &str = "Left/Right or A/D: move   Space: launch   Esc: give up";

fn footer_or_result_row(game: &Game) -> String {
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
    fit_plain(&format!("  {text}"), WIDTH)
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
    fn every_line_is_exactly_eighty_cells_at_the_smallest_size() {
        let g = Game::new(18874368, 18874368, true, 1);
        let out = render(&g, WIDTH, HEIGHT);
        for (i, line) in visible_lines(&out).iter().enumerate() {
            assert_eq!(line.chars().count(), WIDTH, "line {i}: {line:?}");
        }
        assert_eq!(out.lines().count(), HEIGHT);
    }

    #[test]
    fn a_bigger_terminal_centres_rather_than_stretches() {
        let g = Game::new(18874368, 18874368, true, 1);
        let out = render(&g, 120, 40);
        let lines: Vec<&str> = out.lines().collect();
        // Only the top is padded to centre vertically, the same as setup's own screen: the
        // alternate screen is already blank below, so there is nothing to fill in.
        let pad_top = (40 - HEIGHT) / 2;
        assert_eq!(lines.len(), pad_top + HEIGHT);
        let blank_top = lines.iter().take_while(|l| l.trim().is_empty()).count();
        // The screen's own fourteen line block is itself centred inside its nominal 24 rows, so
        // the blank run at the top is that inner padding as well as the outer one.
        let inner_pad_top = (HEIGHT - 14) / 2;
        assert_eq!(blank_top, pad_top + inner_pad_top);
    }

    /// The initial, freshly opened screen: matches `private/plans/2026-09-20-memory-test.md`
    /// exactly in wording and numbers. The mock-up's own hand counted spacing put the header one
    /// column past the frame's right edge and the paddle one column left of centre; both are
    /// reproduced here flush with the frame and mathematically centred instead, which the plan's
    /// own numbers (the ball sits exactly above the paddle's centre) confirm was the intent.
    #[test]
    fn the_initial_screen_matches_the_plan() {
        let g = Game::new(18874368, 18874368, true, 1);
        let out = render(&g, WIDTH, HEIGHT);
        let lines = visible_lines(&out);

        let expected_header = format!(
            " Memory Testing : 18874368K{}Best: 18874368K OK",
            " ".repeat(30)
        );
        assert_eq!(lines[5].trim_end(), expected_header);

        assert_eq!(
            lines[6].trim_end(),
            " +------------------------------------------------------------------------+"
        );
        for r in 0..BRICK_ROWS {
            let row = &lines[7 + r];
            assert_eq!(row.matches('#').count(), FIELD_COLS, "brick row {r}");
        }
        assert!(lines[14].contains('o'));
        assert!(lines[16].contains("========"));
        assert_eq!(
            lines[17].trim_end(),
            " +------------------------------------------------------------------------+"
        );
        assert!(lines[18].contains("Left/Right or A/D: move"));
        assert!(lines[18].contains("Space: launch"));
        assert!(lines[18].contains("Esc: give up"));
    }

    #[test]
    fn the_header_counts_down_as_bricks_go() {
        let mut g = Game::new(600, 0, false, 1);
        g.remaining_kb -= 10;
        let out = render(&g, WIDTH, HEIGHT);
        assert!(out.contains("Memory Testing : 590K"));
        assert!(!out.contains("OK"));
        assert!(!out.contains("FAIL"));
    }

    #[test]
    fn the_header_shows_ok_once_the_game_is_won() {
        let mut g = Game::new(60, 0, false, 1);
        for b in &mut g.bricks {
            b.alive = false;
        }
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        g.ball_x = 70.0;
        g.ball_y = 4.0;
        g.ball_dx = 0.0;
        g.ball_dy = 0.0;
        assert_eq!(g.tick(), crate::setup::memtest::model::Effect::Ended);
        let out = render(&g, WIDTH, HEIGHT);
        assert!(out.contains("Memory Testing : 60K OK"));
        assert!(out.contains("All memory tested. It was fine the whole time. Any key."));
    }

    #[test]
    fn the_header_shows_fail_after_the_last_ball_is_lost() {
        let mut g = Game::new(600, 0, false, 1);
        g.balls_left = 1;
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0;
        g.ball_y = 8.6;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        g.tick();
        assert!(matches!(g.phase, Phase::FailPause { .. }));
        let out = render(&g, WIDTH, HEIGHT);
        assert!(out.contains(&format!("Memory Testing : {}K FAIL", g.remaining_kb)));
        assert!(
            out.contains("Left/Right or A/D: move"),
            "the fail pause keeps the key help"
        );
    }

    #[test]
    fn the_lost_result_screen_shows_the_remaining_k_and_any_key() {
        let mut g = Game::new(600, 0, false, 1);
        g.balls_left = 1;
        g.phase = Phase::Playing;
        g.paddle_col = 32;
        g.ball_x = 10.0;
        g.ball_y = 8.6;
        g.ball_dx = 0.0;
        g.ball_dy = 0.5;
        g.tick();
        for _ in 0..30 {
            g.tick();
        }
        assert_eq!(g.phase, Phase::Result { won: false });
        let out = render(&g, WIDTH, HEIGHT);
        assert!(out.contains(&format!(
            "{}K untested. The BIOS will assume the best. Any key.",
            g.remaining_kb
        )));
    }

    #[test]
    fn a_new_record_prints_above_the_result_line() {
        let mut g = Game::new(60, 0, false, 1);
        for b in &mut g.bricks {
            b.alive = false;
        }
        g.bricks[TOTAL_BRICKS - 1].alive = true;
        g.remaining_kb = g.bricks[TOTAL_BRICKS - 1].kb;
        g.phase = Phase::Playing;
        g.ball_x = 70.0;
        g.ball_y = 4.0;
        g.ball_dx = 0.0;
        g.ball_dy = 0.0;
        g.tick();
        assert!(g.is_new_record());
        let out = render(&g, WIDTH, HEIGHT);
        let lines: Vec<&str> = out.lines().collect();
        let result_line = lines
            .iter()
            .position(|l| l.contains("All memory tested"))
            .unwrap();
        assert!(lines[result_line - 1].contains("New record."));
    }

    #[test]
    fn no_line_is_ever_wider_than_the_terminal() {
        let mut g = Game::new(37748736, 0, false, 7);
        g.key(Key::Space);
        for _ in 0..200 {
            g.tick();
            let out = render(&g, WIDTH, HEIGHT);
            for line in out.lines() {
                assert!(strip_sgr(line).chars().count() <= WIDTH);
            }
        }
    }

    /// The never-flash rule: consecutive frames must not change more than a quarter of the
    /// cells.
    #[test]
    fn consecutive_frames_change_fewer_than_a_quarter_of_the_cells() {
        let mut g = Game::new(37748736, 0, false, 7);
        g.key(Key::Space);
        let mut previous = visible_lines(&render(&g, WIDTH, HEIGHT));
        let total_cells = WIDTH * HEIGHT;
        for _ in 0..200 {
            g.tick();
            let current = visible_lines(&render(&g, WIDTH, HEIGHT));
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
}
