//! Shared hand-edit machinery for a user's TOML file: replacing a single top level key's value
//! while leaving every other byte alone (comments, blank lines, key order, indentation, the
//! trailing newline).
//!
//! `config.rs` uses this on the user's `config.toml`, and `theme.rs` uses it on the user's
//! starship config's `palette` key. Both files hand-edit rather than parse-and-reserialise
//! because a round trip through `toml::Table` drops comments and blank lines and reorders keys,
//! which is exactly the destructive behaviour this module exists to avoid.

/// Whether `line`, trimmed of leading whitespace, starts a TOML table header (`[table]` or
/// `[[array-of-tables]]`). A commented out line does not count.
pub(crate) fn is_table_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.starts_with('#') && trimmed.starts_with('[')
}

/// Whether `line`, trimmed of leading whitespace, is an uncommented `key = ...` assignment for
/// exactly `key`.
fn is_key_line(line: &str, key: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    match trimmed.strip_prefix(key) {
        Some(rest) => rest.trim_start_matches(' ').starts_with('='),
        None => false,
    }
}

/// Whether byte `i` of `bytes` starts `pat`, an exact byte match (never panics on a UTF-8
/// character boundary, unlike slicing a `&str`, since `bytes` is treated as plain bytes).
fn bytes_start_with_at(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
    i + pat.len() <= bytes.len() && &bytes[i..i + pat.len()] == pat
}

