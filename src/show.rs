//! The animated show: plays a machine's steps with their delays, honouring any key press as a
//! skip that jumps straight to the final screen.

use crate::checks::Finding;
use crate::facts::Facts;
use crate::flavour::Flavour;
use crate::machine::Machine;
use crate::render::{self, AnimatedKind, ColorMode, Graphics, Span};

/// The geometry the show renders against: the same three inputs `render_static` takes besides
/// the machine and facts.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub mode: ColorMode,
    pub term_cols: Option<u16>,
    pub graphics: Graphics,
}

/// Keys pressed during the show, in the order they arrived, unfiltered.
pub struct ShowOutcome {
    pub typed: Vec<u8>,
}

fn write_bytes(out: &mut dyn std::io::Write, bytes: &[u8]) {
    let _ = out.write_all(bytes);
    let _ = out.flush();
}

/// The first draw of a row: no cursor movement needed, the cursor is already at column 0 of a
/// fresh line.
fn draw_initial(out: &mut dyn std::io::Write, row: &str) {
    write_bytes(out, row.as_bytes());
}

/// Redraws a row in place: `\r` back to column 0, the new content, and (when unpainted, so the
/// row has no fixed background fill of its own) `\x1b[K` to erase anything left over from a
/// longer previous frame.
fn redraw_in_place(out: &mut dyn std::io::Write, row: &str, painted: bool) {
    let mut buf = String::from("\r");
    buf.push_str(row);
    if !painted {
        buf.push_str("\x1b[K");
    }
    write_bytes(out, buf.as_bytes());
}

fn write_full_row(out: &mut dyn std::io::Write, row: &str) {
    write_bytes(out, row.as_bytes());
    write_bytes(out, b"\n");
}

fn finish_row(out: &mut dyn std::io::Write) {
    write_bytes(out, b"\n");
}

/// Waits for a key, honouring `speed`. `key_fd` of `None` (no terminal to poll) just sleeps, in
/// slices, and never reports a skip. Any bytes read are appended to `typed` and count as a skip,
/// per the rule that with `ISIG` off Ctrl-C arrives as a plain byte (0x03) like any other key.
fn wait_for_skip(typed: &mut Vec<u8>, key_fd: Option<i32>, ms: u64, speed: f32) -> bool {
    let total_ms = ((ms as f64) * (speed as f64)).max(0.0).round() as u64;
    if total_ms == 0 {
        return false;
    }
    match key_fd {
        Some(fd) => {
            let bytes = crate::tty::wait_for_key(fd, total_ms);
            if bytes.is_empty() {
                false
            } else {
                typed.extend_from_slice(&bytes);
                true
            }
        }
        None => {
            let mut remaining = total_ms;
            while remaining > 0 {
                let slice = remaining.min(50);
                std::thread::sleep(std::time::Duration::from_millis(slice));
                remaining -= slice;
            }
            false
        }
    }
}

/// The sequence of `(spans, wait_ms)` frames a step plays through, in order: `wait_ms` is how
/// long that frame holds before the next one appears (0 for a step's last frame, which is
/// followed immediately by the next row).
fn frames_for(step: &render::AnimatedStep) -> Vec<(&Vec<Span>, u64)> {
    match &step.kind {
        AnimatedKind::Instant => vec![(&step.spans, step.ms)],
        AnimatedKind::Detect { label_spans } => vec![(label_spans, step.ms), (&step.spans, 0)],
        AnimatedKind::Count { frames } => {
            let total = frames.len() as u64 + 1;
            let per = step.ms / total;
            let mut sequence: Vec<(&Vec<Span>, u64)> = frames.iter().map(|f| (f, per)).collect();
            sequence.push((&step.spans, 0));
            sequence
        }
    }
}

