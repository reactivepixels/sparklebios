//! Flavour TOML schema, validation, built-ins, lookup, and slot application.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::facts::Facts;
use crate::template;

/// A boot screen's personality: its own wording for every line the machine draws, plus the
/// facts each finding reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Flavour {
    /// The flavour's id, used in `--flavour` and `bios use`.
    pub id: String,
    /// The flavour's display name.
    pub name: String,
    /// Which sprite this flavour draws in its corner of the screen.
    pub sprite: String,
    /// The firmware line, in place of a real BIOS's version string.
    pub firmware: String,
    /// The vendor line, in place of a real BIOS's copyright string.
    pub vendor: String,
    /// The board name line.
    pub board: String,
    /// The joke that stands in for the CPU line.
    pub cpu_gag: String,
    /// What this flavour calls its signature peripheral.
    pub part: String,
    /// The detection result line for `part`.
    pub part_result: String,
    /// The boot streak line template.
    pub streak: String,
    /// The footer line at the very bottom of the screen.
    pub footer: String,
    /// The pool of one-line quips the screen picks from.
    pub quips: Vec<String>,
    /// This flavour's wording for each finding id a check can report.
    pub findings: BTreeMap<String, String>,
    /// What the flavour says away from the boot screen: the tab title, the line after a long
    /// command, the goodbye, and so on. A built-in has all eight; a user flavour may omit any,
    /// and that moment is then silent for it.
    pub presence: BTreeMap<String, String>,
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
    #[serde(default)]
    presence: BTreeMap<String, String>,
}

const UNICORN_TOML: &str = include_str!("../flavours/unicorn.toml");
const SUMO_TOML: &str = include_str!("../flavours/sumo.toml");
const NINJA_TOML: &str = include_str!("../flavours/ninja.toml");
const VIKING_TOML: &str = include_str!("../flavours/viking.toml");
const LUCHADOR_TOML: &str = include_str!("../flavours/luchador.toml");
const YETI_TOML: &str = include_str!("../flavours/yeti.toml");
const RACCOON_TOML: &str = include_str!("../flavours/raccoon.toml");
const WIZARD_TOML: &str = include_str!("../flavours/wizard.toml");

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
    // The presence keys follow the same shape rule as the findings ones: a lowercase name and a
    // value that says something.
    validate_findings(&raw.presence)?;
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
        presence: raw.presence,
    })
}

/// Built-in flavours, in roster order: unicorn, sumo, ninja, viking, luchador, yeti, raccoon,
/// wizard. A built-in that fails to parse is skipped.
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
    ]
    .into_iter()
    .filter_map(|src| parse(src).ok())
    .collect()
}

/// True when `id` names one of the built-in flavours.
pub fn is_builtin_id(id: &str) -> bool {
    builtins().iter().any(|f| f.id == id)
}

/// True for a built-in flavour's id, or for `"random"`: the one other id `bios flavour new` must
/// never let a user claim. `flavour = "random"` means something else entirely (see `resolve`), so
/// a real flavour written under that id would be unreachable at best and silently wrong at worst.
pub fn is_reserved_id(id: &str) -> bool {
    id == "random" || is_builtin_id(id)
}

/// `id`, resolved the way `flavour = "<id>"` in the config, or `--flavour <id>` on the command
/// line, is: today's random pick (see `pick_random`) when `id` is `"random"`, otherwise exactly
/// `find(id, user_dir)`. This is the one place `"random"` is recognised as meaning anything other
/// than a literal, missing flavour id, so every caller that resolves a configured or explicitly
/// requested flavour goes through this rather than `find` directly, and a tab's boot screen, its
/// shell hook, `bios fetch` and the rest can never disagree about what "random" means right now.
pub fn resolve(id: &str, user_dir: Option<&std::path::Path>, now: u64) -> Option<Flavour> {
    if id == "random" {
        let day = crate::clock::day_number(now as i64);
        return pick_random(day, user_dir);
    }
    find(id, user_dir)
}

