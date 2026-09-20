use assert_cmd::Command;
use predicates::prelude::*;

/// Every test runs against an empty config and state directory, so the
/// developer's own `config.toml` can never change which flavour boots.
fn bios() -> Command {
    let sandbox = std::env::temp_dir().join("sparklebios-test-sandbox");
    let mut cmd = Command::cargo_bin("bios").unwrap();
    cmd.env("HOME", &sandbox)
        .env("XDG_CONFIG_HOME", sandbox.join("config"))
        .env("XDG_STATE_HOME", sandbox.join("state"))
        .env("XDG_CACHE_HOME", sandbox.join("cache"))
        .env("XDG_DATA_HOME", sandbox.join("data"))
        // A real $STARSHIP_CONFIG in the ambient environment must never leak into a test: theme
        // use tests that care about it set it themselves.
        .env_remove("STARSHIP_CONFIG");
    cmd
}

#[test]
fn version_prints_crate_version() {
    bios()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}

/// `main.rs`'s whole panic policy: a panic anywhere prints nothing and exits 0, so a bug on the
/// boot path never breaks or slows somebody's shell. Triggered here through
/// `SPARKLEBIOS_PANIC_TEST`, a debug-only test hatch documented on `cli::run`; it does not exist
/// in a release build.
#[test]
fn a_panic_prints_nothing_and_exits_zero_without_debug() {
    bios()
        .env("SPARKLEBIOS_PANIC_TEST", "1")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

/// The other half of the same policy: with `SPARKLEBIOS_DEBUG=1`, the panic message reaches
/// stderr instead of being swallowed, so a developer can see what broke.
#[test]
fn a_panic_reaches_stderr_under_debug() {
    bios()
        .env("SPARKLEBIOS_PANIC_TEST", "1")
        .env("SPARKLEBIOS_DEBUG", "1")
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("SPARKLEBIOS_PANIC_TEST"));
}

#[test]
fn help_and_bare_invocation_print_the_exact_top_level_help() {
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!(
        r#"SparkleBIOS {version}
Boot every terminal tab like a 1995 PC. It counts your RAM, detects a unicorn,
and runs a real health check.

Usage: bios <COMMAND>

Everyday:
  boot               Play the boot screen now
  fetch              Show your machine at a glance
  resume             Change to the project you left work in
  refresh            Run the health checks again now
  flavours           List the personalities you can boot as
  use <FLAVOUR>      Boot as that flavour from now on
  flavour new <ID>   Start your own flavour from a working template
  sprinkles [LEVEL]  Optional effects: off, light or full
  theme list         List the matching Ghostty themes
  theme use <NAME>   Install the themes and switch Ghostty to one

Install:
  init zsh|bash|fish Print the hook. For zsh, add this to the end of ~/.zshrc:
                     command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
  theme install      Install the theme files without switching
  setup              The CMOS Setup Utility. Blue. Arrow keys. You remember.
  config edit        Open the config file in your editor
  config path        Print where the config file lives
  config reset       Factory defaults. The unicorn will be notified.

Try:
  bios boot --flavour sumo     Preview a flavour without changing anything
  bios use sumo                Make it permanent
  bios resume                  Go back to the project you left work in
  bios use                     Show which flavour is set
  bios theme use mane          Switch Ghostty to the Mane theme
  SPARKLEBIOS_BOOT=0           Set this in a shell to stop it booting there

Options:
  -h, --help         Print help
  -V, --version      Print version
"#
    );
    for args in [vec!["--help"], vec!["help"], Vec::<&str>::new()] {
        bios()
            .args(&args)
            .assert()
            .success()
            .stdout(expected.clone());
    }
}

