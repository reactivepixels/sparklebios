# Changelog

All notable changes to SparkleBIOS are recorded here, in the manner of a service
log kept in a binder next to the machine.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The design, the voice rulebook, the theme palettes and the roadmap.
- The crate scaffold: the `bios` binary, its command surface, the zsh hook, and a panic policy that never lets an error reach your terminal.
- POST, the continuous integration workflow, which is exactly what it sounds like.
- A dependency policy that bans network crates outright.
- The "Rainbows and Unicorns" Ghostty theme in four variants, with a test that fails if the files and `docs/theme.md` ever disagree. Install with `bios theme install`.
- Two boot machines, `pc95` and `pc85`, as TOML data, each with a pool of rotating quips.
- Fast macOS fact probes (processor, memory, disk, system, shell, measured shell start time) that never spawn a process. A whole boot takes about 2.5ms.
- `bios boot`: the boot mode decision (off, quiet, fast, full), a boot-day streak, the `SPARKLEBIOS_BOOT=0` kill switch, and previews with `--machine`, `--full` and `--fast`.
- A code of conduct (the Contributor Covenant) and the social preview artwork, with its source.
- The mark: a pixel unicorn with a gold horn and a six-stripe mane, on the banner, the social preview and as `docs/assets/mark.svg`.
- `bios init zsh`, `bios machines`, and the machine authoring guide in `docs/machines.md`.
