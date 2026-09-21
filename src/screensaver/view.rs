//! Drawing the screensaver. Pure: takes the model and a size, returns either the text-mode frame
//! as a full screen of rows, or the small escape that moves an already transmitted image. Nothing
//! here touches a terminal or a clock, so every frame can be snapshot tested.

use super::model::Model;

/// The smallest terminal the screensaver runs in, same floor as `bios setup`.
pub const MIN_WIDTH: usize = 80;
/// See `MIN_WIDTH`.
pub const MIN_HEIGHT: usize = 24;

/// The text mode wordmark: eleven letters, ten single spaces between them, twenty one columns
/// wide. Exact and final; see the plan.
pub const WORDMARK_TEXT: &str = "S P A R K L E B I O S";

/// The wordmark PNG's own pixel width, for working out how tall to draw it once it is scaled to
/// about sixty percent of the terminal's width.
pub const WORDMARK_PNG_WIDTH: u32 = 1618;
/// The wordmark PNG's own pixel height. See `WORDMARK_PNG_WIDTH`.
pub const WORDMARK_PNG_HEIGHT: u32 = 112;

/// The six ANSI colour numbers the plan gives for the wordmark's letters, striped across the
/// text left to right: green, yellow, bright red, red, magenta, blue. Distinct from
/// `sprinkles::STRIPE_PALETTE`, which is a different sweep used elsewhere.
const LETTER_COLOURS: [u8; 6] = [2, 3, 9, 1, 5, 4];

/// The plain SGR reset.
const RESET: &str = "\x1b[0m";

/// The SGR foreground code for ANSI colour number `n`: colours 0 to 7 are `30 + n`, 8 to 15 are
/// the bright set at `90 + (n - 8)`.
fn sgr_foreground(n: u8) -> u8 {
    if n < 8 {
        30 + n
    } else {
        90 + (n - 8)
    }
}

/// The SGR foreground code for every character of `WORDMARK_TEXT`, `None` for a space. The six
/// stripe colours cycle across the eleven letters only: the spaces that space them out are never
/// coloured, so they never eat a turn out of the cycle. Without this, half the six colours (the
/// ones that would only ever land on a space) would never be seen at all.
fn letter_styles() -> [Option<u8>; 21] {
    let mut styles = [None; 21];
    let mut letter_index = 0usize;
    for (i, ch) in WORDMARK_TEXT.chars().enumerate() {
        if ch != ' ' {
            styles[i] = Some(sgr_foreground(
                LETTER_COLOURS[letter_index % LETTER_COLOURS.len()],
            ));
            letter_index += 1;
        }
    }
    styles
}

/// The box's cell size in image mode: about sixty percent of the terminal's width, tall enough
/// to keep the wordmark PNG's own aspect ratio, assuming a terminal cell is about twice as tall
/// as it is wide (true of every monospace font this project has been tried in).
pub fn image_size(cols: u16) -> (u16, u16) {
    let width = ((cols as u32 * 60) / 100).max(1) as u16;
    let height = ((width as u32 * WORDMARK_PNG_HEIGHT) / (WORDMARK_PNG_WIDTH * 2)).max(1) as u16;
    (width, height)
}

/// The placement id every frame's move reuses. Fixed, not one per frame: the Kitty graphics
/// protocol replaces an existing placement, rather than stacking a new one on top of it, when a
/// later placement command names the same image id and the same placement id. Without naming one
/// explicitly a bare `a=p` defaults to placement 0 every time, which is the same replacement in
/// practice on a spec correct terminal, but naming it is what actually says "this is the same
/// placement, moved" rather than leaving that to a default nobody reading the escape can see.
const PLACEMENT_ID: u32 = 1;

/// The escape that moves an already transmitted Kitty image `id` to `(x, y)`, sized `cols` by
/// `rows` cells. This is the whole point of transmitting once: every frame after the first costs
/// only this, a few dozen bytes, never the image itself again. Reusing `PLACEMENT_ID` every time
/// is what makes this a move rather than a new picture stacking on the last one.
pub fn image_move(id: u32, cols: u16, rows: u16, x: u16, y: u16) -> String {
    format!("\x1b[{y};{x}H\x1b_Ga=p,i={id},p={PLACEMENT_ID},c={cols},r={rows},q=2,C=1\x1b\\")
}