#[test]
fn init_zsh_prints_the_hook() {
    bios()
        .args(["init", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("print -z"))
        .stdout(predicate::str::contains("SPARKLEBIOS_BOOTED"));
}

#[test]
fn init_zsh_defines_the_resume_wrapper_and_keeps_the_boot_hook_call() {
    bios()
        .args(["init", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("bios() {"))
        .stdout(predicate::str::contains("command bios resume"))
        .stdout(predicate::str::contains("builtin cd --"));
}

#[test]
fn hook_is_valid_zsh() {
    for (shell, template) in shells() {
        parses(shell, &std::fs::read_to_string(template).unwrap(), template);
    }
}

/// The three shells and the template each one is printed from.
fn shells() -> [(&'static str, &'static str); 3] {
    [
        ("zsh", "shell/init.zsh"),
        ("bash", "shell/init.bash"),
        ("fish", "shell/init.fish"),
    ]
}

/// Asserts `text` parses as `shell`. Skips silently where the shell is not installed, so the
/// suite still runs on a machine without fish.
fn parses(shell: &str, text: &str, what: &str) {
    let path = std::env::temp_dir().join(format!("sparklebios-hook-{shell}-{:x}", hash(what)));
    std::fs::write(&path, text).unwrap();
    let run = std::process::Command::new(shell)
        .arg("-n")
        .arg(&path)
        .output();
    let Ok(out) = run else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "{what} is not valid {shell}:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// The template parsing is only half of it: what a user evals is the template with the flavour's
/// own words written into it, and any of those slots can be empty. Every combination has to parse,
/// because a broken hook takes the whole shell down at startup.
#[test]
fn the_hook_parses_in_its_shell_with_every_combination_of_slots_filled_in() {
    use sparklebios::shell::{render, Mascot, Shell};

    let png = sparklebios::sprite::builtin("ninja").unwrap();
    let flavours: Vec<Option<sparklebios::flavour::Flavour>> = std::iter::once(None)
        .chain(sparklebios::flavour::builtins().into_iter().map(Some))
        .collect();

    for ((name, template), shell) in
        shells()
            .into_iter()
            .zip([Shell::Zsh, Shell::Bash, Shell::Fish])
    {
        for flavour in &flavours {
            for presence in [true, false] {
                for title in [true, false] {
                    for mascot in [Mascot::none(), Mascot::kitty(png, 7, 1, shell.wrap())] {
                        let config = sparklebios::config::Config {
                            presence,
                            title,
                            ..Default::default()
                        };
                        let out = render(shell, &config, flavour.as_ref(), mascot);
                        assert!(!out.contains("{{"), "a slot was left in {template}");
                        let id = flavour.as_ref().map_or("none", |f| f.id.as_str());
                        parses(name, &out, &format!("{template} as {id}"));
                    }
                }
            }
        }
    }
}

#[test]
fn init_bash_prints_the_hook() {
    bios()
        .args(["init", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("SPARKLEBIOS_BOOTED"));
}

#[test]
fn init_bash_defines_the_resume_wrapper_and_keeps_the_boot_hook_call() {
    bios()
        .args(["init", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("bios() {"))
        .stdout(predicate::str::contains("command bios resume"))
        .stdout(predicate::str::contains("builtin cd --"));
}

#[test]
fn hook_is_valid_bash() {
    let bash = std::process::Command::new("bash")
        .args(["-n", "shell/init.bash"])
        .status();
    if let Ok(status) = bash {
        assert!(status.success(), "shell/init.bash has a syntax error");
    }
}

#[test]
fn init_fish_prints_the_hook() {
    bios()
        .args(["init", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("SPARKLEBIOS_BOOTED"));
}

#[test]
fn init_fish_defines_the_resume_wrapper_and_keeps_the_boot_hook_call() {
    bios()
        .args(["init", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bios boot --hook"))
        .stdout(predicate::str::contains("function bios"))
        .stdout(predicate::str::contains("command bios resume"))
        .stdout(predicate::str::contains("builtin cd --"));
}

#[test]
fn hook_is_valid_fish() {
    let fish = std::process::Command::new("fish")
        .args(["--no-execute", "shell/init.fish"])
        .status();
    if let Ok(status) = fish {
        assert!(status.success(), "shell/init.fish has a syntax error");
    }
}

/// `bios boot` typed by hand is a viewing command, not the real boot: stdout not being a tty only
/// stops the show from animating (see `boot_draws_the_final_screen_even_when_stdout_is_not_a_tty`
/// below), it never suppresses the screen the way the real boot's `Off` decision does.
#[test]
fn boot_draws_the_final_screen_even_when_stdout_is_not_a_tty() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .arg("boot")
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
    assert!(!state.path().join("sparklebios/state.json").exists());
}

/// The bug: the shell hook exports `SPARKLEBIOS_BOOTED=1` so a shell started inside another shell
/// does not boot twice, but a hand typed `bios boot` must ignore it entirely and still draw.
#[test]
fn boot_with_the_booted_env_var_set_still_draws_the_screen() {
    bios()
        .arg("boot")
        .env("SPARKLEBIOS_BOOTED", "1")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

/// A hand typed `bios boot` ignores the burst window entirely: two runs back to back both draw
/// the full screen, unlike two real boots inside the same shell startup.
#[test]
fn boot_twice_in_a_row_both_draw_the_full_screen() {
    for _ in 0..2 {
        bios()
            .arg("boot")
            .env("NO_COLOR", "1")
            .assert()
            .success()
            .stdout(predicate::str::contains("Sparkle Modular BIOS"));
    }
}

/// A hand typed `bios boot` never creates or modifies the state file: absent stays absent, and an
/// existing file comes out byte for byte the same.
#[test]
fn boot_never_creates_or_modifies_the_state_file() {
    let state = tempfile::tempdir().unwrap();
    let state_file = state.path().join("sparklebios/state.json");

    bios()
        .arg("boot")
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success();
    assert!(!state_file.exists());

    std::fs::create_dir_all(state_file.parent().unwrap()).unwrap();
    let before = br#"{"last_boot":1000,"last_full_day":"2026-09-19","streak_days":5,"streak_last_day":"2026-09-19"}"#;
    std::fs::write(&state_file, before).unwrap();

    bios()
        .arg("boot")
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success();

    assert_eq!(std::fs::read(&state_file).unwrap(), before);
}

/// `bios boot --hook`, unlike the hand typed viewing command above, still honours
/// `SPARKLEBIOS_BOOTED` (part of the `Off` decision `mode::decide` makes; see `mode.rs`'s
/// `off_conditions` test for the full set) and so never writes the state file either.
#[test]
fn hook_still_honours_the_booted_env_var_and_never_writes_state() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .args(["boot", "--hook"])
        .env("XDG_STATE_HOME", state.path())
        .env("SPARKLEBIOS_BOOTED", "1")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout("");
    assert!(!state.path().join("sparklebios/state.json").exists());
}

/// `SPARKLEBIOS_BOOT=0` is the kill switch a shell sets to stop `bios boot --hook` from ever
/// drawing a screen there (see the top level help). Like `SPARKLEBIOS_BOOTED` above, only the
/// hook honours it: the hand typed `bios boot` is a viewing command and ignores it on purpose
/// (see `run_preview`'s doc comment in `boot.rs`), so this is pinned against `--hook`.
#[test]
fn sparklebios_boot_zero_silences_the_hook() {
    bios()
        .args(["boot", "--hook"])
        .env("SPARKLEBIOS_BOOT", "0")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn preview_with_flavour_renders_pc95_without_touching_state() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .args(["boot", "--flavour", "unicorn"])
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"))
        .stdout(predicate::str::contains("\x1b").not());
    assert!(!state.path().join("sparklebios").exists());
}

/// The memory count needs a hardware probe, and only macOS has probes so far.
#[cfg(target_os = "macos")]
#[test]
fn preview_shows_the_memory_count_on_macos() {
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("K OK"));
}

/// A streak of zero is not a streak, so the line is left out rather than printed as
/// "Boot streak: 0 days", which reads as a broken counter. This can only be reached by an
/// explicit `bios boot` before the shell hook has ever run: a hooked boot counts as day one.
/// The master switch in the config file, the persistent twin of `SPARKLEBIOS_BOOT=0`. The hook
/// path is the one that honours it: an explicit `bios boot` is a viewing command and still draws.
#[test]
fn boot_false_in_the_config_silences_the_hook() {
    let home = tempfile::tempdir().unwrap();
    let cfg = home.path().join("sparklebios");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(cfg.join("config.toml"), "boot = false\n").unwrap();
    bios()
        .args(["boot", "--hook"])
        .env("XDG_CONFIG_HOME", home.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn the_streak_line_is_omitted_when_there_is_no_state_file_yet() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .args(["boot", "--flavour", "unicorn"])
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"))
        .stdout(predicate::str::contains("Boot streak").not())
        .stdout(predicate::str::contains("{").not());
    assert!(!state.path().join("sparklebios/state.json").exists());
}

/// The streak line shows the streak already on disk, read without advancing it: a preview never
/// increments it, and never writes it back either.
#[test]
fn preview_shows_the_stored_streak_without_advancing_it() {
    let state = tempfile::tempdir().unwrap();
    let state_dir = state.path().join("sparklebios");
    std::fs::create_dir_all(&state_dir).unwrap();
    let before = br#"{"last_boot":1000,"last_full_day":"2026-09-19","streak_days":5,"streak_last_day":"2026-09-19"}"#;
    std::fs::write(state_dir.join("state.json"), before).unwrap();

    bios()
        .args(["boot", "--flavour", "unicorn"])
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Boot streak: 5 days"));

    assert_eq!(std::fs::read(state_dir.join("state.json")).unwrap(), before);
}

#[test]
fn preview_omits_shell_boot_time() {
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Shell boot time").not());
}

#[test]
fn unknown_machine_is_silent_and_successful() {
    bios()
        .args(["boot", "--machine", "nope"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn preview_with_a_user_machine_file_renders_it() {
    let config = tempfile::tempdir().unwrap();
    let machines_dir = config.path().join("sparklebios/machines");
    std::fs::create_dir_all(&machines_dir).unwrap();
    std::fs::write(
        machines_dir.join("custom.toml"),
        "id = \"custom\"\nname = \"Custom\"\ncols = 40\nfg = \"#AAAAAA\"\nbright = \"#FFFFFF\"\naccent = \"#FFFF55\"\n[[step]]\nprint = \"A custom machine\"\n",
    )
    .unwrap();
    bios()
        .args(["boot", "--machine", "custom"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("A custom machine"));
}

#[test]
fn an_unknown_graphics_value_does_not_stop_the_boot_screen_from_printing() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "graphics = \"holographic\"\n").unwrap();
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

/// The retired `"blocks"` value must keep booting silently, read as `"auto"`: somebody's config
/// saying `blocks` cannot be allowed to error or warn.
#[test]
fn graphics_legacy_blocks_value_in_the_config_file_is_accepted() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "graphics = \"blocks\"\n").unwrap();
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

#[test]
fn graphics_off_in_the_config_file_is_accepted() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "graphics = \"off\"\n").unwrap();
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

#[test]
fn sparklebios_graphics_env_override_is_accepted() {
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("SPARKLEBIOS_GRAPHICS", "off")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

/// The retired `"blocks"` value, through the environment override too, must keep booting
/// silently, read as `"auto"`.
#[test]
fn sparklebios_graphics_env_override_accepts_the_legacy_blocks_value() {
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("SPARKLEBIOS_GRAPHICS", "blocks")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

#[test]
fn sparklebios_sprinkles_env_override_is_accepted() {
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("SPARKLEBIOS_SPRINKLES", "full")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"));
}

#[test]
fn hook_prints_nothing_to_stdout_and_exits_zero() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .args(["boot", "--hook"])
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout("");
}

#[test]
fn no_animate_flag_is_accepted() {
    bios()
        .args(["boot", "--machine", "pc95", "--no-animate"])
        .env("NO_COLOR", "1")
        .assert()
        .success();
}

#[test]
fn use_with_no_id_prints_the_current_flavour() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Flavour : unicorn\n");
}

#[test]
fn use_with_an_unknown_id_fails_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "nope"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .failure()
        .code(1)
        .stderr("bios: no flavour called nope. Try: bios flavours\n");
    assert!(!config.path().join("sparklebios/config.toml").exists());
}

#[test]
fn use_then_boot_preview_picks_up_the_chosen_flavour() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Yokozuna Modular BIOS v1.991, Immovable",
        ));
}

#[test]
fn sprinkles_with_no_level_prints_the_current_level() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .arg("sprinkles")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Sprinkles : off\n");
}

#[test]
fn sprinkles_light_prints_the_tasteful_line_then_the_preview_hint_and_persists() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["sprinkles", "light"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Sprinkles : light. Tasteful.\nPreview it now: bios boot\n");
    bios()
        .arg("sprinkles")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Sprinkles : light\n");
}

#[test]
fn sprinkles_full_prints_you_asked_for_this_then_the_preview_hint() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["sprinkles", "full"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Sprinkles : full. You asked for this.\nPreview it now: bios boot\n");
}

#[test]
fn sprinkles_off_prints_the_bios_is_not_hurt_line_with_no_preview_hint() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["sprinkles", "off"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Sprinkles : off. The BIOS respects your decision and is not hurt.\n");
}

#[test]
fn sprinkles_with_an_unknown_level_fails_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["sprinkles", "sparkly"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .failure()
        .code(1)
        .stderr("bios: no sprinkle level called sparkly. Try: off, light, full\n");
    assert!(!config.path().join("sparklebios/config.toml").exists());
}

#[test]
fn sprinkles_setting_the_level_preserves_the_configured_flavour() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["sprinkles", "full"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Flavour : sumo\n");
}

#[test]
fn fetch_prints_the_default_flavours_firmware_line_and_name() {
    bios()
        .arg("fetch")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Sparkle Modular BIOS v1.985PG, An Enchantment Star Ally",
        ))
        .stdout(predicate::str::contains("Flavour   : Unicorn"));
}

#[test]
fn fetch_picks_up_the_configured_flavour() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .arg("fetch")
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Flavour   : Sumo"))
        .stdout(predicate::str::contains(
            "Yokozuna Modular BIOS v1.991, Immovable",
        ));
}

