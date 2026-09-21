//! Drawing the screensaver. Pure: takes the model and a size, returns either the text-mode frame
//! as a full screen of rows, or the small escape that moves an already transmitted image. Nothing
//! here touches a terminal or a clock, so every frame can be snapshot tested.

use super::model::{Image, Model, Twinkle};

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
///
/// `offset` rotates the starting colour: `0` is the ordinary striping, and a ripple in progress
/// (see `Model::ripple_color_offset`) passes `1` through `7`, one step further each frame, so the
/// stripe itself seems to travel along the letters while the ripple plays.
fn letter_styles(offset: usize) -> [Option<u8>; 21] {
    let mut styles = [None; 21];
    let mut letter_index = 0usize;
    for (i, ch) in WORDMARK_TEXT.chars().enumerate() {
        if ch != ' ' {
            styles[i] = Some(sgr_foreground(
                LETTER_COLOURS[(letter_index + offset) % LETTER_COLOURS.len()],
            ));
            letter_index += 1;
        }
    }
    styles
}

/// The single SGR foreground code a solid tint colours every letter, or `None` for
/// `Image::Rainbow`, which keeps the ordinary six colour stripe instead (see `tint_letter_styles`
/// and `letter_styles`). The six drawn straight from `sprinkles::STRIPE_PALETTE`, in the same
/// order `Model`'s own `TINTS` lists them, plus plain white for the one tint the stripe palette
/// does not itself carry. Ripple frames never reach this: `render` only calls it once
/// `Model::ripple_color_offset` is `None`.
fn tint_color(tint: Image) -> Option<u8> {
    match tint {
        Image::Rainbow => None,
        Image::Red => Some(crate::sprinkles::STRIPE_PALETTE[0]),
        Image::Yellow => Some(crate::sprinkles::STRIPE_PALETTE[1]),
        Image::Green => Some(crate::sprinkles::STRIPE_PALETTE[2]),
        Image::Cyan => Some(crate::sprinkles::STRIPE_PALETTE[3]),
        Image::Blue => Some(crate::sprinkles::STRIPE_PALETTE[4]),
        Image::Magenta => Some(crate::sprinkles::STRIPE_PALETTE[5]),
        Image::White => Some(37),
        Image::Ripple1
        | Image::Ripple2
        | Image::Ripple3
        | Image::Ripple4
        | Image::Ripple5
        | Image::Ripple6 => None,
    }
}

/// The letters' styling when nothing is rippling: image mode's own tint switch, drawn in text.
/// `Model::tint` is the very same decision either mode reads (see `mod.rs`'s `run_image`, which
/// draws it as a solid picture instead), so this needed no sparkle logic of its own, only a way
/// to draw a tint: the ordinary six colour stripe for `Image::Rainbow`, every letter in one
/// colour otherwise. Without this the text fallback bounced, twinkled and occasionally rippled,
/// but never did the one thing that happens on most bounces, which was exactly the "no sparkle"
/// complaint that started this for the terminals (iTerm2, WezTerm) that only ever see this path.
fn tint_letter_styles(tint: Image) -> [Option<u8>; 21] {
    match tint_color(tint) {
        None => letter_styles(0),
        Some(code) => {
            let mut styles = [None; 21];
            for (i, ch) in WORDMARK_TEXT.chars().enumerate() {
                if ch != ' ' {
                    styles[i] = Some(code);
                }
            }
            styles
        }
    }
}

/// The glyph a twinkle shows on its `frame` (`0`, `1` or `2`): a star, then a plus, then a full
/// stop. Shared by the text mode overlay in `row` and the image mode escapes below; any other
/// frame draws nothing, though `Model` itself never asks for one.
fn twinkle_glyph(frame: u8) -> Option<char> {
    const GLYPHS: [char; 3] = ['*', '+', '.'];
    GLYPHS.get(frame as usize).copied()
}

/// The box's cell size in image mode: about sixty percent of the terminal's width, tall enough
/// to keep the wordmark PNG's own aspect ratio, assuming a terminal cell is about twice as tall
/// as it is wide (true of every monospace font this project has been tried in).
pub fn image_size(cols: u16) -> (u16, u16) {
    let width = ((cols as u32 * 60) / 100).max(1) as u16;
    let height = ((width as u32 * WORDMARK_PNG_HEIGHT) / (WORDMARK_PNG_WIDTH * 2)).max(1) as u16;
    (width, height)
}

/// The placement id every frame's move reuses. Named, not left to the `a=p` default of
/// placement 0, so a reader of the escape can see this is meant as the same placement moved
/// rather than a new one. The spec says naming it is enough on its own to replace rather than
/// stack; Ghostty does not honour that (several wordmarks stayed on screen at once, in different
/// colours, when this shipped without `image_delete`), so `run_image` now deletes the previous
/// frame's image outright before every placement, and this id is closer to documentation than a
/// load bearing part of the fix.
const PLACEMENT_ID: u32 = 1;

