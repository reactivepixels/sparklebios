use assert_cmd::Command;
use predicates::prelude::*;

/// Every test runs against an empty config and state directory, so the
/// developer's own `config.toml` can never change which machine boots.
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
fn preview_renders_pc85_without_touching_state() {
    let state = tempfile::tempdir().unwrap();
    bios()
        .args(["boot", "--fast"])
        .env("XDG_STATE_HOME", state.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "The UNICORN Personal Computer Basic",
        ))
        .stdout(predicate::str::contains("\x1b").not());
    assert!(!state.path().join("sparklebios").exists());
}

/// The memory count needs a hardware probe, and only macOS has probes so far.
#[cfg(target_os = "macos")]
#[test]
fn preview_shows_the_memory_count_on_macos() {
    bios()
        .args(["boot", "--fast"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("K OK"));
}

#[test]
fn preview_full_renders_pc95_and_omits_the_streak_line() {
    bios()
        .args(["boot", "--full"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sparkle Modular BIOS"))
        .stdout(predicate::str::contains("Boot streak").not())
        .stdout(predicate::str::contains("{").not());
}

#[test]
fn preview_full_omits_shell_boot_time() {
    bios()
        .args(["boot", "--full"])
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
fn full_and_fast_conflict() {
    bios().args(["boot", "--full", "--fast"]).assert().failure();
}

#[test]
fn machines_lists_the_roster() {
    bios()
        .arg("machines")
        .assert()
        .success()
        .stdout(predicate::str::contains("pc95"))
        .stdout(predicate::str::contains("pc85"))
        .stdout(predicate::str::contains("c64"));
}

#[test]
fn preview_renders_c64() {
    bios()
        .args(["boot", "--machine", "c64"])
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("**** UNICORN 64 BASIC V2 ****"));
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
        .args(["boot", "--fast", "--no-animate"])
        .env("NO_COLOR", "1")
        .assert()
        .success();
}

#[test]
fn use_sets_both_full_and_fast() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "c64"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .arg("use")
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Full show  : c64\nEvery boot : c64\n");
}

#[test]
fn use_fast_leaves_the_full_show_at_the_default() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "c64", "--fast"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Full show  : pc95\nEvery boot : c64\n");
}

#[test]
fn use_reset_returns_to_the_defaults() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "c64"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["use", "--reset"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout("Full show  : pc95\nEvery boot : pc85\n");
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
        .stderr("bios: no machine called nope. Try: bios machines\n");
    assert!(!config.path().join("sparklebios/config.toml").exists());
}

#[test]
fn use_then_boot_fast_picks_up_the_chosen_machine() {
    let config = tempfile::tempdir().unwrap();
    bios()
        .args(["use", "c64"])
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success();
    bios()
        .args(["boot", "--fast"])
        .env("XDG_CONFIG_HOME", config.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("**** UNICORN 64 BASIC V2 ****"));
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
    ] {
        assert!(dir.path().join(name).is_file(), "{name} missing");
    }
}
