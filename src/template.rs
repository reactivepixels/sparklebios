//! {slot} substitution.

use crate::facts::Facts;

fn is_key_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'
}

/// Replace every `{key}` with the fact of that name.
/// Returns None when any key is missing, so the caller can omit the line.
pub fn render(template: &str, facts: &Facts) -> Option<String> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            let mut j = i + 1;
            while j < chars.len() && is_key_char(chars[j]) {
                j += 1;
            }
            if j > i + 1 && j < chars.len() && chars[j] == '}' {
                let key: String = chars[i + 1..j].iter().collect();
                let value = facts.get(&key)?;
                out.push_str(value);
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::Facts;

    #[test]
    fn replaces_known_slots() {
        let f = Facts::fixture();
        assert_eq!(render("{cpu.cores} cores", &f).as_deref(), Some("14 cores"));
    }

    #[test]
    fn replaces_several_slots_in_one_line() {
        let f = Facts::fixture();
        assert_eq!(
            render("{os.name} {os.version}", &f).as_deref(),
            Some("macOS 26.5")
        );
    }

    #[test]
    fn missing_slot_yields_none() {
        assert_eq!(render("x {disk.model} y", &Facts::fixture()), None);
    }

    #[test]
    fn text_without_slots_is_unchanged() {
        assert_eq!(render("Ok", &Facts::new()).as_deref(), Some("Ok"));
        assert_eq!(render("", &Facts::new()).as_deref(), Some(""));
    }

    #[test]
    fn malformed_braces_are_literal() {
        let f = Facts::new();
        assert_eq!(render("{}", &f).as_deref(), Some("{}"));
        assert_eq!(
            render("{ not a key }", &f).as_deref(),
            Some("{ not a key }")
        );
        assert_eq!(render("open { only", &f).as_deref(), Some("open { only"));
    }
}
