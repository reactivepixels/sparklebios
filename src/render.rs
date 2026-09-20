//! Static rendering and colour modes.

use crate::checks::{Finding, Severity};
use crate::facts::Facts;
use crate::flavour::Flavour;
use crate::machine::{Machine, Step, Style};
use crate::template;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    None,
    Ansi16,
    TrueColor,
}

/// How the logo is drawn in the painted path. Plain and unpainted output never show a logo or
/// a badge, regardless of this setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graphics {
    None,
    HalfBlocks,
    Kitty,
}

/// One resolved, styled run of text within a logical line.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// NO_COLOR set to anything non-empty: None. COLORTERM of "truecolor" or "24bit": TrueColor. Otherwise Ansi16.
pub fn color_mode_from_env(no_color: Option<&str>, colorterm: Option<&str>) -> ColorMode {
    if matches!(no_color, Some(v) if !v.is_empty()) {
        return ColorMode::None;
    }
    match colorterm {
        Some("truecolor") | Some("24bit") => ColorMode::TrueColor,
        _ => ColorMode::Ansi16,
    }
}

/// Parses a `#RRGGBB` colour (validated already by `machine::parse`) into its components.
fn hex_rgb(colour: &str) -> (u8, u8, u8) {
    let bytes = colour.as_bytes();
    let byte = |i: usize| u8::from_str_radix(std::str::from_utf8(&bytes[i..i + 2]).unwrap(), 16);
    (
        byte(1).unwrap_or(0),
        byte(3).unwrap_or(0),
        byte(5).unwrap_or(0),
    )
}

fn colour_for(machine: &Machine, style: Style) -> &str {
    match style {
        Style::Normal => &machine.fg,
        Style::Bright => &machine.bright,
        Style::Accent => &machine.accent,
    }
}

/// Wraps `text` in the escape codes for `style`, or returns it unchanged for `ColorMode::None`.
fn paint(machine: &Machine, mode: ColorMode, style: Style, text: &str) -> String {
    match mode {
        ColorMode::None => text.to_string(),
        ColorMode::TrueColor => {
            let (r, g, b) = hex_rgb(colour_for(machine, style));
            format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m")
        }
        ColorMode::Ansi16 => {
            let code = match style {
                Style::Normal => "37",
                Style::Bright => "97",
                Style::Accent => "93",
            };
            format!("\x1b[{code}m{text}\x1b[0m")
        }
    }
}

/// The quips a machine draws from: a flavoured machine's own `quips` array when it has no
/// flavour applied, or the flavour's quips when it does; an unflavoured machine's own `quips`.
fn quips_for<'a>(machine: &'a Machine, flavour: Option<&'a Flavour>) -> &'a [String] {
    if machine.flavoured {
        flavour.map(|f| f.quips.as_slice()).unwrap_or(&[])
    } else {
        &machine.quips
    }
}

/// One line from a resolved quip, starting at `seed % quips.len()` and wrapping around. A
/// candidate whose rendered length exceeds `cols` is treated as unresolvable.
fn resolve_quip(quips: &[String], cols: u16, facts: &Facts, seed: u64) -> Option<String> {
    if quips.is_empty() {
        return None;
    }
    let start = (seed % quips.len() as u64) as usize;
    (0..quips.len()).find_map(|offset| {
        let idx = (start + offset) % quips.len();
        let text = template::render(&quips[idx], facts)?;
        if text.chars().count() > cols as usize {
            None
        } else {
            Some(text)
        }
    })
}

/// The sprite drawn as a machine's logo, if any: for a flavoured machine, the current flavour's
/// own sprite; for any other machine, its `logo` key (only `"unicorn"` exists today).
fn logo_sprite(machine: &Machine, flavour: Option<&Flavour>) -> Option<crate::sprite::Sprite> {
    let name = if machine.flavoured {
        flavour.map(|f| f.sprite.as_str())
    } else {
        machine.logo.as_deref()
    };
    name.and_then(crate::sprite::builtin)
}

/// The text of a `Detect` step's label: rendered through templates and, when
/// `machine.detect_width` is set, right-padded with spaces to that width (never truncated).
/// Without `detect_width`, the label is used exactly as written, unrendered, as before. `None`
/// when `detect_width` is set and the label's slots do not all resolve.
fn detect_label(machine: &Machine, facts: &Facts, label: &str) -> Option<String> {
    match machine.detect_width {
        Some(width) => {
            let rendered = template::render(label, facts)?;
            let width = width as usize;
            let len = rendered.chars().count();
            if len < width {
                Some(format!("{rendered}{}", " ".repeat(width - len)))
            } else {
                Some(rendered)
            }
        }
        None => Some(label.to_string()),
    }
}

/// Applies `machine.uppercase`, if set.
fn apply_case(machine: &Machine, text: String) -> String {
    if machine.uppercase {
        text.to_uppercase()
    } else {
        text
    }
}

/// A line is blank when every one of its spans is empty text.
fn is_blank(line: &[Span]) -> bool {
    line.iter().all(|span| span.text.is_empty())
}

/// The phrasing for a finding id: the flavour's override when the machine is flavoured and the
/// flavour has that key, otherwise the machine's own table, otherwise None.
fn phrasing<'a>(machine: &'a Machine, flavour: Option<&'a Flavour>, id: &str) -> Option<&'a str> {
    if machine.flavoured {
        if let Some(value) = flavour.and_then(|f| f.findings.get(id)) {
            return Some(value.as_str());
        }
    }
    machine.findings.get(id).map(String::as_str)
}

/// A shortened value never carries fewer than this many visible characters before its `..`.
const MIN_SHORTENED_VALUE_CHARS: usize = 3;

/// The shortest a value can be and still have anything left to give up: at this length,
/// dropping the marker's own cost (2 for `..`, plus the 1 character the line needs to shrink by)
/// still leaves it at the floor.
const MIN_SHRINKABLE_VALUE_CHARS: usize = MIN_SHORTENED_VALUE_CHARS + 3;

/// Shortens `value` by exactly one rendered character, marking it with a trailing `..` (which
/// count towards its length). `None` once `value` is already at or below the floor of
/// `MIN_SHORTENED_VALUE_CHARS` visible characters plus `..`, so the caller knows to stop.
fn shorten_slot_value(value: &str) -> Option<String> {
    if value.chars().count() < MIN_SHRINKABLE_VALUE_CHARS {
        return None;
    }
    let keep = value.chars().count() - 3;
    let content: String = value.chars().take(keep).collect();
    Some(content + "..")
}

/// Renders `phrase` against `facts`, shortening the currently longest slot value one character
/// at a time (instead of truncating the whole line) until it fits `cols`, so the sentence's own
/// wording, not just some prefix of the line, always survives. Falls back to truncating the
/// whole rendered line, as before, only once every slot value is already at its floor. `None`
/// when the phrase has a slot that does not resolve against `facts`.
pub(crate) fn shorten_finding_line(phrase: &str, facts: &Facts, cols: usize) -> Option<String> {
    let mut local = facts.clone();
    let mut rendered = template::render(phrase, &local)?;
    if rendered.chars().count() <= cols {
        return Some(rendered);
    }
    let keys = template::slot_keys(phrase);
    loop {
        let longest = keys
            .iter()
            .filter_map(|key| local.get(key).map(|value| (key, value.chars().count())))
            .filter(|(_, len)| *len >= MIN_SHRINKABLE_VALUE_CHARS)
            .max_by_key(|(_, len)| *len);
        let Some((key, _)) = longest else {
            break;
        };
        let value = local.get(key)?.to_string();
        let Some(shortened) = shorten_slot_value(&value) else {
            break;
        };
        local.insert(key, shortened);
        rendered = template::render(phrase, &local)?;
        if rendered.chars().count() <= cols {
            return Some(rendered);
        }
    }
    Some(rendered.chars().take(cols).collect())
}

