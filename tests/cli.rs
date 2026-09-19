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
