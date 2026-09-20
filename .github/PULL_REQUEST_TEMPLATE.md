## What this changes

<!-- One or two sentences. What and why. -->

## POST checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass
- [ ] No em dashes or en dashes anywhere
- [ ] No new dependency, or the pull request says why it earns its place
- [ ] Nothing on the boot path spawns a process, touches the network, or can return a non-zero exit code
- [ ] Every new line a screen can show follows `docs/voice.md`, and anything that reads like a diagnostic is true
- [ ] New wording and art are original: no real logos, wordmarks or copied copyright strings
- [ ] `CHANGELOG.md` has an entry under Unreleased

## Screens

<!-- If this changes what anyone sees, paste the output of `bios boot --flavour <id> --no-animate`. -->
