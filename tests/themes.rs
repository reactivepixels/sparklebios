use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const VARIANTS: [&str; 10] = [
    "rainbows-and-unicorns",
    "rainbows-and-unicorns-paper",
    "rainbows-and-unicorns-ega",
    "rainbows-and-unicorns-workbench",
    "rainbows-and-unicorns-mane",
    "rainbows-and-unicorns-miami",
    "rainbows-and-unicorns-arcade",
    "rainbows-and-unicorns-vhs",
    "rainbows-and-unicorns-den",
    "rainbows-and-unicorns-sorbet",
];

const ROLE_KEYS: [&str; 6] = [
    "background",
    "foreground",
    "cursor-color",
    "cursor-text",
    "selection-background",
    "selection-foreground",
];

struct ThemeFile {
    palette: BTreeMap<u8, String>,
    roles: BTreeMap<String, String>,
}

fn theme_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("themes")
        .join(name)
}

fn docs_text() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/theme.md"))
        .expect("docs/theme.md should be readable")
}

fn is_lower_hex_color(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn parse_theme_file(path: &Path) -> ThemeFile {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut palette = BTreeMap::new();
    let mut roles = BTreeMap::new();
    for line in text.lines() {
        assert!(!line.trim().is_empty(), "theme file has a blank line");
        let (key, value) = line
            .split_once(" = ")
            .unwrap_or_else(|| panic!("line is not `key = value`: {line:?}"));
        if key == "palette" {
            let (index, hex) = value
                .split_once('=')
                .unwrap_or_else(|| panic!("palette line has no index: {line:?}"));
            let index: u8 = index
                .parse()
                .unwrap_or_else(|_| panic!("bad palette index: {line:?}"));
            assert!(
                palette.insert(index, hex.to_string()).is_none(),
                "duplicate palette index {index} in {path:?}"
            );
        } else {
            assert!(
                roles.insert(key.to_string(), value.to_string()).is_none(),
                "duplicate role key {key} in {path:?}"
            );
        }
    }
    ThemeFile { palette, roles }
}

fn check_shape(theme: &ThemeFile, name: &str) {
    assert_eq!(
        theme.palette.len(),
        16,
        "{name}: expected 16 palette entries, found {}",
        theme.palette.len()
    );
    for i in 0..16u8 {
        assert!(
            theme.palette.contains_key(&i),
            "{name}: missing palette index {i}"
        );
    }
    for (index, hex) in &theme.palette {
        assert!(
            is_lower_hex_color(hex),
            "{name}: palette {index} is not a lowercase #rrggbb colour: {hex}"
        );
    }
    assert_eq!(
        theme.roles.len(),
        ROLE_KEYS.len(),
        "{name}: expected exactly {} role keys, found {}",
        ROLE_KEYS.len(),
        theme.roles.len()
    );
    for role in ROLE_KEYS {
        let hex = theme
            .roles
            .get(role)
            .unwrap_or_else(|| panic!("{name}: missing role {role}"));
        assert!(
            is_lower_hex_color(hex),
            "{name}: role {role} is not a lowercase #rrggbb colour: {hex}"
        );
    }
}

/// Extracts a `#RRGGBB` colour (either case) between the first pair of
/// backticks in `cell`, lower cased. Any trailing text after the closing
/// backtick (such as a contrast figure in parentheses) is ignored.
fn extract_hex_in_backticks(cell: &str) -> Option<String> {
    let start = cell.find('`')?;
    let rest = &cell[start + 1..];
    let end = rest.find('`')?;
    let inner = &rest[..end];
    let bytes = inner.as_bytes();
    let is_hex =
        bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit());
    if is_hex {
        Some(inner.to_lowercase())
    } else {
        None
    }
}