/// Today's pick for `flavour = "random"`: walks a shuffled permutation of every flavour `list`
/// returns (built-ins plus user flavours), drawing a fresh permutation for each stretch of `n`
/// consecutive days, where `n` is the candidate count (a "round"). Within a round every flavour
/// appears exactly once, so there is no repeat and no uneven coverage inside a round; see
/// `pick_index` for how the one remaining seam, a round boundary, is closed. `day` is a
/// sequential day number in local time (see `clock::day_number`); nothing here is stored, so
/// every process that computes the same day number from the same moment computes the same pick.
/// `None` only when there is no flavour at all to choose from, which never happens for the
/// shipped built-in roster.
pub fn pick_random(day: i64, user_dir: Option<&std::path::Path>) -> Option<Flavour> {
    let candidates = list(user_dir);
    if candidates.is_empty() {
        return None;
    }
    let index = pick_index(day, candidates.len());
    candidates.into_iter().nth(index)
}

/// The index into a list of `n` candidates that `day` picks. `n` must be greater than 0.
///
/// `n == 1` always yields the only index, 0. `n == 2` alternates strictly: `day` modulo 2. For
/// `n >= 3`, `day` is split into a round (`day.div_euclid(n)`) and a position within that round
/// (`day.rem_euclid(n)`); each round shuffles `0..n` (see `shuffled`) and walks it position by
/// position, so every index in the round turns up exactly once.
///
/// That leaves exactly one seam: a round's last position meeting the next round's first. The
/// previous round's last pick is compared against this round's first, and if they match, this
/// round's first two positions are swapped, before either is read: `pick_index` is stateless and
/// recomputes `perm` from scratch on every call, so the fix has to be applied for every `pos` in
/// the round, not only when `pos == 0`, or a later call asking for `pos == 1` would see the
/// unswapped permutation and disagree with the earlier call that already returned the swapped
/// `perm[0]`. That comparison uses `shuffled(round - 1, n)`'s own last element directly rather
/// than recursing into this function for `round - 1`, and that is safe precisely because the
/// swap above only ever touches positions 0 and 1: for `n >= 3` that never includes position
/// `n - 1`, so a round's last element is always the same whether or not that round's own
/// boundary fix ran. Breaking that invariant (for example widening the swap, or shrinking `n`
/// below 3 while keeping it) would make the previous round's last element depend on this same
/// fix, and the lookup below would need to recurse to stay correct.
fn pick_index(day: i64, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    if n == 2 {
        return day.rem_euclid(2) as usize;
    }
    let n64 = n as i64;
    let round = day.div_euclid(n64);
    let pos = day.rem_euclid(n64) as usize;
    let mut perm = shuffled(round, n);
    let previous_last = shuffled(round - 1, n)[n - 1];
    if perm[0] == previous_last {
        perm.swap(0, 1);
    }
    perm[pos]
}

/// A deterministic Fisher-Yates shuffle of `0..n`, seeded from `round` alone, so the same round
/// gives the same permutation on every platform and every run forever: no `std::collections`
/// hasher and no real randomness, just `fnv1a32` over `round`'s bytes feeding a small xorshift
/// generator.
fn shuffled(round: i64, n: usize) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..n).collect();
    // xorshift32 has one bad state, all zero bits, where it stalls forever; `| 1` keeps the
    // seed away from it without otherwise touching the hash.
    let mut state = fnv1a32(&round.to_le_bytes()) | 1;
    for i in (1..n).rev() {
        state = next_u32(state);
        let j = (state as usize) % (i + 1);
        perm.swap(i, j);
    }
    perm
}

