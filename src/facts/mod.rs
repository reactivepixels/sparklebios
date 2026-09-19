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
}
