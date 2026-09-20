//! Machine TOML schema, validation, built-ins, lookup.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    #[default]
    Normal,
    Bright,
    Accent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Print {
        text: String,
        style: Style,
        ms: u64,
    },
    Count {
        template: String,
        to: String,
        suffix: String,
        ms: u64,
    },
    Detect {
        label: String,
        result: String,
        style: Style,
        ms: u64,
    },
    Quip {
        style: Style,
        ms: u64,
    },
    Findings {
        style: Style,
        ms: u64,
    },
    F1 {
        style: Style,
        ms: u64,
    },
}

/// One row of a machine's `[[calendar]]` table: a line that replaces the quip, on the day it
/// names, in the Full show and in the preview. Exactly one of `month` plus `day`, `weekday` plus
/// `day`, or `day_of_year` says which day that is.
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarRule {
    pub month: Option<u32>,
    pub day: Option<u32>,
    pub weekday: Option<String>,
    pub day_of_year: Option<u32>,
    pub text: String,
}

impl CalendarRule {
    /// Whether this rule fires for `(year, month, day)`.
    pub fn matches(&self, year: i32, month: u32, day: u32) -> bool {
        if let (Some(rule_month), Some(rule_day)) = (self.month, self.day) {
            return month == rule_month && day == rule_day;
        }
        if let (Some(rule_weekday), Some(rule_day)) = (&self.weekday, self.day) {
            return day == rule_day && crate::clock::weekday_name(year, month, day) == rule_weekday;
        }
        if let Some(rule_day_of_year) = self.day_of_year {
            return crate::clock::day_of_year(year, month, day) == rule_day_of_year;
        }
        false
    }
}

/// The text of the first of `machine`'s calendar rules that fires for `(year, month, day)`, if
/// any.
pub fn matching_calendar_text(machine: &Machine, year: i32, month: u32, day: u32) -> Option<&str> {
    machine
        .calendar
        .iter()
        .find(|rule| rule.matches(year, month, day))
        .map(|rule| rule.text.as_str())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub cols: u16,
    pub fg: String,
    pub bright: String,
    pub accent: String,
    pub bg: Option<String>,
    pub paint: bool,
    pub border: Option<String>,
    pub pad_x: u8,
    pub pad_y: u8,
    pub uppercase: bool,
    pub logo: Option<String>,
    pub badge: Vec<String>,
    pub quips: Vec<String>,
    pub flavoured: bool,
    pub detect_width: Option<u16>,
    pub findings: BTreeMap<String, String>,
    pub calendar: Vec<CalendarRule>,
    pub steps: Vec<Step>,
}

#[derive(Debug, PartialEq)]
pub enum MachineError {
    Parse(String),
    Invalid(String),
}

impl std::fmt::Display for MachineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MachineError::Parse(msg) => write!(f, "parse error: {msg}"),
            MachineError::Invalid(msg) => write!(f, "invalid machine: {msg}"),
        }
    }
}

impl std::error::Error for MachineError {}

#[derive(Debug, Deserialize)]
struct RawMachine {
    id: String,
    name: String,
    cols: u16,
    fg: String,
    bright: String,
    accent: String,
    bg: Option<String>,
    #[serde(default)]
    paint: bool,
    border: Option<String>,
    #[serde(default = "default_pad_x")]
    pad_x: u8,
    #[serde(default = "default_pad_y")]
    pad_y: u8,
    #[serde(default)]
    uppercase: bool,
    logo: Option<String>,
    #[serde(default)]
    badge: Vec<String>,
    #[serde(default)]
    quips: Vec<String>,
    #[serde(default)]
    flavoured: bool,
    detect_width: Option<u16>,
    #[serde(default)]
    findings: BTreeMap<String, String>,
    #[serde(rename = "calendar", default)]
    calendar: Vec<RawCalendarRule>,
    #[serde(rename = "step", default)]
    steps: Vec<RawStep>,
}

fn default_pad_x() -> u8 {
    2
}

fn default_pad_y() -> u8 {
    1
}

