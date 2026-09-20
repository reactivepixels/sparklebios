//! Sprinkles: the optional delight layer on the once-a-day animated show. Off by default, one
//! dial, costs nothing when off. Every function here is only ever reached from `show::play` once
//! the dial reads `light` or `full`, and the plain building blocks below (the splice, the
//! milestone check, the beep timing) are unit tested on their own, away from the show's timing.

/// The `sprinkles` dial. An unknown value, in the config file or the `SPARKLEBIOS_SPRINKLES`
/// override, reads as `Off`, the same tolerant way every other config key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Off,
    Light,
    Full,
}

impl Level {
    pub fn parse(s: &str) -> Level {
        match s {
            "light" => Level::Light,
            "full" => Level::Full,
            _ => Level::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::Light => "light",
            Level::Full => "full",
        }
    }

    /// Shimmer, twinkle and the stripe sweep run at `light` and `full`.
    pub fn text_effects(self) -> bool {
        matches!(self, Level::Light | Level::Full)
    }

    /// The POST beep and the beep codes only run at `full`.
    pub fn sound(self) -> bool {
        matches!(self, Level::Full)
    }
}

/// `env`, when set at all, wins over `config_level`, even when it does not parse to a known
/// level (it then reads as `Off`): the same rule `SPARKLEBIOS_GRAPHICS` follows for `graphics`.
pub fn resolve(config_level: Level, env: Option<&str>) -> Level {
    match env {
        Some(value) => Level::parse(value),
        None => config_level,
    }
}

/// The narrowest terminal a sprinkle is ever drawn on. Below this, every effect is skipped, not
/// squeezed, the same width `render::MIN_COLS_FOR_GRAPHICS` uses for the logo and badge.
pub const MIN_COLS: u16 = 60;

/// Whether the terminal is wide enough for a sprinkle at all.
pub fn wide_enough(term_cols: Option<u16>) -> bool {
    term_cols.is_some_and(|w| w >= MIN_COLS)
}

/// Replaces the visible columns `[start, start + len)` of `row` (a row already rendered by
/// `render::RowGeometry::line`, escape sequences and all) with `replacement`, which must itself
/// render to exactly `len` visible columns. Every other visible column is untouched, and whatever
/// SGR sequence was active immediately before the splice is resumed right after it, so text past
/// the splice keeps the colour it had before a sprinkle touched the row. This is the one move
/// every text effect below makes: shimmer and the stripe sweep splice a stretch of a line's own
/// text; twinkle splices a single margin cell.
pub fn splice_visible(row: &str, start: usize, len: usize, replacement: &str) -> String {
    let chars: Vec<char> = row.chars().collect();
    let mut out = String::with_capacity(row.len() + replacement.len());
    let mut i = 0;
    let mut visible = 0usize;
    let mut active_sgr: Option<String> = None;

    /// Consumes one `\x1b[...m` sequence starting at `i`, returning its end index and the
    /// sequence itself.
    fn read_sgr(chars: &[char], i: usize) -> (usize, String) {
        let mut j = i + 1;
        while j < chars.len() && chars[j] != 'm' {
            j += 1;
        }
        j = (j + 1).min(chars.len());
        (j, chars[i..j].iter().collect())
    }

    while i < chars.len() {
        if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'[') {
            let (end, seq) = read_sgr(&chars, i);
            if visible < start || visible >= start + len {
                out.push_str(&seq);
            }
            active_sgr = if seq == "\x1b[0m" { None } else { Some(seq) };
            i = end;
            continue;
        }
        if visible == start && len > 0 {
            // Skip exactly `len` visible columns (and any escapes interleaved with them): the
            // replacement fully re-specifies both their text and their colour.
            while visible < start + len && i < chars.len() {
                if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'[') {
                    let (end, _) = read_sgr(&chars, i);
                    i = end;
                    continue;
                }
                visible += 1;
                i += 1;
            }
            // Whatever coloured the columns just replaced is either closed right here, with no
            // more visible column left to need it (a reset sits immediately next, with nothing
            // in between: swallow it too, and resume nothing), or it is still governing more of
            // the line past the splice (anything else sits next, including no more input at
            // all): resume it once the replacement's own closing reset has run.
            let mut resume = active_sgr.clone();
            if chars.get(i) == Some(&'\x1b') && chars.get(i + 1) == Some(&'[') {
                let (end, seq) = read_sgr(&chars, i);
                if seq == "\x1b[0m" {
                    i = end;
                    resume = None;
                    active_sgr = None;
                }
            }
            out.push_str(replacement);
            out.push_str(resume.as_deref().unwrap_or(""));
            continue;
        }
        out.push(chars[i]);
        visible += 1;
        i += 1;
    }
    out
}

