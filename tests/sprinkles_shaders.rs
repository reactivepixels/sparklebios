//! `bios sprinkles ultra` and dropping back down: the two shaders (`extras/shaders/scanlines.glsl`
//! and `extras/shaders/cursor-trail.glsl`) turning on and off in a Ghostty config, the way
//! `theme.rs`'s own tests exercise `bios theme use` against a sandboxed config file.
//!
//! A separate file from `tests/cli.rs` (owned elsewhere during this pass), with its own copy of
//! the same sandboxing helper: every command below points `HOME` and all four `XDG_` variables at
//! a fresh temp directory, on the same command line as the command itself, so nothing here ever
//! touches a real Ghostty config.

use assert_cmd::Command;
use predicates::prelude::*;

const ENABLED_LINE: &str =
    "Scanlines and a cursor trail are in your Ghostty config. Reload it to see them.";
const DISABLED_LINE: &str = "Scanlines and the cursor trail are out of your Ghostty config.";

/// Every test runs against a throwaway config, state, cache and data directory, the same
/// sandboxing `tests/cli.rs`'s own `bios()` helper does.
fn bios(sandbox: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("bios").unwrap();
    cmd.env("HOME", sandbox)
        .env("XDG_CONFIG_HOME", sandbox.join("config"))
        .env("XDG_STATE_HOME", sandbox.join("state"))
        .env("XDG_CACHE_HOME", sandbox.join("cache"))
        .env("XDG_DATA_HOME", sandbox.join("data"))
        .env_remove("STARSHIP_CONFIG");
    cmd
}

#[test]
fn ultra_adds_both_shaders_to_an_existing_ghostty_config_and_prints_the_line() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    std::fs::write(&config_path, "font-size = 14\n").unwrap();

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(ENABLED_LINE));

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.starts_with("font-size = 14\n"));
    assert_eq!(contents.matches("custom-shader = ").count(), 2);
    assert!(contents.contains("scanlines.glsl"));
    assert!(contents.contains("cursor-trail.glsl"));
}

#[test]
fn ultra_run_twice_does_not_duplicate_the_shader_lines() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    std::fs::write(&config_path, "font-size = 14\n").unwrap();

    for _ in 0..2 {
        bios(sandbox.path())
            .args(["sprinkles", "ultra", "--ghostty-config"])
            .arg(&config_path)
            .assert()
            .success();
    }

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(contents.matches("custom-shader = ").count(), 2);
}

#[test]
fn dropping_from_ultra_to_off_removes_exactly_the_two_lines_and_restores_the_original() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    let original = "font-size = 14\ntheme = old-theme\ncursor-style = block\n";
    std::fs::write(&config_path, original).unwrap();

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success();
    assert_ne!(std::fs::read_to_string(&config_path).unwrap(), original);

    bios(sandbox.path())
        .args(["sprinkles", "off", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(DISABLED_LINE));

    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);
}

#[test]
fn dropping_from_ultra_to_light_also_removes_the_two_lines() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    std::fs::write(&config_path, "font-size = 14\n").unwrap();

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success();
    bios(sandbox.path())
        .args(["sprinkles", "light", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(DISABLED_LINE));

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(contents, "font-size = 14\n");
}

#[test]
fn a_users_own_custom_shader_line_survives_both_directions() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    std::fs::write(&config_path, "custom-shader = /mine/own.glsl\n").unwrap();

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success();
    let with_ultra = std::fs::read_to_string(&config_path).unwrap();
    assert!(with_ultra.contains("custom-shader = /mine/own.glsl"));
    assert_eq!(with_ultra.matches("custom-shader = ").count(), 3);

    bios(sandbox.path())
        .args(["sprinkles", "full", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "custom-shader = /mine/own.glsl\n"
    );
}

#[test]
fn no_shaders_flag_skips_both_directions() {
    let sandbox = tempfile::tempdir().unwrap();
    let config_path = sandbox.path().join("ghostty-config");
    std::fs::write(&config_path, "font-size = 14\n").unwrap();

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--no-shaders", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(ENABLED_LINE).not())
        .stdout(predicate::str::contains(DISABLED_LINE).not());
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "font-size = 14\n"
    );

    bios(sandbox.path())
        .args(["sprinkles", "ultra", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success();
    bios(sandbox.path())
        .args(["sprinkles", "off", "--no-shaders", "--ghostty-config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(ENABLED_LINE).not())
        .stdout(predicate::str::contains(DISABLED_LINE).not());
    assert_eq!(
        std::fs::read_to_string(&config_path)
            .unwrap()
            .matches("custom-shader = ")
            .count(),
        2
    );
}

#[test]
fn a_missing_ghostty_config_is_silent_in_either_direction_and_still_exits_zero() {
    let sandbox = tempfile::tempdir().unwrap();
    // No `--ghostty-config` override and nothing at any of the real search candidates: the
    // sandbox's HOME and XDG_CONFIG_HOME point nowhere that has a Ghostty config at all.
    bios(sandbox.path())
        .args(["sprinkles", "ultra"])
        .assert()
        .success()
        .stdout(predicate::str::contains(ENABLED_LINE).not())
        .stdout(predicate::str::contains(DISABLED_LINE).not())
        .stdout(predicate::str::contains("Sprinkles : ultra"));

    bios(sandbox.path())
        .args(["sprinkles", "off"])
        .assert()
        .success()
        .stdout(predicate::str::contains(ENABLED_LINE).not())
        .stdout(predicate::str::contains(DISABLED_LINE).not());
}