/// One finding's rendered line: its phrasing looked up by id, rendered against the facts, and
/// shortened to fit `cols` (by shrinking its slot values, or truncating the whole line as a last
/// resort) rather than dropped when it runs long, so a Fail is always visible. `None` when the
/// id has no phrasing or one of its slots does not resolve.
fn finding_line(
    machine: &Machine,
    facts: &Facts,
    flavour: Option<&Flavour>,
    finding: &Finding,
    style: Style,
) -> Option<Vec<Span>> {
    let phrase = phrasing(machine, flavour, &finding.id)?;
    let text = shorten_finding_line(phrase, facts, machine.cols as usize)?;
    if text.is_empty() {
        return Some(vec![]);
    }
    Some(vec![Span {
        text: apply_case(machine, text),
        style,
    }])
}

/// One finding's rendered text, plain: no colour, no border, no padding, only `apply_case`
/// (matching every other line on the screen). The same phrasing lookup, rendering and truncation
/// `Step::Findings` uses, reused here so the quiet-mode Fail line and the full screen can never
/// disagree on wording. `None` when the id has no phrasing or one of its slots does not resolve.
pub fn finding_text(
    machine: &Machine,
    facts: &Facts,
    flavour: Option<&Flavour>,
    finding: &Finding,
) -> Option<String> {
    let spans = finding_line(machine, facts, flavour, finding, Style::Normal)?;
    Some(spans.into_iter().map(|span| span.text).collect())
}

/// The F1 line: `f1_resume` when it resolves, otherwise `f1` when at least one finding is `Warn`
/// or `Fail`, otherwise no line at all.
fn f1_line(
    machine: &Machine,
    facts: &Facts,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    style: Style,
) -> Option<Vec<Span>> {
    if let Some(text) =
        phrasing(machine, flavour, "f1_resume").and_then(|phrase| template::render(phrase, facts))
    {
        return Some(vec![Span {
            text: apply_case(machine, text),
            style,
        }]);
    }
    let warn_or_fail = findings
        .iter()
        .any(|f| matches!(f.severity, Severity::Warn | Severity::Fail));
    if !warn_or_fail {
        return None;
    }
    let phrase = phrasing(machine, flavour, "f1")?;
    let text = template::render(phrase, facts)?;
    Some(vec![Span {
        text: apply_case(machine, text),
        style,
    }])
}

