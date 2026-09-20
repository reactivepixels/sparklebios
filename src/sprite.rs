//! Boot logos: a half-block character grid and a Kitty graphics escape, both built from the
//! same source image, for each built-in sprite.

/// A parsed sprite grid: a palette of `X=#RRGGBB` entries (one character key each) and the square
/// rows of characters that use them (14 by 14 for the small grid, 28 by 28 for the wide one).
/// `'.'` is always transparent and never appears in the palette. A key may be any single
/// character: the small grids only ever need `A`-`Z`, but the wide grids, with up to 40 colours,
/// run `A`-`Z`, then `a`-`z`, then `0`-`9`.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    palette: std::collections::BTreeMap<char, (u8, u8, u8)>,
    rows: Vec<Vec<char>>,
}

impl Grid {
    /// Parses the palette lines up to the first blank line, then the row grid after it. Never
    /// panics: a malformed palette line or an unknown character is simply not in the palette, and
    /// `color` returns `None` for it, same as `'.'`.
    fn parse(src: &str) -> Grid {
        let mut lines = src.lines();
        let mut palette = std::collections::BTreeMap::new();
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            if let Some((key, value)) = line.split_once('=') {
                if let (Some(c), Some(rgb)) = (key.chars().next(), parse_hex_rgb(value)) {
                    palette.insert(c, rgb);
                }
            }
        }
        let rows: Vec<Vec<char>> = lines.map(|line| line.chars().collect()).collect();
        Grid { palette, rows }
    }

    fn color(&self, c: char) -> Option<(u8, u8, u8)> {
        self.palette.get(&c).copied()
    }

    /// The width, in cells, of the grid's rows.
    pub fn cols(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }

    /// The grid's height once drawn as half-blocks: every two pixel rows collapse into one.
    pub fn half_rows(&self) -> usize {
        self.rows.len() / 2
    }
}

/// Parses a `#RRGGBB` colour. Returns `None` for anything else.
fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

/// A built-in sprite: the full-colour source image, transmitted to terminals that support the
/// Kitty graphics protocol, its parsed 14 by 14 half-block grid, and its parsed 28 by 28 half-block
/// grid, drawn instead of the small one when the screen has room for it.
pub struct Sprite {
    pub png: &'static [u8],
    pub grid: Grid,
    pub grid_wide: Grid,
}

const UNICORN_PNG: &[u8] = include_bytes!("../sprites/unicorn.png");
const UNICORN_GRID_SRC: &str = include_str!("../sprites/unicorn14.txt");
const UNICORN_GRID_WIDE_SRC: &str = include_str!("../sprites/unicorn28.txt");
const SUMO_PNG: &[u8] = include_bytes!("../sprites/sumo.png");
const SUMO_GRID_SRC: &str = include_str!("../sprites/sumo14.txt");
const SUMO_GRID_WIDE_SRC: &str = include_str!("../sprites/sumo28.txt");
const NINJA_PNG: &[u8] = include_bytes!("../sprites/ninja.png");
const NINJA_GRID_SRC: &str = include_str!("../sprites/ninja14.txt");
const NINJA_GRID_WIDE_SRC: &str = include_str!("../sprites/ninja28.txt");
const VIKING_PNG: &[u8] = include_bytes!("../sprites/viking.png");
const VIKING_GRID_SRC: &str = include_str!("../sprites/viking14.txt");
const VIKING_GRID_WIDE_SRC: &str = include_str!("../sprites/viking28.txt");
const LUCHADOR_PNG: &[u8] = include_bytes!("../sprites/luchador.png");
const LUCHADOR_GRID_SRC: &str = include_str!("../sprites/luchador14.txt");
const LUCHADOR_GRID_WIDE_SRC: &str = include_str!("../sprites/luchador28.txt");
const YETI_PNG: &[u8] = include_bytes!("../sprites/yeti.png");
const YETI_GRID_SRC: &str = include_str!("../sprites/yeti14.txt");
const YETI_GRID_WIDE_SRC: &str = include_str!("../sprites/yeti28.txt");
const RACCOON_PNG: &[u8] = include_bytes!("../sprites/raccoon.png");
const RACCOON_GRID_SRC: &str = include_str!("../sprites/raccoon14.txt");
const RACCOON_GRID_WIDE_SRC: &str = include_str!("../sprites/raccoon28.txt");