/// Returns the body of the `### \`variant\`` section of `docs/theme.md`,
/// matched on the exact backticked heading so that `rainbows-and-unicorns`
/// is never confused with `rainbows-and-unicorns-paper` and the others.
fn doc_section(doc: &str, variant: &str) -> String {
    let heading_marker = format!("### `{variant}`");
    let mut section_lines = Vec::new();
    let mut found = false;
    for line in doc.lines() {
        if line.starts_with('#') {
            if found {
                break;
            }
            if line.starts_with(&heading_marker) {
                found = true;
            }
            continue;
        }
        if found {
            section_lines.push(line);
        }
    }
    assert!(found, "section for `{variant}` not found in docs/theme.md");
    section_lines.join("\n")
}

/// Palette values are table cells of the form `| <index> | <label> | \`#RRGGBB\` `,
/// possibly followed by extra Contrast columns.
fn doc_palette(section: &str) -> BTreeMap<u8, String> {
    let mut palette = BTreeMap::new();
    for line in section.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let mut i = 0;
        while i + 2 < cells.len() {
            if let Ok(index) = cells[i].parse::<u8>() {
                if index <= 15 {
                    if let Some(hex) = extract_hex_in_backticks(cells[i + 2]) {
                        palette.insert(index, hex);
                    }
                }
            }
            i += 1;
        }
    }
    palette
}

/// Role values are cells of the form `| <role> | \`#RRGGBB\` `, where the
/// foreground cell may carry a trailing contrast figure in parentheses.
fn doc_roles(section: &str) -> BTreeMap<String, String> {
    let mut roles = BTreeMap::new();
    for line in section.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .collect();
        let mut i = 0;
        while i + 1 < cells.len() {
            if ROLE_KEYS.contains(&cells[i]) {
                if let Some(hex) = extract_hex_in_backticks(cells[i + 1]) {
                    roles.insert(cells[i].to_string(), hex);
                }
            }
            i += 1;
        }
    }
    roles
}

fn relative_luminance(hex: &str) -> f64 {
    let channel = |c: u8| -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap();
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap();
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap();
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast_ratio(a: &str, b: &str) -> f64 {
    let la = relative_luminance(a) + 0.05;
    let lb = relative_luminance(b) + 0.05;
    if la > lb {
        la / lb
    } else {
        lb / la
    }
}

#[test]
fn theme_files_have_the_right_shape() {
    for variant in VARIANTS {
        let theme = parse_theme_file(&theme_path(variant));
        check_shape(&theme, variant);
    }
}

#[test]
fn theme_files_match_docs_theme_md() {
    let doc = docs_text();
    for variant in VARIANTS {
        let section = doc_section(&doc, variant);
        let documented_palette = doc_palette(&section);
        let documented_roles = doc_roles(&section);
        let theme = parse_theme_file(&theme_path(variant));

        assert_eq!(
            documented_palette.len(),
            16,
            "{variant}: docs did not yield 16 palette entries"
        );
        for (index, hex) in &theme.palette {
            let documented = documented_palette
                .get(index)
                .unwrap_or_else(|| panic!("{variant}: docs missing palette {index}"));
            assert_eq!(
                hex.to_lowercase(),
                documented.to_lowercase(),
                "{variant}: palette {index} drifted from docs"
            );
        }

        for role in ROLE_KEYS {
            let theme_hex = theme.roles.get(role).unwrap();
            let doc_hex = documented_roles
                .get(role)
                .unwrap_or_else(|| panic!("{variant}: docs missing role {role}"));
            assert_eq!(
                theme_hex.to_lowercase(),
                doc_hex.to_lowercase(),
                "{variant}: role {role} drifted from docs"
            );
        }
    }
}

#[test]
fn foreground_meets_wcag_contrast_against_background() {
    for variant in VARIANTS {
        let theme = parse_theme_file(&theme_path(variant));
        let bg = theme.roles.get("background").unwrap();
        let fg = theme.roles.get("foreground").unwrap();
        let ratio = contrast_ratio(bg, fg);
        assert!(
            ratio >= 7.0,
            "{variant}: foreground/background contrast {ratio:.2} is below 7.0"
        );
    }
}