/// Plays `machine` into `out`, one row at a time: a `Print` or `Quip` row appears and holds for
/// its `ms`; a `Detect` row appears as its label plus `"... "`, holds, then is redrawn with its
/// result; a `Count` row counts up in about 24 redraws over its `ms`, gaining its suffix on the
/// last one. Border and padding rows appear at once, with no delay.
///
/// `key_fd`, when `Some`, is polled for a key press between frames; any key ends the show at
/// once, drawing everything still to come in its final state. `speed` multiplies every delay
/// (`0.0` collapses every wait to nothing). `flavour`, for a flavoured machine, supplies its
/// quips and its logo sprite; with `None` a flavoured machine simply omits whatever it cannot
/// resolve. `findings` supplies the lines for a `Findings` or `F1` step, the same way `flavour`
/// does for a `Quip` step. Returns the bytes read from `key_fd` while the show played, unfiltered
/// and in order.
#[allow(clippy::too_many_arguments)]
pub fn play(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    geometry: Geometry,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    out: &mut dyn std::io::Write,
    key_fd: Option<i32>,
    speed: f32,
) -> ShowOutcome {
    let row_geometry = render::row_geometry(
        machine,
        facts,
        seed,
        geometry.mode,
        geometry.term_cols,
        geometry.graphics,
        flavour,
        findings,
    );
    let steps = render::animated_layout(machine, facts, seed, flavour, findings);
    let pad_y = machine.pad_y as usize;
    let painted = row_geometry.painted();

    let mut typed = Vec::new();
    let mut skipped = false;

    if painted {
        if let Some(row) = row_geometry.border_row() {
            write_full_row(out, &row);
        }
        for _ in 0..pad_y {
            let row = row_geometry.pad_row();
            write_full_row(out, &row);
        }
    }

    for (index, step) in steps.iter().enumerate() {
        if skipped {
            let row = row_geometry.line(index, &step.spans);
            write_full_row(out, &row);
            continue;
        }
        let frames = frames_for(step);
        for (i, (spans, ms)) in frames.iter().enumerate() {
            let row = row_geometry.line(index, spans);
            if i == 0 {
                draw_initial(out, &row);
            } else {
                redraw_in_place(out, &row, painted);
            }
            if wait_for_skip(&mut typed, key_fd, *ms, speed) {
                skipped = true;
                let final_row = row_geometry.line(index, &step.spans);
                redraw_in_place(out, &final_row, painted);
                break;
            }
        }
        finish_row(out);
    }

    if painted {
        for _ in 0..pad_y {
            let row = row_geometry.pad_row();
            write_full_row(out, &row);
        }
        if let Some(row) = row_geometry.border_row() {
            write_full_row(out, &row);
        }
    }

    ShowOutcome { typed }
}