/// One row of the text mode frame, `cols` cells wide, with the wordmark spliced in wherever
/// `model.x` currently places it on this row. `no_color` draws the wordmark in plain text; the
/// six stripe colours never appear.
fn row(
    model: &Model,
    row_index: i32,
    cols: usize,
    no_color: bool,
    styles: &[Option<u8>; 21],
) -> String {
    if row_index != model.y - 1 {
        return " ".repeat(cols);
    }
    let text: Vec<char> = WORDMARK_TEXT.chars().collect();
    let mut out = String::with_capacity(cols * 5);
    for col in 0..cols as i32 {
        let offset = col - (model.x - 1);
        if offset >= 0 && (offset as usize) < text.len() {
            let ch = text[offset as usize];
            match (no_color, styles[offset as usize]) {
                (true, _) | (false, None) => out.push(ch),
                (false, Some(code)) => out.push_str(&format!("\x1b[{code}m{ch}{RESET}")),
            }
        } else {
            out.push(' ');
        }
    }
    out
}

/// The whole text mode frame: `rows` lines, each exactly `cols` visible characters. Callers must
/// have already checked `cols >= MIN_WIDTH` and `rows >= MIN_HEIGHT`.
pub fn render(model: &Model, cols: usize, rows: usize, no_color: bool) -> String {
    let styles = letter_styles();
    (0..rows as i32)
        .map(|r| row(model, r, cols, no_color, &styles))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_sgr(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\x1b' {
                while i < chars.len() && chars[i] != 'm' && chars[i] != '\\' {
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

    #[test]
    fn every_row_is_exactly_the_terminal_width() {
        let model = Model::new(80, 24, 21, 1);
        let out = render(&model, 80, 24, false);
        for (i, line) in strip_sgr(&out).lines().enumerate() {
            assert_eq!(line.chars().count(), 80, "row {i}");
        }
        assert_eq!(out.lines().count(), 24);
    }

    #[test]
    fn the_wordmark_sits_on_the_models_own_row_and_column() {
        let model = Model::new(80, 24, 21, 1);
        let out = render(&model, 80, 24, false);
        let plain = strip_sgr(&out);
        let target_row = plain.lines().nth((model.y - 1) as usize).unwrap();
        assert!(target_row.starts_with(" S P A R K L E B I O S"));
    }

    #[test]
    fn no_color_draws_the_same_text_with_no_escape_sequence() {
        let model = Model::new(80, 24, 21, 1);
        let out = render(&model, 80, 24, true);
        assert!(!out.contains('\x1b'));
        assert!(out.contains(WORDMARK_TEXT));
    }

    #[test]
    fn colour_mode_stripes_the_letters_in_the_six_ansi_colours() {
        let model = Model::new(80, 24, 21, 1);
        let out = render(&model, 80, 24, false);
        // The first letter, S, is coloured with ANSI 2 (green): SGR 32.
        assert!(out.contains("\x1b[32mS\x1b[0m"));
        // The third letter, A, is the third stripe colour, ANSI 9 (bright red): SGR 91.
        assert!(out.contains("\x1b[91mA\x1b[0m"));
    }

    #[test]
    fn a_frame_the_wordmark_has_moved_off_of_is_blank_on_that_row() {
        let mut model = Model::new(80, 24, 21, 1);
        model.y = 10;
        let out = render(&model, 80, 24, false);
        let plain = strip_sgr(&out);
        assert!(!plain.lines().next().unwrap().contains('S'));
    }

    #[test]
    fn image_size_is_about_sixty_percent_of_the_terminal_width_and_preserves_the_wordmarks_shape() {
        let (w, h) = image_size(80);
        assert_eq!(w, 48);
        assert!(h >= 1);
        // A wider terminal should draw a wider (and at least as tall) box.
        let (w2, h2) = image_size(160);
        assert!(w2 > w);
        assert!(h2 >= h);
    }

    #[test]
    fn image_move_positions_and_places_the_stored_image_without_resending_it() {
        let out = image_move(7, 48, 3, 10, 5);
        assert!(
            out.starts_with("\x1b[5;10H"),
            "does not move the cursor first"
        );
        assert!(out.contains("a=p,i=7,p=1,c=48,r=3,q=2,C=1"));
        assert!(out.ends_with("\x1b\\"));
        // No image payload: nothing here is anywhere near the size of a base64 PNG.
        assert!(out.len() < 100);
    }

    #[test]
    fn image_move_always_reuses_the_same_placement_id_so_it_moves_rather_than_stacks() {
        // Two frames, two different positions: the placement id is the one thing that must not
        // change between them, since that is what tells a spec correct terminal this is the same
        // placement moved rather than a second image on top of the first.
        let first = image_move(9999, 48, 3, 10, 5);
        let second = image_move(9999, 48, 3, 40, 20);
        assert_ne!(first, second, "the two frames should move the box");
        assert!(first.contains(&format!("p={PLACEMENT_ID}")));
        assert!(second.contains(&format!("p={PLACEMENT_ID}")));
    }
}