/// The literal escape SparkleBIOS uses for every sprinkle highlight: SGR 97, bright white,
/// applied over whatever the line's own style already was. Truecolor is never needed for it.
pub const HIGHLIGHT_SGR: &str = "\x1b[97m";
const RESET_SGR: &str = "\x1b[0m";

/// The width of the shimmer's moving highlight, in cells.
pub const SHIMMER_WINDOW: usize = 3;

/// How long the shimmer holds each position of its sweep, in milliseconds, at speed 1.0.
pub const SHIMMER_STEP_MS: u64 = 12;

/// Every left-to-right window position the shimmer sweeps a line of `line_len` cells through, one
/// per frame, in order: empty when the line is shorter than the window itself.
pub fn shimmer_positions(line_len: usize) -> Vec<usize> {
    if line_len < SHIMMER_WINDOW {
        return Vec::new();
    }
    (0..=(line_len - SHIMMER_WINDOW)).collect()
}

/// One shimmer frame: `line`, rendered already through `render::RowGeometry::line`, with its
/// `window_start`-th window highlighted.
pub fn shimmer_frame(
    row: &str,
    text_start_col: usize,
    line_text: &str,
    window_start: usize,
) -> String {
    let window: String = line_text
        .chars()
        .skip(window_start)
        .take(SHIMMER_WINDOW)
        .collect();
    let replacement = format!("{HIGHLIGHT_SGR}{window}{RESET_SGR}");
    splice_visible(
        row,
        text_start_col + window_start,
        SHIMMER_WINDOW,
        &replacement,
    )
}

/// How long each twinkle stage holds, in milliseconds, at speed 1.0.
pub const TWINKLE_STAGE_MS: u64 = 120;

/// The glyph a twinkling cell shows `elapsed_ms` after its own countdown began: a star, then a
/// plus, then a full stop, then nothing at all, and nothing ever again after that. One shot, not
/// a loop, so a cell only ever changes state 3 times over the whole show, comfortably under any
/// blink-rate ceiling.
pub fn twinkle_glyph(elapsed_ms: u64) -> Option<char> {
    match elapsed_ms / TWINKLE_STAGE_MS {
        0 => Some('*'),
        1 => Some('+'),
        2 => Some('.'),
        _ => None,
    }
}

/// One twinkle frame: `row` with the glyph for `elapsed_ms` spliced into visible column `col`,
/// unchanged when the twinkle has already reached its blank, final stage.
pub fn twinkle_frame(row: &str, col: usize, elapsed_ms: u64) -> String {
    match twinkle_glyph(elapsed_ms) {
        Some(glyph) => {
            let replacement = format!("{HIGHLIGHT_SGR}{glyph}{RESET_SGR}");
            splice_visible(row, col, 1, &replacement)
        }
        None => row.to_string(),
    }
}

/// Up to three of `candidates` (each a visible column in the mascot's margin), chosen from `seed`:
/// consecutive, wrapping entries starting at `seed % candidates.len()`, so the choice is stable
/// for a given boot but varies from one to the next. Never more than `candidates.len()`, and can
/// be fewer than three even when candidates allow it, since not every boot sparkles the same way.
pub fn twinkle_positions(seed: u64, candidates: &[usize]) -> Vec<usize> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let n = ((seed % 4) as usize).min(candidates.len()).min(3);
    let start = (seed as usize) % candidates.len();
    (0..n)
        .map(|k| candidates[(start + k) % candidates.len()])
        .collect()
}

/// The six ANSI foreground codes (SGR 30 + the number) the stripe sweep cycles through, in the
/// order the plan gives: red, yellow, green, cyan, blue, magenta.
pub const STRIPE_PALETTE: [u8; 6] = [31, 33, 32, 36, 34, 35];

/// How long the stripe sweep holds its rainbow frame, in milliseconds, at speed 1.0, before the
/// streak line settles back to its normal style.
pub const STRIPE_SWEEP_MS: u64 = 600;