#[derive(Debug, Deserialize)]
struct RawCalendarRule {
    text: String,
    month: Option<u32>,
    day: Option<u32>,
    weekday: Option<String>,
    day_of_year: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawStep {
    print: Option<String>,
    count: Option<String>,
    detect: Option<String>,
    quip: Option<bool>,
    findings: Option<bool>,
    f1: Option<bool>,
    to: Option<String>,
    suffix: Option<String>,
    result: Option<String>,
    style: Option<String>,
    ms: Option<u64>,
}

const PC95_TOML: &str = include_str!("../machines/pc95.toml");

pub fn parse(src: &str) -> Result<Machine, MachineError> {
    let raw: RawMachine = toml::from_str(src).map_err(|e| MachineError::Parse(e.to_string()))?;
    validate(raw)
}

/// Built-in machines: just `pc95`, the one screen SparkleBIOS ships. Skipped (yielding an empty
/// list) if it somehow fails to parse.
pub fn builtins() -> Vec<Machine> {
    [PC95_TOML]
        .into_iter()
        .filter_map(|src| parse(src).ok())
        .collect()
}

/// `<user_dir>/<id>.toml` wins over a built-in of the same id. Unreadable or invalid user files are ignored.
pub fn find(id: &str, user_dir: Option<&std::path::Path>) -> Option<Machine> {
    if !valid_id(id) {
        return None;
    }
    if let Some(dir) = user_dir {
        let path = dir.join(format!("{id}.toml"));
        if let Ok(src) = std::fs::read_to_string(&path) {
            if let Ok(m) = parse(&src) {
                if m.id == id {
                    return Some(m);
                }
            }
        }
    }
    builtins().into_iter().find(|m| m.id == id)
}

/// Built-ins plus valid user machines, user files replacing built-ins with the same id.
pub fn list(user_dir: Option<&std::path::Path>) -> Vec<Machine> {
    let mut machines = builtins();
    if let Some(dir) = user_dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for path in paths {
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                let Ok(src) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(m) = parse(&src) else {
                    continue;
                };
                if let Some(existing) = machines.iter_mut().find(|b| b.id == m.id) {
                    *existing = m;
                } else {
                    machines.push(m);
                }
            }
        }
    }
    machines
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn valid_colour(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// A findings key: `[a-z0-9_]+`. An unknown id is still valid, since a machine may carry
/// phrasing for a check that does not exist yet.
fn valid_finding_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn validate_findings(findings: &BTreeMap<String, String>) -> Result<(), MachineError> {
    for (key, value) in findings {
        if !valid_finding_key(key) {
            return Err(MachineError::Invalid(format!(
                "findings key {key:?} does not match [a-z0-9_]+"
            )));
        }
        if value.is_empty() {
            return Err(MachineError::Invalid(format!(
                "findings value for {key:?} is empty"
            )));
        }
    }
    Ok(())
}

const WEEKDAY_NAMES: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

fn valid_weekday(weekday: &str) -> bool {
    WEEKDAY_NAMES.contains(&weekday)
}

/// A `[[calendar]]` row has `text` plus exactly one of `month` and `day`, `weekday` and `day`, or
/// `day_of_year` alone.
fn validate_calendar_rule(idx: usize, raw: RawCalendarRule) -> Result<CalendarRule, MachineError> {
    if raw.text.is_empty() {
        return Err(MachineError::Invalid(format!(
            "calendar {idx}: text is empty"
        )));
    }
    let shapes = [
        raw.month.is_some()
            && raw.day.is_some()
            && raw.weekday.is_none()
            && raw.day_of_year.is_none(),
        raw.weekday.is_some()
            && raw.day.is_some()
            && raw.month.is_none()
            && raw.day_of_year.is_none(),
        raw.day_of_year.is_some()
            && raw.month.is_none()
            && raw.day.is_none()
            && raw.weekday.is_none(),
    ];
    if shapes.iter().filter(|present| **present).count() != 1 {
        return Err(MachineError::Invalid(format!(
            "calendar {idx}: must have exactly one of month plus day, weekday plus day, or day_of_year"
        )));
    }
    if let Some(month) = raw.month {
        if !(1..=12).contains(&month) {
            return Err(MachineError::Invalid(format!(
                "calendar {idx}: month {month} is not between 1 and 12"
            )));
        }
    }
    if let Some(day) = raw.day {
        if !(1..=31).contains(&day) {
            return Err(MachineError::Invalid(format!(
                "calendar {idx}: day {day} is not between 1 and 31"
            )));
        }
    }
    if let Some(weekday) = &raw.weekday {
        if !valid_weekday(weekday) {
            return Err(MachineError::Invalid(format!(
                "calendar {idx}: weekday {weekday:?} is not a day of the week"
            )));
        }
    }
    if let Some(day_of_year) = raw.day_of_year {
        if !(1..=366).contains(&day_of_year) {
            return Err(MachineError::Invalid(format!(
                "calendar {idx}: day_of_year {day_of_year} is not between 1 and 366"
            )));
        }
    }
    Ok(CalendarRule {
        month: raw.month,
        day: raw.day,
        weekday: raw.weekday,
        day_of_year: raw.day_of_year,
        text: raw.text,
    })
}

fn parse_style(raw: Option<&str>, idx: usize) -> Result<Style, MachineError> {
    match raw {
        None => Ok(Style::Normal),
        Some("normal") => Ok(Style::Normal),
        Some("bright") => Ok(Style::Bright),
        Some("accent") => Ok(Style::Accent),
        Some(other) => Err(MachineError::Invalid(format!(
            "step {idx}: unknown style {other:?}"
        ))),
    }
}

fn validate(raw: RawMachine) -> Result<Machine, MachineError> {
    if !valid_id(&raw.id) {
        return Err(MachineError::Invalid(format!(
            "id {:?} does not match [a-z0-9_-]+",
            raw.id
        )));
    }
    if !(20..=200).contains(&raw.cols) {
        return Err(MachineError::Invalid(format!(
            "cols {} is not between 20 and 200",
            raw.cols
        )));
    }
    if !valid_colour(&raw.fg) {
        return Err(MachineError::Invalid(format!(
            "fg {:?} is not a #RRGGBB colour",
            raw.fg
        )));
    }
    if !valid_colour(&raw.bright) {
        return Err(MachineError::Invalid(format!(
            "bright {:?} is not a #RRGGBB colour",
            raw.bright
        )));
    }
    if !valid_colour(&raw.accent) {
        return Err(MachineError::Invalid(format!(
            "accent {:?} is not a #RRGGBB colour",
            raw.accent
        )));
    }
    if let Some(bg) = &raw.bg {
        if !valid_colour(bg) {
            return Err(MachineError::Invalid(format!(
                "bg {bg:?} is not a #RRGGBB colour"
            )));
        }
    }
    if let Some(border) = &raw.border {
        if !raw.paint {
            return Err(MachineError::Invalid(
                "border requires paint to be true".into(),
            ));
        }
        if raw.bg.is_none() {
            return Err(MachineError::Invalid("border requires bg to be set".into()));
        }
        if !valid_colour(border) {
            return Err(MachineError::Invalid(format!(
                "border {border:?} is not a #RRGGBB colour"
            )));
        }
    }
    if !(0..=8).contains(&raw.pad_x) {
        return Err(MachineError::Invalid(format!(
            "pad_x {} is not between 0 and 8",
            raw.pad_x
        )));
    }
    if !(0..=4).contains(&raw.pad_y) {
        return Err(MachineError::Invalid(format!(
            "pad_y {} is not between 0 and 4",
            raw.pad_y
        )));
    }
    if let Some(logo) = &raw.logo {
        if logo != "unicorn" {
            return Err(MachineError::Invalid(format!(
                "logo {logo:?} is not a known logo"
            )));
        }
    }
    if raw.badge.len() > 4 {
        return Err(MachineError::Invalid(format!(
            "badge has {} lines, more than 4",
            raw.badge.len()
        )));
    }
    for line in &raw.badge {
        if line.chars().count() > 20 {
            return Err(MachineError::Invalid(format!(
                "badge line {line:?} is longer than 20 chars"
            )));
        }
    }
    if let Some(width) = raw.detect_width {
        if !(4..=60).contains(&width) {
            return Err(MachineError::Invalid(format!(
                "detect_width {width} is not between 4 and 60"
            )));
        }
    }
    validate_findings(&raw.findings)?;
    let calendar = raw
        .calendar
        .into_iter()
        .enumerate()
        .map(|(idx, rule)| validate_calendar_rule(idx, rule))
        .collect::<Result<Vec<_>, _>>()?;
    if raw.steps.is_empty() {
        return Err(MachineError::Invalid("machine has no steps".into()));
    }

    let mut steps = Vec::with_capacity(raw.steps.len());
    for (idx, s) in raw.steps.into_iter().enumerate() {
        let kinds = [
            s.print.is_some(),
            s.count.is_some(),
            s.detect.is_some(),
            s.quip.is_some(),
            s.findings.is_some(),
            s.f1.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if kinds != 1 {
            return Err(MachineError::Invalid(format!(
                "step {idx}: must have exactly one of print, count, detect, quip, findings, f1"
            )));
        }
        let style = parse_style(s.style.as_deref(), idx)?;
        let step = if let Some(text) = s.print {
            Step::Print {
                text,
                style,
                ms: s.ms.unwrap_or(45),
            }
        } else if let Some(template) = s.count {
            let to = s
                .to
                .ok_or_else(|| MachineError::Invalid(format!("step {idx}: count requires to")))?;
            Step::Count {
                template,
                to,
                suffix: s.suffix.unwrap_or_default(),
                ms: s.ms.unwrap_or(600),
            }
        } else if let Some(label) = s.detect {
            let result = s.result.ok_or_else(|| {
                MachineError::Invalid(format!("step {idx}: detect requires result"))
            })?;
            Step::Detect {
                label,
                result,
                style,
                ms: s.ms.unwrap_or(190),
            }
        } else if s.quip.is_some() {
            match s.quip {
                Some(true) => {
                    if raw.quips.is_empty() && !raw.flavoured {
                        return Err(MachineError::Invalid(format!(
                            "step {idx}: quip requires the machine to have quips"
                        )));
                    }
                    Step::Quip {
                        style,
                        ms: s.ms.unwrap_or(120),
                    }
                }
                _ => {
                    return Err(MachineError::Invalid(format!(
                        "step {idx}: quip must be true"
                    )))
                }
            }
        } else if s.findings.is_some() {
            match s.findings {
                Some(true) => Step::Findings {
                    style,
                    ms: s.ms.unwrap_or(90),
                },
                _ => {
                    return Err(MachineError::Invalid(format!(
                        "step {idx}: findings must be true"
                    )))
                }
            }
        } else {
            match s.f1 {
                Some(true) => Step::F1 {
                    style,
                    ms: s.ms.unwrap_or(45),
                },
                _ => {
                    return Err(MachineError::Invalid(format!(
                        "step {idx}: f1 must be true"
                    )))
                }
            }
        };
        steps.push(step);
    }

    Ok(Machine {
        id: raw.id,
        name: raw.name,
        cols: raw.cols,
        fg: raw.fg,
        bright: raw.bright,
        accent: raw.accent,
        bg: raw.bg,
        paint: raw.paint,
        border: raw.border,
        pad_x: raw.pad_x,
        pad_y: raw.pad_y,
        uppercase: raw.uppercase,
        logo: raw.logo,
        badge: raw.badge,
        quips: raw.quips,
        flavoured: raw.flavoured,
        detect_width: raw.detect_width,
        findings: raw.findings,
        calendar,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r##"
id = "t1"
name = "Test"
cols = 80
fg = "#AAAAAA"
bright = "#FFFFFF"
accent = "#FFFF55"
[[step]]
print = "hello"
"##;

    #[test]
    fn parses_a_minimal_machine_with_defaults() {
        let m = parse(MINIMAL).unwrap();
        assert_eq!(m.id, "t1");
        assert_eq!(m.bg, None);
        assert!(m.quips.is_empty());
        assert_eq!(
            m.steps,
            vec![Step::Print {
                text: "hello".into(),
                style: Style::Normal,
                ms: 45
            }]
        );
    }

    #[test]
    fn builtins_is_pc95_only() {
        let ids: Vec<String> = builtins().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["pc95"]);
    }

    #[test]
    fn defaults_are_unpainted_with_pad_2_and_1() {
        let m = parse(MINIMAL).unwrap();
        assert!(!m.paint);
        assert_eq!(m.border, None);
        assert_eq!(m.pad_x, 2);
        assert_eq!(m.pad_y, 1);
        assert!(!m.uppercase);
    }

    #[test]
    fn flavoured_and_detect_width_parse_with_defaults() {
        let m = parse(MINIMAL).unwrap();
        assert!(!m.flavoured);
        assert_eq!(m.detect_width, None);
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nflavoured = true\ndetect_width = 27\n",
        );
        let m = parse(&src).unwrap();
        assert!(m.flavoured);
        assert_eq!(m.detect_width, Some(27));
    }

    #[test]
    fn rejects_a_detect_width_out_of_range() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\ndetect_width = 2\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn parses_paint_border_pad_and_uppercase() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nbg = \"#000000\"\npaint = true\nborder = \"#123456\"\npad_x = 3\npad_y = 4\nuppercase = true\n",
        );
        let m = parse(&src).unwrap();
        assert!(m.paint);
        assert_eq!(m.border.as_deref(), Some("#123456"));
        assert_eq!(m.pad_x, 3);
        assert_eq!(m.pad_y, 4);
        assert!(m.uppercase);
    }

    #[test]
    fn accepts_paint_without_bg() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\npaint = true\n",
        );
        let m = parse(&src).unwrap();
        assert!(m.paint);
        assert_eq!(m.bg, None);
    }

    #[test]
    fn rejects_border_without_paint() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nbg = \"#000000\"\nborder = \"#123456\"\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn rejects_border_without_bg() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\npaint = true\nborder = \"#123456\"\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn rejects_pad_x_out_of_range() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nbg = \"#000000\"\npaint = true\npad_x = 9\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn parses_logo_and_badge_with_defaults() {
        let m = parse(MINIMAL).unwrap();
        assert_eq!(m.logo, None);
        assert!(m.badge.is_empty());
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nlogo = \"unicorn\"\nbadge = [\"enchantment\", \"*STAR* ALLY\", \"GLITTER SAFE\"]\n",
        );
        let m = parse(&src).unwrap();
        assert_eq!(m.logo.as_deref(), Some("unicorn"));
        assert_eq!(m.badge, vec!["enchantment", "*STAR* ALLY", "GLITTER SAFE"]);
    }

    #[test]
    fn rejects_an_unknown_logo() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nlogo = \"dragon\"\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn rejects_more_than_four_badge_lines() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nbadge = [\"a\", \"b\", \"c\", \"d\", \"e\"]\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn rejects_a_badge_line_over_twenty_chars() {
        let src = MINIMAL.replace(
            "accent = \"#FFFF55\"\n",
            "accent = \"#FFFF55\"\nbadge = [\"this badge line is way too long\"]\n",
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn every_builtin_quip_fits_its_cols_against_fixture_facts() {
        let facts = crate::facts::Facts::fixture();
        for m in builtins() {
            for q in &m.quips {
                if let Some(rendered) = crate::template::render(q, &facts) {
                    assert!(
                        rendered.chars().count() <= m.cols as usize,
                        "{}: quip {:?} is {} chars, cols is {}",
                        m.id,
                        q,
                        rendered.chars().count(),
                        m.cols
                    );
                }
            }
        }
    }

    #[test]
    fn pc95_has_a_count_step() {
        let m = find("pc95", None).unwrap();
        assert!(m.steps.iter().any(
            |s| matches!(s, Step::Count { to, suffix, .. } if to == "{mem.kb}" && suffix == " OK")
        ));
    }

    #[test]
    fn parses_an_accent_detect_and_a_badge() {
        let src = format!(
            "{}\n[[step]]\ndetect = \"Detecting Horn \"\nresult = \"1 found\"\nstyle = \"accent\"\n",
            MINIMAL.replace(
                "[[step]]",
                "paint = true\nbg = \"#000000\"\nbadge = [\"enchantment\", \"*STAR* ALLY\"]\n[[step]]",
            )
        );
        let m = parse(&src).unwrap();
        assert_eq!(m.badge, vec!["enchantment", "*STAR* ALLY"]);
        assert!(m.steps.iter().any(|s| matches!(
            s,
            Step::Detect {
                style: Style::Accent,
                ..
            }
        )));
    }

    #[test]
    fn builtins_carry_a_quip_step() {
        for m in builtins() {
            assert!(m.steps.iter().any(|s| matches!(s, Step::Quip { .. })));
        }
    }

    #[test]
    fn an_unflavoured_machine_carries_its_own_quips() {
        let src = MINIMAL.replace(
            "[[step]]\nprint = \"hello\"\n",
            "quips = [\"first quip\", \"second quip\", \"third quip\"]\n[[step]]\nquip = true\n",
        );
        let m = parse(&src).unwrap();
        assert!(!m.flavoured);
        assert_eq!(m.quips[0], "first quip");
        assert!(m.steps.iter().any(|s| matches!(s, Step::Quip { .. })));
    }

    #[test]
    fn pc95_is_flavoured_with_no_logo_and_no_quips() {
        let m = find("pc95", None).unwrap();
        assert!(m.flavoured);
        assert_eq!(m.detect_width, Some(27));
        assert_eq!(m.logo, None);
        assert!(m.quips.is_empty());
    }

    #[test]
    fn rejects_a_quip_step_without_quips() {
        let src = format!("{MINIMAL}\n[[step]]\nquip = true\n");
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn a_flavoured_machine_may_have_a_quip_step_without_quips() {
        let src = format!(
            "{}\n[[step]]\nquip = true\n",
            MINIMAL.replace(
                "accent = \"#FFFF55\"\n",
                "accent = \"#FFFF55\"\nflavoured = true\n"
            )
        );
        assert!(parse(&src).is_ok());
    }

    #[test]
    fn rejects_a_step_with_two_kinds() {
        let src = format!("{MINIMAL}\n[[step]]\nprint = \"a\"\ndetect = \"b\"\nresult = \"c\"\n");
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn rejects_count_without_to_and_detect_without_result() {
        assert!(matches!(
            parse(&format!("{MINIMAL}\n[[step]]\ncount = \"{{n}}K\"\n")),
            Err(MachineError::Invalid(_))
        ));
        assert!(matches!(
            parse(&format!("{MINIMAL}\n[[step]]\ndetect = \"x\"\n")),
            Err(MachineError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_bad_colour_bad_id_and_bad_cols() {
        assert!(parse(&MINIMAL.replace("#AAAAAA", "grey")).is_err());
        assert!(parse(&MINIMAL.replace("\"t1\"", "\"Bad Id\"")).is_err());
        assert!(parse(&MINIMAL.replace("cols = 80", "cols = 5")).is_err());
    }

    #[test]
    fn malformed_toml_is_a_parse_error() {
        assert!(matches!(parse("id = "), Err(MachineError::Parse(_))));
    }

    #[test]
    fn user_file_overrides_builtin_and_bad_user_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pc95.toml"),
            MINIMAL.replace("\"t1\"", "\"pc95\""),
        )
        .unwrap();
        std::fs::write(dir.path().join("broken.toml"), "id = ").unwrap();
        let m = find("pc95", Some(dir.path())).unwrap();
        assert_eq!(m.name, "Test");
        let ids: Vec<String> = list(Some(dir.path())).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["pc95"]);
    }

    #[test]
    fn a_user_file_with_a_new_id_adds_to_the_roster() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("custom.toml"), MINIMAL).unwrap();
        let ids: Vec<String> = list(Some(dir.path())).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["pc95", "t1"]);
    }

    #[test]
    fn find_rejects_ids_that_are_not_plain_names() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.toml");
        std::fs::write(&outside, MINIMAL.replace("\"t1\"", "\"outside\"")).unwrap();
        let machines = dir.path().join("machines");
        std::fs::create_dir(&machines).unwrap();
        assert!(find("../outside", Some(&machines)).is_none());
        assert!(find("", Some(&machines)).is_none());
        assert!(find("PC95", None).is_none());
    }

    #[test]
    fn find_ignores_a_user_file_whose_id_does_not_match_its_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pc95.toml"), MINIMAL).unwrap();
        assert_eq!(
            find("pc95", Some(dir.path())).unwrap().name,
            "Mid-90s PC POST"
        );
    }

    #[test]
    fn the_findings_table_parses() {
        let src = format!(
            "{MINIMAL}\n[findings]\nboot_order = \"Boot device order: {{boot.devices}}\"\n"
        );
        let m = parse(&src).unwrap();
        assert_eq!(
            m.findings.get("boot_order").map(String::as_str),
            Some("Boot device order: {boot.devices}")
        );
    }

    #[test]
    fn a_findings_key_with_a_capital_letter_and_an_empty_value_are_both_rejected() {
        let bad_key = format!("{MINIMAL}\n[findings]\nBoot_Order = \"x\"\n");
        assert!(matches!(parse(&bad_key), Err(MachineError::Invalid(_))));
        let empty_value = format!("{MINIMAL}\n[findings]\nboot_order = \"\"\n");
        assert!(matches!(parse(&empty_value), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn findings_true_and_f1_true_parse_with_their_default_ms() {
        let src = format!("{MINIMAL}\n[[step]]\nfindings = true\n[[step]]\nf1 = true\n");
        let m = parse(&src).unwrap();
        assert!(m
            .steps
            .iter()
            .any(|s| matches!(s, Step::Findings { ms: 90, .. })));
        assert!(m.steps.iter().any(|s| matches!(s, Step::F1 { ms: 45, .. })));
    }

    #[test]
    fn findings_false_and_f1_false_are_rejected() {
        let findings_false = format!("{MINIMAL}\n[[step]]\nfindings = false\n");
        assert!(matches!(
            parse(&findings_false),
            Err(MachineError::Invalid(_))
        ));
        let f1_false = format!("{MINIMAL}\n[[step]]\nf1 = false\n");
        assert!(matches!(parse(&f1_false), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn a_step_with_both_quip_and_findings_is_rejected() {
        let src = format!(
            "{}\n[[step]]\nquip = true\nfindings = true\n",
            MINIMAL.replace(
                "accent = \"#FFFF55\"\n",
                "accent = \"#FFFF55\"\nflavoured = true\n"
            )
        );
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn pc95_carries_all_seven_finding_keys() {
        let m = find("pc95", None).unwrap();
        for key in [
            "boot_order",
            "boot_dirty",
            "irq_conflict",
            "virus_one",
            "virus_many",
            "f1",
            "f1_resume",
        ] {
            assert!(m.findings.contains_key(key), "missing {key}");
        }
    }

    // --- [[calendar]] -----------------------------------------------------------------------

    #[test]
    fn a_minimal_machine_has_no_calendar_rules_by_default() {
        let m = parse(MINIMAL).unwrap();
        assert!(m.calendar.is_empty());
    }

    #[test]
    fn parses_a_month_and_day_calendar_rule() {
        let src =
            format!("{MINIMAL}\n[[calendar]]\ntext = \"Happy new year.\"\nmonth = 1\nday = 1\n");
        let m = parse(&src).unwrap();
        assert_eq!(m.calendar.len(), 1);
        assert_eq!(m.calendar[0].text, "Happy new year.");
        assert_eq!(m.calendar[0].month, Some(1));
        assert_eq!(m.calendar[0].day, Some(1));
        assert_eq!(m.calendar[0].weekday, None);
        assert_eq!(m.calendar[0].day_of_year, None);
    }

    #[test]
    fn parses_a_weekday_and_day_calendar_rule() {
        let src = format!(
            "{MINIMAL}\n[[calendar]]\ntext = \"Spooky.\"\nweekday = \"friday\"\nday = 13\n"
        );
        let m = parse(&src).unwrap();
        assert_eq!(m.calendar[0].weekday.as_deref(), Some("friday"));
        assert_eq!(m.calendar[0].day, Some(13));
        assert_eq!(m.calendar[0].month, None);
        assert_eq!(m.calendar[0].day_of_year, None);
    }

    #[test]
    fn parses_a_day_of_year_calendar_rule() {
        let src = format!("{MINIMAL}\n[[calendar]]\ntext = \"Day 256.\"\nday_of_year = 256\n");
        let m = parse(&src).unwrap();
        assert_eq!(m.calendar[0].day_of_year, Some(256));
        assert_eq!(m.calendar[0].month, None);
        assert_eq!(m.calendar[0].day, None);
        assert_eq!(m.calendar[0].weekday, None);
    }

    #[test]
    fn rejects_a_calendar_rule_with_no_shape_or_more_than_one() {
        let none = format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\n");
        assert!(matches!(parse(&none), Err(MachineError::Invalid(_))));
        let two_shapes =
            format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nmonth = 1\nday = 1\nday_of_year = 5\n");
        assert!(matches!(parse(&two_shapes), Err(MachineError::Invalid(_))));
        let month_without_day = format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nmonth = 1\n");
        assert!(matches!(
            parse(&month_without_day),
            Err(MachineError::Invalid(_))
        ));
        let weekday_without_day =
            format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nweekday = \"friday\"\n");
        assert!(matches!(
            parse(&weekday_without_day),
            Err(MachineError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_calendar_rules_out_of_range_or_with_a_bad_weekday_or_empty_text() {
        let bad_month = format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nmonth = 13\nday = 1\n");
        assert!(matches!(parse(&bad_month), Err(MachineError::Invalid(_))));
        let bad_day = format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nmonth = 1\nday = 32\n");
        assert!(matches!(parse(&bad_day), Err(MachineError::Invalid(_))));
        let bad_weekday =
            format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nweekday = \"blursday\"\nday = 13\n");
        assert!(matches!(parse(&bad_weekday), Err(MachineError::Invalid(_))));
        let bad_day_of_year = format!("{MINIMAL}\n[[calendar]]\ntext = \"x\"\nday_of_year = 400\n");
        assert!(matches!(
            parse(&bad_day_of_year),
            Err(MachineError::Invalid(_))
        ));
        let empty_text = format!("{MINIMAL}\n[[calendar]]\ntext = \"\"\nmonth = 1\nday = 1\n");
        assert!(matches!(parse(&empty_text), Err(MachineError::Invalid(_))));
    }

    #[test]
    fn calendar_rule_matches_by_month_and_day_only_on_that_date() {
        let rule = CalendarRule {
            month: Some(1),
            day: Some(1),
            weekday: None,
            day_of_year: None,
            text: "x".into(),
        };
        assert!(rule.matches(2026, 1, 1));
        assert!(!rule.matches(2026, 1, 2));
        assert!(!rule.matches(2025, 2, 1));
    }

    #[test]
    fn calendar_rule_matches_friday_the_13th_only_when_the_13th_is_really_a_friday() {
        let rule = CalendarRule {
            month: None,
            day: Some(13),
            weekday: Some("friday".into()),
            day_of_year: None,
            text: "x".into(),
        };
        // 13 September 2024 is a real Friday the 13th.
        assert!(rule.matches(2024, 9, 13));
        // 13 September 2026 is a Sunday: the same day of the month, not the same day of the week.
        assert!(!rule.matches(2026, 9, 13));
        // 30 October 2026 is a Friday, but not the 13th.
        assert!(!rule.matches(2026, 10, 30));
    }

    #[test]
    fn calendar_rule_matches_day_of_year_256_landing_on_different_calendar_dates() {
        let rule = CalendarRule {
            month: None,
            day: None,
            weekday: None,
            day_of_year: Some(256),
            text: "x".into(),
        };
        assert!(rule.matches(2025, 9, 13)); // common year
        assert!(!rule.matches(2025, 9, 12));
        assert!(rule.matches(2024, 9, 12)); // leap year
        assert!(!rule.matches(2024, 9, 13));
    }

    #[test]
    fn pc95_has_all_twelve_calendar_rules() {
        let m = find("pc95", None).unwrap();
        assert_eq!(m.calendar.len(), 12);
    }

    #[test]
    fn each_pc95_calendar_rule_fires_on_its_own_date_and_no_other() {
        let m = find("pc95", None).unwrap();
        let cases: [(i32, u32, u32, &str); 12] = [
            (
                2026,
                1,
                1,
                "Year {date.year} rollover complete. Nothing caught fire. Again.",
            ),
            (
                2026,
                1,
                19,
                "Y2038 check: {y2038.days} days until 32-bit time runs out. Noted.",
            ),
            (
                2024,
                2,
                29,
                "Leap day detected. The calendar chip has been dreading this.",
            ),
            (
                2026,
                3,
                14,
                "Pi Day. Math coprocessor reports 3.14159 and refuses to stop.",
            ),
            (
                2026,
                3,
                31,
                "World Backup Day. The BIOS asks only that you think about it.",
            ),
            (
                2026,
                4,
                1,
                "No jokes today. The BIOS does not trust anyone.",
            ),
            (
                2026,
                5,
                4,
                "Force detected: not found. Proceeding on ordinary electricity.",
            ),
            (
                2025,
                9,
                13,
                "Day 256 of the year. The day counter did not overflow. This time.",
            ),
            (
                2024,
                9,
                13,
                "Friday the 13th. Booting anyway, against advice.",
            ),
            (
                2026,
                10,
                31,
                "Ghost processes are checked for every day. Today it just feels right.",
            ),
            (
                2026,
                12,
                25,
                "Holiday boot. Nobody is watching the build. Go outside.",
            ),
            (
                2026,
                12,
                31,
                "Last boot of {date.year}? Real-time clock standing by.",
            ),
        ];

        for (year, month, day, expected_text) in cases {
            assert_eq!(
                matching_calendar_text(&m, year, month, day),
                Some(expected_text),
                "expected {expected_text:?} to fire on {year:04}-{month:02}-{day:02}"
            );
        }

        // Ordinary days, days either side of a rule's own date, and the years a date-specific
        // rule does not apply in: none of these fire any rule.
        for (year, month, day) in [
            (2026, 1, 2),   // day after New Year
            (2025, 2, 28),  // no leap day in a common year
            (2026, 3, 12),  // day before Pi Day
            (2026, 3, 30),  // day before World Backup Day
            (2026, 4, 2),   // day after April Fools
            (2026, 5, 3),   // day before Star Wars Day
            (2026, 10, 30), // day before Halloween
            (2026, 12, 24), // day before Christmas
            (2026, 12, 30), // day before New Year's Eve
            (2026, 6, 15),  // an ordinary day
            (2026, 9, 12),  // day 255, one before day 256 in a common year
            (2026, 9, 14),  // day 257, one after day 256 in a common year
        ] {
            assert_eq!(
                matching_calendar_text(&m, year, month, day),
                None,
                "expected no calendar line on {year:04}-{month:02}-{day:02}"
            );
        }
    }
}
