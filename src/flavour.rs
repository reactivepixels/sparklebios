//! Flavour TOML schema, validation, built-ins, lookup, and slot application.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::facts::Facts;
use crate::template;

#[derive(Debug, Clone, PartialEq)]
pub struct Flavour {
    pub id: String,
    pub name: String,
    pub sprite: String,
    pub firmware: String,
    pub vendor: String,
    pub board: String,
    pub cpu_gag: String,
    pub part: String,
    pub part_result: String,
    pub streak: String,
    pub footer: String,
    pub quips: Vec<String>,
    pub findings: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RawFlavour {
    id: String,
    name: String,
    sprite: String,
    firmware: String,
    vendor: String,
    board: String,
    cpu_gag: String,
    part: String,
    part_result: String,
    streak: String,
    footer: String,
    #[serde(default)]
    quips: Vec<String>,
    #[serde(default)]
    findings: BTreeMap<String, String>,
}

const UNICORN_TOML: &str = include_str!("../flavours/unicorn.toml");
const SUMO_TOML: &str = include_str!("../flavours/sumo.toml");
const NINJA_TOML: &str = include_str!("../flavours/ninja.toml");
const VIKING_TOML: &str = include_str!("../flavours/viking.toml");
const LUCHADOR_TOML: &str = include_str!("../flavours/luchador.toml");
const YETI_TOML: &str = include_str!("../flavours/yeti.toml");
const RACCOON_TOML: &str = include_str!("../flavours/raccoon.toml");
const WIZARD_TOML: &str = include_str!("../flavours/wizard.toml");
const CAVEMAN_TOML: &str = include_str!("../flavours/caveman.toml");

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// A findings key: `[a-z0-9_]+`. An unknown id is still valid, since a flavour may carry
/// phrasing for a check that does not exist yet.
fn valid_finding_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn validate_findings(findings: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, value) in findings {
        if !valid_finding_key(key) {
            return Err(format!("findings key {key:?} does not match [a-z0-9_]+"));
        }
        if value.is_empty() {
            return Err(format!("findings value for {key:?} is empty"));
        }
    }
    Ok(())
}

/// All string keys are required and non-empty; `id` matches `[a-z0-9_-]+`; `sprite` must name a
/// built-in sprite; at least 3 quips.
pub fn parse(src: &str) -> Result<Flavour, String> {
    let raw: RawFlavour = toml::from_str(src).map_err(|e| e.to_string())?;
    validate(raw)
}

fn validate(raw: RawFlavour) -> Result<Flavour, String> {
    if !valid_id(&raw.id) {
        return Err(format!("id {:?} does not match [a-z0-9_-]+", raw.id));
    }
    let strings = [
        ("name", &raw.name),
        ("sprite", &raw.sprite),
        ("firmware", &raw.firmware),
        ("vendor", &raw.vendor),
        ("board", &raw.board),
        ("cpu_gag", &raw.cpu_gag),
        ("part", &raw.part),
        ("part_result", &raw.part_result),
        ("streak", &raw.streak),
        ("footer", &raw.footer),
    ];
    for (key, value) in strings {
        if value.is_empty() {
            return Err(format!("{key} is empty"));
        }
    }
    if crate::sprite::builtin(&raw.sprite).is_none() {
        return Err(format!("sprite {:?} is not a known sprite", raw.sprite));
    }
    if raw.quips.len() < 3 {
        return Err(format!(
            "flavour has {} quips, fewer than 3",
            raw.quips.len()
        ));
    }
    validate_findings(&raw.findings)?;
    Ok(Flavour {
        id: raw.id,
        name: raw.name,
        sprite: raw.sprite,
        firmware: raw.firmware,
        vendor: raw.vendor,
        board: raw.board,
        cpu_gag: raw.cpu_gag,
        part: raw.part,
        part_result: raw.part_result,
        streak: raw.streak,
        footer: raw.footer,
        quips: raw.quips,
        findings: raw.findings,
    })
}

