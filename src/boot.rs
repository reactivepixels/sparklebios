//! The boot flow: decide, gather, render, print, save.

use std::io::{IsTerminal, Write};

use crate::mode::BootMode;

pub struct BootArgs {
    pub machine: Option<String>,
    pub full: bool,
    pub fast: bool,
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn debug(msg: impl FnOnce() -> String) {
    if env_var("SPARKLEBIOS_DEBUG").as_deref() == Some("1") {
        eprintln!("{}", msg());
    }
}

fn seed_from_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn write_stdout(output: &str) {
    let mut stdout = std::io::stdout();
    if stdout.write_all(output.as_bytes()).is_err() {
        debug(|| "bios: failed to write the boot screen".to_string());
    }
}

fn color_mode() -> crate::render::ColorMode {
    crate::render::color_mode_from_env(
        env_var("NO_COLOR").as_deref(),
        env_var("COLORTERM").as_deref(),
    )
}

/// Never panics outward and never returns an error: a boot that cannot happen prints nothing.
pub fn run(args: &BootArgs) {
    if args.machine.is_some() || args.full || args.fast {
        run_preview(args);
    } else {
        run_real();
    }
}

fn run_preview(args: &BootArgs) {
    let user_dir = crate::paths::user_machines_dir();
    let machine = if let Some(id) = &args.machine {
        crate::machine::find(id, user_dir.as_deref())
    } else {
        let config = crate::config::load(crate::paths::config_dir().as_deref());
        let id = if args.full {
            &config.full
        } else {
            &config.fast
        };
        crate::machine::find(id, user_dir.as_deref()).or_else(|| {
            crate::machine::find(if args.full { "pc95" } else { "pc85" }, user_dir.as_deref())
        })
    };
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };
    let facts = crate::facts::gather();
    let output = crate::render::render_static(
        &machine,
        &facts,
        color_mode(),
        seed_from_time(),
        crate::term::cols(1),
    );
    write_stdout(&output);
}

fn run_real() {
    let stdout_is_tty = std::io::stdout().is_terminal();
    let now = crate::clock::now_unix();
    let today = crate::clock::day_string(now as i64);
    let state_dir = crate::paths::state_dir();
    let mut state = match &state_dir {
        Some(dir) => crate::state::State::load(dir),
        None => crate::state::State::default(),
    };

    let inputs = crate::mode::BootInputs {
        stdout_is_tty,
        term: env_var("TERM"),
        kill_switch: env_var("SPARKLEBIOS_BOOT"),
        already_booted: env_var("SPARKLEBIOS_BOOTED").is_some(),
        now,
        today: today.clone(),
        last_boot: state.last_boot,
        last_full_day: state.last_full_day.clone(),
        burst_window_secs: 10,
    };
    let decision = crate::mode::decide(&inputs);
    if matches!(decision, BootMode::Off | BootMode::Quiet) {
        return;
    }

    let config = crate::config::load(crate::paths::config_dir().as_deref());
    let id = if decision == BootMode::Full {
        &config.full
    } else {
        &config.fast
    };
    let user_dir = crate::paths::user_machines_dir();
    let machine = crate::machine::find(id, user_dir.as_deref()).or_else(|| {
        crate::machine::find(
            if decision == BootMode::Full {
                "pc95"
            } else {
                "pc85"
            },
            user_dir.as_deref(),
        )
    });
    let Some(machine) = machine else {
        debug(|| "bios: unknown machine".to_string());
        return;
    };

    let yesterday = crate::clock::day_string(now as i64 - 86_400);
    state.advance_streak(&today, &yesterday);
    let mut facts = crate::facts::gather();
    facts.insert("streak.days", state.streak_days.to_string());
    facts.insert("streak.label", state.streak_label());

    let output = crate::render::render_static(
        &machine,
        &facts,
        color_mode(),
        seed_from_time(),
        crate::term::cols(1),
    );
    write_stdout(&output);

    state.last_boot = Some(now);
    if decision == BootMode::Full {
        state.last_full_day = Some(today);
    }
    if let Some(dir) = &state_dir {
        if let Err(e) = state.save(dir) {
            debug(|| format!("bios: failed to save state: {e}"));
        }
    }
}