/// The bytes the user typed during the show, with a lone ESC (0x1b), Ctrl-C (0x03), and any
/// complete escape sequence starting with ESC removed: what `--hook` mode hands back to the
/// shell, so the terminal's own escape sequences (arrow keys and the like) never land on the
/// prompt.
pub fn filter_typed(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x03 {
            i += 1;
            continue;
        }
        if b == 0x1b {
            if i + 1 >= bytes.len() {
                // A lone ESC at the end of the stream: drop it.
                i += 1;
                continue;
            }
            if bytes[i + 1] == b'[' {
                // CSI sequence: ESC '[' <parameter/intermediate bytes> <final byte 0x40..=0x7E>.
                let mut j = i + 2;
                while j < bytes.len() && !(0x40..=0x7E).contains(&bytes[j]) {
                    j += 1;
                }
                i = if j < bytes.len() { j + 1 } else { bytes.len() };
                continue;
            }
            // Any other two byte escape sequence: ESC plus one character.
            i += 2;
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;
    use crate::machine;

    /// Resolves a stream of animated frame bytes into its final visible rows: applies `\r`
    /// overwrites within each row, splits on `\n`, and strips SGR (`\x1b[...m`) and Kitty
    /// (`\x1b_G...\x1b\`) escape sequences, leaving only what would be visible on screen once
    /// the show settles.
    fn resolve_rows(stream: &[u8]) -> Vec<String> {
        resolve_raw_rows(stream)
            .iter()
            .map(|row| strip_escapes(row))
            .collect()
    }

    /// Like `resolve_rows`, but keeps the raw SGR and Kitty escapes instead of stripping them.
    fn resolve_raw_rows(stream: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(stream);
        // Each row's final in-place state is everything after its last `\r`.
        let mut rows: Vec<String> = text
            .split('\n')
            .map(|line| line.rsplit('\r').next().unwrap_or(line).to_string())
            .collect();
        // `split('\n')` yields a trailing empty entry for a stream ending in '\n'; drop it.
        if rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows
    }

    /// The byte index in `s` where its `col`-th visible character begins, skipping SGR and Kitty
    /// escape sequences. Returns `s.len()` when `s` has fewer than `col` visible characters.
    fn visible_col_byte_index(s: &str, col: usize) -> usize {
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        let mut visible = 0;
        let mut i = 0;
        while i < chars.len() {
            let (byte_idx, c) = chars[i];
            if c == '\x1b' && chars.get(i + 1).map(|&(_, c2)| c2) == Some('_') {
                i += 2;
                while i < chars.len()
                    && !(chars[i].1 == '\x1b' && chars.get(i + 1).map(|&(_, c2)| c2) == Some('\\'))
                {
                    i += 1;
                }
                i += 2;
                continue;
            }
            if c == '\x1b' {
                i += 1;
                while i < chars.len() && chars[i].1 != 'm' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if visible == col {
                return byte_idx;
            }
            visible += 1;
            i += 1;
        }
        s.len()
    }

    fn strip_escapes(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'_') {
                i += 2;
                while i < chars.len() && !(chars[i] == '\x1b' && chars.get(i + 1) == Some(&'\\')) {
                    i += 1;
                }
                i += 2;
            } else if chars[i] == '\x1b' {
                i += 1;
                while i < chars.len() && chars[i] != 'm' {
                    i += 1;
                }
                i += 1;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    fn resolved_rows_of_render_static(
        machine: &Machine,
        facts: &Facts,
        geometry: Geometry,
        flavour: Option<&Flavour>,
        findings: &[Finding],
    ) -> Vec<String> {
        let rendered = crate::render::render_static(
            machine,
            facts,
            geometry.mode,
            0,
            geometry.term_cols,
            geometry.graphics,
            flavour,
            findings,
        );
        rendered.lines().map(strip_escapes).collect::<Vec<_>>()
    }

    /// A small unflavoured, painted, bordered, uppercase machine with its own quips, exercising
    /// the engine features the retired `pc85` and `c64` machines used to cover.
    const OTHER_MACHINE: &str = r##"
id = "other"
name = "Other"
cols = 40
fg = "#6C5EB5"
bright = "#FFFFFF"
accent = "#B8C76F"
bg = "#352879"
paint = true
border = "#6C5EB5"
pad_x = 0
pad_y = 1
uppercase = true
quips = [
  "quip one",
  "quip two",
  "quip three",
]

[[step]]
print = "ready."

[[step]]
count = "{n}K found"
to = "{mem.kb}"
ms = 40

[[step]]
detect = "Detecting drive "
result = "{disk.free_gb}GB"

[[step]]
quip = true
"##;

    fn other_machine() -> Machine {
        machine::parse(OTHER_MACHINE).unwrap()
    }

    /// The flavour a machine boots with in these tests: the unicorn flavour for the flavoured
    /// `pc95`, none for the others.
    fn flavour_for(id: &str) -> Option<Flavour> {
        (id == "pc95").then(|| crate::flavour::find("unicorn", None).unwrap())
    }

    /// `Facts::fixture()`, with `flavour`'s own slots applied when given.
    fn facts_for(flavour: Option<&Flavour>) -> Facts {
        let mut facts = Facts::fixture();
        if let Some(f) = flavour {
            crate::flavour::apply(f, &mut facts);
        }
        facts
    }

    fn play_to_rows(
        machine: &Machine,
        geometry: Geometry,
        flavour: Option<&Flavour>,
        findings: &[Finding],
    ) -> Vec<String> {
        let facts = facts_for(flavour);
        let mut buf: Vec<u8> = Vec::new();
        play(
            machine, &facts, 0, geometry, flavour, findings, &mut buf, None, 0.0,
        );
        resolve_rows(&buf)
    }

    #[test]
    fn speed_zero_resolves_to_the_same_rows_as_render_static_unpainted() {
        for (id, m) in [
            ("pc95", machine::find("pc95", None).unwrap()),
            ("other", other_machine()),
        ] {
            let flavour = flavour_for(id);
            let geometry = Geometry {
                mode: ColorMode::None,
                term_cols: None,
                graphics: Graphics::None,
            };
            let rows = play_to_rows(&m, geometry, flavour.as_ref(), &[]);
            let facts = facts_for(flavour.as_ref());
            let expected =
                resolved_rows_of_render_static(&m, &facts, geometry, flavour.as_ref(), &[]);
            assert_eq!(rows, expected, "{id} unpainted rows disagree");
        }
    }

    #[test]
    fn speed_zero_resolves_to_the_same_rows_as_render_static_painted() {
        for (id, m) in [
            ("pc95", machine::find("pc95", None).unwrap()),
            ("other", other_machine()),
        ] {
            let flavour = flavour_for(id);
            let geometry = Geometry {
                mode: ColorMode::TrueColor,
                term_cols: Some(100),
                graphics: Graphics::HalfBlocks,
            };
            let rows = play_to_rows(&m, geometry, flavour.as_ref(), &[]);
            let facts = facts_for(flavour.as_ref());
            let expected =
                resolved_rows_of_render_static(&m, &facts, geometry, flavour.as_ref(), &[]);
            assert_eq!(rows, expected, "{id} painted rows disagree");
        }
    }

    #[test]
    fn speed_zero_final_rows_equal_render_static_for_pc95_with_both_flavours() {
        let m = machine::find("pc95", None).unwrap();
        for id in ["unicorn", "sumo"] {
            let flavour = crate::flavour::find(id, None).unwrap();
            let geometry = Geometry {
                mode: ColorMode::None,
                term_cols: None,
                graphics: Graphics::None,
            };
            let rows = play_to_rows(&m, geometry, Some(&flavour), &[]);
            let facts = facts_for(Some(&flavour));
            let expected =
                resolved_rows_of_render_static(&m, &facts, geometry, Some(&flavour), &[]);
            assert_eq!(rows, expected, "pc95 with {id} disagrees");
        }
    }

    fn finding(id: &str, severity: Severity, facts: &[(&str, &str)]) -> Finding {
        Finding {
            id: id.to_string(),
            severity,
            ttl: 100,
            facts: facts
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    /// The four findings from the plan's fixture table, in render order.
    fn fixture_findings() -> Vec<Finding> {
        vec![
            finding(
                "boot_order",
                Severity::Info,
                &[("boot.devices", "eko-pro, sparklebios, klang-stack")],
            ),
            finding(
                "boot_dirty",
                Severity::Info,
                &[
                    ("boot.device", "eko-pro"),
                    ("boot.changes", "3 uncommitted changes"),
                ],
            ),
            finding(
                "irq_conflict",
                Severity::Warn,
                &[
                    ("irq.port", "3000"),
                    ("irq.name", "node"),
                    ("irq.pid", "4821"),
                    ("irq.age", "3 days"),
                ],
            ),
            finding(
                "virus_one",
                Severity::Fail,
                &[
                    ("virus.repo", "eko-pro"),
                    ("virus.file", ".env.local"),
                    ("virus.count", "1"),
                ],
            ),
        ]
    }

    /// `facts_for(flavour)` plus every slot the fixture findings' phrasing needs.
    fn facts_with_findings_for(flavour: Option<&Flavour>) -> Facts {
        let mut facts = facts_for(flavour);
        facts.insert("boot.devices", "eko-pro, sparklebios, klang-stack");
        facts.insert("boot.device", "eko-pro");
        facts.insert("boot.changes", "3 uncommitted changes");
        facts.insert("irq.port", "3000");
        facts.insert("irq.name", "node");
        facts.insert("irq.pid", "4821");
        facts.insert("irq.age", "3 days");
        facts.insert("virus.repo", "eko-pro");
        facts.insert("virus.file", ".env.local");
        facts.insert("virus.count", "1");
        facts
    }

    #[test]
    fn speed_zero_final_rows_equal_render_static_for_pc95_with_findings_for_both_flavours() {
        let m = machine::find("pc95", None).unwrap();
        for id in ["unicorn", "sumo"] {
            let flavour = crate::flavour::find(id, None).unwrap();
            let geometry = Geometry {
                mode: ColorMode::None,
                term_cols: None,
                graphics: Graphics::None,
            };
            let findings = fixture_findings();
            let facts = facts_with_findings_for(Some(&flavour));
            let mut buf: Vec<u8> = Vec::new();
            play(
                &m,
                &facts,
                0,
                geometry,
                Some(&flavour),
                &findings,
                &mut buf,
                None,
                0.0,
            );
            let rows = resolve_rows(&buf);
            let expected =
                resolved_rows_of_render_static(&m, &facts, geometry, Some(&flavour), &findings);
            assert_eq!(rows, expected, "pc95 with {id} and findings disagrees");
        }
    }

    #[test]
    fn a_transparent_pc95_show_never_emits_a_background_colour() {
        let m = machine::find("pc95", None).unwrap();
        assert_eq!(m.bg, None);
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::HalfBlocks,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            &mut buf,
            None,
            0.0,
        );
        // The screen fill colour must never appear. The one exception is a logo pixel where both
        // the upper and lower source pixels are opaque: that background belongs to the sprite,
        // not the screen, and only ever sits inside the 14-cell logo box (7 rows tall, starting
        // after the machine's pad_y blank rows).
        let rows = resolve_raw_rows(&buf);
        let pad_x = m.pad_x as usize;
        let pad_y = m.pad_y as usize;
        const LOGO_ROWS: usize = 7;
        let logo_rows = pad_y..pad_y + LOGO_ROWS;
        for (i, row) in rows.iter().enumerate() {
            if logo_rows.contains(&i) {
                let cutoff = visible_col_byte_index(row, pad_x + 14);
                assert!(
                    !row[cutoff..].contains("48;2;"),
                    "row {i} paints a background colour outside the logo box: {row:?}"
                );
            } else {
                assert!(
                    !row.contains("48;2;"),
                    "row {i} outside the logo rows paints a background colour: {row:?}"
                );
            }
        }
    }

    #[test]
    fn the_stream_shows_an_intermediate_detect_frame_and_many_count_values() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::None,
            term_cols: None,
            graphics: Graphics::None,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            &mut buf,
            None,
            0.0,
        );
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("Detecting Horn             ... \r"));
        assert!(!text.contains("Detecting Horn             ... 1 found\r"));

        let mut values: Vec<&str> = text
            .split('\r')
            .filter(|s| s.starts_with("Memory Testing :"))
            .map(|s| s.split('\n').next().unwrap_or(s))
            .collect();
        values.sort_unstable();
        values.dedup();
        assert!(
            values.len() >= 10,
            "expected at least 10 distinct Memory Testing values, got {}: {values:?}",
            values.len()
        );
    }

    #[test]
    fn a_key_press_ends_the_show_at_once_in_its_final_state() {
        let m = other_machine();
        let facts = Facts::fixture();
        let geometry = Geometry {
            mode: ColorMode::None,
            term_cols: None,
            graphics: Graphics::None,
        };
        let (read_fd, write_fd) = {
            let mut fds = [0i32; 2];
            // SAFETY: `fds` is a valid, writable array of two `c_int`s.
            let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
            assert_eq!(rc, 0);
            (fds[0], fds[1])
        };
        // SAFETY: `write_fd` is a valid, open, writable fd from the pipe just created.
        unsafe {
            libc::write(write_fd, b"x".as_ptr() as *const libc::c_void, 1);
        }
        let mut buf: Vec<u8> = Vec::new();
        let outcome = play(
            &m,
            &facts,
            0,
            geometry,
            None,
            &[],
            &mut buf,
            Some(read_fd),
            1000.0,
        );
        assert_eq!(outcome.typed, b"x");
        let rows = resolve_rows(&buf);
        let expected = resolved_rows_of_render_static(&m, &facts, geometry, None, &[]);
        assert_eq!(rows, expected);
        // SAFETY: both fds are valid, open descriptors owned by this test.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn filter_typed_strips_escape_sequences_and_ctrl_c() {
        assert_eq!(filter_typed(b"ls\x1b"), b"ls");
        assert_eq!(filter_typed(b"\x1b[A"), b"");
        assert_eq!(filter_typed(b"\x03"), b"");
        assert_eq!(filter_typed(b"git st"), b"git st");
    }
}