/// True when `days` lands on a streak milestone: 7, 30, 100, 365, or any later multiple of 365.
pub fn is_streak_milestone(days: u64) -> bool {
    days == 7 || days == 30 || days == 100 || (days > 0 && days % 365 == 0)
}

/// The streak line, drawn once with every character in the next colour of `STRIPE_PALETTE`: `row`
/// with its own text (`text_start_col` for `line_text.chars().count()` cells) replaced by that
/// rainbow.
pub fn stripe_frame(row: &str, text_start_col: usize, line_text: &str) -> String {
    let len = line_text.chars().count();
    if len == 0 {
        return row.to_string();
    }
    let mut replacement = String::new();
    for (i, ch) in line_text.chars().enumerate() {
        let code = STRIPE_PALETTE[i % STRIPE_PALETTE.len()];
        replacement.push_str(&format!("\x1b[{code}m{ch}{RESET_SGR}"));
    }
    splice_visible(row, text_start_col, len, &replacement)
}

/// The POST beep: one BEL, written once the memory count completes.
pub const POST_BEEP: &[u8] = b"\x07";

/// The gap between each of the three beep-code BELs, in milliseconds, when a `fail` finding is
/// present. The plan called for 180ms, three beeps in 540ms, a little over 5.5Hz: many terminals
/// render BEL as a full-screen flash, and that rate sits inside the range associated with
/// photosensitive seizures, and it also breaks this same feature's own promise that nothing
/// blinks faster than 3 times a second. 400ms keeps the "three short beeps" intent (about 2.5Hz)
/// while staying under that ceiling. Do not tune this back down.
pub const BEEP_CODE_GAP_MS: u64 = 400;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_parse_reads_the_three_values_and_anything_else_as_off() {
        assert_eq!(Level::parse("off"), Level::Off);
        assert_eq!(Level::parse("light"), Level::Light);
        assert_eq!(Level::parse("full"), Level::Full);
        for bad in ["", "LIGHT", "on", "holographic"] {
            assert_eq!(Level::parse(bad), Level::Off, "{bad}");
        }
    }

    #[test]
    fn as_str_round_trips_through_parse() {
        for level in [Level::Off, Level::Light, Level::Full] {
            assert_eq!(Level::parse(level.as_str()), level);
        }
    }

    #[test]
    fn text_effects_and_sound_gates() {
        assert!(!Level::Off.text_effects());
        assert!(Level::Light.text_effects());
        assert!(Level::Full.text_effects());
        assert!(!Level::Off.sound());
        assert!(!Level::Light.sound());
        assert!(Level::Full.sound());
    }

    #[test]
    fn resolve_falls_back_to_config_with_no_env_and_env_wins_including_when_unknown() {
        assert_eq!(resolve(Level::Full, None), Level::Full);
        assert_eq!(resolve(Level::Off, Some("full")), Level::Full);
        assert_eq!(resolve(Level::Full, Some("holographic")), Level::Off);
    }

    #[test]
    fn wide_enough_at_the_boundary() {
        assert!(!wide_enough(None));
        assert!(!wide_enough(Some(59)));
        assert!(wide_enough(Some(60)));
    }

    #[test]
    fn splice_replaces_only_the_targeted_columns() {
        assert_eq!(splice_visible("hello world", 6, 5, "PLANE"), "hello PLANE");
    }

    #[test]
    fn splice_resumes_the_active_colour_after_a_mid_span_window() {
        let row = "\x1b[38;2;1;2;3mSparkle\x1b[0m";
        let out = splice_visible(row, 1, 3, "\x1b[97mpar\x1b[0m");
        assert_eq!(
            out,
            "\x1b[38;2;1;2;3mS\x1b[97mpar\x1b[0m\x1b[38;2;1;2;3mkle\x1b[0m"
        );
    }

    #[test]
    fn splice_over_a_whole_span_leaves_no_stray_colour_behind() {
        let row = "\x1b[38;2;1;2;3mhi\x1b[0m tail";
        let out = splice_visible(row, 0, 2, "\x1b[97mHI\x1b[0m");
        // The original span's own closing reset sits immediately after what was replaced, with
        // no more of its colour left to matter: it is swallowed rather than doubled up.
        assert_eq!(out, "\x1b[97mHI\x1b[0m tail");
    }

    #[test]
    fn splice_of_a_prefix_still_resumes_the_colour_for_the_rest_of_the_same_span() {
        // Same idea as `splice_resumes_the_active_colour_after_a_mid_span_window`, but the
        // splice starts at column 0, exactly where the span's own opening SGR sits: the earlier,
        // simpler "did the colour start before the splice" rule mistook this for the whole span
        // being replaced and silently dropped the colour for "rkle" that follows.
        let row = "\x1b[38;2;1;2;3mSparkle\x1b[0m";
        let out = splice_visible(row, 0, 3, "\x1b[97mSpa\x1b[0m");
        assert_eq!(out, "\x1b[97mSpa\x1b[0m\x1b[38;2;1;2;3mrkle\x1b[0m");
    }

    #[test]
    fn shimmer_positions_sweep_every_window_and_are_empty_below_the_window_width() {
        assert_eq!(shimmer_positions(5), vec![0, 1, 2]);
        assert_eq!(shimmer_positions(3), vec![0]);
        assert_eq!(shimmer_positions(2), Vec::<usize>::new());
    }

    #[test]
    fn shimmer_frame_highlights_only_the_window() {
        let row = "\x1b[38;2;1;2;3mSparkle\x1b[0m";
        let out = shimmer_frame(row, 0, "Sparkle", 1);
        assert!(out.contains("\x1b[97mpar\x1b[0m"));
        assert!(out.starts_with("\x1b[38;2;1;2;3mS\x1b[97mpar"));
        assert!(out.ends_with("kle\x1b[0m"));
    }

    #[test]
    fn twinkle_glyph_runs_once_through_star_plus_dot_then_nothing() {
        assert_eq!(twinkle_glyph(0), Some('*'));
        assert_eq!(twinkle_glyph(119), Some('*'));
        assert_eq!(twinkle_glyph(120), Some('+'));
        assert_eq!(twinkle_glyph(239), Some('+'));
        assert_eq!(twinkle_glyph(240), Some('.'));
        assert_eq!(twinkle_glyph(359), Some('.'));
        assert_eq!(twinkle_glyph(360), None);
        assert_eq!(twinkle_glyph(10_000), None);
    }

    #[test]
    fn twinkle_frame_is_unchanged_once_blank() {
        let row = "  ";
        assert_eq!(twinkle_frame(row, 0, 360), row);
        assert_ne!(twinkle_frame(row, 0, 0), row);
    }

    #[test]
    fn twinkle_positions_never_exceed_three_or_the_candidate_list_and_are_distinct() {
        let candidates = vec![0, 1, 16, 17];
        for seed in 0..40u64 {
            let positions = twinkle_positions(seed, &candidates);
            assert!(positions.len() <= 3);
            let mut sorted = positions.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                positions.len(),
                "seed {seed} repeats a position"
            );
            for p in &positions {
                assert!(candidates.contains(p));
            }
        }
    }

    #[test]
    fn twinkle_positions_is_empty_with_no_candidates() {
        assert!(twinkle_positions(5, &[]).is_empty());
    }

    #[test]
    fn is_streak_milestone_matches_the_named_days_and_multiples_of_365_only() {
        for days in [7, 30, 100, 365, 730, 1095] {
            assert!(is_streak_milestone(days), "{days}");
        }
        for days in [0, 1, 6, 8, 29, 31, 99, 101, 364, 366] {
            assert!(!is_streak_milestone(days), "{days}");
        }
    }

    #[test]
    fn stripe_frame_colours_every_character_in_the_six_colour_cycle() {
        let row = "\x1b[38;2;1;2;3mabcdefg\x1b[0m";
        let out = stripe_frame(row, 0, "abcdefg");
        assert!(out.contains("\x1b[31ma\x1b[0m"));
        assert!(out.contains("\x1b[33mb\x1b[0m"));
        assert!(out.contains("\x1b[32mc\x1b[0m"));
        assert!(out.contains("\x1b[36md\x1b[0m"));
        assert!(out.contains("\x1b[34me\x1b[0m"));
        assert!(out.contains("\x1b[35mf\x1b[0m"));
        assert!(out.contains("\x1b[31mg\x1b[0m"));
        assert!(!out.contains("38;2;1;2;3"));
    }
}