/// The escape that moves an already transmitted Kitty image `id` to `(x, y)`, sized `cols` by
/// `rows` cells. This is the whole point of transmitting once: every frame after the first costs
/// only this, a few dozen bytes, never the image itself again.
pub fn image_move(id: u32, cols: u16, rows: u16, x: u16, y: u16) -> String {
    format!("\x1b[{y};{x}H\x1b_Ga=p,i={id},p={PLACEMENT_ID},c={cols},r={rows},q=2,C=1\x1b\\")
}

/// Deletes every placement of Kitty image `id`, wherever it currently sits (`d=i`: by image id,
/// not a placement id or a cell). `run_image` writes this immediately before every frame's
/// `image_move`, naming whichever image the previous frame actually placed: relying on
/// `image_move`'s own placement id to replace rather than stack does not hold in Ghostty (see
/// `PLACEMENT_ID`), so without this, older frames stayed on screen and the wordmark smeared
/// across several positions and colours at once. Deleting by image id, rather than assuming the
/// same image is still showing, is what still clears the old picture on a frame that also
/// switches to a different one, a ripple's own every frame.
pub fn image_delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=i,i={id}\x1b\\")
}

/// One row of the text mode frame, `cols` cells wide, with the wordmark spliced in wherever
/// `model.x` currently places it on this row, and `model.twinkle`, if one is showing, spliced in
/// over whatever would otherwise be there. `no_color` draws the wordmark in plain text; the six
/// stripe colours never appear, on the wordmark or the twinkle.
fn row(
    model: &Model,
    row_index: i32,
    cols: usize,
    no_color: bool,
    styles: &[Option<u8>; 21],
) -> String {
    let text: Vec<char> = WORDMARK_TEXT.chars().collect();
    let on_wordmark_row = row_index == model.y - 1;
    let mut out = String::with_capacity(cols * 5);
    for col in 0..cols as i32 {
        // `model.twinkle`'s cells are one indexed, like `model.x` and `model.y`; `col` and
        // `row_index` here are zero indexed, so both gain one before the comparison.
        let twinkle_here = model
            .twinkle
            .and_then(|t| {
                t.cells
                    .iter()
                    .position(|&cell| cell == (col + 1, row_index + 1))
                    .map(|i| (i, t.frame))
            })
            .and_then(|(i, frame)| twinkle_glyph(frame).map(|glyph| (i, glyph)));
        if let Some((i, glyph)) = twinkle_here {
            if no_color {
                out.push(glyph);
            } else {
                let code =
                    crate::sprinkles::STRIPE_PALETTE[i % crate::sprinkles::STRIPE_PALETTE.len()];
                out.push_str(&format!("\x1b[{code}m{glyph}{RESET}"));
            }
            continue;
        }
        if !on_wordmark_row {
            out.push(' ');
            continue;
        }
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
    let styles = match model.ripple_color_offset() {
        Some(offset) => letter_styles(offset as usize),
        None => tint_letter_styles(model.tint),
    };
    (0..rows as i32)
        .map(|r| row(model, r, cols, no_color, &styles))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One frame of a twinkle, drawn as its own small escape rather than folded into a full redraw:
/// image mode never redraws the whole screen (see `mod.rs`'s `run_image`), only moves the image
/// and now, on a bounce, lights the twinkle beside it. `None` once the twinkle has shown all its
/// frames; see `twinkle_clear_image` for what erases it then.
pub fn twinkle_frame_image(twinkle: &Twinkle) -> String {
    let Some(glyph) = twinkle_glyph(twinkle.frame) else {
        return String::new();
    };
    let mut out = String::new();
    for (i, (col, row)) in twinkle.cells.iter().enumerate() {
        let code = crate::sprinkles::STRIPE_PALETTE[i % crate::sprinkles::STRIPE_PALETTE.len()];
        out.push_str(&format!("\x1b[{row};{col}H\x1b[{code}m{glyph}{RESET}"));
    }
    out
}

/// Blanks the three cells a twinkle just finished lighting in image mode. See
/// `twinkle_frame_image`.
pub fn twinkle_clear_image(cells: [(i32, i32); 3]) -> String {
    let mut out = String::new();
    for (col, row) in cells {
        out.push_str(&format!("\x1b[{row};{col}H "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::model::Kind;
    use super::*;

    /// A text mode model, seeded arbitrarily: none of the tests in this file trigger a bounce,
    /// so the seed and the sparkle it drives never come into play.
    fn text_model(cols: i32, rows: i32) -> Model {
        Model::new(cols, rows, 21, 1, Kind::Text, 1)
    }

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
        let model = text_model(80, 24);
        let out = render(&model, 80, 24, false);
        for (i, line) in strip_sgr(&out).lines().enumerate() {
            assert_eq!(line.chars().count(), 80, "row {i}");
        }
        assert_eq!(out.lines().count(), 24);
    }

    #[test]
    fn the_wordmark_sits_on_the_models_own_row_and_column() {
        let model = text_model(80, 24);
        let out = render(&model, 80, 24, false);
        let plain = strip_sgr(&out);
        let target_row = plain.lines().nth((model.y - 1) as usize).unwrap();
        assert!(target_row.starts_with(" S P A R K L E B I O S"));
    }

    #[test]
    fn no_color_draws_the_same_text_with_no_escape_sequence() {
        let model = text_model(80, 24);
        let out = render(&model, 80, 24, true);
        assert!(!out.contains('\x1b'));
        assert!(out.contains(WORDMARK_TEXT));
    }

    #[test]
    fn colour_mode_stripes_the_letters_in_the_six_ansi_colours() {
        let model = text_model(80, 24);
        let out = render(&model, 80, 24, false);
        // The first letter, S, is coloured with ANSI 2 (green): SGR 32.
        assert!(out.contains("\x1b[32mS\x1b[0m"));
        // The third letter, A, is the third stripe colour, ANSI 9 (bright red): SGR 91.
        assert!(out.contains("\x1b[91mA\x1b[0m"));
    }

    #[test]
    fn a_frame_the_wordmark_has_moved_off_of_is_blank_on_that_row() {
        let mut model = text_model(80, 24);
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

    #[test]
    fn image_delete_names_the_image_id_and_carries_no_cursor_move() {
        let out = image_delete(9990);
        assert_eq!(out, "\x1b_Ga=d,d=i,i=9990\x1b\\");
        assert!(
            !out.contains('['),
            "a delete needs no cell, so it should carry no cursor move"
        );
    }

    #[test]
    fn a_snapshot_of_an_80_by_24_text_frame_with_a_twinkle_showing() {
        // The twinkle sits just left of the box (column 1, the box's own left edge being
        // column 2): row 1 catches it alone, row 2 catches it beside the wordmark, row 3 catches
        // it alone again, exactly as a left wall bounce's own twinkle would land.
        let mut model = text_model(80, 24);
        model.twinkle = Some(Twinkle {
            cells: [(1, 1), (1, 2), (1, 3)],
            frame: 0,
        });

        let plain = render(&model, 80, 24, true);
        let mut expected = vec![" ".repeat(80); 24];
        expected[0].replace_range(0..1, "*");
        expected[1] = format!(
            "*{WORDMARK_TEXT}{}",
            " ".repeat(80 - 1 - WORDMARK_TEXT.len())
        );
        expected[2].replace_range(0..1, "*");
        assert_eq!(plain, expected.join("\n"));

        // The same frame in colour: each of the twinkle's three cells is one of the six stripe
        // colours, red, yellow and green here, distinct from the letters' own six colour stripe.
        let coloured = render(&model, 80, 24, false);
        assert!(
            coloured.contains("\x1b[31m*\x1b[0m"),
            "row 1's twinkle cell"
        );
        assert!(
            coloured.contains("\x1b[33m*\x1b[0m"),
            "row 2's twinkle cell"
        );
        assert!(
            coloured.contains("\x1b[32m*\x1b[0m"),
            "row 3's twinkle cell"
        );
    }

    #[test]
    fn image_mode_twinkle_frame_lights_its_three_cells_in_the_stripe_colours() {
        let twinkle = Twinkle {
            cells: [(10, 5), (20, 5), (30, 5)],
            frame: 1,
        };
        let out = twinkle_frame_image(&twinkle);
        assert!(out.contains("\x1b[5;10H\x1b[31m+\x1b[0m"));
        assert!(out.contains("\x1b[5;20H\x1b[33m+\x1b[0m"));
        assert!(out.contains("\x1b[5;30H\x1b[32m+\x1b[0m"));
    }

    #[test]
    fn image_mode_twinkle_clear_blanks_its_three_cells() {
        let out = twinkle_clear_image([(10, 5), (20, 5), (30, 5)]);
        assert_eq!(out, "\x1b[5;10H \x1b[5;20H \x1b[5;30H ");
    }

    #[test]
    fn a_text_mode_bounce_changes_the_letters_colouring_and_never_repeats_the_previous_one() {
        // The same `random_tint_excluding` a bounce itself calls (see `Model::step`), and the
        // same guarantee `model::tests::a_tint_is_never_chosen_twice_in_a_row` checks for image
        // mode: every possible outgoing tint, including `Rainbow`, over many draws each, must
        // never hand back a colouring equal to the one it replaced.
        let mut m = text_model(80, 24);
        for current in Image::ALL {
            for _ in 0..50 {
                let next = m.random_tint_excluding(current);
                assert_ne!(
                    tint_letter_styles(next),
                    tint_letter_styles(current),
                    "the letters' colouring repeated after a bounce"
                );
            }
        }
    }
}