#[test]
fn fetch_omits_the_theme_line_without_a_ghostty_config() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    bios()
        .arg("fetch")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Theme").not());
}

#[test]
fn fetch_shows_the_theme_a_ghostty_config_names() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let ghostty_dir = config.path().join("ghostty");
    std::fs::create_dir_all(&ghostty_dir).unwrap();
    std::fs::write(
        ghostty_dir.join("config"),
        "theme = rainbows-and-unicorns-mane\n",
    )
    .unwrap();
    bios()
        .arg("fetch")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Theme     : Mane"));
}

#[cfg(target_os = "macos")]
#[test]
fn fetch_shows_real_hardware_facts_on_macos() {
    bios()
        .arg("fetch")
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("OS        : macOS"))
        .stdout(predicate::str::contains("CPU       :"))
        .stdout(predicate::str::contains("Memory    :"));
}

#[test]
fn flavours_lists_both() {
    bios()
        .arg("flavours")
        .assert()
        .success()
        .stdout(predicate::str::contains("unicorn"))
        .stdout(predicate::str::contains("sumo"));
}

#[test]
fn boot_preview_with_flavour_sumo_prints_its_wording() {
    bios()
        .args(["boot", "--flavour", "sumo"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Yokozuna Modular BIOS v1.991, Immovable",
        ));
}