/// Resolves a single step to its logical lines: zero for an unresolvable `print`, `count` or
/// `detect`; exactly one for a resolved `print`, `count`, `detect` or `quip`; zero or more for
/// `findings` (one per finding whose phrasing resolves); zero or one for `f1`.
fn layout_step(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    step: &Step,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> Vec<Vec<Span>> {
    match step {
        Step::Print { text, style, .. } => match template::render(text, facts) {
            Some(text) if text.is_empty() => vec![vec![]],
            Some(text) => vec![vec![Span {
                text: apply_case(machine, text),
                style: *style,
            }]],
            None => vec![],
        },
        Step::Count {
            template: tmpl,
            to,
            suffix,
            ..
        } => {
            let Some(n) = template::render(to, facts) else {
                return vec![];
            };
            let filled = tmpl.replacen("{n}", &n, 1);
            let Some(mut line) = template::render(&filled, facts) else {
                return vec![];
            };
            line.push_str(suffix);
            if line.is_empty() {
                vec![vec![]]
            } else {
                vec![vec![Span {
                    text: apply_case(machine, line),
                    style: Style::Normal,
                }]]
            }
        }
        Step::Detect {
            label,
            result,
            style,
            ..
        } => {
            let Some(label_text) = detect_label(machine, facts, label) else {
                return vec![];
            };
            let Some(result) = template::render(result, facts) else {
                return vec![];
            };
            vec![vec![
                Span {
                    text: apply_case(machine, format!("{label_text}... ")),
                    style: Style::Normal,
                },
                Span {
                    text: apply_case(machine, result),
                    style: *style,
                },
            ]]
        }
        Step::Quip { style, .. } => {
            match resolve_quip(quips_for(machine, flavour), machine.cols, facts, seed) {
                Some(quip) => vec![vec![Span {
                    text: apply_case(machine, quip),
                    style: *style,
                }]],
                None => vec![],
            }
        }
        Step::Findings { style, .. } => findings
            .iter()
            .filter_map(|f| finding_line(machine, facts, flavour, f, *style))
            .collect(),
        Step::F1 { style, .. } => match f1_line(machine, facts, flavour, findings, *style) {
            Some(line) => vec![line],
            None => vec![],
        },
    }
}

/// Never two blank lines in a row, and never a leading or trailing blank line.
fn collapse_blank_lines(lines: Vec<Vec<Span>>) -> Vec<Vec<Span>> {
    let mut collapsed: Vec<Vec<Span>> = Vec::with_capacity(lines.len());
    for line in lines {
        let previous_is_blank = collapsed.last().map(|l| is_blank(l)).unwrap_or(true);
        if is_blank(&line) && previous_is_blank {
            continue;
        }
        collapsed.push(line);
    }
    while collapsed.last().is_some_and(|l| is_blank(l)) {
        collapsed.pop();
    }
    collapsed
}

/// The machine's steps resolved into logical lines, with unresolvable and too-long-to-fit
/// lines omitted and blank lines collapsed. `seed` picks the quip: start at `seed % quips.len()`.
/// `flavour`, when the machine is flavoured, supplies its quips and (in the painted path) its
/// logo; ignored otherwise.
pub fn layout(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> Vec<Vec<Span>> {
    let lines: Vec<Vec<Span>> = machine
        .steps
        .iter()
        .flat_map(|step| layout_step(machine, facts, seed, step, flavour, findings))
        .collect();
    collapse_blank_lines(lines)
}

/// How a step's row changes over the course of the animated show (`show.rs`).
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatedKind {
    /// Appears once, already in its final state: a `Print` or `Quip` step.
    Instant,
    /// Appears first as `label_spans` (the label plus `"... "`), then is redrawn with the final
    /// state (the label plus the result): a `Detect` step.
    Detect { label_spans: Vec<Span> },
    /// Counts up through `frames` before settling on the final state: a `Count` step. Empty
    /// when the target does not parse as a number, in which case the step behaves like
    /// `Instant`.
    Count { frames: Vec<Vec<Span>> },
}

/// One step, resolved for the animated show: its final spans (`spans`, identical to what
/// `layout` produces for the same step), how long to hold on it in milliseconds before speed is
/// applied (`ms`), and how it gets there (`kind`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedStep {
    pub ms: u64,
    pub spans: Vec<Span>,
    pub kind: AnimatedKind,
}

/// Roughly how many redraws a `Count` step plays out over its `ms`: the growing frames plus the
/// final, suffixed value.
const COUNT_FRAMES: u64 = 24;

/// Resolves a single step for the animated show, or an empty `Vec` to omit it. Mirrors
/// `layout_step` exactly for the final state (`AnimatedStep::spans`), so `layout` and
/// `animated_layout` always agree on which lines are visible and what they finally say.
/// `findings` and `f1` are always `AnimatedKind::Instant`, one `AnimatedStep` per finding line.
fn animate_step(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    step: &Step,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> Vec<AnimatedStep> {
    match step {
        Step::Print { text, style, ms } => match template::render(text, facts) {
            Some(text) => {
                let spans = if text.is_empty() {
                    vec![]
                } else {
                    vec![Span {
                        text: apply_case(machine, text),
                        style: *style,
                    }]
                };
                vec![AnimatedStep {
                    ms: *ms,
                    spans,
                    kind: AnimatedKind::Instant,
                }]
            }
            None => vec![],
        },
        Step::Quip { style, ms } => {
            match resolve_quip(quips_for(machine, flavour), machine.cols, facts, seed) {
                Some(quip) => {
                    let spans = vec![Span {
                        text: apply_case(machine, quip),
                        style: *style,
                    }];
                    vec![AnimatedStep {
                        ms: *ms,
                        spans,
                        kind: AnimatedKind::Instant,
                    }]
                }
                None => vec![],
            }
        }
        Step::Detect {
            label,
            result,
            style,
            ms,
        } => {
            let Some(label) = detect_label(machine, facts, label) else {
                return vec![];
            };
            let Some(result) = template::render(result, facts) else {
                return vec![];
            };
            let label_text = apply_case(machine, format!("{label}... "));
            let label_spans = vec![Span {
                text: label_text.clone(),
                style: Style::Normal,
            }];
            let spans = vec![
                Span {
                    text: label_text,
                    style: Style::Normal,
                },
                Span {
                    text: apply_case(machine, result),
                    style: *style,
                },
            ];
            vec![AnimatedStep {
                ms: *ms,
                spans,
                kind: AnimatedKind::Detect { label_spans },
            }]
        }
        Step::Count {
            template: tmpl,
            to,
            suffix,
            ms,
        } => {
            let Some(n) = template::render(to, facts) else {
                return vec![];
            };
            let filled = tmpl.replacen("{n}", &n, 1);
            let Some(mut line) = template::render(&filled, facts) else {
                return vec![];
            };
            line.push_str(suffix);
            let spans = if line.is_empty() {
                vec![]
            } else {
                vec![Span {
                    text: apply_case(machine, line),
                    style: Style::Normal,
                }]
            };
            let frames = n.parse::<u64>().ok().map_or_else(Vec::new, |target| {
                (0..COUNT_FRAMES - 1)
                    .filter_map(|k| {
                        let value = target * k / COUNT_FRAMES;
                        let filled = tmpl.replacen("{n}", &value.to_string(), 1);
                        let line = template::render(&filled, facts)?;
                        Some(if line.is_empty() {
                            vec![]
                        } else {
                            vec![Span {
                                text: apply_case(machine, line),
                                style: Style::Normal,
                            }]
                        })
                    })
                    .collect()
            });
            vec![AnimatedStep {
                ms: *ms,
                spans,
                kind: AnimatedKind::Count { frames },
            }]
        }
        Step::Findings { style, ms } => findings
            .iter()
            .filter_map(|f| finding_line(machine, facts, flavour, f, *style))
            .map(|spans| AnimatedStep {
                ms: *ms,
                spans,
                kind: AnimatedKind::Instant,
            })
            .collect(),
        Step::F1 { style, ms } => match f1_line(machine, facts, flavour, findings, *style) {
            Some(spans) => vec![AnimatedStep {
                ms: *ms,
                spans,
                kind: AnimatedKind::Instant,
            }],
            None => vec![],
        },
    }
}

/// Never two blank lines in a row, and never a leading or trailing blank line: the same rule
/// `collapse_blank_lines` applies to `layout`, applied here to each step's final spans.
fn collapse_blank_animated(steps: Vec<AnimatedStep>) -> Vec<AnimatedStep> {
    let mut collapsed: Vec<AnimatedStep> = Vec::with_capacity(steps.len());
    for step in steps {
        let previous_is_blank = collapsed.last().map(|s| is_blank(&s.spans)).unwrap_or(true);
        if is_blank(&step.spans) && previous_is_blank {
            continue;
        }
        collapsed.push(step);
    }
    while collapsed.last().is_some_and(|s| is_blank(&s.spans)) {
        collapsed.pop();
    }
    collapsed
}

/// The machine's steps resolved for the animated show: one `AnimatedStep` per visible logical
/// line, in the same order and under the same omission and blank-collapsing rules as `layout`,
/// whose final `spans` always agree with it.
pub fn animated_layout(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> Vec<AnimatedStep> {
    let steps: Vec<AnimatedStep> = machine
        .steps
        .iter()
        .flat_map(|step| animate_step(machine, facts, seed, step, flavour, findings))
        .collect();
    collapse_blank_animated(steps)
}

/// The plain rendering: every span painted in place, one line per row.
fn render_plain(machine: &Machine, mode: ColorMode, lines: &[Vec<Span>]) -> String {
    let mut out = String::new();
    for line in lines {
        for span in line {
            out.push_str(&paint(machine, mode, span.style, &span.text));
        }
        out.push('\n');
    }
    out
}

/// `width` cells of `rgb`, as a single coloured span, reset at the end.
fn bg_block(rgb: (u8, u8, u8), width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let (r, g, b) = rgb;
    format!("\x1b[48;2;{r};{g};{b}m{}\x1b[0m", " ".repeat(width))
}

/// `width` cells of fill: `bg_block` when `bg` is set, or plain spaces with no SGR when the
/// screen is a transparent painted one.
fn fill(bg: Option<(u8, u8, u8)>, width: usize) -> String {
    match bg {
        Some(rgb) => bg_block(rgb, width),
        None => " ".repeat(width),
    }
}

/// `text` painted in `rgb` as foreground, plus `bg` as background when it is set: foreground-only
/// SGR for a transparent painted screen.
fn fg_text(rgb: (u8, u8, u8), bg: Option<(u8, u8, u8)>, text: &str) -> String {
    let (r, g, b) = rgb;
    match bg {
        Some((br, bg_g, bb)) => {
            format!("\x1b[38;2;{r};{g};{b};48;2;{br};{bg_g};{bb}m{text}\x1b[0m")
        }
        None => format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m"),
    }
}

/// One row of a painted block, built from a logical line's spans: border pillars, background
/// padding, an optional logo box, the line's text (never truncated to make room for a badge)
/// filled out with background, the badge itself, the padding and pillars mirrored, then a
/// trailing reset.
///
/// `logo` is `Some((cell, kitty_escape))` for the first `logo_rows` text rows of a machine with a
/// logo: `cell` is the pre-rendered `logo_cols`-cell half-block row for that sprite row (absent in
/// Kitty mode, where the box is left as background), and `kitty_escape` is the transmission
/// escape, present only on the row that must emit it (the first text row, once). `logo_cols` is
/// the width of that box: 14 for the small grid, 28 for the wide one; ignored when `logo` is
/// `None`.
///
/// `badge`, when present, is right-aligned inside the `cols` text area, in the accent colour.
/// The row's own text is drawn at its full width first; if it ends within 2 cells of where the
/// badge would start, the badge is dropped for this row instead of the text being cut short.
///
/// `bg` is `None` for a transparent painted screen (`paint = true` with no `bg`): fill and
/// padding are then plain spaces with no SGR, and text and the badge use foreground-only SGR.
#[allow(clippy::too_many_arguments)]
fn painted_row(
    machine: &Machine,
    bg: Option<(u8, u8, u8)>,
    border: Option<(u8, u8, u8)>,
    pad_x: usize,
    cols: usize,
    spans: &[Span],
    logo: Option<(Option<&str>, Option<&str>)>,
    logo_cols: usize,
    badge: Option<&str>,
) -> String {
    let mut row = String::new();
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str(&fill(bg, pad_x));

    let shift = logo.is_some();
    if let Some((cell, kitty_escape)) = logo {
        if let Some(escape) = kitty_escape {
            row.push_str(escape);
        }
        match cell {
            Some(cell) => row.push_str(cell),
            None => row.push_str(&fill(bg, logo_cols)),
        }
        row.push_str(&fill(bg, LOGO_GAP));
    }
    let shift_offset = if shift { logo_cols + LOGO_GAP } else { 0 };
    let text_budget = cols.saturating_sub(shift_offset);

    let mut used = 0usize;
    for span in spans {
        if used >= text_budget {
            break;
        }
        let text: String = span.text.chars().take(text_budget - used).collect();
        if text.is_empty() {
            continue;
        }
        used += text.chars().count();
        let rgb = hex_rgb(colour_for(machine, span.style));
        row.push_str(&fg_text(rgb, bg, &text));
    }

    let text_end = shift_offset + used;
    let badge_len = badge.map(|b| b.chars().count()).unwrap_or(0);
    let badge_start = cols.saturating_sub(badge_len);
    let show_badge = badge.is_some() && text_end + 2 <= badge_start;
    if show_badge {
        let badge = badge.unwrap();
        if badge_start > text_end {
            row.push_str(&fill(bg, badge_start - text_end));
        }
        let rgb = hex_rgb(&machine.accent);
        row.push_str(&fg_text(rgb, bg, badge));
    } else if text_end < cols {
        row.push_str(&fill(bg, cols - text_end));
    }

    row.push_str(&fill(bg, pad_x));
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str("\x1b[0m");
    row
}

/// `cols + 2*pad_x`, plus 4 more when there is a border.
fn painted_total_width(machine: &Machine) -> u16 {
    machine.cols + 2 * u16::from(machine.pad_x) + if machine.border.is_some() { 4 } else { 0 }
}

/// The fixed gap, in cells, `painted_row` leaves between the logo box and where a line's own text
/// begins.
const LOGO_GAP: usize = 2;
const MIN_COLS_FOR_GRAPHICS: usize = 60;

/// The visible length of a logical line: the summed character count of its spans.
fn line_len(line: &[Span]) -> usize {
    line.iter().map(|span| span.text.chars().count()).sum()
}

/// True when a logo box `logo_cols` cells wide and `logo_rows` half-block rows tall, followed by
/// the fixed `LOGO_GAP`, still leaves every one of `lines`' first `logo_rows` lines room to fit
/// within `cols`, holding `pad_x` cells in reserve past the text: the same breathing room the row
/// already keeps on its left.
fn logo_box_fits(
    lines: &[Vec<Span>],
    logo_rows: usize,
    logo_cols: usize,
    cols: usize,
    pad_x: usize,
) -> bool {
    let budget = cols
        .saturating_sub(pad_x)
        .saturating_sub(logo_cols)
        .saturating_sub(LOGO_GAP);
    lines
        .iter()
        .take(logo_rows)
        .all(|line| line_len(line) <= budget)
}

/// The half-block grid chosen for a machine's logo, together with the footprint it occupies:
/// `cols` cells wide, `rows` half-block rows tall.
struct LogoPlan<'a> {
    grid: &'a crate::sprite::Grid,
    cols: usize,
    rows: usize,
}

/// Picks which of `sprite`'s two grids to draw: the wide 28 by 28 one when every line that would
/// sit beside it (per `logo_box_fits`) still fits, the original 14 by 14 one otherwise.
fn choose_logo_grid<'a>(
    sprite: &'a crate::sprite::Sprite,
    lines: &[Vec<Span>],
    cols: usize,
    pad_x: usize,
) -> LogoPlan<'a> {
    let wide_cols = sprite.grid_wide.cols();
    let wide_rows = sprite.grid_wide.half_rows();
    if logo_box_fits(lines, wide_rows, wide_cols, cols, pad_x) {
        LogoPlan {
            grid: &sprite.grid_wide,
            cols: wide_cols,
            rows: wide_rows,
        }
    } else {
        LogoPlan {
            grid: &sprite.grid,
            cols: sprite.grid.cols(),
            rows: sprite.grid.half_rows(),
        }
    }
}

