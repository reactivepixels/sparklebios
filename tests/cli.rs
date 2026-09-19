use assert_cmd::Command;
use predicates::prelude::*;

fn bios() -> Command {
    Command::cargo_bin("bios").unwrap()
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
        .stdout(predicate::str::contains("bios boot"))
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
        .stdout(predicate::str::contains("K OK"))
        .stdout(predicate::str::contains(
            "The UNICORN Personal Computer Basic",
        ))
        .stdout(predicate::str::contains("\x1b").not());
    assert!(!state.path().join("sparklebios").exists());
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
        .stdout(predicate::str::contains("pc85"));
}

#[test]
fn theme_install_writes_four_files() {
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
    ] {
        assert!(dir.path().join(name).is_file(), "{name} missing");
    }
}
