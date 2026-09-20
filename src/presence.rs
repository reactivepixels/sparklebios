//! What the flavour says away from the boot screen.
//!
//! The lines live in each flavour's `[presence]` table. Nothing here runs per prompt: the shell
//! hook is given the finished strings when it is generated, so the shell never spawns `bios` to
//! speak. This module is what puts a line together, and `bios say` is its way in for the shells
//! we do not emit, and for tests.

use crate::flavour::Flavour;

/// The moments a flavour has words for. A user flavour may leave any of them out, and then that
/// moment is simply silent for it.
pub const EVENTS: [&str; 8] = [
    "title",
    "done",
    "failed",
    "goodbye",
    "not_found",
    "refresh",
    "resume",
    "switched",
];

/// The flavour's line for `event`, with `{took}`, `{device}` and `{cmd}` filled in. `None` when
/// the flavour has nothing to say for it, or when a slot it uses was not supplied, under the same
/// omission rule the boot screen uses: better silent than half a sentence.
pub fn line(flavour: &Flavour, event: &str, slots: &[(&str, &str)]) -> Option<String> {
    let template = flavour.presence.get(event)?;
    let mut facts = crate::facts::Facts::new();
    for (key, value) in slots {
        facts.insert(key, *value);
    }
    crate::template::render(template, &facts)
}

/// A duration, worded the way a person would say it: `14s`, `2m 10s`, `1h 4m`.
pub fn duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    if seconds < 3600 {
        return format!("{}m {}s", seconds / 60, seconds % 60);
    }
    format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unicorn() -> Flavour {
        crate::flavour::find("unicorn", None).unwrap()
    }

    #[test]
    fn a_line_with_no_slots_comes_back_as_written() {
        assert_eq!(
            line(&unicorn(), "goodbye", &[]).as_deref(),
            Some("Unicorn: goodbye. Stay sparkly.")
        );
    }

    #[test]
    fn a_slot_is_filled_from_what_the_caller_supplies() {
        assert_eq!(
            line(&unicorn(), "done", &[("took", "2m 10s")]).as_deref(),
            Some("Unicorn: done in 2m 10s. Sparkles settled.")
        );
        assert_eq!(
            line(&unicorn(), "resume", &[("device", "side-project")]).as_deref(),
            Some("Unicorn: booting side-project.")
        );
    }

    #[test]
    fn a_line_whose_slot_was_not_supplied_is_silent_rather_than_half_written() {
        assert_eq!(line(&unicorn(), "done", &[]), None);
    }

    #[test]
    fn an_event_the_flavour_has_no_words_for_is_silent() {
        assert_eq!(line(&unicorn(), "nonsense", &[]), None);
    }

    #[test]
    fn every_built_in_flavour_has_every_event() {
        for f in crate::flavour::builtins() {
            for event in EVENTS {
                assert!(
                    f.presence.contains_key(event),
                    "{} has nothing to say for {event}",
                    f.id
                );
                assert!(!f.presence[event].is_empty(), "{}'s {event} is empty", f.id);
            }
        }
    }

    #[test]
    fn every_line_fits_eighty_columns_with_realistic_values() {
        let slots = [
            ("took", "2m 10s"),
            ("device", "side-project"),
            ("cmd", "gti"),
        ];
        for f in crate::flavour::builtins() {
            for event in EVENTS {
                let rendered = line(&f, event, &slots).unwrap();
                assert!(
                    rendered.chars().count() <= 80,
                    "{} {event} is {} columns: {rendered}",
                    f.id,
                    rendered.chars().count()
                );
            }
        }
    }

    #[test]
    fn the_board_names_are_all_different() {
        let mut titles: Vec<String> = crate::flavour::builtins()
            .iter()
            .map(|f| f.presence["title"].clone())
            .collect();
        let before = titles.len();
        titles.sort();
        titles.dedup();
        assert_eq!(titles.len(), before, "two flavours share a board name");
    }

    #[test]
    fn a_duration_is_worded_the_way_a_person_would_say_it() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(14), "14s");
        assert_eq!(duration(59), "59s");
        assert_eq!(duration(60), "1m 0s");
        assert_eq!(duration(130), "2m 10s");
        assert_eq!(duration(3599), "59m 59s");
        assert_eq!(duration(3600), "1h 0m");
        assert_eq!(duration(3840), "1h 4m");
    }
}