/// The painted block: a border bar, `pad_y` blank rows, the text rows, `pad_y` more blank rows,
/// then another border bar. The logo (if any) is drawn over its first rows, as many as its chosen
/// grid is tall. The badge (if any) is right-aligned over the fill of the following few text
/// rows: badge line `i` on text row `i + 1`, so the badge starts on the second text row and never
/// competes with the header line for room.
fn render_painted(
    machine: &Machine,
    lines: &[Vec<Span>],
    graphics: Graphics,
    flavour: Option<&Flavour>,
) -> String {
    let bg = machine.bg.as_deref().map(hex_rgb);
    let border = machine.border.as_deref().map(hex_rgb);
    let pad_x = machine.pad_x as usize;
    let pad_y = machine.pad_y as usize;
    let cols = machine.cols as usize;
    let total_width = painted_total_width(machine) as usize;
    let blank: Vec<Span> = Vec::new();

    let sprite = logo_sprite(machine, flavour);
    let show_logo = graphics != Graphics::None && cols >= MIN_COLS_FOR_GRAPHICS && sprite.is_some();
    let logo_plan = show_logo
        .then(|| {
            sprite
                .as_ref()
                .map(|s| choose_logo_grid(s, lines, cols, pad_x))
        })
        .flatten();
    let sprite_rows = if graphics == Graphics::HalfBlocks {
        logo_plan
            .as_ref()
            .map(|plan| crate::sprite::half_blocks(plan.grid, bg))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let kitty_escape = if show_logo && graphics == Graphics::Kitty {
        Some(crate::sprite::kitty_image(
            sprite.as_ref().unwrap().png,
            14,
            7,
        ))
    } else {
        None
    };
    let logo_cols = logo_plan.as_ref().map_or(0, |p| p.cols);
    let logo_rows = logo_plan.as_ref().map_or(0, |p| p.rows);
    let show_badge =
        graphics != Graphics::None && cols >= MIN_COLS_FOR_GRAPHICS && !machine.badge.is_empty();

    let mut out = String::new();
    if let Some(border) = border {
        out.push_str(&bg_block(border, total_width));
        out.push_str("\x1b[0m\n");
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, &blank, None, 0, None,
        ));
        out.push('\n');
    }
    for (i, line) in lines.iter().enumerate() {
        let logo = if show_logo && i < logo_rows {
            let cell = sprite_rows.get(i).map(String::as_str);
            let escape = if i == 0 {
                kitty_escape.as_deref()
            } else {
                None
            };
            Some((cell, escape))
        } else {
            None
        };
        let badge = if show_badge && i >= 1 {
            machine.badge.get(i - 1).map(String::as_str)
        } else {
            None
        };
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, line, logo, logo_cols, badge,
        ));
        out.push('\n');
    }
    for _ in 0..pad_y {
        out.push_str(&painted_row(
            machine, bg, border, pad_x, cols, &blank, None, 0, None,
        ));
        out.push('\n');
    }
    if let Some(border) = border {
        out.push_str(&bg_block(border, total_width));
        out.push_str("\x1b[0m\n");
    }
    out
}

/// Precomputed geometry for rendering one logical line at a time: the same rules
/// `render_painted` and `render_plain` use for a whole screen, so the animated show (`show.rs`)
/// can redraw a single row, in an intermediate or final state, through the same code path.
pub struct RowGeometry<'a> {
    machine: &'a Machine,
    mode: ColorMode,
    painted: bool,
    bg: Option<(u8, u8, u8)>,
    border: Option<(u8, u8, u8)>,
    pad_x: usize,
    cols: usize,
    show_logo: bool,
    sprite_rows: Vec<String>,
    kitty_escape: Option<String>,
    logo_cols: usize,
    logo_rows: usize,
    show_badge: bool,
}

