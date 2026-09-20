//! The animated show: plays a machine's steps with their delays, honouring any key press as a
//! skip that jumps straight to the final screen.

use crate::checks::{Finding, Severity};
use crate::facts::Facts;
use crate::flavour::Flavour;
use crate::machine::Machine;
use crate::render::{self, AnimatedKind, ColorMode, Graphics, Span};
use crate::sprinkles;

/// The geometry the show renders against: the same three inputs `render_static` takes besides
/// the machine and facts.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    /// Whether the terminal supports colour, and how much.
    pub mode: ColorMode,
    /// The terminal's width, if known.
    pub term_cols: Option<u16>,
    /// How much graphical detail to draw.
    pub graphics: Graphics,
}

/// Keys pressed during the show, in the order they arrived, unfiltered.
pub struct ShowOutcome {
    /// The raw bytes typed.
    pub typed: Vec<u8>,
}

/// A test-only timeline of exactly what `play` wrote and exactly how long, nominally (before
/// `speed`), it held between writes: every `write_bytes` call and every `wait_for_skip` call
/// records itself here when `sim::record` is recording, so a test can measure the real frame
/// timing the player uses instead of reimplementing it. Compiled out entirely otherwise.
#[cfg(test)]
pub(crate) mod sim {
    use std::cell::RefCell;

    pub(crate) enum Event {
        Draw(Vec<u8>),
        Wait(u64),
    }

    thread_local! {
        static LOG: RefCell<Option<Vec<Event>>> = const { RefCell::new(None) };
    }

    fn log(event: Event) {
        LOG.with(|log| {
            if let Some(events) = log.borrow_mut().as_mut() {
                events.push(event);
            }
        });
    }

    pub(crate) fn draw(bytes: &[u8]) {
        log(Event::Draw(bytes.to_vec()));
    }

    pub(crate) fn wait(ms: u64) {
        log(Event::Wait(ms));
    }

    /// Runs `f`, returning every event it recorded, in order.
    pub(crate) fn record(f: impl FnOnce()) -> Vec<Event> {
        LOG.with(|log| *log.borrow_mut() = Some(Vec::new()));
        f();
        LOG.with(|log| log.borrow_mut().take().unwrap())
    }
}

