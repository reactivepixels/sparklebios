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
- Flavours: one screen, swappable personality. A flavour is a data file and a sprite that supply the mascot, the firmware and vendor wording, one signature detect line, the streak wording, the footer code and the quips. Ships with `unicorn` (the default) and `sumo`. `bios flavours`, `bios use --flavour <id>`, `bios boot --flavour <id>`.
- Painted screens: a machine can paint itself as a block of its own background colour, with a border, so a boot looks like a screen and not like text. Falls back to plain text when the terminal is too narrow or has no truecolor.
- The unicorn, in the terminal: real pixels through the kitty graphics protocol where the terminal supports it (Ghostty, kitty), half-block characters everywhere else. `pc95` gains its logo and a compliance badge.
- The show: the POST now plays. The memory count ticks up, each device is detected in turn, and any key skips to the end. Keys typed during the show are handed back to the prompt, and the terminal is never left in raw mode. `--no-animate`, `SPARKLEBIOS_ANIMATE=0` or `animate = false` turn it off.
- Transparent painted screens: a machine with `paint = true` and no `bg` keeps its layout and logo but sits on the terminal's own background. `pc95` and `pc85` now do this, so they belong to whatever theme you run. `c64` keeps its blue.
- The `c64` machine: forty columns, light blue on blue, one word changed.
- `bios use <id>` chooses which machine boots, with `--full`, `--fast` and `--reset`. It edits `~/.config/sparklebios/config.toml` for you and leaves every other setting alone.
- Mane, a fifth theme variant built from the unicorn's own colours, made to sit under `pc95`. Opt in with `theme = rainbows-and-unicorns-mane`.
- `extras/starship-palette.toml`, a starship palette to match the theme.
- `bios init zsh`, `bios machines`, and the machine authoring guide in `docs/machines.md`.
