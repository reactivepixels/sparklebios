//! Machine TOML schema, validation, built-ins, lookup.

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
    pub quips: Vec<String>,
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
    quips: Vec<String>,
    #[serde(rename = "step", default)]
    steps: Vec<RawStep>,
}

#[derive(Debug, Deserialize)]
struct RawStep {
    print: Option<String>,
    count: Option<String>,
    detect: Option<String>,
    quip: Option<bool>,
    to: Option<String>,
    suffix: Option<String>,
    result: Option<String>,
    style: Option<String>,
    ms: Option<u64>,
}

const PC95_TOML: &str = include_str!("../machines/pc95.toml");
const PC85_TOML: &str = include_str!("../machines/pc85.toml");

pub fn parse(src: &str) -> Result<Machine, MachineError> {
    let raw: RawMachine = toml::from_str(src).map_err(|e| MachineError::Parse(e.to_string()))?;
    validate(raw)
}

/// Built-in machines, in roster order: pc95, pc85. A built-in that fails to parse is skipped.
pub fn builtins() -> Vec<Machine> {
    [PC95_TOML, PC85_TOML]
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
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if kinds != 1 {
            return Err(MachineError::Invalid(format!(
                "step {idx}: must have exactly one of print, count, detect, quip"
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
        } else {
            match s.quip {
                Some(true) => {
                    if raw.quips.is_empty() {
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
        quips: raw.quips,
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
    fn builtins_are_pc95_then_pc85() {
        let ids: Vec<String> = builtins().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["pc95", "pc85"]);
    }

    #[test]
    fn pc95_has_a_count_and_an_accent_detect() {
        let m = find("pc95", None).unwrap();
        assert!(m.steps.iter().any(
            |s| matches!(s, Step::Count { to, suffix, .. } if to == "{mem.kb}" && suffix == " OK")
        ));
        assert!(m.steps.iter().any(|s| matches!(
            s,
            Step::Detect {
                style: Style::Accent,
                ..
            }
        )));
    }

    #[test]
    fn builtins_carry_quips_and_a_quip_step() {
        for m in builtins() {
            assert!(m.quips.len() >= 5, "{} needs quips", m.id);
            assert!(m.steps.iter().any(|s| matches!(s, Step::Quip { .. })));
        }
        assert_eq!(
            find("pc95", None).unwrap().quips[0],
            "Turbo button engaged. It does nothing. It never did."
        );
    }

    #[test]
    fn rejects_a_quip_step_without_quips() {
        let src = format!("{MINIMAL}\n[[step]]\nquip = true\n");
        assert!(matches!(parse(&src), Err(MachineError::Invalid(_))));
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
            dir.path().join("pc85.toml"),
            MINIMAL.replace("\"t1\"", "\"pc85\""),
        )
        .unwrap();
        std::fs::write(dir.path().join("broken.toml"), "id = ").unwrap();
        let m = find("pc85", Some(dir.path())).unwrap();
        assert_eq!(m.name, "Test");
        let ids: Vec<String> = list(Some(dir.path())).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["pc95", "pc85"]);
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
        std::fs::write(dir.path().join("pc85.toml"), MINIMAL).unwrap();
        assert_eq!(
            find("pc85", Some(dir.path())).unwrap().name,
            "1985 PC/AT style"
        );
    }
}