fn write_bytes(out: &mut dyn std::io::Write, bytes: &[u8]) {
    #[cfg(test)]
    sim::draw(bytes);
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

/// A logical line's plain text: every span's own text, concatenated, with no styling. Used to
/// spot two rows sprinkles care about without either machine or step needing to say so: the
/// streak line (matched against `facts.get("flavour.streak")`) and the first findings line
/// (matched against `render::finding_text` for the first finding).
fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Waits for a key, honouring `speed`. `key_fd` of `None` (no terminal to poll) just sleeps, in
/// slices, and never reports a skip. Any bytes read are appended to `typed` and count as a skip,
/// per the rule that with `ISIG` off Ctrl-C arrives as a plain byte (0x03) like any other key.
fn wait_for_skip(typed: &mut Vec<u8>, key_fd: Option<i32>, ms: u64, speed: f32) -> bool {
    #[cfg(test)]
    sim::wait(ms);
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
/// does for a `Quip` step. `calendar_line`, when given, is tried ahead of the ordinary quip
/// rotation for the `quip` step, falling back to it when the calendar line does not resolve or
/// does not fit; whether one is passed at all is the caller's call (`boot.rs`), never this
/// function's or `render.rs`'s to make: see `render::resolve_quip_line`. Returns the bytes read
/// from `key_fd` while the show played, unfiltered and in order.
///
/// `sprinkles`, at `Off`, runs none of the code below this point beyond reading the value itself:
/// every row is exactly what it would have been before sprinkles existed. At `Light` or `Full`,
/// on a screen at least `sprinkles::MIN_COLS` wide and only until a key is typed, the show also
/// plays: a shimmer once the first line finishes (`sprinkles::shimmer_frame`); a twinkle in the
/// margin either side of the mascot while the memory count runs, the one row in the logo box that
/// redraws enough times for a sprinkle to be transient there (`sprinkles::twinkle_frame`); and,
/// only on a streak milestone, a stripe sweep across the streak line (`sprinkles::stripe_frame`).
/// `Full` also beeps: once after the memory count, and three times, `sprinkles::BEEP_CODE_GAP_MS`
/// apart, before the findings print, when one of them is a `fail`.
#[allow(clippy::too_many_arguments)]
pub fn play(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    geometry: Geometry,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
    out: &mut dyn std::io::Write,
    key_fd: Option<i32>,
    speed: f32,
    sprinkles: sprinkles::Level,
) -> ShowOutcome {
    let row_geometry = render::row_geometry(
        machine,
        geometry.mode,
        geometry.term_cols,
        geometry.graphics,
        flavour,
    );
    let steps = render::animated_layout(machine, facts, seed, flavour, findings, calendar_line);
    let pad_y = machine.pad_y as usize;
    let painted = row_geometry.painted();

    let wide_enough = sprinkles::wide_enough(geometry.term_cols);
    let sprinkle_text = sprinkles.text_effects() && wide_enough;
    // Sound is skipped, not just the text effects, below the same width: a screen too narrow to
    // sparkle in is too narrow to be the one that suddenly beeps at you either.
    let sprinkle_sound = sprinkles.sound() && wide_enough;
    let streak_milestone = facts
        .get("streak.days")
        .and_then(|s| s.parse::<u64>().ok())
        .is_some_and(sprinkles::is_streak_milestone);
    let streak_text = facts.get("flavour.streak").map(str::to_string);
    let first_fail_finding_text = findings
        .iter()
        .any(|f| f.severity == Severity::Fail)
        .then(|| findings.first())
        .flatten()
        .and_then(|f| render::finding_text(machine, facts, flavour, f));
    let mut findings_beeped = false;

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

        // Beep codes: three BELs, spaced out to stay under 3 flashes a second on a terminal that
        // renders BEL as a screen flash, right before the first findings line, when one finding
        // is a `fail`. Sound never redraws anything, so it needs no row of its own.
        if sprinkle_sound && !findings_beeped {
            if let Some(text) = &first_fail_finding_text {
                if &plain_text(&step.spans) == text {
                    findings_beeped = true;
                    for i in 0..3 {
                        write_bytes(out, sprinkles::POST_BEEP);
                        if i < 2
                            && wait_for_skip(&mut typed, key_fd, sprinkles::BEEP_CODE_GAP_MS, speed)
                        {
                            skipped = true;
                            break;
                        }
                    }
                }
            }
        }
        if skipped {
            let row = row_geometry.line(index, &step.spans);
            write_full_row(out, &row);
            continue;
        }

        // Stripe sweep: on a streak milestone, the streak line is drawn once, rainbow coloured,
        // held for its own sweep duration, then settled to its normal style, replacing its usual
        // frame entirely.
        if sprinkle_text
            && streak_milestone
            && streak_text.as_deref() == Some(plain_text(&step.spans).as_str())
        {
            let text_col = row_geometry.text_start_col(index);
            let line_text = plain_text(&step.spans);
            let normal_row = row_geometry.line(index, &step.spans);
            let rainbow_row = sprinkles::stripe_frame(&normal_row, text_col, &line_text);
            draw_initial(out, &rainbow_row);
            if wait_for_skip(&mut typed, key_fd, sprinkles::STRIPE_SWEEP_MS, speed) {
                skipped = true;
            }
            redraw_in_place(out, &normal_row, painted);
            finish_row(out);
            continue;
        }

        // Twinkle only ever touches the memory count row: the one row in the logo box that
        // redraws enough times, over enough of its own span, for a sprinkle placed there to stay
        // transient and still settle before its own final, suffixed value is shown.
        let twinkle_cols: Vec<usize> = if sprinkle_text {
            match step.kind {
                AnimatedKind::Count { .. } => {
                    let candidates = row_geometry.twinkle_margin_cols(index);
                    sprinkles::twinkle_positions(seed, &candidates)
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let frames = frames_for(step);
        let mut elapsed_in_row: u64 = 0;
        for (i, (spans, ms)) in frames.iter().enumerate() {
            let mut row = row_geometry.line(index, spans);
            for col in &twinkle_cols {
                row = sprinkles::twinkle_frame(&row, *col, elapsed_in_row);
            }
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
            elapsed_in_row += ms;
        }

        // Shimmer: once the first line's own frame(s) have settled, one highlight sweeps across
        // it left to right, then it settles back to exactly what it already was.
        if !skipped && sprinkle_text && index == 0 {
            let text_col = row_geometry.text_start_col(index);
            let line_text = plain_text(&step.spans);
            for window_start in sprinkles::shimmer_positions(line_text.chars().count()) {
                let row = row_geometry.line(index, &step.spans);
                let shimmer_row =
                    sprinkles::shimmer_frame(&row, text_col, &line_text, window_start);
                redraw_in_place(out, &shimmer_row, painted);
                if wait_for_skip(&mut typed, key_fd, sprinkles::SHIMMER_STEP_MS, speed) {
                    skipped = true;
                    break;
                }
            }
            let row = row_geometry.line(index, &step.spans);
            redraw_in_place(out, &row, painted);
        }

        finish_row(out);

        // The POST beep: one BEL, once the memory count completes.
        if !skipped && sprinkle_sound {
            if let AnimatedKind::Count { .. } = step.kind {
                write_bytes(out, sprinkles::POST_BEEP);
            }
        }
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

    /// The number of Kitty transmission sequences (`\x1b_Ga=T...`) in `bytes`: the mascot image
    /// should be sent exactly once per show, never once per redraw of the row it sits in.
    fn apc_transmit_count(bytes: &[u8]) -> usize {
        String::from_utf8_lossy(bytes).matches("\x1b_Ga=T").count()
    }

    /// Every Kitty graphics APC sequence (`\x1b_G...\x1b\`) in `bytes` is well formed: its body
    /// contains no bare ESC byte (which would mean some other escape, most likely a sprinkle's
    /// own SGR, has been spliced into the middle of it, corrupting the image payload) and it is
    /// properly terminated. Applied broadly, not only to sprinkles tests, since a malformed APC
    /// is a bug in the base show as much as in any effect layered over it.
    fn assert_apc_sequences_are_well_formed(bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == 0x1b
                && bytes.get(i + 1) == Some(&b'_')
                && bytes.get(i + 2) == Some(&b'G')
            {
                let start = i;
                i += 3;
                let mut terminated = false;
                while i < bytes.len() {
                    if bytes[i] == 0x1b {
                        if bytes.get(i + 1) == Some(&b'\\') {
                            terminated = true;
                            i += 2;
                            break;
                        }
                        panic!(
                            "APC sequence starting at byte {start} contains a bare ESC at byte \
                             {i}: an effect likely spliced into the image payload"
                        );
                    }
                    i += 1;
                }
                assert!(
                    terminated,
                    "APC sequence starting at byte {start} was never terminated"
                );
            } else {
                i += 1;
            }
        }
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
            machine,
            &facts,
            0,
            geometry,
            flavour,
            findings,
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Off,
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
                graphics: Graphics::Kitty,
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
                None,
                &mut buf,
                None,
                0.0,
                sprinkles::Level::Off,
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
            graphics: Graphics::Kitty,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Off,
        );
        // The screen fill colour must never appear anywhere: a transparent painted screen never
        // emits a background SGR, and the mascot box, drawn as an actual image, never emits one
        // either.
        let rows = resolve_raw_rows(&buf);
        for (i, row) in rows.iter().enumerate() {
            assert!(
                !row.contains("48;2;"),
                "row {i} paints a background colour: {row:?}"
            );
        }
        // This is also the show's own base path, with no sprinkle in sight: the mascot image
        // must still be sent exactly once and stay well formed.
        assert_eq!(apc_transmit_count(&buf), 1);
        assert_apc_sequences_are_well_formed(&buf);
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
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Off,
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
            None,
            &mut buf,
            Some(read_fd),
            1000.0,
            sprinkles::Level::Off,
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

    // --- Sprinkles ---------------------------------------------------------------------------

    /// A frozen copy of `play`'s own body exactly as it read before sprinkles existed (see git
    /// history for `show.rs` before this milestone): `sprinkles::Level::Off` is required to
    /// reproduce this, byte for byte, rather than trusted to.
    #[allow(clippy::too_many_arguments)]
    fn baseline_play(
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
            geometry.mode,
            geometry.term_cols,
            geometry.graphics,
            flavour,
        );
        let steps = render::animated_layout(machine, facts, seed, flavour, findings, None);
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

    #[test]
    fn sprinkles_off_is_byte_identical_to_the_pre_sprinkles_player() {
        let m = machine::find("pc95", None).unwrap();
        let wide = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };
        let narrow = Geometry {
            mode: ColorMode::None,
            term_cols: Some(50),
            graphics: Graphics::None,
        };

        let unicorn = crate::flavour::find("unicorn", None).unwrap();
        let sumo = crate::flavour::find("sumo", None).unwrap();
        let scenarios: Vec<(Option<&Flavour>, Vec<Finding>, Geometry)> = vec![
            (Some(&unicorn), vec![], wide),
            (Some(&sumo), fixture_findings(), wide),
            (None, vec![], narrow),
        ];

        for (flavour, findings, geometry) in scenarios {
            let facts = if findings.is_empty() {
                facts_for(flavour)
            } else {
                facts_with_findings_for(flavour)
            };

            let mut sprinkled: Vec<u8> = Vec::new();
            play(
                &m,
                &facts,
                7,
                geometry,
                flavour,
                &findings,
                None,
                &mut sprinkled,
                None,
                0.0,
                sprinkles::Level::Off,
            );

            let mut baseline: Vec<u8> = Vec::new();
            baseline_play(
                &m,
                &facts,
                7,
                geometry,
                flavour,
                &findings,
                &mut baseline,
                None,
                0.0,
            );

            assert_eq!(
                sprinkled, baseline,
                "flavour {flavour:?} diverges from the pre-sprinkles player"
            );
        }
    }

    #[test]
    fn speed_zero_light_final_rows_equal_render_static() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Light,
        );
        let rows = resolve_rows(&buf);
        let expected = resolved_rows_of_render_static(&m, &facts, geometry, Some(&flavour), &[]);
        assert_eq!(rows, expected);
    }

    #[test]
    fn light_shows_a_shimmer_over_the_firmware_line_and_plays_no_sound() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Light,
        );
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("\x1b[97m"), "no shimmer highlight found");
        assert!(!buf.contains(&0x07), "light must never beep");
        // The shimmer sweeps across the firmware line, the same row the mascot's image is
        // transmitted on: it must never re-send that image, nor splice into it.
        assert_eq!(
            apc_transmit_count(&buf),
            1,
            "the mascot image was sent more than once"
        );
        assert_apc_sequences_are_well_formed(&buf);
    }

    /// The bug this guards against: a text effect that redraws the logo row (only ever the
    /// shimmer, since it is the only effect that ever touches row 0) once treated the row's own
    /// pre-rendered string, image escape and all, as plain text to splice a highlight into,
    /// landing bright-white SGR bytes in the middle of the image's base64 payload and, because
    /// that same string was re-rendered on every sweep frame, retransmitting the whole image with
    /// it each time. `RowGeometry::line` now only ever emits the transmission escape once, so a
    /// redraw of the logo row after that never carries the image at all for an effect to touch.
    #[test]
    fn kitty_sprinkles_full_sends_the_image_once_and_the_stream_stays_a_small_multiple_of_off() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };

        let mut off: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut off,
            None,
            0.0,
            sprinkles::Level::Off,
        );
        assert_eq!(apc_transmit_count(&off), 1);
        assert_apc_sequences_are_well_formed(&off);

        let mut full: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut full,
            None,
            0.0,
            sprinkles::Level::Full,
        );
        assert_eq!(
            apc_transmit_count(&full),
            1,
            "the mascot image should be sent exactly once, sprinkles or not"
        );
        assert_apc_sequences_are_well_formed(&full);
        // Every text effect Full can add (shimmer, twinkle, the stripe sweep, none of which fire
        // here since there is no streak milestone) redraws the same handful of rows a few more
        // times over; it must never come close to the ~22x a re-sent image caused before this was
        // fixed.
        assert!(
            full.len() <= off.len() * 4,
            "sprinkles full is {} bytes against {} off, more than 4x",
            full.len(),
            off.len()
        );
    }

    /// With no mascot drawn (`Graphics::None`), there is no margin left to twinkle in, so twinkle
    /// stands down entirely rather than drawing over cells that no longer belong to a logo box or
    /// panicking on an empty candidate list. Sprinkles otherwise still plays: the shimmer, which
    /// does not depend on a mascot being present, still sweeps across the firmware line. Seed 1 is
    /// chosen because it is one `twinkle_positions` would place a glyph at were a mascot on
    /// screen (see `text_start_col_and_margin_cols_sit_either_side_of_the_pc95_logo_box`'s margin
    /// columns), so its absence here is not just an artefact of a seed that never twinkles at all.
    #[test]
    fn twinkle_does_not_run_with_no_mascot_but_shimmer_still_does() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::None,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            1,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Light,
        );
        let text = String::from_utf8_lossy(&buf);
        assert!(
            text.contains("\x1b[97m"),
            "shimmer should still run with no mascot"
        );
        assert!(
            !text.contains('*'),
            "a twinkle star appeared with no mascot drawn"
        );
        assert!(
            !text.contains('+'),
            "a twinkle plus appeared with no mascot drawn"
        );
    }

    #[test]
    fn full_beeps_once_with_no_fail_and_four_times_with_one() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };

        let facts_no_fail = facts_for(Some(&flavour));
        let mut buf_no_fail: Vec<u8> = Vec::new();
        play(
            &m,
            &facts_no_fail,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf_no_fail,
            None,
            0.0,
            sprinkles::Level::Full,
        );
        assert_eq!(buf_no_fail.iter().filter(|&&b| b == 0x07).count(), 1);
        assert_eq!(apc_transmit_count(&buf_no_fail), 1);
        assert_apc_sequences_are_well_formed(&buf_no_fail);

        let findings = fixture_findings();
        let facts_with_fail = facts_with_findings_for(Some(&flavour));
        let mut buf_fail: Vec<u8> = Vec::new();
        play(
            &m,
            &facts_with_fail,
            0,
            geometry,
            Some(&flavour),
            &findings,
            None,
            &mut buf_fail,
            None,
            0.0,
            sprinkles::Level::Full,
        );
        assert_eq!(buf_fail.iter().filter(|&&b| b == 0x07).count(), 4);
        assert_eq!(apc_transmit_count(&buf_fail), 1);
        assert_apc_sequences_are_well_formed(&buf_fail);
    }

    #[test]
    fn a_key_press_produces_no_sprinkle_output_after_it() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let findings = fixture_findings();
        let facts = facts_with_findings_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
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
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &findings,
            None,
            &mut buf,
            Some(read_fd),
            1000.0,
            sprinkles::Level::Full,
        );
        let text = String::from_utf8_lossy(&buf);
        assert!(!text.contains("\x1b[97m"));
        assert!(!buf.contains(&0x07));
        // SAFETY: both fds are valid, open descriptors owned by this test.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    #[test]
    fn at_fifty_columns_no_sprinkle_output_at_all() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let findings = fixture_findings();
        let facts = facts_with_findings_for(Some(&flavour));
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(50),
            graphics: Graphics::Kitty,
        };
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &findings,
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Full,
        );
        assert!(!String::from_utf8_lossy(&buf).contains("\x1b[97m"));
        assert!(!buf.contains(&0x07));
    }

    #[test]
    fn light_stripe_sweep_appears_only_on_a_streak_milestone() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };

        let mut milestone_facts = Facts::fixture();
        milestone_facts.insert("streak.days", "30");
        milestone_facts.insert("streak.label", "30 days");
        crate::flavour::apply(&flavour, &mut milestone_facts);
        let mut buf: Vec<u8> = Vec::new();
        play(
            &m,
            &milestone_facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf,
            None,
            0.0,
            sprinkles::Level::Light,
        );
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("\x1b[31m"), "no red in the rainbow sweep");
        assert!(text.contains("\x1b[35m"), "no magenta in the rainbow sweep");
        assert_eq!(apc_transmit_count(&buf), 1);
        assert_apc_sequences_are_well_formed(&buf);

        let mut plain_facts = Facts::fixture();
        crate::flavour::apply(&flavour, &mut plain_facts);
        assert_ne!(plain_facts.get("streak.days"), Some("30"));
        let mut buf2: Vec<u8> = Vec::new();
        play(
            &m,
            &plain_facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut buf2,
            None,
            0.0,
            sprinkles::Level::Light,
        );
        assert!(!String::from_utf8_lossy(&buf2).contains("\x1b[31m"));
    }

    /// One cell of `TermSim`'s grid: the visible character and whatever SGR sequence is active
    /// over it, verbatim.
    type Cell = (char, String);
    /// `TermSim`'s whole grid, and the type a captured frame snapshot is kept in.
    type Grid = Vec<Vec<Cell>>;

    /// A minimal terminal simulator: just enough to answer "which cells actually changed between
    /// two consecutive frames", not to render anything for a person. Tracks one `Cell` per cell
    /// of a `cols` by `rows` grid, and understands `\r` (return to column 0 of the current row),
    /// `\n` (advance to the next row), `\x1b[...m` (the colour active from here on, until a
    /// `\x1b[0m` clears it), `\x1b[K` (erase to the end of the current row) and BEL (zero width:
    /// it never advances the cursor or touches a cell). A Kitty image escape (`\x1b_G...\x1b\`)
    /// is consumed but never touches a cell either, the same way the real image overlays the
    /// grid without the terminal moving its cursor for it.
    struct TermSim {
        cols: usize,
        grid: Grid,
        row: usize,
        col: usize,
    }

    impl TermSim {
        fn new(cols: usize, rows: usize) -> TermSim {
            TermSim {
                cols,
                grid: vec![vec![(' ', String::new()); cols]; rows],
                row: 0,
                col: 0,
            }
        }

        fn apply(&mut self, chunk: &[u8]) {
            let text = String::from_utf8_lossy(chunk);
            let chars: Vec<char> = text.chars().collect();
            let mut active = String::new();
            let mut i = 0;
            while i < chars.len() {
                match chars[i] {
                    '\r' => {
                        self.col = 0;
                        i += 1;
                    }
                    '\n' => {
                        self.col = 0;
                        self.row += 1;
                        i += 1;
                    }
                    '\x07' => {
                        i += 1;
                    }
                    '\x1b' if chars.get(i + 1) == Some(&'_') => {
                        i += 2;
                        while i < chars.len()
                            && !(chars[i] == '\x1b' && chars.get(i + 1) == Some(&'\\'))
                        {
                            i += 1;
                        }
                        i += 2;
                    }
                    '\x1b' if chars.get(i + 1) == Some(&'[') => {
                        let mut j = i + 1;
                        while j < chars.len() && chars[j] != 'm' && chars[j] != 'K' {
                            j += 1;
                        }
                        let terminator = chars.get(j).copied();
                        let end = (j + 1).min(chars.len());
                        let seq: String = chars[i..end].iter().collect();
                        if terminator == Some('K') {
                            if self.row < self.grid.len() {
                                for c in self.col..self.cols {
                                    self.grid[self.row][c] = (' ', String::new());
                                }
                            }
                        } else {
                            active = if seq == "\x1b[0m" { String::new() } else { seq };
                        }
                        i = end;
                    }
                    c => {
                        if self.row < self.grid.len() && self.col < self.cols {
                            self.grid[self.row][self.col] = (c, active.clone());
                        }
                        self.col += 1;
                        i += 1;
                    }
                }
            }
        }
    }

    fn changed_cells(a: &[Vec<Cell>], b: &[Vec<Cell>]) -> usize {
        a.iter()
            .zip(b)
            .flat_map(|(ra, rb)| ra.iter().zip(rb))
            .filter(|(x, y)| x != y)
            .count()
    }

    #[test]
    fn never_flash_no_frame_changes_more_than_480_cells_and_no_cell_blinks_faster_than_3_times_a_second(
    ) {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let facts = facts_for(Some(&flavour));
        // Wide enough to paint and to show the logo box, so shimmer, twinkle and sound all get to
        // run; a real 80 column terminal would fall back to plain rendering for pc95 (it needs 84
        // to paint), which would only exercise the shimmer. The 480 cell cap is used verbatim
        // regardless, which is stricter here than "a quarter of the screen" would be at this
        // width.
        let geometry = Geometry {
            mode: ColorMode::TrueColor,
            term_cols: Some(100),
            graphics: Graphics::Kitty,
        };

        let events = sim::record(|| {
            let mut buf: Vec<u8> = Vec::new();
            play(
                &m,
                &facts,
                0,
                geometry,
                Some(&flavour),
                &[],
                None,
                &mut buf,
                None,
                0.0,
                sprinkles::Level::Full,
            );
        });

        let cols = 100usize;
        let rows = 24usize;
        let mut sim_term = TermSim::new(cols, rows);
        let mut elapsed: u64 = 0;
        let mut frames: Vec<(u64, Grid)> = Vec::new();
        for event in &events {
            match event {
                sim::Event::Draw(bytes) => sim_term.apply(bytes),
                sim::Event::Wait(ms) => {
                    frames.push((elapsed, sim_term.grid.clone()));
                    elapsed += ms;
                }
            }
        }
        // The show's very last write is never followed by a `wait_for_skip` call (there is
        // nothing left to hold for), so it is missing from `frames` above: add it, so the final
        // settle is measured too.
        frames.push((elapsed, sim_term.grid.clone()));

        assert!(
            frames.len() > 10,
            "expected many frames, got {}",
            frames.len()
        );

        for i in 0..frames.len() - 1 {
            let changed = changed_cells(&frames[i].1, &frames[i + 1].1);
            assert!(
                changed <= 480,
                "frame {i} changed {changed} cells, more than a quarter of an 80x24 screen (480)"
            );
        }

        // No single sprinkled cell changes state more than 3 times in any second, anywhere in
        // the show: for every change, count how many changes (including itself) land inside the
        // second that starts there. This is the literal reading of "3 times a second", a bound
        // on any rolling window, not a minimum gap between one change and the next: a cell that
        // changes exactly twice, however close together, can never be part of a
        // 4-times-a-second flicker on its own.
        //
        // Scoped to the cells a sprinkle can actually touch (the firmware line's own text, for
        // the shimmer, and the memory count row's chosen margin cells, for the twinkle), not
        // every cell on screen: the memory count itself redraws its digits far more than 3 times
        // a second as it counts up, which is the base show's own long-standing behaviour, not a
        // sprinkle, and is well outside this milestone's scope.
        let row_geometry = render::row_geometry(
            &m,
            geometry.mode,
            geometry.term_cols,
            geometry.graphics,
            Some(&flavour),
        );
        let steps = render::animated_layout(&m, &facts, 0, Some(&flavour), &[], None);
        let prefix_rows = usize::from(m.border.is_some()) + m.pad_y as usize;

        let mut sprinkle_cells: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();
        let firmware_text = plain_text(&steps[0].spans);
        let text_col0 = row_geometry.text_start_col(0);
        for c in 0..firmware_text.chars().count() {
            sprinkle_cells.insert((prefix_rows, text_col0 + c));
        }
        if let Some(count_index) = steps
            .iter()
            .position(|s| matches!(s.kind, AnimatedKind::Count { .. }))
        {
            let candidates = row_geometry.twinkle_margin_cols(count_index);
            for col in sprinkles::twinkle_positions(0, &candidates) {
                sprinkle_cells.insert((prefix_rows + count_index, col));
            }
        }
        assert!(!sprinkle_cells.is_empty(), "no sprinkle cells identified");

        for &(row, col) in &sprinkle_cells {
            if row >= rows || col >= cols {
                continue;
            }
            let mut change_times: Vec<u64> = Vec::new();
            let mut previous = &frames[0].1[row][col];
            for (t, grid) in frames.iter().skip(1) {
                let cell = &grid[row][col];
                if cell != previous {
                    change_times.push(*t);
                    previous = cell;
                }
            }
            for (i, &t) in change_times.iter().enumerate() {
                let count_in_next_second = change_times[i..]
                    .iter()
                    .take_while(|&&later| later - t < 1000)
                    .count();
                assert!(
                    count_in_next_second <= 3,
                    "cell ({row},{col}) changed {count_in_next_second} times within a second starting at {t}ms"
                );
            }
        }
    }

    // --- Calendar lines -------------------------------------------------------------------------

    /// The animated show (this is the real path a Full boot or an interactive preview take), on a
    /// real calendar date, shows the calendar line in place of the quip with no row added, and
    /// its final rows agree exactly with the static path (`render::render_static_with_calendar`),
    /// so the two can never disagree about what a calendar day looks like.
    #[test]
    fn a_calendar_day_replaces_the_quip_in_the_animated_show_and_agrees_with_the_static_path() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let mut facts = facts_for(Some(&flavour));
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-01");
        let calendar_line = crate::machine::matching_calendar_text(&m, 2026, 1, 1);
        assert_eq!(
            calendar_line,
            Some("Year {date.year} rollover complete. Nothing caught fire. Again.")
        );
        let geometry = Geometry {
            mode: ColorMode::None,
            term_cols: None,
            graphics: Graphics::None,
        };

        let mut ordinary_buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut ordinary_buf,
            None,
            0.0,
            sprinkles::Level::Off,
        );
        let ordinary_rows = resolve_rows(&ordinary_buf);

        let mut calendar_buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            calendar_line,
            &mut calendar_buf,
            None,
            0.0,
            sprinkles::Level::Off,
        );
        let calendar_rows = resolve_rows(&calendar_buf);

        assert_eq!(
            ordinary_rows.len(),
            calendar_rows.len(),
            "a calendar line must replace the quip row in the animated show, never add one"
        );
        assert!(calendar_rows
            .iter()
            .any(|r| r.contains("Year 2026 rollover complete. Nothing caught fire. Again.")));
        assert!(!ordinary_rows
            .iter()
            .any(|r| r.contains("Year 2026 rollover complete")));

        // The static path must draw exactly the same final rows for the same calendar day: one
        // override point, reached by both.
        let expected = crate::render::render_static_with_calendar(
            &m,
            &facts,
            geometry.mode,
            0,
            geometry.term_cols,
            geometry.graphics,
            Some(&flavour),
            &[],
            calendar_line,
        );
        let expected_rows: Vec<String> = expected.lines().map(strip_escapes).collect();
        assert_eq!(calendar_rows, expected_rows);
    }

    /// The omission rule holds in the animated show too: on 19 January, with the one slot its
    /// calendar text needs deliberately missing, the show falls back to the ordinary quip, byte
    /// for byte identical to a show with no calendar override, rather than dropping the row.
    #[test]
    fn an_unresolved_calendar_line_falls_back_to_the_ordinary_quip_in_the_animated_show() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = crate::flavour::find("unicorn", None).unwrap();
        let mut facts = facts_for(Some(&flavour));
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-19");
        facts.remove("y2038.days");
        let calendar_line = crate::machine::matching_calendar_text(&m, 2026, 1, 19);
        assert_eq!(
            calendar_line,
            Some("Y2038 check: {y2038.days} days until 32-bit time runs out. Noted.")
        );
        let geometry = Geometry {
            mode: ColorMode::None,
            term_cols: None,
            graphics: Graphics::None,
        };

        let mut ordinary_buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            None,
            &mut ordinary_buf,
            None,
            0.0,
            sprinkles::Level::Off,
        );

        let mut calendar_buf: Vec<u8> = Vec::new();
        play(
            &m,
            &facts,
            0,
            geometry,
            Some(&flavour),
            &[],
            calendar_line,
            &mut calendar_buf,
            None,
            0.0,
            sprinkles::Level::Off,
        );

        assert_eq!(
            calendar_buf, ordinary_buf,
            "an unresolved calendar line must fall back to the ordinary quip exactly"
        );
        assert!(!String::from_utf8_lossy(&calendar_buf).contains("Y2038 check"));
    }
}