#[test]
fn use_flavour_sumo_then_boot_preview_picks_it_up_and_use_reports_it() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["boot", "--machine", "pc95"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Yokozuna Modular BIOS v1.991, Immovable",
        ));
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Flavour : sumo"));
}

#[test]
fn theme_install_writes_every_theme_file() {
    let dir = tempfile::tempdir().unwrap();
    bios()
        .args(["theme", "install", "--dir"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("bold-is-bright = false"));
    for name in [
        "rainbows-and-unicorns",
        "rainbows-and-unicorns-paper",
        "rainbows-and-unicorns-ega",
        "rainbows-and-unicorns-workbench",
        "rainbows-and-unicorns-mane",
        "rainbows-and-unicorns-miami",
        "rainbows-and-unicorns-arcade",
        "rainbows-and-unicorns-vhs",
        "rainbows-and-unicorns-den",
        "rainbows-and-unicorns-sorbet",
    ] {
        assert!(dir.path().join(name).is_file(), "{name} missing");
    }
}

#[test]
fn theme_list_prints_short_names_and_full_names_in_order() {
    let expected = [
        ("six", "rainbows-and-unicorns"),
        ("paper", "rainbows-and-unicorns-paper"),
        ("ega", "rainbows-and-unicorns-ega"),
        ("workbench", "rainbows-and-unicorns-workbench"),
        ("mane", "rainbows-and-unicorns-mane"),
        ("miami", "rainbows-and-unicorns-miami"),
        ("arcade", "rainbows-and-unicorns-arcade"),
        ("vhs", "rainbows-and-unicorns-vhs"),
        ("den", "rainbows-and-unicorns-den"),
        ("sorbet", "rainbows-and-unicorns-sorbet"),
    ]
    .iter()
    .map(|(short, full)| format!("{short:<11}{full}\n"))
    .collect::<String>();
    bios()
        .args(["theme", "list"])
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn theme_use_installs_files_and_sets_the_config_line() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    std::fs::write(
        &config_path,
        "font-size = 14\ntheme = old-theme\ncursor-style = block\n",
    )
    .unwrap();
    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(
            "Ghostty theme  : rainbows-and-unicorns-mane\nReload Ghostty's config or restart the terminal to see it.\n",
        );
    assert!(themes_dir
        .path()
        .join("rainbows-and-unicorns-mane")
        .is_file());
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        contents,
        "font-size = 14\ntheme = rainbows-and-unicorns-mane\ncursor-style = block\n"
    );
}