/// Builds the geometry for `machine`, choosing between the painted and plain paths under the
/// same rule `render_static` uses. `facts` and `seed` lay the screen out (the same way
/// `render_static` does) purely to learn each line's length, so the painted path can decide
/// whether the wide logo grid fits beside it; they play no other part here.
#[allow(clippy::too_many_arguments)]
pub fn row_geometry<'a>(
    machine: &'a Machine,
    facts: &Facts,
    seed: u64,
    mode: ColorMode,
    term_cols: Option<u16>,
    graphics: Graphics,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> RowGeometry<'a> {
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        let lines = layout(machine, facts, seed, flavour, findings);
        RowGeometry::build_painted(machine, &lines, graphics, flavour)
    } else {
        RowGeometry::build_plain(machine, mode)
    }
}

impl<'a> RowGeometry<'a> {
    fn build_painted(
        machine: &'a Machine,
        lines: &[Vec<Span>],
        graphics: Graphics,
        flavour: Option<&Flavour>,
    ) -> Self {
        let bg = machine.bg.as_deref().map(hex_rgb);
        let border = machine.border.as_deref().map(hex_rgb);
        let pad_x = machine.pad_x as usize;
        let cols = machine.cols as usize;
        let sprite = logo_sprite(machine, flavour);
        let show_logo =
            graphics != Graphics::None && cols >= MIN_COLS_FOR_GRAPHICS && sprite.is_some();
        let logo_plan = show_logo
            .then(|| {
                sprite
                    .as_ref()
                    .map(|s| choose_logo_grid(s, lines, cols, pad_x))
            })
            .flatten();
        let sprite_rows = if graphics == Graphics::HalfBlocks {
            logo_plan
                .as_ref()
                .map(|plan| crate::sprite::half_blocks(plan.grid, bg))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let kitty_escape = if show_logo && graphics == Graphics::Kitty {
            Some(crate::sprite::kitty_image(
                sprite.as_ref().unwrap().png,
                14,
                7,
            ))
        } else {
            None
        };
        let logo_cols = logo_plan.as_ref().map_or(0, |p| p.cols);
        let logo_rows = logo_plan.as_ref().map_or(0, |p| p.rows);
        let show_badge = graphics != Graphics::None
            && cols >= MIN_COLS_FOR_GRAPHICS
            && !machine.badge.is_empty();
        RowGeometry {
            machine,
            mode: ColorMode::TrueColor,
            painted: true,
            bg,
            border,
            pad_x,
            cols,
            show_logo,
            sprite_rows,
            kitty_escape,
            logo_cols,
            logo_rows,
            show_badge,
        }
    }

    fn build_plain(machine: &'a Machine, mode: ColorMode) -> Self {
        RowGeometry {
            machine,
            mode,
            painted: false,
            bg: None,
            border: None,
            pad_x: 0,
            cols: machine.cols as usize,
            show_logo: false,
            sprite_rows: Vec::new(),
            kitty_escape: None,
            logo_cols: 0,
            logo_rows: 0,
            show_badge: false,
        }
    }

    /// Whether this geometry uses the painted block path (border, padding, logo, badge) rather
    /// than the plain one.
    pub fn painted(&self) -> bool {
        self.painted
    }

    /// The top or bottom border bar, full width. `None` when not painted or the machine has no
    /// `border`.
    pub fn border_row(&self) -> Option<String> {
        self.border.map(|border| {
            let total_width = painted_total_width(self.machine) as usize;
            let mut row = bg_block(border, total_width);
            row.push_str("\x1b[0m");
            row
        })
    }

    /// A blank painted row: `pad_y` of these go above and below the text rows.
    pub fn pad_row(&self) -> String {
        painted_row(
            self.machine,
            self.bg,
            self.border,
            self.pad_x,
            self.cols,
            &[],
            None,
            0,
            None,
        )
    }

    /// Renders logical line `index` with `spans` as its current text: the same row
    /// `render_static` would draw for that line, letting the caller pass an intermediate state
    /// (a `Detect`'s label, a `Count`'s partial value) through the same layout as the final one.
    pub fn line(&self, index: usize, spans: &[Span]) -> String {
        if self.painted {
            let logo = if self.show_logo && index < self.logo_rows {
                let cell = self.sprite_rows.get(index).map(String::as_str);
                let escape = if index == 0 {
                    self.kitty_escape.as_deref()
                } else {
                    None
                };
                Some((cell, escape))
            } else {
                None
            };
            let badge = if self.show_badge && index >= 1 {
                self.machine.badge.get(index - 1).map(String::as_str)
            } else {
                None
            };
            painted_row(
                self.machine,
                self.bg,
                self.border,
                self.pad_x,
                self.cols,
                spans,
                logo,
                self.logo_cols,
                badge,
            )
        } else {
            let mut out = String::new();
            for span in spans {
                out.push_str(&paint(self.machine, self.mode, span.style, &span.text));
            }
            out
        }
    }
}

/// The finished screen as text. Every line ends with '\n'. `seed` picks the quip: start at
/// `seed % quips.len()`. `term_cols` is the terminal's width, when known: a painted machine uses
/// it to decide whether the block fits, falling back to the plain rendering when it does not (or
/// when the width is unknown, or the mode is not TrueColor). `graphics` chooses how a logo is
/// drawn in the painted path; the plain path never shows a logo or badge regardless of it.
/// `flavour`, for a flavoured machine, supplies its quips and its logo sprite; with `None` a
/// flavoured machine simply omits whatever it cannot resolve.
#[allow(clippy::too_many_arguments)]
pub fn render_static(
    machine: &Machine,
    facts: &Facts,
    mode: ColorMode,
    seed: u64,
    term_cols: Option<u16>,
    graphics: Graphics,
    flavour: Option<&Flavour>,
    findings: &[Finding],
) -> String {
    let lines = layout(machine, facts, seed, flavour, findings);
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        render_painted(machine, &lines, graphics, flavour)
    } else {
        render_plain(machine, mode, &lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{facts::Facts, machine};

    fn golden(id: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/golden/{id}.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// Strips `\x1b[...m` SGR sequences and `\x1b_G...\x1b\` kitty sequences, leaving only the
    /// visible characters.
    fn strip_ansi(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\x1b' && chars.get(i + 1) == Some(&'_') {
                i += 2;
                while i < chars.len() && !(chars[i] == '\x1b' && chars.get(i + 1) == Some(&'\\')) {
                    i += 1;
                }
                i += 2; // skip the ESC \ terminator
            } else if chars[i] == '\x1b' {
                i += 1;
                while i < chars.len() && chars[i] != 'm' {
                    i += 1;
                }
                i += 1; // skip the 'm'
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    fn visible_width(s: &str) -> usize {
        strip_ansi(s).chars().count()
    }

    fn unicorn_flavour() -> Flavour {
        crate::flavour::find("unicorn", None).unwrap()
    }

    fn sumo_flavour() -> Flavour {
        crate::flavour::find("sumo", None).unwrap()
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

    /// `Facts::fixture()` with `flavour`'s own slots applied, for tests that render a flavoured
    /// machine's wording rather than just checking that its quips are reachable.
    fn fixture_with_flavour(flavour: &Flavour) -> Facts {
        let mut facts = Facts::fixture();
        crate::flavour::apply(flavour, &mut facts);
        facts
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

    #[test]
    fn pc95_matches_golden() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        assert_eq!(
            render_static(
                &m,
                &facts,
                ColorMode::None,
                0,
                None,
                Graphics::None,
                Some(&flavour),
                &[]
            ),
            golden("pc95")
        );
    }
    #[test]
    fn unresolved_lines_vanish_and_blank_lines_collapse() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let f = Facts::fixture();
        let mut g = Facts::new();
        for k in ["date.year", "date.bios", "cpu.name", "cpu.cores", "mem.kb"] {
            g.insert(k, f.get(k).unwrap());
        }
        crate::flavour::apply(&flavour, &mut g);
        let out = render_static(
            &m,
            &g,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(!out.contains("Detecting Shell"));
        assert!(!out.contains("Boot streak"));
        assert!(out.contains("Detecting Horn"));
        assert!(!out.contains("\n\n\n"));
        assert!(!out.starts_with('\n') && !out.ends_with("\n\n"));
        assert!(!out.contains('{'));
    }
    #[test]
    fn empty_facts_never_leak_a_slot() {
        for m in machine::builtins() {
            let out = render_static(
                &m,
                &Facts::new(),
                ColorMode::None,
                0,
                None,
                Graphics::None,
                None,
                &[],
            );
            assert!(
                !out.contains('{') && !out.contains('}'),
                "{} leaked a slot",
                m.id
            );
        }
    }
    #[test]
    fn pc95_with_no_flavour_omits_the_firmware_line() {
        let m = machine::find("pc95", None).unwrap();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            0,
            None,
            Graphics::None,
            None,
            &[],
        );
        assert!(!out.contains('{'));
        assert!(!out.contains("Sparkle Modular BIOS"));
    }
    #[test]
    fn truecolor_wraps_spans_and_styles_only_the_detect_result() {
        let src = r##"
id = "t4"
name = "Test"
cols = 80
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
[[step]]
print = "Sparkle Modular BIOS"
style = "bright"
[[step]]
detect = "Detecting Horn"
result = "1 found"
style = "accent"
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::TrueColor,
            0,
            None,
            Graphics::None,
            None,
            &[],
        );
        assert!(out.contains("\x1b[38;2;255;255;255mSparkle Modular BIOS"));
        assert!(out.contains(
            "\x1b[38;2;170;170;170mDetecting Horn... \x1b[0m\x1b[38;2;255;255;85m1 found"
        ));
        assert!(!out.contains("\x1b[1m"));
    }
    #[test]
    fn ansi16_uses_basic_codes() {
        let src = r##"
id = "t5"
name = "Test"
cols = 80
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
[[step]]
print = "37748736K OK"
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::Ansi16,
            0,
            None,
            Graphics::None,
            None,
            &[],
        );
        assert!(out.contains("\x1b[37m37748736K OK\x1b[0m"));
    }
    #[test]
    fn seed_rotates_quips_and_skips_unresolvable_ones() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let out1 = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            1,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(out1.contains("Plug and Pray devices found: 1 unicorn"));
        // quip 7 needs {mem.kb}; without it the next resolvable quip is used
        let mut f = Facts::new();
        f.insert("date.year", "2026");
        let out7 = render_static(
            &m,
            &f,
            ColorMode::None,
            7,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(out7.contains("Floppy drive A: not found. Nobody is surprised."));
        let wrapped = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            flavour.quips.len() as u64,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(wrapped.contains("Turbo button engaged."));
    }
    #[test]
    fn color_mode_from_env_rules() {
        assert_eq!(
            color_mode_from_env(Some("1"), Some("truecolor")),
            ColorMode::None
        );
        assert_eq!(
            color_mode_from_env(Some(""), Some("truecolor")),
            ColorMode::TrueColor
        );
        assert_eq!(
            color_mode_from_env(None, Some("24bit")),
            ColorMode::TrueColor
        );
        assert_eq!(color_mode_from_env(None, None), ColorMode::Ansi16);
    }
    #[test]
    fn a_painted_bordered_uppercase_machine_is_44_wide_with_matching_colours() {
        let m = other_machine();
        let out = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::TrueColor,
            0,
            Some(80),
            Graphics::None,
            None,
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        assert!(!rows.is_empty());
        for row in &rows {
            assert_eq!(visible_width(row), 44, "row {row:?} is not 44 wide");
        }
        let first = rows.first().unwrap();
        let last = rows.last().unwrap();
        for border_row in [first, last] {
            assert!(border_row.contains("48;2;108;94;181"));
            assert!(!border_row.contains("38;2;"));
        }
        let text_row = rows.iter().find(|r| r.contains("READY")).unwrap();
        assert!(text_row.contains("38;2;108;94;181;48;2;53;40;121"));
    }
    #[test]
    fn a_painted_machine_falls_back_to_plain_truecolor_when_too_narrow_or_unknown() {
        let m = other_machine();
        for term_cols in [Some(30), None] {
            let out = render_static(
                &m,
                &Facts::fixture(),
                ColorMode::TrueColor,
                0,
                term_cols,
                Graphics::None,
                None,
                &[],
            );
            assert!(!out.contains("48;2;"));
        }
    }
    #[test]
    fn none_mode_with_paint_is_the_same_regardless_of_term_cols() {
        let m = other_machine();
        let with_cols = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            0,
            Some(80),
            Graphics::None,
            None,
            &[],
        );
        let without_cols = render_static(
            &m,
            &Facts::fixture(),
            ColorMode::None,
            0,
            None,
            Graphics::None,
            None,
            &[],
        );
        assert_eq!(with_cols, without_cols);
    }
    #[test]
    fn a_quip_longer_than_cols_is_skipped() {
        let src = r##"
id = "t3"
name = "Test"
cols = 20
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
quips = ["this quip is much too long to fit", "short"]
[[step]]
quip = true
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::None,
            0,
            None,
            Graphics::None,
            None,
            &[],
        );
        assert_eq!(out, "short\n");
    }