/// One step of xorshift32. Never fed a zero state (see `shuffled`), so it never gets stuck.
fn next_u32(state: u32) -> u32 {
    let mut x = state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

/// FNV-1a, 32 bit: hand rolled rather than pulling in a crate for it (see the "no new
/// dependencies" rule), the same algorithm `shell::hook_hash` uses for its own fingerprint,
/// reimplemented here so this module does not have to reach into `shell` for ten lines of code.
fn fnv1a32(bytes: &[u8]) -> u32 {
    const OFFSET: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The id rule for `bios flavour new`: lowercase letters, digits and hyphens, starting with a
/// letter. Stricter than `valid_id` (which also allows an underscore or a leading digit), so that
/// nothing that looks like a path, a hidden file or `..` can ever reach a join with the flavours
/// directory.
pub fn well_formed_new_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `id`, read as a human readable label: each hyphen separated part capitalised and joined with a
/// space, for example `my-flavour` becomes `My Flavour`.
pub fn display_name(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The starter file `bios flavour new <id>` writes: a complete, valid flavour with every key
/// filled in and a comment above each one, built from the real schema (this struct, plus every
/// event in `presence::EVENTS`), so `bios boot --flavour <id>` works before a single character is
/// changed. `id` must already satisfy `well_formed_new_id`; the caller is responsible for that.
pub fn starter_template(id: &str, name: &str) -> String {
    let upper = id.to_uppercase();
    format!(
        "\
# {name}, a SparkleBIOS flavour.
#
# A flavour is one personality: the mascot, the firmware and vendor wording, the
# boot streak line, the footer code and the quips. Every key below is required
# and already works; change the wording and keep the shape. See
# docs/flavours.md for the full schema and docs/voice.md before you write a
# joke of your own.

# Lowercase letters, digits and hyphens, starting with a letter. The file name must match.
id = \"{id}\"

# A human readable label, shown by `bios flavours`.
name = \"{name}\"

# A built-in sprite: unicorn, sumo, ninja, viking, luchador, yeti, raccoon or wizard.
# Shown as a real image where the terminal speaks the Kitty graphics protocol.
sprite = \"unicorn\"

# The BIOS/firmware banner line, the first thing the boot screen prints.
firmware = \"{name} Modular BIOS v1.0, Homebrew Edition\"

# The copyright line. {{date.year}} is filled in with the current year.
vendor = \"Copyright (C) {{date.year}}, {name} Systems.\"

# The board or chassis revision line.
board = \"{upper}-1 Board Revision A\"

# The trailing joke on the processor line.
cpu_gag = \"0 problems detected\"

# The name of the one part this flavour detects, for example the unicorn flavour's Horn.
part = \"Mascot\"

# The result shown for that part's detect line.
part_result = \"1 found (unverified)\"

# The boot streak line. {{streak.label}} is filled in once there is a streak to report,
# and the line is left out entirely until then, same as a preview never showing it.
streak = \"Boot streak: {{streak.label}}.\"

# The trailing segment of the footer serial.
footer = \"{upper}-1-HOME-0001BREW-00\"

# At least 3 one line jokes, deadpan, in the voice docs/voice.md sets out.
quips = [
  \"{name} detected. Diagnostics inconclusive.\",
  \"Self test passed. Nobody checked the results.\",
  \"Floppy drive A: not found. Nobody is surprised.\",
]

# What this flavour says away from the boot screen: the tab title, the line after a
# long command, the goodbye, and so on. A built-in may leave one out and stay silent
# for that moment; a flavour you write yourself should keep all eight.
[presence]
title = \"{upper}-1\"
done = \"{name}: done in {{took}}.\"
failed = \"{name}: failed after {{took}}.\"
goodbye = \"{name}: goodbye.\"
not_found = \"Bad command. {name} looked everywhere.\"
refresh = \"{name}: checks refreshed.\"
resume = \"{name}: booting {{device}}.\"
switched = \"{name} is loaded.\"
"
    )
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
            vec!["unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon", "wizard",]
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

    /// The CI job greps the flavour files on disk for these; this pins the same rule on the
    /// parsed `Flavour` data instead, so it also covers a flavour loaded from a user's own
    /// flavours directory, not only the ones built into the binary.
    #[test]
    fn no_builtin_flavours_presence_or_findings_line_has_an_em_or_en_dash() {
        for f in builtins() {
            for (event, line) in &f.presence {
                assert!(
                    !line.contains('\u{2014}') && !line.contains('\u{2013}'),
                    "{}: presence.{event} {line:?} has an em or en dash",
                    f.id
                );
            }
            for (id, line) in &f.findings {
                assert!(
                    !line.contains('\u{2014}') && !line.contains('\u{2013}'),
                    "{}: findings.{id} {line:?} has an em or en dash",
                    f.id
                );
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
            vec!["unicorn", "sumo", "ninja", "viking", "luchador", "yeti", "raccoon", "wizard",]
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

    // --- random -----------------------------------------------------------------------------

    #[test]
    fn random_is_a_reserved_id_alongside_every_builtin() {
        assert!(is_reserved_id("random"));
        for f in builtins() {
            assert!(is_reserved_id(&f.id));
        }
        assert!(!is_reserved_id("mine"));
    }

    #[test]
    fn resolve_of_a_literal_id_is_exactly_find() {
        assert_eq!(
            resolve("sumo", None, 0).map(|f| f.id),
            find("sumo", None).map(|f| f.id)
        );
        assert_eq!(resolve("nope", None, 0), None);
    }

    #[test]
    fn resolve_random_is_stable_for_the_same_moment() {
        let now = 1_700_000_000;
        assert_eq!(resolve("random", None, now), resolve("random", None, now));
        assert!(resolve("random", None, now).is_some());
    }

    #[test]
    fn pick_random_is_stable_for_the_same_day() {
        let a = pick_random(19_000, None);
        let b = pick_random(19_000, None);
        assert_eq!(a, b);
        assert!(a.is_some());
    }

    /// How many consecutive day numbers the property tests below walk. Comfortably more than the
    /// 4380 days (12 years) the shipped algorithm was simulated over to find the old bug.
    const PROPERTY_DAYS: i64 = 20_000;

    #[test]
    fn pick_index_never_repeats_between_consecutive_days_for_every_roster_size() {
        for n in 2..=12usize {
            let mut previous = pick_index(0, n);
            for day in 1..PROPERTY_DAYS {
                let current = pick_index(day, n);
                assert_ne!(
                    previous,
                    current,
                    "n={n}: day {} repeated day {}'s pick",
                    day,
                    day - 1
                );
                previous = current;
            }
        }
    }

    #[test]
    fn pick_index_covers_every_aligned_window_exactly_once_for_every_roster_size() {
        for n in 2..=12usize {
            let windows = (PROPERTY_DAYS / n as i64) as usize;
            for w in 0..windows {
                let start = w as i64 * n as i64;
                let mut seen: Vec<bool> = vec![false; n];
                for day in start..start + n as i64 {
                    let index = pick_index(day, n);
                    assert!(
                        !seen[index],
                        "n={n}: window starting at {start} picked index {index} twice"
                    );
                    seen[index] = true;
                }
                assert!(
                    seen.iter().all(|&s| s),
                    "n={n}: window starting at {start} missed an index"
                );
            }
        }
    }

    /// The pair the shipped, now replaced, algorithm got wrong: both landed on `sumo`. Noon UTC
    /// on each date, so the local calendar day matches in every timezone this runs in.
    #[test]
    fn regression_2026_02_01_and_2026_02_02_no_longer_repeat() {
        let day_one = crate::clock::day_number(1_769_947_200);
        let day_two = crate::clock::day_number(1_770_033_600);
        assert_eq!(day_two, day_one + 1);
        let pick_one = pick_random(day_one, None).unwrap();
        let pick_two = pick_random(day_two, None).unwrap();
        assert_ne!(
            pick_one.id, pick_two.id,
            "2026-02-01 and 2026-02-02 picked the same flavour again"
        );
    }

    #[test]
    fn pick_random_includes_a_user_flavour_in_its_candidates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mine.toml"),
            MINIMAL.replace("\"t1\"", "\"mine\""),
        )
        .unwrap();
        let ever_mine =
            (0..PROPERTY_DAYS).any(|day| pick_random(day, Some(dir.path())).unwrap().id == "mine");
        assert!(ever_mine, "the user flavour never came up over 20000 days");
    }

    #[test]
    fn pick_index_of_a_single_candidate_is_always_zero() {
        for day in [-100, -1, 0, 1, 100, 20_000] {
            assert_eq!(pick_index(day, 1), 0);
        }
    }

    #[test]
    fn pick_index_of_two_candidates_alternates_strictly() {
        for day in 0..PROPERTY_DAYS {
            assert_eq!(pick_index(day, 2), (day.rem_euclid(2)) as usize);
        }
    }

    #[test]
    fn shuffled_is_a_valid_permutation_across_roster_sizes_and_rounds() {
        for n in 1..=12usize {
            for round in -5..5 {
                let mut perm = shuffled(round, n);
                perm.sort_unstable();
                assert_eq!(perm, (0..n).collect::<Vec<_>>(), "n={n} round={round}");
            }
        }
    }
}