#[test]
fn theme_use_accepts_a_full_name() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    bios()
        .args(["theme", "use", "rainbows-and-unicorns-paper", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(
            "Ghostty theme  : rainbows-and-unicorns-paper\nReload Ghostty's config or restart the terminal to see it.\n",
        );
}

#[test]
fn theme_use_appends_the_theme_line_when_none_exists() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    std::fs::write(&config_path, "font-size = 14\n").unwrap();
    bios()
        .args(["theme", "use", "six", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success();
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(contents, "font-size = 14\ntheme = rainbows-and-unicorns\n");
}

#[test]
fn theme_use_creates_the_config_file_when_missing() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    bios()
        .args(["theme", "use", "ega", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "theme = rainbows-and-unicorns-ega\n"
    );
}

#[test]
fn theme_use_with_an_unknown_name_fails_and_writes_nothing() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    bios()
        .args(["theme", "use", "nope", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .assert()
        .failure()
        .code(1)
        .stderr("bios: no theme called nope. Try: bios theme list\n");
    assert!(!config_path.exists());
    assert!(!themes_dir.path().join("rainbows-and-unicorns").exists());
}

#[test]
fn theme_use_repoints_an_existing_starship_palette_and_appends_the_auto_table() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    let original =
        "format = \"$all\"\n\npalette = 'gruvbox_dark'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(
            "Ghostty theme  : rainbows-and-unicorns-mane\nPrompt palette : rainbows_and_unicorns_auto\nReload Ghostty's config or restart the terminal to see it.\n",
        );

    let contents = std::fs::read_to_string(&starship_path).unwrap();
    let expected_prefix =
        "format = \"$all\"\n\npalette = 'rainbows_and_unicorns_auto'\n\n[palettes.gruvbox_dark]\ncolor_fg0 = '#123456'\n";
    assert!(contents.starts_with(expected_prefix), "{contents}");
    assert_eq!(
        contents
            .matches("[palettes.rainbows_and_unicorns_auto]")
            .count(),
        1
    );
    // The appended table ends at its own last key line: no orphaned write-up about the next
    // table in the source file (a blank line then trailing comments) leaks in.
    assert!(contents.ends_with("color_yellow = '3'\n"), "{contents}");
    assert!(
        !contents.contains("rainbows_and_unicorns_paper"),
        "{contents}"
    );

    // The written file is valid TOML.
    let parsed: toml::Table = contents.parse().unwrap();
    assert!(parsed.contains_key("palettes"));

    // No temp file left behind next to the target.
    let entries: Vec<_> = std::fs::read_dir(starship_dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn theme_use_repoints_a_palette_line_that_follows_a_multiline_format_string() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    // Shaped like starship's real Gruvbox Rainbow preset: `format` is a triple-quoted
    // multi-line string full of `[...]` segment lines that are not TOML table headers.
    let original = "format = \"\"\"\n\
[](color_orange)\\\n\
$os\\\n\
[](bg:color_yellow fg:color_orange)\\\n\
$directory\\\n\
[ ](fg:color_bg1)\\\n\
$line_break$character\"\"\"\n\
\n\
palette = 'gruvbox_dark'\n\
\n\
[palettes.gruvbox_dark]\n\
color_fg0 = '#fbf1c7'\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Prompt palette : rainbows_and_unicorns_auto",
        ));

    let contents = std::fs::read_to_string(&starship_path).unwrap();
    assert!(
        contents.contains("\npalette = 'rainbows_and_unicorns_auto'\n"),
        "{contents}"
    );
    assert!(contents.contains("[palettes.rainbows_and_unicorns_auto]"));
    // The format string's segment syntax survives untouched.
    assert!(contents.contains("[](color_orange)\\\n"));
    // The written file is valid TOML.
    let parsed: toml::Table = contents.parse().unwrap();
    assert!(parsed.contains_key("palettes"));
}

#[test]
fn theme_use_paper_points_at_the_paper_palette() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    std::fs::write(&starship_path, "palette = 'gruvbox_dark'\n").unwrap();

    bios()
        .args(["theme", "use", "paper", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Prompt palette : rainbows_and_unicorns_paper",
        ));

    let contents = std::fs::read_to_string(&starship_path).unwrap();
    assert!(contents.starts_with("palette = 'rainbows_and_unicorns_paper'\n"));
    assert!(contents.contains("[palettes.rainbows_and_unicorns_paper]"));
}

#[test]
fn theme_use_does_not_duplicate_an_already_present_starship_table() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    std::fs::write(
        &starship_path,
        "palette = 'gruvbox_dark'\n\n[palettes.rainbows_and_unicorns_auto]\ncolor_fg0 = 'placeholder'\n",
    )
    .unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Prompt palette : rainbows_and_unicorns_auto",
        ));

    let contents = std::fs::read_to_string(&starship_path).unwrap();
    assert_eq!(
        contents
            .matches("[palettes.rainbows_and_unicorns_auto]")
            .count(),
        1
    );
    assert!(contents.contains("color_fg0 = 'placeholder'"));
    assert!(contents.starts_with("palette = 'rainbows_and_unicorns_auto'\n"));
}

#[test]
fn theme_use_leaves_a_starship_config_with_no_palette_line_untouched() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    let original = "format = \"$all\"\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Prompt palette").not());

    assert_eq!(std::fs::read_to_string(&starship_path).unwrap(), original);
}

#[test]
fn theme_use_ignores_a_palette_key_nested_in_a_table() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    let original = "[palettes.foo]\npalette = 'inner'\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Prompt palette").not());

    assert_eq!(std::fs::read_to_string(&starship_path).unwrap(), original);
}

#[test]
fn theme_use_ignores_a_commented_out_palette_line() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    let original = "# palette = 'gruvbox_dark'\nformat = \"$all\"\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Prompt palette").not());

    assert_eq!(std::fs::read_to_string(&starship_path).unwrap(), original);
}

#[test]
fn theme_use_with_a_missing_starship_config_switches_the_theme_and_prints_nothing_extra() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .assert()
        .success()
        .code(0)
        .stdout(
            "Ghostty theme  : rainbows-and-unicorns-mane\nReload Ghostty's config or restart the terminal to see it.\n",
        );

    assert!(!starship_path.exists());
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "theme = rainbows-and-unicorns-mane\n"
    );
}

#[test]
fn theme_use_no_prompt_leaves_the_starship_config_untouched() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    let original = "palette = 'gruvbox_dark'\n";
    std::fs::write(&starship_path, original).unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .arg("--starship-config")
        .arg(&starship_path)
        .arg("--no-prompt")
        .assert()
        .success()
        .stdout(predicate::str::contains("Prompt palette").not());

    assert_eq!(std::fs::read_to_string(&starship_path).unwrap(), original);
}

#[test]
fn theme_use_honours_the_starship_config_env_var_when_the_flag_is_absent() {
    let themes_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("config");
    let starship_dir = tempfile::tempdir().unwrap();
    let starship_path = starship_dir.path().join("starship.toml");
    std::fs::write(&starship_path, "palette = 'gruvbox_dark'\n").unwrap();

    bios()
        .args(["theme", "use", "mane", "--dir"])
        .arg(themes_dir.path())
        .arg("--config")
        .arg(&config_path)
        .env("STARSHIP_CONFIG", &starship_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Prompt palette : rainbows_and_unicorns_auto",
        ));

    let contents = std::fs::read_to_string(&starship_path).unwrap();
    assert!(contents.starts_with("palette = 'rainbows_and_unicorns_auto'\n"));
}

/// A fully isolated environment for the checks/cache commands: its own `HOME`, config, state and
/// cache directories, plus a `project_dirs` config entry pointing at a fresh temp project root
/// containing one bare repo.
struct RefreshSandbox {
    _home: tempfile::TempDir,
    config: tempfile::TempDir,
    _state: tempfile::TempDir,
    cache: tempfile::TempDir,
    _project_root: tempfile::TempDir,
    repo: std::path::PathBuf,
}

