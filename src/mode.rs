//! The BootMode decision, a pure function.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Off,
    Quiet,
    Fast,
    Full,
}

#[derive(Debug, Clone)]
pub struct BootInputs {
    pub stdout_is_tty: bool,
    pub term: Option<String>,
    pub kill_switch: Option<String>,
    pub already_booted: bool,
    pub now: u64,
    pub today: String,
    pub last_boot: Option<u64>,
    pub last_full_day: Option<String>,
    pub burst_window_secs: u64,
}

pub fn decide(i: &BootInputs) -> BootMode {
    let off = !i.stdout_is_tty
        || i.term.as_deref() == Some("dumb")
        || i.kill_switch.as_deref() == Some("0")
        || i.already_booted;
    if off {
        return BootMode::Off;
    }

    let within_burst = match i.last_boot {
        Some(last_boot) if last_boot <= i.now => i.now - last_boot <= i.burst_window_secs,
        _ => false,
    };
    if within_burst {
        return BootMode::Quiet;
    }

    if i.last_full_day.as_deref() != Some(i.today.as_str()) {
        return BootMode::Full;
    }

    BootMode::Fast
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> BootInputs {
        BootInputs {
            stdout_is_tty: true,
            term: Some("xterm-ghostty".into()),
            kill_switch: None,
            already_booted: false,
            now: 1_000_000,
            today: "2026-09-19".into(),
            last_boot: Some(1_000_000 - 3600),
            last_full_day: Some("2026-09-19".into()),
            burst_window_secs: 10,
        }
    }
    #[test]
    fn fast_on_an_ordinary_new_tab() {
        assert_eq!(decide(&base()), BootMode::Fast);
    }
    #[test]
    fn full_on_the_first_boot_of_the_day() {
        assert_eq!(
            decide(&BootInputs {
                last_full_day: Some("2026-09-18".into()),
                ..base()
            }),
            BootMode::Full
        );
        assert_eq!(
            decide(&BootInputs {
                last_full_day: None,
                last_boot: None,
                ..base()
            }),
            BootMode::Full
        );
    }
    #[test]
    fn quiet_inside_the_burst_window() {
        assert_eq!(
            decide(&BootInputs {
                last_boot: Some(1_000_000 - 4),
                ..base()
            }),
            BootMode::Quiet
        );
    }
    #[test]
    fn quiet_beats_full() {
        assert_eq!(
            decide(&BootInputs {
                last_boot: Some(1_000_000 - 4),
                last_full_day: None,
                ..base()
            }),
            BootMode::Quiet
        );
    }
    #[test]
    fn a_last_boot_in_the_future_is_ignored() {
        assert_eq!(
            decide(&BootInputs {
                last_boot: Some(1_000_000 + 500),
                ..base()
            }),
            BootMode::Fast
        );
    }
    #[test]
    fn off_conditions() {
        assert_eq!(
            decide(&BootInputs {
                stdout_is_tty: false,
                ..base()
            }),
            BootMode::Off
        );
        assert_eq!(
            decide(&BootInputs {
                term: Some("dumb".into()),
                ..base()
            }),
            BootMode::Off
        );
        assert_eq!(
            decide(&BootInputs {
                kill_switch: Some("0".into()),
                ..base()
            }),
            BootMode::Off
        );
        assert_eq!(
            decide(&BootInputs {
                already_booted: true,
                ..base()
            }),
            BootMode::Off
        );
    }
    #[test]
    fn kill_switch_only_means_zero() {
        assert_eq!(
            decide(&BootInputs {
                kill_switch: Some("1".into()),
                ..base()
            }),
            BootMode::Fast
        );
    }
}
