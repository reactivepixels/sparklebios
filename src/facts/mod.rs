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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_known_and_missing_facts() {
        let f = Facts::fixture();
        assert_eq!(f.get("mem.kb"), Some("37748736"));
        assert_eq!(f.get("nope"), None);
    }
}