impl RefreshSandbox {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let repo = project_root.path().join("demo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let sparklebios_config = config.path().join("sparklebios");
        std::fs::create_dir_all(&sparklebios_config).unwrap();
        std::fs::write(
            sparklebios_config.join("config.toml"),
            format!("project_dirs = [\"{}\"]\n", project_root.path().display()),
        )
        .unwrap();
        RefreshSandbox {
            _home: home,
            config,
            _state: state,
            cache,
            _project_root: project_root,
            repo,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("bios").unwrap();
        cmd.env("HOME", self._home.path())
            .env("XDG_CONFIG_HOME", self.config.path())
            .env("XDG_STATE_HOME", self._state.path())
            .env("XDG_CACHE_HOME", self.cache.path());
        cmd
    }

    fn cache_file(&self) -> std::path::PathBuf {
        self.cache.path().join("sparklebios/facts.json")
    }
}

#[test]
fn refresh_print_prints_json_containing_boot_order() {
    let sandbox = RefreshSandbox::new();
    sandbox
        .cmd()
        .args(["refresh", "--print"])
        .assert()
        .success()
        .stdout(predicate::str::contains("boot_order"));
}

#[test]
fn a_second_refresh_immediately_after_is_a_no_op() {
    let sandbox = RefreshSandbox::new();
    sandbox.cmd().arg("refresh").assert().success();
    let first_mtime = std::fs::metadata(sandbox.cache_file())
        .unwrap()
        .modified()
        .unwrap();

    sandbox.cmd().arg("refresh").assert().success();
    let second_mtime = std::fs::metadata(sandbox.cache_file())
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(first_mtime, second_mtime);
}

#[test]
fn resume_prints_the_expected_absolute_path() {
    let sandbox = RefreshSandbox::new();
    sandbox
        .cmd()
        .arg("resume")
        .assert()
        .success()
        .stdout(format!("{}\n", sandbox.repo.display()));
}

#[test]
fn resume_with_no_candidate_fails_with_the_exact_message() {
    let home = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let empty_root = tempfile::tempdir().unwrap();
    let sparklebios_config = config.path().join("sparklebios");
    std::fs::create_dir_all(&sparklebios_config).unwrap();
    std::fs::write(
        sparklebios_config.join("config.toml"),
        format!("project_dirs = [\"{}\"]\n", empty_root.path().display()),
    )
    .unwrap();

    Command::cargo_bin("bios")
        .unwrap()
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", config.path())
        .env("XDG_STATE_HOME", state.path())
        .env("XDG_CACHE_HOME", cache.path())
        .arg("resume")
        .assert()
        .failure()
        .code(1)
        .stderr("bios: no boot device to resume.\n");
}

/// A fully isolated environment for the boot-path findings tests: its own `HOME`, config, state
/// and cache directories, with a `facts.json` this test seeds by hand rather than through a real
/// `bios refresh`.
struct FindingsSandbox {
    _home: tempfile::TempDir,
    config: tempfile::TempDir,
    _state: tempfile::TempDir,
    cache: tempfile::TempDir,
}

impl FindingsSandbox {
    fn new() -> Self {
        FindingsSandbox {
            _home: tempfile::tempdir().unwrap(),
            config: tempfile::tempdir().unwrap(),
            _state: tempfile::tempdir().unwrap(),
            cache: tempfile::tempdir().unwrap(),
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("bios").unwrap();
        cmd.env("HOME", self._home.path())
            .env("XDG_CONFIG_HOME", self.config.path())
            .env("XDG_STATE_HOME", self._state.path())
            .env("XDG_CACHE_HOME", self.cache.path());
        cmd
    }

    /// Writes `<cache>/sparklebios/facts.json` with a single fresh `boot_order` finding,
    /// `generated` at `now - age_secs`.
    fn seed_boot_order(&self, age_secs: u64) {
        let dir = self.cache.path().join("sparklebios");
        std::fs::create_dir_all(&dir).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let generated = now.saturating_sub(age_secs);
        let json = format!(
            r#"{{"generated":{generated},"findings":[{{"id":"boot_order","severity":"info","ttl":57600,"facts":{{"boot.devices":"eko-pro, sparklebios"}}}}]}}"#
        );
        std::fs::write(dir.join("facts.json"), json).unwrap();
    }

    /// Writes `<config>/sparklebios/config.toml` with `checks = false`.
    fn disable_checks(&self) {
        let dir = self.config.path().join("sparklebios");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "checks = false\n").unwrap();
    }
}

#[test]
fn a_fresh_finding_in_the_cache_reaches_the_boot_screen() {
    let sandbox = FindingsSandbox::new();
    sandbox.seed_boot_order(0);
    sandbox
        .cmd()
        .args(["boot", "--machine", "pc95", "--no-animate"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Boot device order: eko-pro, sparklebios",
        ));
}

#[test]
fn the_sumo_flavour_phrases_the_same_finding_differently() {
    let sandbox = FindingsSandbox::new();
    sandbox.seed_boot_order(0);
    sandbox
        .cmd()
        .args([
            "boot",
            "--machine",
            "pc95",
            "--flavour",
            "sumo",
            "--no-animate",
        ])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Bout order: eko-pro, sparklebios"))
        .stdout(predicate::str::contains("Boot device order:").not());
}

#[test]
fn a_finding_past_its_own_ttl_is_not_shown() {
    let sandbox = FindingsSandbox::new();
    // boot_order's ttl is 57600 seconds (16 hours); generated well past that.
    sandbox.seed_boot_order(100_000);
    sandbox
        .cmd()
        .args(["boot", "--machine", "pc95", "--no-animate"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Boot device order:").not());
}

#[test]
fn config_path_prints_the_expected_path() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "path"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(format!(
            "{}\n",
            config.path().join("sparklebios/config.toml").display()
        ));
}

#[test]
fn config_path_prints_the_expected_path_when_the_file_is_absent() {
    let config = tempfile::tempdir().unwrap();
    assert!(!config.path().join("sparklebios/config.toml").exists());
    bios()
        .args(["config", "path"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(format!(
            "{}\n",
            config.path().join("sparklebios/config.toml").display()
        ));
}

#[test]
fn config_reset_writes_the_template_and_prints_the_exact_line() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Factory defaults restored. The unicorn has been notified.\n");
    let contents = std::fs::read_to_string(config.path().join("sparklebios/config.toml")).unwrap();
    assert_eq!(contents, sparklebios::config::TEMPLATE);
}

#[test]
fn config_reset_over_a_different_existing_file_leaves_a_backup_with_the_old_contents() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "flavour = \"sumo\"\n").unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let backup = std::fs::read_to_string(dir.join("config.toml.bak")).unwrap();
    assert_eq!(backup, "flavour = \"sumo\"\n");
}

#[test]
fn config_reset_over_a_file_identical_to_the_template_writes_no_backup() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), sparklebios::config::TEMPLATE).unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    assert!(!dir.join("config.toml.bak").exists());
}