    #[test]
    fn painted_pc95_with_half_blocks_places_the_logo() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }
        assert!(rows[1].contains('\u{2580}'));
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        let pad_x = m.pad_x as usize;
        let byte_col = stripped[1].find("Sparkle Modular BIOS").unwrap();
        let text_col = stripped[1][..byte_col].chars().count();
        assert_eq!(text_col, pad_x + 16);
        assert!(stripped[1].contains("Sparkle Modular BIOS v1.985PG, An Enchantment Star Ally"));
        assert!(!out.contains("enchantment"));
    }

    /// The half-block rows a painted, half-block render draws its logo over: every row, in
    /// order from the top of the text area (`pad_y` blank rows down, with no border on either
    /// test machine), that carries an upper or lower half-block glyph.
    fn logo_row_count(out: &str, pad_y: usize) -> usize {
        out.lines()
            .skip(pad_y)
            .take_while(|row| row.contains('\u{2580}') || row.contains('\u{2584}'))
            .count()
    }

    #[test]
    fn pc95_is_too_narrow_for_the_wide_grid_and_falls_back_to_the_seven_row_mascot() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        assert_eq!(logo_row_count(&out, m.pad_y as usize), 7);
    }

    #[test]
    fn a_machine_wide_enough_for_the_wide_grid_chooses_the_fourteen_row_mascot() {
        let src = r##"
id = "widelogo"
name = "Wide"
cols = 140
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
paint = true
logo = "unicorn"
pad_x = 2
pad_y = 1
quips = ["a quip"]

[[step]]
print = "Line one"
[[step]]
print = "Line two"
[[step]]
print = "Line three"
[[step]]
print = "Line four"
[[step]]
print = "Line five"
[[step]]
print = "Line six"
[[step]]
print = "Line seven"
[[step]]
print = "Line eight"
[[step]]
print = "Line nine"
[[step]]
print = "Line ten"
[[step]]
print = "Line eleven"
[[step]]
print = "Line twelve"
[[step]]
print = "Line thirteen"
[[step]]
print = "Line fourteen"
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::TrueColor,
            0,
            Some(160),
            Graphics::HalfBlocks,
            None,
            &[],
        );
        assert_eq!(logo_row_count(&out, m.pad_y as usize), 14);
    }

    #[test]
    fn transparent_painted_pc95_has_no_background_but_keeps_geometry() {
        let m = machine::find("pc95", None).unwrap();
        assert_eq!(m.bg, None);
        assert!(m.paint);
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }
        // The screen fill colour must never appear. The one exception is a logo pixel where both
        // the upper and lower source pixels are opaque: that background belongs to the sprite,
        // not the screen, and only ever sits inside the 14-cell logo box.
        let pad_x = m.pad_x as usize;
        let pad_y = m.pad_y as usize;
        // pc95 is 80 columns wide, too narrow for the wide grid's longest line to fit beside it
        // (see `logo_box_fits`), so it always falls back to the small grid's 7 rows.
        let logo_rows = pad_y..pad_y + 7;
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
        assert!(rows[1].contains("\x1b[49m") || rows[1].contains("  "));
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        let pad_x = m.pad_x as usize;
        let byte_col = stripped[1].find("Sparkle Modular BIOS").unwrap();
        let text_col = stripped[1][..byte_col].chars().count();
        assert_eq!(text_col, pad_x + 16);
    }

    #[test]
    fn a_painted_machine_places_its_badge_from_the_second_row() {
        let src = r##"
id = "tbadgelogo"
name = "Test"
cols = 80
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
bg = "#000000"
paint = true
logo = "unicorn"
badge = ["enchantment", "*STAR* ALLY", "GLITTER SAFE"]
[[step]]
print = "Header line"
[[step]]
print = "Second line"
[[step]]
print = "Third line"
[[step]]
print = "Fourth line"
[[step]]
print = "Fifth line"
"##;
        let m = machine::parse(src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
            None,
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        assert!(stripped[2].trim_end().ends_with("enchantment"));
        assert!(stripped[3].trim_end().ends_with("*STAR* ALLY"));
        assert!(stripped[4].trim_end().ends_with("GLITTER SAFE"));
    }

    #[test]
    fn a_badge_line_is_dropped_rather_than_truncating_the_row_it_collides_with() {
        let long_line = "A".repeat(60);
        let src = format!(
            r##"
id = "tbadge"
name = "Test"
cols = 60
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
bg = "#000000"
paint = true
badge = ["ABCDE"]
[[step]]
print = "Header"
[[step]]
print = "{long_line}"
"##
        );
        let m = machine::parse(&src).unwrap();
        let out = render_static(
            &m,
            &Facts::new(),
            ColorMode::TrueColor,
            0,
            Some(80),
            Graphics::HalfBlocks,
            None,
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        // rows[0] is the pad_y blank row; rows[1] is "Header" (no badge target); rows[2] is the
        // long line, which is the badge's target row and collides with it.
        assert!(stripped[2].contains(&long_line));
        assert!(!stripped[2].contains("ABCDE"));
    }

    #[test]
    fn painted_pc95_with_kitty_emits_one_image_and_no_half_blocks() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::Kitty,
            Some(&flavour),
            &[],
        );
        assert_eq!(out.matches("\x1b_Ga=T").count(), 1);
        assert!(!out.contains('\u{2580}'));
    }

    #[test]
    fn painted_pc95_kitty_embeds_the_flavours_own_sprite() {
        let m = machine::find("pc95", None).unwrap();
        for id in ["unicorn", "sumo"] {
            let flavour = crate::flavour::find(id, None).unwrap();
            let png = crate::sprite::builtin(id).unwrap().png;
            let facts = fixture_with_flavour(&flavour);
            let out = render_static(
                &m,
                &facts,
                ColorMode::TrueColor,
                0,
                Some(100),
                Graphics::Kitty,
                Some(&flavour),
                &[],
            );
            let escape = crate::sprite::kitty_image(&png[..30], 1, 1);
            let body = escape
                .strip_prefix("\x1b_Ga=T,f=100,q=2,C=1,c=1,r=1,m=0;")
                .and_then(|s| s.strip_suffix("\x1b\\"))
                .unwrap();
            assert!(out.contains(body), "{id}'s sprite bytes were not embedded");
        }
    }

    #[test]
    fn painted_pc95_with_graphics_none_has_no_logo_or_badge() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(!out.contains("\x1b_G"));
        assert!(!out.contains('\u{2580}'));
        assert!(!out.contains("enchantment"));
    }

    #[test]
    fn plain_pc95_has_no_logo_or_badge() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            None,
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        assert!(!out.contains("\x1b_G"));
        assert!(!out.contains('\u{2580}'));
        assert!(!out.contains("enchantment"));
    }

    #[test]
    fn pc95_with_sumo_shows_sumo_wording_and_hides_unicorn_wording() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = sumo_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        assert!(out.contains("Yokozuna Modular BIOS v1.991, Immovable"));
        assert!(out.contains("Stance: low. Centre of gravity: lower."));
        assert!(out.contains("DOHYO-15-SUMO-1991RING-00"));
        assert!(!out.to_lowercase().contains("horn"));
        assert!(!out.to_lowercase().contains("unicorn"));
    }

    #[test]
    fn a_flavoured_detect_label_is_padded_to_detect_width_like_a_literal_one() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = sumo_flavour();
        let facts = fixture_with_flavour(&flavour);
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        let expected = format!("{:<27}... {}", "Detecting Salt", "1 handful (thrown)");
        assert!(out.contains(&expected));
    }

    #[test]
    fn animated_layout_final_spans_agree_with_layout() {
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let m = machine::find("pc95", None).unwrap();
        let lines = layout(&m, &facts, 0, Some(&flavour), &[]);
        let animated = animated_layout(&m, &facts, 0, Some(&flavour), &[]);
        let animated_spans: Vec<Vec<Span>> = animated.into_iter().map(|s| s.spans).collect();
        assert_eq!(lines, animated_spans, "pc95 disagrees on its final lines");

        let m = other_machine();
        let lines = layout(&m, &Facts::fixture(), 0, None, &[]);
        let animated = animated_layout(&m, &Facts::fixture(), 0, None, &[]);
        let animated_spans: Vec<Vec<Span>> = animated.into_iter().map(|s| s.spans).collect();
        assert_eq!(lines, animated_spans, "other disagrees on its final lines");
    }

    #[test]
    fn animated_count_steps_grow_from_zero_to_the_final_value() {
        let m = machine::find("pc95", None).unwrap();
        let animated = animated_layout(&m, &Facts::fixture(), 0, None, &[]);
        let count = animated
            .iter()
            .find(|s| matches!(s.kind, AnimatedKind::Count { .. }))
            .unwrap();
        let AnimatedKind::Count { frames } = &count.kind else {
            unreachable!()
        };
        assert!(frames.len() >= 10, "expected several distinct frames");
        assert_eq!(frames[0][0].text, "Memory Testing : 0K");
        assert_eq!(count.spans[0].text, "Memory Testing : 37748736K OK");
    }

    #[test]
    fn row_geometry_line_matches_render_static_for_pc95() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let lines = layout(&m, &facts, 0, Some(&flavour), &[]);
        let geometry = row_geometry(
            &m,
            &facts,
            0,
            ColorMode::TrueColor,
            Some(100),
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        let mut rebuilt = String::new();
        if let Some(row) = geometry.border_row() {
            rebuilt.push_str(&row);
            rebuilt.push('\n');
        }
        for _ in 0..m.pad_y {
            rebuilt.push_str(&geometry.pad_row());
            rebuilt.push('\n');
        }
        for (i, line) in lines.iter().enumerate() {
            rebuilt.push_str(&geometry.line(i, line));
            rebuilt.push('\n');
        }
        for _ in 0..m.pad_y {
            rebuilt.push_str(&geometry.pad_row());
            rebuilt.push('\n');
        }
        if let Some(row) = geometry.border_row() {
            rebuilt.push_str(&row);
            rebuilt.push('\n');
        }
        let expected = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            0,
            Some(100),
            Graphics::HalfBlocks,
            Some(&flavour),
            &[],
        );
        assert_eq!(rebuilt, expected);
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

    /// `Facts::fixture()` plus every slot the fixture findings' phrasing needs.
    fn facts_with_findings() -> Facts {
        let mut facts = Facts::fixture();
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
    fn pc95_findings_matches_golden_for_both_flavours() {
        let m = machine::find("pc95", None).unwrap();
        for (golden_id, flavour) in [
            ("pc95-findings", unicorn_flavour()),
            ("pc95-findings-sumo", sumo_flavour()),
        ] {
            let mut facts = facts_with_findings();
            crate::flavour::apply(&flavour, &mut facts);
            let out = render_static(
                &m,
                &facts,
                ColorMode::None,
                0,
                None,
                Graphics::None,
                Some(&flavour),
                &fixture_findings(),
            );
            assert_eq!(out, golden(golden_id), "{golden_id} disagrees");
        }
    }

    #[test]
    fn phrasing_prefers_flavour_falls_back_to_machine_and_is_none_for_unknown_id() {
        let m = machine::find("pc95", None).unwrap();
        let sumo = sumo_flavour();
        assert_eq!(
            phrasing(&m, Some(&sumo), "boot_order"),
            Some("Bout order: {boot.devices}")
        );
        // unicorn deliberately carries no findings table, so this falls back to the machine's own.
        let unicorn = unicorn_flavour();
        assert_eq!(
            phrasing(&m, Some(&unicorn), "boot_order"),
            Some("Boot device order: {boot.devices}")
        );
        assert_eq!(phrasing(&m, Some(&sumo), "nope"), None);
    }

    #[test]
    fn an_unflavoured_machine_ignores_a_flavours_findings_table() {
        let m = other_machine();
        assert!(!m.flavoured);
        let sumo = sumo_flavour();
        assert_eq!(phrasing(&m, Some(&sumo), "boot_order"), None);
    }

    #[test]
    fn a_finding_with_an_unresolved_slot_is_omitted_while_others_render() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = facts_with_findings();
        facts.remove("irq.age");
        crate::flavour::apply(&flavour, &mut facts);
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &fixture_findings(),
        );
        assert!(!out.contains("IRQ conflict"));
        assert!(out.contains("Boot device order"));
        assert!(out.contains("Virus scan"));
    }

    #[test]
    fn a_finding_with_no_phrasing_at_all_is_omitted() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = facts_with_findings();
        crate::flavour::apply(&flavour, &mut facts);
        let mut findings = fixture_findings();
        findings.push(finding("mystery_check", Severity::Info, &[]));
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &findings,
        );
        assert!(!out.to_lowercase().contains("mystery"));
        assert!(out.contains("Boot device order"));
    }

    #[test]
    fn an_over_long_finding_is_truncated_to_exactly_cols() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = Facts::fixture();
        facts.insert("boot.devices", "a".repeat(200));
        crate::flavour::apply(&flavour, &mut facts);
        let findings = vec![finding("boot_order", Severity::Info, &[])];
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &findings,
        );
        let line = out
            .lines()
            .find(|l| l.starts_with("Boot device order:"))
            .unwrap();
        assert_eq!(line.chars().count(), m.cols as usize);
    }

    #[test]
    fn a_finding_that_already_fits_is_unchanged_with_no_marker() {
        let m = machine::find("pc95", None).unwrap();
        let mut facts = Facts::fixture();
        facts.insert("virus.repo", "eko-pro");
        facts.insert("virus.file", ".env.local");
        let f = finding("virus_one", Severity::Fail, &[]);
        let text = finding_text(&m, &facts, None, &f).unwrap();
        assert_eq!(
            text,
            "Virus scan: eko-pro has .env.local tracked by git. Quarantine advised."
        );
        assert!(!text.contains(".."));
    }

    #[test]
    fn a_finding_with_one_very_long_slot_value_fits_exactly_cols_and_keeps_its_final_sentence() {
        let m = machine::find("pc95", None).unwrap();
        let mut facts = Facts::fixture();
        facts.insert("virus.repo", "r".repeat(100));
        facts.insert("virus.file", ".env.local");
        let f = finding("virus_one", Severity::Fail, &[]);
        let text = finding_text(&m, &facts, None, &f).unwrap();
        assert_eq!(text.chars().count(), m.cols as usize);
        assert!(text.ends_with("tracked by git. Quarantine advised."));
        assert!(text.contains(".."));
    }

    #[test]
    fn the_longest_of_two_slot_values_is_the_one_shortened() {
        let m = machine::find("pc95", None).unwrap();
        let mut facts = Facts::fixture();
        facts.insert("virus.repo", "z".repeat(30));
        facts.insert("virus.file", "shortfile1");
        let f = finding("virus_one", Severity::Fail, &[]);
        let text = finding_text(&m, &facts, None, &f).unwrap();
        assert_eq!(
            text,
            format!(
                "Virus scan: {} has shortfile1 tracked by git. Quarantine advised.",
                "z".repeat(15) + ".."
            )
        );
    }

    #[test]
    fn a_value_is_never_shortened_below_its_floor_and_falls_back_to_whole_line_truncation() {
        let mut facts = Facts::new();
        facts.insert("a", "abcdefghij");
        // At the floor (3 visible characters plus `..`), the value already fits exactly.
        let at_floor = shorten_finding_line("{a}", &facts, 5).unwrap();
        assert_eq!(at_floor, "abc..");
        // One column tighter than the floor allows, shrinking cannot help any further, so the
        // whole line is truncated instead, exactly as it was before this change.
        let below_floor = shorten_finding_line("{a}", &facts, 4).unwrap();
        assert_eq!(below_floor, "abc.");
    }

    #[test]
    fn multi_byte_characters_are_shortened_by_character_count_not_bytes() {
        let mut facts = Facts::new();
        facts.insert("a", "café".repeat(10));
        let text = shorten_finding_line("{a}", &facts, 20).unwrap();
        assert_eq!(text.chars().count(), 20);
        assert!(text.ends_with(".."));
    }

    #[test]
    fn f1_resume_wins_when_boot_device_is_present() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = Facts::fixture();
        facts.insert("boot.device", "eko-pro");
        crate::flavour::apply(&flavour, &mut facts);
        let findings = vec![finding("boot_order", Severity::Info, &[])];
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &findings,
        );
        assert!(out.contains("Press F1 to continue, or bios resume to boot eko-pro."));
    }

    #[test]
    fn f1_appears_when_a_warn_is_present_and_boot_device_is_absent() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        assert_eq!(facts.get("boot.device"), None);
        let findings = vec![finding("irq_conflict", Severity::Warn, &[])];
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &findings,
        );
        assert!(out.contains("Press F1 to continue."));
    }

    #[test]
    fn no_f1_line_when_only_info_findings_fired_and_boot_device_is_absent() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        assert_eq!(facts.get("boot.device"), None);
        let findings = vec![finding("boot_order", Severity::Info, &[])];
        let out = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &findings,
        );
        assert!(!out.contains("Press F1"));
    }
}