/// Built-in flavours, in roster order: unicorn, sumo, ninja, viking, luchador, yeti, raccoon,
/// wizard, caveman. A built-in that fails to parse is skipped.
pub fn builtins() -> Vec<Flavour> {
    [
        UNICORN_TOML,
        SUMO_TOML,
        NINJA_TOML,
        VIKING_TOML,
        LUCHADOR_TOML,
        YETI_TOML,
        RACCOON_TOML,
        WIZARD_TOML,
        CAVEMAN_TOML,
    ]
    .into_iter()
    .filter_map(|src| parse(src).ok())
    .collect()
}

/// `<user_dir>/<id>.toml` wins over a built-in of the same id. Unreadable or invalid user files
/// are ignored, and only a plain id (no path separators) is looked up.
pub fn find(id: &str, user_dir: Option<&std::path::Path>) -> Option<Flavour> {
    if !valid_id(id) {
        return None;
    }
    if let Some(dir) = user_dir {
        let path = dir.join(format!("{id}.toml"));
        if let Ok(src) = std::fs::read_to_string(&path) {
            if let Ok(f) = parse(&src) {
                if f.id == id {
                    return Some(f);
                }
            }
        }
    }
    builtins().into_iter().find(|f| f.id == id)
}

/// Built-ins plus valid user flavours, user files replacing built-ins with the same id.
pub fn list(user_dir: Option<&std::path::Path>) -> Vec<Flavour> {
    let mut flavours = builtins();
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
                let Ok(f) = parse(&src) else {
                    continue;
                };
                if let Some(existing) = flavours.iter_mut().find(|b| b.id == f.id) {
                    *existing = f;
                } else {
                    flavours.push(f);
                }
            }
        }
    }
    flavours
}

