//! The unicorn logo: a half-block character grid and a Kitty graphics escape, both built from
//! the same source image.

/// The full-colour source image, transmitted to terminals that support the Kitty graphics
/// protocol.
pub const UNICORN_PNG: &[u8] = include_bytes!("../sprites/unicorn.png");
/// A 14 by 14 character grid, one letter per pixel, for terminals without Kitty graphics.
pub const UNICORN_GRID: &str = include_str!("../sprites/unicorn14.txt");

/// Palette for grid characters; `'.'` is transparent.
pub fn grid_color(c: char) -> Option<(u8, u8, u8)> {
    match c {
        'K' => Some((0x10, 0x0E, 0x14)),
        'W' => Some((0xF6, 0xF4, 0xEE)),
        'S' => Some((0xC8, 0xCA, 0xE4)),
        'D' => Some((0x46, 0x37, 0x5F)),
        'P' => Some((0xF0, 0xA0, 0xBE)),
        'p' => Some((0xC8, 0x78, 0xA0)),
        'R' => Some((0xEB, 0x41, 0x3A)),
        'O' => Some((0xFA, 0x82, 0x1E)),
        'Y' => Some((0xFE, 0xDE, 0x3C)),
        'G' => Some((0x68, 0xC4, 0x4A)),
        'B' => Some((0x30, 0x92, 0xE2)),
        'V' => Some((0x92, 0x30, 0xAA)),
        'H' => Some((0xFF, 0xD6, 0x5A)),
        'h' => Some((0xD6, 0xA0, 0x32)),
        'n' => Some((0x96, 0x64, 0x1A)),
        'L' => Some((0xFF, 0xF4, 0xBE)),
        'U' => Some((0x96, 0x87, 0xBE)),
        _ => None,
    }
}

/// Half-block rendering: each text row packs two pixel rows using U+2580 (upper half block),
/// fg = upper pixel, bg = lower pixel. A transparent pixel takes `bg`. Returns one string per
/// text row, each exactly `width` cells wide, with its own SGR and a reset.
pub fn half_blocks(grid: &str, bg: (u8, u8, u8)) -> Vec<String> {
    let rows: Vec<Vec<char>> = grid.lines().map(|line| line.chars().collect()).collect();
    let (bg_r, bg_g, bg_b) = bg;
    let mut out = Vec::new();
    let mut pair = rows.chunks_exact(2);
    for chunk in &mut pair {
        let top = &chunk[0];
        let bottom = &chunk[1];
        let width = top.len().max(bottom.len());
        let mut row = String::new();
        for i in 0..width {
            let upper = top.get(i).copied().and_then(grid_color);
            let lower = bottom.get(i).copied().and_then(grid_color);
            if upper.is_none() && lower.is_none() {
                row.push_str(&format!("\x1b[48;2;{bg_r};{bg_g};{bg_b}m \x1b[0m"));
            } else {
                let (fr, fg, fb) = upper.unwrap_or(bg);
                let (br, bgg, bb) = lower.unwrap_or(bg);
                row.push_str(&format!(
                    "\x1b[38;2;{fr};{fg};{fb};48;2;{br};{bgg};{bb}m\u{2580}\x1b[0m"
                ));
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

    #[test]
    fn half_blocks_of_the_real_grid_is_seven_rows_of_fourteen_cells() {
        let rows = half_blocks(UNICORN_GRID, (0, 0, 0));
        assert_eq!(rows.len(), 7);
        for row in &rows {
            let visible = strip_sgr(row);
            assert_eq!(visible.chars().count(), 14);
            assert!(visible.chars().all(|c| c == '\u{2580}' || c == ' '));
        }
    }

    #[test]
    fn half_blocks_of_a_tiny_grid_colours_the_upper_pixel_as_fg() {
        let rows = half_blocks("R.\n.B", (1, 2, 3));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].starts_with("\x1b[38;2;235;65;58;48;2;1;2;3m"));
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
}