/// For each of `lines`, whether it is live TOML at the moment it begins, as opposed to sitting
/// inside a multi-line string (`"""..."""` or `'''...'''`) carried in from an earlier line.
///
/// A user's starship config routinely holds a multi-line `format` string full of lines that look
/// exactly like table headers (starship segment syntax uses `[...]` too), so `is_table_header`
/// and `is_key_line` must never be trusted on a line still inside one of those strings. This is
/// the one scanner every top level key lookup here consults, so they can never disagree about
/// where a string ends.
///
/// Walks the file byte by byte, toggling into and out of a string on `"""` or `'''` (whichever
/// opened it; the other delimiter does nothing while inside), so an open and a close on the same
/// line, or several of either on one line, are all counted rather than assumed to be one each.
/// Only the state a line is entered with is recorded: a line that closes a string partway through
/// still counts as "inside a string" for `is_table_header`/`is_key_line`'s purposes, since its
/// content up to the close is still string data, not TOML syntax.
pub(crate) fn live_toml_lines(lines: &[&str]) -> Vec<bool> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        None,
        Basic,
        Literal,
    }

    let mut state = State::None;
    let mut live = Vec::with_capacity(lines.len());
    for line in lines {
        live.push(state == State::None);
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match state {
                State::None => {
                    if bytes_start_with_at(bytes, i, b"\"\"\"") {
                        state = State::Basic;
                        i += 3;
                    } else if bytes_start_with_at(bytes, i, b"'''") {
                        state = State::Literal;
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
                State::Basic => {
                    if bytes_start_with_at(bytes, i, b"\"\"\"") {
                        state = State::None;
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
                State::Literal => {
                    if bytes_start_with_at(bytes, i, b"'''") {
                        state = State::None;
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
    live
}

/// The index of `lines`' top level `key =` line: one that is live TOML (see `live_toml_lines`)
/// and appears before any live table header, so `key` nested inside a table such as
/// `[palettes.foo]` does not count, and nor does anything that merely looks like it inside a
/// multi-line string. `None` when there is no such line.
pub(crate) fn find_top_level_key_line(lines: &[&str], key: &str) -> Option<usize> {
    let live = live_toml_lines(lines);
    let mut past_first_table = false;
    for (index, line) in lines.iter().enumerate() {
        if !live[index] {
            continue;
        }
        if is_table_header(line) {
            past_first_table = true;
            continue;
        }
        if !past_first_table && is_key_line(line, key) {
            return Some(index);
        }
    }
    None
}

/// Replaces the value of `contents`' top level `key = ...` line (see `find_top_level_key_line`)
/// with `value`, already formatted as TOML (for example `"sumo"`, quotes included, rather than
/// the bare identifier), and returns the new contents.
///
/// Leaves every other byte alone: comments, blank lines, key order, the trailing newline, and the
/// replaced line's own leading whitespace.
///
/// When `key` has no top level line, appends `key = value` at the end of the top level section:
/// right before the first live top level table header, if there is one, so the new line is never
/// swallowed into that table (which would move the key rather than add it, a worse bug than a
/// missing key). With no table header at all, it is appended at the end of the file.
pub fn set_top_level_key(contents: &str, key: &str, value: &str) -> String {
    let had_trailing_newline = contents.is_empty() || contents.ends_with('\n');
    let body = contents.strip_suffix('\n').unwrap_or(contents);
    let lines: Vec<&str> = if contents.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };

    let mut out_lines: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();

    if let Some(index) = find_top_level_key_line(&lines, key) {
        let indent: String = lines[index]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        out_lines[index] = format!("{indent}{key} = {value}");
    } else {
        let live = live_toml_lines(&lines);
        let insert_at = lines
            .iter()
            .enumerate()
            .find(|(index, line)| live[*index] && is_table_header(line))
            .map(|(index, _)| index)
            .unwrap_or(out_lines.len());
        out_lines.insert(insert_at, format!("{key} = {value}"));
    }

    let mut new_contents = out_lines.join("\n");
    if had_trailing_newline {
        new_contents.push('\n');
    }
    new_contents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_toml_lines_marks_the_inside_of_a_multiline_basic_string_and_recovers_after_it() {
        let source = "a = \"\"\"\n[not a table]\nstill inside\n\"\"\"\nb = 1\n[real_table]";
        let lines: Vec<&str> = source.split('\n').collect();
        assert_eq!(
            live_toml_lines(&lines),
            vec![true, false, false, false, true, true]
        );
    }

    #[test]
    fn live_toml_lines_treats_a_string_opened_and_closed_on_one_line_as_never_leaving() {
        let source = "a = \"\"\"one line\"\"\"\nb = 1";
        let lines: Vec<&str> = source.split('\n').collect();
        assert_eq!(live_toml_lines(&lines), vec![true, true]);
    }

    #[test]
    fn live_toml_lines_handles_a_multiline_literal_string() {
        let source = "a = '''\nstill inside\n'''\nb = 1";
        let lines: Vec<&str> = source.split('\n').collect();
        assert_eq!(live_toml_lines(&lines), vec![true, false, false, true]);
    }

    #[test]
    fn set_top_level_key_replaces_an_existing_value_and_leaves_the_rest_alone() {
        let contents = "# a comment\nanimate = true\nflavour = \"unicorn\"\nchecks = true\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(
            updated,
            "# a comment\nanimate = true\nflavour = \"sumo\"\nchecks = true\n"
        );
    }

    #[test]
    fn set_top_level_key_appends_when_the_key_is_absent() {
        let contents = "animate = true\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(updated, "animate = true\nflavour = \"sumo\"\n");
    }

    #[test]
    fn set_top_level_key_appends_before_the_first_table_header_rather_than_after_it() {
        let contents = "animate = true\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(
            updated,
            "animate = true\n\nflavour = \"sumo\"\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n"
        );
    }

    #[test]
    fn set_top_level_key_ignores_the_same_key_inside_a_table() {
        let contents = "[table]\nflavour = \"inner\"\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(
            updated,
            "flavour = \"sumo\"\n[table]\nflavour = \"inner\"\n"
        );
    }

    #[test]
    fn set_top_level_key_ignores_a_commented_out_line() {
        let contents = "# flavour = \"old\"\nanimate = true\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(
            updated,
            "# flavour = \"old\"\nanimate = true\nflavour = \"sumo\"\n"
        );
    }

    #[test]
    fn set_top_level_key_keeps_the_original_lines_leading_whitespace() {
        let contents = "  flavour = \"old\"\nanimate = true\n";
        let updated = set_top_level_key(contents, "flavour", "\"sumo\"");
        assert_eq!(updated, "  flavour = \"sumo\"\nanimate = true\n");
    }

    #[test]
    fn set_top_level_key_preserves_the_trailing_newline_exactly() {
        let with_newline = "flavour = \"old\"\n";
        assert!(set_top_level_key(with_newline, "flavour", "\"sumo\"").ends_with('\n'));

        let without_newline = "flavour = \"old\"";
        assert!(!set_top_level_key(without_newline, "flavour", "\"sumo\"").ends_with('\n'));
    }
}