/// Inserts `flavour.firmware`, `flavour.vendor`, `flavour.board`, `flavour.cpu_gag`,
/// `flavour.part`, `flavour.part_result`, `flavour.streak` and `flavour.footer` into `facts`.
/// Each value is first rendered with `template::render` against the facts already present; a
/// value that cannot be resolved is not inserted, so the machine line that uses it is omitted.
pub fn apply(flavour: &Flavour, facts: &mut Facts) {
    let fields: [(&str, &str); 8] = [
        ("flavour.firmware", &flavour.firmware),
        ("flavour.vendor", &flavour.vendor),
        ("flavour.board", &flavour.board),
        ("flavour.cpu_gag", &flavour.cpu_gag),
        ("flavour.part", &flavour.part),
        ("flavour.part_result", &flavour.part_result),
        ("flavour.streak", &flavour.streak),
        ("flavour.footer", &flavour.footer),
    ];
    for (key, value) in fields {
        if let Some(rendered) = template::render(value, facts) {
            facts.insert(key, rendered);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r##"
id = "t1"
name = "Test"
sprite = "unicorn"
firmware = "f"
vendor = "v"
board = "b"
cpu_gag = "c"
part = "p"
part_result = "pr"
streak = "s"
footer = "ft"
quips = ["a", "b", "c"]
"##;

    #[test]
    fn parses_a_minimal_flavour() {
        let f = parse(MINIMAL).unwrap();
        assert_eq!(f.id, "t1");
        assert_eq!(f.sprite, "unicorn");
        assert_eq!(f.quips.len(), 3);
    }

    #[test]
    fn builtins_are_the_full_roster_in_order() {
        let ids: Vec<String> = builtins().into_iter().map(|f| f.id).collect();
        assert_eq!(
            ids,
            vec![
                "unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon", "wizard",
                "caveman"
            ]
        );
    }

    #[test]
    fn rejects_a_missing_key() {
        let src = MINIMAL.replace("footer = \"ft\"\n", "");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn rejects_an_empty_string() {
        let src = MINIMAL.replace("footer = \"ft\"", "footer = \"\"");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn rejects_a_bad_id() {
        let src = MINIMAL.replace("\"t1\"", "\"Bad Id\"");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn rejects_an_unknown_sprite() {
        let src = MINIMAL.replace("sprite = \"unicorn\"", "sprite = \"dragon\"");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn rejects_fewer_than_three_quips() {
        let src = MINIMAL.replace("quips = [\"a\", \"b\", \"c\"]", "quips = [\"a\", \"b\"]");
        assert!(parse(&src).is_err());
    }

    #[test]
    fn apply_inserts_all_eight_slots_for_fixture_facts() {
        let flavour = find("unicorn", None).unwrap();
        let mut facts = Facts::fixture();
        apply(&flavour, &mut facts);
        for key in [
            "flavour.firmware",
            "flavour.vendor",
            "flavour.board",
            "flavour.cpu_gag",
            "flavour.part",
            "flavour.part_result",
            "flavour.streak",
            "flavour.footer",
        ] {
            assert!(facts.get(key).is_some(), "{key} was not inserted");
        }
        assert_eq!(
            facts.get("flavour.vendor"),
            Some("Copyright (C) 1985-2026, Rainbows & Unicorns, Inc.")
        );
    }

    #[test]
    fn apply_skips_streak_when_streak_label_is_absent() {
        let flavour = find("unicorn", None).unwrap();
        let mut facts = Facts::new();
        facts.insert("date.year", "2026");
        apply(&flavour, &mut facts);
        assert_eq!(facts.get("flavour.streak"), None);
        assert!(facts.get("flavour.vendor").is_some());
    }

    #[test]
    fn every_builtin_flavour_quip_fits_80_columns_against_fixture_facts() {
        let facts = Facts::fixture();
        for f in builtins() {
            for q in &f.quips {
                if let Some(rendered) = template::render(q, &facts) {
                    assert!(
                        rendered.chars().count() <= 80,
                        "{}: quip {:?} is {} chars",
                        f.id,
                        q,
                        rendered.chars().count()
                    );
                }
            }
        }
    }

    /// `boot.*`, `irq.*` and `virus.*`, the facts a finding's phrasing renders against, on top of
    /// `Facts::fixture()`.
    fn finding_facts() -> Facts {
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
        facts.insert("virus.count", "3");
        facts.insert("stash.repo", "eko-pro");
        facts.insert("stash.count", "3 stashes");
        facts.insert("stash.age", "4 months");
        facts.insert("dotfiles.repo", ".dotfiles");
        facts.insert("dotfiles.changes", "3 uncommitted changes");
        facts.insert("runtime.repo", "eko-pro");
        facts.insert("runtime.name", "node");
        facts.insert("runtime.wanted", "20.11.0");
        facts.insert("runtime.found", "22.3.0");
        facts.insert("disk.days_left", "18 days");
        facts.insert("disk.rate", "2.1GB a day");
        facts.insert("battery.health", "79%");
        facts.insert("battery.cycles", "412");
        facts
    }

    /// `boot.*`, `irq.*` and `virus.*` with deliberately long values, on top of `Facts::fixture()`,
    /// so a finding whose slots resolve to something short today cannot hide a shortening bug.
    fn finding_facts_with_long_values() -> Facts {
        let mut facts = Facts::fixture();
        facts.insert(
            "boot.devices",
            "side-project, another-long-repo, third-repo-here",
        );
        facts.insert("boot.device", "side-project");
        facts.insert("boot.changes", "3 uncommitted changes");
        facts.insert("irq.port", "3000");
        facts.insert("irq.name", "node");
        facts.insert("irq.pid", "4821");
        facts.insert("irq.age", "3 days");
        facts.insert("virus.repo", "side-project");
        facts.insert("virus.file", ".env.production.local");
        facts.insert("virus.count", "3");
        facts.insert("stash.repo", "side-project");
        facts.insert("stash.count", "13 stashes");
        facts.insert("stash.age", "14 months");
        facts.insert("dotfiles.repo", ".dotfiles-personal");
        facts.insert("dotfiles.changes", "17 uncommitted changes");
        facts.insert("runtime.repo", "side-project");
        facts.insert("runtime.name", "python");
        facts.insert("runtime.wanted", "3.12.4");
        facts.insert("runtime.found", "3.13.0");
        facts.insert("disk.days_left", "18 days");
        facts.insert("disk.rate", "12.5GB a day");
        facts.insert("battery.health", "68%");
        facts.insert("battery.cycles", "1284");
        facts
    }

    #[test]
    fn every_builtin_finding_phrasing_fits_80_columns_against_fixture_facts() {
        for facts in [finding_facts(), finding_facts_with_long_values()] {
            for f in builtins() {
                for (key, value) in &f.findings {
                    let Some(rendered) = crate::render::shorten_finding_line(value, &facts, 80)
                    else {
                        continue;
                    };
                    assert!(
                        rendered.chars().count() <= 80,
                        "{}: finding {key} {:?} rendered to {} chars: {rendered:?}",
                        f.id,
                        value,
                        rendered.chars().count()
                    );
                    let tail = value.rsplit_once('}').map_or(value.as_str(), |(_, t)| t);
                    assert!(
                        rendered.ends_with(tail),
                        "{}: finding {key} {:?} lost its final sentence, rendered {rendered:?}",
                        f.id,
                        value
                    );
                }
            }
        }
    }

    #[test]
    fn every_builtin_finding_phrasing_resolves_against_fixture_facts() {
        let facts = finding_facts();
        for f in builtins() {
            for (key, value) in &f.findings {
                assert!(
                    template::render(value, &facts).is_some(),
                    "{}: finding {key} {:?} did not resolve",
                    f.id,
                    value
                );
            }
        }
    }

    #[test]
    fn every_builtin_flavours_sprite_resolves() {
        for f in builtins() {
            assert!(
                crate::sprite::builtin(&f.sprite).is_some(),
                "{}: sprite {:?} did not resolve",
                f.id,
                f.sprite
            );
        }
    }

    #[test]
    fn user_file_overrides_builtin_and_bad_user_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("sumo.toml"),
            MINIMAL.replace("\"t1\"", "\"sumo\""),
        )
        .unwrap();
        std::fs::write(dir.path().join("broken.toml"), "id = ").unwrap();
        let f = find("sumo", Some(dir.path())).unwrap();
        assert_eq!(f.name, "Test");
        let ids: Vec<String> = list(Some(dir.path())).into_iter().map(|f| f.id).collect();
        assert_eq!(
            ids,
            vec![
                "unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon", "wizard",
                "caveman"
            ]
        );
    }

    #[test]
    fn find_rejects_ids_that_are_not_plain_names() {
        assert!(find("../outside", None).is_none());
        assert!(find("", None).is_none());
        assert!(find("UNICORN", None).is_none());
    }

    #[test]
    fn find_ignores_a_user_file_whose_id_does_not_match_its_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sumo.toml"), MINIMAL).unwrap();
        assert_eq!(find("sumo", Some(dir.path())).unwrap().name, "Sumo");
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(parse("id = ").is_err());
    }

    #[test]
    fn sumos_findings_table_parses_and_has_all_seven_keys() {
        let sumo = find("sumo", None).unwrap();
        for key in [
            "boot_order",
            "boot_dirty",
            "irq_conflict",
            "virus_one",
            "virus_many",
            "f1",
            "f1_resume",
        ] {
            assert!(sumo.findings.contains_key(key), "sumo missing {key}");
        }
    }

    #[test]
    fn unicorns_findings_table_is_empty() {
        let unicorn = find("unicorn", None).unwrap();
        assert!(unicorn.findings.is_empty());
    }

    #[test]
    fn rejects_a_bad_findings_key_and_an_empty_value() {
        let bad_key = format!("{MINIMAL}\n[findings]\nBoot_Order = \"x\"\n");
        assert!(parse(&bad_key).is_err());
        let empty_value = format!("{MINIMAL}\n[findings]\nboot_order = \"\"\n");
        assert!(parse(&empty_value).is_err());
    }
}
