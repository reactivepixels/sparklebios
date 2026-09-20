//! Boot logos: the source PNG for each built-in sprite, and the Kitty graphics escape that
//! transmits it.

const UNICORN_PNG: &[u8] = include_bytes!("../sprites/unicorn.png");
const SUMO_PNG: &[u8] = include_bytes!("../sprites/sumo.png");
const NINJA_PNG: &[u8] = include_bytes!("../sprites/ninja.png");
const VIKING_PNG: &[u8] = include_bytes!("../sprites/viking.png");
const LUCHADOR_PNG: &[u8] = include_bytes!("../sprites/luchador.png");
const YETI_PNG: &[u8] = include_bytes!("../sprites/yeti.png");
const RACCOON_PNG: &[u8] = include_bytes!("../sprites/raccoon.png");

/// The built-in sprite named `name`, as its PNG bytes, or `None` if there is no sprite by that
/// name.
pub fn builtin(name: &str) -> Option<&'static [u8]> {
    match name {
        "unicorn" => Some(UNICORN_PNG),
        "sumo" => Some(SUMO_PNG),
        "ninja" => Some(NINJA_PNG),
        "viking" => Some(VIKING_PNG),
        "luchador" => Some(LUCHADOR_PNG),
        "yeti" => Some(YETI_PNG),
        "raccoon" => Some(RACCOON_PNG),
        _ => None,
    }
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
    fn every_builtin_flavours_sprite_resolves_to_a_non_empty_png() {
        for f in crate::flavour::builtins() {
            let png = builtin(&f.sprite)
                .unwrap_or_else(|| panic!("{}: sprite {:?} did not resolve", f.id, f.sprite));
            assert!(!png.is_empty(), "{}: sprite {:?} is empty", f.id, f.sprite);
        }
    }

    #[test]
    fn every_builtin_sprite_png_starts_with_the_png_signature() {
        const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        for name in [
            "unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon",
        ] {
            let png = builtin(name).unwrap();
            assert!(
                png.starts_with(&PNG_SIGNATURE),
                "{name}: sprite does not start with the PNG signature"
            );
        }
    }
}