#[test]
fn config_reset_leaves_no_temp_file_behind() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "flavour = \"sumo\"\n").unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec!["config.toml", "config.toml.bak"]);
}

#[test]
fn config_edit_creates_the_file_from_the_template_when_absent() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "edit"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("VISUAL", "/usr/bin/true")
        .assert()
        .success();
    let contents = std::fs::read_to_string(config.path().join("sparklebios/config.toml")).unwrap();
    assert_eq!(contents, sparklebios::config::TEMPLATE);
}

#[test]
fn config_edit_does_not_overwrite_an_existing_files_contents() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "flavour = \"sumo\"\n").unwrap();
    bios()
        .args(["config", "edit"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("VISUAL", "/usr/bin/true")
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(contents, "flavour = \"sumo\"\n");
}

#[test]
fn config_edit_prefers_visual_over_editor() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "edit"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("VISUAL", "/usr/bin/true")
        .env("EDITOR", "/usr/bin/false")
        .assert()
        .success();
}

#[test]
fn config_edit_returns_non_zero_when_the_editor_exits_non_zero() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "edit"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("VISUAL", "/usr/bin/false")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn config_reset_then_use_preserves_every_comment_and_key_order_and_reads_back_the_new_flavour() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let path = config.path().join("sparklebios/config.toml");
    let comment_lines = sparklebios::config::TEMPLATE
        .lines()
        .filter(|line| line.starts_with('#'))
        .count();
    assert_eq!(
        std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with('#'))
            .count(),
        comment_lines
    );

    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();

    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        contents
            .lines()
            .filter(|line| line.starts_with('#'))
            .count(),
        comment_lines,
        "a comment was lost:\n{contents}"
    );
    let keys: Vec<&str> = contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.split('=').next().unwrap().trim())
        .collect();
    assert_eq!(
        keys,
        vec![
            "flavour",
            "animate",
            "graphics",
            "checks",
            "project_dirs",
            "sprinkles",
            "boot",
            "presence",
            "presence_after",
            "title"
        ]
    );
    assert!(contents.contains("flavour = \"sumo\""));
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Flavour : sumo\n");
}

#[test]
fn config_reset_then_sprinkles_light_preserves_every_comment() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["config", "reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let path = config.path().join("sparklebios/config.toml");
    let comment_lines = sparklebios::config::TEMPLATE
        .lines()
        .filter(|line| line.starts_with('#'))
        .count();

    bios()
        .args(["sprinkles", "light"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();

    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        contents
            .lines()
            .filter(|line| line.starts_with('#'))
            .count(),
        comment_lines,
        "a comment was lost:\n{contents}"
    );
    assert!(contents.contains("sprinkles = \"light\""));
}

#[test]
fn use_appends_flavour_to_a_hand_written_file_missing_it_and_the_file_still_parses() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "animate = false\n").unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(contents, "animate = false\nflavour = \"sumo\"\n");
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Flavour : sumo\n");
}

#[test]
fn use_appends_the_new_flavour_line_before_a_table_header_rather_than_after_it() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "animate = false\n\n[extra]\nfoo = 1\n",
    )
    .unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(
        contents,
        "animate = false\n\nflavour = \"sumo\"\n[extra]\nfoo = 1\n"
    );
}

#[test]
fn use_does_not_mistake_a_flavour_line_inside_a_table_for_the_top_level_one() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "[extra]\nflavour = \"inner\"\n").unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(
        contents,
        "flavour = \"sumo\"\n[extra]\nflavour = \"inner\"\n"
    );
}

#[test]
fn use_does_not_mistake_a_commented_out_flavour_line_for_the_real_one() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "# flavour = \"old\"\nanimate = true\n",
    )
    .unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(
        contents,
        "# flavour = \"old\"\nanimate = true\nflavour = \"sumo\"\n"
    );
}

#[test]
fn use_keeps_the_flavour_lines_original_leading_whitespace() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "  flavour = \"unicorn\"\n").unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert_eq!(contents, "  flavour = \"sumo\"\n");
}

#[test]
fn use_preserves_the_trailing_newline_and_leaves_no_temp_file_behind() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "flavour = \"unicorn\"\n").unwrap();
    bios()
        .args(["use", "sumo"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let contents = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert!(contents.ends_with('\n'));
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["config.toml".to_string()]);
}