/// The built-in sprite named `name`, or `None` if there is no sprite by that name.
pub fn builtin(name: &str) -> Option<Sprite> {
    match name {
        "unicorn" => Some(Sprite {
            png: UNICORN_PNG,
            grid: Grid::parse(UNICORN_GRID_SRC),
            grid_wide: Grid::parse(UNICORN_GRID_WIDE_SRC),
        }),
        "sumo" => Some(Sprite {
            png: SUMO_PNG,
            grid: Grid::parse(SUMO_GRID_SRC),
            grid_wide: Grid::parse(SUMO_GRID_WIDE_SRC),
        }),
        "ninja" => Some(Sprite {
            png: NINJA_PNG,
            grid: Grid::parse(NINJA_GRID_SRC),
            grid_wide: Grid::parse(NINJA_GRID_WIDE_SRC),
        }),
        "viking" => Some(Sprite {
            png: VIKING_PNG,
            grid: Grid::parse(VIKING_GRID_SRC),
            grid_wide: Grid::parse(VIKING_GRID_WIDE_SRC),
        }),
        "luchador" => Some(Sprite {
            png: LUCHADOR_PNG,
            grid: Grid::parse(LUCHADOR_GRID_SRC),
            grid_wide: Grid::parse(LUCHADOR_GRID_WIDE_SRC),
        }),
        "yeti" => Some(Sprite {
            png: YETI_PNG,
            grid: Grid::parse(YETI_GRID_SRC),
            grid_wide: Grid::parse(YETI_GRID_WIDE_SRC),
        }),
        "raccoon" => Some(Sprite {
            png: RACCOON_PNG,
            grid: Grid::parse(RACCOON_GRID_SRC),
            grid_wide: Grid::parse(RACCOON_GRID_WIDE_SRC),
        }),
        _ => None,
    }
}

