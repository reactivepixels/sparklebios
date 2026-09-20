//! Static rendering and colour modes.

use crate::checks::{Finding, Severity};
use crate::facts::Facts;
use crate::flavour::Flavour;
use crate::machine::{Machine, Step, Style};
use crate::template;

/// How much colour the terminal supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// No colour at all.
    None,
    /// The 16 ANSI colours.
    Ansi16,
    /// Full 24-bit colour.
    TrueColor,
}

/// Whether the logo is drawn in the painted path, and how. Plain and unpainted output never show
/// a logo or a badge, regardless of this setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graphics {
    /// No mascot: no terminal support, or the user turned it off.
    None,
    /// The mascot image, transmitted through the Kitty graphics protocol.
    Kitty,
    /// The mascot image, transmitted through the iTerm2 inline image protocol.
    Iterm,
}

impl Graphics {
    /// Whether a mascot image is drawn at all. The two protocols differ only in the bytes they
    /// emit, so every layout decision asks this rather than naming one of them.
    pub fn draws_image(self) -> bool {
        matches!(self, Graphics::Kitty | Graphics::Iterm)
    }

    /// The escape that transmits `png` into a `cols` by `rows` cell box, in whichever protocol
    /// this is. `None` when no image is drawn.
    fn image_escape(self, png: &[u8], cols: u16, rows: u16) -> Option<String> {
        match self {
            Graphics::None => None,
            Graphics::Kitty => Some(crate::sprite::kitty_image(png, cols, rows)),
            Graphics::Iterm => Some(crate::sprite::iterm_image(png, cols, rows)),
        }
    }
}

/// One resolved, styled run of text within a logical line.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    /// The run's text, after template substitution.
    pub text: String,
    /// Which colour to draw it in.
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
/// Total: anything shorter, non-UTF-8 at that range, or not a hex pair falls back to `0` a
/// channel at a time, so a malformed colour never panics the render path.
fn hex_rgb(colour: &str) -> (u8, u8, u8) {
    let bytes = colour.as_bytes();
    let byte = |i: usize| {
        bytes
            .get(i..i + 2)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            .unwrap_or(0)
    };
    (byte(1), byte(3), byte(5))
}