#[test]
fn checks_false_shows_no_findings() {
    let sandbox = FindingsSandbox::new();
    sandbox.seed_boot_order(0);
    sandbox.disable_checks();
    sandbox
        .cmd()
        .args(["boot", "--machine", "pc95", "--no-animate"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Boot device order:").not());
}

#[test]
fn prompt_with_ghostty_prints_one_placement_and_one_transmission() {
    let assert = bios()
        .arg("prompt")
        .env("TERM", "xterm-ghostty")
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.matches("a=p").count(), 1, "{stdout}");
    assert_eq!(stdout.matches("a=t").count(), 1, "{stdout}");
}

#[test]
fn prompt_with_the_prompt_img_var_set_prints_the_placement_and_no_transmission() {
    let assert = bios()
        .arg("prompt")
        .env("TERM", "xterm-ghostty")
        .env("SPARKLEBIOS_PROMPT_IMG", "1")
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.matches("a=p").count(), 1, "{stdout}");
    assert!(!stdout.contains("a=t"), "{stdout}");
}

#[test]
fn prompt_with_no_image_protocol_prints_nothing_and_exits_zero() {
    bios()
        .arg("prompt")
        .env("TERM", "xterm-256color")
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .assert()
        .success()
        .stdout("");
}

#[test]
fn init_zsh_with_ghostty_defines_the_mascot_variable_with_exactly_one_placement() {
    let assert = bios()
        .args(["init", "zsh"])
        .env("TERM", "xterm-ghostty")
        .env_remove("TERM_PROGRAM")
        .env_remove("LC_TERMINAL")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.matches("a=p").count(), 1, "{stdout}");
    // The transmission that sends the image appears exactly once in the whole hook.
    assert_eq!(stdout.matches("a=t").count(), 1, "{stdout}");

    let mascot_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("export SPARKLEBIOS_MASCOT="))
        .unwrap();
    assert!(mascot_line.contains("%{"), "{mascot_line}");
    assert!(mascot_line.contains("%}"), "{mascot_line}");
    assert!(!mascot_line.contains("a=t"), "{mascot_line}");
}

#[test]
fn say_done_with_took_130_puts_the_worded_duration_in_the_line() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["say", "done", "--took", "130"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("2m 10s"));
}

#[test]
fn presence_false_in_the_config_silences_say() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "presence = false\n").unwrap();
    bios()
        .args(["say", "goodbye"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

/// The one that matters most: a template with a hole in it is the whole failure mode of
/// `bios flavour new`, and only booting it for real catches that.
#[test]
fn flavour_new_writes_a_template_that_boots_immediately() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["flavour", "new", "mine"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["boot", "--flavour", "mine", "--no-animate"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Mine Modular BIOS"));
}

#[test]
fn flavour_new_prints_the_exact_success_message_and_writes_the_file() {
    let config = tempfile::tempdir().unwrap();
    let path = config.path().join("sparklebios/flavours/mine.toml");
    bios()
        .args(["flavour", "new", "mine"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(format!(
            "Created {}\n\nOpen it and make it yours, then:\n  bios boot --flavour mine    Preview it\n  bios use mine               Boot as it from now on\n",
            path.display()
        ));
    assert!(path.is_file());
}

#[test]
fn flavour_new_rejects_ids_that_do_not_match_the_shape_rule_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    for id in ["Bad-Id", "1abc", "has_underscore", "UNICORN2", ""] {
        bios()
            .args(["flavour", "new", id])
            .env("XDG_CONFIG_HOME", config.path())
            .assert()
            .failure()
            .code(1)
            .stderr(
                "bios: a flavour id is lowercase letters, digits and hyphens, starting with a letter.\n",
            );
    }
    assert!(!config.path().join("sparklebios").exists());
}

#[test]
fn flavour_new_rejects_an_id_starting_with_a_hyphen_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["flavour", "new", "--"])
        .arg("-abc")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .failure()
        .code(1)
        .stderr(
            "bios: a flavour id is lowercase letters, digits and hyphens, starting with a letter.\n",
        );
    assert!(!config.path().join("sparklebios").exists());
}

/// The id rule is what has to catch these, since it runs before any path is ever built: an id
/// with a path separator, a leading dot or `..` must never reach a join with the flavours
/// directory.
#[test]
fn flavour_new_refuses_path_traversal_ids_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    for id in ["../../etc/passwd", "..", ".hidden", "a/b"] {
        bios()
            .args(["flavour", "new", id])
            .env("XDG_CONFIG_HOME", config.path())
            .assert()
            .failure()
            .code(1)
            .stderr(
                "bios: a flavour id is lowercase letters, digits and hyphens, starting with a letter.\n",
            );
    }
    assert!(!config.path().join("sparklebios").exists());
}

#[test]
fn flavour_new_rejects_a_built_in_id_and_writes_nothing() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["flavour", "new", "unicorn"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .failure()
        .code(1)
        .stderr("bios: unicorn is a built-in flavour. Pick another name.\n");
    assert!(!config.path().join("sparklebios").exists());
}

#[test]
fn flavour_new_refuses_to_overwrite_an_existing_file() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("sparklebios/flavours");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mine.toml"), "already here\n").unwrap();
    bios()
        .args(["flavour", "new", "mine"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .failure()
        .code(1)
        .stderr(format!(
            "bios: {} already exists. Delete it or pick another name.\n",
            dir.join("mine.toml").display()
        ));
    assert_eq!(
        std::fs::read_to_string(dir.join("mine.toml")).unwrap(),
        "already here\n"
    );
}

#[test]
fn flavour_new_writes_a_file_that_parses_and_has_every_presence_event() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["flavour", "new", "my-flavour"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    let path = config.path().join("sparklebios/flavours/my-flavour.toml");
    let src = std::fs::read_to_string(&path).unwrap();
    let flavour = sparklebios::flavour::parse(&src).unwrap();
    assert_eq!(flavour.id, "my-flavour");
    assert_eq!(flavour.name, "My Flavour");
    assert!(flavour.quips.len() >= 3);
    for event in sparklebios::presence::EVENTS {
        assert!(
            flavour.presence.contains_key(event),
            "missing presence event {event}"
        );
        assert!(!flavour.presence[event].is_empty());
    }
}

#[test]
fn flavour_new_then_flavours_lists_it_by_id_and_name() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["flavour", "new", "my-flavour"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .arg("flavours")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-flavour"))
        .stdout(predicate::str::contains("My Flavour"));
}