/// Half-block rendering: each text row packs two pixel rows using U+2580 (upper half block),
/// fg = upper pixel, bg = lower pixel. Returns one string per text row, each exactly `width`
/// cells wide, with its own SGR and a reset.
///
/// A transparent pixel takes `bg` when it is `Some`. When `bg` is `None` (a transparent painted
/// screen) a cell only emits a background colour of its own when both its pixels are opaque,
/// because that colour belongs to the sprite, not the screen fill: a cell where both pixels are
/// transparent is a plain space; a cell whose upper pixel is opaque and lower is transparent
/// draws U+2580 (upper half block) with the upper pixel as foreground and `\x1b[49m` for its
/// background; a cell whose upper pixel is transparent and lower is opaque draws U+2584 (lower
/// half block) with the lower pixel as foreground and `\x1b[49m`; a cell where both pixels are
/// opaque draws U+2580 with the upper pixel as foreground and the lower pixel as a real
/// `48;2;R;G;B` background, keeping the sprite at full vertical resolution.
pub fn half_blocks(grid: &Grid, bg: Option<(u8, u8, u8)>) -> Vec<String> {
    let rows = &grid.rows;
    let mut out = Vec::new();
    let mut pair = rows.chunks_exact(2);
    for chunk in &mut pair {
        let top = &chunk[0];
        let bottom = &chunk[1];
        let width = top.len().max(bottom.len());
        let mut row = String::new();
        for i in 0..width {
            let upper = top.get(i).copied().and_then(|c| grid.color(c));
            let lower = bottom.get(i).copied().and_then(|c| grid.color(c));
            match bg {
                Some((bg_r, bg_g, bg_b)) => {
                    if upper.is_none() && lower.is_none() {
                        row.push_str(&format!("\x1b[48;2;{bg_r};{bg_g};{bg_b}m \x1b[0m"));
                    } else {
                        let (fr, fg, fb) = upper.unwrap_or((bg_r, bg_g, bg_b));
                        let (br, bgg, bb) = lower.unwrap_or((bg_r, bg_g, bg_b));
                        row.push_str(&format!(
                            "\x1b[38;2;{fr};{fg};{fb};48;2;{br};{bgg};{bb}m\u{2580}\x1b[0m"
                        ));
                    }
                }
                None => match (upper, lower) {
                    (None, None) => row.push(' '),
                    (None, Some((lr, lg, lb))) => {
                        row.push_str(&format!("\x1b[38;2;{lr};{lg};{lb};49m\u{2584}\x1b[0m"));
                    }
                    (Some((fr, fg, fb)), None) => {
                        row.push_str(&format!("\x1b[38;2;{fr};{fg};{fb};49m\u{2580}\x1b[0m"));
                    }
                    (Some((fr, fg, fb)), Some((lr, lg, lb))) => {
                        row.push_str(&format!(
                            "\x1b[38;2;{fr};{fg};{fb};48;2;{lr};{lg};{lb}m\u{2580}\x1b[0m"
                        ));
                    }
                },
            }
        }
        out.push(row);
    }
    out
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, with padding. Written by hand: no crate.
fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(BASE64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(BASE64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Kitty graphics protocol: transmit and display the PNG scaled into `cols` x `rows` cells,
/// without moving the cursor (C=1). Base64 payload split into chunks of at most 4096 bytes
/// (m=1 on all but the last). First chunk keys: a=T,f=100,q=2,C=1,c=<cols>,r=<rows>.
pub fn kitty_image(png: &[u8], cols: u16, rows: u16) -> String {
    let payload = base64_encode(png);
    let chunks: Vec<&[u8]> = payload.as_bytes().chunks(4096).collect();
    let mut out = String::new();
    let last_index = chunks.len().saturating_sub(1);
    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i == last_index { 0 } else { 1 };
        if i == 0 {
            out.push_str(&format!("\x1b_Ga=T,f=100,q=2,C=1,c={cols},r={rows},m={m};"));
        } else {
            out.push_str(&format!("\x1b_Gm={m};"));
        }
        // Base64 output is pure ASCII, so every chunk is valid UTF-8: no unsafe code needed.
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push_str("\x1b\\");
    }
    out
}

/// True when TERM is "xterm-ghostty" or "xterm-kitty", or TERM_PROGRAM is "ghostty".
pub fn supports_kitty(term: Option<&str>, term_program: Option<&str>) -> bool {
    matches!(term, Some("xterm-ghostty") | Some("xterm-kitty")) || term_program == Some("ghostty")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strips `\x1b[...m` SGR sequences, leaving only the visible characters.
    fn strip_sgr(s: &str) -> String {
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

    /// A tiny, standards-compliant base64 decoder, for round-tripping in tests only.
    fn base64_decode(s: &str) -> Vec<u8> {
        let value = |c: u8| -> u32 {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                _ => 0,
            }
        };
        let mut out = Vec::new();
        let bytes = s.as_bytes();
        for chunk in bytes.chunks(4) {
            let pad = chunk.iter().filter(|&&b| b == b'=').count();
            let c0 = value(chunk[0]);
            let c1 = value(chunk[1]);
            let c2 = value(*chunk.get(2).unwrap_or(&b'A'));
            let c3 = value(*chunk.get(3).unwrap_or(&b'A'));
            let n = (c0 << 18) | (c1 << 12) | (c2 << 6) | c3;
            out.push(((n >> 16) & 0xFF) as u8);
            if pad < 2 {
                out.push(((n >> 8) & 0xFF) as u8);
            }
            if pad < 1 {
                out.push((n & 0xFF) as u8);
            }
        }
        out
    }

    fn unicorn_grid() -> Grid {
        builtin("unicorn").unwrap().grid
    }

    #[test]
    fn half_blocks_of_the_real_grid_is_seven_rows_of_fourteen_cells() {
        let rows = half_blocks(&unicorn_grid(), Some((0, 0, 0)));
        assert_eq!(rows.len(), 7);
        for row in &rows {
            let visible = strip_sgr(row);
            assert_eq!(visible.chars().count(), 14);
            assert!(visible.chars().all(|c| c == '\u{2580}' || c == ' '));
        }
    }

    #[test]
    fn half_blocks_of_a_tiny_grid_colours_the_upper_pixel_as_fg() {
        let grid = Grid::parse("R=#EB413A\nB=#3092E2\n\nR.\n.B");
        let rows = half_blocks(&grid, Some((1, 2, 3)));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].starts_with("\x1b[38;2;235;65;58;48;2;1;2;3m"));
    }

    #[test]
    fn half_blocks_with_no_background_is_a_plain_space_when_both_pixels_are_transparent() {
        let grid = Grid::parse("R=#EB413A\n\n..\n..");
        let rows = half_blocks(&grid, None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], "  ");
    }

    #[test]
    fn half_blocks_with_no_background_uses_49_for_a_transparent_lower_pixel() {
        let grid = Grid::parse("R=#EB413A\n\nR.\n..");
        let rows = half_blocks(&grid, None);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].contains("48;2;"));
        assert!(rows[0].contains("\x1b[38;2;235;65;58;49m\u{2580}"));
    }

    #[test]
    fn half_blocks_with_no_background_uses_49_for_a_transparent_upper_pixel() {
        let grid = Grid::parse("R=#EB413A\n\n.\nR");
        let rows = half_blocks(&grid, None);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].contains("48;2;"));
        assert!(rows[0].contains('\u{2584}'));
        assert!(rows[0].starts_with("\x1b[38;2;235;65;58;49m\u{2584}"));
    }

    #[test]
    fn half_blocks_with_no_background_still_paints_a_real_background_when_both_pixels_are_opaque() {
        let grid = Grid::parse("R=#EB413A\nB=#3092E2\n\nR\nB");
        let rows = half_blocks(&grid, None);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("\x1b[38;2;235;65;58;48;2;48;146;226m\u{2580}"));
    }

    #[test]
    fn base64_encodes_the_textbook_examples() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
    }

    #[test]
    fn kitty_image_chunks_a_large_payload() {
        let png = vec![0u8; 10000];
        let out = kitty_image(&png, 14, 7);
        assert!(out.starts_with("\x1b_Ga=T,f=100,q=2,C=1,c=14,r=7,m=1;"));
        assert!(out.contains("\x1b_Gm=0;"));

        // Every chunk is terminated by ESC \, and nothing follows the last one.
        let segments: Vec<&str> = out.split("\x1b\\").collect();
        assert_eq!(segments.last(), Some(&""));
        let chunks = &segments[..segments.len() - 1];
        assert!(
            chunks.len() > 1,
            "10000 zero bytes should need more than one chunk"
        );

        let mut payload = String::new();
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.starts_with("\x1b_G"),
                "chunk {i} missing its header start"
            );
            let body = &chunk["\x1b_G".len()..];
            let semicolon = body.find(';').unwrap();
            let (header, data) = (&body[..semicolon], &body[semicolon + 1..]);
            if i == chunks.len() - 1 {
                assert_eq!(header, "m=0");
            } else if i > 0 {
                assert_eq!(header, "m=1");
            }
            assert!(
                data.len() <= 4096,
                "chunk {i} payload is {} bytes",
                data.len()
            );
            payload.push_str(data);
        }
        assert_eq!(base64_decode(&payload), png);
    }

    #[test]
    fn supports_kitty_truth_table() {
        assert!(supports_kitty(Some("xterm-ghostty"), None));
        assert!(supports_kitty(Some("xterm-kitty"), None));
        assert!(supports_kitty(None, Some("ghostty")));
        assert!(!supports_kitty(Some("xterm-256color"), None));
        assert!(!supports_kitty(None, None));
    }

    #[test]
    fn unknown_sprite_name_is_none() {
        assert!(builtin("dragon").is_none());
    }

    #[test]
    fn every_builtin_flavours_sprite_grid_is_14_by_14_with_a_full_palette() {
        for f in crate::flavour::builtins() {
            let sprite = builtin(&f.sprite)
                .unwrap_or_else(|| panic!("{}: sprite {:?} did not resolve", f.id, f.sprite));
            assert_eq!(sprite.grid.rows.len(), 14, "{}: row count", f.id);
            for row in &sprite.grid.rows {
                assert_eq!(row.len(), 14, "{}: row width", f.id);
                for &c in row {
                    assert!(
                        c == '.' || sprite.grid.color(c).is_some(),
                        "{}: {c:?} has no palette entry",
                        f.id
                    );
                }
            }
        }
    }

    #[test]
    fn every_builtin_sprite_grid_is_14_by_14_with_a_full_palette() {
        for name in [
            "unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon",
        ] {
            let sprite = builtin(name).unwrap();
            assert_eq!(sprite.grid.rows.len(), 14, "{name} row count");
            for row in &sprite.grid.rows {
                assert_eq!(row.len(), 14, "{name} row width");
                for &c in row {
                    assert!(
                        c == '.' || sprite.grid.color(c).is_some(),
                        "{name}: {c:?} has no palette entry"
                    );
                }
            }
        }
    }

    #[test]
    fn every_builtin_sprite_wide_grid_is_28_by_28_with_a_full_palette() {
        for name in [
            "unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon",
        ] {
            let sprite = builtin(name).unwrap();
            assert_eq!(sprite.grid_wide.rows.len(), 28, "{name} wide row count");
            for row in &sprite.grid_wide.rows {
                assert_eq!(row.len(), 28, "{name} wide row width");
                for &c in row {
                    assert!(
                        c == '.' || sprite.grid_wide.color(c).is_some(),
                        "{name}: {c:?} has no palette entry in the wide grid"
                    );
                }
            }
        }
    }

    #[test]
    fn grid_parse_reads_lowercase_and_digit_palette_keys() {
        let grid = Grid::parse("A=#111111\na=#222222\n0=#333333\n\nAa0\n0aA");
        assert_eq!(grid.color('A'), Some((0x11, 0x11, 0x11)));
        assert_eq!(grid.color('a'), Some((0x22, 0x22, 0x22)));
        assert_eq!(grid.color('0'), Some((0x33, 0x33, 0x33)));
    }

    #[test]
    fn grid_cols_and_half_rows_match_the_wide_unicorn_grid() {
        let sprite = builtin("unicorn").unwrap();
        assert_eq!(sprite.grid.cols(), 14);
        assert_eq!(sprite.grid.half_rows(), 7);
        assert_eq!(sprite.grid_wide.cols(), 28);
        assert_eq!(sprite.grid_wide.half_rows(), 14);
    }
}
