//! Facts type, fixture, gather().

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod other;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts(std::collections::BTreeMap<String, String>);

impl Facts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: &str, value: impl Into<String>) {
        self.0.insert(key.to_string(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }

    /// Deterministic facts for tests and golden files.
    pub fn fixture() -> Self {
        let mut facts = Self::new();
        facts.insert("cpu.name", "Apple M4 Pro");
        facts.insert("cpu.cores", "14");
        facts.insert("mem.kb", "37748736");
        facts.insert("disk.size_gb", "994");
        facts.insert("disk.free_gb", "212");
        facts.insert("disk.used_pct", "78");
        facts.insert("os.name", "macOS");
        facts.insert("os.version", "26.5");
        facts.insert("host.name", "unicorn");
        facts.insert("shell.name", "zsh");
        facts.insert("shell.boot_ms", "412");
        facts.insert("date.bios", "09/19/2026");
        facts.insert("date.year", "2026");
        facts.insert("date.today", "2026-09-19");
        facts.insert("streak.days", "12");
        facts.insert("streak.label", "12 days");
        facts
    }
}

/// All fast facts. Each probe fails silently by leaving its keys absent.
pub fn gather() -> Facts {
    let mut facts = Facts::new();

    let now = crate::clock::now_unix();
    let (year, month, day) = crate::clock::local_ymd(now as i64);
    facts.insert("date.today", format!("{year:04}-{month:02}-{day:02}"));
    facts.insert("date.bios", format!("{month:02}/{day:02}/{year:04}"));
    facts.insert("date.year", format!("{year:04}"));

    #[cfg(target_os = "macos")]
    macos::probe(&mut facts);
    #[cfg(not(target_os = "macos"))]
    other::probe(&mut facts);

    facts
}

/// The terminal name for `bios fetch`: `TERM_PROGRAM` when it is set and non-empty, falling back
/// to `TERM`. Reported exactly as the environment gives it, never prettified: a value neither
/// probe is confident about is worse than the raw one.
pub fn terminal_name(term_program: Option<&str>, term: Option<&str>) -> Option<String> {
    term_program
        .filter(|s| !s.is_empty())
        .or_else(|| term.filter(|s| !s.is_empty()))
        .map(String::from)
}

/// `kb` kilobytes, rounded to the nearest whole gigabyte, for `bios fetch`'s `Memory` line.
pub fn kb_to_whole_gb(kb: u64) -> u64 {
    (kb as f64 / 1024.0 / 1024.0).round() as u64
}

/// The theme a Ghostty config names, for `bios fetch`'s `Theme` line: the value of the first top
/// level `theme =` line (the same syntax `theme::set_ghostty_theme` writes and
/// `theme::pick_config_path` looks for) in whichever of `candidates` `theme::pick_config_path`
/// picks. `None` when no candidate exists, the winning file has no such line, or the line names
/// nothing.
pub fn ghostty_theme(candidates: &[std::path::PathBuf]) -> Option<String> {
    let path = crate::theme::pick_config_path(candidates)?;
    let contents = std::fs::read_to_string(path).ok()?;
    let line = contents.lines().find(|line| {
        line.strip_prefix("theme")
            .is_some_and(|rest| rest.trim_start_matches(' ').starts_with('='))
    })?;
    let value = line.split_once('=')?.1.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_known_and_missing_facts() {
        let f = Facts::fixture();
        assert_eq!(f.get("mem.kb"), Some("37748736"));
        assert_eq!(f.get("nope"), None);
    }

    #[test]
    fn remove_deletes_a_key() {
        let mut f = Facts::fixture();
        assert!(f.get("shell.boot_ms").is_some());
        f.remove("shell.boot_ms");
        assert_eq!(f.get("shell.boot_ms"), None);
    }

    #[test]
    fn gather_always_sets_the_date_facts() {
        let f = gather();
        assert_eq!(f.get("date.today").unwrap().len(), 10);
        assert_eq!(f.get("date.bios").unwrap().len(), 10);
        assert_eq!(f.get("date.year").unwrap().len(), 4);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gather_reads_real_hardware_on_macos() {
        let f = gather();
        assert!(f.get("mem.kb").unwrap().parse::<u64>().unwrap() > 1_000_000);
        assert!(f.get("cpu.cores").unwrap().parse::<u32>().unwrap() >= 1);
        assert!(!f.get("cpu.name").unwrap().is_empty());
        assert!(f.get("disk.used_pct").unwrap().parse::<u32>().unwrap() <= 100);
        assert!(f.get("disk.size_gb").unwrap().parse::<u64>().unwrap() > 0);
        assert_eq!(f.get("os.name"), Some("macOS"));
        assert!(!f.get("shell.name").unwrap().starts_with('-'));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gather_reads_real_hardware_on_linux() {
        let f = gather();
        assert!(f.get("mem.kb").unwrap().parse::<u64>().unwrap() > 1_000_000);
        assert!(f.get("cpu.cores").unwrap().parse::<u32>().unwrap() >= 1);
        assert!(f.get("disk.used_pct").unwrap().parse::<u32>().unwrap() <= 100);
        assert!(f.get("disk.size_gb").unwrap().parse::<u64>().unwrap() > 0);
        assert!(!f.get("os.name").unwrap().is_empty());
        assert!(!f.get("shell.name").unwrap().starts_with('-'));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gather_is_fast() {
        let t = std::time::Instant::now();
        let _ = gather();
        assert!(
            t.elapsed().as_millis() < 50,
            "gather took {:?}",
            t.elapsed()
        );
    }

    #[test]
    fn terminal_name_prefers_term_program_and_falls_back_to_term() {
        assert_eq!(
            terminal_name(Some("ghostty"), Some("xterm-256color")),
            Some("ghostty".to_string())
        );
        assert_eq!(
            terminal_name(None, Some("xterm-256color")),
            Some("xterm-256color".to_string())
        );
        assert_eq!(terminal_name(None, None), None);
    }

    #[test]
    fn terminal_name_treats_an_empty_value_as_unset() {
        assert_eq!(
            terminal_name(Some(""), Some("xterm-256color")),
            Some("xterm-256color".to_string())
        );
        assert_eq!(terminal_name(Some(""), Some("")), None);
    }

    #[test]
    fn kb_to_whole_gb_rounds_to_the_nearest_gigabyte() {
        assert_eq!(kb_to_whole_gb(37_748_736), 36);
        assert_eq!(kb_to_whole_gb(0), 0);
    }

    #[test]
    fn ghostty_theme_reads_the_top_level_theme_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(
            &path,
            "font-size = 14\ntheme = rainbows-and-unicorns-mane\n",
        )
        .unwrap();
        assert_eq!(
            ghostty_theme(&[path]),
            Some("rainbows-and-unicorns-mane".to_string())
        );
    }

    #[test]
    fn ghostty_theme_is_none_without_a_theme_line_or_a_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "font-size = 14\n").unwrap();
        assert_eq!(ghostty_theme(&[path]), None);
        assert_eq!(ghostty_theme(&[dir.path().join("missing")]), None);
    }

    #[test]
    fn ghostty_theme_is_none_when_the_line_names_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "theme =   \n").unwrap();
        assert_eq!(ghostty_theme(&[path]), None);
    }

    #[test]
    fn ghostty_theme_picks_the_candidate_pick_config_path_would_pick() {
        let dir = tempfile::tempdir().unwrap();
        let no_theme = dir.path().join("no_theme");
        let has_theme = dir.path().join("has_theme");
        std::fs::write(&no_theme, "font-size = 14\n").unwrap();
        std::fs::write(&has_theme, "theme = rainbows-and-unicorns\n").unwrap();
        assert_eq!(
            ghostty_theme(&[no_theme, has_theme]),
            Some("rainbows-and-unicorns".to_string())
        );
    }
}
