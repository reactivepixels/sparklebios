use assert_cmd::Command;
use predicates::prelude::*;

/// Every test runs against an empty config and state directory, so the
/// developer's own `config.toml` can never change which flavour boots.
fn bios() -> Command {
    let sandbox = std::env::temp_dir().join("sparklebios-test-sandbox");
    let mut cmd = Command::cargo_bin("bios").unwrap();
    cmd.env("XDG_CONFIG_HOME", sandbox.join("config"))
        .env("XDG_STATE_HOME", sandbox.join("state"));
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

#[test]
fn help_and_bare_invocation_print_the_exact_top_level_help() {
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!(
        r#"SparkleBIOS {version}
A 1995 POST screen for your terminal that is secretly a health check.

Usage: bios <COMMAND>

Everyday:
  boot               Play the boot screen now
  flavours           List the personalities you can boot as
  use <FLAVOUR>      Boot as that flavour from now on
  theme list         List the matching Ghostty themes
  theme use <NAME>   Install the themes and switch Ghostty to one

Setup:
  init zsh           Print the hook. Add this to the end of ~/.zshrc:
                     command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
  theme install      Install the theme files without switching

Try:
  bios boot --flavour sumo     Preview a flavour without changing anything
  bios use sumo                Make it permanent
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
fn hook_is_valid_zsh() {
    let zsh = std::process::Command::new("zsh")
        .args(["-n", "shell/init.zsh"])
        .status();
    if let Ok(status) = zsh {
        assert!(status.success(), "shell/init.zsh has a syntax error");
    }
}

#[test]
fn boot_prints_nothing_when_stdout_is_not_a_tty() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .arg("boot")
        .env("XDG_STATE_HOME", state.path())
        .assert()
        .success()
        .stdout("");
    assert!(!state.path().join("sparklebios/state.json").exists());
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

#[test]
fn preview_omits_the_streak_line() {
    bios()
        .args(["boot", "--flavour", "unicorn"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"))
        .stdout(predicate::str::contains("Boot streak").not())
        .stdout(predicate::str::contains("{").not());
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
            "Ghostty theme : rainbows-and-unicorns-mane\nReload Ghostty's config or restart the terminal to see it.\n",
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
            "Ghostty theme : rainbows-and-unicorns-paper\nReload Ghostty's config or restart the terminal to see it.\n",
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