/// Wraps `text` in the escape codes for `style`, or returns it unchanged for `ColorMode::None`.
///
/// The styles are attributes on the terminal's own foreground, never a fixed colour. A painted
/// screen with no background of its own sits on whatever theme is loaded, so text painted a fixed
/// near-white disappeared on a light theme: Paper White's background is `#efede6`, and the
/// firmware and copyright lines were being drawn `#FFFFFF` on top of it. Bold and faint read
/// correctly on a light theme and a dark one, and the accent uses the palette's own yellow rather
/// than a hardcoded one, so it follows the theme too.
fn paint(mode: ColorMode, style: Style, text: &str) -> String {
    match mode {
        ColorMode::None => text.to_string(),
        ColorMode::TrueColor | ColorMode::Ansi16 => {
            format!("\x1b[{}m{text}\x1b[0m", style_code(style))
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

/// A single candidate's own resolve-and-fit rule: its slots all resolve against `facts`, and the
/// rendered result is no longer than `cols`. The same rule `resolve_quip` applies to each of its
/// candidates in turn, used here for the one calendar candidate a day may bring.
fn resolve_one(candidate: &str, cols: u16, facts: &Facts) -> Option<String> {
    let text = template::render(candidate, facts)?;
    (text.chars().count() <= cols as usize).then_some(text)
}

/// The line a `Quip` step actually shows: `calendar_line` (a machine's own calendar text, still
/// carrying its `{slot}`s) when one is active and it resolves and fits `cols`, otherwise the
/// ordinary quip rotation, exactly as if no calendar line existed. This is the one place a `Quip`
/// step is ever resolved, reached by both the animated show and the static render, so a calendar
/// day can never look different depending on which of the two drew it. Whether `calendar_line` is
/// `Some` at all is entirely the caller's decision (`boot.rs`, which knows whether this is a Full
/// boot, a preview, a Fast boot or Quiet): this function never asks which show it is in.
fn resolve_quip_line(
    calendar_line: Option<&str>,
    quips: &[String],
    cols: u16,
    facts: &Facts,
    seed: u64,
) -> Option<String> {
    calendar_line
        .and_then(|candidate| resolve_one(candidate, cols, facts))
        .or_else(|| resolve_quip(quips, cols, facts, seed))
}

/// The sprite drawn as a machine's logo, if any, as its PNG bytes: for a flavoured machine, the
/// current flavour's own sprite; for any other machine, its `logo` key (only `"unicorn"` exists
/// today).
fn logo_sprite(machine: &Machine, flavour: Option<&Flavour>) -> Option<&'static [u8]> {
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
#[allow(clippy::too_many_arguments)]
fn layout_step(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    step: &Step,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
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
            let quips = quips_for(machine, flavour);
            match resolve_quip_line(calendar_line, quips, machine.cols, facts, seed) {
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
/// logo; ignored otherwise. `calendar_line`, when given, is tried ahead of the ordinary quip
/// rotation for the `quip` step; see `resolve_quip_line`.
pub fn layout(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
) -> Vec<Vec<Span>> {
    let lines: Vec<Vec<Span>> = machine
        .steps
        .iter()
        .flat_map(|step| layout_step(machine, facts, seed, step, flavour, findings, calendar_line))
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
    Detect {
        /// The label plus its trailing `"... "`, shown before the result arrives.
        label_spans: Vec<Span>,
    },
    /// Counts up through `frames` before settling on the final state: a `Count` step. Empty
    /// when the target does not parse as a number, in which case the step behaves like
    /// `Instant`.
    Count {
        /// The growing frames the count animates through, ending on the final state.
        frames: Vec<Vec<Span>>,
    },
}

/// One step, resolved for the animated show: its final spans (`spans`, identical to what
/// `layout` produces for the same step), how long to hold on it in milliseconds before speed is
/// applied (`ms`), and how it gets there (`kind`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedStep {
    /// How long to hold on this step, in milliseconds, before speed is applied.
    pub ms: u64,
    /// The step's final spans, identical to what `layout` produces for the same step.
    pub spans: Vec<Span>,
    /// How this step gets to its final state.
    pub kind: AnimatedKind,
}

/// Roughly how many redraws a `Count` step plays out over its `ms`: the growing frames plus the
/// final, suffixed value.
const COUNT_FRAMES: u64 = 24;

/// Resolves a single step for the animated show, or an empty `Vec` to omit it. Mirrors
/// `layout_step` exactly for the final state (`AnimatedStep::spans`), so `layout` and
/// `animated_layout` always agree on which lines are visible and what they finally say.
/// `findings` and `f1` are always `AnimatedKind::Instant`, one `AnimatedStep` per finding line.
#[allow(clippy::too_many_arguments)]
fn animate_step(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    step: &Step,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
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
            let quips = quips_for(machine, flavour);
            match resolve_quip_line(calendar_line, quips, machine.cols, facts, seed) {
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
/// whose final `spans` always agree with it. `calendar_line` is forwarded to the `quip` step
/// exactly as `layout` does; see `resolve_quip_line`.
pub fn animated_layout(
    machine: &Machine,
    facts: &Facts,
    seed: u64,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
) -> Vec<AnimatedStep> {
    let steps: Vec<AnimatedStep> = machine
        .steps
        .iter()
        .flat_map(|step| animate_step(machine, facts, seed, step, flavour, findings, calendar_line))
        .collect();
    collapse_blank_animated(steps)
}

/// The plain rendering: every span painted in place, one line per row.
fn render_plain(mode: ColorMode, lines: &[Vec<Span>]) -> String {
    let mut out = String::new();
    for line in lines {
        for span in line {
            out.push_str(&paint(mode, span.style, &span.text));
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
/// The SGR attributes for a text style: bold, faint, or bold plus the palette's own yellow. Never
/// a fixed colour, so the text follows whatever theme the terminal is running. See `paint`.
fn style_code(style: Style) -> &'static str {
    match style {
        Style::Normal => "2",
        Style::Bright => "1",
        Style::Accent => "1;33",
    }
}

/// The machine's own colour for a style, used only where the machine paints its own background.
fn colour_for(machine: &Machine, style: Style) -> &str {
    match style {
        Style::Normal => &machine.fg,
        Style::Bright => &machine.bright,
        Style::Accent => &machine.accent,
    }
}

/// A run of styled text inside a painted row.
///
/// Which foreground to use depends on whether the machine brought its own background. A machine
/// that paints one has chosen both halves of the pair and its colours are the only ones known to
/// have contrast against it, so they are used. A transparent machine is sitting on whatever theme
/// the terminal is running, and there its own colours are a liability: `pc95` painted its header
/// `#FFFFFF`, which is invisible on Paper White's `#efede6` background. There the style is an
/// attribute on the terminal's own foreground instead.
fn styled_text(machine: &Machine, style: Style, bg: Option<(u8, u8, u8)>, text: &str) -> String {
    match bg {
        Some((br, bg_g, bb)) => {
            let (r, g, b) = hex_rgb(colour_for(machine, style));
            format!("\x1b[38;2;{r};{g};{b};48;2;{br};{bg_g};{bb}m{text}\x1b[0m")
        }
        None => format!("\x1b[{}m{text}\x1b[0m", style_code(style)),
    }
}

/// One row of a painted block, built from a logical line's spans: border pillars, background
/// padding, an optional logo box, the line's text (never truncated to make room for a badge)
/// filled out with background, the badge itself, the padding and pillars mirrored, then a
/// trailing reset.
///
/// `logo` is `Some(kitty_escape)` for the first `logo_rows` text rows of a machine with a logo
/// box on screen: the box itself is always left as background (the mascot is an actual image,
/// drawn by the terminal over those cells), and `kitty_escape`, when present, is the transmission
/// escape for the image, emitted only on the row that must send it (the first text row, once).
/// `logo_cols` is the width of that box; ignored when `logo` is `None`.
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
    logo: Option<Option<&str>>,
    logo_cols: usize,
    badge: Option<&str>,
) -> String {
    let mut row = String::new();
    if let Some(border) = border {
        row.push_str(&bg_block(border, 2));
    }
    row.push_str(&fill(bg, pad_x));

    let shift = logo.is_some();
    if let Some(kitty_escape) = logo {
        if let Some(escape) = kitty_escape {
            row.push_str(escape);
        }
        row.push_str(&fill(bg, logo_cols));
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
        row.push_str(&styled_text(machine, span.style, bg, &text));
    }

    let text_end = shift_offset + used;
    let badge_len = badge.map_or(0, |b| b.chars().count());
    let badge_start = cols.saturating_sub(badge_len);
    let fitting_badge = badge.filter(|_| text_end + 2 <= badge_start);
    if let Some(badge) = fitting_badge {
        if badge_start > text_end {
            row.push_str(&fill(bg, badge_start - text_end));
        }
        row.push_str(&styled_text(machine, Style::Accent, bg, badge));
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

/// The mascot's fixed footprint on a painted screen: the width, in cells, and the height, in
/// text rows, of the box the Kitty image is transmitted into.
const MASCOT_COLS: usize = 14;
const MASCOT_ROWS: usize = 7;

/// The painted block: a border bar, `pad_y` blank rows, the text rows, `pad_y` more blank rows,
/// then another border bar. The logo (if any) is drawn over its first `MASCOT_ROWS` rows. The
/// badge (if any) is right-aligned over the fill of the following few text rows: badge line `i`
/// on text row `i + 1`, so the badge starts on the second text row and never competes with the
/// header line for room.
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
    let visible_sprite = (graphics.draws_image() && cols >= MIN_COLS_FOR_GRAPHICS)
        .then_some(sprite)
        .flatten();
    let show_logo = visible_sprite.is_some();
    let kitty_escape = visible_sprite.map(|sprite| {
        graphics
            .image_escape(sprite, MASCOT_COLS as u16, MASCOT_ROWS as u16)
            .unwrap_or_default()
    });
    let logo_cols = if show_logo { MASCOT_COLS } else { 0 };
    let logo_rows = if show_logo { MASCOT_ROWS } else { 0 };
    let show_badge =
        graphics.draws_image() && cols >= MIN_COLS_FOR_GRAPHICS && !machine.badge.is_empty();

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
            let escape = if i == 0 {
                kitty_escape.as_deref()
            } else {
                None
            };
            Some(escape)
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
    kitty_escape: Option<String>,
    /// Whether `line` has already emitted `kitty_escape` once. The escape transmits the whole
    /// mascot image; the image itself only needs to be sent once for the life of a show, not
    /// once per redraw of the row it sits in (a row can redraw many times over a show, e.g. the
    /// shimmer sweeping across it), so `line` embeds it on the first call for the logo row and
    /// never again after, however many more times that row is asked for.
    kitty_sent: std::cell::Cell<bool>,
    logo_cols: usize,
    logo_rows: usize,
    show_badge: bool,
}

/// Builds the geometry for `machine`, choosing between the painted and plain paths under the
/// same rule `render_static` uses.
pub fn row_geometry<'a>(
    machine: &'a Machine,
    mode: ColorMode,
    term_cols: Option<u16>,
    graphics: Graphics,
    flavour: Option<&Flavour>,
) -> RowGeometry<'a> {
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        RowGeometry::build_painted(machine, graphics, flavour)
    } else {
        RowGeometry::build_plain(machine, mode)
    }
}

impl<'a> RowGeometry<'a> {
    fn build_painted(machine: &'a Machine, graphics: Graphics, flavour: Option<&Flavour>) -> Self {
        let bg = machine.bg.as_deref().map(hex_rgb);
        let border = machine.border.as_deref().map(hex_rgb);
        let pad_x = machine.pad_x as usize;
        let cols = machine.cols as usize;
        let sprite = logo_sprite(machine, flavour);
        let visible_sprite = (graphics.draws_image() && cols >= MIN_COLS_FOR_GRAPHICS)
            .then_some(sprite)
            .flatten();
        let show_logo = visible_sprite.is_some();
        let kitty_escape = visible_sprite.map(|sprite| {
            graphics
                .image_escape(sprite, MASCOT_COLS as u16, MASCOT_ROWS as u16)
                .unwrap_or_default()
        });
        let logo_cols = if show_logo { MASCOT_COLS } else { 0 };
        let logo_rows = if show_logo { MASCOT_ROWS } else { 0 };
        let show_badge =
            graphics.draws_image() && cols >= MIN_COLS_FOR_GRAPHICS && !machine.badge.is_empty();
        RowGeometry {
            machine,
            mode: ColorMode::TrueColor,
            painted: true,
            bg,
            border,
            pad_x,
            cols,
            show_logo,
            kitty_escape,
            kitty_sent: std::cell::Cell::new(false),
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
            kitty_escape: None,
            kitty_sent: std::cell::Cell::new(false),
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

    /// The visible column, within a row this geometry draws, where logical line `index`'s own
    /// text begins: past any border, the left padding, and, for a row inside the logo box, the
    /// logo image and its fixed gap. Zero for unpainted geometry, which has none of those. Used
    /// by the sprinkle effects (`sprinkles.rs`) to place a highlight against a line's text
    /// without knowing the painted block's own layout.
    pub fn text_start_col(&self, index: usize) -> usize {
        if !self.painted {
            return 0;
        }
        let border_and_pad = if self.border.is_some() { 2 } else { 0 } + self.pad_x;
        if self.show_logo && index < self.logo_rows {
            border_and_pad + self.logo_cols + LOGO_GAP
        } else {
            border_and_pad
        }
    }

    /// The blank margin columns immediately beside the logo box on row `index`: the left padding
    /// (between the border and the logo) and the fixed gap (between the logo and the line's own
    /// text), never a column over the logo image or the text itself. Empty whenever `index` is
    /// not one of the rows the logo box actually covers.
    pub fn twinkle_margin_cols(&self, index: usize) -> Vec<usize> {
        if !(self.painted && self.show_logo && index < self.logo_rows) {
            return Vec::new();
        }
        let border = if self.border.is_some() { 2 } else { 0 };
        let pad_zone = border..(border + self.pad_x);
        let gap_start = border + self.pad_x + self.logo_cols;
        let gap_zone = gap_start..(gap_start + LOGO_GAP);
        pad_zone.chain(gap_zone).collect()
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
    ///
    /// The very first call for the logo row (row 0, when a mascot is shown) is the one and only
    /// time this embeds the Kitty transmission escape: every later call for that same row, from
    /// any caller, for any reason (the show's own redraws, or an effect like the shimmer sweeping
    /// across it), renders it as plain background fill instead, never repeating the image bytes.
    pub fn line(&self, index: usize, spans: &[Span]) -> String {
        if self.painted {
            let logo = if self.show_logo && index < self.logo_rows {
                let escape = if index == 0 && !self.kitty_sent.replace(true) {
                    self.kitty_escape.as_deref()
                } else {
                    None
                };
                Some(escape)
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
                out.push_str(&paint(self.mode, span.style, &span.text));
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
/// flavoured machine simply omits whatever it cannot resolve. Never shows a calendar line; see
/// `render_static_with_calendar` for the entry point that can.
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
    render_static_with_calendar(
        machine, facts, mode, seed, term_cols, graphics, flavour, findings, None,
    )
}

/// `render_static`, but the `quip` step may show `calendar_line` instead: a machine's own
/// calendar text (still carrying its `{slot}`s, unrendered), tried ahead of the ordinary quip
/// rotation under the same resolve-and-fit rule every quip candidate follows, so an unresolved or
/// over-long calendar line falls back to an ordinary quip rather than vanishing. This is the same
/// `layout` call, and so the same `Step::Quip` handling, that `show::play` reaches through
/// `render::animated_layout`: the one place a calendar line is ever chosen, whichever of the two
/// shows it. `render_static` is this function called with `None`, so a caller with nothing to say
/// about a calendar line never has to think about one. Whether `calendar_line` is `Some` at all is
/// entirely `boot.rs`'s call: this function only ever obeys what it is given.
#[allow(clippy::too_many_arguments)]
pub fn render_static_with_calendar(
    machine: &Machine,
    facts: &Facts,
    mode: ColorMode,
    seed: u64,
    term_cols: Option<u16>,
    graphics: Graphics,
    flavour: Option<&Flavour>,
    findings: &[Finding],
    calendar_line: Option<&str>,
) -> String {
    let lines = layout(machine, facts, seed, flavour, findings, calendar_line);
    let painted = machine.paint
        && mode == ColorMode::TrueColor
        && term_cols.is_some_and(|w| w >= painted_total_width(machine));
    if painted {
        render_painted(machine, &lines, graphics, flavour)
    } else {
        render_plain(mode, &lines)
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
        // Attributes on the terminal's own foreground, never a fixed colour, so the screen
        // reads correctly on a light theme as well as a dark one.
        assert!(out.contains("\x1b[1mSparkle Modular BIOS"));
        assert!(out.contains("\x1b[2mDetecting Horn... \x1b[0m\x1b[1;33m1 found"));
        assert!(
            !out.contains("38;2;"),
            "a fixed colour leaked into the text styles"
        );
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
        assert!(out.contains("\x1b[2m37748736K OK\x1b[0m"));
        assert!(
            !out.contains("\x1b[37m"),
            "still emitting a fixed basic colour"
        );
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
    fn painted_pc95_with_kitty_places_the_logo() {
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
        let rows: Vec<&str> = out.lines().collect();
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }
        assert_eq!(out.matches("\x1b_Ga=T").count(), 1);
        let stripped: Vec<String> = rows.iter().map(|r| strip_ansi(r)).collect();
        let pad_x = m.pad_x as usize;
        let byte_col = stripped[1].find("Sparkle Modular BIOS").unwrap();
        let text_col = stripped[1][..byte_col].chars().count();
        assert_eq!(text_col, pad_x + 16);
        assert!(stripped[1].contains("Sparkle Modular BIOS v1.985PG, An Enchantment Star Ally"));
        assert!(!out.contains("enchantment"));
    }

    /// The mascot box is a fixed footprint now, never a wide-vs-narrow choice: it stays 14 cells
    /// by 7 rows whether the machine is pc95, 80 columns wide, or a machine wide enough that an
    /// old, retired wide grid would once have been chosen instead.
    #[test]
    fn the_mascot_box_is_a_fixed_footprint_regardless_of_machine_width() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let geometry = row_geometry(
            &m,
            ColorMode::TrueColor,
            Some(100),
            Graphics::Kitty,
            Some(&flavour),
        );
        let pad_x = m.pad_x as usize;
        assert_eq!(geometry.text_start_col(6), pad_x + 14 + 2);
        assert_eq!(geometry.text_start_col(7), pad_x);

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
"##;
        let m = machine::parse(src).unwrap();
        let geometry = row_geometry(&m, ColorMode::TrueColor, Some(160), Graphics::Kitty, None);
        let pad_x = m.pad_x as usize;
        assert_eq!(geometry.text_start_col(6), pad_x + 14 + 2);
        assert_eq!(geometry.text_start_col(7), pad_x);
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
            Graphics::Kitty,
            Some(&flavour),
            &[],
        );
        let rows: Vec<&str> = out.lines().collect();
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }
        // The screen fill colour must never appear anywhere: a transparent painted screen never
        // emits a background SGR, and the mascot box, drawn as an actual image, never emits one
        // either.
        for (i, row) in rows.iter().enumerate() {
            assert!(
                !row.contains("48;2;"),
                "row {i} paints a background colour: {row:?}"
            );
        }
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
            Graphics::Kitty,
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
            Graphics::Kitty,
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

    /// The iTerm2 path draws the same layout through a different protocol: one OSC 1337, no Kitty
    /// escape anywhere, and the text still starting at the same column as the Kitty path, since
    /// the mascot box is the same size whichever protocol fills it.
    #[test]
    fn painted_pc95_with_iterm_emits_one_osc_image_and_no_kitty_escape() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let render = |g| {
            render_static(
                &m,
                &facts,
                ColorMode::TrueColor,
                0,
                Some(100),
                g,
                Some(&flavour),
                &[],
            )
        };
        let out = render(Graphics::Iterm);
        assert_eq!(out.matches("\x1b]1337;File=").count(), 1);
        assert!(
            !out.contains("\x1b_G"),
            "emitted a Kitty escape on the iTerm2 path"
        );
        assert!(out.contains("width=14;height=7"));

        // Same layout: strip each protocol's own image bytes and the rows must match.
        let strip = |s: &str, start: &str, end: &str| {
            let mut out = String::new();
            let mut rest = s;
            while let Some(i) = rest.find(start) {
                out.push_str(&rest[..i]);
                let after = &rest[i..];
                match after.find(end) {
                    Some(j) => rest = &after[j + end.len()..],
                    None => {
                        rest = "";
                        break;
                    }
                }
            }
            out.push_str(rest);
            out
        };
        let iterm_stripped = strip(&out, "\x1b7\x1b]1337;File=", "\x07\x1b8");
        let kitty_stripped = strip(&render(Graphics::Kitty), "\x1b_G", "\x1b\\");
        assert_eq!(iterm_stripped, kitty_stripped);
    }

    #[test]
    fn painted_pc95_kitty_embeds_the_flavours_own_sprite() {
        let m = machine::find("pc95", None).unwrap();
        for id in ["unicorn", "sumo"] {
            let flavour = crate::flavour::find(id, None).unwrap();
            let png = crate::sprite::builtin(id).unwrap();
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

    /// With no image support (`Graphics::None` on a painted, wide-enough screen), there is no
    /// blank column left where the mascot used to be: the header line starts right at the block's
    /// own left pad, the same fixed offset a painted screen with no mascot at all has always used,
    /// derived here straight from the rendered text rather than from any internal layout constant.
    #[test]
    fn painted_pc95_with_no_image_support_has_no_blank_indent() {
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
        let rows: Vec<&str> = out.lines().collect();
        let header = strip_ansi(rows[m.pad_y as usize]);
        let header_col = header.find("Sparkle Modular BIOS").unwrap();
        assert_eq!(header_col, m.pad_x as usize);
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
            Graphics::Kitty,
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
        let lines = layout(&m, &facts, 0, Some(&flavour), &[], None);
        let animated = animated_layout(&m, &facts, 0, Some(&flavour), &[], None);
        let animated_spans: Vec<Vec<Span>> = animated.into_iter().map(|s| s.spans).collect();
        assert_eq!(lines, animated_spans, "pc95 disagrees on its final lines");

        let m = other_machine();
        let lines = layout(&m, &Facts::fixture(), 0, None, &[], None);
        let animated = animated_layout(&m, &Facts::fixture(), 0, None, &[], None);
        let animated_spans: Vec<Vec<Span>> = animated.into_iter().map(|s| s.spans).collect();
        assert_eq!(lines, animated_spans, "other disagrees on its final lines");
    }

    #[test]
    fn animated_count_steps_grow_from_zero_to_the_final_value() {
        let m = machine::find("pc95", None).unwrap();
        let animated = animated_layout(&m, &Facts::fixture(), 0, None, &[], None);
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
        let lines = layout(&m, &facts, 0, Some(&flavour), &[], None);
        let geometry = row_geometry(
            &m,
            ColorMode::TrueColor,
            Some(100),
            Graphics::Kitty,
            Some(&flavour),
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
            Graphics::Kitty,
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

    /// The quip index pinned for `pc95-painted.txt`: seed 6 lands on the unicorn flavour's
    /// "Shadow RAM enabled. Shadow unicorn also enabled.", picked once and fixed so the golden's
    /// quip line never drifts to a different one as the flavour's list grows.
    const PC95_PAINTED_SEED: u64 = 6;

    /// Pins the painted layout with no mascot drawn (the default shape once nothing supports
    /// images): findings, the badge-free block, and the border are all still exercised, but the
    /// mascot itself is not, since embedding its PNG as a base64 blob would make the golden an
    /// opaque, fragile copy of the image bytes rather than a layout worth reviewing. The Kitty
    /// escape itself is covered directly by `painted_pc95_with_kitty_places_the_logo` and
    /// `painted_pc95_kitty_embeds_the_flavours_own_sprite`.
    /// A transparent painted screen must not name a colour for its text. `pc95` has no background
    /// of its own, so it sits on whatever theme is loaded, and a fixed near-white header is
    /// invisible on a light one: Paper White's background is `#efede6`. The styles are attributes
    /// on the terminal's own foreground instead.
    #[test]
    fn a_transparent_painted_screen_styles_text_without_naming_a_colour() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = facts_with_findings();
        crate::flavour::apply(&flavour, &mut facts);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            PC95_PAINTED_SEED,
            Some(100),
            Graphics::None,
            Some(&flavour),
            &fixture_findings(),
        );
        assert!(
            !out.contains("38;2;"),
            "a fixed foreground colour leaked into a transparent screen"
        );
        assert!(out.contains("\x1b[1m"), "the bright rows are not bold");
        assert!(out.contains("\x1b[2m"), "the normal rows are not faint");
    }

    #[test]
    fn pc95_painted_matches_golden() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = facts_with_findings();
        crate::flavour::apply(&flavour, &mut facts);
        assert_eq!(
            render_static(
                &m,
                &facts,
                ColorMode::TrueColor,
                PC95_PAINTED_SEED,
                Some(100),
                Graphics::None,
                Some(&flavour),
                &fixture_findings(),
            ),
            golden("pc95-painted")
        );
    }

    /// A second, narrower check on the same screen as `pc95_painted_matches_golden`: with no
    /// mascot drawn, there is no reserved logo box left behind. Every text row's own text starts
    /// at the same fixed left pad, derived straight from the rendered output, never at the wider
    /// offset a leftover box would leave.
    #[test]
    fn painted_pc95_with_no_mascot_shape_is_pinned() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = facts_with_findings();
        crate::flavour::apply(&flavour, &mut facts);
        let out = render_static(
            &m,
            &facts,
            ColorMode::TrueColor,
            PC95_PAINTED_SEED,
            Some(100),
            Graphics::None,
            Some(&flavour),
            &fixture_findings(),
        );
        let rows: Vec<&str> = out.lines().collect();

        // Every painted row fills the same total display width.
        let width = visible_width(rows[0]);
        for row in &rows {
            assert_eq!(visible_width(row), width, "row {row:?} is not {width} wide");
        }

        let pad_x = m.pad_x as usize;
        let text_cols: Vec<usize> = rows
            .iter()
            .skip(m.pad_y as usize)
            .map(|r| strip_ansi(r))
            .filter_map(|row| row.chars().position(|c| c != ' '))
            .collect();
        assert!(!text_cols.is_empty());
        assert!(
            text_cols.iter().all(|&c| c == pad_x),
            "a row's text did not start at the left pad: {text_cols:?}"
        );
    }

    #[test]
    fn text_start_col_is_zero_when_unpainted() {
        let m = other_machine();
        let geometry = RowGeometry::build_plain(&m, ColorMode::None);
        assert_eq!(geometry.text_start_col(0), 0);
        assert!(geometry.twinkle_margin_cols(0).is_empty());
    }

    #[test]
    fn text_start_col_and_margin_cols_sit_either_side_of_the_pc95_logo_box() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let geometry = row_geometry(
            &m,
            ColorMode::TrueColor,
            Some(100),
            Graphics::Kitty,
            Some(&flavour),
        );
        assert!(geometry.painted());
        // Row 0 (the firmware line) sits inside the logo box: its text starts past the mascot box
        // (14 cells) plus the fixed gap (2 cells), and pad_x (2) is the left margin.
        assert_eq!(geometry.text_start_col(0), 2 + 14 + 2);
        assert_eq!(geometry.twinkle_margin_cols(0), vec![0, 1, 16, 17]);
        // Well past the logo box, there is no margin left to twinkle in and the text starts
        // right after the left padding.
        assert_eq!(geometry.text_start_col(20), 2);
        assert!(geometry.twinkle_margin_cols(20).is_empty());
    }

    /// With no mascot drawn (`Graphics::None`), there is no margin beside it to twinkle in, on
    /// any row, including one that would have sat inside the logo box had the mascot been drawn
    /// (row 0, see `text_start_col_and_margin_cols_sit_either_side_of_the_pc95_logo_box`).
    #[test]
    fn twinkle_margin_cols_is_empty_with_graphics_none_even_inside_the_would_be_logo_box() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let geometry = row_geometry(
            &m,
            ColorMode::TrueColor,
            Some(100),
            Graphics::None,
            Some(&flavour),
        );
        assert!(geometry.painted());
        assert_eq!(geometry.text_start_col(0), 2);
        assert!(geometry.twinkle_margin_cols(0).is_empty());
    }

    // --- Calendar lines -------------------------------------------------------------------------

    #[test]
    fn resolve_quip_line_prefers_a_calendar_candidate_that_resolves_and_fits() {
        let facts = Facts::fixture();
        let quips = vec!["ordinary quip".to_string()];
        let resolved = resolve_quip_line(
            Some("Year {date.year} rollover complete. Nothing caught fire. Again."),
            &quips,
            80,
            &facts,
            0,
        );
        assert_eq!(
            resolved.as_deref(),
            Some("Year 2026 rollover complete. Nothing caught fire. Again.")
        );
    }

    #[test]
    fn resolve_quip_line_falls_back_to_the_ordinary_quip_when_the_calendar_slot_does_not_resolve() {
        // `y2038.days` resolves in the fixture today, so the failing case has to be built by
        // hand: remove the one slot the 19 January line needs.
        let mut facts = Facts::fixture();
        assert!(facts.get("y2038.days").is_some());
        facts.remove("y2038.days");
        let quips = vec!["ordinary quip".to_string()];
        let resolved = resolve_quip_line(
            Some("Y2038 check: {y2038.days} days until 32-bit time runs out. Noted."),
            &quips,
            80,
            &facts,
            0,
        );
        assert_eq!(resolved.as_deref(), Some("ordinary quip"));
    }

    #[test]
    fn resolve_quip_line_falls_back_to_the_ordinary_quip_when_the_calendar_candidate_is_too_long() {
        let facts = Facts::fixture();
        // 15 columns: room enough for the ordinary quip (13 characters), not for the calendar
        // candidate.
        let quips = vec!["ordinary quip".to_string()];
        let resolved = resolve_quip_line(
            Some("this calendar line is far too long to fit in the space given here"),
            &quips,
            15,
            &facts,
            0,
        );
        assert_eq!(resolved.as_deref(), Some("ordinary quip"));
    }

    #[test]
    fn resolve_quip_line_with_no_calendar_candidate_is_the_ordinary_rotation() {
        let facts = Facts::fixture();
        let quips = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        for seed in 0..quips.len() as u64 {
            assert_eq!(
                resolve_quip_line(None, &quips, 80, &facts, seed),
                resolve_quip(&quips, 80, &facts, seed)
            );
        }
    }

    /// `render_static` (the calendar-oblivious entry point `src/setup/mod.rs` still calls) and
    /// `render_static_with_calendar` called with `None` must never disagree: the whole point of
    /// keeping both is that a caller with nothing to say about a calendar line gets exactly the
    /// old behaviour.
    #[test]
    fn render_static_is_render_static_with_calendar_given_none() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let facts = fixture_with_flavour(&flavour);
        let plain = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        let explicit_none = render_static_with_calendar(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
            None,
        );
        assert_eq!(plain, explicit_none);
    }

    /// The static path (`render_static_with_calendar`, what `bios boot` falls back to under
    /// `NO_COLOR`, a non-tty, or `--no-animate`, per `docs/machines.md`), on a real calendar date
    /// (1 January), shows the calendar line in place of the quip, with no row added or removed.
    #[test]
    fn a_calendar_day_replaces_the_quip_on_the_static_path_with_no_row_added() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = fixture_with_flavour(&flavour);
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-01");
        let calendar_line = crate::machine::matching_calendar_text(&m, 2026, 1, 1);
        assert_eq!(
            calendar_line,
            Some("Year {date.year} rollover complete. Nothing caught fire. Again.")
        );

        let geometry = (ColorMode::None, None, Graphics::None);
        let ordinary = render_static(
            &m,
            &facts,
            geometry.0,
            0,
            geometry.1,
            geometry.2,
            Some(&flavour),
            &[],
        );
        let with_calendar = render_static_with_calendar(
            &m,
            &facts,
            geometry.0,
            0,
            geometry.1,
            geometry.2,
            Some(&flavour),
            &[],
            calendar_line,
        );

        assert_eq!(
            ordinary.lines().count(),
            with_calendar.lines().count(),
            "a calendar line must replace the quip row, never add one"
        );
        assert!(with_calendar.contains("Year 2026 rollover complete. Nothing caught fire. Again."));
        assert!(!ordinary.contains("Year 2026 rollover complete"));
    }

    /// The omission rule still holds for a calendar line: on 19 January, with the one slot its
    /// text needs deliberately missing, the screen falls back to the ordinary quip, byte for byte
    /// identical to a screen with no calendar override at all, rather than dropping the row.
    #[test]
    fn an_unresolved_calendar_line_falls_back_to_the_ordinary_quip_on_the_static_path() {
        let m = machine::find("pc95", None).unwrap();
        let flavour = unicorn_flavour();
        let mut facts = fixture_with_flavour(&flavour);
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-01-19");
        facts.remove("y2038.days");
        let calendar_line = crate::machine::matching_calendar_text(&m, 2026, 1, 19);
        assert_eq!(
            calendar_line,
            Some("Y2038 check: {y2038.days} days until 32-bit time runs out. Noted.")
        );

        let ordinary = render_static(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
        );
        let with_calendar = render_static_with_calendar(
            &m,
            &facts,
            ColorMode::None,
            0,
            None,
            Graphics::None,
            Some(&flavour),
            &[],
            calendar_line,
        );
        assert_eq!(
            with_calendar, ordinary,
            "an unresolved calendar line must fall back to the ordinary quip exactly"
        );
        assert!(!with_calendar.contains("Y2038 check"));
    }
}
